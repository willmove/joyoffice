//! Installed dictionary syntax and paragraph proofing rules. The editor supplies
//! tokenization, language and issue wording. Paragraph offsets address the original UTF-8.
mod clauses;
pub mod dictionary;
mod english;
mod italian;
mod spanish;
pub use clauses::{ClauseVocabulary, conjunction_comma, polish_preposition, subordinate_commas};
pub use english::{EnglishIssue, english_article, english_words};
pub use italian::{ItalianForms, ItalianIssue, italian_elisions, italian_qual};
pub use spanish::spanish_opening_marks;
mod punctuation;
mod sentence;

pub use punctuation::{MarkIssue, Punctuation, punctuation};
pub use sentence::{capitalize, sentence_case};

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

#[cfg(test)]
mod lexical_tests {
    use super::*;

    #[test]
    fn article_suggestions_use_sound_and_skip_unresolved_acronyms() {
        for (article, word, fix) in [
            ("a", "hour", Some("an")),
            ("An", "university", Some("A")),
            ("a", "MRI", Some("an")),
            ("an", "UFO", Some("a")),
            ("a", "unidentified", Some("an")),
            ("an", "honest", None),
            ("a", "SQL", None),
            ("an", "hotel", None),
            ("a", "herb", None),
            ("an", "hors", None),
            ("a", "urologist", None),
            ("an", "RMX", None),
            ("a", "one", None),
            ("A", "EU", None),
        ] {
            assert_eq!(english_article(article, word), fix, "{article} {word}");
        }
    }

    #[test]
    fn punctuation_stays_in_its_clause_and_points_into_the_original_text() {
        let text = "Ana: ¿¡Sí!? Envía a alguien@example.es. Cuándo? https://a.es/¿? Vendrás!";
        let mut found = Vec::new();
        spanish_opening_marks(
            text,
            |w| w.contains('@') || w.contains("://"),
            |span, opener| found.push((text[span].to_owned(), opener)),
        );
        assert_eq!(
            found,
            [
                (String::from("Cuándo?"), '¿'),
                (String::from("Vendrás!"), '¡')
            ]
        );
        assert_eq!(conjunction_comma(" \u{a0},\t"), Some(3));
        assert_eq!(conjunction_comma("», "), None);
        assert_eq!(conjunction_comma(",\n"), None);
        assert_eq!(polish_preposition("Z", "szkoły"), Some(("ze", "Ze")));
        assert_eq!(polish_preposition("z", "szafy"), None);
        assert_eq!(italian_qual("QUAL’ERA"), Some("QUAL ERA".into()));
    }
}
