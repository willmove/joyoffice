//! Conservative local decisions based on the original paragraph, with licensed
//! vocabulary supplied by the locale adapter.
use std::ops::Range;

pub enum ClauseVocabulary<'a> {
    Polish {
        openers: &'a [&'a str],
        exceptions: &'a [&'a str],
        prepositions: &'a [&'a str],
    },
    Russian {
        openers: &'a [&'a str],
        exceptions: &'a [&'a str],
        prepositions: &'a [&'a str],
    },
}

fn space(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| matches!(c, ' ' | '\u{a0}'))
}

pub fn subordinate_commas<'a>(
    text: &'a str,
    words: &[(usize, usize)],
    vocabulary: ClauseVocabulary<'_>,
    mut emit: impl FnMut(Range<usize>, &'a str),
) {
    for (index, pair) in words.windows(2).enumerate() {
        let ((left, end), (start, right)) = (pair[0], pair[1]);
        let (Some(previous), Some(gap), Some(word)) = (
            text.get(left..end),
            text.get(end..start),
            text.get(start..right),
        ) else {
            continue;
        };
        if !space(gap) || !previous.chars().all(char::is_alphabetic) {
            continue;
        }
        let qualifies = match &vocabulary {
            ClauseVocabulary::Polish {
                openers,
                exceptions,
                prepositions,
            } => {
                if !openers.contains(&word) {
                    continue;
                }
                let prev = previous.to_lowercase();
                !exceptions.contains(&prev.as_str())
                    && !(word.starts_with("któr")
                        && (prev == "to" || prepositions.contains(&prev.as_str())))
            }
            ClauseVocabulary::Russian {
                openers,
                exceptions,
                prepositions,
            } => {
                let folded = word.to_lowercase();
                let compound = matches!(folded.as_str(), "потому" | "оттого");
                // “Когда” also has non-clausal uses. Limit indirect questions
                // to reporting verbs followed by another word in the same clause.
                let indirect_when = folded == "когда"
                    && matches!(
                        previous.to_lowercase().as_str(),
                        "знаю"
                            | "знает"
                            | "знаем"
                            | "знали"
                            | "помню"
                            | "помнит"
                            | "скажи"
                            | "скажите"
                    )
                    && words
                        .get(index + 2)
                        .is_some_and(|&(a, _)| text.get(right..a).is_some_and(space));
                if !compound && !indirect_when && !openers.contains(&folded.as_str()) {
                    continue;
                }
                if compound || folded == "хотя" {
                    let following = words.get(index + 2).and_then(|&(a, b)| {
                        space(text.get(right..a)?)
                            .then(|| text.get(a..b))?
                            .map(str::to_lowercase)
                    });
                    if (compound && following.as_deref() != Some("что"))
                        || (folded == "хотя" && following.as_deref() == Some("бы"))
                    {
                        continue;
                    }
                }
                let prev = previous.to_lowercase();
                !exceptions.contains(&prev.as_str()) && !prepositions.contains(&prev.as_str())
            }
        };
        if qualifies {
            emit(end..right, word);
        }
    }
}

pub fn polish_preposition(article: &str, next: &str) -> Option<(&'static str, &'static str)> {
    if !matches!(article, "w" | "W" | "z" | "Z") {
        return None;
    }
    let lower = next.to_lowercase();
    let mut chars = lower.chars();
    let first = chars.next()?;
    let second = chars.next();
    let consonant = |c: char| c.is_alphabetic() && !"aąeęioóuyéáöüíú".contains(c);
    match article {
        "w" | "W" if lower == "mnie" || (first == 'w' && second.is_some_and(consonant)) => {
            Some(("we", if article == "w" { "we" } else { "We" }))
        }
        "z" | "Z" => {
            let following = if first == 's' && second == Some('z') {
                chars.next()
            } else {
                second
            };
            let wanted =
                lower == "mną" || ("szśźż".contains(first) && following.is_some_and(consonant));
            wanted.then_some(("ze", if article == "z" { "ze" } else { "Ze" }))
        }
        _ => None,
    }
}

/// Only a comma in the same horizontal gap joins two adjacent Turkish words.
/// A quote, bracket or line end can belong to an independent clause.
pub fn conjunction_comma(gap: &str) -> Option<usize> {
    let trimmed = gap.trim_matches([' ', '\t', '\u{a0}']);
    (trimmed == ",").then(|| gap.len() - gap.trim_start_matches([' ', '\t', '\u{a0}']).len())
}
