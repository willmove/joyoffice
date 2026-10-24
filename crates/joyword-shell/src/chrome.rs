//! Ribbon, ruler and status bar (PLATFORM-003, EDIT-009).
//!
//! The ribbon lists commands the registry already implements. A command that
//! is not registered is omitted, so an unfinished feature cannot appear as a
//! control. Buttons call [`crate::command::Registry::invoke_with`] with the
//! arguments [`RibbonControl::bind`] builds. Font and paragraph dialogs send
//! no arguments, so invoke asks for `commit` and the window opens the dialog.
//! Formatting marks, hidden text and field codes stay on the ribbon and stay
//! disabled: the page scene has no view option for them.
//!
//! Icons are original 16×16 cell drawings in this file. They are not copied
//! from Office, WPS or an icon font. [`TextCensus`] counts Han, kana and
//! Hangul per character, Latin letter runs as words, and punctuation and
//! spaces separately. That is JoyWord's reading of EDIT-009, not a Word
//! word-count dialog comparison.

use unicode_segmentation::UnicodeSegmentation;

use joyword_model::{Indent, Measure, OBJECT_MARKER, TabAlign, TabStop, indent_edges};

use crate::command::{
    ArgValue, CommandId, EDIT_ALIGN_CENTER, EDIT_ALIGN_LEFT, EDIT_ALIGN_RIGHT, EDIT_BOLD,
    EDIT_COPY, EDIT_CUT, EDIT_ITALIC, EDIT_JUSTIFY, EDIT_PASTE, EDIT_REDO, EDIT_UNDERLINE,
    EDIT_UNDO, FIND_NEXT, FIND_PREVIOUS, FIND_REPLACE, FIND_REPLACE_ALL, FORMAT_CANCEL,
    FORMAT_COPY, FORMAT_FONT, FORMAT_PAINT, FORMAT_PARAGRAPH, LIST_APPLY, LIST_LEVEL, LIST_REMOVE,
    PAGE_BREAK, PAGE_EDIT_BODY, PAGE_EDIT_FOOTER, PAGE_EDIT_HEADER, PAGE_MARGINS, PAGE_ORIENTATION,
    PAGE_SIZE, REVIEW_ACCEPT, REVIEW_ACCEPT_ALL, REVIEW_FINAL, REVIEW_MARKUP, REVIEW_ORIGINAL,
    REVIEW_REJECT, REVIEW_REJECT_ALL, REVIEW_TRACK, STYLE_LIST, SWITCH_LANGUAGE, TABLE_INSERT,
    TABLE_PROPERTIES, VIEW_FIELD_CODES, VIEW_FORMATTING_MARKS, VIEW_HIDDEN_TEXT, ZOOM_IN, ZOOM_OUT,
    ZOOM_RESET,
};
use crate::l10n::Catalog;

/// `w:defaultTabStop` when the settings omit it. ECMA-376 Part 1 §17.15.1.25.
pub const DEFAULT_TAB_TWIPS: i64 = 720;
/// US Letter, 8.5 by 11 inches, in twips. A paper size, not a Word template.
pub const LETTER_WIDTH_TWIPS: i64 = 12_240;
pub const LETTER_HEIGHT_TWIPS: i64 = 15_840;
/// Half-inch margins. A JoyWord preset, not a claimed Word "Narrow" template.
pub const NARROW_MARGIN_TWIPS: i64 = 720;
pub const MAX_RULER_TABS: usize = 48;
/// Nominal ruler span when the page box is not known yet: six inches.
const NOMINAL_RULER_EMU: i64 = 914_400 * 6;

/// Evidence behind a shown control. The feature catalog counts every level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Evidence {
    Exists,
    Unit,
    Corpus,
    Reference,
}

impl Evidence {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exists => "exists",
            Self::Unit => "unit",
            Self::Corpus => "corpus",
            Self::Reference => "reference",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RibbonTab {
    #[default]
    Start,
    Insert,
    Layout,
    Review,
    View,
}

impl RibbonTab {
    pub const ALL: [Self; 5] = [
        Self::Start,
        Self::Insert,
        Self::Layout,
        Self::Review,
        Self::View,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Insert => "insert",
            Self::Layout => "layout",
            Self::Review => "review",
            Self::View => "view",
        }
    }

