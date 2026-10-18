//! Resolved styles for the shared page scene (ECMA-376 Part 1 §17.7.2, §17.7.3).
//!
//! Document defaults, then the `basedOn` chain, then direct formatting. The
//! line fitter paints one size, weight and posture per paragraph, plus the
//! spacing and alignment it already places. A theme font is recorded by the
//! typeface the scheme names; the caller-supplied face still shapes the
//! glyphs. Anything this slice cannot paint is [`LayoutError::Unsupported`].

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_model::{
    Color, Document, FontRef, Justification, LineSpacing, Measure, NodeId, Paragraph, RunProps,
    StyleProblem, TabAlign, TabLeader, Underline, displayed_paragraph, indent_edges,
};

use crate::{LayoutError, SingleFontProfile};

pub(crate) struct Flow {
    pub before: Emu,
    pub after: Emu,
    pub line: Option<LineSpacing>,
    pub snap_to_grid: Option<bool>,
    pub keep_next: bool,
    pub keep_lines: bool,
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
    pub flow: Flow,
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
) -> Result<Paint, LayoutError> {
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
    // The paragraph mark is not a painted glyph. Its properties still have
    // to be representable, so an unsupported mark is refused.
    let _mark_face = face_of(node, &mark.props)?;
    let mut uniform: Option<Face> = None;
    if paragraph.runs().is_empty() {
        let empty =
            document
                .styles()
                .resolve_run(paragraph.style.as_ref(), None, &RunProps::default());
        reject_problems(node, &empty.problems)?;
        uniform = Some(face_of(node, &empty.props)?);
    }
    for run in paragraph.runs() {
        let resolved =
            document
                .styles()
                .resolve_run(paragraph.style.as_ref(), run.style.as_ref(), &run.props);
        reject_problems(node, &resolved.problems)?;
        let face = face_of(node, &resolved.props)?;
        match &uniform {
            None => uniform = Some(face),
            Some(first) if first == &face => {}
            Some(_) => {
                return Err(unsupported(node, "mixed character formatting"));
            }
        }
    }
    let face = uniform.unwrap_or(Face {
        size: None,
        bold: false,
        italic: false,
        latin: None,
        east_asia: None,
        complex: None,
    });
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
        flow,
    })
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
    if (listed || edges.moves_text() || custom_tabs)
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
    if !matches!(props.spacing.line, None | Some(LineSpacing::Auto(240))) {
        return Err(unsupported(node, "paragraph line spacing"));
    }
    let single = matches!(props.spacing.line, Some(LineSpacing::Auto(240)));
    if props.snap_to_grid == Some(true) && !single {
        return Err(unsupported(node, "paragraph layout properties"));
    }
    if props.widow_control == Some(true) {
        return Err(unsupported(node, "widow control"));
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
        page_break_before: props.page_break_before == Some(true),
        justification: props.justification,
    })
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

#[derive(Clone, PartialEq, Eq)]
struct Face {
    size: Option<Emu>,
    bold: bool,
    italic: bool,
    latin: Option<Arc<str>>,
    east_asia: Option<Arc<str>>,
    complex: Option<Arc<str>>,
}

fn face_of(node: Option<NodeId>, props: &RunProps) -> Result<Face, LayoutError> {
    if props.preserved.is_some() {
        return Err(unsupported(node, "preserved character properties"));
    }
    if props.strike == Some(true) || props.hidden == Some(true) {
        return Err(unsupported(node, "character formatting"));
    }
    if props
        .underline
        .is_some_and(|underline| underline != Underline::None)
    {
        return Err(unsupported(node, "character formatting"));
    }
    if props.color.is_some_and(|color| color != Color::Auto) {
        return Err(unsupported(node, "character colour"));
    }
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
