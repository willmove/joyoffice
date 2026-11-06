//! Range-scoped click actions on ordinary text runs.
use super::runs;
use joydeck_model::{
    TextBody,
    text::{Hyperlink, Run, RunKind},
};
use std::ops::Range;

#[derive(Debug, PartialEq)]
pub struct Selection {
    pub range: Range<usize>,
    pub common: Option<Hyperlink>,
    pub has_link: bool,
}

pub fn selection(body: &TextBody, mut range: Range<usize>) -> Option<Selection> {
    if range.is_empty() {
        // Treat the trailing boundary as part of the preceding link, like the caret style.
        let caret = range.start;
        let mut joined: Option<(Range<usize>, &Hyperlink)> = None;
        for (span, run) in runs(body) {
            if let (RunKind::Text, Some(link)) = (&run.kind, &run.props.link) {
                if let Some((previous, old)) = joined.as_mut()
                    && previous.end == span.start
                    && *old == link
                {
                    previous.end = span.end;
                    continue;
                }
                if let Some((previous, _)) = &joined
                    && previous.start <= caret
                    && caret <= previous.end
                {
                    break;
                }
                joined = Some((span, link));
            }
        }
        let (span, _) = joined?;
        if caret < span.start || caret > span.end {
            return None;
        }
        range = span;
    }
    let mut selected = runs(body).filter(|(span, run)| {
        run.kind == RunKind::Text && span.start < range.end && range.start < span.end
    });
    let (_, first) = selected.next()?;
    let initial = first.props.link.as_ref();
    let mut has_link = initial.is_some();
    let mut common = initial;
    for (_, run) in selected {
        has_link |= run.props.link.is_some();
        if run.props.link.as_ref() != initial {
            common = None;
        }
    }
    Some(Selection {
        range,
        common: common.cloned(),
        has_link,
    })
}

/// A single-line label can be changed without replacing fields or breaks.
pub fn plain_label(body: &TextBody, range: Range<usize>) -> Option<String> {
    let text = body.text();
    let label = text.get(range.clone())?;
    if label.chars().any(char::is_control)
        || runs(body).any(|(span, run)| {
            span.start < range.end && range.start < span.end && run.kind != RunKind::Text
        })
    {
        return None;
    }
    Some(label.to_owned())
}

pub fn apply(
    body: &TextBody,
    range: Range<usize>,
    target: Option<&Hyperlink>,
) -> Result<TextBody, String> {
    let text = body.text();
    if range.is_empty()
        || range.start > range.end
        || range.end > text.len()
        || !text.is_char_boundary(range.start)
        || !text.is_char_boundary(range.end)
    {
        return Err("invalid link selection".into());
    }
    let mut output = body.clone();
    let mut offset = 0;
    for paragraph in &mut output.paragraphs {
        let mut changed = false;
        let mut out = Vec::with_capacity(paragraph.runs.len());
        for mut run in std::mem::take(&mut paragraph.runs) {
            let start = offset;
            offset += run.content().len();
            let a = range.start.max(start);
            let b = range.end.min(offset);
            if run.kind != RunKind::Text || a >= b || run.props.link.as_ref() == target {
                out.push(run);
                continue;
            }
            changed = true;
            let a = a - start;
            let b = b - start;
            if a > 0 {
                out.push(Run::with(&run.text[..a], run.props.clone()));
            }
            let trailing =
                (b < run.text.len()).then(|| Run::with(&run.text[b..], run.props.clone()));
            run.text.truncate(b);
            run.text.drain(..a);
            run.props.link = target.cloned();
            out.push(run);
            out.extend(trailing);
        }
        paragraph.runs = out;
        if changed {
            paragraph.normalize();
        }
        offset += 1;
    }
    super::validate(&output)?;
    Ok(output)
}
