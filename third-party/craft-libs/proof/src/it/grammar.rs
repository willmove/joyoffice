//! Italian grammar and punctuation checks: cheap, rule-based and tuned to stay quiet when
//! unsure (a missed mistake costs less than a wrong mark on correct text). Messages are in
//! Italian.
//!
//! - **Repeated word** (`la la`), except words Italian doubles on purpose (`piano piano`,
//!   `sì sì`, interjections).
//! - **Space before punctuation** (`bene ,`) and **double spaces**; emoticons (` :)`) and
//!   ellipses are left alone.
//! - **Capital letter at the start of a sentence**, knowing Italian abbreviations (`ecc.`,
//!   `dott.`, `prof.`, `sig.`), initials, ordinal numbers (`1. posto`) and ellipses.
//! - **Doubled punctuation** (`,,`, `..`, `;;`).
//! - **`po'`**: `un pò` and `un po` → `un po'` (a truncation of `poco`: apostrophe, no accent).
//! - **`qual è`**: `qual'è`, `qual'era` → `qual è`, `qual era` (a truncation, never elided).
//! - **`È`**: `E'` written for the verb → `È`.
//! - **`un` / `un'`**: the apostrophe only before a feminine word. `un'altro`, `un'anno` →
//!   `un altro`, `un anno` (a masculine word in `-o`; `auto` and `eco` are feminine), and
//!   `un altra`, `un amica` → `un'altra`, `un'amica` (a word in `-a`, except the masculine and
//!   common-gender ones: `-ista`, `-ita`, `-ota`, `-cida`, `-ma`, `-eta`, `atleta`, `acrobata`…).
//!   The same for `nessun`, `ciascun`, `qualcun` and `alcun`.
//!
//! No comma rule and no English rule (such as `a`/`an`) applies to Italian text.

use crate::{Issue, IssueKind, ProofLang, words};

/// Words Italian repeats on purpose: emphatic doubling, interjections, onomatopoeia.
const ALLOWED_REPEATS: &[&str] = &[
    "sì", "no", "piano", "bene", "male", "ecco", "oh", "ah", "eh", "mh", "bah", "ehi", "urrà",
    "evviva", "ciao", "addio", "adagio", "forte", "lento", "chiaro", "pian", "poco", "quasi",
    "presto", "subito", "zitto", "via", "così", "tanto", "molto", "pure", "dai", "su", "avanti",
    "basta", "aiuto",
];

/// Indefinite articles (and the like) that elide to `un'` before a feminine word only.
const UN_FORMS: &[&str] = &["un", "nessun", "ciascun", "qualcun", "alcun"];

/// Feminine words in `-o` that start with a vowel (`un'auto`, `un'eco`).
const FEMININE_IN_O: &[&str] = &["auto", "autoradio", "eco", "ecografo"];

/// Endings of masculine or common-gender words in `-a` (`un artista`, `un idiota`, `un enigma`).
const MASCULINE_A_ENDINGS: &[&str] = &[
    "ista", "ita", "ota", "cida", "ma", "eta", "nauta", "iatra", "arca",
];

/// Words in `-a` after `un` that aren't feminine nouns: masculine or common-gender ones that fit
/// no ending above, and verbs and pronouns (`un ha`, `un una`).
const NOT_FEMININE_IN_A: &[&str] = &["acrobata", "autodidatta", "alfa", "omega", "ha", "una"];

/// Abbreviations written with a final period after which a sentence goes on in lower case
/// (lower case, without the period).
const ABBREVIATIONS: &[&str] = &[
    "pp",
    "tab",
    "dr",
    "ecc",
    "es",
    "dott",
    "prof",
    "profssa",
    "ing",
    "avv",
    "sig",
    "sigg",
    "sigra",
    "sigr",
    "geom",
    "rag",
    "dottssa",
    "avvoc",
    "pag",
    "pagg",
    "tel",
    "art",
    "artt",
    "fig",
    "figg",
    "vol",
    "voll",
    "ed",
    "sez",
    "cap",
    "capp",
    "par",
    "n",
    "num",
    "vs",
    "cfr",
    "cfrt",
    "op",
    "cit",
    "ibid",
    "sez",
    "all",
    "tav",
    "pag",
    "ditta",
    "soc",
    "spa",
    "srl",
    "on",
    "sen",
    "spett",
    "eg",
    "ca",
    "v",
    "vv",
    "sec",
    "min",
    "max",
    "lun",
    "mar",
    "mer",
    "gio",
    "ven",
    "sab",
    "dom",
    "gen",
    "feb",
    "mar",
    "apr",
    "mag",
    "giu",
    "lug",
    "ago",
    "set",
    "ott",
    "nov",
    "dic",
    "genn",
    "febb",
    "sett",
    "dottor",
    "professor",
    "cav",
    "commend",
    "mons",
    "fr",
    "suor",
    "s",
    "ss",
    "b",
    "beato",
];

