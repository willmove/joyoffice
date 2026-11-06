//! UTF-16/grapheme and composition adapter adapted from JoyOffice joydeck-model/text.rs.
use joydeck_model::{
    Paragraph, TextBody, edit as ed,
    text::{Run, RunKind},
};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[path = "text_links.rs"]
pub mod links;

fn runs(body: &TextBody) -> impl Iterator<Item = (Range<usize>, &Run)> {
    let mut offset = 0;
    body.paragraphs.iter().flat_map(move |paragraph| {
        let mut start = offset;
        offset += paragraph
            .runs
            .iter()
            .map(|r| r.content().len())
            .sum::<usize>()
            + 1;
        paragraph.runs.iter().map(move |run| {
            let end = start + run.content().len();
            let range = start..end;
            start = end;
            (range, run)
        })
    })
}

/// Expand a selection to whole fields or move an interior caret to its closest edge.
pub fn field_range(body: &TextBody, range: Range<usize>) -> Range<usize> {
    let (mut start, mut end) = (range.start, range.end);
    for (field, _) in runs(body).filter(|(_, run)| matches!(run.kind, RunKind::Field { .. })) {
        if range.is_empty() {
            if field.start < start && start < field.end {
                let at = if start - field.start < field.end - start {
                    field.start
                } else {
                    field.end
                };
                return at..at;
            }
        } else if field.start < range.end && range.start < field.end {
            start = start.min(field.start);
            end = end.max(field.end);
        }
    }
    start..end
}
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

/// First grapheme boundary at or after this byte offset.
pub fn ceil_grapheme(text: &str, at: usize) -> usize {
    if snap(text, at) == at {
        at
    } else {
        next(text, at)
    }
}

fn normalize_newlines(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
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
    let range = field_range(body, range);
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
#[derive(Clone)]
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
        let at = self.editable_range(&text, at..at).start;
        let anchor = if self.reversed {
            self.selection.end
        } else {
            self.selection.start
        };
        let range = if extend {
            anchor.min(at)..anchor.max(at)
        } else {
            at..at
        };
        self.selection = field_range(&self.body, range);
        self.reversed = extend && at < anchor;
    }

    /// Change the native selection without bisecting a committed grapheme.
    pub fn select_utf16(&mut self, range: Range<usize>) -> Result<(), String> {
        let value = self.body.text();
        let range = utf16_range(&value, range).ok_or("invalid UTF-16 selection range")?;
        let selection = self.editable_range(&value, range);
        if self.selection != selection {
            self.typing_props = None;
        }
        self.selection = selection;
        self.reversed = false;
        Ok(())
    }

    /// Translate an AppKit setMarkedText range to document UTF-16 coordinates.
    pub fn marked_relative_range(&self, range: Range<usize>) -> Option<Range<usize>> {
        let start = self.marked.as_ref().map_or(0, |marked| {
            let value = self.body.text();
            to_utf16(&value, marked.start..marked.start).start
        });
        Some(start.checked_add(range.start)?..start.checked_add(range.end)?)
    }

    pub fn editable_range(&self, value: &str, range: Range<usize>) -> Range<usize> {
        let in_mark = |at| {
            self.marked
                .as_ref()
                .is_some_and(|r| r.start <= at && at <= r.end)
        };
        let start = if in_mark(range.start) {
            range.start
        } else {
            snap(value, range.start)
        };
        let end = if range.is_empty() {
            start
        } else if in_mark(range.end) {
            range.end
        } else {
            ceil_grapheme(value, range.end)
        };
        field_range(&self.body, start..end)
    }

    pub fn replace(
        &mut self,
        range: Option<Range<usize>>,
        value: &str,
        composing: bool,
        selection: Option<Range<usize>>,
    ) -> Result<(), String> {
        if composing && value.is_empty() {
            self.cancel();
            return Ok(());
        }
        let text = self.body.text();
        let range = match range {
            Some(r) => self.editable_range(
                &text,
                utf16_range(&text, r).ok_or("invalid UTF-16 replacement range")?,
            ),
            None => field_range(
                &self.body,
                self.marked.clone().unwrap_or(self.selection.clone()),
            ),
        };
        let selected = selection
            .map(|r| utf16_range(value, r).ok_or("invalid preedit selection"))
            .transpose()?
            .map(|r| {
                normalize_newlines(&value[..r.start]).len()
                    ..normalize_newlines(&value[..r.end]).len()
            });
        let value = normalize_newlines(value);
        let mut body = self.body.clone();
        if text.get(range.clone()) != Some(value.as_str()) {
            replace(&mut body, range.clone(), &value)?;
        }
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
            self.settle();
        }
        Ok(())
    }
    /// Replace the current selection with one dynamic field and its fallback text.
    pub fn insert_field(
        &mut self,
        kind: &str,
        value: &str,
        lang: Option<&str>,
    ) -> Result<(), String> {
        if self.marked.is_some()
            || value.is_empty()
            || value.len() > 256
            || value.chars().any(char::is_control)
        {
            return Err("invalid field text or active composition".into());
        }
        let date = kind
            .strip_prefix("datetime")
            .and_then(|n| n.parse::<u8>().ok());
        if kind != "slidenum"
            && !date.is_some_and(|n| (1..=13).contains(&n) && kind == format!("datetime{n}"))
        {
            return Err("unsupported new text field".into());
        }
        let value_before = self.body.text();
        let range = self.editable_range(&value_before, self.selection.clone());
        let start = pos(&value_before, range.start);
        let end = pos(&value_before, range.end);
        let mut props = self
            .typing_props
            .clone()
            .unwrap_or_else(|| ed::props_at(&self.body, start));
        if let Some(lang) = lang {
            props.lang = Some(lang.into());
        }
        props.link = None;
        props.hover = None;
        let mut next = self.body.clone();
        ed::delete(&mut next, start, end);
        let field = TextBody {
            paragraphs: vec![Paragraph {
                runs: vec![Run {
                    text: value.into(),
                    props,
                    kind: RunKind::Field { field: kind.into() },
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let end = ed::insert_body(&mut next, start, &field);
        validate(&next)?;
        let at = byte_pos(&next.text(), end);
        self.body = next;
        self.selection = at..at;
        self.reversed = false;
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
        let value = self.body.text();
        self.selection = self.editable_range(&value, self.selection.clone());
    }
}
