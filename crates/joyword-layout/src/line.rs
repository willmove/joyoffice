//! Limited single-font, horizontal LTR line fitting. The caller supplies legal
//! break opportunities as UTF-8 byte offsets, or uses an incremental Unicode
//! default UAX #14 entry point. Neither implements Word-specific Chinese
//! compatibility rules or bidi analysis.
//!
//! Opportunities must also be whole-paragraph shaping cluster boundaries.
//! Remaining-text cluster boundaries are checked again after each break.
//! Each candidate is independently shaped with the existing `shape` function;
//! paragraph glyph slices are never used as line glyphs. The furthest fitting
//! opportunity wins, without assuming candidate widths are monotonic. If none
//! fits, the shortest legal span is emitted intact with an overflow diagnostic.
//! Whitespace, missing glyphs and original text are retained without trimming.
//!
//! Sources: UAX #14 §1 (opportunities versus fitting),
//! <https://www.unicode.org/reports/tr14/tr14-57.html>, and HarfBuzz's cluster
//! model, <https://harfbuzz.github.io/working-with-harfbuzz-clusters.html>.

use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use harfrust::{Direction, UnicodeBuffer};
use joyword_foundation::Emu;
use joyword_model::RevisionKind;

use crate::{FeatureSet, GlyphRun, LoadedFont, ShapedGlyph, ShapedRun, shape};

/// Absolute positions in the input paragraph. Scalars are Unicode scalar
/// values, not graphemes, UTF-16 units, bytes or glyph indices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRange {
    pub bytes: Range<usize>,
    pub scalars: Range<usize>,
}

/// One cluster of the independently shaped line. Glyph indices are local to
/// `LayoutLine::run`; source ranges are absolute in the input paragraph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterRange {
    pub source: SourceRange,
    pub glyphs: Range<usize>,
}

/// A revision span on a fitted line, in model scalars. Paint reads this
/// after placement; it is not a run property.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineRevision {
    pub scalars: Range<usize>,
    pub kind: RevisionKind,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

/// An image attached to a fitted line, before page placement.
///
/// When [`absolute_top`] is `None`, `x` is the distance from the line origin
/// before justification and the picture sits on the baseline. When it is
/// `Some`, `x` and `absolute_top` are page coordinates and the picture does
/// not take line width or line height.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineImage {
    pub media_type: Arc<str>,
    pub bytes: Arc<[u8]>,
    pub x: Emu,
    pub width: Emu,
    pub height: Emu,
    /// Paragraph scalar of the image marker.
    pub scalar: usize,
    /// Page-absolute top. `None` keeps the picture on the line.
    pub absolute_top: Option<Emu>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutLine {
    pub source: SourceRange,
    /// Font identity, EMU size/width and line-local glyph positions/clusters.
    pub run: ShapedRun,
    pub clusters: Vec<ClusterRange>,
    pub images: Vec<LineImage>,
    /// Final-with-markup spans. Empty for an ordinary line and for Final
    /// or Original, which project hidden runs out before shaping.
    pub revisions: Vec<LineRevision>,
    /// Scalar ranges of this line that carry a hyperlink. Screen and PDF
    /// paint these as underlines; run properties stay unchanged.
    pub hyperlinks: Vec<Range<usize>>,
    /// Zero uses the paragraph line box. A positive value replaces it.
    pub box_height: Emu,
    /// Tab gaps in line-local EMU `(x, width)`, recorded where the fitter
    /// consumed a tab without leaving a glyph. Formatting marks draw an arrow
    /// here; the gap is already part of the line's advance.
    pub tab_gaps: Vec<(i64, i64)>,
    /// Glyphs and text bytes of the list label, before tab leaders and body.
    /// Placement splits them into their own run when size or bold differs.
    pub label_glyphs: usize,
    pub label_text_bytes: usize,
    /// Zero means the label uses the body size.
    pub label_size: Emu,
    pub label_bold: bool,
}

