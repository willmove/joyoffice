use gridcraft_engine::{
    core::RangeRef,
    formula::lexer::{self, Tok},
};
use std::ops::Range;
pub const COLORS: [u32; 6] = [0x2274bf, 0xbd3b63, 0x15875a, 0x9b6bb8, 0xc77a18, 0x168b97];
#[derive(Clone, Debug)]
pub struct Reference {
    pub sheet: Option<String>,
    pub range: RangeRef,
    pub label: String,
    pub color: usize,
}
pub fn references(text: &str) -> Vec<Reference> {
    let Some(text) = text.strip_prefix('=') else { return Vec::new() };
    let tokens = match lexer::tokenize(text) {
        Ok(t) => t,
        Err(e) => lexer::tokenize(text.get(..e.pos).unwrap_or("")).unwrap_or_default(),
    };
    let mut result: Vec<Reference> = Vec::new();
    let mut i = 0;
    let mut sheet = None;
    while i < tokens.len() && result.len() < 64 {
        match &tokens[i].tok {
            Tok::Sheet(name, None) => sheet = Some(name.clone()),
            Tok::Space => {}
            Tok::Word(word) => {
                let mut value = word.clone();
                if i + 2 < tokens.len()
                    && matches!(tokens[i + 1].tok, Tok::Colon)
                    && let Tok::Word(end) = &tokens[i + 2].tok
                {
                    value.push(':');
                    value.push_str(end);
                    i += 2;
                }
                if let Some(range) = RangeRef::parse(&value.replace('$', "")) {
                    let color = result.iter().find(|r| r.range == range && r.sheet == sheet).map(|r| r.color).unwrap_or(result.len() % COLORS.len());
                    let label = if let Some(s) = &sheet { format!("{s}!{value}") } else { value };
                    result.push(Reference { sheet: sheet.take(), range, label, color });
                } else {
                    sheet = None;
                }
            }
            _ => sheet = None,
        }
        i += 1;
    }
    result
}
pub struct Hint {
    pub signature: String,
    pub description: String,
    pub active: Range<usize>,
}
pub fn context(text: &str, caret: usize) -> Option<(String, usize)> {
    let prefix = text.get(..caret)?;
    let prefix = prefix.strip_prefix('=')?;
    let tokens = lexer::tokenize(prefix).ok()?;
    let mut stack: Vec<Option<(String, usize)>> = Vec::new();
    for token in tokens {
        match token.tok {
            Tok::Func(name) => stack.push(Some((name, 0))),
            Tok::LParen | Tok::LBrace => stack.push(None),
            Tok::RParen | Tok::RBrace => {
                stack.pop();
            }
            Tok::Comma | Tok::Semicolon => {
                if let Some(Some((_, arg))) = stack.last_mut() {
                    *arg += 1;
                }
            }
            _ => {}
        }
    }
    stack.into_iter().rev().flatten().next()
}
pub fn argument_span(signature: &str, index: usize) -> Range<usize> {
    let Some(start) = signature.find('(').map(|i| i + 1) else { return 0..0 };
    let end = signature.rfind(')').unwrap_or(signature.len());
    let mut offset = start;
    let mut spans = Vec::new();
    for part in signature.get(start..end).unwrap_or("").split(',') {
        spans.push(offset..offset + part.len());
        offset += part.len() + 1;
    }
    spans.get(index.min(spans.len().saturating_sub(1))).cloned().unwrap_or(0..0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caret_nested_calls_and_quoted_references() {
        let text = "=SUM(A1,IF(B2,3,4),\"A99\",'数据'!$C$3:$D$4)";
        let caret = text.find(",\"A99\"").unwrap();
        assert_eq!(context(text, caret), Some(("SUM".into(), 1)));
        let refs = references(text);
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[2].sheet.as_deref(), Some("数据"));
        assert_eq!(refs[2].range, RangeRef::parse("C3:D4").unwrap());
        let span = argument_span("IF(test,yes,no)", 1);
        assert_eq!(&"IF(test,yes,no)"[span], "yes");
        assert!(references("=SUM(\"A1\",42)").is_empty());
    }
}
