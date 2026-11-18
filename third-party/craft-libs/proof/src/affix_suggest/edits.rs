//! Enumerate dictionary-directed edits in the order in which tied candidates are offered.
//! KEY, TRY, MAP and REP are supplied by the dictionary or the language profile.
use super::{COST_DROP_OR_ADD, COST_KEY, COST_SWAP, MAX_MAP_VARIANTS, Tuning, split_cost};

/// Indices come from char_indices or match_indices on the original word. Reuse
/// one allocation for candidates the collector can reject or deduplicate.
struct Splice<'a> {
    word: &'a str,
    text: String,
}

impl Splice<'_> {
    fn apply(&mut self, start: usize, end: usize, with: impl IntoIterator<Item = char>) -> &str {
        self.text.clear();
        self.text.push_str(&self.word[..start]);
        self.text.extend(with);
        self.text.push_str(&self.word[end..]);
        &self.text
    }
}

pub(super) fn visit<'a>(
    word: &str,
    chars: &[char],
    confusions: impl Iterator<Item = (&'a str, &'a str)>,
    keyboard: &str,
    alphabet: &str,
    tuning: &Tuning,
    mut offer: impl FnMut(&str, u32),
) {
    let mut edit = Splice {
        word,
        text: String::with_capacity(word.len() + 4),
    };
    for (old, new) in confusions.filter(|(old, _)| !old.is_empty()) {
        for (byte, matched) in word.match_indices(old) {
            offer(
                edit.apply(byte, byte + matched.len(), new.chars()),
                tuning.rep_cost,
            );
        }
    }
    let boundaries: Vec<_> = word
        .char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(word.len()))
        .collect();
    for &byte in boundaries
        .iter()
        .skip(1)
        .take(chars.len().saturating_sub(1))
    {
        let (left, right) = word.split_at(byte);
        if let Some(cost) = split_cost(&tuning.split, left, right) {
            offer(edit.apply(byte, byte, [' ']), cost);
        }
    }
    for row in keyboard.split('|').take(32) {
        let mut keys = row.chars().take(128);
        let Some(mut previous) = keys.next() else {
            continue;
        };
        for next in keys {
            for (i, &typed) in chars.iter().enumerate() {
                if typed == previous || typed == next {
                    let replacement = if typed == previous { next } else { previous };
                    offer(
                        edit.apply(boundaries[i], boundaries[i + 1], [replacement]),
                        COST_KEY,
                    );
                }
            }
            previous = next;
        }
    }
    for (i, pair) in chars.windows(2).enumerate() {
        if pair[0] != pair[1] {
            offer(
                edit.apply(boundaries[i], boundaries[i + 2], [pair[1], pair[0]]),
                COST_SWAP,
            );
        }
    }
    for range in boundaries.windows(2) {
        offer(edit.apply(range[0], range[1], []), COST_DROP_OR_ADD);
    }
    let mut letters = Vec::new();
    for c in alphabet.chars().filter(|c| c.is_lowercase()).take(128) {
        if !letters.contains(&c) {
            letters.push(c);
        }
    }
    for &byte in &boundaries {
        for &letter in &letters {
            offer(edit.apply(byte, byte, [letter]), COST_DROP_OR_ADD);
        }
    }
    for (i, &original) in chars.iter().enumerate() {
        for &letter in &letters {
            if original != letter {
                offer(
                    edit.apply(boundaries[i], boundaries[i + 1], [letter]),
                    (tuning.mistype_cost)(chars, i, letter),
                );
            }
        }
    }
}

/// Skip positions with no MAP siblings. For a fixed number of edits the first
/// change sweeps right to left, with dictionary order at each changed position.
pub(super) fn map_variants(
    chars: &[char],
    alts: &[Vec<char>],
    tuning: &Tuning,
    out: &mut Vec<(String, u32)>,
) {
    let positions: Vec<_> = alts
        .iter()
        .map(Vec::as_slice)
        .enumerate()
        .filter(|(_, choices)| !choices.is_empty())
        .collect();
    let mut word = chars.to_vec();
    for n in 1..=tuning.max_map_changes.min(positions.len()) {
        combinations(&mut word, &positions, n, tuning.map_cost, 0, out);
        if out.len() == MAX_MAP_VARIANTS {
            break;
        }
    }
}

