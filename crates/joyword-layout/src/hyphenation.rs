//! Break positions for English words shared by automatic and manual hyphenation.
//! The embedded Moby Hyphenator II data and optional language packs retain their provenance.
use joyword_proof::{
    ProofLang,
    hyphen::{Dictionary, Limits},
};

/// Return character boundaries under the document's hyphenation settings.
pub fn points(word: &str, limits: &Limits, language: ProofLang) -> Vec<usize> {
    if !language.is_english() || !word.bytes().all(|c| c.is_ascii_alphabetic()) {
        return joyword_proof::hyphen::hyphen_points_in(word, limits, language);
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
    // Installed language data takes precedence. The built-in dictionary gives positions as
    // bits in a mask, so exact matches can be emitted without constructing character arrays.
    if joyword_proof::packs::packs_dir().is_none()
        && let Some(mask) = Dictionary::en_us().get(&word.to_ascii_lowercase())
    {
        return (first..=last.min(63))
            .filter(|&i| mask & (1u64 << i) != 0)
            .collect();
    }
    let (mut candidates, _) = joyword_proof::hyphen::word_breaks_in(word, language);
    candidates.retain(|p| (first..=last).contains(p));
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
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
