//! Rich-text fragments and edits. Paragraph offsets count Unicode scalar values.

use std::ops::Range;

use crate::text::{Paragraph, Run, RunKind, RunProps, TextBody};

pub type Pos = (usize, usize);

pub(super) fn clamp(body: &TextBody, (paragraph, offset): Pos) -> Pos {
    let index = paragraph.min(body.paragraphs.len().saturating_sub(1));
    let length = body.paragraphs.get(index).map_or(0, Paragraph::char_len);
    (index, offset.min(length))
}

fn ordered(body: &TextBody, a: Pos, b: Pos) -> (Pos, Pos) {
    let (a, b) = (clamp(body, a), clamp(body, b));
    (a.min(b), a.max(b))
}

// Split a paragraph into two run sequences. Non-text runs are indivisible.
fn detach(runs: &mut Vec<Run>, mut offset: usize) -> Vec<Run> {
    for index in 0..runs.len() {
        if offset == 0 {
            return runs.split_off(index);
        }
        let run = &mut runs[index];
        let count = run.char_len();
        if offset < count {
            let head = run.split_text_at(offset);
            let mut tail = runs.split_off(index + 1);
            if let Some(piece) = head {
                tail.insert(0, piece);
            }
            return tail;
        }
        offset -= count;
    }
    Vec::new()
}

/// The caret takes its explicit style from the preceding run or the first run at paragraph start.
pub fn props_at(body: &TextBody, pos: Pos) -> RunProps {
    let (index, offset) = clamp(body, pos);
    let Some(paragraph) = body.paragraphs.get(index) else {
        return RunProps::default();
    };
    let mut remaining = offset;
    for run in &paragraph.runs {
        let count = run.char_len();
        if remaining <= count {
            return run.props.clone();
        }
        remaining -= count;
    }
    paragraph.end_props.clone()
}

fn style_only(paragraph: &Paragraph) -> Paragraph {
    Paragraph {
        level: paragraph.level,
        props: paragraph.props.clone(),
        def_props: paragraph.def_props.clone(),
        end_props: paragraph.end_props.clone(),
        runs: Vec::new(),
    }
}

fn typed_runs(line: &str, props: &RunProps) -> Vec<Run> {
    let mut runs = Vec::new();
    for (index, text) in line.split('\u{b}').enumerate() {
        if index != 0 {
            runs.push(Run {
                text: String::new(),
                props: props.clone(),
                kind: RunKind::Break,
            });
        }
        if !text.is_empty() {
            runs.push(Run::with(text, props.clone()));
        }
    }
    runs
}

/// Insert typed content using the paragraph's formatting and the requested caret style.
pub fn insert(body: &mut TextBody, pos: Pos, text: &str, props: Option<RunProps>) -> Pos {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let (index, offset) = clamp(body, pos);
    let props = props.unwrap_or_else(|| props_at(body, (index, offset)));
    let template = body
        .paragraphs
        .get(index)
        .map_or_else(Paragraph::default, style_only);
    let mut lines = text.split('\n').peekable();
    let mut fragment = Vec::new();
    while let Some(line) = lines.next() {
        let mut paragraph = style_only(&template);
        paragraph.runs = typed_runs(line, &props);
        if lines.peek().is_some() {
            paragraph.end_props = props.clone();
        }
        fragment.push(paragraph);
    }
    let caret = splice(body, (index, offset), fragment, false);
    if body.paragraphs[caret.0].runs.is_empty() {
        body.paragraphs[caret.0].end_props = props;
    }
    caret
}

/// Delete a scalar range or join its surrounding paragraphs, keeping the leading paragraph style.
pub fn delete(body: &mut TextBody, a: Pos, b: Pos) -> Pos {
    let (a, b) = ordered(body, a, b);
    if a == b {
        return a;
    }
    let old_style = props_at(body, a);
    let tail = detach(&mut body.paragraphs[b.0].runs, b.1);
    let first = &mut body.paragraphs[a.0];
    detach(&mut first.runs, a.1);
    first.runs.extend(tail);
    first.normalize();
    if a.0 != b.0 {
        body.paragraphs.drain(a.0 + 1..=b.0);
    } else if first.runs.is_empty() {
        first.end_props = old_style;
    }
    a
}

/// Format a paragraph range. A caret addresses the end mark, as does a selection ending there.
pub fn format_paragraph(p: &mut Paragraph, columns: Range<usize>, f: impl Fn(&mut RunProps)) {
    let mark = columns.is_empty() || columns.end == p.char_len();
    if !columns.is_empty() {
        let tail = detach(&mut p.runs, columns.end);
        let mut selected = detach(&mut p.runs, columns.start);
        for run in &mut selected {
            f(&mut run.props);
        }
        p.runs.extend(selected);
        p.runs.extend(tail);
        p.normalize();
    }
    if mark {
        f(&mut p.end_props);
    }
}

pub fn format(body: &mut TextBody, a: Pos, b: Pos, f: &dyn Fn(&mut RunProps)) {
    let (a, b) = ordered(body, a, b);
    for (index, paragraph) in body
        .paragraphs
        .iter_mut()
        .enumerate()
        .take(b.0 + 1)
        .skip(a.0)
    {
        let columns = columns(paragraph, index, a, b);
        if a == b || !columns.is_empty() || columns.end == paragraph.char_len() {
            format_paragraph(paragraph, columns, f);
        }
    }
}

pub fn format_all(body: &mut TextBody, f: &dyn Fn(&mut RunProps)) {
    for paragraph in &mut body.paragraphs {
        for run in &mut paragraph.runs {
            f(&mut run.props);
        }
        f(&mut paragraph.end_props);
        paragraph.normalize();
    }
}

