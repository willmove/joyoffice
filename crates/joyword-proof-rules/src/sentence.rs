use crate::Language;
use std::ops::Range;

fn closing(c: char) -> bool {
    matches!(c, '"' | '”' | '’' | '\'' | ')' | '»' | ']' | '\u{1}')
}

fn opening(lang: Language, c: char) -> bool {
    match c {
        '"' | '“' | '«' | '(' | '[' | '\'' => true,
        '‚' => lang != Language::Italian,
        '„' => matches!(lang, Language::Polish | Language::Russian),
        '—' => matches!(
            lang,
            Language::Portuguese | Language::Croatian | Language::Italian
        ),
        _ => false,
    }
}

fn listed(word: &str, abbreviations: &[&str], lang: Language) -> bool {
    let folded = if lang == Language::Turkish {
        word.chars()
            .flat_map(|c| match c {
                'I' => 'ı'.to_lowercase(),
                'İ' => 'i'.to_lowercase(),
                _ => c.to_lowercase(),
            })
            .collect()
    } else {
        word.to_lowercase()
    };
    abbreviations.contains(&folded.as_str())
}

fn turkish_break(previous: &str, gap: &str, abbreviations: &[&str]) -> bool {
    let trimmed = gap.trim();
    let marks = ['.', '?', '!'];
    if !trimmed.ends_with(marks) || trimmed.contains("...") {
        return false;
    }
    let Some(last) = gap.rfind(marks) else {
        return false;
    };
    gap[last + 1..].contains(char::is_whitespace)
        && !(trimmed.ends_with('.') && previous.bytes().any(|b| b.is_ascii_digit()))
        && !(trimmed.starts_with('.') && previous.chars().count() == 1)
        && !listed(previous, abbreviations, Language::Turkish)
}

fn boundary(lang: Language, previous: &str, gap: &str, abbreviations: &[&str]) -> bool {
    if lang == Language::Turkish {
        return turkish_break(previous, gap, abbreviations);
    }
    if lang == Language::English {
        let tail = gap.trim_start_matches(['"', '”', ')', '\'']);
        return tail.starts_with(['.', '!', '?'])
            && !tail.starts_with("..")
            && tail
                .trim_start_matches(['.', '!', '?', '"', '”', ')'])
                .starts_with(' ');
    }
    let marks = gap.trim_start_matches(closing);
    if !marks.starts_with(['.', '!', '?']) || marks.starts_with("..") {
        return false;
    }
    let period = marks.starts_with('.');
    if period
        && (previous.chars().nth(1).is_none()
            || previous.bytes().any(|b| b.is_ascii_digit())
            || previous.bytes().all(|b| b"IVXLCDM".contains(&b))
            || listed(previous, abbreviations, lang))
    {
        return false;
    }
    let after = marks.trim_start_matches(['.', '!', '?']);
    let rest = after.trim_start_matches(closing);
    let dialogue = matches!(lang, Language::Italian | Language::Portuguese);
    if dialogue && !period && after.trim_start_matches('\u{1}').starts_with(closing) {
        return false;
    }
    if (lang == Language::Portuguese || (lang == Language::Italian && !period))
        && rest.contains('—')
    {
        return false;
    }
    rest.starts_with(char::is_whitespace)
        && rest
            .trim_start()
            .chars()
            .all(|c| opening(lang, c) || (lang == Language::Italian && c.is_whitespace()))
}

pub fn capitalize(word: &str, lang: Language) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    if lang == Language::Turkish {
        let initial = match first {
            'i' => 'İ',
            'ı' => 'I',
            c => c.to_uppercase().next().unwrap_or(c),
        };
        return std::iter::once(initial).chain(chars).collect();
    }
    first.to_uppercase().chain(chars).collect()
}

/// Check the first word and the gaps between the caller's ordered token ranges.
/// Abbreviations and tokenization come from the document's language profile.
pub fn sentence_case(
    text: &str,
    words: &[(usize, usize)],
    lang: Language,
    capital_first: bool,
    abbreviations: &[&str],
    mut emit: impl FnMut(Range<usize>, String),
) {
    if matches!(
        lang,
        Language::Czech | Language::Indonesian | Language::Spanish
    ) {
        return;
    }
    let first_unconditional = matches!(
        lang,
        Language::English | Language::Italian | Language::Turkish
    );
    let mut previous = None;
    for &(start, end) in words {
        let Some(word) = text.get(start..end) else {
            continue;
        };
        let starts = match previous {
            Some((last_end, prev)) => text
                .get(last_end..start)
                .is_some_and(|gap| boundary(lang, prev, gap, abbreviations)),
            None => {
                capital_first
                    && (first_unconditional
                        || (!text.trim_end().ends_with([',', ';'])
                            && text[..start]
                                .chars()
                                .all(|c| c.is_whitespace() || opening(lang, c))))
            }
        };
        if starts
            && word.starts_with(char::is_lowercase)
            && (lang == Language::Turkish || word.chars().all(char::is_alphabetic))
        {
            emit(start..end, capitalize(word, lang));
        }
        previous = Some((end, word));
    }
}
