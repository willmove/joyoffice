//! Format painter (EDIT-007).
//!
//! The brush stores a paragraph style, justification and the direct character
//! formatting [`crate::command::RunChange`] can apply. Theme colours, theme
//! fonts, hidden text, languages, preserved markup and sizes that are not
//! non-negative half-points are refused at copy time. Applying refuses a
//! target whose result the DOCX exporter cannot represent: a style-id change
//! must stay inside the style-assignment subset, and a direct-only change
//! must start from a plain paragraph.

use joyword_model::{
    Block, Color, Document, FontRef, Indent, InlineKind, Justification, Languages, Measure,
    Paragraph, Position, RunProps, ScalarOffset, Spacing, StyleId,
};

use crate::command::{Command, ParagraphChange, RunChange};
use crate::locate::{self, Location};

#[derive(Clone, Debug, PartialEq)]
pub struct FormatBrush {
    /// Set when the source selection is a caret, a whole paragraph, or spans
    /// more than one paragraph.
    pub paint_paragraph: bool,
    pub paragraph_style: Option<StyleId>,
    pub justification: Option<Justification>,
    pub character_style: Option<StyleId>,
    pub changes: Vec<RunChange>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintError {
    Unexportable(&'static str),
    Locked,
    CrossContainer,
    Missing,
    Nothing,
}

impl std::fmt::Display for PaintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaintError::Unexportable(reason) => f.write_str(reason),
            PaintError::Locked => f.write_str("locked content"),
            PaintError::CrossContainer => f.write_str("range crosses a container"),
            PaintError::Missing => f.write_str("missing paragraph"),
            PaintError::Nothing => f.write_str("nothing to paint"),
        }
    }
}

/// Copy exportable formatting from the selection. `start` and `end` may be
/// reversed; they must sit in one container.
pub fn capture(
    document: &Document,
    start: Position,
    end: Position,
) -> Result<FormatBrush, PaintError> {
    let (start, end) = normalize(document, start, end)?;
    let paragraph = paragraph_at(document, start)?;
    let sample_at = if start.offset.0 < paragraph.len() {
        ScalarOffset(start.offset.0 + 1)
    } else {
        start.offset
    };
    let (character_style, props) = paragraph.format_at(sample_at);
    let changes = direct_changes(&props)?;
    let paint_paragraph = covers_paragraph(paragraph, start, end);
    Ok(FormatBrush {
        paint_paragraph,
        paragraph_style: paragraph.style.clone(),
        justification: paragraph.props.justification,
        character_style,
        changes,
    })
}

/// Commands for one undoable apply. The target is refused whole when export
/// of the result would be refused.
pub fn paint_commands(
    document: &Document,
    start: Position,
    end: Position,
    brush: &FormatBrush,
) -> Result<Vec<Command>, PaintError> {
    let (start, end) = normalize(document, start, end)?;
    let (first, last) = ends(document, start, end)?;
    let seq =
        locate::container(document, start.story, &first.containers).ok_or(PaintError::Missing)?;
    let mut targets: Vec<(&Paragraph, usize, usize)> = Vec::new();
    for index in first.index..=last.index {
        let paragraph = seq
            .get(index)
            .and_then(Block::as_paragraph)
            .ok_or(PaintError::CrossContainer)?;
        let from = if paragraph.id() == start.paragraph {
            start.offset.0
        } else {
            0
        };
        let to = if paragraph.id() == end.paragraph {
            end.offset.0
        } else {
            paragraph.len()
        };
        targets.push((paragraph, from, to));
    }
    let paints_characters = start.paragraph != end.paragraph || start.offset != end.offset;
    if !brush.paint_paragraph && !paints_characters {
        return Err(PaintError::Nothing);
    }
    if !brush.paint_paragraph && brush.character_style.is_none() && brush.changes.is_empty() {
        return Err(PaintError::Nothing);
    }
    for (paragraph, from, to) in &targets {
        let reassign = will_reassign(paragraph, brush, *from, *to, paints_characters);
        if reassign {
            style_subset(paragraph)?;
        } else if !before_plain(paragraph) {
            return Err(PaintError::Unexportable(
                "existing formatting cannot be repainted without a style change",
            ));
        }
    }
    let mut commands = Vec::new();
    if paints_characters && let Some(style) = &brush.character_style {
        commands.push(Command::ApplyStyle {
            start,
            end,
            style: style.clone(),
        });
    }
    if brush.paint_paragraph {
        for (paragraph, _, _) in &targets {
            commands.push(Command::FormatParagraph {
                story: start.story,
                paragraph: paragraph.id(),
                change: ParagraphChange::Style(brush.paragraph_style.clone()),
            });
            commands.push(Command::FormatParagraph {
                story: start.story,
                paragraph: paragraph.id(),
                change: ParagraphChange::Justification(brush.justification),
            });
        }
    }
    if paints_characters {
        for change in &brush.changes {
            commands.push(Command::FormatText {
                start,
                end,
                change: change.clone(),
            });
        }
    }
    if commands.is_empty() {
        return Err(PaintError::Nothing);
    }
    Ok(commands)
}

