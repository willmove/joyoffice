//! Resolved styles for the shared page scene (ECMA-376 Part 1 §17.7.2, §17.7.3).
//!
//! Document defaults, then the `basedOn` chain, then direct formatting. Body
//! paragraphs paint one size, weight and posture per run span
//! ([`paint_spanned`]); headers and footers keep the one-face-per-paragraph
//! contract of [`paint`]. A theme font is recorded by the typeface the scheme
//! names; the caller-supplied face still shapes the glyphs. Anything this
//! slice cannot paint is [`LayoutError::Unsupported`].

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_model::{
    Color, Document, FontRef, Justification, LineSpacing, Measure, NodeId, Paragraph, RunProps,
    StyleId, StyleProblem, TabAlign, TabLeader, TableFormat, TableLook, Underline,
    displayed_paragraph, indent_edges,
};

use crate::shape::PaintedUnderline;
use crate::view::{ViewOptions, run_is_hidden};
use crate::{LayoutError, SingleFontProfile};

pub(crate) struct Flow {
    pub before: Emu,
    pub after: Emu,
    pub line: Option<LineSpacing>,
    pub snap_to_grid: Option<bool>,
    pub keep_next: bool,
    pub keep_lines: bool,
    /// ECMA-376 Part 1 §17.3.1.26 `w:widowControl`: when true, avoid a single
    /// line of this paragraph alone at the top or bottom of a page. JoyWord
    /// keeps at least two lines together at an internal page break when both
    /// fit on the following page; taller-than-page paragraphs still split.
    pub widow_control: bool,
    pub page_break_before: bool,
    pub justification: Option<Justification>,
}

pub(crate) struct Paint {
    pub size: Emu,
    pub leading: Emu,
    pub baseline: Emu,
    pub bold: bool,
    pub italic: bool,
    pub latin: Option<Arc<str>>,
    pub east_asia: Option<Arc<str>>,
    pub color: [u8; 3],
    pub underline: Option<PaintedUnderline>,
}

/// Where a cell sits in its table grid. `column` is the cell's grid start,
/// after `gridBefore`.
pub(crate) struct TableCellPlace {
    pub row: usize,
    pub row_count: usize,
    pub column: usize,
    pub column_count: usize,
}

/// Bold, shading and borders from the table style for one cell.
///
/// ECMA-376 §17.7.6 stacks whole-table formatting, then vertical bands,
/// horizontal bands, the first row, the last row, the first column and the
/// last column. Later layers replace the same property. Direct cell borders,
/// direct shading and a direct run bold are applied by the caller and win.
pub(crate) fn table_cell_format(
    document: &Document,
    table: NodeId,
    style: Option<&StyleId>,
    look: Option<TableLook>,
    place: TableCellPlace,
) -> Result<TableFormat, LayoutError> {
    let (format, problems) = document.styles().resolve_table_cell(
        style,
        look,
        place.row,
        place.row_count,
        place.column,
        place.column_count,
    );
    if problems.is_empty() {
        Ok(format)
    } else {
        Err(unsupported(Some(table), "style resolution"))
    }
}

pub(crate) fn flow(document: &Document, paragraph: &Paragraph) -> Result<Flow, LayoutError> {
    let node = Some(paragraph.id());
    let resolved = document
        .styles()
        .resolve_paragraph(paragraph.style.as_ref(), &paragraph.props);
    reject_problems(node, &resolved.problems)?;
    let props = displayed_paragraph(document, paragraph);
    accept_paragraph(node, &props)
}

