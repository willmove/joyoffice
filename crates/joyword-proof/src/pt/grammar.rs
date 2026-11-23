//! Portuguese grammar and punctuation checks: cheap, rule-based and tuned to stay quiet when
//! unsure (a missed mistake costs less than a wrong mark on correct text). Messages are in
//! Portuguese.
//!
//! - **Repeated word** (`ele ele`), except interjections and words Portuguese doubles on purpose
//!   (`sim sim`, `não não`, `ora ora`).
//! - **Space before punctuation** (`bem ,`) and **double spaces**; emoticons (` :)`) and
//!   ellipses are left alone.
//! - **Capital letter at the start of a sentence**, knowing Portuguese abbreviations (`etc.`,
//!   `pág.`, `Prof.`, `Dr.`), initials, ordinal numbers (`5. colocado`), ellipses and dialogue
//!   dashes (`— Vai — disse`), after which lower case is correct.
//! - **Doubled punctuation** (`,,`, `..`, `;;`).
//!
//! No comma rule (`que` opens clauses only sometimes) and no English rule (such as `a`/`an`)
//! applies to Portuguese text.

use crate::{Issue, IssueKind, ProofLang, words};

/// Words Portuguese repeats on purpose: interjections, onomatopoeia and emphatic doubling.
const ALLOWED_REPEATS: &[&str] = &[
    "sim", "não", "ora", "olha", "já", "bem", "muito", "vai", "lá", "cá", "aí", "ha", "he", "hi",
    "oh", "ah", "ai", "ui", "uf", "uxa", "oba", "viva", "psiu", "olá", "tchau", "xau", "adeus",
    "devagar",
];

/// Abbreviations written with a final period after which a sentence goes on in lower case
/// (lower case, without the period).
const ABBREVIATIONS: &[&str] = &[
    "etc", "ex", "pág", "págs", "séc", "sécs", "fl", "av", "vol", "p", "pp", "ed", "tel", "min",
    "max", "art", "par", "prof", "profa", "dr", "dra", "sr", "sra", "srta", "eng", "arq", "op",
    "cit", "cf", "ibid", "id", "vide", "seg", "ter", "qua", "qui", "sex", "sáb", "dom", "jan",
    "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez", "s.a", "d.c",
    "a.c", "pg", "ref", "cod", "dept", "univ", "inst", "lat", "ing", "esp", "fr", "it", "al",
    "obs", "att", "obs",
];

fn issue(start: usize, end: usize, message: String, suggestions: Vec<String>) -> Issue {
    Issue {
        start,
        end,
        kind: IssueKind::Grammar,
        message,
        suggestions,
        lang: ProofLang::Pt,
    }
}

/// Grammar issues in a paragraph of Portuguese text, in order.
pub fn check(text: &str) -> Vec<Issue> {
    check_with(text, true)
}

