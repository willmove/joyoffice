//! List labels and tab stops on the shared page scene.
//!
//! The ordinary line fitter rejects tab characters. This path is used only
//! when a paragraph has a list label, a non-zero indent, a custom tab stop,
//! or a tab in its text. Body glyphs keep paragraph source offsets. A label
//! is prefixed onto the first line's run so screen and PDF both show it.
//! Cluster ranges stay on the body, which is what carets follow.

use joyword_foundation::Emu;
use joyword_model::{
    Justification, Measure, NodeId, NumberLabel, NumberSuffix, ParaProps, TabAlign, TabLeader,
    indent_edges, tab_positions,
};

use crate::cjk::{self, CjkIssue, ScriptRun};

use crate::line::{ClusterRange, LayoutLine, LineDiagnostic, ParagraphLines, SourceRange};
use crate::shape::{FeatureSet, ShapedGlyph, ShapedRun, shape};
use crate::{LayoutError, LoadedFont};

pub(crate) fn needs_list_layout(
    props: &ParaProps,
    text: &str,
    label: Option<&NumberLabel>,
) -> bool {
    let indent = indent_edges(&props.indent).is_some_and(|edges| edges.moves_text());
    let tabs = props
        .tabs
        .as_ref()
        .is_some_and(|stops| stops.iter().any(|stop| stop.align != TabAlign::Clear));
    let marked =
        label.is_some_and(|label| !label.text.is_empty() || label.suffix != NumberSuffix::Tab);
    indent || tabs || marked || text.as_bytes().contains(&b'\t')
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn layout_marked<F>(
    text: &str,
    props: &ParaProps,
    label: Option<&NumberLabel>,
    content_width: Emu,
    interval: i64,
    font: &LoadedFont,
    size: Emu,
    features: FeatureSet,
    justification: Option<Justification>,
    node: Option<NodeId>,
    mut fit_width: F,
) -> Result<ParagraphLines, LayoutError>
where
    F: FnMut(&str, Emu) -> Result<ParagraphLines, LayoutError>,
{
    let Some(edges) = indent_edges(&props.indent) else {
        return Err(unsupported("relative paragraph indent"));
    };
    let right_edge = content_width.raw() - edges.end;
    let right_edge = if right_edge > edges.start {
        right_edge
    } else {
        edges.start.saturating_add(1)
    };
    let stops = props.tabs.as_deref().unwrap_or(&[]);
    let Some(stops) = tab_positions(stops, interval, right_edge) else {
        return Err(unsupported("relative tab stop"));
    };
    let left = edges.start;
    let label_x = left - edges.hanging;
    let mut label_end = label_x;
    let mut prefix: Option<ShapedRun> = None;
    let label_size = match label.and_then(|item| item.size) {
        None => size,
        Some(measure) => absolute_label_size(measure)?,
    };
    let label_bold = label.is_some_and(|item| item.bold);
    if let Some(label) = label {
        if !label.text.is_empty() {
            let shaped = shape(font, &label.text, label_size, features);
            label_end = label_x.saturating_add(shaped.width.raw());
            prefix = Some(shaped);
        }
        label_end = match label.suffix {
            NumberSuffix::Tab => next_stop(&stops, label_end)
                .map(|stop| stop.position)
                .unwrap_or(label_end),
            NumberSuffix::Space => {
                let space = shape(font, " ", label_size, features);
                let end = label_end.saturating_add(space.width.raw());
                if let Some(run) = prefix.as_mut() {
                    append_run(run, &space, run.width.raw());
                } else {
                    prefix = Some(space);
                }
                end
            }
            NumberSuffix::Nothing => label_end,
        };
    }
    let body_start = if label.is_some() {
        label_end
    } else if edges.hanging != 0 {
        left - edges.hanging
    } else {
        left + edges.first_line
    };
    let continuation = left;
    let mut line = LineBuilder::new(body_start);
    if let Some(run) = &prefix {
        let leader = label
            .and_then(|item| (item.suffix == NumberSuffix::Tab).then_some(item))
            .and_then(|_| {
                next_stop(
                    &stops,
                    label_x.saturating_add(prefix_width(prefix.as_ref())),
                )
            })
            .map(|stop| stop.leader)
            .unwrap_or(TabLeader::None);
        // The label itself sits at label_x. A tab suffix's leader fills the
        // gap from the end of the label glyphs to the body.
        let glyph_end = label_x.saturating_add(run.width.raw());
        line.add_decoration(run, label_x);
        line.mark_label(
            label_bold,
            if label_size == size {
                Emu::new(0)
            } else {
                label_size
            },
        );
        if body_start > glyph_end
            && let Some(leaders) = leader_run(font, size, features, leader, body_start - glyph_end)?
        {
            line.add_decoration(&leaders, glyph_end);
        }
    }
    let mut lines = Vec::new();
    let mut diagnostics = Vec::new();
    let mut omitted = 0usize;
    let mut stats = crate::line::LineStats::default();
    let mut pen = body_start;
    let mut at_break = true;
    for piece in split_tabs(text) {
        if piece.tab_before {
            let stop = next_stop(&stops, pen);
            let align = stop.map(|stop| stop.align).unwrap_or(TabAlign::Left);
            let leader = stop.map(|stop| stop.leader).unwrap_or(TabLeader::None);
            let stop_at = stop.map(|stop| stop.position).unwrap_or(pen);
            let placed = if piece.text.is_empty() {
                stop_at
            } else {
                align_start(font, piece.text, size, features, align, stop_at)?
            };
            if placed > pen
                && let Some(leaders) = leader_run(font, size, features, leader, placed - pen)?
            {
                line.add_decoration(&leaders, pen);
            }
            line.cover(
                piece.byte.saturating_sub(1),
                piece.scalar.saturating_sub(1),
                piece.byte,
                piece.scalar,
            );
            pen = placed.max(pen);
            if piece.text.is_empty() {
                line.body_edge = pen;
                continue;
            }
            if placed + measure_width(font, piece.text, size, features) > right_edge
                && !matches!(align, TabAlign::Left | TabAlign::Num)
            {
                flush(&mut lines, &mut line, continuation, font, size);
                pen = continuation;
                at_break = true;
            } else {
                at_break = false;
            }
        }
        if piece.text.is_empty() {
            continue;
        }
        let origin = if at_break { continuation } else { pen };
        let available = (right_edge - origin).max(1);
        let continuation_width = (right_edge - continuation).max(1);
        let fitted = fit_width(piece.text, Emu::new(available))?;
        stats.shape_calls += fitted.stats.shape_calls;
        stats.shaped_bytes += fitted.stats.shaped_bytes;
        omitted += fitted.omitted_diagnostics;
        let first_width = available;
        if fitted.lines.len() > 1 && first_width != continuation_width {
            let consumed = fitted.lines[0].source.bytes.end;
            absorb(
                &mut line,
                &fitted.lines[0],
                origin,
                piece.byte,
                piece.scalar,
                &mut diagnostics,
            );
            pen = origin.saturating_add(fitted.lines[0].run.width.raw());
            flush(&mut lines, &mut line, continuation, font, size);
            if consumed < piece.text.len() {
                let rest = &piece.text[consumed..];
                let rest_scalar = piece.scalar + piece.text[..consumed].chars().count();
                let rest_byte = piece.byte + consumed;
                let refit = fit_width(rest, Emu::new(continuation_width))?;
                stats.shape_calls += refit.stats.shape_calls;
                stats.shaped_bytes += refit.stats.shaped_bytes;
                omitted += refit.omitted_diagnostics;
                place_lines(
                    &mut lines,
                    &mut line,
                    &refit,
                    continuation,
                    rest_byte,
                    rest_scalar,
                    &mut diagnostics,
                    &mut pen,
                    font,
                    size,
                )?;
            }
        } else {
            place_lines(
                &mut lines,
                &mut line,
                &fitted,
                origin,
                piece.byte,
                piece.scalar,
                &mut diagnostics,
                &mut pen,
                font,
                size,
            )?;
        }
        at_break = false;
    }
    lines.push(line.finish(font, size));
    align_lines(
        &mut lines,
        justification,
        edges.start,
        right_edge,
        content_width.raw(),
        text,
        props,
        node,
    )?;
    Ok(ParagraphLines {
        lines,
        diagnostics,
        omitted_diagnostics: omitted,
        stats,
    })
}

fn prefix_width(run: Option<&ShapedRun>) -> i64 {
    run.map(|run| run.width.raw()).unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
fn place_lines(
    lines: &mut Vec<LayoutLine>,
    current: &mut LineBuilder,
    fitted: &ParagraphLines,
    origin: i64,
    byte_base: usize,
    scalar_base: usize,
    diagnostics: &mut Vec<LineDiagnostic>,
    pen: &mut i64,
    font: &LoadedFont,
    size: Emu,
) -> Result<(), LayoutError> {
    for (index, line) in fitted.lines.iter().enumerate() {
        if index > 0 {
            flush(lines, current, origin, font, size);
        }
        let line_origin = if index == 0 {
            origin
        } else {
            current.body_edge
        };
        absorb(
            current,
            line,
            line_origin,
            byte_base,
            scalar_base,
            diagnostics,
        );
        *pen = line_origin.saturating_add(line.run.width.raw());
        current.body_edge = *pen;
    }
    Ok(())
}

fn flush(
    lines: &mut Vec<LayoutLine>,
    current: &mut LineBuilder,
    continuation: i64,
    font: &LoadedFont,
    size: Emu,
) {
    let finished = std::mem::replace(current, LineBuilder::new(continuation));
    lines.push(finished.finish(font, size));
}

fn absorb(
    current: &mut LineBuilder,
    line: &LayoutLine,
    origin: i64,
    byte_base: usize,
    scalar_base: usize,
    diagnostics: &mut Vec<LineDiagnostic>,
) {
    let text_base = current.text.len();
    let glyph_base = current.glyphs.len();
    current.text.push_str(&line.run.text);
    for glyph in &line.run.glyphs {
        let mut glyph = *glyph;
        glyph.cluster += text_base;
        glyph.x = Emu::new(glyph.x.raw().saturating_add(origin));
        current.glyphs.push(glyph);
    }
    for cluster in &line.clusters {
        current.clusters.push(ClusterRange {
            source: SourceRange {
                bytes: cluster.source.bytes.start + byte_base..cluster.source.bytes.end + byte_base,
                scalars: cluster.source.scalars.start + scalar_base
                    ..cluster.source.scalars.end + scalar_base,
            },
            glyphs: cluster.glyphs.start + glyph_base..cluster.glyphs.end + glyph_base,
        });
    }
    if !line.source.bytes.is_empty() || current.source_start.is_none() {
        let start = byte_base + line.source.bytes.start;
        let end = byte_base + line.source.bytes.end;
        let scalar_start = scalar_base + line.source.scalars.start;
        let scalar_end = scalar_base + line.source.scalars.end;
        current.cover(start, scalar_start, end, scalar_end);
    }
    let _ = diagnostics;
}

struct Piece<'a> {
    text: &'a str,
    byte: usize,
    scalar: usize,
    tab_before: bool,
}

fn split_tabs(text: &str) -> Vec<Piece<'_>> {
    let mut parts = Vec::new();
    let mut byte = 0usize;
    let mut scalar = 0usize;
    let mut tab_before = false;
    for (index, ch) in text.char_indices() {
        if ch != '\t' {
            continue;
        }
        parts.push(Piece {
            text: &text[byte..index],
            byte,
            scalar,
            tab_before,
        });
        scalar += text[byte..index].chars().count() + 1;
        byte = index + ch.len_utf8();
        tab_before = true;
    }
    parts.push(Piece {
        text: &text[byte..],
        byte,
        scalar,
        tab_before,
    });
    parts
}

