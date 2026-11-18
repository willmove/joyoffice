//! Pair question/exclamation marks while retaining the current clause and address
//! boundary. Each Unicode scalar and each whitespace-delimited token is visited once.
use std::ops::Range;

pub fn spanish_opening_marks(
    text: &str,
    is_address: impl Fn(&str) -> bool,
    mut emit: impl FnMut(Range<usize>, char),
) {
    let mut tokens = text.split_whitespace();
    let mut token_end = 0;
    let mut address = false;
    let mut depth = 0usize;
    let mut clause = 0;
    let mut has_letter = false;
    let mut previous = None;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let end = i + c.len_utf8();
        if i >= token_end {
            address = false;
            if !c.is_whitespace() {
                let Some(token) = tokens.next() else { break };
                token_end = i + token.len();
                address = is_address(token);
            }
        }
        let next = chars.peek().map(|&(_, ch)| ch);
        let mut boundary = false;
        if address {
            boundary = end == token_end && matches!(c, '.' | '!' | '?');
        } else {
            match c {
                '¿' | '¡' => {
                    if !matches!(previous, Some('¿' | '¡')) {
                        depth += 1;
                    }
                    boundary = true;
                }
                '?' | '!' => {
                    let paired_run = matches!(previous, Some('?' | '!'));
                    let isolated = previous == Some('(') && next == Some(')');
                    if !paired_run && !isolated && !next.is_some_and(char::is_alphanumeric) {
                        if depth != 0 {
                            depth -= 1;
                        } else if has_letter {
                            let phrase = &text[clause..end];
                            let trimmed = phrase.trim_start();
                            emit(end - trimmed.len()..end, if c == '?' { '¿' } else { '¡' });
                        }
                    }
                    boundary = true;
                }
                '.' | ':' | ';' => boundary = !next.is_some_and(char::is_alphanumeric),
                '…' | '\n' | '\r' | '\t' | '«' | '“' | '"' | '(' | '[' | '—' => {
                    boundary = true
                }
                _ => {}
            }
        }
        if boundary {
            clause = end;
            has_letter = false;
        } else {
            has_letter |= c.is_alphabetic();
        }
        previous = Some(c);
    }
}
