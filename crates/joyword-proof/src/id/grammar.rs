//! Generic repeated-word and spacing checks with Indonesian messages. No Indonesian-specific
//! grammar rules are inferred. A word written twice with a space is flagged; since standard
//! Indonesian spelling (PUEBI) writes reduplication with a hyphen (`anak-anak`), the hyphenated
//! form is offered first, then the single word.

use crate::{Issue, IssueKind, ProofLang};

fn issue(start: usize, end: usize, message: String, suggestions: Vec<String>) -> Issue {
    Issue {
        start,
        end,
        kind: IssueKind::Grammar,
        message,
        suggestions,
        lang: ProofLang::Id,
    }
}

pub fn check(text: &str) -> Vec<Issue> {
    let mut issues = Vec::new();
    crate::repeated::visit(text, crate::word_ranges(text), ProofLang::Id, &[], |pair| {
        let (first, second) = (pair.first, pair.second);
        issues.push(issue(
            pair.range.start,
            pair.range.end,
            format!("Kata berulang: “{second}”"),
            vec![format!("{first}-{second}"), first.to_owned()],
        ));
    });
    crate::editorial::punctuation(text, ProofLang::Id, &mut issues);
    issues.sort_by_key(|i| (i.start, i.end));
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_words_and_spaces_have_indonesian_messages() {
        let text = "Anak anak ,  sekolah.";
        let issues = check(text);
        assert_eq!(issues.len(), 3, "{issues:?}");
        assert_eq!(issues[0].message, "Kata berulang: “anak”");
        assert_eq!(issues[0].suggestions, ["Anak-anak", "Anak"]);
        assert!(
            issues
                .iter()
                .any(|i| i.message == "Spasi sebelum tanda baca" && &text[i.start..i.end] == " ,")
        );
        assert!(issues.iter().any(|i| i.message == "Spasi ganda"));
        assert!(
            issues
                .iter()
                .all(|i| i.lang == ProofLang::Id && i.kind == IssueKind::Grammar)
        );
        for text in [
            "anak-anak bermain",
            "rumah. Sekolah",
            "Dr. Budi",
            "Anak, anak!",
            "Senyum :) Angka .5 dan titik ...",
            "kupu-kupu",
        ] {
            assert!(check(text).is_empty(), "{text}: {:?}", check(text));
        }
    }

    #[test]
    fn ratios_and_scores_keep_their_spaces() {
        for text in ["3 : 1", "Skor 2 : 1.", "Rasio 1 : 100."] {
            assert!(check(text).is_empty(), "{text}: {:?}", check(text));
        }
        for text in ["Kata : lain", "3 : kata", "3 :"] {
            assert!(
                check(text)
                    .iter()
                    .any(|i| i.message == "Spasi sebelum tanda baca"),
                "{text}"
            );
        }
    }

    proptest::proptest! {
        #[test]
        fn never_panics_and_ranges_are_valid(s in "\\PC{0,80}") {
            for issue in check(&s) {
                proptest::prop_assert!(issue.start <= issue.end && issue.end <= s.len());
                proptest::prop_assert!(s.is_char_boundary(issue.start) && s.is_char_boundary(issue.end));
            }
        }
    }
}
