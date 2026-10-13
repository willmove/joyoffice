//! Persistent B-tree sequence with cached summaries (ARCHITECTURE §2, ADR-003).
//!
//! Cloning a [`SeqTree`] is O(1). An edit copies only the O(log n) nodes on the
//! path to the change, so a snapshot taken before the edit keeps sharing every
//! other node. Index and summary lookups descend the tree in O(log n).

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

/// Data cached on every node and combined in sequence order.
pub trait Summary: Clone + Default {
    fn add_summary(&mut self, other: &Self);
}

impl Summary for () {
    fn add_summary(&mut self, _: &Self) {}
}

/// A value stored in a [`SeqTree`]. `summary` should be cheap: it runs on
/// every node refresh along an edited path.
pub trait Item: Clone {
    type Summary: Summary;
    fn summary(&self) -> Self::Summary;
}

const MAX_FANOUT: usize = 16;
const MIN_FANOUT: usize = MAX_FANOUT / 2;

#[derive(Clone)]
enum Node<T: Item> {
    Leaf {
        items: Vec<T>,
        summary: T::Summary,
    },
    Internal {
        children: Vec<Arc<Node<T>>>,
        len: usize,
        summary: T::Summary,
    },
}

impl<T: Item> Node<T> {
    fn leaf(items: Vec<T>) -> Self {
        let mut node = Node::Leaf {
            items,
            summary: T::Summary::default(),
        };
        node.refresh();
        node
    }

    fn internal(children: Vec<Arc<Node<T>>>) -> Self {
        let mut node = Node::Internal {
            children,
            len: 0,
            summary: T::Summary::default(),
        };
        node.refresh();
        node
    }

    fn len(&self) -> usize {
        match self {
            Node::Leaf { items, .. } => items.len(),
            Node::Internal { len, .. } => *len,
        }
    }

    fn summary(&self) -> &T::Summary {
        match self {
            Node::Leaf { summary, .. } | Node::Internal { summary, .. } => summary,
        }
    }

    fn fanout(&self) -> usize {
        match self {
            Node::Leaf { items, .. } => items.len(),
            Node::Internal { children, .. } => children.len(),
        }
    }

    /// Recomputes the cached length and summary from direct children.
    fn refresh(&mut self) {
        match self {
            Node::Leaf { items, summary } => {
                let mut total = T::Summary::default();
                for item in items.iter() {
                    total.add_summary(&item.summary());
                }
                *summary = total;
            }
            Node::Internal {
                children,
                len,
                summary,
            } => {
                let mut total = T::Summary::default();
                for child in children.iter() {
                    total.add_summary(child.summary());
                }
                *len = children.iter().map(|child| child.len()).sum();
                *summary = total;
            }
        }
    }
}

/// Child containing `index`, which must be `< total`. Returns (child, offset).
fn child_at<T: Item>(children: &[Arc<Node<T>>], index: usize) -> (usize, usize) {
    let mut offset = 0;
    for (position, child) in children.iter().enumerate() {
        if index < offset + child.len() {
            return (position, offset);
        }
        offset += child.len();
    }
    unreachable!("index {index} is outside a node of length {offset}")
}

/// Child that receives an insertion at `index <= total`. Returns (child, offset).
fn child_for_insert<T: Item>(children: &[Arc<Node<T>>], index: usize) -> (usize, usize) {
    let mut offset = 0;
    let last = children.len() - 1;
    for (position, child) in children.iter().enumerate() {
        if index <= offset + child.len() || position == last {
            return (position, offset);
        }
        offset += child.len();
    }
    unreachable!("internal nodes always have children")
}

/// Splits `items` into the fewest chunks of at most MAX_FANOUT whose sizes
/// differ by at most one, so every chunk has at least MIN_FANOUT when there
/// are two or more.
fn chunk_evenly<X>(items: Vec<X>) -> Vec<Vec<X>> {
    let count = items.len().div_ceil(MAX_FANOUT).max(1);
    let base = items.len() / count;
    let extra = items.len() % count;
    let mut chunks = Vec::with_capacity(count);
    let mut iter = items.into_iter();
    for chunk in 0..count {
        let size = base + usize::from(chunk < extra);
        chunks.push(iter.by_ref().take(size).collect());
    }
    chunks
}

