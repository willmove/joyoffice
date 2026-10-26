//! Font and paragraph dialogs for T205 slice 2 (PLATFORM-003).
//!
//! A cancelled dialog produces no commands. Apply builds one list of engine
//! commands; the input layer submits that list as a single undoable
//! transaction. Values outside the export subset are refused here, so the
//! transaction is not a formatting change the package writer would drop.
//!
//! Chinese sizes use the Microsoft Office 号→point table recorded by OriginLab
//! FAQ-850 and the Word column of the W3C clreq discussion (二号 is 22 pt, not
//! the historical 21 pt). Half-points are ECMA-376 `ST_HpsMeasure` (1 pt = 2).
//! Single, double and dotted underlines and sRGB colour are written and
//! painted by the page scene. Other underline styles and theme colours stay
//! refused. [`ViewToggles`] is the session copy of the layout view options.

use std::collections::BTreeMap;

use joyword_engine::{Command, ParagraphChange, RunChange, TablePropertyChange};
use joyword_model::{
    BorderEdge, BorderKind, CellBorders, CellMargins, Color, FontRef, Indent, Justification,
    LineSpacing, Measure, NodeId, ParaProps, Position, RevisionView, StoryId, TabAlign, TabLeader,
    TabStop, TableAlign, TableWidth, Underline, indent_edges,
};

use crate::command::{
    ArgValue, CommandId, CommandSpec, FORMAT_FONT, FORMAT_PARAGRAPH, TABLE_PROPERTIES,
    VIEW_FIELD_CODES, VIEW_FORMATTING_MARKS, VIEW_HIDDEN_TEXT,
};

/// Office 号 sizes from 初号 through 八号, including the 小 sizes that sit
/// between them. `points_tenths` is the published point size times ten.
pub struct ChineseSize {
    pub name: &'static str,
    pub points_tenths: i64,
    pub half_points: i64,
}

/// Published Office mapping. Not a claim that every historical foundry used
/// the same points; clreq records those other columns and this table does not.
pub const CHINESE_SIZES: &[ChineseSize] = &[
    ChineseSize {
        name: "初号",
        points_tenths: 420,
        half_points: 84,
    },
    ChineseSize {
        name: "小初",
        points_tenths: 360,
        half_points: 72,
    },
    ChineseSize {
        name: "一号",
        points_tenths: 260,
        half_points: 52,
    },
    ChineseSize {
        name: "小一",
        points_tenths: 240,
        half_points: 48,
    },
    ChineseSize {
        name: "二号",
        points_tenths: 220,
        half_points: 44,
    },
    ChineseSize {
        name: "小二",
        points_tenths: 180,
        half_points: 36,
    },
    ChineseSize {
        name: "三号",
        points_tenths: 160,
        half_points: 32,
    },
    ChineseSize {
        name: "小三",
        points_tenths: 150,
        half_points: 30,
    },
    ChineseSize {
        name: "四号",
        points_tenths: 140,
        half_points: 28,
    },
    ChineseSize {
        name: "小四",
        points_tenths: 120,
        half_points: 24,
    },
    ChineseSize {
        name: "五号",
        points_tenths: 105,
        half_points: 21,
    },
    ChineseSize {
        name: "小五",
        points_tenths: 90,
        half_points: 18,
    },
    ChineseSize {
        name: "六号",
        points_tenths: 75,
        half_points: 15,
    },
    ChineseSize {
        name: "小六",
        points_tenths: 65,
        half_points: 13,
    },
    ChineseSize {
        name: "七号",
        points_tenths: 55,
        half_points: 11,
    },
    ChineseSize {
        name: "八号",
        points_tenths: 50,
        half_points: 10,
    },
];

/// Half-points Word's size box accepts: 1 pt through 1638 pt, in half-points.
const MIN_HALF_POINTS: i64 = 2;
const MAX_HALF_POINTS: i64 = 3_276;
/// Twenty-two inches. Longer direct indents are refused rather than wrapped.
const MAX_TWIPS: i64 = 22 * 1_440;
const MAX_TABS: usize = 48;
const MAX_FONT_NAME: usize = 64;

/// Session view switches. They are not document content. The application
/// maps the flags and the revision projection onto the layout snapshot key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewToggles {
    pub formatting_marks: bool,
    pub show_hidden: bool,
    pub field_codes: bool,
    /// Markup, simple, final or original. Markup is the session default.
    pub revision: RevisionView,
}

