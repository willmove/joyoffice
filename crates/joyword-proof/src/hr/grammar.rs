//! Conservative Croatian grammar and punctuation checks.
//! Handles repeated words, whitespace, capitals and duplicated punctuation;
//! intentionally avoids speculative agreement and comma rules.

use crate::{Issue, IssueKind, ProofLang, words};

/// Words Croatian repeats on purpose: interjections, onomatopoeia and emphatic doubling.
const ALLOWED_REPEATS: &[&str] = &[
    "da", "ne", "no", "ha", "hej", "joj", "aj", "oh", "uh", "hop", "kuc", "tik", "tak", "bum",
    "malo", "brzo", "polako", "više", "dobro",
];
const ABBREVIATIONS: &[&str] = &[
    "npr", "itd", "itp", "tj", "tzv", "odn", "sl", "str", "br", "god", "st", "dr", "mr", "prof",
    "doc", "ing", "dipl", "mag", "univ", "g", "god", "mj", "min", "sek", "čl", "st", "č", "toč",
    "tab", "sl", "sv", "ul", "trg", "pr", "izd", "ur", "svez", "vol", "vid", "usp", "prim", "engl",
    "njem", "hrv", "lat", "tel", "mob", "e", "p", "o", "d", "m", "v", "z", "r", "pon", "uto",
    "sri", "čet", "pet", "sub", "ned", "sij", "velj", "ožu", "tra", "svi", "lip", "srp", "kol",
    "ruj", "lis", "stu", "pro",
];

fn issue(start: usize, end: usize, message: String, suggestions: Vec<String>) -> Issue {
    Issue {
        start,
        end,
        kind: IssueKind::Grammar,
        message,
        suggestions,
        lang: ProofLang::Hr,
    }
}

/// Grammar issues in a paragraph of Croatian text, in order.
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
        ProofLang::Hr,
        ALLOWED_REPEATS,
        |pair| {
            let (first, second) = (pair.first, pair.second);
            v.push(issue(
                pair.range.start,
                pair.range.end,
                format!("Ponovljena riječ: \"{second}\""),
                vec![first.to_owned()],
            ));
        },
    );
    crate::editorial::punctuation(text, ProofLang::Hr, &mut v);
    crate::editorial::capitals(
        text,
        &ws,
        ProofLang::Hr,
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
    #[test]
    fn checks_croatian_punctuation() {
        let t = "Ovo ovo je primjer , s dva  razmaka..";
        let issues = check(t);
        assert!(
            issues
                .iter()
                .any(|i| i.message.starts_with("Ponovljena riječ"))
        );
        assert!(issues.iter().any(|i| i.message == "Dvostruki razmak"));
        assert!(issues.iter().all(|i| i.lang == ProofLang::Hr));
    }
    #[test]
    fn abbreviations_do_not_trigger_capitals() {
        let issues = check("To je npr. sasvim dobar primjer.");
        assert!(
            !issues
                .iter()
                .any(|i| i.message == "Rečenica treba započeti velikim slovom")
        );
    }
}