pub(crate) fn paint(
    document: &Document,
    paragraph: &Paragraph,
    profile: SingleFontProfile,
    view: ViewOptions,
    table_bold: Option<bool>,
) -> Result<Paint, LayoutError> {
    let node = Some(paragraph.id());
    let resolved_para = document
        .styles()
        .resolve_paragraph(paragraph.style.as_ref(), &paragraph.props);
    reject_problems(node, &resolved_para.problems)?;
    let props = displayed_paragraph(document, paragraph);
    // Story painting still refuses paragraph properties it cannot place.
    accept_paragraph(node, &props)?;
    let mark = document
        .styles()
        .resolve_run(paragraph.style.as_ref(), None, &paragraph.mark_props);
    reject_problems(node, &mark.problems)?;
    // The paragraph mark is not a painted glyph. A vanished mark would join
    // this paragraph to the next; that merge is not implemented.
    if run_is_hidden(&mark.props) {
        return Err(unsupported(node, "character formatting"));
    }
    let _mark_face = face_of(node, &mark.props)?;
    let mut uniform: Option<Face> = None;
    if paragraph.runs().is_empty() {
        let empty =
            document
                .styles()
                .resolve_run(paragraph.style.as_ref(), None, &RunProps::default());
        reject_problems(node, &empty.problems)?;
        let mut face = face_of(node, &empty.props)?;
        if let Some(forced) = table_bold {
            face.bold = forced;
        }
        uniform = Some(face);
    }
    for run in paragraph.runs() {
        let resolved =
            document
                .styles()
                .resolve_run(paragraph.style.as_ref(), run.style.as_ref(), &run.props);
        reject_problems(node, &resolved.problems)?;
        if !view.show_hidden && run_is_hidden(&resolved.props) {
            continue;
        }
        let mut face = face_of(node, &resolved.props)?;
        if run.props.bold.is_none()
            && let Some(forced) = table_bold
        {
            face.bold = forced;
        }
        match &uniform {
            None => uniform = Some(face),
            Some(first) if first == &face => {}
            Some(_) => {
                return Err(unsupported(node, "mixed character formatting"));
            }
        }
    }
    let face = uniform.unwrap_or_else(default_face);
    let size = face.size.unwrap_or(profile.size);
    if !(1..=1_000_000_000).contains(&size.raw()) {
        return Err(unsupported(node, "relative font size"));
    }
    let leading = scale(profile.leading.raw(), size.raw(), profile.size.raw())?;
    let baseline = scale(profile.baseline.raw(), size.raw(), profile.size.raw())?;
    if leading <= 0 || baseline < 0 || baseline > leading {
        return Err(LayoutError::LineTooTall);
    }
    Ok(Paint {
        size,
        leading: Emu::new(leading),
        baseline: Emu::new(baseline),
        bold: face.bold,
        italic: face.italic,
        latin: face.latin,
        east_asia: face.east_asia,
        color: face.color,
        underline: face.underline,
    })
}

/// One visible run's resolved face over its model scalar range.
#[derive(Clone, Debug)]
pub(crate) struct RunFace {
    pub scalars: std::ops::Range<usize>,
    pub face: Face,
}

/// Paragraph paint with per-run faces. `size`, `leading` and `baseline`
/// describe the line box, sized by the largest visible run; `faces` maps
/// model scalar ranges to the face that paints them, adjacent-equal merged
/// so a uniform paragraph yields one span. Hidden runs take no span when the
/// view omits them. `first` feeds consumers that have not switched to spans.
pub(crate) struct SpannedPaint {
    pub flow: Flow,
    pub size: Emu,
    pub leading: Emu,
    pub baseline: Emu,
    pub first: Face,
    pub faces: Vec<RunFace>,
}