fn covers_paragraph(paragraph: &Paragraph, start: Position, end: Position) -> bool {
    if start.paragraph != end.paragraph {
        return true;
    }
    if start.offset == end.offset {
        return true;
    }
    start.offset.0 == 0 && end.offset.0 == paragraph.len()
}

fn will_reassign(
    paragraph: &Paragraph,
    brush: &FormatBrush,
    from: usize,
    to: usize,
    paints_characters: bool,
) -> bool {
    if brush.paint_paragraph && brush.paragraph_style != paragraph.style {
        return true;
    }
    if !paints_characters {
        return false;
    }
    if let Some(style) = &brush.character_style
        && from != to
        && runs_differ(paragraph, from, to, style)
    {
        return true;
    }
    false
}

fn runs_differ(paragraph: &Paragraph, from: usize, to: usize, style: &StyleId) -> bool {
    let mut cursor = 0usize;
    for run in paragraph.runs() {
        let next = cursor + run.len;
        if next > from && cursor < to && run.style.as_ref() != Some(style) {
            return true;
        }
        cursor = next;
    }
    false
}

fn before_plain(paragraph: &Paragraph) -> bool {
    paragraph.style.is_none()
        && paragraph.props.is_empty()
        && paragraph.mark_props.is_empty()
        && paragraph.inlines().is_empty()
        && paragraph
            .runs()
            .iter()
            .all(|run| run.style.is_none() && run.props.is_empty())
}

/// Mirrors `style_assignment_ok` plus the theme-colour refusal in `synthesize_rpr`.
fn style_subset(paragraph: &Paragraph) -> Result<(), PaintError> {
    let props = &paragraph.props;
    if preserved(paragraph) {
        return Err(PaintError::Unexportable("preserved markup"));
    }
    if props.tabs.is_some()
        || props.indent != Indent::default()
        || props.spacing != Spacing::default()
        || props.keep_next.is_some()
        || props.keep_lines.is_some()
        || props.widow_control.is_some()
        || props.page_break_before.is_some()
        || props.snap_to_grid.is_some()
        || props.outline_level.is_some()
    {
        return Err(PaintError::Unexportable(
            "paragraph formatting is outside the exportable subset",
        ));
    }
    if !paragraph.mark_props.is_empty() || !paragraph.inlines().is_empty() {
        return Err(PaintError::Unexportable("paragraph mark or inline objects"));
    }
    for run in paragraph.runs() {
        run_exportable(&run.props)?;
    }
    Ok(())
}

fn preserved(paragraph: &Paragraph) -> bool {
    paragraph.props.preserved.is_some()
        || paragraph.mark_props.preserved.is_some()
        || paragraph
            .runs()
            .iter()
            .any(|run| run.props.preserved.is_some())
        || paragraph
            .inlines()
            .iter()
            .any(|inline| matches!(inline.kind, InlineKind::Preserved(_)))
}

