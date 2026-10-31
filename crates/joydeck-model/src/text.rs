//! Document text uses UTF-8 byte ranges; platform ranges are checked UTF-16.
use crate::document::{Object, Result};
use serde::{Deserialize, Serialize};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Style {
    pub family: String,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub color: String,
}
impl Style {
    pub fn for_object(o: &Object) -> Self {
        Self {
            family: "sans-serif".into(),
            size: o.font_size,
            bold: false,
            italic: false,
            color: o.fill.clone(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub start: usize,
    pub end: usize,
    pub style: Style,
}
pub fn validate(text: &str, runs: &[Run]) -> Result<()> {
    if runs.len() > 32768 {
        return Err("文字样式数量超限".into());
    }
    let mut end = 0;
    for run in runs {
        if run.start != end
            || run.end <= run.start
            || run.end > text.len()
            || !text.is_char_boundary(run.start)
            || !text.is_char_boundary(run.end)
            || !run.style.size.is_finite()
            || !(1. ..=512.).contains(&run.style.size)
            || run.style.family.is_empty()
            || run.style.family.len() > 256
            || !crate::document::color(&run.style.color)
        {
            return Err("富文本范围或样式无效".into());
        }
        end = run.end;
    }
    if !runs.is_empty() && end != text.len() {
        return Err("文字样式未覆盖全文".into());
    }
    Ok(())
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
pub fn runs(o: &Object) -> Vec<Run> {
    if o.runs.is_empty() && !o.text.is_empty() {
        vec![Run {
            start: 0,
            end: o.text.len(),
            style: Style::for_object(o),
        }]
    } else {
        o.runs.clone()
    }
}
fn append(out: &mut Vec<Run>, len: usize, style: Style) {
    if len == 0 {
        return;
    }
    let start = out.last().map_or(0, |r| r.end);
    if let Some(last) = out.last_mut()
        && last.style == style
    {
        last.end += len;
        return;
    }
    out.push(Run {
        start,
        end: start + len,
        style,
    });
}
pub fn replace(o: &mut Object, range: Range<usize>, value: &str, style: &Style) -> Result<()> {
    if range.start > range.end
        || range.end > o.text.len()
        || !o.text.is_char_boundary(range.start)
        || !o.text.is_char_boundary(range.end)
    {
        return Err("非法文字替换范围".into());
    }
    let original = runs(o);
    let mut out = vec![];
    for r in &original {
        append(
            &mut out,
            r.end.min(range.start).saturating_sub(r.start),
            r.style.clone(),
        );
    }
    append(&mut out, value.len(), style.clone());
    for r in &original {
        append(
            &mut out,
            r.end.saturating_sub(r.start.max(range.end)),
            r.style.clone(),
        );
    }
    let mut text = o.text.clone();
    text.replace_range(range, value);
    validate(&text, &out)?;
    if text.len() > 1024 * 1024 {
        return Err("单个文本框超过 1 MiB".into());
    }
    o.text = text;
    o.runs = out;
    Ok(())
}
pub fn format(o: &mut Object, range: Range<usize>, style: Style) -> Result<()> {
    if range.start > range.end
        || range.end > o.text.len()
        || !o.text.is_char_boundary(range.start)
        || !o.text.is_char_boundary(range.end)
    {
        return Err("非法格式范围".into());
    }
    let mut out = vec![];
    for r in runs(o) {
        append(
            &mut out,
            r.end.min(range.start).saturating_sub(r.start),
            r.style.clone(),
        );
        append(
            &mut out,
            r.end
                .min(range.end)
                .saturating_sub(r.start.max(range.start)),
            style.clone(),
        );
        append(
            &mut out,
            r.end.saturating_sub(r.start.max(range.end)),
            r.style,
        );
    }
    validate(&o.text, &out)?;
    o.runs = out;
    Ok(())
}
/// Composition is a draft. Only commit publishes an Object to command history.
pub struct TextEdit {
    pub object: Object,
    pub selection: Range<usize>,
    pub reversed: bool,
    pub marked: Option<Range<usize>>,
    pub typing: Style,
    before_composition: Option<(Object, Range<usize>)>,
}
impl TextEdit {
    pub fn new(object: Object) -> Self {
        let typing = Style::for_object(&object);
        Self {
            selection: 0..object.text.len(),
            reversed: false,
            object,
            marked: None,
            typing,
            before_composition: None,
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
        let at = snap(&self.object.text, at);
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
        if let Some(r) = runs(&self.object)
            .iter()
            .find(|r| r.start < at && r.end >= at)
            .or_else(|| self.object.runs.first())
        {
            self.typing = r.style.clone();
        }
    }
    pub fn replace(
        &mut self,
        range: Option<Range<usize>>,
        value: &str,
        composing: bool,
        selection: Option<Range<usize>>,
    ) -> Result<()> {
        let range = match range {
            Some(r) => utf16_range(&self.object.text, r).ok_or("非法 UTF-16 替换范围")?,
            None => self.marked.clone().unwrap_or(self.selection.clone()),
        };
        if composing && value.is_empty() && self.before_composition.is_some() {
            self.cancel();
            return Ok(());
        }
        let selected = selection
            .map(|r| utf16_range(value, r).ok_or("非法预编辑选区"))
            .transpose()?;
        let mut draft = self.object.clone();
        replace(&mut draft, range.clone(), value, &self.typing)?;
        if composing && self.before_composition.is_none() {
            self.before_composition = Some((self.object.clone(), self.selection.clone()));
        }
        self.object = draft;
        let end = range.start + value.len();
        self.selection = selected.map_or(end..end, |r| range.start + r.start..range.start + r.end);
        self.reversed = false;
        self.marked = composing.then_some(range.start..end);
        if !composing {
            self.before_composition = None;
        }
        Ok(())
    }
    pub fn cancel(&mut self) {
        if let Some((o, s)) = self.before_composition.take() {
            self.object = o;
            self.selection = s;
        }
        self.marked = None;
    }
    pub fn settle(&mut self) {
        self.marked = None;
        self.before_composition = None;
    }
}