fn next_stop(stops: &[joyword_model::PlacedTab], pen: i64) -> Option<joyword_model::PlacedTab> {
    stops.iter().copied().find(|stop| stop.position > pen)
}

fn align_start(
    font: &LoadedFont,
    text: &str,
    size: Emu,
    features: FeatureSet,
    align: TabAlign,
    stop: i64,
) -> Result<i64, LayoutError> {
    let shaped = shape(font, text, size, features);
    let width = shaped.width.raw();
    Ok(match align {
        TabAlign::Left | TabAlign::Num | TabAlign::Clear => stop,
        TabAlign::Right => stop.saturating_sub(width),
        TabAlign::Center => stop.saturating_sub(width / 2),
        TabAlign::Decimal => stop.saturating_sub(before_decimal(text, &shaped)),
        TabAlign::Bar => return Err(unsupported("bar tab")),
    })
}

fn before_decimal(text: &str, run: &ShapedRun) -> i64 {
    let Some(index) = text.find(['.', '\u{ff0e}']) else {
        return run.width.raw();
    };
    let mut end = 0i64;
    for glyph in &run.glyphs {
        if glyph.cluster >= index {
            break;
        }
        end = glyph.x.raw().saturating_add(glyph.advance.raw());
    }
    end
}

fn measure_width(font: &LoadedFont, text: &str, size: Emu, features: FeatureSet) -> i64 {
    shape(font, text, size, features).width.raw()
}