impl Default for ViewToggles {
    fn default() -> Self {
        Self {
            formatting_marks: false,
            show_hidden: false,
            field_codes: false,
            revision: RevisionView::FinalWithMarkup,
        }
    }
}

impl ViewToggles {
    pub fn toggled(self, id: CommandId) -> Self {
        let mut next = self;
        match id {
            VIEW_FORMATTING_MARKS => next.formatting_marks = !next.formatting_marks,
            VIEW_HIDDEN_TEXT => next.show_hidden = !next.show_hidden,
            VIEW_FIELD_CODES => next.field_codes = !next.field_codes,
            _ => {}
        }
        next
    }

    pub fn with_review(self, revision: RevisionView) -> Self {
        Self { revision, ..self }
    }
}

pub fn is_dialog_command(id: CommandId) -> bool {
    id == FORMAT_FONT || id == FORMAT_PARAGRAPH || id == TABLE_PROPERTIES
}

/// `Leave` keeps the property. `Set` writes it, including an explicit empty
/// value where the model uses `None` for inherit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Field<T> {
    Leave,
    Set(T),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontDraft {
    pub family: Option<String>,
    pub size_half_points: Option<i64>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<Underline>,
    pub color: Option<Color>,
}

impl FontDraft {
    pub fn leaves_everything(&self) -> bool {
        self.family.is_none()
            && self.size_half_points.is_none()
            && self.bold.is_none()
            && self.italic.is_none()
            && self.underline.is_none()
            && self.color.is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParagraphDraft {
    pub alignment: Field<Option<Justification>>,
    pub indent: Field<Indent>,
    pub spacing: Field<SpacingDraft>,
    pub tabs: Field<Vec<TabStop>>,
}

/// Spacing the dialog can ask the engine to store. Before and after are
/// twips. Line spacing uses the ECMA-376 `line` / `lineRule` pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpacingDraft {
    pub before: Option<i64>,
    pub after: Option<i64>,
    pub line: Option<LineSpacing>,
}

impl ParagraphDraft {
    pub fn leaves_everything(&self) -> bool {
        matches!(self.alignment, Field::Leave)
            && matches!(self.indent, Field::Leave)
            && matches!(self.spacing, Field::Leave)
            && matches!(self.tabs, Field::Leave)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndentMarker {
    FirstLine,
    Body,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RulerDrag {
    Indent {
        marker: IndentMarker,
        position_emu: i64,
        content_width_emu: Option<i64>,
    },
    /// `to_emu` none removes a custom stop. A `from_emu` that is not a custom
    /// stop adds a left stop (the ruler's default stops are not stored).
    Tab { from_emu: i64, to_emu: Option<i64> },
}

/// Registry rows for the dialog and the disabled view switches.
pub fn command_specs() -> Vec<CommandSpec> {
    use crate::command::{MenuKind, MenuPlacement, ParamKind, ParamSpec, Shortcut};
    const FONT_PARAMS: &[ParamSpec] = &[
        ParamSpec {
            name: "commit",
            kind: ParamKind::Text,
            required: true,
        },
        ParamSpec {
            name: "family",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "size",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "bold",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "italic",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "underline",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "color",
            kind: ParamKind::Text,
            required: false,
        },
    ];
    const PARAGRAPH_PARAMS: &[ParamSpec] = &[
        ParamSpec {
            name: "commit",
            kind: ParamKind::Text,
            required: true,
        },
        ParamSpec {
            name: "align",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "start",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "end",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "first",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "hanging",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "before",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "after",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "line",
            kind: ParamKind::Text,
            required: false,
        },
    ];
    let edit = Some(MenuPlacement {
        menu: MenuKind::Edit,
        group: 2,
    });
    vec![
        CommandSpec {
            id: FORMAT_FONT,
            label: "command.format.font",
            menu: edit,
            shortcut: Shortcut::primary("cmd-d", "ctrl-d"),
            modifies_document: true,
            parameters: FONT_PARAMS,
        },
        CommandSpec {
            id: FORMAT_PARAGRAPH,
            label: "command.format.paragraph",
            menu: edit,
            shortcut: Shortcut::NONE,
            modifies_document: true,
            parameters: PARAGRAPH_PARAMS,
        },
        CommandSpec {
            id: VIEW_FORMATTING_MARKS,
            label: "command.view.formatting_marks",
            menu: None,
            shortcut: Shortcut::NONE,
            modifies_document: false,
            parameters: &[],
        },
        CommandSpec {
            id: VIEW_HIDDEN_TEXT,
            label: "command.view.hidden_text",
            menu: None,
            shortcut: Shortcut::NONE,
            modifies_document: false,
            parameters: &[],
        },
        CommandSpec {
            id: VIEW_FIELD_CODES,
            label: "command.view.field_codes",
            menu: None,
            shortcut: Shortcut::NONE,
            modifies_document: false,
            parameters: &[],
        },
        CommandSpec {
            id: TABLE_PROPERTIES,
            label: "command.table.properties",
            menu: Some(MenuPlacement {
                menu: MenuKind::Edit,
                group: 4,
            }),
            shortcut: Shortcut::NONE,
            modifies_document: true,
            parameters: TABLE_PROPERTY_PARAMS,
        },
    ]
}

const TABLE_PROPERTY_PARAMS: &[crate::command::ParamSpec] = {
    use crate::command::{ParamKind, ParamSpec};
    &[
        ParamSpec {
            name: "commit",
            kind: ParamKind::Text,
            required: true,
        },
        ParamSpec {
            name: "width",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "align",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "top",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "left",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "bottom",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "right",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "border",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "border_size",
            kind: ParamKind::Text,
            required: false,
        },
        ParamSpec {
            name: "border_color",
            kind: ParamKind::Text,
            required: false,
        },
    ]
};

pub fn chinese_size(name: &str) -> Option<&'static ChineseSize> {
    CHINESE_SIZES.iter().find(|size| size.name == name)
}

pub fn chinese_name_for_half_points(half_points: i64) -> Option<&'static str> {
    CHINESE_SIZES
        .iter()
        .find(|size| size.half_points == half_points)
        .map(|size| size.name)
}

/// `None` is cancel: the caller must not open a transaction.
pub fn font_draft_from_decision(
    decision: Option<&BTreeMap<&'static str, ArgValue>>,
) -> Result<Option<FontDraft>, &'static str> {
    let Some(args) = decision else {
        return Ok(None);
    };
    parse_font(args).map(Some)
}

pub fn paragraph_draft_from_decision(
    decision: Option<&BTreeMap<&'static str, ArgValue>>,
) -> Result<Option<ParagraphDraft>, &'static str> {
    let Some(args) = decision else {
        return Ok(None);
    };
    parse_paragraph(args).map(Some)
}

