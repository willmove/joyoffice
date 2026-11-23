//! Spanish grammar checks with Spanish messages: the generic repeated-word and spacing checks,
//! and the opening question and exclamation marks Spanish writes before a question or an
//! exclamation (`¿Vienes?`, `¡Qué bien!`): a closing `?` or `!` whose sentence has no opening mark
//! is flagged, with the opening mark offered at the sentence's start, and so is a space after an
//! opening mark (`¿ Vienes?`). Either opening mark may pair with either closing one (`¡Qué
//! dices?`), and `(?)` and `(!)`, which mark doubt or surprise, stand alone. No other
//! Spanish-specific grammar, comma or sentence-capital rules are inferred.

use crate::{Issue, IssueKind, ProofLang};

fn issue(
    start: usize,
    end: usize,
    message: String,
    suggestions: Vec<String>,
    lang: ProofLang,
) -> Issue {
    Issue {
        start,
        end,
        kind: IssueKind::Grammar,
        message,
        suggestions,
        lang,
    }
}

/// Grammar issues in a paragraph of Spanish text; `lang` is the variety (they explain the same in
/// every one).
pub fn check(text: &str, lang: ProofLang) -> Vec<Issue> {
    let mut issues = Vec::new();
    crate::repeated::visit(text, crate::word_ranges(text), lang, &[], |pair| {
        let (first, second) = (pair.first, pair.second);
        issues.push(issue(
            pair.range.start,
            pair.range.end,
            format!("Palabra repetida: «{second}»"),
            vec![first.to_owned()],
            lang,
        ));
    });
    crate::editorial::punctuation(text, lang, &mut issues);
    joyword_proof_rules::spanish_opening_marks(text, crate::is_address, |span, opener| {
        let message = if opener == '¿' {
            "Falta el signo de interrogación de apertura (¿)"
        } else {
            "Falta el signo de exclamación de apertura (¡)"
        };
        let replacement = format!("{opener}{}", &text[span.clone()]);
        issues.push(issue(
            span.start,
            span.end,
            message.into(),
            vec![replacement],
            lang,
        ));
    });
    issues.sort_by_key(|i| (i.start, i.end));
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(text: &str) -> Vec<(String, String)> {
        check(text, ProofLang::Es)
            .into_iter()
            .map(|i| (text[i.start..i.end].to_string(), i.message))
            .collect()
    }

    /// The missing-opening-mark issues of `text`: the flagged text and the suggestions offered.
    fn missing(text: &str) -> Vec<(String, Vec<String>)> {
        check(text, ProofLang::Es)
            .into_iter()
            .filter(|i| i.message.starts_with("Falta"))
            .map(|i| (text[i.start..i.end].to_string(), i.suggestions))
            .collect()
    }

    /// craft-libs#496 review: a run of spaces is one issue over the whole run.
    #[test]
    fn a_run_of_spaces_is_reported_whole() {
        for (text, run) in [
            ("casa  roja", "  "),
            ("casa   roja", "   "),
            ("casa    roja", "    "),
            ("casa     roja", "     "),
        ] {
            let m: Vec<_> = messages(text)
                .into_iter()
                .filter(|(_, msg)| msg == "Espacio doble")
                .collect();
            assert_eq!(
                m,
                [(run.to_string(), "Espacio doble".to_string())],
                "{text}"
            );
        }
        assert!(messages("casa roja").is_empty());
    }

    #[test]
    fn repeated_words_and_spaces_have_spanish_messages() {
        let text = "Es es la casa ,  dijo.";
        let issues = check(text, ProofLang::EsMx);
        assert_eq!(issues.len(), 3, "{issues:?}");
        assert_eq!(issues[0].message, "Palabra repetida: «es»");
        assert_eq!(issues[0].suggestions, ["Es"]);
        assert!(
            issues
                .iter()
                .any(|i| i.message == "Espacio antes de un signo de puntuación"
                    && &text[i.start..i.end] == " ,")
        );
        assert!(issues.iter().any(|i| i.message == "Espacio doble"));
        assert!(
            issues
                .iter()
                .all(|i| i.lang == ProofLang::EsMx && i.kind == IssueKind::Grammar)
        );
        for text in [
            "La casa. Es grande",
            "Sr. García",
            "No, no lo sé",
            "Hola :) número .5 y puntos ...",
            "3 : 1",
            "teórico-práctico",
        ] {
            assert!(
                check(text, ProofLang::Es).is_empty(),
                "{text}: {:?}",
                check(text, ProofLang::Es)
            );
        }
    }

    #[test]
    fn questions_and_exclamations_need_their_opening_mark() {
        assert_eq!(
            messages("Vienes mañana?"),
            [(
                "Vienes mañana?".into(),
                "Falta el signo de interrogación de apertura (¿)".into()
            )]
        );
        assert_eq!(
            check("Vienes mañana?", ProofLang::Es)[0].suggestions,
            ["¿Vienes mañana?"]
        );
        assert_eq!(
            messages("Llegó. Qué bien!"),
            [(
                "Qué bien!".into(),
                "Falta el signo de exclamación de apertura (¡)".into()
            )]
        );
        assert_eq!(messages("Le preguntó: vienes?")[0].0, "vienes?");
        assert_eq!(messages("Dijo «vienes?»")[0].0, "vienes?");
        assert_eq!(messages("Hola!!!").len(), 1);
        assert_eq!(messages("Qué?!").len(), 1);
        for text in [
            "¿Vienes mañana?",
            "¡Qué bien!",
            "Si vienes, ¿me llamas?",
            "¡Qué dices?",
            "¿Qué?! ¡No!",
            "Nació en 1950 (?) en Montevideo.",
            "Lo dijo (!) en público.",
            "Ver https://example.com/?q=1 y a!b",
            "¿Qué? ¿Cómo?",
            "2 + 2 = ?",
            "",
            "?",
        ] {
            assert!(
                messages(text).iter().all(|(_, m)| !m.starts_with("Falta")),
                "{text}: {:?}",
                messages(text)
            );
        }
    }

    /// Maintainer review on #3562: a URL ending in `/?` isn't a question, and the dots and colons
    /// of an address or a word (`example.com`, `https:`, `10:30`) don't start the sentence the
    /// missing `¿` is offered for; ordinary questions are still checked.
    #[test]
    fn addresses_are_not_questions_or_sentence_starts() {
        for text in [
            "Ver https://example.com/?",
            "Ver https://example.com/? ¿Vienes?",
            "Ver http://example.com/buscar/? y www.example.com/?",
            "Escribí a ana@example.com!",
            "¿Viste https://example.com/?",
        ] {
            assert!(missing(text).is_empty(), "{text}: {:?}", missing(text));
        }
        assert_eq!(
            missing("Mirá https://example.com/ y decime si vienes?"),
            [(
                "Mirá https://example.com/ y decime si vienes?".to_string(),
                vec!["¿Mirá https://example.com/ y decime si vienes?".to_string()]
            )]
        );
        assert_eq!(
            missing("Mirá example.com y decime si vienes?")[0].0,
            "Mirá example.com y decime si vienes?"
        );
        assert_eq!(
            missing("Son las 10:30, vienes?")[0].0,
            "Son las 10:30, vienes?"
        );
        assert_eq!(
            missing("Ver https://example.com/? Vienes mañana?"),
            [(
                "Vienes mañana?".to_string(),
                vec!["¿Vienes mañana?".to_string()]
            )]
        );
        assert_eq!(
            missing("Llegó. Vienes?")[0].0,
            "Vienes?",
            "a full stop before a space still ends a sentence"
        );
    }

    /// Maintainer review on #3562: a correctly opened question or exclamation is never flagged,
    /// whatever sits between its marks: quotes, an abbreviation's full stop, an ellipsis, a dash
    /// aside, parentheses, nested questions, a line break.
    #[test]
    fn correctly_opened_questions_are_not_flagged() {
        for text in [
            "¿Leíste «Rayuela»?",
            "¿Viene el Sr. García?",
            "¿Y si... no viene?",
            "¿Vendrás —preguntó Juan— mañana?",
            "¿Qué significa (aquí) la palabra?",
            "¿Has leído \"Rayuela\" de Cortázar?",
            "¿Leíste “Rayuela”?",
            "¿Es 'Rayuela' o 'Los premios'?",
            "¿Y tú, qué opinas… vienes?",
            "¿Dijo «¿qué?» o no?",
            "¿Viene la Dra. López, es decir, la jefa (la de 2.5 años)?",
            "¿Dónde: aquí o allí?",
            "¿Cuánto cuesta: 3,50 o a las 10:30?",
            "¿Vienes a las 10:30; o mejor mañana?",
            "¡Qué dices, Dr. Pérez!",
            "¡Qué dices, Dr. Pérez? ¡Basta!",
            "¡¡Hola!! ¿Qué tal?",
            "¡¡¡Basta!!!",
            "¿Vienes?!",
            "¿Qué?! ¡No!",
            "¿Vienes?\n¿O no?",
            "¿Vienes\nmañana?",
            "Ana dijo: «¿Vienes?» y yo: «Sí».",
            "—¿Vienes? —preguntó Juan.",
            "¿Viste https://example.com/? Dime.",
            "¿Es verdad (!) o no?",
            "Nació en 1950 (?) en Montevideo.",
        ] {
            assert!(missing(text).is_empty(), "{text:?}: {:?}", missing(text));
        }
    }

    /// A closing mark with nothing open is still flagged, after a sentence or a clause that closed
    /// an earlier question, and with the sentence it closes offered with its opening mark.
    #[test]
    fn closing_marks_with_nothing_open_are_still_flagged() {
        for (text, flagged, offered) in [
            ("Vienes mañana?", "Vienes mañana?", "¿Vienes mañana?"),
            ("Llegó. Vienes?", "Vienes?", "¿Vienes?"),
            ("Le preguntó: vienes?", "vienes?", "¿vienes?"),
            ("Dijo «vienes?»", "vienes?", "¿vienes?"),
            ("Qué bien!", "Qué bien!", "¡Qué bien!"),
            (
                "¿Vienes? No sé qué hacer?",
                "No sé qué hacer?",
                "¿No sé qué hacer?",
            ),
            (
                "¿Viene el Sr. García? Llegó tarde!",
                "Llegó tarde!",
                "¡Llegó tarde!",
            ),
            ("¡¡Hola!! Qué bien!", "Qué bien!", "¡Qué bien!"),
            (
                "¿Qué?! No sé qué decir!",
                "No sé qué decir!",
                "¡No sé qué decir!",
            ),
        ] {
            let found = missing(text);
            assert_eq!(found.len(), 1, "{text:?}: {found:?}");
            assert_eq!(
                (found[0].0.as_str(), found[0].1.first().map(String::as_str)),
                (flagged, Some(offered)),
                "{text:?}"
            );
        }
    }

    #[test]
    fn questions_after_addresses_start_at_the_last_clause() {
        for (text, expected) in [
            ("https://a.test/?q=2? Vienes?", "Vienes?"),
            ("Envía el correo a ana@example.es. Cuándo?", "Cuándo?"),
            ("Cuesta 3.5; vienes?", "vienes?"),
        ] {
            let found: Vec<_> = check(text, ProofLang::Es)
                .into_iter()
                .filter(|r| r.message.contains("apertura"))
                .map(|r| text[r.start..r.end].to_owned())
                .collect();
            assert_eq!(found, [expected]);
        }
    }

    /// Maintainer review on #3562: a paragraph of thousands of addresses followed by a question was
    /// checked in quadratic time (every character before the question scanned every address).
    /// About 99 KB (under the 100,000-byte paragraph cap) took hundreds of milliseconds; it is now
    /// O(n log m): about 1 to 5 ms in a debug build, against about 300 ms for the scan at
    /// `opt-level = 1`. craft-libs builds tests at `opt-level = 0`, where the fixed lookup took up
    /// to ~80 ms under a parallel test run and more on a busy machine. The test guards against a
    /// return to the quadratic blowup, not machine load, so the bound is 1 s.
    #[cfg_attr(
        not(craft_proof_data),
        ignore = "needs the Language data (CRAFT_I18N_DIR)"
    )]
    #[test]
    fn many_addresses_before_a_question_are_checked_quickly() {
        for (unit, ending, whole_paragraph) in [
            ("https://x/ ", "Vienes?", true),
            ("https://x/? ", "Vienes?", false),
            ("www.x.com ", "Qué bien!", true),
        ] {
            // As many as fit in about 99 KB: 9,000 of the first, as in the review.
            let text = format!("{}{ending}", unit.repeat(99_000 / unit.len()));
            assert!(text.len() < 100_000, "{}", text.len());
            let started = std::time::Instant::now();
            let issues = check(&text, ProofLang::Es);
            let took = started.elapsed();
            assert!(
                took < std::time::Duration::from_secs(1),
                "{unit:?} took {took:?}"
            );
            let missing: Vec<&Issue> = issues
                .iter()
                .filter(|i| i.message.starts_with("Falta"))
                .collect();
            assert_eq!(missing.len(), 1, "{unit:?}: {missing:?}");
            // The mark's sentence is the whole paragraph when the addresses don't end with a mark
            // of their own, and only the last question when they do.
            let want_start = if whole_paragraph {
                0
            } else {
                text.len() - ending.len()
            };
            assert_eq!(
                (missing[0].start, missing[0].end),
                (want_start, text.len()),
                "{unit:?}"
            );
        }
    }

    /// A word repeated once composed and once decomposed is still repeated.
    #[test]
    fn repeated_words_match_across_normalization_forms() {
        let text = "también tambie\u{301}n";
        let issues = check(text, ProofLang::Es);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!((issues[0].start, issues[0].end), (0, text.len()));
        assert!(issues[0].message.starts_with("Palabra repetida"));
    }

    #[test]
    fn a_space_after_an_opening_mark_is_flagged() {
        let text = "¿ Vienes? ¡ Hola!";
        let issues = check(text, ProofLang::EsUy);
        let spaced: Vec<&str> = issues
            .iter()
            .filter(|i| i.message == "Espacio después de un signo de apertura")
            .map(|i| &text[i.start..i.end])
            .collect();
        assert_eq!(spaced, ["¿ ", "¡ "], "{issues:?}");
        assert!(
            issues.iter().all(|i| !i.message.starts_with("Falta")),
            "{issues:?}"
        );
        assert!(check("¿ ", ProofLang::Es).is_empty());
    }

    proptest::proptest! {
        #[test]
        fn never_panics_and_ranges_are_valid(s in "\\PC{0,80}|[¿¡?!a-z .,(«»):]{0,40}") {
            for issue in check(&s, ProofLang::Es) {
                proptest::prop_assert!(issue.start <= issue.end && issue.end <= s.len());
                proptest::prop_assert!(s.is_char_boundary(issue.start) && s.is_char_boundary(issue.end));
            }
        }
    }
}