fn insert_rec<T: Item>(node: &mut Arc<Node<T>>, index: usize, item: T) -> Option<Arc<Node<T>>> {
    let node = Arc::make_mut(node);
    let overflow = match node {
        Node::Leaf { items, .. } => {
            items.insert(index, item);
            (items.len() > MAX_FANOUT).then(|| {
                let right = items.split_off(items.len() / 2);
                Arc::new(Node::leaf(right))
            })
        }
        Node::Internal { children, .. } => {
            let (position, offset) = child_for_insert(children, index);
            if let Some(split) = insert_rec(&mut children[position], index - offset, item) {
                children.insert(position + 1, split);
            }
            (children.len() > MAX_FANOUT).then(|| {
                let right = children.split_off(children.len() / 2);
                Arc::new(Node::internal(right))
            })
        }
    };
    node.refresh();
    overflow
}

fn remove_rec<T: Item>(node: &mut Arc<Node<T>>, index: usize) -> T {
    let node = Arc::make_mut(node);
    let removed = match node {
        Node::Leaf { items, .. } => items.remove(index),
        Node::Internal { children, .. } => {
            let (position, offset) = child_at(children, index);
            let removed = remove_rec(&mut children[position], index - offset);
            if children[position].fanout() < MIN_FANOUT {
                rebalance(children, position);
            }
            removed
        }
    };
    node.refresh();
    removed
}

fn update_rec<T: Item, R>(
    node: &mut Arc<Node<T>>,
    index: usize,
    update: impl FnOnce(&mut T) -> R,
) -> R {
    let node = Arc::make_mut(node);
    let result = match node {
        Node::Leaf { items, .. } => update(&mut items[index]),
        Node::Internal { children, .. } => {
            let (position, offset) = child_at(children, index);
            update_rec(&mut children[position], index - offset, update)
        }
    };
    node.refresh();
    result
}

/// Restores the minimum fan-out of `children[position]` by borrowing from or
/// merging with a sibling on the same level.
fn rebalance<T: Item>(children: &mut Vec<Arc<Node<T>>>, position: usize) {
    if position > 0 && children[position - 1].fanout() > MIN_FANOUT {
        let (left, right) = children.split_at_mut(position);
        let from = Arc::make_mut(&mut left[position - 1]);
        let to = Arc::make_mut(&mut right[0]);
        match (&mut *from, &mut *to) {
            (Node::Leaf { items: a, .. }, Node::Leaf { items: b, .. }) => {
                b.insert(0, a.pop().expect("sibling above minimum"));
            }
            (Node::Internal { children: a, .. }, Node::Internal { children: b, .. }) => {
                b.insert(0, a.pop().expect("sibling above minimum"));
            }
            _ => unreachable!("siblings are on the same level"),
        }
        from.refresh();
        to.refresh();
    } else if position + 1 < children.len() && children[position + 1].fanout() > MIN_FANOUT {
        let (left, right) = children.split_at_mut(position + 1);
        let to = Arc::make_mut(&mut left[position]);
        let from = Arc::make_mut(&mut right[0]);
        match (&mut *from, &mut *to) {
            (Node::Leaf { items: a, .. }, Node::Leaf { items: b, .. }) => b.push(a.remove(0)),
            (Node::Internal { children: a, .. }, Node::Internal { children: b, .. }) => {
                b.push(a.remove(0));
            }
            _ => unreachable!("siblings are on the same level"),
        }
        from.refresh();
        to.refresh();
    } else if position > 0 {
        let node = children.remove(position);
        append_node(Arc::make_mut(&mut children[position - 1]), node);
    } else if position + 1 < children.len() {
        let node = children.remove(position + 1);
        append_node(Arc::make_mut(&mut children[position]), node);
    }
}