pub fn parse_font(args: &BTreeMap<&'static str, ArgValue>) -> Result<FontDraft, &'static str> {
    require_commit(args)?;
    Ok(FontDraft {
        family: optional_family(args.get("family"))?,
        size_half_points: optional_size(args.get("size"))?,
        bold: optional_bool(args.get("bold"), "dialog.font.bold")?,
        italic: optional_bool(args.get("italic"), "dialog.font.italic")?,
        underline: optional_underline(args.get("underline"))?,
        color: optional_color(args.get("color"))?,
    })
}

pub fn parse_paragraph(
    args: &BTreeMap<&'static str, ArgValue>,
) -> Result<ParagraphDraft, &'static str> {
    require_commit(args)?;
    let alignment = if args.contains_key("align") {
        Field::Set(optional_alignment(args.get("align"))?)
    } else {
        Field::Leave
    };
    let indent_keys = ["start", "end", "first", "hanging"]
        .into_iter()
        .any(|name| args.contains_key(name));
    let indent = if indent_keys {
        let indent = Indent {
            start: optional_twips(args.get("start"), true, "dialog.paragraph.indent")?,
            end: optional_twips(args.get("end"), true, "dialog.paragraph.indent")?,
            first_line: optional_twips(args.get("first"), false, "dialog.paragraph.indent")?,
            hanging: optional_twips(args.get("hanging"), false, "dialog.paragraph.indent")?,
        };
        if indent.first_line.is_some() && indent.hanging.is_some() {
            return Err("dialog.paragraph.indent");
        }
        Field::Set(indent)
    } else {
        Field::Leave
    };
    let spacing_keys = ["before", "after", "line"]
        .into_iter()
        .any(|name| args.contains_key(name));
    let spacing = if spacing_keys {
        Field::Set(SpacingDraft {
            before: optional_gap(args.get("before"))?,
            after: optional_gap(args.get("after"))?,
            line: optional_line(args.get("line"))?,
        })
    } else {
        Field::Leave
    };
    Ok(ParagraphDraft {
        alignment,
        indent,
        spacing,
        tabs: Field::Leave,
    })
}