fn leader_run(
    font: &LoadedFont,
    size: Emu,
    features: FeatureSet,
    leader: TabLeader,
    gap: i64,
) -> Result<Option<ShapedRun>, LayoutError> {
    let glyph = match leader {
        TabLeader::None => return Ok(None),
        TabLeader::Dot => ".",
        TabLeader::Hyphen => "-",
        TabLeader::Underscore => "_",
        TabLeader::MiddleDot | TabLeader::Heavy => return Err(unsupported("tab leader")),
    };
    if gap <= 0 {
        return Ok(None);
    }
    let one = shape(font, glyph, size, features);
    let advance = one.width.raw();
    if advance <= 0 {
        return Ok(None);
    }
    let count = (gap / advance) as usize;
    if count == 0 {
        return Ok(None);
    }
    Ok(Some(shape(font, &glyph.repeat(count), size, features)))
}

fn append_run(base: &mut ShapedRun, extra: &ShapedRun, origin: i64) {
    let text_base = base.text.len();
    base.text.push_str(&extra.text);
    for glyph in &extra.glyphs {
        let mut glyph = *glyph;
        glyph.cluster += text_base;
        glyph.x = Emu::new(glyph.x.raw().saturating_add(origin));
        base.glyphs.push(glyph);
    }
    base.width = Emu::new(base.width.raw().saturating_add(extra.width.raw()));
}

