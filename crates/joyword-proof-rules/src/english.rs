//! English indefinite articles follow the initial sound. Ambiguous pronunciations
//! have no automatic correction.
use std::ops::Range;
fn vowel_sound(word: &str) -> Option<bool> {
    let initial = word.bytes().next()?.to_ascii_lowercase();
    if !initial.is_ascii_alphabetic() {
        return None;
    }
    if word.len() > 1
        && word.as_bytes()[0].is_ascii_uppercase()
        && word.bytes().all(|b| b.is_ascii_uppercase())
    {
        return match word {
            "FBI" | "MRI" | "MBA" | "MP" | "MPH" | "MVP" | "HTML" | "HTTP" | "LCD" | "NSA"
            | "NHS" | "RN" | "RNA" | "SOS" | "XML" => Some(true),
            "BBC" | "CEO" | "CPU" | "DNA" | "DVD" | "GPU" | "PDF" | "TV" | "UFO" | "UK" | "UN"
            | "US" | "USA" | "USB" => Some(false),
            _ => None,
        };
    }
    let begins = |p: &str| {
        word.get(..p.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(p))
    };
    match initial {
        b'a' | b'i' => Some(true),
        b'e' => Some(
            !["euro", "eul", "euph", "ewe", "euca"]
                .iter()
                .any(|p| begins(p)),
        ),
        b'o' => Some(
            !["one", "once", "ones"]
                .iter()
                .any(|p| word.eq_ignore_ascii_case(p)),
        ),
        b'u' => Some(
            ![
                "univers", "uniform", "union", "unit", "unif", "uniqu", "unanim", "unilat",
                "unicorn", "use", "usual", "utensil", "util", "ubiq", "uran", "urea", "uret",
                "urol",
            ]
            .iter()
            .any(|p| begins(p)),
        ),
        // Retain the conservative house style for h with dialectal or unknown pronunciation.
        b'h' => (["honest", "honor", "honour", "heir"]
            .iter()
            .any(|p| begins(p))
            || ["hour", "hours", "hourly", "hourglass"]
                .iter()
                .any(|p| word.eq_ignore_ascii_case(p)))
        .then_some(true),
        _ => Some(false),
    }
}

pub fn english_article(article: &str, following: &str) -> Option<&'static str> {
    match article {
        "a" | "A" if vowel_sound(following)? => Some(if article == "a" { "an" } else { "An" }),
        "an" | "An" if !vowel_sound(following)? => Some(if article == "an" { "a" } else { "A" }),
        _ => None,
    }
}

pub enum EnglishIssue<'a> {
    Repeated { first: &'a str, second: &'a str },
    Article(&'static str),
}

/// Check articles and repetitions together. Most neighbouring words differ, so
/// compare their borrowed source slices before testing the repetition exceptions.
pub fn english_words<'a>(
    text: &'a str,
    spans: &[(usize, usize)],
    mut emit: impl FnMut(Range<usize>, EnglishIssue<'a>),
) {
    for pair in spans.windows(2) {
        let ((a, b), (c, d)) = (pair[0], pair[1]);
        let (Some(first), Some(gap), Some(second)) =
            (text.get(a..b), text.get(b..c), text.get(c..d))
        else {
            continue;
        };
        if first.eq_ignore_ascii_case(second)
            && !gap.is_empty()
            && gap.chars().all(char::is_whitespace)
            && first.chars().all(char::is_alphabetic)
            && !["had", "that", "bye", "so"]
                .iter()
                .any(|word| first.eq_ignore_ascii_case(word))
        {
            emit(a..d, EnglishIssue::Repeated { first, second });
        }
        if gap == " "
            && let Some(fix) = english_article(first, second)
        {
            emit(a..b, EnglishIssue::Article(fix));
        }
    }
}