fn append_node<T: Item>(into: &mut Node<T>, other: Arc<Node<T>>) {
    match (&mut *into, Arc::unwrap_or_clone(other)) {
        (Node::Leaf { items: a, .. }, Node::Leaf { items: b, .. }) => a.extend(b),
        (Node::Internal { children: a, .. }, Node::Internal { children: b, .. }) => a.extend(b),
        _ => unreachable!("siblings are on the same level"),
    }
    into.refresh();
}

/// Persistent sequence; see the module documentation.
pub struct SeqTree<T: Item> {
    root: Option<Arc<Node<T>>>,
}

impl<T: Item> Clone for SeqTree<T> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
        }
    }
}

impl<T: Item> Default for SeqTree<T> {
    fn default() -> Self {
        Self { root: None }
    }
}

impl<T: Item + fmt::Debug> fmt::Debug for SeqTree<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: Item> FromIterator<T> for SeqTree<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::from_vec(iter.into_iter().collect())
    }
}

impl<T: Item> SeqTree<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a balanced tree bottom-up in O(n).
    pub fn from_vec(items: Vec<T>) -> Self {
        if items.is_empty() {
            return Self::new();
        }
        let mut level: Vec<Arc<Node<T>>> = chunk_evenly(items)
            .into_iter()
            .map(|chunk| Arc::new(Node::leaf(chunk)))
            .collect();
        while level.len() > 1 {
            level = chunk_evenly(level)
                .into_iter()
                .map(|chunk| Arc::new(Node::internal(chunk)))
                .collect();
        }
        Self { root: level.pop() }
    }

    pub fn len(&self) -> usize {
        self.root.as_ref().map_or(0, |root| root.len())
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn summary(&self) -> T::Summary {
        self.root
            .as_ref()
            .map(|root| root.summary().clone())
            .unwrap_or_default()
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        let mut node = self.root.as_deref()?;
        if index >= node.len() {
            return None;
        }
        let mut index = index;
        loop {
            match node {
                Node::Leaf { items, .. } => return items.get(index),
                Node::Internal { children, .. } => {
                    let (position, offset) = child_at(children, index);
                    node = &children[position];
                    index -= offset;
                }
            }
        }
    }

    pub fn first(&self) -> Option<&T> {
        self.get(0)
    }

    pub fn last(&self) -> Option<&T> {
        self.len().checked_sub(1).and_then(|index| self.get(index))
    }

    pub fn iter(&self) -> Iter<'_, T> {
        let mut iter = Iter {
            stack: Vec::new(),
            leaf: [].iter(),
        };
        if let Some(root) = self.root.as_deref() {
            iter.descend(root);
        }
        iter
    }

    pub fn push(&mut self, item: T) {
        self.insert(self.len(), item);
    }

    /// Inserts before `index`. Panics if `index > len`, like `Vec::insert`.
    pub fn insert(&mut self, index: usize, item: T) {
        assert!(index <= self.len(), "insert index {index} out of bounds");
        let Some(root) = self.root.as_mut() else {
            self.root = Some(Arc::new(Node::leaf(vec![item])));
            return;
        };
        if let Some(split) = insert_rec(root, index, item) {
            let left = self.root.take().expect("root exists");
            self.root = Some(Arc::new(Node::internal(vec![left, split])));
        }
    }

    /// Removes the item at `index`. Panics if out of bounds, like `Vec::remove`.
    pub fn remove(&mut self, index: usize) -> T {
        assert!(index < self.len(), "remove index {index} out of bounds");
        let root = self.root.as_mut().expect("non-empty tree has a root");
        let removed = remove_rec(root, index);
        let collapse = match root.as_ref() {
            Node::Leaf { items, .. } => items.is_empty().then_some(None),
            Node::Internal { children, .. } => {
                (children.len() == 1).then(|| Some(children[0].clone()))
            }
        };
        if let Some(replacement) = collapse {
            self.root = replacement;
        }
        removed
    }

    /// Replaces the item at `index`, returning the previous value.
    pub fn replace(&mut self, index: usize, item: T) -> T {
        self.update(index, |slot| std::mem::replace(slot, item))
    }

    /// Mutates the item at `index` in place, copying only the path to it.
    pub fn update<R>(&mut self, index: usize, update: impl FnOnce(&mut T) -> R) -> R {
        assert!(index < self.len(), "update index {index} out of bounds");
        let root = self.root.as_mut().expect("non-empty tree has a root");
        update_rec(root, index, update)
    }

    /// Finds the item whose extent in `measure` contains `target`, together
    /// with the summary of every item before it. A target on a boundary
    /// resolves to the later item; `target == total` resolves to the last item.
    /// Returns `None` for an empty tree or a target beyond the total.
    pub fn locate(
        &self,
        target: usize,
        measure: impl Fn(&T::Summary) -> usize,
    ) -> Option<(usize, T::Summary)> {
        let mut node = self.root.as_deref()?;
        if target > measure(node.summary()) {
            return None;
        }
        let mut index = 0;
        let mut prefix = T::Summary::default();
        loop {
            match node {
                Node::Leaf { items, .. } => {
                    let last = items.len() - 1;
                    for (position, item) in items.iter().enumerate() {
                        let summary = item.summary();
                        if position == last || target < measure(&prefix) + measure(&summary) {
                            return Some((index + position, prefix));
                        }
                        prefix.add_summary(&summary);
                    }
                    unreachable!("leaves are never empty");
                }
                Node::Internal { children, .. } => {
                    let last = children.len() - 1;
                    for (position, child) in children.iter().enumerate() {
                        if position == last || target < measure(&prefix) + measure(child.summary())
                        {
                            node = child;
                            break;
                        }
                        prefix.add_summary(child.summary());
                        index += child.len();
                    }
                }
            }
        }
    }

    /// Number of nodes in `self` that are not shared with `base`. Used to
    /// verify that snapshots share structure instead of copying it.
    pub fn unshared_nodes(&self, base: &Self) -> usize {
        let mut shared = HashSet::new();
        let mut pending: Vec<&Arc<Node<T>>> = base.root.iter().collect();
        while let Some(node) = pending.pop() {
            shared.insert(Arc::as_ptr(node));
            if let Node::Internal { children, .. } = node.as_ref() {
                pending.extend(children.iter());
            }
        }
        let mut count = 0;
        let mut pending: Vec<&Arc<Node<T>>> = self.root.iter().collect();
        while let Some(node) = pending.pop() {
            if shared.contains(&Arc::as_ptr(node)) {
                continue;
            }
            count += 1;
            if let Node::Internal { children, .. } = node.as_ref() {
                pending.extend(children.iter());
            }
        }
        count
    }

    /// Whether both trees are the same version (pointer equality).
    pub fn ptr_eq(&self, other: &Self) -> bool {
        match (&self.root, &other.root) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }
}