struct LineBuilder {
    text: String,
    glyphs: Vec<ShapedGlyph>,
    clusters: Vec<ClusterRange>,
    source_start: Option<(usize, usize)>,
    source_end: (usize, usize),
    body_edge: i64,
    font_key: Option<crate::FontKey>,
    label_glyphs: usize,
    label_text_bytes: usize,
    label_size: Emu,
    label_bold: bool,
}

impl LineBuilder {
    fn new(body_edge: i64) -> Self {
        Self {
            text: String::new(),
            glyphs: Vec::new(),
            clusters: Vec::new(),
            source_start: None,
            source_end: (0, 0),
            body_edge,
            font_key: None,
            label_glyphs: 0,
            label_text_bytes: 0,
            label_size: Emu::new(0),
            label_bold: false,
        }
    }

    fn mark_label(&mut self, bold: bool, size: Emu) {
        self.label_glyphs = self.glyphs.len();
        self.label_text_bytes = self.text.len();
        self.label_bold = bold;
        self.label_size = size;
    }

    fn cover(&mut self, byte: usize, scalar: usize, end_byte: usize, end_scalar: usize) {
        if self.source_start.is_none() {
            self.source_start = Some((byte, scalar));
        }
        self.source_end = (end_byte, end_scalar);
    }

    fn add_decoration(&mut self, run: &ShapedRun, origin: i64) {
        let text_base = self.text.len();
        self.text.push_str(&run.text);
        self.font_key = Some(run.font);
        for glyph in &run.glyphs {
            let mut glyph = *glyph;
            glyph.cluster += text_base;
            glyph.x = Emu::new(glyph.x.raw().saturating_add(origin));
            self.glyphs.push(glyph);
        }
    }

    fn finish(self, font: &LoadedFont, size: Emu) -> LayoutLine {
        let glyph_right = self
            .glyphs
            .iter()
            .map(|glyph| glyph.x.raw().saturating_add(glyph.advance.raw()))
            .max()
            .unwrap_or(0);
        let width = glyph_right.max(self.body_edge).max(0);
        let (start_byte, start_scalar) = self.source_start.unwrap_or((0, 0));
        LayoutLine {
            source: SourceRange {
                bytes: start_byte..self.source_end.0.max(start_byte),
                scalars: start_scalar..self.source_end.1.max(start_scalar),
            },
            run: ShapedRun {
                font: self.font_key.unwrap_or_else(|| font.key()),
                size,
                text: self.text,
                glyphs: self.glyphs,
                width: Emu::new(width),
                missing: Vec::new(),
                bold: false,
                italic: false,
                latin_family: None,
                east_asia_family: None,
            },
            clusters: self.clusters,
            images: Vec::new(),
            revisions: Vec::new(),
            hyperlinks: Vec::new(),
            box_height: Emu::new(0),
            label_glyphs: self.label_glyphs,
            label_text_bytes: self.label_text_bytes,
            label_size: self.label_size,
            label_bold: self.label_bold,
        }
    }
}

fn absolute_label_size(measure: Measure) -> Result<Emu, LayoutError> {
    match measure.absolute().filter(|length| length.raw() > 0) {
        Some(length) => Ok(length),
        None => Err(unsupported("relative font size")),
    }
}