/// Commands for one non-empty range. The note list stays empty: the page
/// scene paints single, double and dotted underlines and sRGB colour.
pub fn font_commands(
    draft: &FontDraft,
    start: Position,
    end: Position,
) -> Result<(Vec<Command>, Vec<&'static str>), &'static str> {
    let mut commands = Vec::new();
    let notes = Vec::new();
    if let Some(family) = &draft.family {
        let font = FontRef::Named(family.as_str().into());
        commands.push(Command::FormatText {
            start,
            end,
            change: RunChange::AsciiFont(Some(font.clone())),
        });
        commands.push(Command::FormatText {
            start,
            end,
            change: RunChange::EastAsiaFont(Some(font)),
        });
    }
    if let Some(size) = draft.size_half_points {
        if !(MIN_HALF_POINTS..=MAX_HALF_POINTS).contains(&size) {
            return Err("dialog.font.size");
        }
        commands.push(Command::FormatText {
            start,
            end,
            change: RunChange::Size(Some(Measure::HalfPoints(size))),
        });
    }
    if let Some(bold) = draft.bold {
        commands.push(Command::FormatText {
            start,
            end,
            change: RunChange::Bold(Some(bold)),
        });
    }
    if let Some(italic) = draft.italic {
        commands.push(Command::FormatText {
            start,
            end,
            change: RunChange::Italic(Some(italic)),
        });
    }
    if let Some(underline) = draft.underline {
        commands.push(Command::FormatText {
            start,
            end,
            change: RunChange::Underline(Some(underline)),
        });
    }
    if let Some(color) = draft.color {
        commands.push(Command::FormatText {
            start,
            end,
            change: RunChange::Color(Some(color)),
        });
    }
    Ok((commands, notes))
}

pub fn paragraph_commands(
    story: StoryId,
    paragraphs: &[(NodeId, ParaProps)],
    draft: &ParagraphDraft,
) -> Result<Vec<Command>, &'static str> {
    validate_paragraph(draft)?;
    let mut commands = Vec::new();
    for (id, current) in paragraphs {
        let alignment = match &draft.alignment {
            Field::Leave => current.justification,
            Field::Set(value) => *value,
        };
        let indent = match &draft.indent {
            Field::Leave => current.indent,
            Field::Set(value) => *value,
        };
        if matches!(
            alignment,
            Some(Justification::Center | Justification::End | Justification::Both)
        ) && indent_edges(&indent).is_some_and(|edges| edges.moves_text())
        {
            return Err("dialog.paragraph.justify_indent");
        }
        if current.snap_to_grid == Some(true)
            && let Field::Set(spacing) = &draft.spacing
            && !matches!(spacing.line, None | Some(LineSpacing::Auto(240)))
        {
            return Err("dialog.paragraph.grid");
        }
        if let Field::Set(value) = draft.alignment {
            commands.push(Command::FormatParagraph {
                story,
                paragraph: *id,
                change: ParagraphChange::Justification(value),
            });
        }
        if let Field::Set(value) = draft.indent {
            commands.push(Command::FormatParagraph {
                story,
                paragraph: *id,
                change: ParagraphChange::Indent(value),
            });
        }
        if let Field::Set(spacing) = &draft.spacing {
            commands.push(Command::FormatParagraph {
                story,
                paragraph: *id,
                change: ParagraphChange::Spacing(spacing_of(spacing)),
            });
        }
        if let Field::Set(tabs) = &draft.tabs {
            commands.push(Command::FormatParagraph {
                story,
                paragraph: *id,
                change: ParagraphChange::Tabs(Some(tabs.clone())),
            });
        }
    }
    Ok(commands)
}