impl<T: Item<Summary = ()>> SeqTree<T> {
    /// Mutable access with path copying. Limited to items without a summary,
    /// because cached summaries could not be refreshed once the borrow ends.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len() {
            return None;
        }
        let mut node = Arc::make_mut(self.root.as_mut()?);
        let mut index = index;
        loop {
            match node {
                Node::Leaf { items, .. } => return items.get_mut(index),
                Node::Internal { children, .. } => {
                    let (position, offset) = child_at(children, index);
                    index -= offset;
                    node = Arc::make_mut(&mut children[position]);
                }
            }
        }
    }
}

#[cfg(test)]
impl<T: Item> SeqTree<T>
where
    T::Summary: PartialEq + fmt::Debug,
{
    /// Checks uniform leaf depth, fan-out bounds and every cached value.
    pub(crate) fn assert_invariants(&self) {
        fn walk<T: Item>(
            node: &Node<T>,
            is_root: bool,
            depth: usize,
            leaf_depth: &mut Option<usize>,
        ) where
            T::Summary: PartialEq + fmt::Debug,
        {
            let fanout = node.fanout();
            assert!(fanout <= MAX_FANOUT, "fan-out {fanout} above maximum");
            assert!(fanout > 0, "empty node");
            if !is_root {
                assert!(fanout >= MIN_FANOUT, "fan-out {fanout} below minimum");
            }
            let mut copy = node.clone();
            copy.refresh();
            assert_eq!(copy.len(), node.len(), "stale cached length");
            assert_eq!(copy.summary(), node.summary(), "stale cached summary");
            match node {
                Node::Leaf { .. } => {
                    let expected = *leaf_depth.get_or_insert(depth);
                    assert_eq!(expected, depth, "leaves at different depths");
                }
                Node::Internal { children, .. } => {
                    if is_root {
                        assert!(children.len() >= 2, "internal root with one child");
                    }
                    for child in children {
                        walk(child, false, depth + 1, leaf_depth);
                    }
                }
            }
        }
        if let Some(root) = &self.root {
            walk(root, true, 0, &mut None);
        }
    }
}

