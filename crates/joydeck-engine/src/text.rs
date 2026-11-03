//! UTF-16/grapheme and composition adapter adapted from JoyOffice joydeck-model/text.rs.
use joydeck_model::{TextBody, edit as ed};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;
pub fn byte_at_utf16(text: &str, offset: usize) -> Option<usize> {
    let mut n = 0;
    for (i, c) in text.char_indices() {
        if n == offset {
            return Some(i);
        }
        n += c.len_utf16();
        if n > offset {
            return None;
        }
    }
    (n == offset).then_some(text.len())
}
pub fn utf16_range(text: &str, range: Range<usize>) -> Option<Range<usize>> {
    if range.start > range.end {
        return None;
    }
    Some(byte_at_utf16(text, range.start)?..byte_at_utf16(text, range.end)?)
}
pub fn to_utf16(text: &str, range: Range<usize>) -> Range<usize> {
    text[..range.start].encode_utf16().count()..text[..range.end].encode_utf16().count()
}
pub fn previous(text: &str, at: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(i, _)| i)
        .take_while(|i| *i < at)
        .last()
        .unwrap_or(0)
}
pub fn next(text: &str, at: usize) -> usize {
    text.grapheme_indices(true)
        .find(|(i, _)| *i > at)
        .map_or(text.len(), |(i, _)| i)
}
pub fn snap(text: &str, at: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(i, _)| i)
        .chain([text.len()])
        .take_while(|i| *i <= at)
        .last()
        .unwrap_or(0)
}

pub fn validate(body: &TextBody) -> Result<(), String> {
    if body.text().len() > 65536
        || body.paragraphs.len() > 4096
        || body.paragraphs.iter().map(|p| p.runs.len()).sum::<usize>() > 8192
    {
        return Err("text body exceeds editing budget".into());
    }
    for p in &body.paragraphs {
        for r in std::iter::once(&p.def_props)
            .chain(std::iter::once(&p.end_props))
            .chain(p.runs.iter().map(|r| &r.props))
        {
            if r.size
                .is_some_and(|v| !v.is_finite() || !(1. ..=4000.).contains(&v))
            {
                return Err("invalid font size".into());
            }
        }
    }
    Ok(())
}
pub fn pos(text: &str, byte: usize) -> ed::Pos {
    let prefix = &text[..byte];
    (
        prefix.bytes().filter(|c| *c == b'\n').count(),
        prefix.rsplit('\n').next().unwrap_or("").chars().count(),
    )
}
pub fn byte_pos(text: &str, pos: ed::Pos) -> usize {
    let mut byte = 0;
    for (i, part) in text.split('\n').enumerate() {
        if i == pos.0 {
            return byte
                + part
                    .char_indices()
                    .nth(pos.1)
                    .map_or(part.len(), |(i, _)| i);
        }
        byte += part.len() + 1;
    }
    text.len()
}
pub fn replace(body: &mut TextBody, range: Range<usize>, value: &str) -> Result<(), String> {
    let text = body.text();
    if range.start > range.end
        || range.end > text.len()
        || !text.is_char_boundary(range.start)
        || !text.is_char_boundary(range.end)
    {
        return Err("invalid UTF-8 replacement range".into());
    }
    if text
        .len()
        .saturating_sub(range.len())
        .saturating_add(value.len())
        > 65536
    {
        return Err("text exceeds 64 KiB".into());
    }
    let a = pos(&text, range.start);
    let b = pos(&text, range.end);
    let style_at = if range.is_empty() {
        a
    } else {
        pos(
            &text,
            range.start + text[range.start..].chars().next().map_or(0, char::len_utf8),
        )
    };
    let props = ed::props_at(body, style_at);
    ed::delete(body, a, b);
    ed::insert(body, a, value, Some(props));
    validate(body)
}
/// A composition draft owns the body and selection; only settle/commit may publish it.
pub struct Draft {
    pub body: TextBody,
    pub selection: Range<usize>,
    pub reversed: bool,
    pub marked: Option<Range<usize>>,
    pub typing_props: Option<joydeck_model::RunProps>,
    before: Option<(TextBody, Range<usize>, bool)>,
}
impl Draft {
    pub fn new(body: TextBody) -> Self {
        Self {
            selection: 0..body.text().len(),
            body,
            reversed: false,
            marked: None,
            typing_props: None,
            before: None,
        }
    }
    pub fn cursor(&self) -> usize {
        if self.reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }
    pub fn move_to(&mut self, at: usize, extend: bool) {
        self.typing_props = None;
        let text = self.body.text();
        let at = snap(&text, at);
        let anchor = if self.reversed {
            self.selection.end
        } else {
            self.selection.start
        };
        self.selection = if extend {
            anchor.min(at)..anchor.max(at)
        } else {
            at..at
        };
        self.reversed = extend && at < anchor;
    }
    pub fn replace(
        &mut self,
        range: Option<Range<usize>>,
        value: &str,
        composing: bool,
        selection: Option<Range<usize>>,
    ) -> Result<(), String> {
        let text = self.body.text();
        let range = match range {
            Some(r) => utf16_range(&text, r).ok_or("invalid UTF-16 replacement range")?,
            None => self.marked.clone().unwrap_or(self.selection.clone()),
        };
        if composing && value.is_empty() && self.before.is_some() {
            self.cancel();
            return Ok(());
        }
        let value = value.replace("\r\n", "\n").replace('\r', "\n");
        let selected = selection
            .map(|r| utf16_range(&value, r).ok_or("invalid preedit selection"))
            .transpose()?;
        let mut body = self.body.clone();
        replace(&mut body, range.clone(), &value)?;
        if let Some(props) = &self.typing_props
            && !value.is_empty()
        {
            let after = body.text();
            ed::format(
                &mut body,
                pos(&after, range.start),
                pos(&after, range.start + value.len()),
                &|p| *p = props.clone(),
            );
            validate(&body)?;
        }
        if composing && self.before.is_none() {
            self.before = Some((self.body.clone(), self.selection.clone(), self.reversed));
        }
        self.body = body;
        let end = range.start + value.len();
        self.selection = selected.map_or(end..end, |r| range.start + r.start..range.start + r.end);
        self.reversed = false;
        self.marked = composing.then_some(range.start..end);
        if !composing {
            self.before = None;
        }
        Ok(())
    }
    pub fn cancel(&mut self) {
        if let Some((body, selection, reversed)) = self.before.take() {
            self.body = body;
            self.selection = selection;
            self.reversed = reversed;
        }
        self.marked = None;
    }
    pub fn settle(&mut self) {
        self.before = None;
        self.marked = None;
    }
}