fn columns(paragraph: &Paragraph, index: usize, a: Pos, b: Pos) -> Range<usize> {
    let from = if index == a.0 { a.1 } else { 0 };
    let to = if index == b.0 {
        b.1
    } else {
        paragraph.char_len()
    };
    from..to
}

pub fn text_range(body: &TextBody, a: Pos, b: Pos) -> String {
    let (a, b) = ordered(body, a, b);
    let mut text = String::new();
    for (index, paragraph) in body.paragraphs.iter().enumerate().take(b.0 + 1).skip(a.0) {
        if index != a.0 {
            text.push('\n');
        }
        let cols = columns(paragraph, index, a, b);
        text.extend(
            paragraph
                .runs
                .iter()
                .flat_map(|run| run.content().chars())
                .skip(cols.start)
                .take(cols.len()),
        );
    }
    text
}

/// Take a styled fragment, keeping each paragraph's explicit defaults and final mark.
pub fn slice(body: &TextBody, a: Pos, b: Pos) -> TextBody {
    let (a, b) = ordered(body, a, b);
    let paragraphs = body
        .paragraphs
        .iter()
        .enumerate()
        .take(b.0 + 1)
        .skip(a.0)
        .map(|(index, paragraph)| {
            let range = columns(paragraph, index, a, b);
            let mut fragment = paragraph.clone();
            detach(&mut fragment.runs, range.end);
            fragment.runs = detach(&mut fragment.runs, range.start);
            fragment
        })
        .collect();
    TextBody {
        body: body.body.clone(),
        list_style: body.list_style.clone(),
        paragraphs,
    }
}

/// Insert a rich fragment. Its first paragraph joins the destination, and later paragraphs keep
/// their own list level, paragraph defaults, and typing mark.
pub fn insert_body(body: &mut TextBody, pos: Pos, source: &TextBody) -> Pos {
    let at = clamp(body, pos);
    let mut fragment = Vec::with_capacity(source.paragraphs.len());
    for paragraph in &source.paragraphs {
        let mut part = style_only(paragraph);
        for run in &paragraph.runs {
            // Older documents may still carry literal line separators in a text run.
            if run.kind == RunKind::Text && run.text.contains(['\r', '\n', '\u{b}']) {
                let value = run.text.replace("\r\n", "\n").replace('\r', "\n");
                for (index, line) in value.split('\n').enumerate() {
                    if index > 0 {
                        part.end_props = run.props.clone();
                        fragment.push(part);
                        part = style_only(paragraph);
                    }
                    part.runs.extend(typed_runs(line, &run.props));
                }
            } else {
                part.runs.push(run.clone());
            }
        }
        fragment.push(part);
    }
    if let Some(first) = fragment.first_mut() {
        for run in &mut first.runs {
            run.props.inherit(&first.def_props);
        }
    }
    splice(body, at, fragment, true)
}

fn splice(body: &mut TextBody, (index, offset): Pos, fragment: Vec<Paragraph>, rich: bool) -> Pos {
    let mut pieces = fragment.into_iter();
    let Some(first) = pieces.next() else {
        return (index, offset);
    };
    if body.paragraphs.is_empty() {
        body.paragraphs.push(Paragraph::default());
    }
    let target = &mut body.paragraphs[index];
    let mut suffix = detach(&mut target.runs, offset);
    let first_end = target.char_len() + first.char_len();
    target.runs.extend(first.runs);
    let mut rest: Vec<Paragraph> = pieces.collect();
    let count = rest.len();
    let Some(last) = rest.last_mut() else {
        target.runs.extend(suffix);
        target.normalize();
        return (index, first_end);
    };
    if rich && !suffix.is_empty() {
        last.end_props.clone_from(&target.end_props);
        if last.def_props != target.def_props {
            for run in &mut suffix {
                run.props.inherit(&target.def_props);
            }
        }
    }
    let last_end = last.char_len();
    last.runs.extend(suffix);
    target.end_props = first.end_props;
    target.normalize();
    for paragraph in &mut rest {
        paragraph.normalize();
    }
    body.paragraphs.splice(index + 1..index + 1, rest);
    (index + count, last_end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{Align, Bullet};

    #[test]
    fn rich_paste_retains_paragraph_defaults_and_destination_tail_style() {
        let mut target = TextBody::from_text("LeftRight");
        target.paragraphs[0].def_props.size = Some(15.0);
        target.paragraphs[0].end_props.size = Some(19.0);
        let mut source = TextBody::from_text("one\ntwo");
        source.paragraphs[0].def_props.bold = Some(true);
        source.paragraphs[1].level = 2;
        source.paragraphs[1].props.align = Some(Align::Right);
        source.paragraphs[1].props.bullet = Some(Bullet::AutoNum {
            scheme: "romanUcPeriod".into(),
            start_at: 3,
        });
        source.paragraphs[1].def_props.size = Some(26.0);
        assert_eq!(insert_body(&mut target, (0, 4), &source), (1, 3));
        assert_eq!(target.text(), "Leftone\ntwoRight");
        assert_eq!(target.paragraphs[0].runs[1].props.bold, Some(true));
        let numbered = &target.paragraphs[1];
        assert_eq!(numbered.level, 2);
        assert_eq!(numbered.props, source.paragraphs[1].props);
        assert_eq!(numbered.def_props.size, Some(26.0));
        assert_eq!(numbered.runs[1].props.size, Some(15.0));
        assert_eq!(numbered.end_props.size, Some(19.0));
    }
}
