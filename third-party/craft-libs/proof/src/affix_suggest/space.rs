//! Positions and relative penalties for an omitted word separator. Enumerate
//! extended grapheme boundaries once; dictionary lengths are measured in Unicode
//! scalars as in the language profiles.
use super::{COST_SPLIT, COST_SPLIT_LAST, COST_SPLIT_LONG, COST_SPLIT_SHORT};
use unicode_segmentation::UnicodeSegmentation;

pub(crate) enum Split {
    Anywhere,
    Near {
        leading: &'static [&'static str],
        trailing: &'static [&'static str],
    },
    LongWords,
    WithPrepositions {
        leading: &'static [&'static str],
        trailing: &'static [&'static str],
        prepositions: &'static [&'static str],
        verb_endings: &'static [&'static str],
        short_cost: u32,
    },
}

impl Split {
    fn cost(&self, a: &str, b: &str, a_len: usize, b_len: usize) -> Option<u32> {
        let both_long = a_len >= 3 && b_len >= 3;
        let (leading, trailing) = match self {
            Self::Anywhere => {
                return Some(if both_long {
                    COST_SPLIT
                } else {
                    COST_SPLIT_SHORT
                });
            }
            Self::LongWords => return both_long.then_some(COST_SPLIT_LAST),
            Self::Near { leading, trailing } => (leading, trailing),
            Self::WithPrepositions {
                leading,
                trailing,
                prepositions,
                verb_endings,
                short_cost,
            } => {
                if prepositions.contains(&a) {
                    if verb_endings.iter().any(|ending| b.ends_with(ending)) {
                        return None;
                    }
                    if a_len < 3 && b_len >= 3 && leading.contains(&a) {
                        return Some(*short_cost);
                    }
                }
                (leading, trailing)
            }
        };
        if (b_len >= 3 && leading.contains(&a)) || (a_len >= 3 && trailing.contains(&b)) {
            Some(COST_SPLIT)
        } else {
            both_long.then_some(COST_SPLIT_LONG)
        }
    }

    pub(super) fn visit(&self, word: &str, total: usize, mut emit: impl FnMut(usize, u32)) {
        let mut left = 0;
        for (byte, cluster) in word.grapheme_indices(true) {
            if byte != 0 {
                let (a, b) = word.split_at(byte);
                if let Some(cost) = self.cost(a, b, left, total - left) {
                    emit(byte, cost)
                }
            }
            left += cluster.chars().count();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_space_preserves_the_original_graphemes_and_scalar_lengths() {
        for word in [
            "e\u{301}cole",
            "가나다",
            "إنَّما",
            "क्\u{200d}षत्र",
            "x👩‍💻y",
            "🇨🇳🇪🇸",
        ] {
            let chars = word.chars().count();
            let mut actual = Vec::new();
            Split::Anywhere.visit(word, chars, |byte, _| actual.push(byte));
            assert_eq!(
                actual,
                word.grapheme_indices(true)
                    .skip(1)
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>()
            );
            let mut long = Vec::new();
            Split::LongWords.visit(word, chars, |byte, cost| {
                assert_eq!(cost, COST_SPLIT_LAST);
                long.push(byte)
            });
            assert_eq!(
                long,
                actual
                    .into_iter()
                    .filter(|&i| word[..i].chars().count() >= 3 && word[i..].chars().count() >= 3)
                    .collect::<Vec<_>>()
            );
        }
    }
}