fn combinations(
    word: &mut [char],
    positions: &[(usize, &[char])],
    remaining: usize,
    price: fn(char, char) -> u32,
    cost: u32,
    out: &mut Vec<(String, u32)>,
) {
    if remaining == 0 {
        out.push((word.iter().collect(), cost));
        return;
    }
    for first in (0..=positions.len() - remaining).rev() {
        let (index, alternatives) = positions[first];
        let original = word[index];
        for &replacement in alternatives {
            if out.len() >= MAX_MAP_VARIANTS {
                return;
            }
            word[index] = replacement;
            combinations(
                word,
                &positions[first + 1..],
                remaining - 1,
                price,
                cost + price(original, replacement),
                out,
            );
            word[index] = original;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::affix_suggest::{COST_MISTYPE, COST_REP, COST_SPLIT_SHORT};

    #[test]
    fn mixed_width_edits_keep_dictionary_order_and_cost() {
        let mut actual = Vec::new();
        let word = "é中a";
        visit(
            word,
            &['é', '中', 'a'],
            [("", "x"), ("中", "œu"), ("a", "")].into_iter(),
            "éa|中",
            "aAéa",
            &Tuning::DEFAULT,
            |s, c| actual.push((s.to_owned(), c)),
        );
        let groups: &[(u32, &[&str])] = &[
            (COST_REP, &["éœua", "é中"]),
            (COST_SPLIT_SHORT, &["é 中a", "é中 a"]),
            (COST_KEY, &["a中a", "é中é"]),
            (COST_SWAP, &["中éa", "éa中"]),
            (
                COST_DROP_OR_ADD,
                &[
                    "中a", "éa", "é中", "aé中a", "éé中a", "éa中a", "éé中a", "é中aa", "é中éa",
                    "é中aa", "é中aé",
                ],
            ),
            (COST_MISTYPE, &["a中a", "éaa", "ééa", "é中é"]),
        ];
        let expected: Vec<_> = groups
            .iter()
            .flat_map(|(c, words)| words.iter().map(|w| (w.to_string(), *c)))
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn map_order_cost_and_cutoff_match_cartesian_products() {
        for len in 1..=9 {
            // Include repeated positions, a fixed non-Latin letter and different UTF-8 widths.
            let chars: Vec<_> = ['é', 'a', 'ж', 'a'].into_iter().cycle().take(len).collect();
            let choices: Vec<_> = chars
                .iter()
                .map(|&c| match c {
                    'é' => vec!['é', 'e', 'ê'],
                    'a' => vec!['a', 'ä', 'á', 'å'],
                    c => vec![c],
                })
                .collect();
            let alternatives: Vec<_> = choices.iter().map(|group| group[1..].to_vec()).collect();
            let mut expected = Vec::new();
            let count: usize = choices.iter().map(Vec::len).product();
            for mut code in 1..count {
                let mut text = chars.clone();
                let mut changes = 0;
                let mut cost = 0;
                for i in (0..len).rev() {
                    let slot = code % choices[i].len();
                    code /= choices[i].len();
                    if slot != 0 {
                        text[i] = choices[i][slot];
                        changes += 1;
                        cost += (Tuning::DEFAULT.map_cost)(chars[i], text[i]);
                    }
                }
                if changes <= Tuning::DEFAULT.max_map_changes {
                    expected.push((changes, (text.iter().collect::<String>(), cost)));
                }
            }
            expected.sort_by_key(|p| p.0);
            let expected: Vec<_> = expected
                .into_iter()
                .take(MAX_MAP_VARIANTS)
                .map(|(_, v)| v)
                .collect();
            let mut actual = Vec::new();
            map_variants(&chars, &alternatives, &Tuning::DEFAULT, &mut actual);
            assert_eq!(actual, expected, "length {len}");
        }
    }

    #[test]
    fn key_and_try_limits_precede_deduplication() {
        let alphabet = format!("{}é", "a".repeat(128));
        let keyboard = format!("{}|{}a", vec!["a"; 31].join("|"), "é".repeat(128));
        let mut found = Vec::new();
        visit(
            "a",
            &['a'],
            std::iter::empty(),
            &keyboard,
            &alphabet,
            &Tuning::DEFAULT,
            |word, cost| found.push((word.to_owned(), cost)),
        );
        assert_eq!(
            found,
            [
                ("", COST_DROP_OR_ADD),
                ("aa", COST_DROP_OR_ADD),
                ("aa", COST_DROP_OR_ADD)
            ]
            .map(|(word, cost)| (word.to_owned(), cost))
        );
    }
}