/// In-order iterator over a [`SeqTree`].
pub struct Iter<'a, T: Item> {
    stack: Vec<(&'a [Arc<Node<T>>], usize)>,
    leaf: std::slice::Iter<'a, T>,
}

impl<'a, T: Item> Iter<'a, T> {
    fn descend(&mut self, mut node: &'a Node<T>) {
        loop {
            match node {
                Node::Leaf { items, .. } => {
                    self.leaf = items.iter();
                    return;
                }
                Node::Internal { children, .. } => {
                    self.stack.push((children, 1));
                    node = &children[0];
                }
            }
        }
    }
}

impl<'a, T: Item> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        loop {
            if let Some(item) = self.leaf.next() {
                return Some(item);
            }
            let (children, position) = self.stack.last_mut()?;
            let children: &'a [Arc<Node<T>>] = children;
            if *position < children.len() {
                let child = &children[*position];
                *position += 1;
                self.descend(child);
            } else {
                self.stack.pop();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct Weighted(u32);

    #[derive(Clone, Debug, Default, PartialEq)]
    struct Total {
        count: usize,
        weight: usize,
    }

    impl Summary for Total {
        fn add_summary(&mut self, other: &Self) {
            self.count += other.count;
            self.weight += other.weight;
        }
    }

    impl Item for Weighted {
        type Summary = Total;
        fn summary(&self) -> Total {
            Total {
                count: 1,
                weight: self.0 as usize,
            }
        }
    }

    /// Small deterministic generator so failures reproduce from the seed.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self, bound: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((self.0 >> 33) as usize) % bound.max(1)
        }
    }

    fn contents(tree: &SeqTree<Weighted>) -> Vec<u32> {
        tree.iter().map(|item| item.0).collect()
    }

    #[test]
    fn matches_vec_under_random_edits() {
        for seed in 1..=20 {
            let mut rng = Lcg(seed);
            let mut tree = SeqTree::new();
            let mut model: Vec<u32> = Vec::new();
            for step in 0..2_000 {
                let choice = rng.next(10);
                if choice < 5 || model.is_empty() {
                    let index = rng.next(model.len() + 1);
                    let value = rng.next(1_000) as u32;
                    tree.insert(index, Weighted(value));
                    model.insert(index, value);
                } else if choice < 8 {
                    let index = rng.next(model.len());
                    assert_eq!(
                        tree.remove(index).0,
                        model.remove(index),
                        "seed {seed} step {step}"
                    );
                } else {
                    let index = rng.next(model.len());
                    let value = rng.next(1_000) as u32;
                    tree.replace(index, Weighted(value));
                    model[index] = value;
                }
                assert_eq!(tree.len(), model.len());
            }
            tree.assert_invariants();
            assert_eq!(contents(&tree), model, "seed {seed}");
            for (index, value) in model.iter().enumerate() {
                assert_eq!(tree.get(index), Some(&Weighted(*value)));
            }
            assert_eq!(tree.get(model.len()), None);
        }
    }

    #[test]
    fn drains_to_empty_and_rebuilds() {
        let mut tree: SeqTree<Weighted> = (0..1_000).map(Weighted).collect();
        tree.assert_invariants();
        for _ in 0..1_000 {
            tree.remove(tree.len() / 2);
            tree.assert_invariants();
        }
        assert!(tree.is_empty());
        tree.push(Weighted(7));
        assert_eq!(contents(&tree), vec![7]);
    }

    #[test]
    fn bulk_build_is_balanced_for_every_small_size() {
        for size in 0..300 {
            let tree: SeqTree<Weighted> = (0..size).map(Weighted).collect();
            tree.assert_invariants();
            assert_eq!(contents(&tree), (0..size).collect::<Vec<_>>());
        }
    }

    #[test]
    fn locate_resolves_boundaries_to_the_later_item() {
        let tree: SeqTree<Weighted> = [3, 0, 2, 5].into_iter().map(Weighted).collect();
        let weight = |total: &Total| total.weight;
        assert_eq!(
            tree.locate(0, weight).map(|(i, p)| (i, p.weight)),
            Some((0, 0))
        );
        assert_eq!(
            tree.locate(2, weight).map(|(i, p)| (i, p.weight)),
            Some((0, 0))
        );
        // 3 is the end of item 0; the zero-weight item 1 cannot contain it.
        assert_eq!(
            tree.locate(3, weight).map(|(i, p)| (i, p.weight)),
            Some((2, 3))
        );
        assert_eq!(
            tree.locate(5, weight).map(|(i, p)| (i, p.weight)),
            Some((3, 5))
        );
        assert_eq!(
            tree.locate(10, weight).map(|(i, p)| (i, p.weight)),
            Some((3, 5))
        );
        assert_eq!(tree.locate(11, weight), None);
        assert_eq!(SeqTree::<Weighted>::new().locate(0, weight), None);
    }

    #[test]
    fn locate_matches_linear_scan_in_deep_trees() {
        let tree: SeqTree<Weighted> = (0..5_000).map(|i| Weighted(i % 7)).collect();
        let weights: Vec<usize> = tree.iter().map(|item| item.0 as usize).collect();
        let total: usize = weights.iter().sum();
        for target in (0..=total).step_by(13) {
            let mut start = 0;
            let mut expected = weights.len() - 1;
            for (index, weight) in weights.iter().enumerate() {
                if target < start + weight {
                    expected = index;
                    break;
                }
                start += weight;
            }
            let (index, prefix) = tree.locate(target, |t| t.weight).unwrap();
            assert_eq!(index, expected, "target {target}");
            assert_eq!(prefix.weight, weights[..index].iter().sum::<usize>());
            assert_eq!(prefix.count, index);
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    struct Plain(u32);

    impl Item for Plain {
        type Summary = ();
        fn summary(&self) {}
    }

    #[test]
    fn get_mut_copies_only_the_path() {
        let base: SeqTree<Plain> = (0..10_000).map(Plain).collect();
        let mut edited = base.clone();
        edited.get_mut(7_777).unwrap().0 = 0;
        assert!(edited.unshared_nodes(&base) <= 5);
        assert_eq!(base.get(7_777), Some(&Plain(7_777)));
        assert_eq!(edited.get(7_777), Some(&Plain(0)));
        assert!(edited.get_mut(10_000).is_none());
        assert!(SeqTree::<Plain>::new().get_mut(0).is_none());
    }

    #[test]
    fn edits_copy_only_the_path_and_keep_snapshots_intact() {
        let base: SeqTree<Weighted> = (0..100_000).map(Weighted).collect();
        let mut edited = base.clone();
        assert_eq!(edited.unshared_nodes(&base), 0);
        edited.replace(54_321, Weighted(0));
        // Depth is about log16(100 000 / 16) + 1 = 5 nodes on the path.
        let copied = edited.unshared_nodes(&base);
        assert!((1..=6).contains(&copied), "copied {copied} nodes");
        assert_eq!(base.get(54_321), Some(&Weighted(54_321)));
        assert_eq!(edited.get(54_321), Some(&Weighted(0)));
        edited.insert(10, Weighted(1));
        edited.remove(99_000);
        assert!(edited.unshared_nodes(&base) <= 18);
        assert_eq!(base.len(), 100_000);
        assert_eq!(base.summary().weight, (0..100_000).sum::<usize>());
    }
}