#[allow(clippy::too_many_arguments)]
fn align_lines(
    lines: &mut [LayoutLine],
    justification: Option<Justification>,
    area_left: i64,
    area_right: i64,
    frame_width: i64,
    text: &str,
    props: &ParaProps,
    node: Option<NodeId>,
) -> Result<(), LayoutError> {
    let Some(justification) = justification else {
        return Ok(());
    };
    if !matches!(
        justification,
        Justification::Center | Justification::End | Justification::Both
    ) {
        return Ok(());
    }
    let tabs = text.as_bytes().contains(&b'\t')
        || props
            .tabs
            .as_ref()
            .is_some_and(|stops| stops.iter().any(|stop| stop.align != TabAlign::Clear));
    let last = lines.len().saturating_sub(1);
    for (index, line) in lines.iter_mut().enumerate() {
        if !tabs {
            match justification {
                Justification::Center | Justification::End => {
                    shift_body(line, area_left, area_right, justification);
                }
                Justification::Both => justify_body(line, area_right, index == last, node)?,
                _ => {}
            }
        }
        line.run.width = Emu::new(frame_width.max(0));
    }
    Ok(())
}

fn body_glyphs(line: &LayoutLine) -> Vec<usize> {
    let mut indexes = Vec::new();
    for cluster in &line.clusters {
        indexes.extend(cluster.glyphs.clone());
    }
    indexes.sort_unstable();
    indexes.dedup();
    indexes.retain(|index| *index < line.run.glyphs.len());
    indexes
}

fn shift_body(
    line: &mut LayoutLine,
    area_left: i64,
    area_right: i64,
    justification: Justification,
) {
    let indexes = body_glyphs(line);
    if indexes.is_empty() {
        return;
    }
    let left = indexes
        .iter()
        .map(|index| line.run.glyphs[*index].x.raw())
        .min()
        .unwrap_or(0);
    let right = indexes
        .iter()
        .map(|index| {
            let glyph = &line.run.glyphs[*index];
            glyph.x.raw().saturating_add(glyph.advance.raw())
        })
        .max()
        .unwrap_or(left);
    let width = (right - left).max(0);
    let slack = (area_right - area_left - width).max(0);
    let target = match justification {
        Justification::End => area_left + slack,
        _ => area_left + slack / 2,
    };
    let shift = target - left;
    if shift == 0 {
        return;
    }
    for index in indexes {
        let glyph = &mut line.run.glyphs[index];
        glyph.x = Emu::new(glyph.x.raw().saturating_add(shift));
    }
}

fn justify_body(
    line: &mut LayoutLine,
    area_right: i64,
    last: bool,
    node: Option<NodeId>,
) -> Result<(), LayoutError> {
    let indexes = body_glyphs(line);
    if indexes.is_empty() {
        return Ok(());
    }
    let mut han = false;
    for index in &indexes {
        let cluster = line.run.glyphs[*index].cluster;
        let Some(ch) = line.run.text[cluster..].chars().next() else {
            return Err(unsupported_node(node, "inter-character justification"));
        };
        if cjk::script_run(ch) == ScriptRun::Han {
            han = true;
        } else if !r3_mark(ch) {
            return Err(unsupported_node(node, "inter-character justification"));
        }
    }
    if !han {
        return Err(unsupported_node(node, "inter-character justification"));
    }
    let origin = line.run.glyphs[indexes[0]].x.raw();
    let width = Emu::new((area_right - origin).max(0));
    let advances: Vec<Emu> = indexes
        .iter()
        .map(|index| line.run.glyphs[*index].advance)
        .collect();
    let gaps = match cjk::justify_intercharacter(&advances, width, last) {
        Ok(justified) => justified.gaps,
        Err(CjkIssue::UnverifiedJustify | CjkIssue::DoesNotFit) => return Ok(()),
        Err(CjkIssue::BudgetExceeded) => {
            return Err(unsupported_node(node, "justification budget"));
        }
        Err(_) => return Err(LayoutError::GeometryOverflow),
    };
    let mut pen = origin;
    let count = indexes.len();
    for (glyph_index, index) in indexes.into_iter().enumerate() {
        let gap = if glyph_index + 1 == count {
            0
        } else {
            gaps[glyph_index].raw()
        };
        let advance = line.run.glyphs[index]
            .advance
            .raw()
            .checked_add(gap)
            .ok_or(LayoutError::GeometryOverflow)?;
        line.run.glyphs[index].x = Emu::new(pen);
        line.run.glyphs[index].advance = Emu::new(advance);
        pen = pen
            .checked_add(advance)
            .ok_or(LayoutError::GeometryOverflow)?;
    }
    Ok(())
}

fn r3_mark(ch: char) -> bool {
    !ch.is_ascii()
        && (cjk::prohibits_line_start(ch)
            || cjk::prohibits_line_end(ch)
            || matches!(ch, '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}'))
}

fn unsupported_node(node: Option<NodeId>, feature: &'static str) -> LayoutError {
    LayoutError::Unsupported { node, feature }
}

fn unsupported(feature: &'static str) -> LayoutError {
    LayoutError::Unsupported {
        node: None,
        feature,
    }
}
