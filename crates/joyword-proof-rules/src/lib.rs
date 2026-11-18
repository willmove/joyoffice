//! First-party paragraph proofing rules. All offsets address the original UTF-8
//! paragraph. The editor supplies its tokenization, language and issue wording.
mod punctuation;
mod sentence;

pub use punctuation::{MarkIssue, Punctuation, punctuation};
pub use sentence::sentence_case;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Czech,
    Croatian,
    English,
    Indonesian,
    Italian,
    Polish,
    Portuguese,
    Russian,
    Spanish,
    Turkish,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn punctuation_keeps_original_unicode_ranges_and_token_exceptions() {
        let findings = |text: &str, language| {
            let mut result = Vec::new();
            punctuation(text, language, |issue| {
                result.push((text[issue.span].to_owned(), issue.replacements.to_vec()));
            });
            result
        };
        assert_eq!(
            findings("Árbol  ¿ Bien ?»", Language::Spanish),
            [
                ("  ".into(), vec![" "]),
                ("¿ ".into(), vec!["¿"]),
                (" ?".into(), vec!["?"])
            ]
        );
        assert_eq!(
            findings(" ,,Кто .. ;;", Language::Russian),
            [
                (",,".into(), vec!["«"]),
                ("..".into(), vec![".", "…"]),
                (" ;".into(), vec![";"]),
                (";;".into(), vec![";"])
            ]
        );
        for language in [
            Language::English,
            Language::Italian,
            Language::Czech,
            Language::Turkish,
        ] {
            assert!(findings("Amount .5 ... 1 : 3 ;)", language).is_empty());
        }
        assert!(findings("Ecco....", Language::Italian).is_empty());
        assert_eq!(
            findings("Bitti ;Dün", Language::Turkish),
            [(" ;".into(), vec![";"])]
        );
        assert_eq!(
            findings("Neden ????", Language::Italian),
            [
                (" ?".into(), vec!["?"]),
                ("??".into(), vec!["?"]),
                ("??".into(), vec!["?"])
            ]
        );
    }

    #[test]
    fn sentence_case_respects_local_dialogue_and_list_context() {
        fn fixes(text: &str, language: Language, first: bool) -> Vec<String> {
            let mut words = Vec::new();
            let mut start = None;
            for (i, c) in text
                .char_indices()
                .chain(std::iter::once((text.len(), ' ')))
            {
                if c.is_alphabetic() {
                    start.get_or_insert(i);
                } else if let Some(begin) = start.take() {
                    words.push((begin, i));
                }
            }
            let mut result = Vec::new();
            sentence_case(text, &words, language, first, &["dr"], |span, word| {
                assert!(text.is_char_boundary(span.start) && text.is_char_boundary(span.end));
                result.push(word);
            });
            result
        }
        assert_eq!(fixes("Şehir. izmir", Language::Turkish, true), ["İzmir"]);
        assert!(fixes("tamam. Dr. izmir", Language::Turkish, false).is_empty());
        assert!(fixes("«Vai?» disse ele.", Language::Portuguese, true).is_empty());
        assert_eq!(
            fixes("«Vai?» disse ele.", Language::Croatian, true),
            ["Disse"]
        );
        assert_eq!(fixes("Finì. —domani", Language::Italian, false), ["Domani"]);
        assert!(fixes("item,", Language::Portuguese, true).is_empty());
        assert_eq!(fixes("item,", Language::English, true), ["Item"]);
    }
}