/// Resolve every visible run's face over its scalar range. The body layout
/// path uses this instead of [`paint`]: runs may differ in size, weight,
/// posture, colour and underline, and the fitter and placement split the
/// paragraph on the boundaries.
pub(crate) fn paint_spanned(
    document: &Document,
    paragraph: &Paragraph,
    profile: SingleFontProfile,
    view: ViewOptions,
    table_bold: Option<bool>,
) -> Result<SpannedPaint, LayoutError> {
    let node = Some(paragraph.id());
    let resolved_para = document
        .styles()
        .resolve_paragraph(paragraph.style.as_ref(), &paragraph.props);
    reject_problems(node, &resolved_para.problems)?;
    let props = displayed_paragraph(document, paragraph);
    let flow = accept_paragraph(node, &props)?;
    let mark = document
        .styles()
        .resolve_run(paragraph.style.as_ref(), None, &paragraph.mark_props);
    reject_problems(node, &mark.problems)?;
    // The paragraph mark is not a painted glyph. A vanished mark would join
    // this paragraph to the next; that merge is not implemented.
    if run_is_hidden(&mark.props) {
        return Err(unsupported(node, "character formatting"));
    }
    let _mark_face = face_of(node, &mark.props)?;
    let mut faces: Vec<RunFace> = Vec::new();
    let mut offset = 0usize;
    for run in paragraph.runs() {
        let end = offset + run.len;
        let resolved =
            document
                .styles()
                .resolve_run(paragraph.style.as_ref(), run.style.as_ref(), &run.props);
        reject_problems(node, &resolved.problems)?;
        if !view.show_hidden && run_is_hidden(&resolved.props) {
            offset = end;
            continue;
        }
        let mut face = face_of(node, &resolved.props)?;
        if let Some(size) = face.size
            && !(1..=1_000_000_000).contains(&size.raw())
        {
            return Err(unsupported(node, "relative font size"));
        }
        if run.props.bold.is_none()
            && let Some(forced) = table_bold
        {
            face.bold = forced;
        }
        match faces.last_mut() {
            Some(last) if last.scalars.end == offset && last.face == face => {
                last.scalars.end = end;
            }
            _ => faces.push(RunFace {
                scalars: offset..end,
                face,
            }),
        }
        offset = end;
    }
    let size_of = |face: &Face| face.size.unwrap_or(profile.size);
    let size = faces
        .iter()
        .map(|span| size_of(&span.face))
        .max()
        .unwrap_or_else(|| size_of(&default_face()));
    let leading = scale(profile.leading.raw(), size.raw(), profile.size.raw())?;
    let baseline = scale(profile.baseline.raw(), size.raw(), profile.size.raw())?;
    if leading <= 0 || baseline < 0 || baseline > leading {
        return Err(LayoutError::LineTooTall);
    }
    let first = faces
        .first()
        .map(|span| span.face.clone())
        .unwrap_or_else(default_face);
    Ok(SpannedPaint {
        flow,
        size,
        leading: Emu::new(leading),
        baseline: Emu::new(baseline),
        first,
        faces,
    })
}

fn default_face() -> Face {
    Face {
        size: None,
        bold: false,
        italic: false,
        latin: None,
        east_asia: None,
        complex: None,
        color: [0, 0, 0],
        underline: None,
    }
}

fn reject_problems(node: Option<NodeId>, problems: &[StyleProblem]) -> Result<(), LayoutError> {
    if problems.is_empty() {
        Ok(())
    } else {
        Err(unsupported(node, "style resolution"))
    }
}

fn accept_paragraph(
    node: Option<NodeId>,
    props: &joyword_model::ParaProps,
) -> Result<Flow, LayoutError> {
    let Some(edges) = indent_edges(&props.indent) else {
        return Err(unsupported(node, "relative paragraph indent"));
    };
    if let Some(stops) = &props.tabs {
        for stop in stops {
            if stop.position.absolute().is_none() {
                return Err(unsupported(node, "relative tab stop"));
            }
            if stop.align == TabAlign::Bar {
                return Err(unsupported(node, "bar tab"));
            }
            if matches!(stop.leader, TabLeader::MiddleDot | TabLeader::Heavy) {
                return Err(unsupported(node, "tab leader"));
            }
        }
    }
    let listed = props.numbering.is_some_and(|item| item.num_id != 0);
    let custom_tabs = props
        .tabs
        .as_ref()
        .is_some_and(|stops| stops.iter().any(|stop| stop.align != TabAlign::Clear));
    // A list or a custom tab stop carries its own placement. Indent alone
    // still has no centered, right or justified line.
    if edges.moves_text()
        && !listed
        && !custom_tabs
        && matches!(
            props.justification,
            Some(Justification::Center | Justification::End | Justification::Both)
        )
    {
        return Err(unsupported(
            node,
            "justification with indent, numbering or tabs",
        ));
    }
    if props.preserved.is_some() {
        return Err(unsupported(node, "preserved paragraph properties"));
    }
    if matches!(props.justification, Some(Justification::Distribute)) {
        return Err(unsupported(node, "paragraph layout properties"));
    }
    if !line_spacing_supported(props.spacing.line) {
        return Err(unsupported(node, "paragraph line spacing"));
    }
    let single = matches!(props.spacing.line, Some(LineSpacing::Auto(240)));
    if props.snap_to_grid == Some(true) && !single {
        return Err(unsupported(node, "paragraph layout properties"));
    }
    if props.spacing.before_auto == Some(true) || props.spacing.after_auto == Some(true) {
        return Err(unsupported(node, "paragraph spacing"));
    }
    Ok(Flow {
        before: absolute_gap(node, props.spacing.before)?,
        after: absolute_gap(node, props.spacing.after)?,
        line: props.spacing.line,
        snap_to_grid: props.snap_to_grid,
        keep_next: props.keep_next == Some(true),
        keep_lines: props.keep_lines == Some(true),
        widow_control: props.widow_control == Some(true),
        page_break_before: props.page_break_before == Some(true),
        justification: props.justification,
    })
}

