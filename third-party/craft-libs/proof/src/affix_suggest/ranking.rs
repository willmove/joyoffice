//! Candidate costs and presentation order for JoyWord's Hunspell suggestions.
//! The length bound for an edit-distance diagonal follows Ukkonen (1985).
//! Adjacent transpositions use optimal string alignment and cost one ordinary edit.

use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
};

#[derive(Default)]
struct Row {
    start: usize,
    scores: Vec<u32>,
}

impl Row {
    fn get(&self, column: usize, outside: u32) -> u32 {
        column
            .checked_sub(self.start)
            .and_then(|i| self.scores.get(i))
            .copied()
            .unwrap_or(outside)
    }
}

/// Insertions, deletions and transpositions cost two; related letters can be
/// substituted for one. Outside the diagonal band the length difference alone
/// already exceeds the limit. Keep only three bands, including the transpose origin.
pub(crate) fn weighted_distance(
    left: &[char],
    right: &[char],
    related: &dyn Fn(char, char) -> bool,
    limit: u32,
) -> u32 {
    let outside = limit.saturating_add(1);
    let band = (limit / 2) as usize;
    if left.len().abs_diff(right.len()) > band {
        return outside;
    }
    let mut previous = Row {
        start: 0,
        scores: (0..=right.len().min(band)).map(|j| 2 * j as u32).collect(),
    };
    let mut diagonal = Row::default();
    let mut current = Row::default();
    for (row, &letter) in left.iter().enumerate() {
        let i = row + 1;
        let first = i.saturating_sub(band);
        let last = i.saturating_add(band).min(right.len());
        current.start = first;
        current.scores.clear();
        for j in first..=last {
            let deletion = previous.get(j, outside).saturating_add(2);
            let insertion = current
                .scores
                .last()
                .copied()
                .unwrap_or(outside)
                .saturating_add(2);
            let mut score = deletion.min(insertion).min(outside);
            if j > 0 {
                let target = right[j - 1];
                let replacement = if letter == target {
                    0
                } else if related(letter, target) {
                    1
                } else {
                    2
                };
                score = score.min(previous.get(j - 1, outside).saturating_add(replacement));
                if i >= 2 && j >= 2 && left[i - 2] == target && letter == right[j - 2] {
                    score = score.min(diagonal.get(j - 2, outside).saturating_add(2));
                }
            }
            current.scores.push(score);
        }
        std::mem::swap(&mut diagonal, &mut previous);
        std::mem::swap(&mut previous, &mut current);
    }
    previous.get(right.len(), outside)
}

/// Compute the first differing character once for each accepted candidate.
/// Pull only as many as the caller needs after casing and output conversion.
pub(super) fn best_first(
    found: HashMap<String, (u32, usize)>,
    input: &[char],
    word_len: usize,
) -> impl Iterator<Item = String> {
    let mut ranked: BinaryHeap<_> = found
        .into_iter()
        .map(|(text, (cost, sequence))| {
            let difference = input
                .iter()
                .copied()
                .zip(text.chars())
                .position(|(a, b)| a != b)
                .unwrap_or(word_len);
            Reverse((cost, difference, sequence, text))
        })
        .collect();
    std::iter::from_fn(move || ranked.pop().map(|Reverse((_, _, _, text))| text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn related(a: char, b: char) -> bool {
        matches!((a, b), ('a', 'á') | ('á', 'a'))
    }

    // Full matrix with no cutoff, pruning or rolling storage.
    fn exact(a: &[char], b: &[char]) -> u32 {
        let mut grid = vec![vec![0; b.len() + 1]; a.len() + 1];
        for (i, row) in grid.iter_mut().enumerate() {
            row[0] = 2 * i as u32;
        }
        for (j, value) in grid[0].iter_mut().enumerate() {
            *value = 2 * j as u32;
        }
        for i in 1..=a.len() {
            for j in 1..=b.len() {
                let cost = if a[i - 1] == b[j - 1] {
                    0
                } else if related(a[i - 1], b[j - 1]) {
                    1
                } else {
                    2
                };
                let mut best = (grid[i - 1][j] + 2)
                    .min(grid[i][j - 1] + 2)
                    .min(grid[i - 1][j - 1] + cost);
                if i >= 2 && j >= 2 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                    best = best.min(grid[i - 2][j - 2] + 2);
                }
                grid[i][j] = best;
            }
        }
        grid[a.len()][b.len()]
    }

    #[test]
    fn cutoff_and_transpositions_agree_with_every_small_matrix() {
        let mut words = vec![Vec::new()];
        let mut layer = words.clone();
        for _ in 0..4 {
            layer = layer
                .iter()
                .flat_map(|prefix| {
                    ['a', 'á', 'b'].map(|c| {
                        let mut word = prefix.clone();
                        word.push(c);
                        word
                    })
                })
                .collect();
            words.extend(layer.iter().cloned());
        }
        for a in &words {
            for b in &words {
                let expected = exact(a, b);
                for limit in [0, 1, 2, 3, 4, 5, 6, u32::MAX] {
                    assert_eq!(
                        weighted_distance(a, b, &related, limit),
                        expected.min(limit.saturating_add(1)),
                        "{a:?} {b:?} {limit}"
                    );
                }
            }
        }
    }
}