/// [`check`] where `capital_first` says whether the text's first word must start a sentence with a capital
/// (false for a list item, which may be a fragment).
pub fn check_with(text: &str, capital_first: bool) -> Vec<Issue> {
    let ws: Vec<(usize, usize)> = words(text);
    let mut v = Vec::new();
    crate::repeated::visit(
        text,
        ws.iter().copied(),
        ProofLang::Pt,
        ALLOWED_REPEATS,
        |pair| {
            let (first, second) = (pair.first, pair.second);
            v.push(issue(
                pair.range.start,
                pair.range.end,
                format!("Palavra repetida: \"{second}\""),
                vec![first.to_owned()],
            ));
        },
    );
    crate::editorial::punctuation(text, ProofLang::Pt, &mut v);
    crate::editorial::capitals(
        text,
        &ws,
        ProofLang::Pt,
        capital_first,
        ABBREVIATIONS,
        &mut v,
    );
    v.sort_by_key(|i| (i.start, i.end));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(t: &str) -> Vec<String> {
        check(t)
            .into_iter()
            .map(|i| format!("{}@{}", i.message, &t[i.start..i.end]))
            .collect()
    }

    fn clean(t: &str) {
        let m = messages(t);
        assert!(m.is_empty(), "{t:?}: {m:?}");
    }

    #[test]
    fn repeated_words() {
        let v = check("Eu eu vi isso e ele ele saiu.");
        assert_eq!(v.len(), 2, "{v:?}");
        assert_eq!(v[0].message, "Palavra repetida: \"eu\"");
        assert_eq!(v[0].suggestions, ["Eu"]);
        assert!(
            v.iter()
                .all(|i| i.lang == ProofLang::Pt && i.kind == IssueKind::Grammar)
        );
        clean("Sim, sim! Não, não! Ora ora, que surpresa.");
    }

    #[test]
    fn spaces_and_punctuation() {
        let m = messages("Olá , mundo .  Tudo bem ?");
        assert!(
            m.contains(&"Espaço antes da pontuação@ ,".to_string()),
            "{m:?}"
        );
        assert!(
            m.contains(&"Espaço antes da pontuação@ .".to_string()),
            "{m:?}"
        );
        assert!(
            m.contains(&"Espaço antes da pontuação@ ?".to_string()),
            "{m:?}"
        );
        assert!(m.contains(&"Espaço duplo@  ".to_string()), "{m:?}");
        clean("Legal :) Muito bom :-) Ou :D Valor .5 e reticências ...");
        let m = messages("Falou,, e saiu.. depois;; fim.");
        assert_eq!(
            m.iter().filter(|x| x.starts_with("Sinal repetido")).count(),
            3,
            "{m:?}"
        );
        clean("Esperei… e esperei... até ele voltar.");
    }

    #[test]
    fn sentence_start_capitals() {
        let m = messages("isso é um teste. a segunda frase! terceira? ok.");
        assert_eq!(
            m.iter()
                .filter(|x| x.starts_with("Letra maiúscula"))
                .count(),
            4,
            "{m:?}"
        );
        assert_eq!(check("ele tem razão.")[0].suggestions, ["Ele"]);
        // Abbreviations, initials, ordinals, ellipses and dialogue dashes.
        clean("Comprei frutas, etc. e depois saímos.");
        clean("Conheci o Dr. Silva na Av. Paulista.");
        clean("Zhou ficou em 5. lugar no séc. XX.");
        clean("Esperei... e ele apareceu.");
        clean("— Vai — disse ele. — Amanhã eu volto.");
        // craft-libs#488 review: no space after the dash.
        clean("—Vai? —disse ele.");
        clean("— Vai? — disse ele.");
        // craft-libs#496 review: a closing quote after the mark.
        clean("\"Vai?\" disse ele.");
        clean("«Vai!» gritou ela.");
        assert!(
            !messages("Vai? \"sim\".").is_empty(),
            "a bare question still ends the sentence"
        );
        // An inline object (`\u{1}`) before a real closing quote is skipped.
        clean("\"Vai?\u{1}\" disse ele.");
        // An inline object (`\u{1}`) is not a closing quote.
        assert!(!messages("Quem era?\u{1} era Marco.").is_empty());
        clean("Ele disse: \"Volto amanhã\". E saiu.");
        // A list item may start in lower case.
        clean("maçãs,");
    }

    #[test]
    fn correct_portuguese_text_is_clean() {
        clean(
            "O rato roeu a roupa do rei de Roma. Quem não tem cão caça com gato, \
             e quem tem paciência colhe o que plantou.",
        );
        clean("Brasília, 12 de outubro de 2026 — uma data memorável.");
    }

    proptest::proptest! {
        #[test]
        fn never_panics(s in "\\PC{0,80}") {
            let _ = check(&s);
        }

        #[test]
        fn never_panics_on_portuguese_like_text(s in "[a-záàâãéêíóôõúüçA-Z ,.;:!?\"…\\-—]{0,60}") {
            for i in check(&s) {
                proptest::prop_assert!(i.start <= i.end && i.end <= s.len());
                proptest::prop_assert!(s.is_char_boundary(i.start) && s.is_char_boundary(i.end));
            }
        }
    }
}