pub fn dragged_indent(
    current: &Indent,
    marker: IndentMarker,
    position_emu: i64,
    content_width_emu: Option<i64>,
) -> Result<Indent, &'static str> {
    let edges = indent_edges(current).ok_or("ruler.drag.unresolved")?;
    let body = edges.start.saturating_add(edges.hanging);
    let first = if edges.first_line == 0 {
        edges.start
    } else {
        edges.start.saturating_add(edges.first_line)
    };
    let end = as_twips(current.end)?;
    let position = position_emu.max(0);
    let (start, first_line, hanging, end) = match marker {
        IndentMarker::FirstLine => {
            if position >= body {
                (
                    Some(Measure::Twips(emu_to_twips(body)?)),
                    Some(Measure::Twips(emu_to_twips(position - body)?)),
                    None,
                    end,
                )
            } else {
                (
                    Some(Measure::Twips(emu_to_twips(position)?)),
                    None,
                    Some(Measure::Twips(emu_to_twips(body - position)?)),
                    end,
                )
            }
        }
        IndentMarker::Body => {
            if position >= first {
                (
                    Some(Measure::Twips(emu_to_twips(first)?)),
                    None,
                    Some(Measure::Twips(emu_to_twips(position - first)?)),
                    end,
                )
            } else {
                (
                    Some(Measure::Twips(emu_to_twips(position)?)),
                    Some(Measure::Twips(emu_to_twips(first - position)?)),
                    None,
                    end,
                )
            }
        }
        IndentMarker::Right => {
            let width = content_width_emu.ok_or("ruler.drag.width")?;
            if position > width {
                return Err("ruler.drag.width");
            }
            (
                as_twips(current.start)?,
                as_twips(current.first_line)?,
                as_twips(current.hanging)?,
                Some(Measure::Twips(emu_to_twips(width - position)?)),
            )
        }
    };
    let indent = Indent {
        start,
        end,
        first_line,
        hanging,
    };
    if indent.first_line.is_some() && indent.hanging.is_some() {
        return Err("dialog.paragraph.indent");
    }
    Ok(indent)
}

/// Rewrites the paragraph's own stops. Inherited style stops are the caller's
/// problem: pass only the direct list, and refuse when the resolved list has
/// a custom stop that is not in it.
pub fn dragged_tabs(
    direct: &[TabStop],
    from_emu: i64,
    to_emu: Option<i64>,
) -> Result<Vec<TabStop>, &'static str> {
    if direct.len() > MAX_TABS {
        return Err("ruler.drag.tab");
    }
    let mut out = Vec::new();
    for stop in direct {
        if stop.align == TabAlign::Clear {
            continue;
        }
        if !tab_writable(stop) {
            return Err("ruler.drag.tab");
        }
        out.push(*stop);
    }
    let index = out.iter().position(|stop| {
        stop.position
            .absolute()
            .is_some_and(|length| length.raw() == from_emu)
    });
    match to_emu {
        None => {
            let index = index.ok_or("ruler.drag.tab")?;
            out.remove(index);
        }
        Some(to) => {
            let position = Measure::Twips(emu_to_twips(to.max(0))?);
            if let Some(index) = index {
                out[index].position = position;
            } else {
                if out.len() == MAX_TABS {
                    return Err("ruler.drag.tab");
                }
                out.push(TabStop {
                    position,
                    align: TabAlign::Left,
                    leader: TabLeader::None,
                });
            }
        }
    }
    Ok(out)
}

pub fn inherited_custom_tabs(direct: &[TabStop], resolved: &[TabStop]) -> bool {
    resolved.iter().any(|stop| {
        stop.align != TabAlign::Clear
            && stop.position.absolute().is_some()
            && !direct.iter().any(|owned| {
                owned.align != TabAlign::Clear
                    && match (owned.position.absolute(), stop.position.absolute()) {
                        (Some(left), Some(right)) => left == right,
                        _ => false,
                    }
            })
    })
}

fn validate_paragraph(draft: &ParagraphDraft) -> Result<(), &'static str> {
    if let Field::Set(Some(Justification::Distribute)) = draft.alignment {
        return Err("dialog.paragraph.distribute");
    }
    if let Field::Set(indent) = draft.indent {
        if indent_edges(&indent).is_none() {
            return Err("dialog.paragraph.indent");
        }
        if indent.first_line.is_some() && indent.hanging.is_some() {
            return Err("dialog.paragraph.indent");
        }
    }
    if let Field::Set(spacing) = &draft.spacing {
        if spacing
            .before
            .is_some_and(|value| !(0..=MAX_TWIPS).contains(&value))
            || spacing
                .after
                .is_some_and(|value| !(0..=MAX_TWIPS).contains(&value))
        {
            return Err("dialog.paragraph.spacing");
        }
        if let Some(line) = spacing.line {
            let accepted = match line {
                LineSpacing::Auto(value) => (1..=MAX_TWIPS).contains(&value),
                // Exact zero cannot hold a baseline. At least zero falls back
                // to the profile leading in layout.
                LineSpacing::Exact(Measure::Twips(value)) => (1..=MAX_TWIPS).contains(&value),
                LineSpacing::AtLeast(Measure::Twips(value)) => (0..=MAX_TWIPS).contains(&value),
                _ => false,
            };
            if !accepted {
                return Err("dialog.paragraph.line");
            }
        }
    }
    if let Field::Set(tabs) = &draft.tabs
        && (tabs.len() > MAX_TABS || tabs.iter().any(|stop| !tab_writable(stop)))
    {
        return Err("ruler.drag.tab");
    }
    Ok(())
}

