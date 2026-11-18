//! Circular formulas in the subgraph of cached #CIRC! cells. The two stacks represent
//! open paths and their possible roots, following Gabow's path-based SCC algorithm
//! (Information Processing Letters 74, 2000). DFS frames live on the heap for long chains.

pub(super) const ACYCLIC: usize = usize::MAX;

/// A cyclic component is identified by its first vertex; singleton non-loops have no group.
pub(super) fn cycle_groups(edges: &[Vec<usize>]) -> Vec<usize> {
    const FINISHED: usize = usize::MAX - 1;
    let mut position = vec![usize::MAX; edges.len()];
    let mut groups = vec![ACYCLIC; edges.len()];
    let mut path = Vec::new();
    let mut roots = Vec::new();
    let mut frames = Vec::new();
    for start in 0..edges.len() {
        if position[start] != usize::MAX {
            continue;
        }
        position[start] = 0;
        roots.push(0);
        path.push(start);
        frames.push((start, 0));
        while let Some((node, next)) = frames.last_mut() {
            let node = *node;
            if let Some(&target) = edges[node].get(*next) {
                *next += 1;
                let offset = position[target];
                if offset == usize::MAX {
                    position[target] = path.len();
                    roots.push(path.len());
                    path.push(target);
                    frames.push((target, 0));
                } else {
                    // A back edge joins every unfinished root after its target.
                    // A completed component's sentinel is larger than every offset.
                    while roots.last().is_some_and(|root| *root > offset) {
                        roots.pop();
                    }
                }
            } else {
                frames.pop();
                let offset = position[node];
                if roots.last() == Some(&offset) {
                    roots.pop();
                    let cyclic = path.len() - offset > 1 || edges[node].contains(&node);
                    for member in path.drain(offset..) {
                        position[member] = FINISHED;
                        if cyclic {
                            groups[member] = node;
                        }
                    }
                }
            }
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_four_cell_graph_agrees_with_independent_reachability() {
        for mask in 0u32..(1 << 16) {
            let mut reachable = [0u32; 4];
            let edges: Vec<Vec<_>> = (0..4)
                .map(|row| {
                    reachable[row] = (mask >> (4 * row)) & 15;
                    (0..4).filter(|col| reachable[row] & (1 << col) != 0).collect()
                })
                .collect();
            for via in 0..4 {
                for row in 0..4 {
                    if reachable[row] & (1 << via) != 0 {
                        reachable[row] |= reachable[via];
                    }
                }
            }
            let groups = cycle_groups(&edges);
            for i in 0..4 {
                assert_eq!(groups[i] != ACYCLIC, reachable[i] & (1 << i) != 0, "graph {mask}");
                for j in i + 1..4 {
                    let mutual = reachable[i] & (1 << j) != 0 && reachable[j] & (1 << i) != 0;
                    assert_eq!(groups[i] != ACYCLIC && groups[i] == groups[j], mutual, "graph {mask}");
                }
            }
        }
    }

    #[test]
    fn a_long_chain_only_marks_the_closed_tail() {
        let length = 50_000;
        let mut edges: Vec<Vec<_>> = (1..=length).map(|next| vec![next]).collect();
        edges[length - 1] = vec![length - 3];
        let groups = cycle_groups(&edges);
        assert!(groups[..length - 3].iter().all(|id| *id == ACYCLIC));
        assert!(groups[length - 3..].iter().all(|id| *id == length - 3));
    }
}