impl LayoutLine {
    /// Place already fitted glyphs into the existing scene. The caller owns
    /// baseline spacing and page arrangement; no pagination is implied.
    pub fn to_glyph_run(&self, origin_x: Emu, baseline_y: Emu) -> GlyphRun {
        GlyphRun::place(&self.run, origin_x, baseline_y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowKind {
    SingleCluster,
    UnbreakableSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineDiagnostic {
    Overflow {
        source: SourceRange,
        width: Emu,
        available: Emu,
        kind: OverflowKind,
    },
    MissingGlyph {
        source: SourceRange,
    },
    /// Numbering that this slice will not paint. The paragraph body is still
    /// fitted; the label is omitted rather than guessed.
    Numbering {
        detail: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineLimit {
    TextBytes,
    Breakpoints,
    ShapeCalls,
    ShapedBytes,
    Lines,
}

/// Per-request limits. Values above `default()` are capped at those hard
/// ceilings; callers may lower them. Budget exhaustion returns an error,
/// never a successful partial paragraph. Diagnostic suppression is counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineLimits {
    pub max_text_bytes: usize,
    pub max_breakpoints: usize,
    pub max_shape_calls: usize,
    pub max_shaped_bytes: usize,
    pub max_lines: usize,
    pub max_diagnostics: usize,
}

impl Default for LineLimits {
    fn default() -> Self {
        Self {
            max_text_bytes: 65_536,
            max_breakpoints: 4_096,
            max_shape_calls: 8_192,
            max_shaped_bytes: 16 * 1024 * 1024,
            max_lines: 4_096,
            max_diagnostics: 128,
        }
    }
}

impl LineLimits {
    fn capped(self) -> Self {
        let hard = Self::default();
        Self {
            max_text_bytes: self.max_text_bytes.min(hard.max_text_bytes),
            max_breakpoints: self.max_breakpoints.min(hard.max_breakpoints),
            max_shape_calls: self.max_shape_calls.min(hard.max_shape_calls),
            max_shaped_bytes: self.max_shaped_bytes.min(hard.max_shaped_bytes),
            max_lines: self.max_lines.min(hard.max_lines),
            max_diagnostics: self.max_diagnostics.min(hard.max_diagnostics),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LineStats {
    /// Includes the whole-paragraph cluster probe and every candidate.
    pub shape_calls: usize,
    pub shaped_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParagraphLines {
    /// One empty line for an empty paragraph; otherwise contiguous,
    /// nonempty ranges covering every input byte and scalar exactly once.
    pub lines: Vec<LayoutLine>,
    pub diagnostics: Vec<LineDiagnostic>,
    pub omitted_diagnostics: usize,
    pub stats: LineStats,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakError {
    NotIncreasing,
    NotUtf8Boundary,
    InsideCluster,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineError {
    NonPositiveWidth,
    /// Only the crate-internal cancellable document request returns this.
    Cancelled,
    /// Size must be 1..=1_000_000_000 EMU for bounded shaping arithmetic.
    InvalidSize,
    LimitExceeded(LineLimit),
    InvalidBreak {
        offset: usize,
        reason: BreakError,
    },
    /// Controls, hard separators, bidi formatting controls, or a scalar
    /// whose script HarfRust guesses as RTL. The automatic entry also
    /// rejects discretionary soft hyphens. This is not full UAX #9.
    UnsupportedText {
        offset: usize,
    },
    /// Invalid/nonmonotonic clusters, negative advances or inconsistent
    /// cumulative EMU geometry. No approximate fallback is returned.
    InvalidShaping,
}

/// Fit one paragraph using a single loaded face and explicit features.
///
/// `breakpoints` are strictly increasing offsets in `1..=text.len()` on UTF-8
/// and whole-paragraph cluster boundaries. The end is implicit if omitted;
/// an empty paragraph requires an empty list. Invalid points are rejected.
/// After a break, opportunities inside a newly formed remaining-text cluster
/// are withheld; emitted endpoints always belong to the caller's set or end.
/// The caller must supply linguistically legal opportunities for LTR text;
/// this module only checks their structural safety. Tabs, newlines and bidi
/// controls/scripts are rejected. Combining marks and emoji follow the
/// existing shaper's cluster model, not a separate grapheme implementation.
///
/// Width is advance width, not the ink bounding box. Oversized indivisible
/// spans keep their actual width and original source, with diagnostics.
pub fn layout_paragraph(
    font: &LoadedFont,
    text: &str,
    size: Emu,
    features: FeatureSet,
    width: Emu,
    breakpoints: &[usize],
    limits: LineLimits,
) -> Result<ParagraphLines, LineError> {
    layout_paragraph_adjusted(
        font,
        text,
        size,
        features,
        width,
        breakpoints,
        LineFit {
            limits,
            adjust: keep_shaped,
        },
        &[],
    )
}

fn keep_shaped(_at: usize, _run: &mut ShapedRun) -> Result<(), LineError> {
    Ok(())
}

pub(crate) struct LineFit<F> {
    pub limits: LineLimits,
    pub adjust: F,
}

/// A byte range of the fitted text shaped at its own size, relative to that
/// text. Boundaries fall on scalar boundaries; together the spans cover the
/// whole text. Advances and breaks follow each span's size; weight, posture
/// and colour do not affect shaping and stay with placement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SizeSpan {
    pub bytes: Range<usize>,
    pub size: Emu,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn layout_paragraph_adjusted<F>(
    font: &LoadedFont,
    text: &str,
    size: Emu,
    features: FeatureSet,
    width: Emu,
    breakpoints: &[usize],
    fit: LineFit<F>,
    spans: &[SizeSpan],
) -> Result<ParagraphLines, LineError>
where
    F: FnMut(usize, &mut ShapedRun) -> Result<(), LineError>,
{
    layout_with_breaks(
        font,
        text,
        size,
        features,
        width,
        BreakInput::Explicit(breakpoints),
        fit,
        spans,
    )
}

/// Fit single-font LTR text with Unicode 15.0 default UAX #14 opportunities.
/// Candidates inside whole-paragraph shaping clusters are removed; the usual
/// remaining-text cluster checks and per-candidate re-shaping still apply.
/// All scans, shaping and output use one request budget. Text and whitespace
/// are never trimmed. Hard separators, tabs, bidi and discretionary soft
/// hyphens remain unsupported; no invisible hyphen is silently laid out.
///
/// This is a Unicode default profile, not Word's language-specific Chinese
/// compatibility rules, dictionary segmentation, or automatic hyphenation.
/// Source: UAX #14 rev. 49 (Unicode 15.0), sections 1, 6 and 8.
/// <https://www.unicode.org/reports/tr14/tr14-49.html>
pub fn layout_paragraph_auto(
    font: &LoadedFont,
    text: &str,
    size: Emu,
    features: FeatureSet,
    width: Emu,
    limits: LineLimits,
) -> Result<ParagraphLines, LineError> {
    layout_with_breaks(
        font,
        text,
        size,
        features,
        width,
        BreakInput::Unicode(None),
        LineFit {
            limits,
            adjust: keep_shaped,
        },
        &[],
    )
}

#[derive(Clone, Copy)]
enum BreakInput<'a> {
    Explicit(&'a [usize]),
    Unicode(Option<&'a AtomicBool>),
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn layout_paragraph_auto_cancellable_adjusted<F>(
    font: &LoadedFont,
    text: &str,
    size: Emu,
    features: FeatureSet,
    width: Emu,
    cancellation: &AtomicBool,
    fit: LineFit<F>,
    spans: &[SizeSpan],
) -> Result<ParagraphLines, LineError>
where
    F: FnMut(usize, &mut ShapedRun) -> Result<(), LineError>,
{
    layout_with_breaks(
        font,
        text,
        size,
        features,
        width,
        BreakInput::Unicode(Some(cancellation)),
        fit,
        spans,
    )
}

impl BreakInput<'_> {
    fn check(self) -> Result<(), LineError> {
        if let Self::Unicode(Some(flag)) = self
            && flag.load(Ordering::Acquire)
        {
            return Err(LineError::Cancelled);
        }
        Ok(())
    }
}

fn unicode_breaks(text: &str, limit: usize) -> Result<Vec<usize>, LineError> {
    let mut points = Vec::new();
    for (offset, _) in unicode_linebreak::linebreaks(text) {
        // End is implicit in the fitter, and empty text has no breakpoint.
        if offset == 0 || offset == text.len() {
            continue;
        }
        if points.len() >= limit {
            return Err(LineError::LimitExceeded(LineLimit::Breakpoints));
        }
        let adjusted = crate::slots::adjust_cjk_break(text, offset);
        let chosen = if adjusted > 0 && adjusted < text.len() {
            adjusted
        } else {
            offset
        };
        if !points.contains(&chosen) {
            points.push(chosen);
        }
    }
    Ok(points)
}

#[allow(clippy::too_many_arguments)]
fn layout_with_breaks<F>(
    font: &LoadedFont,
    text: &str,
    size: Emu,
    features: FeatureSet,
    width: Emu,
    input: BreakInput<'_>,
    mut fit: LineFit<F>,
    spans: &[SizeSpan],
) -> Result<ParagraphLines, LineError>
where
    F: FnMut(usize, &mut ShapedRun) -> Result<(), LineError>,
{
    input.check()?;
    let limits = fit.limits.capped();
    let explicit = match input {
        BreakInput::Explicit(points) => points,
        BreakInput::Unicode(_) => &[],
    };
    validate_input(text, size, width, explicit, limits)?;
    if matches!(input, BreakInput::Unicode(_))
        && let Some((offset, _)) = text.char_indices().find(|(_, ch)| *ch == '\u{00ad}')
    {
        // UAX #14 permits breaking after SHY, but rendering a discretionary
        // hyphen requires an additional glyph/position policy not present in
        // this slice. Reject rather than break without the visible hyphen.
        return Err(LineError::UnsupportedText { offset });
    }
    let generated = match input {
        BreakInput::Unicode(_) => unicode_breaks(text, limits.max_breakpoints)?,
        BreakInput::Explicit(_) => Vec::new(),
    };
    let breakpoints = match input {
        BreakInput::Explicit(points) => points,
        BreakInput::Unicode(_) => &generated,
    };
    let mut stats = LineStats::default();
    let mut measure = |byte_start: usize, part: &str| {
        input.check()?;
        let mut run = if spans.is_empty() {
            charge_shape(&mut stats, limits, part.len())?;
            shape(font, part, size, features)
        } else {
            shape_spanned(
                font, part, byte_start, spans, size, features, &mut stats, limits,
            )?
        };
        validate_run(&run)?;
        (fit.adjust)(byte_start, &mut run)?;
        validate_run(&run)?;
        Ok(run)
    };
    let paragraph = measure(0, text)?;
    let mut cluster_starts: Vec<usize> = paragraph.glyphs.iter().map(|g| g.cluster).collect();
    cluster_starts.dedup();
    let safe: Vec<_> = match input {
        BreakInput::Unicode(_) => breakpoints
            .iter()
            .copied()
            .filter(|offset| cluster_starts.binary_search(offset).is_ok())
            .collect(),
        BreakInput::Explicit(_) => Vec::new(),
    };
    let breakpoints = match input {
        BreakInput::Unicode(_) => safe.as_slice(),
        BreakInput::Explicit(_) => breakpoints,
    };
    let ends = validate_breaks(text, breakpoints, &cluster_starts)?;
    let mut result = ParagraphLines {
        lines: Vec::new(),
        diagnostics: Vec::new(),
        omitted_diagnostics: 0,
        stats: LineStats::default(),
    };
    let mut byte_start = 0;
    let mut scalar_start = 0;
    loop {
        input.check()?;
        if result.lines.len() >= limits.max_lines {
            return Err(LineError::LimitExceeded(LineLimit::Lines));
        }
        let (end, run) = if text.is_empty() {
            (0, paragraph.clone())
        } else {
            // Changing the start can change contextual clusters. Retain only
            // opportunities safe in both the paragraph and remaining run.
            let remaining = if byte_start == 0 {
                paragraph.clone()
            } else {
                measure(byte_start, &text[byte_start..])?
            };
            let mut starts: Vec<usize> = remaining
                .glyphs
                .iter()
                .map(|g| byte_start + g.cluster)
                .collect();
            starts.dedup();
            let safe_ends: Vec<usize> = ends
                .iter()
                .copied()
                .filter(|&end| end == text.len() || starts.binary_search(&end).is_ok())
                .collect();
            select_line(byte_start, &safe_ends, width, |end| {
                let run = if end == text.len() {
                    remaining.clone()
                } else {
                    measure(byte_start, &text[byte_start..end])?
                };
                Ok((run.width, run))
            })?
        };
        let source = SourceRange {
            bytes: byte_start..end,
            scalars: scalar_start..scalar_start + run.text.chars().count(),
        };
        let clusters = map_clusters(&run, &source);
        if run.width.raw() > width.raw() {
            result.diagnose(
                LineDiagnostic::Overflow {
                    source: source.clone(),
                    width: run.width,
                    available: width,
                    kind: if clusters.len() == 1 {
                        OverflowKind::SingleCluster
                    } else {
                        OverflowKind::UnbreakableSpan
                    },
                },
                limits.max_diagnostics,
            );
        }
        for cluster in &clusters {
            if run
                .missing
                .binary_search(&(cluster.source.bytes.start - byte_start))
                .is_ok()
            {
                result.diagnose(
                    LineDiagnostic::MissingGlyph {
                        source: cluster.source.clone(),
                    },
                    limits.max_diagnostics,
                );
            }
        }
        byte_start = end;
        scalar_start = source.scalars.end;
        result.lines.push(LayoutLine {
            source,
            run,
            clusters,
            images: Vec::new(),
            revisions: Vec::new(),
            hyperlinks: Vec::new(),
            box_height: Emu::new(0),
            tab_gaps: Vec::new(),
            label_glyphs: 0,
            label_text_bytes: 0,
            label_size: Emu::new(0),
            label_bold: false,
        });
        if byte_start == text.len() {
            break;
        }
    }
    result.stats = stats;
    Ok(result)
}

/// Shape `part` across the size spans of its text. `byte_start` places the
/// part inside the text the spans were built for; every byte of the part is
/// shaped exactly once, at its span's size (gaps at `default_size`). The
/// combined run keeps each glyph's cluster and pen position, so cluster and
/// width invariants match a single-face shape. Every piece charges its own
/// shape call and bytes against the request budget.
#[allow(clippy::too_many_arguments)]
fn shape_spanned(
    font: &LoadedFont,
    part: &str,
    byte_start: usize,
    spans: &[SizeSpan],
    default_size: Emu,
    features: FeatureSet,
    stats: &mut LineStats,
    limits: LineLimits,
) -> Result<ShapedRun, LineError> {
    let end_total = byte_start + part.len();
    let mut combined = ShapedRun {
        font: font.key(),
        size: default_size,
        text: part.to_owned(),
        glyphs: Vec::new(),
        width: Emu::new(0),
        missing: Vec::new(),
        bold: false,
        italic: false,
        latin_family: None,
        east_asia_family: None,
        color: [0, 0, 0],
        underline: None,
    };
    let mut pen = 0_i64;
    let mut push = |shaped: &ShapedRun, offset: usize, pen: &mut i64| -> Result<(), LineError> {
        for glyph in &shaped.glyphs {
            let x = (*pen)
                .checked_add(glyph.x.raw())
                .ok_or(LineError::InvalidShaping)?;
            combined.glyphs.push(ShapedGlyph {
                glyph_id: glyph.glyph_id,
                cluster: glyph.cluster + offset,
                x: Emu::new(x),
                advance: glyph.advance,
                x_offset: glyph.x_offset,
                y_offset: glyph.y_offset,
            });
        }
        for missing in &shaped.missing {
            combined.missing.push(missing + offset);
        }
        *pen = pen
            .checked_add(shaped.width.raw())
            .ok_or(LineError::InvalidShaping)?;
        if shaped.size > combined.size {
            combined.size = shaped.size;
        }
        Ok(())
    };
    let mut cursor = byte_start;
    for span in spans {
        let start = span.bytes.start.max(byte_start);
        if start > cursor {
            let shaped = shape_gap(
                font,
                &part[cursor - byte_start..start - byte_start],
                default_size,
                features,
                stats,
                limits,
            )?;
            push(&shaped, cursor - byte_start, &mut pen)?;
        }
        let end = span.bytes.end.min(end_total).max(start);
        if end > start {
            let shaped = shape_gap(
                font,
                &part[start - byte_start..end - byte_start],
                span.size,
                features,
                stats,
                limits,
            )?;
            push(&shaped, start - byte_start, &mut pen)?;
        }
        cursor = cursor.max(end);
    }
    if cursor < end_total {
        let shaped = shape_gap(
            font,
            &part[cursor - byte_start..],
            default_size,
            features,
            stats,
            limits,
        )?;
        push(&shaped, cursor - byte_start, &mut pen)?;
    }
    combined.width = Emu::new(pen);
    Ok(combined)
}

fn shape_gap(
    font: &LoadedFont,
    piece: &str,
    size: Emu,
    features: FeatureSet,
    stats: &mut LineStats,
    limits: LineLimits,
) -> Result<ShapedRun, LineError> {
    charge_shape(stats, limits, piece.len())?;
    Ok(shape(font, piece, size, features))
}

impl ParagraphLines {
    fn diagnose(&mut self, diagnostic: LineDiagnostic, max: usize) {
        if self.diagnostics.len() < max {
            self.diagnostics.push(diagnostic);
        } else {
            self.omitted_diagnostics += 1;
        }
    }
}

fn validate_input(
    text: &str,
    size: Emu,
    width: Emu,
    breaks: &[usize],
    limits: LineLimits,
) -> Result<(), LineError> {
    if width.raw() <= 0 {
        return Err(LineError::NonPositiveWidth);
    }
    if !(1..=1_000_000_000).contains(&size.raw()) {
        return Err(LineError::InvalidSize);
    }
    if text.len() > limits.max_text_bytes {
        return Err(LineError::LimitExceeded(LineLimit::TextBytes));
    }
    if breaks.len() > limits.max_breakpoints {
        return Err(LineError::LimitExceeded(LineLimit::Breakpoints));
    }
    // Match shape's direction guessing for each scalar, so an RTL script
    // hidden behind an initial Latin/CJK scalar cannot slip through.
    for (offset, ch) in text.char_indices() {
        if ch.is_control()
            || matches!(ch, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            return Err(LineError::UnsupportedText { offset });
        }
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(ch.encode_utf8(&mut [0; 4]));
        buffer.guess_segment_properties();
        if buffer.direction() != Direction::LeftToRight {
            return Err(LineError::UnsupportedText { offset });
        }
    }
    // Byte validity is checked before any candidate slicing or shaping.
    validate_byte_breaks(text, breaks)
}

fn validate_byte_breaks(text: &str, breaks: &[usize]) -> Result<(), LineError> {
    let mut previous = 0;
    for &offset in breaks {
        let reason = if offset <= previous {
            Some(BreakError::NotIncreasing)
        } else if !text.is_char_boundary(offset) {
            Some(BreakError::NotUtf8Boundary)
        } else {
            None
        };
        if let Some(reason) = reason {
            return Err(LineError::InvalidBreak { offset, reason });
        }
        previous = offset;
    }
    Ok(())
}

fn validate_breaks(
    text: &str,
    breaks: &[usize],
    clusters: &[usize],
) -> Result<Vec<usize>, LineError> {
    validate_byte_breaks(text, breaks)?;
    let mut ends = Vec::with_capacity(breaks.len() + 1);
    for &offset in breaks {
        if offset < text.len() && clusters.binary_search(&offset).is_err() {
            return Err(LineError::InvalidBreak {
                offset,
                reason: BreakError::InsideCluster,
            });
        }
        ends.push(offset);
    }
    if ends.last() != Some(&text.len()) {
        ends.push(text.len());
    }
    Ok(ends)
}

fn charge_shape(stats: &mut LineStats, limits: LineLimits, bytes: usize) -> Result<(), LineError> {
    if stats.shape_calls >= limits.max_shape_calls {
        return Err(LineError::LimitExceeded(LineLimit::ShapeCalls));
    }
    if bytes > limits.max_shaped_bytes.saturating_sub(stats.shaped_bytes) {
        return Err(LineError::LimitExceeded(LineLimit::ShapedBytes));
    }
    stats.shape_calls += 1;
    stats.shaped_bytes += bytes;
    Ok(())
}

/// Descending source order finds the furthest fit even when reshaping makes
/// width decrease at a later endpoint. Every iteration consumes a budget in
/// production. Keep the final (shortest) measured span for overflow fallback.
fn select_line<T>(
    start: usize,
    ends: &[usize],
    width: Emu,
    mut measure: impl FnMut(usize) -> Result<(Emu, T), LineError>,
) -> Result<(usize, T), LineError> {
    let mut shortest = None;
    for &end in ends.iter().rev().take_while(|&&end| end > start) {
        let (measured_width, value) = measure(end)?;
        if measured_width.raw() <= width.raw() {
            return Ok((end, value));
        }
        shortest = Some((end, value));
    }
    shortest.ok_or(LineError::InvalidShaping)
}

fn validate_run(run: &ShapedRun) -> Result<(), LineError> {
    let mut previous_cluster = 0;
    let mut pen = 0_i64;
    if run.text.is_empty() != run.glyphs.is_empty()
        || run.glyphs.first().is_some_and(|g| g.cluster != 0)
    {
        return Err(LineError::InvalidShaping);
    }
    for glyph in &run.glyphs {
        if glyph.cluster < previous_cluster
            || glyph.cluster >= run.text.len()
            || !run.text.is_char_boundary(glyph.cluster)
            || glyph.advance.raw() < 0
            || glyph.x.raw() != pen
        {
            return Err(LineError::InvalidShaping);
        }
        previous_cluster = glyph.cluster;
        pen = pen
            .checked_add(glyph.advance.raw())
            .ok_or(LineError::InvalidShaping)?;
        // Keep positioned ink-origin arithmetic in the existing scene safe
        // relative to the run. Page origin arithmetic belongs to the caller.
        glyph
            .x
            .raw()
            .checked_add(glyph.x_offset.raw())
            .ok_or(LineError::InvalidShaping)?;
    }
    if pen != run.width.raw() || run.missing.windows(2).any(|p| p[0] >= p[1]) {
        return Err(LineError::InvalidShaping);
    }
    Ok(())
}

fn map_clusters(run: &ShapedRun, source: &SourceRange) -> Vec<ClusterRange> {
    let mut clusters = Vec::new();
    let mut glyph_start = 0;
    let mut scalar_start = source.scalars.start;
    while glyph_start < run.glyphs.len() {
        let byte_start = run.glyphs[glyph_start].cluster;
        let mut glyph_end = glyph_start + 1;
        while glyph_end < run.glyphs.len() && run.glyphs[glyph_end].cluster == byte_start {
            glyph_end += 1;
        }
        let byte_end = run
            .glyphs
            .get(glyph_end)
            .map_or(run.text.len(), |g| g.cluster);
        let scalar_end = scalar_start + run.text[byte_start..byte_end].chars().count();
        clusters.push(ClusterRange {
            source: SourceRange {
                bytes: source.bytes.start + byte_start..source.bytes.start + byte_end,
                scalars: scalar_start..scalar_end,
            },
            glyphs: glyph_start..glyph_end,
        });
        glyph_start = glyph_end;
        scalar_start = scalar_end;
    }
    clusters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_byte_errors_without_slicing_multibyte_text() {
        let text = "甲e\u{301}😀";
        for (breaks, reason) in [
            (vec![0], BreakError::NotIncreasing),
            (vec![3, 3], BreakError::NotIncreasing),
            (vec![4, 3], BreakError::NotIncreasing),
            (vec![1], BreakError::NotUtf8Boundary),
            (vec![5], BreakError::NotUtf8Boundary),
            (vec![text.len() + 1], BreakError::NotUtf8Boundary),
            (vec![usize::MAX], BreakError::NotUtf8Boundary),
        ] {
            assert!(
                matches!(validate_byte_breaks(text, &breaks), Err(LineError::InvalidBreak { reason: actual, .. }) if actual == reason)
            );
        }
        assert_eq!(validate_byte_breaks(text, &[3, 4, 6, 10]), Ok(()));
    }

    #[test]
    fn combining_and_ligature_cluster_interiors_are_rejected() {
        assert_eq!(
            validate_breaks("e\u{301}fi", &[1], &[0, 3]),
            Err(LineError::InvalidBreak {
                offset: 1,
                reason: BreakError::InsideCluster
            })
        );
        assert_eq!(
            validate_breaks("e\u{301}fi", &[4], &[0, 3]),
            Err(LineError::InvalidBreak {
                offset: 4,
                reason: BreakError::InsideCluster
            })
        );
        assert_eq!(validate_breaks("e\u{301}fi", &[3], &[0, 3]), Ok(vec![3, 5]));
        assert_eq!(validate_breaks("", &[], &[]), Ok(vec![0]));
    }

    #[test]
    fn furthest_fit_does_not_assume_monotonic_widths() {
        let mut measured = Vec::new();
        let selected = select_line(0, &[1, 2, 3, 4], Emu::new(10), |end| {
            measured.push(end);
            Ok((Emu::new([0, 9, 20, 8, 30][end]), end))
        })
        .unwrap();
        assert_eq!(selected, (3, 3));
        assert_eq!(measured, [4, 3]);
    }

    #[test]
    fn exhaustive_small_opportunity_sets_preserve_progress_and_best_fit() {
        for length in 1..=8 {
            for mask in 0..(1 << (length - 1)) {
                let ends: Vec<usize> = (1..=length)
                    .filter(|&end| end == length || mask & (1 << (end - 1)) != 0)
                    .collect();
                for available in 1..=8 {
                    let mut start = 0;
                    let mut covered = Vec::new();
                    while start < length {
                        // Deliberately nonmonotonic synthetic advances.
                        let measure = |end: usize| ((end - start) * 7 + end * 3) % 19 + 1;
                        let expected = ends
                            .iter()
                            .copied()
                            .filter(|&end| end > start && measure(end) <= available)
                            .max()
                            .unwrap_or_else(|| *ends.iter().find(|&&end| end > start).unwrap());
                        let (end, _) =
                            select_line(start, &ends, Emu::new(available as i64), |end| {
                                Ok((Emu::new(measure(end) as i64), ()))
                            })
                            .unwrap();
                        assert_eq!(end, expected);
                        assert!(end > start && end <= length);
                        covered.extend(start..end);
                        start = end;
                    }
                    assert_eq!(covered, (0..length).collect::<Vec<_>>());
                }
            }
        }
    }

    #[test]
    fn indivisible_span_overflows_and_measurement_errors_propagate() {
        assert_eq!(
            select_line(0, &[5, 10], Emu::new(1), |end| Ok((Emu::new(100), end))).unwrap(),
            (5, 5)
        );
        assert_eq!(
            select_line::<()>(0, &[5], Emu::new(1), |_| Err(LineError::LimitExceeded(
                LineLimit::ShapeCalls
            ))),
            Err(LineError::LimitExceeded(LineLimit::ShapeCalls))
        );
    }

    #[test]
    fn validation_covers_dimensions_controls_direction_and_input_limits() {
        let limits = LineLimits::default();
        for width in [i64::MIN, -1, 0] {
            assert_eq!(
                validate_input("", Emu::new(1), Emu::new(width), &[], limits),
                Err(LineError::NonPositiveWidth)
            );
        }
        for size in [i64::MIN, 0, 1_000_000_001, i64::MAX] {
            assert_eq!(
                validate_input("", Emu::new(size), Emu::new(1), &[], limits),
                Err(LineError::InvalidSize)
            );
        }
        for text in [
            "甲\n",
            "甲\r",
            "甲\t",
            "甲\u{2028}",
            "甲\u{2067}",
            "甲א",
            "甲ش",
        ] {
            assert_eq!(
                validate_input(text, Emu::new(1), Emu::new(1), &[], limits),
                Err(LineError::UnsupportedText { offset: 3 })
            );
        }
        assert_eq!(
            validate_input(
                "甲e\u{301}👩\u{200d}💻  ",
                Emu::new(1),
                Emu::new(1),
                &[],
                limits
            ),
            Ok(())
        );
        assert_eq!(
            validate_input(
                "abc",
                Emu::new(1),
                Emu::new(1),
                &[],
                LineLimits {
                    max_text_bytes: 2,
                    ..limits
                }
            ),
            Err(LineError::LimitExceeded(LineLimit::TextBytes))
        );
        assert_eq!(
            validate_input(
                "abc",
                Emu::new(1),
                Emu::new(1),
                &[1],
                LineLimits {
                    max_breakpoints: 0,
                    ..limits
                }
            ),
            Err(LineError::LimitExceeded(LineLimit::Breakpoints))
        );
    }

    #[test]
    fn shaping_budgets_charge_before_work_and_cannot_be_raised() {
        let hard = LineLimits::default();
        let high = LineLimits {
            max_text_bytes: usize::MAX,
            max_breakpoints: usize::MAX,
            max_shape_calls: usize::MAX,
            max_shaped_bytes: usize::MAX,
            max_lines: usize::MAX,
            max_diagnostics: usize::MAX,
        };
        assert_eq!(high.capped(), hard);
        let mut stats = LineStats::default();
        let limits = LineLimits {
            max_shape_calls: 2,
            max_shaped_bytes: 3,
            ..hard
        };
        assert_eq!(charge_shape(&mut stats, limits, 2), Ok(()));
        assert_eq!(
            charge_shape(&mut stats, limits, 2),
            Err(LineError::LimitExceeded(LineLimit::ShapedBytes))
        );
        assert_eq!(
            stats,
            LineStats {
                shape_calls: 1,
                shaped_bytes: 2
            }
        );
        assert_eq!(charge_shape(&mut stats, limits, 1), Ok(()));
        assert_eq!(
            charge_shape(&mut stats, limits, 0),
            Err(LineError::LimitExceeded(LineLimit::ShapeCalls))
        );
    }
}