fn spacing_of(spacing: &SpacingDraft) -> joyword_model::Spacing {
    joyword_model::Spacing {
        before: spacing.before.map(Measure::Twips),
        after: spacing.after.map(Measure::Twips),
        before_auto: None,
        after_auto: None,
        line: spacing.line,
    }
}

fn tab_writable(stop: &TabStop) -> bool {
    matches!(stop.position, Measure::Twips(value) if (0..=MAX_TWIPS).contains(&value))
        && !matches!(stop.align, TabAlign::Bar | TabAlign::Clear)
        && !matches!(stop.leader, TabLeader::MiddleDot | TabLeader::Heavy)
}

fn as_twips(measure: Option<Measure>) -> Result<Option<Measure>, &'static str> {
    match measure {
        None => Ok(None),
        Some(measure) => {
            let length = measure.absolute().ok_or("ruler.drag.unresolved")?;
            Ok(Some(Measure::Twips(emu_to_twips(length.raw())?)))
        }
    }
}

fn emu_to_twips(emu: i64) -> Result<i64, &'static str> {
    let per = Measure::Twips(1)
        .absolute()
        .ok_or("ruler.drag.unresolved")?
        .raw();
    if per <= 0 {
        return Err("ruler.drag.unresolved");
    }
    let half = per / 2;
    let adjusted = if emu >= 0 {
        emu.checked_add(half)
    } else {
        emu.checked_sub(half)
    }
    .ok_or("ruler.drag.unresolved")?;
    let twips = adjusted / per;
    if !(-MAX_TWIPS..=MAX_TWIPS).contains(&twips) {
        return Err("dialog.paragraph.indent");
    }
    Ok(twips)
}

fn require_commit(args: &BTreeMap<&'static str, ArgValue>) -> Result<(), &'static str> {
    match args.get("commit") {
        Some(ArgValue::Text(text)) if text == "apply" => Ok(()),
        _ => Err("dialog.commit"),
    }
}

fn text_arg(value: Option<&ArgValue>) -> Result<Option<&str>, &'static str> {
    match value {
        None => Ok(None),
        Some(ArgValue::Text(text)) => Ok(Some(text)),
        Some(_) => Err("dialog.commit"),
    }
}

fn optional_family(value: Option<&ArgValue>) -> Result<Option<String>, &'static str> {
    let Some(text) = text_arg(value)? else {
        return Ok(None);
    };
    let name = text.trim();
    if name.is_empty() {
        return Ok(None);
    }
    if name.chars().count() > MAX_FONT_NAME
        || name
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '"' | '<' | '>' | '&'))
    {
        return Err("dialog.font.family");
    }
    Ok(Some(name.to_owned()))
}

fn optional_size(value: Option<&ArgValue>) -> Result<Option<i64>, &'static str> {
    let Some(text) = text_arg(value)? else {
        return Ok(None);
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if let Some(size) = chinese_size(text) {
        return Ok(Some(size.half_points));
    }
    let half_points = parse_half_points(text).ok_or("dialog.font.size")?;
    if !(MIN_HALF_POINTS..=MAX_HALF_POINTS).contains(&half_points) {
        return Err("dialog.font.size");
    }
    Ok(Some(half_points))
}

fn parse_half_points(text: &str) -> Option<i64> {
    let (whole, frac) = match text.split_once('.') {
        Some((whole, frac)) => (whole, Some(frac)),
        None => (text, None),
    };
    if whole.is_empty() || !whole.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let points = whole.parse::<i64>().ok()?;
    let extra = match frac {
        None => 0,
        Some("0" | "00") => 0,
        Some("5" | "50") => 1,
        _ => return None,
    };
    points.checked_mul(2)?.checked_add(extra)
}