fn run_exportable(props: &RunProps) -> Result<(), PaintError> {
    if props.hidden.is_some()
        || props.preserved.is_some()
        || props.languages != Languages::default()
        || matches!(props.color, Some(Color::Theme(_)))
    {
        return Err(PaintError::Unexportable(
            "character formatting is outside the exportable subset",
        ));
    }
    if props
        .size
        .is_some_and(|size| !matches!(size, Measure::HalfPoints(value) if value >= 0))
    {
        return Err(PaintError::Unexportable("font size"));
    }
    for font in [
        &props.fonts.ascii,
        &props.fonts.h_ansi,
        &props.fonts.east_asia,
        &props.fonts.complex_script,
    ] {
        if matches!(font, Some(FontRef::Theme(_))) {
            return Err(PaintError::Unexportable("theme font"));
        }
    }
    Ok(())
}

fn direct_changes(props: &RunProps) -> Result<Vec<RunChange>, PaintError> {
    run_exportable(props)?;
    if let (Some(FontRef::Named(ascii)), Some(FontRef::Named(hansi))) =
        (&props.fonts.ascii, &props.fonts.h_ansi)
        && ascii != hansi
    {
        return Err(PaintError::Unexportable("ascii and hAnsi fonts differ"));
    }
    let mut changes = Vec::new();
    if let Some(value) = props.bold {
        changes.push(RunChange::Bold(Some(value)));
    }
    if let Some(value) = props.italic {
        changes.push(RunChange::Italic(Some(value)));
    }
    if let Some(value) = props.strike {
        changes.push(RunChange::Strike(Some(value)));
    }
    if let Some(value) = props.underline {
        changes.push(RunChange::Underline(Some(value)));
    }
    if let Some(size) = props.size {
        changes.push(RunChange::Size(Some(size)));
    }
    if let Some(color) = props.color {
        changes.push(RunChange::Color(Some(color)));
    }
    let named = match (&props.fonts.ascii, &props.fonts.h_ansi) {
        (Some(font), _) | (None, Some(font)) => Some(font.clone()),
        _ => None,
    };
    if let Some(font) = named {
        changes.push(RunChange::AsciiFont(Some(font)));
    }
    if let Some(FontRef::Named(name)) = &props.fonts.east_asia {
        changes.push(RunChange::EastAsiaFont(Some(FontRef::Named(name.clone()))));
    }
    Ok(changes)
}

fn normalize(
    document: &Document,
    mut start: Position,
    mut end: Position,
) -> Result<(Position, Position), PaintError> {
    if start.story != end.story {
        return Err(PaintError::CrossContainer);
    }
    let first = located(document, start)?;
    let last = located(document, end)?;
    if first.containers != last.containers {
        return Err(PaintError::CrossContainer);
    }
    if first.content_locked || last.content_locked {
        return Err(PaintError::Locked);
    }
    if first.index > last.index || (first.index == last.index && start.offset > end.offset) {
        std::mem::swap(&mut start, &mut end);
    }
    Ok((start, end))
}

fn ends(
    document: &Document,
    start: Position,
    end: Position,
) -> Result<(Location, Location), PaintError> {
    Ok((located(document, start)?, located(document, end)?))
}

fn located(document: &Document, at: Position) -> Result<Location, PaintError> {
    let location = locate::locate(document, at.story, at.paragraph).ok_or(PaintError::Missing)?;
    if locate::paragraph(document, &location).is_none() {
        return Err(PaintError::Missing);
    }
    let len = locate::paragraph(document, &location)
        .ok_or(PaintError::Missing)?
        .len();
    if at.offset.0 > len {
        return Err(PaintError::Missing);
    }
    Ok(location)
}

fn paragraph_at(document: &Document, at: Position) -> Result<&Paragraph, PaintError> {
    let location = located(document, at)?;
    locate::paragraph(document, &location).ok_or(PaintError::Missing)
}
