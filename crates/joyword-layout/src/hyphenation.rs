//! Paragraph hyphenation limits shared by the automatic and manual editing commands.
//! The embedded Moby Hyphenator II data and optional language packs retain their provenance.
use joyword_proof::{
    ProofLang,
    hyphen::{Dictionary, Limits},
};

/// Return character boundaries under the document's hyphenation settings.
pub fn points(word: &str, limits: &Limits, language: ProofLang) -> Vec<usize> {
    if !language.is_english() || !word.bytes().all(|c| c.is_ascii_alphabetic()) {
        let (mut candidates, _) = joyword_proof::hyphen::word_breaks_in(word, language);
        if candidates.is_empty() {
            return candidates;
        }
        let ranges = permitted_ranges(word, limits);
        candidates.retain(|point| {
            let index = ranges.partition_point(|&(_, end)| end < *point);
            ranges.get(index).is_some_and(|&(start, _)| start <= *point)
        });
        return candidates;
    }
    let len = word.len();
    let first = limits.after_first.max(1);
    let last = len.saturating_sub(limits.before_last.max(1));
    if len < limits.min_word.max(2)
        || first > last
        || word.bytes().all(|c| c.is_ascii_uppercase())
        || (!limits.capitalized && word.as_bytes()[0].is_ascii_uppercase())
    {
        return Vec::new();
    }
    // These four English variants use the embedded dictionary for hyphenation even
    // with other language packs installed (see the proofing provider's locale table).
    if let Some(mask) = Dictionary::en_us().get(&word.to_ascii_lowercase()) {
        return (first..=last.min(63))
            .filter(|&i| mask & (1u64 << i) != 0)
            .collect();
    }
    let (mut candidates, _) = joyword_proof::hyphen::word_breaks_in(word, language);
    candidates.retain(|p| (first..=last).contains(p));
    candidates
}

/// Compute one allowed interval per lexical run. Character positions refer to the original
/// text, including its combining marks, while each of the user's margins counts letters.
fn permitted_ranges(word: &str, limits: &Limits) -> Vec<(usize, usize)> {
    let apostrophe = |c| matches!(c, '\'' | '’' | 'ʼ');
    let separator = |c: char| {
        !c.is_alphabetic() && !apostrophe(c) && !unicode_normalization::char::is_combining_mark(c)
    };
    let mut base = 0;
    word.split(separator)
        .filter_map(|run| {
            let start = base;
            let (mut length, mut letters, mut capitals) = (0, 0, 0);
            let mut left = None;
            for c in run.chars() {
                length += 1;
                if c.is_alphabetic() {
                    letters += 1;
                    capitals += usize::from(c.is_uppercase());
                    if letters == limits.after_first.max(1) {
                        left = Some(start + length);
                    }
                }
            }
            base += length + 1;
            if letters < limits.min_word.max(2)
                || capitals == letters
                || (!limits.capitalized
                    && run
                        .chars()
                        .find(|&c| !apostrophe(c))
                        .is_some_and(char::is_uppercase))
            {
                return None;
            }
            let from_end = run
                .chars()
                .rev()
                .enumerate()
                .filter(|(_, c)| c.is_alphabetic())
                .nth(limits.before_last.max(1) - 1)?
                .0;
            let right = start + length - from_end - 1;
            let left = left?;
            (left <= right).then_some((left, right))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multilingual_limits_match_lexical_providers_in_original_character_offsets() {
        let languages = [
            "de", "pl", "pt", "it", "bg", "tr", "ru", "en-GB", "de-CH", "pt-PT",
        ];
        let words = [
            "Silbentrennung",
            "’Überarbeitung’",
            "ko-respondencja",
            "programação",
            "programac\u{327}a\u{303}o",
            "internazionale",
            "български",
            "Türkiye’nin",
            "геометрический",
            "ABC-Baumwolle-123",
            "réélection's",
            "’\u{301}étude’",
            "中文１２３",
        ];
        let mut nonempty = 0;
        for tag in languages {
            let lang = ProofLang::from_tag(Some(tag));
            for word in words {
                for limits in [
                    Limits::default(),
                    Limits {
                        min_word: 2,
                        after_first: 1,
                        before_last: 1,
                        capitalized: true,
                    },
                    Limits {
                        min_word: 4,
                        after_first: 1,
                        before_last: 3,
                        capitalized: false,
                    },
                ] {
                    let actual = points(word, &limits, lang);
                    nonempty += usize::from(!actual.is_empty());
                    assert_eq!(
                        actual,
                        joyword_proof::hyphen::hyphen_points_in(word, &limits, lang),
                        "{tag} {word}"
                    );
                }
            }
        }
        assert!(nonempty > 30);
    }
    #[test]
    fn matches_dictionary_and_pattern_provider_at_word_boundaries() {
        let limits = Limits::default();
        for (word, _) in Dictionary::en_us().iter().step_by(313) {
            for case in [word.to_string(), word.to_ascii_uppercase()] {
                assert_eq!(
                    points(&case, &limits, ProofLang::En),
                    joyword_proof::hyphen::hyphen_points_in(&case, &limits, ProofLang::En)
                );
            }
        }
        for word in [
            "",
            "A",
            "Exceptional",
            "co-operate",
            "organisation's",
            "élégance",
            "unexampledly",
        ] {
            for limits in [
                Limits::default(),
                Limits {
                    min_word: 3,
                    after_first: 1,
                    before_last: 3,
                    capitalized: false,
                },
            ] {
                assert_eq!(
                    points(word, &limits, ProofLang::EnGb),
                    joyword_proof::hyphen::hyphen_points_in(word, &limits, ProofLang::EnGb)
                );
            }
        }
    }
}