fn issue(start: usize, end: usize, message: String, suggestions: Vec<String>) -> Issue {
    Issue {
        start,
        end,
        kind: IssueKind::Grammar,
        message,
        suggestions,
        lang: ProofLang::It,
    }
}

pub(crate) use joyword_proof_rules::italian_qual as qual_elided;

/// Grammar issues in a paragraph of Italian text, in order.
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
        ProofLang::It,
        ALLOWED_REPEATS,
        |pair| {
            let (first, second) = (pair.first, pair.second);
            v.push(issue(
                pair.range.start,
                pair.range.end,
                format!("Parola ripetuta: \"{second}\""),
                vec![first.to_owned()],
            ));
        },
    );
    crate::editorial::punctuation(text, ProofLang::It, &mut v);
    crate::editorial::capitals(
        text,
        &ws,
        ProofLang::It,
        capital_first,
        ABBREVIATIONS,
        &mut v,
    );
    use joyword_proof_rules::{ItalianForms, ItalianIssue};
    let forms = ItalianForms {
        articles: UN_FORMS,
        feminine_o: FEMININE_IN_O,
        masculine_endings: MASCULINE_A_ENDINGS,
        ambiguous_a: NOT_FEMININE_IN_A,
    };
    joyword_proof_rules::italian_elisions(text, &ws, forms, |span, kind, fix| {
        let label = match kind {
            ItalianIssue::Truncation => "Si scrive «po'», con l'apostrofo e senza accento".into(),
            ItalianIssue::Qual => "«Qual è» si scrive senza apostrofo".into(),
            ItalianIssue::Accent => "«È» si scrive con l'accento, non con l'apostrofo".into(),
            ItalianIssue::Masculine(head) => {
                format!("Davanti a un nome maschile «{head}» non vuole l'apostrofo")
            }
            ItalianIssue::Feminine(head) => {
                format!("Davanti a un nome femminile si scrive «{head}'», con l'apostrofo")
            }
        };
        v.push(issue(span.start, span.end, label, vec![fix]));
    });
    v.sort_by_key(|i| (i.start, i.end));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn italian_grammar_flags() {
        test_dict_install();
        let t = "questo questo è un test ,  fatto male.. Vedi dott. rossi";
        let v = check(t);
        let msgs: Vec<&str> = v.iter().map(|i| i.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("ripetuta")), "{msgs:?}");
        assert!(
            msgs.iter().any(|m| m.contains("prima della punteggiatura")),
            "{msgs:?}"
        );
        assert!(msgs.iter().any(|m| m.contains("doppio")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("doppia")), "{msgs:?}");
        assert!(v.iter().all(|i| i.lang == ProofLang::It), "{v:?}");
    }

    #[test]
    fn italian_grammar_stays_quiet() {
        test_dict_install();
        assert!(
            check("Vado a Roma. Il dott. Rossi è già lì... piano piano!").is_empty(),
            "{:?}",
            check("Vado a Roma. Il dott. Rossi è già lì... piano piano!")
        );
        assert!(check("Sì sì, bene bene :)").is_empty());
    }

    /// The fixes the rules offer for `t`, in order.
    fn fixes(t: &str) -> Vec<(String, String)> {
        check(t)
            .into_iter()
            .map(|i| {
                (
                    t.get(i.start..i.end).unwrap_or("").to_string(),
                    i.suggestions.join("|"),
                )
            })
            .collect()
    }

    fn pair(a: &str, b: &str) -> (String, String) {
        (a.to_string(), b.to_string())
    }

    /// CodeRabbit on craft-libs#484: a sentence after the first one starts with a capital too.
    #[test]
    fn later_sentences_start_with_a_capital() {
        assert!(fixes("Vado a Roma. il cane dorme.").contains(&pair("il", "Il")));
        assert!(fixes("Vado a Roma? sì.").contains(&pair("sì", "Sì")));
        assert!(
            !fixes("Vado a Roma. Il cane dorme.")
                .iter()
                .any(|(_, b)| b == "Il")
        );
        for t in [
            "Il sig. rossi arriva.",
            "Vedi pp. dieci e oltre.",
            "Nella tab. due.",
            "Il dr. bianchi arriva.",
            "Mele, pere ecc. sono frutti.",
        ] {
            assert!(
                !fixes(t)
                    .iter()
                    .any(|(_, b)| b.starts_with(char::is_uppercase)
                        && b != "Il"
                        && b != "Vedi"
                        && b != "Nella"
                        && b != "Mele"),
                "after an abbreviation: {t}"
            );
        }
    }

    /// craft-libs#496 review: narration after a quoted or dashed question or exclamation goes on in
    /// lower case.
    #[test]
    fn narration_after_a_quoted_question_is_not_a_new_sentence() {
        for t in [
            "«Vai?» disse lui.",
            "«Vieni!» gridò Anna.",
            "\"Vai?\" disse lui.",
            "—Vai? —disse lui.",
            "— Vai! — disse lui.",
        ] {
            assert!(fixes(t).is_empty(), "{t}: {:?}", fixes(t));
        }
        assert!(
            fixes("Vado a Roma? sì.").contains(&pair("sì", "Sì")),
            "a bare question still ends the sentence"
        );
        // After a period the dash opens the next line of speech: a sentence starts.
        assert!(
            fixes("Finì. —domani partiamo.").contains(&pair("domani", "Domani")),
            "{:?}",
            fixes("Finì. —domani partiamo.")
        );
        assert!(
            fixes("Finì. — domani partiamo.").contains(&pair("domani", "Domani")),
            "{:?}",
            fixes("Finì. — domani partiamo.")
        );
        // An inline object (`\u{1}`) before a real closing quote is skipped.
        assert!(
            fixes("«Vai?\u{1}» disse lui.").is_empty(),
            "{:?}",
            fixes("«Vai?\u{1}» disse lui.")
        );
        // An inline object (`\u{1}`) is not a closing quote.
        assert!(
            fixes("Chi era?\u{1} era Marco.").contains(&pair("era", "Era")),
            "{:?}",
            fixes("Chi era?\u{1} era Marco.")
        );
    }

    #[test]
    fn po_takes_an_apostrophe() {
        assert_eq!(fixes("Dammi un pò di pane"), [pair("pò", "po'")]);
        assert_eq!(fixes("Dammi un po di pane"), [pair("po", "po'")]);
        assert_eq!(fixes("Pò di più"), [pair("Pò", "Po'")]);
        assert!(fixes("Dammi un po' di pane, un po’ d'acqua").is_empty());
        assert!(fixes("Il Po scorre a Torino").is_empty(), "the river");
    }

    #[test]
    fn qual_e_has_no_apostrophe() {
        assert_eq!(
            fixes("Dimmi qual'è il problema"),
            [pair("qual'è", "qual è")]
        );
        assert_eq!(fixes("Qual’era la strada?"), [pair("Qual’era", "Qual era")]);
        assert!(fixes("Qual è il problema? Quale era?").is_empty());
        assert!(fixes("Quale'è").is_empty(), "not the truncation");
        assert_eq!(qual_elided("qual'è").as_deref(), Some("qual è"));
        assert_eq!(qual_elided("qua"), None);
        assert_eq!(qual_elided(""), None);
    }

    #[test]
    fn e_apostrophe_becomes_e_grave() {
        assert_eq!(fixes("E' vero"), [pair("E'", "È")]);
        assert_eq!(fixes("Lo so. E’ tardi"), [pair("E’", "È")]);
        assert!(fixes("È vero. E poi?").is_empty());
        assert!(
            fixes("La lettera 'E' è una vocale").is_empty(),
            "a quoted letter"
        );
    }

    #[test]
    fn un_apostrophe_follows_the_gender() {
        assert_eq!(
            fixes("Ho visto un'altro amico"),
            [pair("un'altro", "un altro")]
        );
        assert_eq!(fixes("È passato un’anno"), [pair("un’anno", "un anno")]);
        assert_eq!(
            fixes("Prendo un altra strada"),
            [pair("un altra", "un'altra")]
        );
        assert_eq!(fixes("Ho un amica a Roma"), [pair("un amica", "un'amica")]);
        assert_eq!(
            fixes("Non c'è nessun altra via"),
            [pair("nessun altra", "nessun'altra")]
        );
        for ok in [
            "Un altro amico",
            "Un'altra amica",
            "Un'auto nuova",
            "Un'eco lontana",
            "Un artista",
            "Un atleta",
            "Un idiota",
            "Un ipocrita",
            "Un enigma",
            "Un omicida",
            "Un astronauta",
            "Un acrobata",
            "Un'ora",
            "Un'idea",
            "Un uomo",
            "Un'Alfa",
            "Un Andrea",
            "Un ha",
        ] {
            assert!(fixes(ok).is_empty(), "{ok}: {:?}", fixes(ok));
        }
    }

    #[test]
    fn more_emphatic_doubles_are_allowed() {
        for ok in [
            "Pian piano",
            "Poco poco",
            "Zitto zitto",
            "Via via",
            "Così così",
            "Dai dai",
        ] {
            assert!(fixes(ok).is_empty(), "{ok}: {:?}", fixes(ok));
        }
    }

    proptest::proptest! {
        #[test]
        fn never_panics(s in "\\PC{0,80}") {
            for i in check(&s) {
                proptest::prop_assert!(i.start <= i.end && i.end <= s.len());
            }
        }

        #[test]
        fn never_panics_on_italian_like_text(s in "[a-zàèéìòóùA-ZÈ ,.;:!?'’‘\"«»…\\-]{0,60}") {
            for i in check(&s) {
                proptest::prop_assert!(i.start <= i.end && i.end <= s.len() && s.is_char_boundary(i.start) && s.is_char_boundary(i.end));
            }
        }
    }

    fn test_dict_install() {
        crate::it::test_dict::install();
    }
}