fn optional_bool(
    value: Option<&ArgValue>,
    error: &'static str,
) -> Result<Option<bool>, &'static str> {
    let Some(text) = text_arg(value)? else {
        return Ok(None);
    };
    match text {
        "true" => Ok(Some(true)),
        "false" => Ok(Some(false)),
        "" => Ok(None),
        _ => Err(error),
    }
}

fn optional_underline(value: Option<&ArgValue>) -> Result<Option<Underline>, &'static str> {
    let Some(text) = text_arg(value)? else {
        return Ok(None);
    };
    if text.is_empty() {
        return Ok(None);
    }
    Ok(Some(match text {
        "none" => Underline::None,
        "single" => Underline::Single,
        "double" => Underline::Double,
        "thick" => Underline::Thick,
        "dotted" => Underline::Dotted,
        "dash" => Underline::Dash,
        "wave" => Underline::Wave,
        "words" => Underline::Words,
        _ => return Err("dialog.font.underline"),
    }))
}

fn optional_color(value: Option<&ArgValue>) -> Result<Option<Color>, &'static str> {
    let Some(text) = text_arg(value)? else {
        return Ok(None);
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if text.eq_ignore_ascii_case("auto") {
        return Ok(Some(Color::Auto));
    }
    if text.len() != 6 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("dialog.font.color");
    }
    let rgb = u32::from_str_radix(text, 16).map_err(|_| "dialog.font.color")?;
    Ok(Some(Color::Rgb(
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
    )))
}

fn optional_alignment(value: Option<&ArgValue>) -> Result<Option<Justification>, &'static str> {
    let Some(text) = text_arg(value)? else {
        return Ok(None);
    };
    if text.is_empty() {
        return Ok(None);
    }
    Ok(Some(match text {
        "start" | "left" => Justification::Start,
        "center" => Justification::Center,
        "end" | "right" => Justification::End,
        "both" => Justification::Both,
        "distribute" => Justification::Distribute,
        _ => return Err("dialog.paragraph.align"),
    }))
}

fn optional_twips(
    value: Option<&ArgValue>,
    allow_negative: bool,
    error: &'static str,
) -> Result<Option<Measure>, &'static str> {
    let Some(text) = text_arg(value)? else {
        return Ok(None);
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let twips = text.parse::<i64>().map_err(|_| error)?;
    let ok = if allow_negative {
        (-MAX_TWIPS..=MAX_TWIPS).contains(&twips)
    } else {
        (0..=MAX_TWIPS).contains(&twips)
    };
    if !ok {
        return Err(error);
    }
    Ok(Some(Measure::Twips(twips)))
}

fn optional_gap(value: Option<&ArgValue>) -> Result<Option<i64>, &'static str> {
    match optional_twips(value, false, "dialog.paragraph.spacing")? {
        None => Ok(None),
        Some(Measure::Twips(twips)) => Ok(Some(twips)),
        Some(_) => Err("dialog.paragraph.spacing"),
    }
}

fn optional_line(value: Option<&ArgValue>) -> Result<Option<LineSpacing>, &'static str> {
    let Some(text) = text_arg(value)? else {
        return Ok(None);
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let (rule, amount) = text.split_once(':').ok_or("dialog.paragraph.line")?;
    let amount = amount.parse::<i64>().map_err(|_| "dialog.paragraph.line")?;
    if !(0..=MAX_TWIPS).contains(&amount) || (rule == "auto" && amount == 0) {
        return Err("dialog.paragraph.line");
    }
    Ok(Some(match rule {
        "auto" => LineSpacing::Auto(amount),
        "exact" => LineSpacing::Exact(Measure::Twips(amount)),
        "atLeast" => LineSpacing::AtLeast(Measure::Twips(amount)),
        _ => return Err("dialog.paragraph.line"),
    }))
}

/// Table properties the dialog can apply. Empty strings leave that field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableDraft {
    pub width: Option<TableWidth>,
    pub alignment: Option<TableAlign>,
    pub cell_margin: Option<CellMargins>,
    pub borders: Option<CellBorders>,
}

impl TableDraft {
    pub fn leaves_everything(&self) -> bool {
        self.width.is_none()
            && self.alignment.is_none()
            && self.cell_margin.is_none()
            && self.borders.is_none()
    }
}

/// `None` is cancel: the caller must not open a transaction.
pub fn table_draft_from_decision(
    decision: Option<&BTreeMap<&'static str, ArgValue>>,
) -> Result<Option<TableDraft>, &'static str> {
    let Some(args) = decision else {
        return Ok(None);
    };
    parse_table_properties(args).map(Some)
}

