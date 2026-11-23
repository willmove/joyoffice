//! Compact radix indices of the byte strings added by Hunspell prefix and suffix rules.
//! Compress an entire unbranched run into one edge. Final nodes address rule groups in
//! dictionary order. A walk reports the empty affix first, then increasing matched lengths.
use std::ops::Range;

#[derive(Clone, Debug, Default)]
struct Node {
    edge: Range<usize>,
    children: Range<usize>,
    value: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct EdgeIndex<V, const SUFFIX: bool> {
    bytes: Vec<u8>,
    nodes: Vec<Node>,
    values: Vec<V>,
}

impl<V, const SUFFIX: bool> EdgeIndex<V, SUFFIX> {
    pub(super) fn build(entries: impl IntoIterator<Item = (Box<str>, V)>) -> Self {
        let mut sorted: Vec<_> = entries
            .into_iter()
            .map(|(key, rules)| {
                let mut bytes = key.into_string().into_bytes();
                if SUFFIX {
                    bytes.reverse();
                }
                (bytes, rules)
            })
            .collect();
        sorted.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        let (keys, values): (Vec<Vec<u8>>, Vec<V>) = sorted.into_iter().unzip();
        let mut tree = Self {
            bytes: Vec::new(),
            nodes: vec![Node::default()],
            values,
        };
        let mut pending = vec![(0, 0..keys.len(), 0)];
        while let Some((node, group, depth)) = pending.pop() {
            let mut first = group.start;
            if first < group.end && keys[first].len() == depth {
                tree.nodes[node].value = Some(first);
                first += 1;
            }
            let children = tree.nodes.len();
            while first < group.end {
                let key = &keys[first];
                let initial = key[depth];
                let end = first + keys[first..group.end].partition_point(|k| k[depth] == initial);
                // A sorted run shares exactly the prefix common to its first and last keys.
                let tail = &keys[end - 1];
                let common = if end == first + 1 {
                    key.len()
                } else {
                    depth
                        + 1
                        + key[depth + 1..]
                            .iter()
                            .zip(&tail[depth + 1..])
                            .take_while(|(a, b)| a == b)
                            .count()
                };
                let start = tree.bytes.len();
                tree.bytes.extend_from_slice(&key[depth..common]);
                let child = tree.nodes.len();
                tree.nodes.push(Node {
                    edge: start..tree.bytes.len(),
                    ..Node::default()
                });
                pending.push((child, first..end, common));
                first = end;
            }
            tree.nodes[node].children = children..tree.nodes.len();
        }
        tree
    }

    pub(super) fn matches<'a>(&'a self, word: &'a str) -> impl Iterator<Item = (usize, &'a V)> {
        let mut input = word.as_bytes().iter();
        let mut take = move || {
            if SUFFIX {
                input.next_back().copied()
            } else {
                input.next().copied()
            }
        };
        let mut current = self.nodes.first();
        let mut length = 0;
        std::iter::from_fn(move || {
            loop {
                let node = current?;
                let hit = node.value.map(|id| (length, &self.values[id]));
                current = (|| {
                    let first = take()?;
                    let children = &self.nodes[node.children.clone()];
                    let i = children
                        .binary_search_by_key(&first, |child| self.bytes[child.edge.start])
                        .ok()?;
                    let child = &children[i];
                    if self.bytes[child.edge.start + 1..child.edge.end]
                        .iter()
                        .any(|&b| take() != Some(b))
                    {
                        return None;
                    }
                    length += child.edge.len();
                    Some(child)
                })();
                if hit.is_some() {
                    return hit;
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressed_edges_match_independent_utf8_prefix_and_suffix_searches() {
        let mut keys = vec![String::new()];
        for _ in 0..4 {
            let more: Vec<_> = keys
                .iter()
                .flat_map(|s| ['a', 'é', '漢'].map(|c| format!("{s}{c}")))
                .collect();
            keys.extend(more);
            keys.sort();
            keys.dedup();
        }
        let entries = || {
            keys.iter()
                .enumerate()
                .filter(|(i, _)| i % 3 != 1)
                .map(|(i, k)| (k.clone().into_boxed_str(), i))
        };
        let pref = EdgeIndex::<_, false>::build(entries());
        let suff = EdgeIndex::<_, true>::build(entries());
        for word in keys
            .iter()
            .flat_map(|w| [w.clone(), format!("x{w}"), format!("{w}x")])
        {
            for (is_suffix, hits) in [
                (false, pref.matches(&word).collect::<Vec<_>>()),
                (true, suff.matches(&word).collect()),
            ] {
                let mut expected: Vec<_> = entries()
                    .filter(|(k, _)| {
                        if is_suffix {
                            word.ends_with(&**k)
                        } else {
                            word.starts_with(&**k)
                        }
                    })
                    .map(|(k, i)| (k.len(), i))
                    .collect();
                expected.sort_unstable();
                assert_eq!(
                    hits.iter().map(|&(n, i)| (n, *i)).collect::<Vec<_>>(),
                    expected
                );
            }
        }
        let common = "ä".repeat(10000);
        let tree = EdgeIndex::<_, false>::build([
            (common.clone().into_boxed_str(), 0),
            (format!("{common}x").into_boxed_str(), 1),
            (format!("{common}y").into_boxed_str(), 2),
        ]);
        assert_eq!(tree.nodes.len(), 4);
        assert_eq!(tree.bytes.len(), common.len() + 2);
    }
}
