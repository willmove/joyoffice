//! Formatting helpers and effective style summaries for half-open text selections.
use crate::text;
use joydeck_model::{Paragraph, RunProps, TextBody, text::ParaProps};
use std::{collections::BTreeSet, ops::Range};

#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub run: RunProps,
    pub para: ParaProps,
    pub mixed: BTreeSet<&'static str>,
    runs: usize,
    paragraphs: usize,
}
impl Summary {
    pub fn is_empty(&self) -> bool {
        self.runs == 0
    }
    pub fn add(&mut self, other: Self) {
        if other.is_empty() {
            return;
        }
        self.add_run(other.run);
        self.add_para(other.para);
        self.mixed.extend(other.mixed);
    }
    fn add_run(&mut self, value: RunProps) {
        if self.runs == 0 {
            self.run = value;
        } else {
            macro_rules! common {
                ($($field:ident),*) => {$({
                    if self.run.$field != value.$field {
                        self.run.$field = None;
                        self.mixed.insert(stringify!($field));
                    }
                })*};
            }
            common!(
                font, font_ea, size, bold, italic, underline, fill, strike, baseline, spacing
            );
        }
        self.runs += 1;
    }
    fn add_para(&mut self, value: ParaProps) {
        if self.paragraphs == 0 {
            self.para = value;
        } else {
            macro_rules! common {
                ($($field:ident),*) => {$({
                    if self.para.$field != value.$field {
                        self.para.$field = None;
                        self.mixed.insert(stringify!($field));
                    }
                })*};
            }
            common!(
                align,
                margin_left,
                margin_right,
                indent,
                line_spacing,
                space_before,
                space_after,
                bullet,
                rtl
            );
        }
        self.paragraphs += 1;
    }
}

/// Paragraphs touched by a half-open selection. An end at the next paragraph's
/// start does not format that paragraph. A caret always addresses one paragraph.
pub fn paragraphs(body: &TextBody, range: Range<usize>) -> Result<Range<usize>, String> {
    let value = body.text();
    if range.start > range.end
        || range.end > value.len()
        || !value.is_char_boundary(range.start)
        || !value.is_char_boundary(range.end)
    {
        return Err("invalid text selection".into());
    }
    let a = text::pos(&value, range.start);
    let b = text::pos(&value, range.end);
    let end = if !range.is_empty() && b.1 == 0 && b.0 > a.0 {
        b.0
    } else {
        b.0 + 1
    };
    Ok(a.0..end.min(body.paragraphs.len()))
}

/// Change selected explicit runs using their individual inherited style. This
/// keeps relative operations (for example grow/shrink) relative in mixed text.
pub fn format_resolved(
    body: &mut TextBody,
    range: Option<Range<usize>>,
    resolve: impl Fn(usize, &Paragraph, &RunProps) -> RunProps,
    change: impl Fn(&mut RunProps, &RunProps),
) -> Result<(), String> {
    if body.paragraphs.is_empty() {
        body.paragraphs.push(Paragraph::default());
    }
    let value = body.text();
    let range = range.unwrap_or(0..value.len());
    let touched = paragraphs(body, range.clone())?;
    let a = text::pos(&value, range.start);
    let b = text::pos(&value, range.end);
    for pi in touched {
        let original = body.paragraphs[pi].clone();
        let from = if pi == a.0 { a.1 } else { 0 };
        let to = if pi == b.0 { b.1 } else { original.char_len() };
        let mut one = TextBody {
            paragraphs: vec![original.clone()],
            ..Default::default()
        };
        joydeck_model::edit::format(&mut one, (0, from), (0, to), &|r| {
            let effective = resolve(pi, &original, r);
            change(r, &effective);
        });
        if let Some(p) = one.paragraphs.into_iter().next() {
            body.paragraphs[pi] = p;
        }
    }
    Ok(())
}

/// The resolver closures use the same inherited paragraph and run properties as
/// the renderer. `None` summarizes the whole body; a caret uses its typing style.
pub fn summarize(
    body: &TextBody,
    range: Option<Range<usize>>,
    resolve_run: impl Fn(&Paragraph, &RunProps) -> RunProps,
    resolve_para: impl Fn(&Paragraph) -> ParaProps,
) -> Result<Summary, String> {
    let value = body.text();
    let range = range.unwrap_or(0..value.len());
    let touched = paragraphs(body, range.clone())?;
    let a = text::pos(&value, range.start);
    let b = text::pos(&value, range.end);
    let mut out = Summary::default();
    if range.is_empty() {
        if let Some(p) = body.paragraphs.get(a.0) {
            out.add_run(resolve_run(p, &joydeck_model::edit::props_at(body, a)));
            out.add_para(resolve_para(p));
        }
        return Ok(out);
    }
    for pi in touched {
        let Some(p) = body.paragraphs.get(pi) else {
            continue;
        };
        out.add_para(resolve_para(p));
        if p.runs.is_empty() {
            out.add_run(resolve_run(p, &p.end_props));
        }
        let from = if pi == a.0 { a.1 } else { 0 };
        let to = if pi == b.0 { b.1 } else { p.char_len() };
        let mut start = 0;
        for r in &p.runs {
            let end = start + r.char_len();
            if start < to && end > from {
                out.add_run(resolve_run(p, &r.props));
            }
            start = end;
        }
    }
    Ok(out)
}

/// Clear visual character overrides without changing navigation or proofing semantics.
pub fn clear_character_format(props: &mut RunProps) {
    *props = RunProps {
        link: props.link.take(),
        hover: props.hover.take(),
        lang: props.lang.take(),
        no_proof: props.no_proof,
        ..Default::default()
    };
}
