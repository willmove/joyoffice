use crate::Language;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkIssue {
    ExtraSpace,
    BeforeMark,
    AfterOpening,
    RepeatedMark,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Punctuation {
    pub span: Range<usize>,
    pub kind: MarkIssue,
    pub replacements: &'static [&'static str],
}

impl Punctuation {
    fn new(span: Range<usize>, kind: MarkIssue, replacements: &'static [&'static str]) -> Self {
        Self {
            span,
            kind,
            replacements,
        }
    }
}

fn single(mark: u8) -> &'static [&'static str] {
    match mark {
        b',' => &[","],
        b';' => &[";"],
        b'.' => &["."],
        b':' => &[":"],
        b'?' => &["?"],
        b'!' => &["!"],
        _ => &[],
    }
}

fn emoticon(after: &str) -> bool {
    let mut chars = after.chars();
    match chars.next() {
        Some(')' | '(') => true,
        Some('-') => matches!(chars.next(), Some(')' | '(')),
        Some('D' | 'P') => chars.next().is_none_or(char::is_whitespace),
        _ => false,
    }
}

fn unwanted_space(lang: Language, text: &str, mark_at: usize, mark: u8) -> bool {
    let tail = &text[mark_at + 1..];
    let next = tail.chars().next();
    if lang == Language::English {
        return matches!(mark, b',' | b'.' | b';')
            && !tail.starts_with(['.', ','])
            && !(mark == b'.' && next.is_some_and(|c| c.is_ascii_digit()))
            && !(mark == b';' && emoticon(tail));
    }
    if mark == b'.' {
        return next.is_none_or(|c| c != '.' && !c.is_ascii_digit());
    }
    let bytes = text.as_bytes();
    if mark == b':'
        && mark_at >= 2
        && bytes[mark_at - 2].is_ascii_digit()
        && tail.starts_with(' ')
        && tail.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
    {
        return false;
    }
    if lang == Language::Turkish {
        return !matches!(mark, b':' | b';') || !emoticon(tail);
    }
    let typed_quotes = matches!(lang, Language::Polish | Language::Russian);
    match mark {
        b',' => !(typed_quotes && next == Some(',')),
        b';' => !matches!(next, Some(')' | '(' | '-' | 'P' | 'D')),
        _ => next.is_none_or(|c| {
            c.is_whitespace()
                || matches!(c, '"' | '”' | '’' | '!' | '?')
                || (c == '»' && (typed_quotes || lang == Language::Spanish))
        }),
    }
}

fn repeated(
    lang: Language,
    text: &str,
    span: Range<usize>,
    mark: u8,
    emit: &mut impl FnMut(Punctuation),
) {
    let length = span.len();
    if length < 2 {
        return;
    }
    if lang == Language::Italian {
        // A run of three or more periods is an ellipsis. Italian also marks
        // pairs of question marks, exclamation points and colons.
        if mark != b'.' || length == 2 {
            for start in (span.start..span.end - 1).step_by(2) {
                emit(Punctuation::new(
                    start..start + 2,
                    MarkIssue::RepeatedMark,
                    single(mark),
                ));
            }
        }
        return;
    }
    if !matches!(
        lang,
        Language::Polish | Language::Russian | Language::Portuguese | Language::Croatian
    ) || !matches!(mark, b',' | b';' | b'.')
        || (mark == b'.' && length != 2)
    {
        return;
    }
    let opening_quote = mark == b','
        && matches!(lang, Language::Polish | Language::Russian)
        && (span.start == 0 || text[..span.start].ends_with(char::is_whitespace))
        && text[span.end..].starts_with(char::is_alphabetic);
    let fixes: &'static [&'static str] = match (mark, opening_quote, lang) {
        (_, true, Language::Polish) => &["„"],
        (_, true, Language::Russian) => &["«"],
        (b'.', _, _) => &[".", "…"],
        _ => single(mark),
    };
    emit(Punctuation::new(span, MarkIssue::RepeatedMark, fixes));
}

/// Visit horizontal ASCII space runs and punctuation in text order. Multibyte
/// scalars only need decoding at a mark with a locale-sensitive neighbor.
pub fn punctuation(text: &str, lang: Language, mut emit: impl FnMut(Punctuation)) {
    let bytes = text.as_bytes();
    let mut i = 0;
    while let Some(&mark) = bytes.get(i) {
        let start = i;
        i += 1;
        match mark {
            b' ' => {
                while bytes.get(i) == Some(&b' ') {
                    i += 1;
                }
                if i - start > 1 {
                    emit(Punctuation::new(start..i, MarkIssue::ExtraSpace, &[" "]));
                }
                if let Some(&next @ (b',' | b'.' | b';' | b':' | b'!' | b'?')) = bytes.get(i)
                    && unwanted_space(lang, text, i, next)
                {
                    emit(Punctuation::new(
                        i - 1..i + 1,
                        MarkIssue::BeforeMark,
                        single(next),
                    ));
                }
            }
            b',' | b'.' | b';' | b':' | b'!' | b'?' => {
                while bytes.get(i) == Some(&mark) {
                    i += 1;
                }
                repeated(lang, text, start..i, mark, &mut emit);
            }
            0xc2 if lang == Language::Spanish && matches!(bytes.get(i), Some(0xbf | 0xa1)) => {
                let fixes: &'static [&'static str] =
                    if bytes[i] == 0xbf { &["¿"] } else { &["¡"] };
                i += 1;
                if bytes.get(i) == Some(&b' ')
                    && text[i..].trim_start().starts_with(char::is_alphanumeric)
                {
                    emit(Punctuation::new(
                        start..i + 1,
                        MarkIssue::AfterOpening,
                        fixes,
                    ));
                }
            }
            _ => {}
        }
    }
}
