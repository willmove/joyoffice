use std::ops::Range;

pub struct ItalianForms<'a> {
    pub articles: &'a [&'a str],
    pub feminine_o: &'a [&'a str],
    pub masculine_endings: &'a [&'a str],
    pub ambiguous_a: &'a [&'a str],
}

#[derive(Clone, Copy)]
pub enum ItalianIssue<'a> {
    Truncation,
    Qual,
    Accent,
    Masculine(&'a str),
    Feminine(&'a str),
}

pub fn italian_qual(word: &str) -> Option<String> {
    let (head, verb) = word.split_once(['\'', '’'])?;
    (head.eq_ignore_ascii_case("qual") && matches!(verb.to_lowercase().as_str(), "è" | "era"))
        .then(|| format!("{head} {verb}"))
}

fn vowel_noun(word: &str) -> bool {
    word.starts_with(|c| "aeiouhàèéìòóù".contains(c))
        && word.chars().all(|c| c.is_alphabetic() && c.is_lowercase())
}

pub fn italian_elisions<'a>(
    text: &'a str,
    spans: &[(usize, usize)],
    forms: ItalianForms<'_>,
    mut emit: impl FnMut(Range<usize>, ItalianIssue<'a>, String),
) {
    for (idx, &(begin, end)) in spans.iter().enumerate() {
        let Some(word) = text.get(begin..end) else {
            continue;
        };
        let after = &text[end..];
        let apostrophe = after.chars().next().filter(|c| matches!(c, '\'' | '’'));
        let range = begin..end;
        if matches!(word, "pò" | "Pò") || (word == "po" && apostrophe.is_none()) {
            emit(
                range,
                ItalianIssue::Truncation,
                if word == "Pò" { "Po'" } else { "po'" }.to_owned(),
            );
        } else if word == "E"
            && let Some(mark) = apostrophe
        {
            let next = &after[mark.len_utf8()..];
            if (next.is_empty() || next.starts_with(char::is_whitespace))
                && !text[..begin].ends_with(['\'', '‘', '’'])
            {
                emit(
                    begin..end + mark.len_utf8(),
                    ItalianIssue::Accent,
                    "È".into(),
                );
            }
        } else if let Some((head, noun)) = word.split_once(['\'', '’']) {
            if let Some(fix) = italian_qual(word) {
                emit(range, ItalianIssue::Qual, fix);
            } else if forms.articles.iter().any(|a| head.eq_ignore_ascii_case(a))
                && vowel_noun(noun)
                && noun.ends_with('o')
                && !forms.feminine_o.contains(&noun)
            {
                emit(
                    range,
                    ItalianIssue::Masculine(head),
                    format!("{head} {noun}"),
                );
            }
        } else if let Some(&(start, stop)) = spans.get(idx + 1)
            && text.get(end..start) == Some(" ")
            && forms.articles.iter().any(|a| word.eq_ignore_ascii_case(a))
            && let Some(noun) = text.get(start..stop)
            && noun.ends_with('a')
            && noun.chars().nth(1).is_some()
            && vowel_noun(noun)
            && !forms.ambiguous_a.contains(&noun)
            && !forms
                .masculine_endings
                .iter()
                .any(|suffix| noun.ends_with(suffix))
        {
            emit(
                begin..stop,
                ItalianIssue::Feminine(word),
                format!("{word}'{noun}"),
            );
        }
    }
}