pub fn parse_table_properties(
    args: &BTreeMap<&'static str, ArgValue>,
) -> Result<TableDraft, &'static str> {
    require_commit(args)?;
    let margin_keys = ["top", "left", "bottom", "right"]
        .into_iter()
        .any(|name| args.contains_key(name));
    Ok(TableDraft {
        width: optional_table_width(args.get("width"))?,
        alignment: optional_table_align(args.get("align"))?,
        cell_margin: if margin_keys {
            Some(CellMargins {
                top: margin_twips(args.get("top"))?,
                left: margin_twips(args.get("left"))?,
                bottom: margin_twips(args.get("bottom"))?,
                right: margin_twips(args.get("right"))?,
            })
        } else {
            None
        },
        borders: optional_table_borders(args)?,
    })
}

/// One `SetTableProperties` command, or an empty list when every field is left.
pub fn table_property_commands(
    draft: &TableDraft,
    at: Position,
) -> Result<Vec<Command>, &'static str> {
    if draft.leaves_everything() {
        return Ok(Vec::new());
    }
    Ok(vec![Command::SetTableProperties {
        at,
        change: TablePropertyChange {
            width: draft.width,
            alignment: draft.alignment,
            cell_margin: draft.cell_margin,
            borders: draft.borders,
        },
    }])
}

fn optional_table_width(value: Option<&ArgValue>) -> Result<Option<TableWidth>, &'static str> {
    let Some(text) = text_arg(value)? else {
        return Ok(None);
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if text == "auto" {
        return Ok(Some(TableWidth::Auto));
    }
    let (kind, amount) = text.split_once(':').ok_or("dialog.table.width")?;
    let amount = amount.parse::<i64>().map_err(|_| "dialog.table.width")?;
    match kind {
        "dxa" if (1..=MAX_TWIPS).contains(&amount) => Ok(Some(TableWidth::Dxa(amount))),
        "pct" if (1..=50_000).contains(&amount) => Ok(Some(TableWidth::Pct(amount))),
        _ => Err("dialog.table.width"),
    }
}

fn optional_table_align(value: Option<&ArgValue>) -> Result<Option<TableAlign>, &'static str> {
    let Some(text) = text_arg(value)? else {
        return Ok(None);
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    match text {
        "left" => Ok(Some(TableAlign::Left)),
        "center" => Ok(Some(TableAlign::Center)),
        "right" => Ok(Some(TableAlign::Right)),
        _ => Err("dialog.table.align"),
    }
}

fn margin_twips(value: Option<&ArgValue>) -> Result<i64, &'static str> {
    match optional_twips(value, false, "dialog.table.margin")? {
        None => Ok(0),
        Some(Measure::Twips(twips)) => Ok(twips),
        Some(_) => Err("dialog.table.margin"),
    }
}

fn optional_table_borders(
    args: &BTreeMap<&'static str, ArgValue>,
) -> Result<Option<CellBorders>, &'static str> {
    let Some(text) = text_arg(args.get("border"))? else {
        if args.contains_key("border_size") || args.contains_key("border_color") {
            return Err("dialog.table.border");
        }
        return Ok(None);
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let kind = match text {
        "nil" => BorderKind::Nil,
        "single" => BorderKind::Single,
        "double" => BorderKind::Double,
        "dotted" => BorderKind::Dotted,
        "dashed" => BorderKind::Dashed,
        "thick" => BorderKind::Thick,
        _ => return Err("dialog.table.border"),
    };
    let size = match text_arg(args.get("border_size"))? {
        None => 4u32,
        Some(text) if text.trim().is_empty() => 4,
        Some(text) => text
            .trim()
            .parse::<u32>()
            .map_err(|_| "dialog.table.border")?,
    };
    if kind == BorderKind::Nil {
        if size > 255 {
            return Err("dialog.table.border");
        }
    } else if !(1..=255).contains(&size) {
        return Err("dialog.table.border");
    }
    let color = match optional_color(args.get("border_color"))? {
        Some(color) => color,
        None => Color::Auto,
    };
    let edge = BorderEdge {
        kind,
        size_eighths: size,
        color,
    };
    Ok(Some(CellBorders {
        top: Some(edge),
        left: Some(edge),
        bottom: Some(edge),
        right: Some(edge),
    }))
}