    pub const fn label_key(self) -> &'static str {
        match self {
            Self::Start => "ribbon.tab.start",
            Self::Insert => "ribbon.tab.insert",
            Self::Layout => "ribbon.tab.layout",
            Self::Review => "ribbon.tab.review",
            Self::View => "ribbon.tab.view",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresetArg {
    pub name: &'static str,
    pub value: PresetValue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetValue {
    Text(&'static str),
    Bool(bool),
}

/// How a ribbon control fills command parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RibbonArgs {
    None,
    Fixed(&'static [PresetArg]),
    /// Find or replace. An empty query produces no arguments, so invoke asks
    /// for input and the window focuses the find field.
    Find {
        replacement: bool,
    },
    FormatCopy,
    /// The window asks for a file path, then invokes the command.
    PromptPath,
    /// Opens a dialog. Bind sends no arguments, so a required `commit`
    /// parameter stays pending until the dialog applies.
    Dialog,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RibbonControl {
    pub id: &'static str,
    pub tab: RibbonTab,
    pub group_key: &'static str,
    pub command: CommandId,
    pub label: &'static str,
    pub icon: &'static str,
    pub args: RibbonArgs,
    pub requirement: &'static str,
    pub evidence: Evidence,
}

/// Values the ribbon reads from the window when it binds a control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RibbonInput<'a> {
    pub query: &'a str,
    pub replacement: &'a str,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub format_repeat: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RibbonCall {
    pub command: CommandId,
    pub args: Vec<(&'static str, ArgValue)>,
}

const BULLET_LIST: &[PresetArg] = &[PresetArg {
    name: "bullet",
    value: PresetValue::Text("1"),
}];
const TABLE_TWO_BY_TWO: &[PresetArg] = &[
    PresetArg {
        name: "rows",
        value: PresetValue::Text("2"),
    },
    PresetArg {
        name: "columns",
        value: PresetValue::Text("2"),
    },
];
const LEVEL_ONE: &[PresetArg] = &[PresetArg {
    name: "level",
    value: PresetValue::Text("0"),
}];
const LEVEL_TWO: &[PresetArg] = &[PresetArg {
    name: "level",
    value: PresetValue::Text("1"),
}];
const A4_SIZE: &[PresetArg] = &[
    PresetArg {
        name: "width",
        value: PresetValue::Text("11906"),
    },
    PresetArg {
        name: "height",
        value: PresetValue::Text("16838"),
    },
];
const LETTER_SIZE: &[PresetArg] = &[
    PresetArg {
        name: "width",
        value: PresetValue::Text("12240"),
    },
    PresetArg {
        name: "height",
        value: PresetValue::Text("15840"),
    },
];
const NORMAL_MARGINS: &[PresetArg] = &[
    PresetArg {
        name: "top",
        value: PresetValue::Text("1440"),
    },
    PresetArg {
        name: "right",
        value: PresetValue::Text("1800"),
    },
    PresetArg {
        name: "bottom",
        value: PresetValue::Text("1440"),
    },
    PresetArg {
        name: "left",
        value: PresetValue::Text("1800"),
    },
];
const NARROW_MARGINS: &[PresetArg] = &[
    PresetArg {
        name: "top",
        value: PresetValue::Text("720"),
    },
    PresetArg {
        name: "right",
        value: PresetValue::Text("720"),
    },
    PresetArg {
        name: "bottom",
        value: PresetValue::Text("720"),
    },
    PresetArg {
        name: "left",
        value: PresetValue::Text("720"),
    },
];
const PORTRAIT: &[PresetArg] = &[PresetArg {
    name: "orientation",
    value: PresetValue::Text("portrait"),
}];
const LANDSCAPE: &[PresetArg] = &[PresetArg {
    name: "orientation",
    value: PresetValue::Text("landscape"),
}];

/// Controls the Start, Insert, Layout and View ribbon can show.
pub const RIBBON: &[RibbonControl] = &[
    RibbonControl {
        id: "start.cut",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.clipboard",
        command: EDIT_CUT,
        label: "command.edit.cut",
        icon: "cut",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.copy",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.clipboard",
        command: EDIT_COPY,
        label: "command.edit.copy",
        icon: "copy",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.paste",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.clipboard",
        command: EDIT_PASTE,
        label: "command.edit.paste",
        icon: "paste",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.undo",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.history",
        command: EDIT_UNDO,
        label: "command.edit.undo",
        icon: "undo",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.redo",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.history",
        command: EDIT_REDO,
        label: "command.edit.redo",
        icon: "redo",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.bold",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.font",
        command: EDIT_BOLD,
        label: "command.edit.bold",
        icon: "bold",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.italic",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.font",
        command: EDIT_ITALIC,
        label: "command.edit.italic",
        icon: "italic",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.underline",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.font",
        command: EDIT_UNDERLINE,
        label: "command.edit.underline",
        icon: "underline",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.font",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.font",
        command: FORMAT_FONT,
        label: "command.format.font",
        icon: "font",
        args: RibbonArgs::Dialog,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.align_left",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.paragraph",
        command: EDIT_ALIGN_LEFT,
        label: "command.edit.align_left",
        icon: "align-left",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.align",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.paragraph",
        command: EDIT_ALIGN_CENTER,
        label: "command.edit.align_center",
        icon: "align-center",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.align_right",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.paragraph",
        command: EDIT_ALIGN_RIGHT,
        label: "command.edit.align_right",
        icon: "align-right",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.justify",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.paragraph",
        command: EDIT_JUSTIFY,
        label: "command.edit.justify",
        icon: "align-justify",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.paragraph",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.paragraph",
        command: FORMAT_PARAGRAPH,
        label: "command.format.paragraph",
        icon: "paragraph",
        args: RibbonArgs::Dialog,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.styles",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.styles",
        command: STYLE_LIST,
        label: "command.style.list",
        icon: "styles",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.list_bullet",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.list",
        command: LIST_APPLY,
        label: "ribbon.list.bullet",
        icon: "list-bullet",
        args: RibbonArgs::Fixed(BULLET_LIST),
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.list_remove",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.list",
        command: LIST_REMOVE,
        label: "command.list.remove",
        icon: "list-remove",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.list_level_1",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.list",
        command: LIST_LEVEL,
        label: "ribbon.list.level_1",
        icon: "list-level",
        args: RibbonArgs::Fixed(LEVEL_ONE),
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.list_level_2",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.list",
        command: LIST_LEVEL,
        label: "ribbon.list.level_2",
        icon: "list-level",
        args: RibbonArgs::Fixed(LEVEL_TWO),
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.find_next",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.find",
        command: FIND_NEXT,
        label: "command.find.next",
        icon: "find",
        args: RibbonArgs::Find { replacement: false },
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.find_previous",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.find",
        command: FIND_PREVIOUS,
        label: "command.find.previous",
        icon: "find-previous",
        args: RibbonArgs::Find { replacement: false },
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.replace",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.find",
        command: FIND_REPLACE,
        label: "command.find.replace",
        icon: "replace",
        args: RibbonArgs::Find { replacement: true },
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.replace_all",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.find",
        command: FIND_REPLACE_ALL,
        label: "command.find.replace_all",
        icon: "replace-all",
        args: RibbonArgs::Find { replacement: true },
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.format_copy",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.painter",
        command: FORMAT_COPY,
        label: "command.format.copy",
        icon: "painter",
        args: RibbonArgs::FormatCopy,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.format_paint",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.painter",
        command: FORMAT_PAINT,
        label: "command.format.paint",
        icon: "paint",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "start.format_cancel",
        tab: RibbonTab::Start,
        group_key: "ribbon.group.painter",
        command: FORMAT_CANCEL,
        label: "command.format.cancel",
        icon: "cancel",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "insert.picture",
        tab: RibbonTab::Insert,
        group_key: "ribbon.group.illustrations",
        command: crate::command::EDIT_INSERT_IMAGE,
        label: "command.edit.insert_image",
        icon: "picture",
        args: RibbonArgs::PromptPath,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "insert.page_break",
        tab: RibbonTab::Insert,
        group_key: "ribbon.group.pages",
        command: PAGE_BREAK,
        label: "command.page.break",
        icon: "page-break",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "insert.table_grid",
        tab: RibbonTab::Insert,
        group_key: "ribbon.group.tables",
        command: TABLE_INSERT,
        label: "command.table.insert",
        icon: "table",
        args: RibbonArgs::Fixed(TABLE_TWO_BY_TWO),
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "insert.table",
        tab: RibbonTab::Insert,
        group_key: "ribbon.group.tables",
        command: TABLE_PROPERTIES,
        label: "command.table.properties",
        icon: "table",
        args: RibbonArgs::Dialog,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "layout.a4",
        tab: RibbonTab::Layout,
        group_key: "ribbon.group.page",
        command: PAGE_SIZE,
        label: "ribbon.page.a4",
        icon: "page-a4",
        args: RibbonArgs::Fixed(A4_SIZE),
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "layout.letter",
        tab: RibbonTab::Layout,
        group_key: "ribbon.group.page",
        command: PAGE_SIZE,
        label: "ribbon.page.letter",
        icon: "page-letter",
        args: RibbonArgs::Fixed(LETTER_SIZE),
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "layout.margins",
        tab: RibbonTab::Layout,
        group_key: "ribbon.group.page",
        command: PAGE_MARGINS,
        label: "ribbon.margins.normal",
        icon: "margins",
        args: RibbonArgs::Fixed(NORMAL_MARGINS),
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "layout.margins_narrow",
        tab: RibbonTab::Layout,
        group_key: "ribbon.group.page",
        command: PAGE_MARGINS,
        label: "ribbon.margins.narrow",
        icon: "margins-narrow",
        args: RibbonArgs::Fixed(NARROW_MARGINS),
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "layout.portrait",
        tab: RibbonTab::Layout,
        group_key: "ribbon.group.page",
        command: PAGE_ORIENTATION,
        label: "ribbon.orientation.portrait",
        icon: "portrait",
        args: RibbonArgs::Fixed(PORTRAIT),
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "layout.landscape",
        tab: RibbonTab::Layout,
        group_key: "ribbon.group.page",
        command: PAGE_ORIENTATION,
        label: "ribbon.orientation.landscape",
        icon: "landscape",
        args: RibbonArgs::Fixed(LANDSCAPE),
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "layout.edit_header",
        tab: RibbonTab::Layout,
        group_key: "ribbon.group.page",
        command: PAGE_EDIT_HEADER,
        label: "ribbon.page.edit_header",
        icon: "header",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "layout.edit_footer",
        tab: RibbonTab::Layout,
        group_key: "ribbon.group.page",
        command: PAGE_EDIT_FOOTER,
        label: "ribbon.page.edit_footer",
        icon: "footer",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "layout.edit_body",
        tab: RibbonTab::Layout,
        group_key: "ribbon.group.page",
        command: PAGE_EDIT_BODY,
        label: "ribbon.page.edit_body",
        icon: "body",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "view.zoom_in",
        tab: RibbonTab::View,
        group_key: "ribbon.group.zoom",
        command: ZOOM_IN,
        label: "command.view.zoom_in",
        icon: "zoom-in",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "view.zoom_out",
        tab: RibbonTab::View,
        group_key: "ribbon.group.zoom",
        command: ZOOM_OUT,
        label: "command.view.zoom_out",
        icon: "zoom-out",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "view.zoom_reset",
        tab: RibbonTab::View,
        group_key: "ribbon.group.zoom",
        command: ZOOM_RESET,
        label: "command.view.zoom_reset",
        icon: "zoom-reset",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "view.language",
        tab: RibbonTab::View,
        group_key: "ribbon.group.language",
        command: SWITCH_LANGUAGE,
        label: "command.view.switch_language",
        icon: "language",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "view.formatting_marks",
        tab: RibbonTab::View,
        group_key: "ribbon.group.marks",
        command: VIEW_FORMATTING_MARKS,
        label: "command.view.formatting_marks",
        icon: "marks",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "view.hidden_text",
        tab: RibbonTab::View,
        group_key: "ribbon.group.marks",
        command: VIEW_HIDDEN_TEXT,
        label: "command.view.hidden_text",
        icon: "hidden",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "view.field_codes",
        tab: RibbonTab::View,
        group_key: "ribbon.group.marks",
        command: VIEW_FIELD_CODES,
        label: "command.view.field_codes",
        icon: "field-code",
        args: RibbonArgs::None,
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "review.track",
        tab: RibbonTab::Review,
        group_key: "ribbon.group.tracking",
        command: REVIEW_TRACK,
        label: "command.review.track",
        icon: "review-track",
        args: RibbonArgs::None,
        requirement: "REVIEW-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "review.accept",
        tab: RibbonTab::Review,
        group_key: "ribbon.group.changes",
        command: REVIEW_ACCEPT,
        label: "command.review.accept",
        icon: "review-accept",
        args: RibbonArgs::None,
        requirement: "REVIEW-004",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "review.reject",
        tab: RibbonTab::Review,
        group_key: "ribbon.group.changes",
        command: REVIEW_REJECT,
        label: "command.review.reject",
        icon: "review-reject",
        args: RibbonArgs::None,
        requirement: "REVIEW-004",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "review.accept_all",
        tab: RibbonTab::Review,
        group_key: "ribbon.group.changes",
        command: REVIEW_ACCEPT_ALL,
        label: "command.review.accept_all",
        icon: "review-accept",
        args: RibbonArgs::None,
        requirement: "REVIEW-004",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "review.reject_all",
        tab: RibbonTab::Review,
        group_key: "ribbon.group.changes",
        command: REVIEW_REJECT_ALL,
        label: "command.review.reject_all",
        icon: "review-reject",
        args: RibbonArgs::None,
        requirement: "REVIEW-004",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "review.markup",
        tab: RibbonTab::Review,
        group_key: "ribbon.group.review_view",
        command: REVIEW_MARKUP,
        label: "command.review.markup",
        icon: "review-markup",
        args: RibbonArgs::None,
        requirement: "REVIEW-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "review.final",
        tab: RibbonTab::Review,
        group_key: "ribbon.group.review_view",
        command: REVIEW_FINAL,
        label: "command.review.final",
        icon: "review-final",
        args: RibbonArgs::None,
        requirement: "REVIEW-003",
        evidence: Evidence::Unit,
    },
    RibbonControl {
        id: "review.original",
        tab: RibbonTab::Review,
        group_key: "ribbon.group.review_view",
        command: REVIEW_ORIGINAL,
        label: "command.review.original",
        icon: "review-original",
        args: RibbonArgs::None,
        requirement: "REVIEW-003",
        evidence: Evidence::Unit,
    },
];

/// Status and ruler surfaces. They are not commands.
pub const SURFACES: &[ChromeSurface] = &[
    ChromeSurface {
        id: "status.page",
        kind: "status",
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    ChromeSurface {
        id: "status.words",
        kind: "status",
        requirement: "EDIT-009",
        evidence: Evidence::Unit,
    },
    ChromeSurface {
        id: "status.graphemes",
        kind: "status",
        requirement: "EDIT-009",
        evidence: Evidence::Unit,
    },
    ChromeSurface {
        id: "status.review",
        kind: "status",
        requirement: "REVIEW-003",
        evidence: Evidence::Unit,
    },
    ChromeSurface {
        id: "ruler.indent",
        kind: "ruler",
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
    ChromeSurface {
        id: "ruler.tabs",
        kind: "ruler",
        requirement: "PLATFORM-003",
        evidence: Evidence::Unit,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChromeSurface {
    pub id: &'static str,
    pub kind: &'static str,
    pub requirement: &'static str,
    pub evidence: Evidence,
}

/// Catalog floor. It tracks the shown controls; lowering it is a coverage regression.
pub const FEATURE_FLOOR: usize = RIBBON.len() + SURFACES.len();

impl RibbonControl {
    /// Arguments the window passes to the registry for this control.
    pub fn bind(&self, input: RibbonInput<'_>) -> RibbonCall {
        let args = match self.args {
            RibbonArgs::None => Vec::new(),
            RibbonArgs::Fixed(pairs) => pairs
                .iter()
                .map(|pair| {
                    let value = match pair.value {
                        PresetValue::Text(text) => ArgValue::Text(text.to_owned()),
                        PresetValue::Bool(value) => ArgValue::Bool(value),
                    };
                    (pair.name, value)
                })
                .collect(),
            RibbonArgs::Find { replacement } => find_args(input, replacement),
            RibbonArgs::Dialog => Vec::new(),
            RibbonArgs::FormatCopy => {
                vec![("repeat", ArgValue::Bool(input.format_repeat))]
            }
            RibbonArgs::PromptPath => Vec::new(),
        };
        RibbonCall {
            command: self.command,
            args,
        }
    }
}

fn find_args(input: RibbonInput<'_>, replacement: bool) -> Vec<(&'static str, ArgValue)> {
    if input.query.trim().is_empty() {
        return Vec::new();
    }
    let mut args = vec![
        ("query", ArgValue::Text(input.query.to_owned())),
        ("case", ArgValue::Bool(input.case_sensitive)),
        ("word", ArgValue::Bool(input.whole_word)),
    ];
    if replacement {
        args.push(("replacement", ArgValue::Text(input.replacement.to_owned())));
    }
    args
}

/// Ribbon controls whose commands are actually registered.
pub fn visible_controls<'a>(
    registry: &'a crate::command::Registry,
    controls: &'a [RibbonControl],
) -> Vec<&'a RibbonControl> {
    controls
        .iter()
        .filter(|control| registry.get(control.command).is_some())
        .collect()
}

pub fn controls_on<'a>(
    tab: RibbonTab,
    controls: impl IntoIterator<Item = &'a RibbonControl>,
) -> Vec<&'a RibbonControl> {
    controls
        .into_iter()
        .filter(|control| control.tab == tab)
        .collect()
}

/// Groups in ribbon order. Adjacent controls that share a group stay together.
pub fn groups<'a>(controls: &'a [&RibbonControl]) -> Vec<(&'static str, Vec<&'a RibbonControl>)> {
    let mut grouped: Vec<(&str, Vec<&RibbonControl>)> = Vec::new();
    for control in controls {
        if grouped
            .last()
            .is_none_or(|(key, _)| *key != control.group_key)
        {
            grouped.push((control.group_key, Vec::new()));
        }
        if let Some((_, items)) = grouped.last_mut() {
            items.push(control);
        }
    }
    grouped
}

/// Body text broken into the EDIT-009 buckets, plus grapheme clusters.
///
/// Han (including extensions and compatibility ideographs), kana and Hangul
/// each count as one word. A run of Latin letters is one word; an apostrophe
/// between letters stays inside the word. A run of decimal digits is one
/// word, and `.` or `,` between digits stays inside it. Punctuation and
/// spaces are counted on their own and are not words. The object marker used
/// for a page break is skipped. This is not a Word dialog comparison.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextCensus {
    pub cjk: u64,
    pub latin_words: u64,
    pub numbers: u64,
    pub punctuation: u64,
    pub spaces: u64,
    pub graphemes: u64,
    pub other: u64,
}

impl TextCensus {
    /// CJK characters plus Latin words plus numbers. Punctuation and spaces stay out.
    pub fn words(self) -> u64 {
        self.cjk
            .saturating_add(self.latin_words)
            .saturating_add(self.numbers)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WordRun {
    None,
    Latin,
    Number,
}

pub fn census(text: &str) -> TextCensus {
    let mut out = TextCensus::default();
    let mut run = WordRun::None;
    let mut graphemes = text.graphemes(true).peekable();
    while let Some(grapheme) = graphemes.next() {
        if grapheme.chars().eq([OBJECT_MARKER]) {
            close_run(&mut run, &mut out);
            continue;
        }
        out.graphemes = out.graphemes.saturating_add(1);
        if grapheme.chars().all(char::is_whitespace) {
            close_run(&mut run, &mut out);
            out.spaces = out.spaces.saturating_add(1);
            continue;
        }
        let Some(base) = grapheme.chars().next() else {
            continue;
        };
        if is_east_asian_letter(base) {
            close_run(&mut run, &mut out);
            out.cjk = out.cjk.saturating_add(1);
            continue;
        }
        if is_apostrophe(grapheme) && run == WordRun::Latin && next_is_latin(graphemes.peek()) {
            continue;
        }
        if is_number_separator(grapheme)
            && run == WordRun::Number
            && next_is_digit(graphemes.peek())
        {
            continue;
        }
        if base.is_alphabetic() {
            if run == WordRun::Number {
                close_run(&mut run, &mut out);
            }
            run = WordRun::Latin;
            continue;
        }
        if is_decimal_digit(base) {
            if run == WordRun::Latin {
                close_run(&mut run, &mut out);
            }
            run = WordRun::Number;
            continue;
        }
        close_run(&mut run, &mut out);
        if is_punctuation(base) {
            out.punctuation = out.punctuation.saturating_add(1);
        } else {
            out.other = out.other.saturating_add(1);
        }
    }
    close_run(&mut run, &mut out);
    out
}

fn close_run(run: &mut WordRun, out: &mut TextCensus) {
    match *run {
        WordRun::Latin => out.latin_words = out.latin_words.saturating_add(1),
        WordRun::Number => out.numbers = out.numbers.saturating_add(1),
        WordRun::None => {}
    }
    *run = WordRun::None;
}

fn next_is_latin(next: Option<&&str>) -> bool {
    next.and_then(|grapheme| grapheme.chars().next())
        .is_some_and(|ch| ch.is_alphabetic() && !is_east_asian_letter(ch))
}

fn next_is_digit(next: Option<&&str>) -> bool {
    next.and_then(|grapheme| grapheme.chars().next())
        .is_some_and(is_decimal_digit)
}

fn is_apostrophe(grapheme: &str) -> bool {
    matches!(grapheme, "'" | "’")
}

fn is_number_separator(grapheme: &str) -> bool {
    matches!(grapheme, "." | ",")
}

fn is_decimal_digit(ch: char) -> bool {
    ch.is_ascii_digit() || ('\u{FF10}'..='\u{FF19}').contains(&ch)
}

fn is_east_asian_letter(ch: char) -> bool {
    let code = u32::from(ch);
    matches!(
        code,
        0x1100..=0x11FF
            | 0x2E80..=0x2EFF
            | 0x2F00..=0x2FDF
            | 0x3040..=0x30FF
            | 0x31F0..=0x31FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xAC00..=0xD7AF
            | 0xF900..=0xFAFF
            | 0xFF66..=0xFF9D
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x2CEB0..=0x2EBEF
            | 0x30000..=0x3134F
            | 0x31350..=0x323AF
    )
}

fn is_punctuation(ch: char) -> bool {
    if ch.is_ascii_punctuation() {
        return true;
    }
    matches!(
        ch,
        '，' | '。' | '、' | '；' | '：' | '？' | '！'
            | '“' | '”' | '‘' | '’'
            | '（' | '）'
            | '《' | '》'
            | '〈' | '〉'
            | '【' | '】'
            | '—' | '…' | '·' | '～'
            | '「' | '」'
            | '『' | '』'
            | '\u{FF01}'..='\u{FF0F}'
            | '\u{FF1A}'..='\u{FF20}'
            | '\u{FF3B}'..='\u{FF40}'
            | '\u{FF5B}'..='\u{FF65}'
            | '\u{2010}'..='\u{2027}'
            | '\u{3001}'..='\u{303F}'
    )
}

/// 1-based page and page count for the status bar. `index` is the 0-based
/// caret page from the current layout. A missing caret still reports the count.
pub fn display_page(index: Option<usize>, count: usize) -> (Option<u32>, Option<u32>) {
    let pages = u32::try_from(count).ok().filter(|pages| *pages > 0);
    let page = match (index, pages) {
        (Some(index), Some(pages)) => u32::try_from(index)
            .ok()
            .map(|index| index.saturating_add(1).min(pages)),
        _ => None,
    };
    (page, pages)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusSnapshot {
    pub page: Option<u32>,
    pub pages: Option<u32>,
    pub census: TextCensus,
    /// `markup`, `final` or `original`.
    pub review: &'static str,
    pub tracking: bool,
}

pub fn status_line(catalog: &Catalog, status: &StatusSnapshot) -> String {
    let page = match (status.page, status.pages) {
        (Some(page), Some(pages)) => catalog.format(
            "status.page",
            &[("page", &page.to_string()), ("pages", &pages.to_string())],
        ),
        (None, Some(pages)) => catalog.format("status.pages", &[("pages", &pages.to_string())]),
        _ => catalog.text("status.page.unknown"),
    };
    let census = status.census;
    let words = census.words().to_string();
    let graphemes = census.graphemes.to_string();
    let cjk = census.cjk.to_string();
    let latin = census.latin_words.to_string();
    let numbers = census.numbers.to_string();
    let punctuation = census.punctuation.to_string();
    let spaces = census.spaces.to_string();
    let counts = catalog.format(
        "status.counts",
        &[
            ("words", &words),
            ("graphemes", &graphemes),
            ("cjk", &cjk),
            ("latin", &latin),
            ("numbers", &numbers),
            ("punctuation", &punctuation),
            ("spaces", &spaces),
        ],
    );
    let view_key = match status.review {
        "final" => "status.review.final",
        "original" => "status.review.original",
        _ => "status.review.markup",
    };
    let tracking_key = if status.tracking {
        "status.tracking.on"
    } else {
        "status.tracking.off"
    };
    let view = catalog.text(view_key);
    let tracking = catalog.text(tracking_key);
    let review = catalog.format(
        "status.review",
        &[("view", view.as_str()), ("tracking", tracking.as_str())],
    );
    format!("{page} · {counts} · {review}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RulerTab {
    pub position: i64,
    pub align: TabAlign,
    pub custom: bool,
}

/// Indent and tab marks in EMU from the content left. `right` is also from
/// the content left. Relative units stay unset and raise `unresolved`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RulerModel {
    pub content_width: Option<i64>,
    pub first_line: Option<i64>,
    pub left: Option<i64>,
    pub right: Option<i64>,
    pub tabs: Vec<RulerTab>,
    pub unresolved: bool,
}

/// The painter's horizontal scale. Unknown page boxes use six inches so an
/// indent still has a place to sit; default tabs are not invented in that case.
pub fn ruler_scale(model: &RulerModel) -> i64 {
    model.content_width.unwrap_or(NOMINAL_RULER_EMU).max(1)
}

pub fn default_tab_interval(measure: Option<Measure>) -> (i64, bool) {
    let fallback = emu_from_twips(DEFAULT_TAB_TWIPS).unwrap_or(0);
    match measure {
        None => (fallback, false),
        Some(measure) => match measure.absolute() {
            Some(length) if length.raw() > 0 => (length.raw(), false),
            _ => (fallback, true),
        },
    }
}

fn emu_from_twips(twips: i64) -> Option<i64> {
    Measure::Twips(twips).absolute().map(|length| length.raw())
}

/// Places the current paragraph's indent and tabs.
///
/// `w:ind` edges follow ECMA-376 Part 1 §17.3.1.12 through [`indent_edges`].
/// A hanging indent moves the body marker; a first-line indent moves the
/// first-line marker. Both values, when stored together, are shown as stored.
/// Custom tabs use absolute EMU. Default stops continue from
/// [`DEFAULT_TAB_TWIPS`] (or the document's `w:defaultTabStop`) out to the
/// content width, matching the placement in [`joyword_model::tab_positions`].
pub fn ruler_model(
    indent: &Indent,
    tabs: &[TabStop],
    default_interval: i64,
    content_width: Option<i64>,
) -> RulerModel {
    let mut model = RulerModel {
        content_width,
        ..RulerModel::default()
    };
    match indent_edges(indent) {
        Some(edges) => {
            model.left = Some(edges.start.saturating_add(edges.hanging));
            model.first_line = Some(if edges.first_line == 0 {
                edges.start
            } else {
                edges.start.saturating_add(edges.first_line)
            });
            if let Some(width) = content_width {
                model.right = Some(width.saturating_sub(edges.end));
            }
        }
        None => model.unresolved = true,
    }
    for stop in tabs {
        if stop.align == TabAlign::Clear {
            continue;
        }
        match stop.position.absolute() {
            Some(position) => model.tabs.push(RulerTab {
                position: position.raw(),
                align: stop.align,
                custom: true,
            }),
            None => model.unresolved = true,
        }
    }
    model.tabs.sort_by_key(|stop| stop.position);
    if default_interval <= 0 {
        model.unresolved = true;
        return model;
    }
    let Some(limit) = content_width else {
        return model;
    };
    let mut position = match model.tabs.last() {
        None => default_interval,
        Some(last) => {
            let remainder = last.position.rem_euclid(default_interval);
            if remainder == 0 {
                last.position.saturating_add(default_interval)
            } else {
                last.position - remainder + default_interval
            }
        }
    };
    let mut added = 0;
    while position <= limit && added < MAX_RULER_TABS {
        model.tabs.push(RulerTab {
            position,
            align: TabAlign::Left,
            custom: false,
        });
        added += 1;
        let next = position.saturating_add(default_interval);
        if next <= position {
            break;
        }
        position = next;
    }
    model
}

/// Original icon art: 16 rows of 16 cells, `#` ink and `.` paper, joined by `|`.
const ICONS: &[(&str, &str)] = &[
    (
        "copy",
        "................|................|................|..######........|..#....#........|..#..######.....|..#..#.#..#.....|..#..#.#..#.....|..######..#.....|.....#....#.....|.....######.....|................|................|................|................|................",
    ),
    (
        "cut",
        "................|................|...#.......#....|....#.....#.....|.....#...#......|......#.#.......|.......#........|......#.#.......|....###.###.....|...#..#.#..#....|...#..#.#..#....|....##...##.....|................|................|................|................",
    ),
    (
        "paste",
        "................|................|......####......|.....######.....|....#......#....|....#......#....|....#......#....|....#......#....|....#......#....|....#......#....|....#......#....|....########....|................|................|................|................",
    ),
    (
        "undo",
        "................|................|................|........####....|.......#....#...|......#.........|.....#..........|...##...........|.....#..........|......#.........|.......#...#....|........###.....|................|................|................|................",
    ),
    (
        "redo",
        "................|................|................|....####........|...#....#.......|.........#......|..........#.....|...........##...|..........#.....|.........#......|....#...#.......|.....###........|................|................|................|................",
    ),
    (
        "bold",
        "................|................|....#####.......|....#...#.......|....#...#.......|....#####.......|....#....#......|....#....#......|....#....#......|....######......|....#...........|....#...........|....#####.......|................|................|................",
    ),
    (
        "italic",
        "................|................|........####....|.........#......|........#.......|........#.......|.......#........|.......#........|......#.........|......#.........|.....#..........|.....#..........|....####........|................|................|................",
    ),
    (
        "underline",
        "................|................|....#.....#.....|....#.....#.....|....#.....#.....|....#.....#.....|....#.....#.....|....#.....#.....|.....#...#......|......###.......|................|...#########....|................|................|................|................",
    ),
    (
        "align-center",
        "................|................|................|................|...##########...|................|................|.....######.....|................|................|....########....|................|................|................|................|................",
    ),
    (
        "align-left",
        "................|................|................|................|...##########...|................|................|...######.......|................|................|...########.....|................|................|................|................|................",
    ),
    (
        "align-right",
        "................|................|................|................|...##########...|................|................|.......######...|................|................|.....########...|................|................|................|................|................",
    ),
    (
        "align-justify",
        "................|................|................|................|...##########...|................|................|...##########...|................|................|...##########...|................|................|................|................|................",
    ),
    (
        "styles",
        "................|................|................|...#######......|................|................|...#####........|................|................|...#########....|................|................|...####.........|................|................|................",
    ),
    (
        "list-remove",
        "................|................|................|................|...###.#######..|.............#..|...........###..|...###.#####....|................|................|...###.....###..|.............#..|...........#.#..|................|................|................",
    ),
    (
        "list-bullet",
        "................|................|................|................|...##..######...|................|................|................|...##..######...|................|................|................|...##..######...|................|................|................",
    ),
    (
        "list-level",
        "................|................|................|................|...##.#####.....|................|................|................|.....##.####....|................|................|................|.......##.####..|................|................|................",
    ),
    (
        "find",
        "................|................|................|....####........|...#....#.......|...#....#.......|...#....#.......|...#....#.......|....####........|.......#........|........#.......|.........#......|..........#.....|...........#....|................|................",
    ),
    (
        "find-previous",
        "................|................|................|........####....|.......#....#...|.......#....#...|.......#....#...|.......#....#...|........####....|......#.........|.....#..........|....#...........|...#............|..#.............|................|................",
    ),
    (
        "replace",
        "................|................|................|................|.......#........|..#######.......|.......#........|................|................|...#............|..#######.......|...#............|................|................|................|................",
    ),
    (
        "replace-all",
        "................|................|................|......#.........|..######........|......#.........|...#............|..######........|...#.........#..|.........######.|.............#..|..........#.....|.........######.|..........#.....|................|................",
    ),
    (
        "painter",
        "................|................|............##..|...........###..|..........###...|.........##.....|........##......|.......##.......|......##........|.....##.........|....##..........|...##...........|...#............|................|................|................",
    ),
    (
        "paint",
        "................|................|................|................|....#####.......|....#...#.......|....#...#.......|.....###........|......#.........|......#.........|......#.........|......#.........|.....###........|................|................|................",
    ),
    (
        "cancel",
        "................|................|................|................|....#......#....|.....#....#.....|......#..#......|.......##.......|.......##.......|......#..#......|.....#....#.....|....#......#....|................|................|................|................",
    ),
    (
        "picture",
        "................|................|...##########...|...#........#...|...#..####..#...|...#..#..#..#...|...#..####..#...|...#........#...|...#..##....#...|...#........#...|...##########...|................|................|................|................|................",
    ),
    (
        "table",
        "................|................|..############..|..#.....#....#..|..#.....#....#..|..############..|..#.....#....#..|..#.....#....#..|..############..|..#.....#....#..|..#.....#....#..|..############..|................|................|................|................",
    ),
    (
        "page-break",
        "................|................|....########....|....#......#....|....#......#....|....#......#....|................|................|...###...####...|................|....#......#....|....#......#....|....########....|................|................|................",
    ),
    (
        "page-a4",
        "................|................|.....######.....|.....#....#.....|.....#....#.....|.....#....#.....|.....#....#.....|.....#....#.....|.....#....#.....|.....#....#.....|.....#....#.....|.....#....#.....|.....######.....|................|................|................",
    ),
    (
        "page-letter",
        "................|................|................|...##########...|...#........#...|...#........#...|...#........#...|...#........#...|...#........#...|...#........#...|...#........#...|...##########...|................|................|................|................",
    ),
    (
        "margins",
        "................|................|................|..############..|..#..........#..|..#..........#..|..#..######..#..|..#..#....#..#..|..#..#....#..#..|..#..######..#..|..#..........#..|..#..........#..|..############..|................|................|................",
    ),
    (
        "margins-narrow",
        "................|................|................|..############..|..#..........#..|..############..|..##........##..|..##........##..|..##........##..|..##........##..|..############..|..#..........#..|..############..|................|................|................",
    ),
    (
        "portrait",
        "................|................|......####......|......#..#......|......#..#......|......#..#......|......#..#......|......#..#......|......#..#......|......#..#......|......#..#......|......#..#......|......####......|................|................|................",
    ),
    (
        "landscape",
        "................|................|................|................|................|..############..|..#..........#..|..#..........#..|..#..........#..|..#..........#..|..############..|................|................|................|................|................",
    ),
    (
        "header",
        "................|................|................|..############..|..#..........#..|..############..|................|................|................|................|................|................|................|................|................|................",
    ),
    (
        "footer",
        "................|................|................|................|................|................|................|................|................|................|..############..|..#..........#..|..############..|................|................|................",
    ),
    (
        "body",
        "................|................|................|................|................|..############..|..#..........#..|..#..........#..|..#..........#..|..#..........#..|..############..|................|................|................|................|................",
    ),
    (
        "zoom-in",
        "................|................|................|.......##.......|.......##.......|................|....########....|....########....|.......##.......|.......##.......|.......##.......|................|................|................|................|................",
    ),
    (
        "zoom-out",
        "................|................|................|................|................|................|................|....########....|....########....|................|................|................|................|................|................|................",
    ),
    (
        "zoom-reset",
        "................|................|................|................|....########....|....#......#....|....#......#....|....#......#....|....#......#....|....#......#....|....#......#....|....########....|................|................|................|................",
    ),
    (
        "language",
        "................|................|................|..#####..#####..|..#...#..#...#..|..#...#..#...#..|..#####..#####..|..#...#..#...#..|..#...#..#...#..|..#...#..#...#..|..#####..#####..|................|................|................|................|................",
    ),
    (
        "font",
        "................|................|................|....######......|....#....#......|....#....#......|....######......|....#....#......|....#....#......|....#....#......|....######......|................|................|................|................|................",
    ),
    (
        "paragraph",
        "................|................|................|................|..############..|................|................|....########....|................|................|..##########....|................|................|................|................|................",
    ),
    (
        "marks",
        "................|................|................|....#......#....|....##....##....|....#.#..#.#....|....#..##..#....|....#..##..#....|....#.#..#.#....|....##....##....|....#......#....|................|................|................|................|................",
    ),
    (
        "hidden",
        "................|................|................|......####......|.....#....#.....|.....#....#.....|......####......|................|....########....|................|....########....|................|....########....|................|................|................",
    ),
    (
        "field-code",
        "................|................|................|...##......##...|...#.#....#.#...|...#..#..#..#...|...#...##...#...|...#...##...#...|...#..#..#..#...|...#.#....#.#...|...##......##...|................|................|................|................|................",
    ),
    (
        "review-track",
        "................|................|..#.............|..#.............|..#.............|..#.............|..#.............|..#.............|..#.............|..#.............|..#.............|..#.............|................|................|................|................",
    ),
    (
        "review-accept",
        "................|................|................|................|............#...|...........#....|..........#.....|..#......#......|...#....#.......|....#..#........|.....#..........|................|................|................|................|................",
    ),
    (
        "review-reject",
        "................|................|................|................|...#........#...|....#......#....|.....#....#.....|......#..#......|......#..#......|.....#....#.....|....#......#....|...#........#...|................|................|................|................",
    ),
    (
        "review-markup",
        "................|................|..#.............|..#..######.....|..#.............|..#..####.......|..#.............|..#..######.....|..#.............|..#.............|................|................|................|................|................|................",
    ),
    (
        "review-final",
        "................|................|................|....########....|................|................|....######......|................|................|....########....|................|................|................|................|................|................",
    ),
    (
        "review-original",
        "................|................|................|....########....|................|................|################|................|................|....########....|................|................|................|................|................|................",
    ),
];

pub fn icon_ids() -> impl Iterator<Item = &'static str> {
    ICONS.iter().map(|(id, _)| *id)
}

pub fn icon_art(id: &str) -> Option<&'static str> {
    ICONS
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, art)| *art)
}

/// Ink cells of one original icon. Empty when the id is unknown or the art is blank.
pub fn icon_cells(id: &str) -> Vec<(u8, u8)> {
    icon_art(id)
        .and_then(|art| parse_icon(art).ok())
        .unwrap_or_default()
}

fn parse_icon(art: &str) -> Result<Vec<(u8, u8)>, ()> {
    let rows: Vec<&str> = art.split('|').collect();
    if rows.len() != 16 {
        return Err(());
    }
    let mut cells = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        let chars: Vec<char> = row.chars().collect();
        if chars.len() != 16 {
            return Err(());
        }
        for (x, ch) in chars.iter().enumerate() {
            match ch {
                '#' => cells.push((
                    u8::try_from(x).map_err(|_| ())?,
                    u8::try_from(y).map_err(|_| ())?,
                )),
                '.' => {}
                _ => return Err(()),
            }
        }
    }
    if cells.is_empty() {
        return Err(());
    }
    Ok(cells)
}
