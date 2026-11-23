//! Adjacent words with the same spelling. Ranges always refer to the untouched
//! paragraph, including when a language compares canonically equivalent forms.
use crate::ProofLang;
use std::borrow::Cow;
use std::ops::Range;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark, is_nfc};

pub(crate) struct Repetition<'a> {
    pub(crate) range: Range<usize>,
    pub(crate) second_start: usize,
    pub(crate) first: &'a str,
    pub(crate) second: &'a str,
}

fn eligible(word: &str, lang: ProofLang) -> bool {
    if lang == ProofLang::Tr {
        return !word.chars().any(char::is_numeric) && word.chars().any(char::is_alphabetic);
    }
    word.chars()
        .all(|c| c.is_alphabetic() || (lang.is_spanish() && is_combining_mark(c)))
}

fn key(word: &str, lang: ProofLang) -> Cow<'_, str> {
    if lang == ProofLang::Tr {
        return Cow::Owned(crate::tr::suggest::turkish_to_lower(word));
    }
    if lang.is_spanish() && !is_nfc(word) {
        return Cow::Owned(word.nfc().collect::<String>().to_lowercase());
    }
    if word.is_ascii() && !word.bytes().any(|c| c.is_ascii_uppercase()) {
        Cow::Borrowed(word)
    } else {
        Cow::Owned(word.to_lowercase())
    }
}

/// Keep just the previous eligible token and its folded form. The language
/// owns its intentional repetitions, replacement text and highlighted range.
pub(crate) fn visit<'a>(
    text: &'a str,
    spans: impl IntoIterator<Item = (usize, usize)>,
    lang: ProofLang,
    allowed: &[&str],
    mut emit: impl FnMut(Repetition<'a>),
) {
    let mut previous: Option<(usize, usize, &str, Cow<'_, str>)> = None;
    for (start, end) in spans {
        let Some(word) = text.get(start..end) else {
            previous = None;
            continue;
        };
        let fold = key(word, lang);
        if let Some((begin, stop, first, last)) = previous
            && let Some(gap) = text.get(stop..start)
            && !gap.is_empty()
            && (if lang == ProofLang::Tr {
                gap.bytes().all(|c| c == b' ' || c == b'\t')
            } else {
                gap.chars().all(char::is_whitespace)
            })
            && last == fold
            && !allowed.contains(&fold.as_ref())
        {
            emit(Repetition {
                range: begin..end,
                second_start: start,
                first,
                second: word,
            });
        }
        previous = eligible(word, lang).then_some((start, end, word, fold));
    }
}
