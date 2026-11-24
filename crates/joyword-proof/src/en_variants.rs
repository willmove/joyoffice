//! Regional spelling preferences for the English proofing dictionaries.
//!
//! The licensed locale table supplies base pairs. Regular English inflection and the
//! locale-specific acceptance rules add the forms consulted by spelling and suggestions.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::{ProofLang, data::EN_VARIANTS};

#[derive(Default)]
enum Spelling {
    #[default]
    Unlisted,
    Accepted(String),
    Suggest(String),
}

type Index = BTreeMap<String, [Spelling; 3]>;

fn catalog_rows() -> impl Iterator<Item = (&'static [u8; 3], &'static str, &'static str)> {
    EN_VARIANTS.lines().filter_map(|line| {
        let mut fields = line.split_whitespace();
        let codes: &[u8; 3] = fields.next()?.as_bytes().try_into().ok()?;
        let american = fields.next()?;
        let british = fields.next()?;
        if fields.next().is_some()
            || american == british
            || !codes.iter().all(|c| matches!(c, b'u' | b'b' | b'*'))
            || !american
                .bytes()
                .chain(british.bytes())
                .all(|b| b.is_ascii_lowercase())
        {
            return None;
        }
        Some((codes, american, british))
    })
}

fn affixes(us: &str, uk: &str) -> &'static [&'static str] {
    if us
        .strip_suffix("or")
        .is_some_and(|root| uk.strip_suffix("our") == Some(root))
    {
        return &[
            "s", "ed", "ing", "er", "ers", "ful", "fully", "less", "able", "ably",
        ];
    }
    if us
        .strip_suffix("er")
        .is_some_and(|root| uk.strip_suffix("re") == Some(root))
    {
        return &["s", "ed", "ing"];
    }
    if us.len() == uk.len() && us.bytes().zip(uk.bytes()).filter(|(a, b)| a != b).count() == 1 {
        if us.ends_with('y') && uk.ends_with('y') {
            return &["s", "er", "est", "ly", "ness"];
        }
        if us.ends_with('e') && uk.ends_with('e') {
            if us.ends_with("ize") || us.ends_with("yze") {
                return &[
                    "s", "ed", "ing", "er", "ers", "ation", "ations", "ational", "able",
                ];
            }
            return &["s", "ed", "ing", "er", "ers"];
        }
    }
    &["s"]
}

fn inflect(stem: &str, ending: &str) -> String {
    let is_vowel = |b: &u8| b"aeiou".contains(b);
    if let Some(root) = stem.strip_suffix('y')
        && !root.as_bytes().last().is_some_and(is_vowel)
    {
        match ending {
            "s" => return format!("{root}ies"),
            _ if !ending.starts_with('i') => return format!("{root}i{ending}"),
            _ => {}
        }
    }
    if ending == "s"
        && (stem.ends_with(['s', 'x', 'z']) || stem.ends_with("ch") || stem.ends_with("sh"))
    {
        return format!("{stem}es");
    }
    match stem.strip_suffix('e') {
        Some(root) if ending.as_bytes().first().is_some_and(is_vowel) => format!("{root}{ending}"),
        _ => format!("{stem}{ending}"),
    }
}

fn register(index: &mut Index, spelling: &str, partner: &str, codes: &[u8; 3], side: u8) {
    if spelling == partner {
        return;
    }
    let entries = index.entry(spelling.to_owned()).or_default();
    for (entry, &code) in entries.iter_mut().zip(codes) {
        if code == side || code == b'*' {
            *entry = Spelling::Accepted(partner.to_owned());
        } else if !matches!(entry, Spelling::Accepted(_)) && !spelling.ends_with("yses") {
            // "Analyses" and "paralyses" are ordinary plurals in every region.
            *entry = Spelling::Suggest(partner.to_owned());
        }
    }
}

fn lookup(word: &str, lang: ProofLang) -> Option<(&'static Spelling, bool)> {
    let locale = match lang {
        ProofLang::En => 0,
        ProofLang::EnCa => 1,
        ProofLang::EnGb | ProofLang::EnAu => 2,
        _ => return None,
    };
    static INDEX: OnceLock<Index> = OnceLock::new();
    let table = INDEX.get_or_init(|| {
        let mut index = Index::new();
        for (codes, us, uk) in catalog_rows() {
            register(&mut index, us, uk, codes, b'u');
            register(&mut index, uk, us, codes, b'b');
            for suffix in affixes(us, uk) {
                let left = inflect(us, suffix);
                let right = inflect(uk, suffix);
                register(&mut index, &left, &right, codes, b'u');
                register(&mut index, &right, &left, codes, b'b');
            }
        }
        index
    });
    let base = word
        .strip_suffix("'s")
        .filter(|s| !s.is_empty())
        .unwrap_or(word);
    Some((table.get(base)?.get(locale)?, base.len() != word.len()))
}

/// Return the local spelling to suggest for an unexpected regional form.
pub fn preferred(lower: &str, lang: ProofLang) -> Option<String> {
    let (Spelling::Suggest(spelling), possessive) = lookup(lower, lang)? else {
        return None;
    };
    Some(if possessive {
        format!("{spelling}'s")
    } else {
        spelling.clone()
    })
}

/// Accept a local spelling when its paired form is in the active dictionary.
pub fn known_variant(lower: &str, lang: ProofLang, known: impl Fn(&str) -> bool) -> bool {
    matches!(lookup(lower, lang), Some((Spelling::Accepted(other), _)) if known(other))
}

/// Report whether a regional preference flags this spelling.
pub fn is_foreign(lower: &str, lang: ProofLang) -> bool {
    matches!(lookup(lower, lang), Some((Spelling::Suggest(_), _)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(craft_proof_data)]
    #[test]
    fn every_catalog_row_is_usable() {
        let lines = EN_VARIANTS
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .count();
        assert!(lines > 250);
        assert_eq!(catalog_rows().count(), lines);
        assert_eq!(
            preferred("colour's", ProofLang::En).as_deref(),
            Some("color's")
        );
        assert!(known_variant("realisation", ProofLang::EnGb, |s| s == "realization"));
        assert!(!is_foreign("analyses", ProofLang::En));
    }

    #[test]
    fn ordinary_inflections() {
        assert_eq!(inflect("centre", "ing"), "centring");
        assert_eq!(inflect("cosy", "s"), "cosies");
        assert_eq!(inflect("finish", "s"), "finishes");
        assert_eq!(inflect("play", "ed"), "played");
        assert_eq!(affixes("program", "programme"), ["s"]);
    }
}