fn line_spacing_supported(line: Option<LineSpacing>) -> bool {
    match line {
        None => true,
        Some(LineSpacing::Auto(value)) => value > 0,
        Some(LineSpacing::Exact(measure) | LineSpacing::AtLeast(measure)) => {
            measure.absolute().is_some_and(|length| length.raw() >= 0)
        }
    }
}

fn absolute_gap(node: Option<NodeId>, measure: Option<Measure>) -> Result<Emu, LayoutError> {
    match measure {
        None => Ok(Emu::new(0)),
        Some(measure) => {
            let Some(length) = measure.absolute() else {
                return Err(unsupported(node, "relative or negative paragraph spacing"));
            };
            if length.raw() < 0 {
                return Err(unsupported(node, "relative or negative paragraph spacing"));
            }
            Ok(length)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Face {
    pub(crate) size: Option<Emu>,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) latin: Option<Arc<str>>,
    pub(crate) east_asia: Option<Arc<str>>,
    pub(crate) complex: Option<Arc<str>>,
    pub(crate) color: [u8; 3],
    pub(crate) underline: Option<PaintedUnderline>,
}

fn face_of(node: Option<NodeId>, props: &RunProps) -> Result<Face, LayoutError> {
    if props.preserved.is_some() {
        return Err(unsupported(node, "preserved character properties"));
    }
    if props.strike == Some(true) {
        return Err(unsupported(node, "character formatting"));
    }
    let underline = match props.underline {
        None | Some(Underline::None) => None,
        Some(Underline::Single) => Some(PaintedUnderline::Single),
        Some(Underline::Double) => Some(PaintedUnderline::Double),
        Some(Underline::Dotted) => Some(PaintedUnderline::Dotted),
        Some(_) => return Err(unsupported(node, "character formatting")),
    };
    let color = match props.color {
        None | Some(Color::Auto) => [0, 0, 0],
        Some(Color::Rgb(red, green, blue)) => [red, green, blue],
        Some(Color::Theme { .. }) => return Err(unsupported(node, "character colour")),
    };
    for slot in [
        &props.fonts.ascii,
        &props.fonts.h_ansi,
        &props.fonts.east_asia,
        &props.fonts.complex_script,
    ] {
        if matches!(slot, Some(FontRef::Theme(_))) {
            return Err(unsupported(node, "theme font"));
        }
    }
    let size = match props.size {
        None => None,
        Some(measure) => {
            let Some(length) = measure.absolute().filter(|length| length.raw() > 0) else {
                return Err(unsupported(node, "relative font size"));
            };
            Some(length)
        }
    };
    let latin = latin_name(node, props)?;
    Ok(Face {
        size,
        bold: props.bold == Some(true),
        italic: props.italic == Some(true),
        latin,
        east_asia: typeface(&props.fonts.east_asia),
        complex: typeface(&props.fonts.complex_script),
        color,
        underline,
    })
}

fn latin_name(node: Option<NodeId>, props: &RunProps) -> Result<Option<Arc<str>>, LayoutError> {
    match (typeface(&props.fonts.ascii), typeface(&props.fonts.h_ansi)) {
        (Some(ascii), Some(h_ansi)) if ascii != h_ansi => {
            Err(unsupported(node, "mixed character formatting"))
        }
        (Some(ascii), _) => Ok(Some(ascii)),
        (None, h_ansi) => Ok(h_ansi),
    }
}

fn typeface(font: &Option<FontRef>) -> Option<Arc<str>> {
    match font {
        Some(FontRef::Named(name)) => Some(Arc::clone(name)),
        _ => None,
    }
}

fn scale(value: i64, size: i64, base: i64) -> Result<i64, LayoutError> {
    if size == base {
        return Ok(value);
    }
    let product = i128::from(value) * i128::from(size);
    let denom = i128::from(base);
    let half = denom / 2;
    let rounded = if product >= 0 {
        (product + half) / denom
    } else {
        (product - half) / denom
    };
    i64::try_from(rounded).map_err(|_| LayoutError::GeometryOverflow)
}

fn unsupported(node: Option<NodeId>, feature: &'static str) -> LayoutError {
    LayoutError::Unsupported { node, feature }
}
