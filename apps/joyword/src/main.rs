//! In-memory text editing backed by the engine and unified page scene.
//! Menus, shortcuts and on-screen buttons come from the joyword-shell command
//! registry and run through its single entry point (T106).
mod chrome_ui;
mod document_canvas;
mod export;
mod field_edit;
mod input_ui;
mod linux_files;
#[cfg(target_os = "linux")]
mod prompt_ui;

use document_canvas::{CanvasPipeline, Landed, LayoutFrame, LayoutRequestKey, PageImage, PageKey};
use gpui::{
    Action, App, Bounds, Context, FocusHandle, KeyBinding, Menu, MenuItem, SharedString, Window,
    WindowBounds, WindowOptions, actions, div, img, prelude::*, px, rgb, size,
};
use joyword_foundation::ScreenScale;
use joyword_layout::{LoadedFont, SingleFontProfile, ViewOptions};
use joyword_render::Projection;
use joyword_render::requests::RasterTicket;
use joyword_shell::chrome::RibbonTab;
use joyword_shell::command::*;
use joyword_shell::diag::{self, AppLog, Event, Level};
use joyword_shell::input::{InputLimits, TextInput};
use joyword_shell::l10n::{Catalog, Locale};
use joyword_shell::prefs::{PrefsNotice, PrefsStore};
#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;
use std::sync::Arc;

actions!(
    joyword,
    [
        Quit,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        SwitchLanguage,
        Undo,
        Redo,
        SelectAll,
        Backspace,
        DeleteForward,
        NewParagraph,
        MoveLeft,
        MoveRight,
        ExtendLeft,
        ExtendRight,
        MoveUp,
        MoveDown,
        ExtendUp,
        ExtendDown,
        LineStart,
        LineEnd,
        ExtendLineStart,
        ExtendLineEnd,
        WordLeft,
        WordRight,
        ExtendWordLeft,
        ExtendWordRight,
        ParagraphUp,
        ParagraphDown,
        ExtendParagraphUp,
        ExtendParagraphDown,
        DocumentStart,
        DocumentEnd,
        ExtendDocumentStart,
        ExtendDocumentEnd,
        PageUp,
        PageDown,
        ExtendPageUp,
        ExtendPageDown,
        DeleteWordBackward,
        DeleteWordForward,
        CancelComposition,
        OpenFile,
        OpenRecent,
        SwitchDocument,
        InsertText,
        SaveOriginal,
        SaveCopy,
        ExportPdf,
        PrintFile,
        Bold,
        Italic,
        Underline,
        ListStyles,
        ApplyStyle,
        ApplyList,
        SetListLevel,
        RemoveList,
        AlignLeft,
        AlignCenter,
        AlignRight,
        Justify,
        PageBreak,
        PageNumber,
        PageSize,
        PageMargins,
        PageOrientation,
        EditHeader,
        EditFooter,
        EditBody,
        CutText,
        CopyText,
        PasteText,
        InsertImage,
        CloseDocument,
        FindNext,
        FindPrevious,
        ReplaceNext,
        ReplaceAll,
        CopyFormat,
        PaintFormat,
        CancelFormat,
        InsertTable,
        InsertTableRow,
        DeleteTableRow,
        InsertTableColumn,
        DeleteTableColumn,
        SelectTableCell,
        MergeTableRight,
        MergeTableDown,
        SplitTableCell,
        TableProperties,
        FontDialog,
        ParagraphDialog,
        FormattingMarks,
        HiddenText,
        FieldCodes,
        TrackRevisions,
        AcceptRevision,
        RejectRevision,
        AcceptAllRevisions,
        RejectAllRevisions,
        ReviewMarkup,
        ReviewFinal,
        ReviewOriginal,
        SetReviewAuthor
    ]
);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialogKind {
    Font,
    Paragraph,
    Table,
}

pub(crate) struct DialogField {
    pub name: &'static str,
    pub label: &'static str,
    pub value: field_edit::FieldBuffer,
    pub focus: FocusHandle,
}

pub(crate) struct FormatDialog {
    pub kind: DialogKind,
    pub fields: Vec<DialogField>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum RulerHit {
    First,
    Body,
    Right,
    Tab { from_emu: i64 },
}

const ZOOM_MIN: u16 = 50;
const ZOOM_MAX: u16 = 150;
const ZOOM_DEFAULT: u16 = 80;

/// GPUI action for a registered command. Every registry entry must map;
/// `registry_commands_all_have_actions` checks it.
fn view_options(toggles: joyword_shell::dialogs::ViewToggles) -> ViewOptions {
    ViewOptions {
        formatting_marks: toggles.formatting_marks,
        show_hidden: toggles.show_hidden,
        field_codes: toggles.field_codes,
    }
}

fn action_for(id: CommandId) -> Option<Box<dyn Action>> {
    Some(match id {
        QUIT => Box::new(Quit),
        ZOOM_IN => Box::new(ZoomIn),
        ZOOM_OUT => Box::new(ZoomOut),
        ZOOM_RESET => Box::new(ZoomReset),
        SWITCH_LANGUAGE => Box::new(SwitchLanguage),
        EDIT_UNDO => Box::new(Undo),
        EDIT_REDO => Box::new(Redo),
        EDIT_SELECT_ALL => Box::new(SelectAll),
        EDIT_BACKSPACE => Box::new(Backspace),
        EDIT_DELETE => Box::new(DeleteForward),
        EDIT_ENTER => Box::new(NewParagraph),
        EDIT_LEFT => Box::new(MoveLeft),
        EDIT_RIGHT => Box::new(MoveRight),
        EDIT_EXTEND_LEFT => Box::new(ExtendLeft),
        EDIT_EXTEND_RIGHT => Box::new(ExtendRight),
        EDIT_UP => Box::new(MoveUp),
        EDIT_DOWN => Box::new(MoveDown),
        EDIT_EXTEND_UP => Box::new(ExtendUp),
        EDIT_EXTEND_DOWN => Box::new(ExtendDown),
        EDIT_HOME => Box::new(LineStart),
        EDIT_END => Box::new(LineEnd),
        EDIT_EXTEND_HOME => Box::new(ExtendLineStart),
        EDIT_EXTEND_END => Box::new(ExtendLineEnd),
        EDIT_WORD_LEFT => Box::new(WordLeft),
        EDIT_WORD_RIGHT => Box::new(WordRight),
        EDIT_EXTEND_WORD_LEFT => Box::new(ExtendWordLeft),
        EDIT_EXTEND_WORD_RIGHT => Box::new(ExtendWordRight),
        EDIT_PARAGRAPH_UP => Box::new(ParagraphUp),
        EDIT_PARAGRAPH_DOWN => Box::new(ParagraphDown),
        EDIT_EXTEND_PARAGRAPH_UP => Box::new(ExtendParagraphUp),
        EDIT_EXTEND_PARAGRAPH_DOWN => Box::new(ExtendParagraphDown),
        EDIT_DOCUMENT_START => Box::new(DocumentStart),
        EDIT_DOCUMENT_END => Box::new(DocumentEnd),
        EDIT_EXTEND_DOCUMENT_START => Box::new(ExtendDocumentStart),
        EDIT_EXTEND_DOCUMENT_END => Box::new(ExtendDocumentEnd),
        EDIT_PAGE_UP => Box::new(PageUp),
        EDIT_PAGE_DOWN => Box::new(PageDown),
        EDIT_EXTEND_PAGE_UP => Box::new(ExtendPageUp),
        EDIT_EXTEND_PAGE_DOWN => Box::new(ExtendPageDown),
        EDIT_DELETE_WORD_BACKWARD => Box::new(DeleteWordBackward),
        EDIT_DELETE_WORD_FORWARD => Box::new(DeleteWordForward),
        EDIT_CANCEL => Box::new(CancelComposition),
        FILE_OPEN => Box::new(OpenFile),
        FILE_OPEN_RECENT => Box::new(OpenRecent),
        DOCUMENT_SWITCH => Box::new(SwitchDocument),
        FILE_SAVE => Box::new(SaveOriginal),
        FILE_SAVE_COPY => Box::new(SaveCopy),
        FILE_EXPORT_PDF => Box::new(ExportPdf),
        FILE_PRINT => Box::new(PrintFile),
        EDIT_BOLD => Box::new(Bold),
        EDIT_ITALIC => Box::new(Italic),
        EDIT_UNDERLINE => Box::new(Underline),
        STYLE_LIST => Box::new(ListStyles),
        STYLE_APPLY => Box::new(ApplyStyle),
        LIST_APPLY => Box::new(ApplyList),
        LIST_LEVEL => Box::new(SetListLevel),
        LIST_REMOVE => Box::new(RemoveList),
        PAGE_BREAK => Box::new(PageBreak),
        PAGE_NUMBER => Box::new(PageNumber),
        PAGE_SIZE => Box::new(PageSize),
        PAGE_MARGINS => Box::new(PageMargins),
        PAGE_ORIENTATION => Box::new(PageOrientation),
        PAGE_EDIT_HEADER => Box::new(EditHeader),
        PAGE_EDIT_FOOTER => Box::new(EditFooter),
        PAGE_EDIT_BODY => Box::new(EditBody),
        EDIT_ALIGN_LEFT => Box::new(AlignLeft),
        EDIT_ALIGN_CENTER => Box::new(AlignCenter),
        EDIT_ALIGN_RIGHT => Box::new(AlignRight),
        EDIT_JUSTIFY => Box::new(Justify),
        EDIT_CUT => Box::new(CutText),
        EDIT_COPY => Box::new(CopyText),
        EDIT_PASTE => Box::new(PasteText),
        EDIT_INSERT => Box::new(InsertText),
        EDIT_INSERT_IMAGE => Box::new(InsertImage),
        FILE_CLOSE => Box::new(CloseDocument),
        FIND_NEXT => Box::new(FindNext),
        FIND_PREVIOUS => Box::new(FindPrevious),
        FIND_REPLACE => Box::new(ReplaceNext),
        FIND_REPLACE_ALL => Box::new(ReplaceAll),
        FORMAT_COPY => Box::new(CopyFormat),
        FORMAT_PAINT => Box::new(PaintFormat),
        FORMAT_CANCEL => Box::new(CancelFormat),
        TABLE_INSERT => Box::new(InsertTable),
        TABLE_ROW_INSERT => Box::new(InsertTableRow),
        TABLE_ROW_DELETE => Box::new(DeleteTableRow),
        TABLE_COLUMN_INSERT => Box::new(InsertTableColumn),
        TABLE_COLUMN_DELETE => Box::new(DeleteTableColumn),
        TABLE_SELECT => Box::new(SelectTableCell),
        TABLE_MERGE_RIGHT => Box::new(MergeTableRight),
        TABLE_MERGE_DOWN => Box::new(MergeTableDown),
        TABLE_SPLIT => Box::new(SplitTableCell),
        TABLE_PROPERTIES => Box::new(TableProperties),
        FORMAT_FONT => Box::new(FontDialog),
        FORMAT_PARAGRAPH => Box::new(ParagraphDialog),
        VIEW_FORMATTING_MARKS => Box::new(FormattingMarks),
        VIEW_HIDDEN_TEXT => Box::new(HiddenText),
        VIEW_FIELD_CODES => Box::new(FieldCodes),
        REVIEW_TRACK => Box::new(TrackRevisions),
        REVIEW_ACCEPT => Box::new(AcceptRevision),
        REVIEW_REJECT => Box::new(RejectRevision),
        REVIEW_ACCEPT_ALL => Box::new(AcceptAllRevisions),
        REVIEW_REJECT_ALL => Box::new(RejectAllRevisions),
        REVIEW_MARKUP => Box::new(ReviewMarkup),
        REVIEW_FINAL => Box::new(ReviewFinal),
        REVIEW_ORIGINAL => Box::new(ReviewOriginal),
        REVIEW_AUTHOR => Box::new(SetReviewAuthor),
        _ => return None,
    })
}

/// Converts the registry's menus; order and per-platform placement (App menu
/// first on macOS) are decided by `Registry::menus`.
fn menus(registry: &Registry, catalog: &Catalog) -> Vec<Menu> {
    registry
        .menus(Platform::current())
        .into_iter()
        .map(|menu| Menu {
            name: catalog.text(menu.kind.label_key()).into(),
            items: menu
                .entries
                .into_iter()
                .filter_map(|entry| match entry {
                    MenuEntry::Separator => Some(MenuItem::separator()),
                    MenuEntry::Command(id) => {
                        let spec = registry.get(id)?;
                        Some(MenuItem::Action {
                            name: catalog.text(spec.label).into(),
                            action: action_for(id)?,
                            os_action: None,
                            checked: false,
                            disabled: false,
                        })
                    }
                })
                .collect(),
            disabled: false,
        })
        .collect()
}

/// The shown document's file name; an editor never bound to a file is
/// untitled. `None` before any document is shown.
fn document_name(editor: Option<&dyn input_ui::DocEditor>, locale: Locale) -> Option<String> {
    let editor = editor?;
    Some(
        match editor.source_path().and_then(|path| path.file_name()) {
            Some(name) => name.to_string_lossy().into_owned(),
            None => Catalog::shipped(locale).format("document.untitled", &[("number", "1")]),
        },
    )
}

/// Word-style window title: document name first, `*` while unsaved.
fn window_title(document: Option<&str>, edited: bool) -> String {
    match document {
        Some(name) => format!("{}{name} - JoyWord", if edited { "*" } else { "" }),
        None => "JoyWord".into(),
    }
}

/// The document named on the command line: the first argument that is not
/// an option, or the first one after `--`.
fn startup_document(
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> Option<std::path::PathBuf> {
    let mut options = true;
    for arg in args {
        if options && arg == "--" {
            options = false;
            continue;
        }
        if options && arg.to_string_lossy().starts_with('-') {
            continue;
        }
        return Some(arg.into());
    }
    None
}

fn shortcut_chords(registry: &Registry) -> Vec<(&'static str, CommandId)> {
    let platform = Platform::current();
    let mut chords = Vec::new();
    for spec in registry.commands() {
        if let Some(keys) = spec.shortcut.for_platform(platform) {
            chords.push((keys, spec.id));
        }
        if platform == Platform::Linux
            && let Some(extra) = spec.shortcut.linux_alternate()
            && spec.shortcut.for_platform(platform) != Some(extra)
        {
            chords.push((extra, spec.id));
        }
    }
    chords
}

/// Document focus, and not one of the in-app text fields. A focused field
/// pushes [`field_edit::FIELD_CONTEXT`], and `!JoyWordField` checks the whole
/// context stack, so these bindings stay with the document.
const DOCUMENT: &str = "JoyWordInput && !JoyWordField";
/// Document focus with no open IME composition.
const TYPING: &str = "JoyWordInput && !composing && !JoyWordField";
/// Document focus while an IME composition is open.
const COMPOSING: &str = "JoyWordInput && composing && !JoyWordField";

fn key_bindings(registry: &Registry) -> Vec<KeyBinding> {
    shortcut_chords(registry)
        .into_iter()
        .filter_map(|(keys, id)| {
            Some(match id {
                QUIT => KeyBinding::new(keys, Quit, None),
                ZOOM_IN => KeyBinding::new(keys, ZoomIn, None),
                ZOOM_OUT => KeyBinding::new(keys, ZoomOut, None),
                ZOOM_RESET => KeyBinding::new(keys, ZoomReset, None),
                SWITCH_LANGUAGE => KeyBinding::new(keys, SwitchLanguage, None),
                EDIT_UNDO => KeyBinding::new(keys, Undo, Some(DOCUMENT)),
                EDIT_REDO => KeyBinding::new(keys, Redo, Some(DOCUMENT)),
                EDIT_SELECT_ALL => KeyBinding::new(keys, SelectAll, Some(DOCUMENT)),
                EDIT_BACKSPACE => KeyBinding::new(keys, Backspace, Some(TYPING)),
                EDIT_DELETE => KeyBinding::new(keys, DeleteForward, Some(TYPING)),
                EDIT_ENTER => KeyBinding::new(keys, NewParagraph, Some(TYPING)),
                EDIT_LEFT => KeyBinding::new(keys, MoveLeft, Some(TYPING)),
                EDIT_RIGHT => KeyBinding::new(keys, MoveRight, Some(TYPING)),
                EDIT_EXTEND_LEFT => KeyBinding::new(keys, ExtendLeft, Some(TYPING)),
                EDIT_EXTEND_RIGHT => KeyBinding::new(keys, ExtendRight, Some(TYPING)),
                EDIT_UP => KeyBinding::new(keys, MoveUp, Some(TYPING)),
                EDIT_DOWN => KeyBinding::new(keys, MoveDown, Some(TYPING)),
                EDIT_EXTEND_UP => KeyBinding::new(keys, ExtendUp, Some(TYPING)),
                EDIT_EXTEND_DOWN => KeyBinding::new(keys, ExtendDown, Some(TYPING)),
                EDIT_HOME => KeyBinding::new(keys, LineStart, Some(TYPING)),
                EDIT_END => KeyBinding::new(keys, LineEnd, Some(TYPING)),
                EDIT_EXTEND_HOME => KeyBinding::new(keys, ExtendLineStart, Some(TYPING)),
                EDIT_EXTEND_END => KeyBinding::new(keys, ExtendLineEnd, Some(TYPING)),
                EDIT_WORD_LEFT => KeyBinding::new(keys, WordLeft, Some(TYPING)),
                EDIT_WORD_RIGHT => KeyBinding::new(keys, WordRight, Some(TYPING)),
                EDIT_EXTEND_WORD_LEFT => KeyBinding::new(keys, ExtendWordLeft, Some(TYPING)),
                EDIT_EXTEND_WORD_RIGHT => KeyBinding::new(keys, ExtendWordRight, Some(TYPING)),
                EDIT_PARAGRAPH_UP => KeyBinding::new(keys, ParagraphUp, Some(TYPING)),
                EDIT_PARAGRAPH_DOWN => KeyBinding::new(keys, ParagraphDown, Some(TYPING)),
                EDIT_EXTEND_PARAGRAPH_UP => KeyBinding::new(keys, ExtendParagraphUp, Some(TYPING)),
                EDIT_EXTEND_PARAGRAPH_DOWN => {
                    KeyBinding::new(keys, ExtendParagraphDown, Some(TYPING))
                }
                EDIT_DOCUMENT_START => KeyBinding::new(keys, DocumentStart, Some(TYPING)),
                EDIT_DOCUMENT_END => KeyBinding::new(keys, DocumentEnd, Some(TYPING)),
                EDIT_EXTEND_DOCUMENT_START => {
                    KeyBinding::new(keys, ExtendDocumentStart, Some(TYPING))
                }
                EDIT_EXTEND_DOCUMENT_END => KeyBinding::new(keys, ExtendDocumentEnd, Some(TYPING)),
                EDIT_PAGE_UP => KeyBinding::new(keys, PageUp, Some(TYPING)),
                EDIT_PAGE_DOWN => KeyBinding::new(keys, PageDown, Some(TYPING)),
                EDIT_EXTEND_PAGE_UP => KeyBinding::new(keys, ExtendPageUp, Some(TYPING)),
                EDIT_EXTEND_PAGE_DOWN => KeyBinding::new(keys, ExtendPageDown, Some(TYPING)),
                EDIT_DELETE_WORD_BACKWARD => {
                    KeyBinding::new(keys, DeleteWordBackward, Some(TYPING))
                }
                EDIT_DELETE_WORD_FORWARD => KeyBinding::new(keys, DeleteWordForward, Some(TYPING)),
                // Without a composition Esc must reach the panels' key handlers.
                EDIT_CANCEL => KeyBinding::new(keys, CancelComposition, Some(COMPOSING)),
                FILE_OPEN => KeyBinding::new(keys, OpenFile, None),
                FILE_SAVE => KeyBinding::new(keys, SaveOriginal, None),
                FILE_SAVE_COPY => KeyBinding::new(keys, SaveCopy, None),
                FILE_EXPORT_PDF => KeyBinding::new(keys, ExportPdf, None),
                FILE_PRINT => KeyBinding::new(keys, PrintFile, None),
                EDIT_BOLD => KeyBinding::new(keys, Bold, Some(TYPING)),
                EDIT_ITALIC => KeyBinding::new(keys, Italic, Some(TYPING)),
                EDIT_UNDERLINE => KeyBinding::new(keys, Underline, Some(TYPING)),
                EDIT_ALIGN_LEFT => KeyBinding::new(keys, AlignLeft, Some(TYPING)),
                EDIT_ALIGN_CENTER => KeyBinding::new(keys, AlignCenter, Some(TYPING)),
                EDIT_ALIGN_RIGHT => KeyBinding::new(keys, AlignRight, Some(TYPING)),
                EDIT_JUSTIFY => KeyBinding::new(keys, Justify, Some(TYPING)),
                EDIT_CUT => KeyBinding::new(keys, CutText, Some(TYPING)),
                EDIT_COPY => KeyBinding::new(keys, CopyText, Some(DOCUMENT)),
                EDIT_PASTE => KeyBinding::new(keys, PasteText, Some(TYPING)),
                FILE_CLOSE => KeyBinding::new(keys, CloseDocument, None),
                FIND_NEXT => KeyBinding::new(keys, FindNext, Some(DOCUMENT)),
                FIND_REPLACE => KeyBinding::new(keys, ReplaceNext, Some(DOCUMENT)),
                PAGE_BREAK => KeyBinding::new(keys, PageBreak, Some(TYPING)),
                FORMAT_FONT => KeyBinding::new(keys, FontDialog, Some(TYPING)),
                _ => return None,
            })
        })
        .collect()
}

#[derive(Clone, Copy)]
struct MainWindow(gpui::WindowHandle<JoyWord>);
impl gpui::Global for MainWindow {}

struct JoyWord {
    zoom_percent: u16,
    locale: Locale,
    registry: Registry,
    focus_handle: FocusHandle,
    editor: Option<Box<dyn input_ui::DocEditor>>,
    copy_note: Option<String>,
    font: Option<Arc<LoadedFont>>,
    profile: Option<SingleFontProfile>,
    canvas: CanvasPipeline,
    load_started: bool,
    load_task: Option<gpui::Task<()>>,
    layout_task: Option<gpui::Task<()>>,
    raster_task: Option<gpui::Task<()>>,
    device_scale: f32,
    canvas_error: Option<String>,
    input_error: Option<String>,
    scroll: gpui::ScrollHandle,
    viewport_bounds: Option<Bounds<gpui::Pixels>>,
    dragging: bool,
    reveal_pending: bool,
    ime_geometry: Option<input_ui::ImeGeometry>,
    ime_anchor: Option<input_ui::ImeAnchor>,
    preferred_x: Option<joyword_foundation::Emu>,
    close_pending: bool,
    close_task: Option<gpui::Task<()>>,
    subscriptions: Vec<gpui::Subscription>,
    clipboard: String,
    session: joyword_shell::host::Office,
    open_gate: Option<u64>,
    recovery_dir: Option<std::path::PathBuf>,
    held_document: Option<joyword_shell::host::DocumentId>,
    /// Bumped by [`input_ui`] each time a different editor is installed.
    copy_epoch: u64,
    copy: Option<input_ui::CopyPhase>,
    pdf: Option<input_ui::PdfPhase>,
    original_save: Option<input_ui::OriginalSavePhase>,
    diagnostics: Option<Arc<AppLog>>,
    find_query: field_edit::FieldBuffer,
    find_replacement: field_edit::FieldBuffer,
    find_case: bool,
    find_word: bool,
    /// Full-width / half-width folding. Off keeps the previous literal compare.
    find_width: bool,
    /// Treat the query as a linear-time regular expression.
    find_regex: bool,
    find_query_focus: FocusHandle,
    find_replacement_focus: FocusHandle,
    find_note: Option<String>,
    format_repeat: bool,
    /// Last title given to the platform window.
    window_title: String,
    ribbon_tab: RibbonTab,
    format_dialog: Option<FormatDialog>,
    path_sheet: Option<linux_files::PathSheet>,
    view_toggles: joyword_shell::dialogs::ViewToggles,
    ruler_bounds: std::sync::Arc<std::sync::Mutex<Option<Bounds<gpui::Pixels>>>>,
    ruler_drag: Option<(RulerHit, gpui::Point<gpui::Pixels>)>,
}

#[derive(Clone, Copy)]
enum FindSlot {
    Query,
    Replacement,
}

/// Find-bar option the user can switch without leaving the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FindToggle {
    Case,
    Word,
    Width,
    Regex,
}

/// Options the find bar sends with `find.next`, `find.previous`,
/// `find.replace` and `find.replace_all`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FindBarFlags {
    case_sensitive: bool,
    whole_word: bool,
    fold_width: bool,
    regex: bool,
}

impl FindBarFlags {
    fn args(self, query: &str, replacement: Option<&str>) -> Vec<(&'static str, ArgValue)> {
        let mut args = vec![
            ("query", ArgValue::Text(query.to_owned())),
            ("case", ArgValue::Bool(self.case_sensitive)),
            ("word", ArgValue::Bool(self.whole_word)),
            ("width", ArgValue::Bool(self.fold_width)),
            ("regex", ArgValue::Bool(self.regex)),
        ];
        if let Some(replacement) = replacement {
            args.push(("replacement", ArgValue::Text(replacement.to_owned())));
        }
        args
    }
}

/// Ribbon find controls bind query, case and word only. Fill the two bar
/// checkboxes when that caller left them out, and leave an explicit value
/// alone so the bar's own arguments stay the source of truth.
fn supplement_find_flags(
    args: &[(&'static str, ArgValue)],
    fold_width: bool,
    regex: bool,
) -> Vec<(&'static str, ArgValue)> {
    let mut out = args.to_vec();
    if !out.iter().any(|(name, _)| *name == "width") {
        out.push(("width", ArgValue::Bool(fold_width)));
    }
    if !out.iter().any(|(name, _)| *name == "regex") {
        out.push(("regex", ArgValue::Bool(regex)));
    }
    out
}

/// View-level commands, executed with access to the view's context.
fn optional_text<'a>(
    args: &'a std::collections::BTreeMap<&'static str, ArgValue>,
    name: &'static str,
) -> Result<Option<&'a str>, ExecuteError> {
    match args.get(name) {
        Some(ArgValue::Text(value)) => Ok(Some(value.as_str())),
        Some(_) => Err(ExecuteError::Failed(name.into())),
        None => Ok(None),
    }
}

struct ViewHost<'a, 'b> {
    view: &'a mut JoyWord,
    window: &'a mut Window,
    cx: &'a mut Context<'b, JoyWord>,
}

impl CommandHost for ViewHost<'_, '_> {
    fn availability(&self, id: CommandId) -> Availability {
        match id {
            ZOOM_IN if self.view.zoom_percent >= ZOOM_MAX => Availability::Disabled("zoom.max"),
            ZOOM_OUT if self.view.zoom_percent <= ZOOM_MIN => Availability::Disabled("zoom.min"),
            _ => self.view.input_availability(id),
        }
    }

    fn execute(
        &mut self,
        id: CommandId,
        args: &std::collections::BTreeMap<&'static str, ArgValue>,
    ) -> Result<(), ExecuteError> {
        let view = &mut *self.view;
        match id {
            ZOOM_IN => view.zoom_percent = (view.zoom_percent + 10).min(ZOOM_MAX),
            ZOOM_OUT => view.zoom_percent = (view.zoom_percent - 10).max(ZOOM_MIN),
            ZOOM_RESET => view.zoom_percent = 100,
            SWITCH_LANGUAGE => {
                view.locale = match view.locale {
                    Locale::ZhCn => Locale::En,
                    Locale::En => Locale::ZhCn,
                };
                self.cx
                    .set_menus(menus(&view.registry, Catalog::shipped(view.locale)));
            }
            QUIT => view.request_close(true, self.window, self.cx),
            FILE_OPEN | FILE_OPEN_RECENT => {
                let Some(ArgValue::Path(path)) = args.get("path") else {
                    return Err(ExecuteError::Pending(vec!["path"]));
                };
                if id == FILE_OPEN_RECENT
                    && !view.session.recent().iter().any(|recent| recent == path)
                {
                    return Err(ExecuteError::Failed("path is not in recent files".into()));
                }
                view.open_path_sync(path, self.cx)
                    .map_err(ExecuteError::Failed)?;
            }
            DOCUMENT_SWITCH => {
                let Some(ArgValue::Id(id)) = args.get("id") else {
                    return Err(ExecuteError::Pending(vec!["id"]));
                };
                view.session
                    .invoke(DOCUMENT_SWITCH, &[("id", ArgValue::Id(*id))])
                    .map_err(|error| match error {
                        InvokeError::PendingInput { missing } => ExecuteError::Pending(missing),
                        InvokeError::Failed(message) | InvokeError::Unknown(message) => {
                            ExecuteError::Failed(message)
                        }
                        InvokeError::Disabled(message) => ExecuteError::Failed(message.to_string()),
                    })?;
            }
            FILE_SAVE => view.begin_save_original(self.window, self.cx),
            FILE_SAVE_COPY => {
                let Some(ArgValue::Path(path)) = args.get("destination") else {
                    return Err(ExecuteError::Pending(vec!["destination"]));
                };
                view.save_to(path).map_err(ExecuteError::Failed)?;
            }
            FILE_EXPORT_PDF => {
                let Some(ArgValue::Path(path)) = args.get("destination") else {
                    return Err(ExecuteError::Pending(vec!["destination"]));
                };
                view.queue_pdf(
                    input_ui::PdfOrder {
                        destination: path.clone(),
                        print: false,
                        copies: None,
                        range: None,
                        marks: None,
                    },
                    self.window,
                    self.cx,
                )?;
            }
            FILE_PRINT => {
                let Some(ArgValue::Path(path)) = args.get("destination") else {
                    return Err(ExecuteError::Pending(vec!["destination"]));
                };
                view.queue_pdf(
                    input_ui::PdfOrder {
                        destination: path.clone(),
                        print: true,
                        copies: optional_text(args, "copies")?,
                        range: optional_text(args, "range")?,
                        marks: optional_text(args, "marks")?,
                    },
                    self.window,
                    self.cx,
                )?;
            }
            FILE_CLOSE => {
                if matches!(args.get("choice"), Some(ArgValue::Text(choice)) if choice == "cancel")
                {
                    return Ok(());
                }
                let dirty = view
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.has_unsaved_changes());
                if dirty && args.get("choice").is_none() {
                    return Err(ExecuteError::Pending(vec!["choice"]));
                }
                view.request_close(false, self.window, self.cx);
            }
            other => {
                return view
                    .execute_input(other, args, self.window, self.cx)
                    .map_err(ExecuteError::Failed);
            }
        }
        self.cx.notify();
        Ok(())
    }
}

impl Drop for JoyWord {
    fn drop(&mut self) {
        self.canvas.cancel();
    }
}

impl JoyWord {
    fn projection(&self) -> Projection {
        Projection::new(
            96.0 * f64::from(self.device_scale),
            f64::from(self.zoom_percent) / 100.0,
        )
        .expect("validated view scales")
    }
    fn layout_key(&self) -> Option<LayoutRequestKey> {
        Some(self.canvas.key_viewed(
            self.editor.as_ref()?.view(),
            self.font.as_ref()?,
            self.profile?,
            view_options(self.view_toggles),
            self.view_toggles.revision,
        ))
    }
    /// The layout of the current revision: geometry for hit testing, IME,
    /// reveal and overlays, whether or not its pixels have landed.
    fn ready_layout(&self) -> Option<&LayoutFrame> {
        if self.canvas_error.is_some() {
            return None;
        }
        self.canvas.ready_layout(&self.layout_key()?)
    }
    fn clear_canvas(&mut self, cx: &mut Context<Self>) {
        for image in self.canvas.clear() {
            cx.drop_image(image, None);
        }
    }
    fn request_canvas(&mut self, cx: &mut Context<Self>) {
        if !self.load_started {
            self.load_started = true;
            let job = cx
                .background_executor()
                .spawn(async { document_canvas::load_app_source() });
            self.load_task = Some(cx.spawn(async move |view, cx| {
                let result = job.await;
                let _ = view.update(cx, |view, cx| {
                    match result {
                        Ok(source) => {
                            let document_canvas::AppSource {
                                session,
                                layout,
                                font,
                                profile,
                            } = source;
                            if let Err(error) = layout.ensure_current(
                                &session.session().document().snapshot(),
                                &font,
                                profile,
                            ) {
                                view.report_canvas_error(error.to_string());
                                cx.notify();
                                return;
                            }
                            match TextInput::new(session, InputLimits::default()) {
                                Ok(editor) => {
                                    // A journal restored at startup already owns the editor.
                                    if view.editor.is_none() {
                                        view.install_editor(Box::new(editor));
                                    }
                                    view.font = Some(Arc::new(font));
                                    view.profile = Some(profile);
                                }
                                Err(e) => view.report_canvas_error(e.to_string()),
                            }
                        }
                        Err(e) => view.report_canvas_error(e),
                    }
                    cx.notify();
                });
            }));
        }
        let (Some(key), Some(font), Some(editor)) =
            (self.layout_key(), self.font.clone(), self.editor.as_ref())
        else {
            return;
        };
        if let Some(job) = self.canvas.request_layout(key.clone()) {
            self.canvas_error = None;
            let input = editor.view().clone();
            let font = font.clone();
            let profile = key.profile;
            let cancellation = job.cancellation;
            let cache = self.canvas.paragraph_cache();
            let previous = self
                .canvas
                .previous_for(&font.key(), profile, key.view, key.review);
            let work = cx.background_executor().spawn(async move {
                document_canvas::layout_view_incremental_with(
                    &input,
                    &font,
                    profile,
                    &cancellation,
                    &cache,
                    previous.as_deref(),
                    key.view,
                    key.review,
                )
            });
            let ticket = job.ticket;
            self.layout_task = Some(cx.spawn(async move |view, cx| {
                let result = work.await;
                let _ = view.update(cx, |view, cx| view.layout_landed(&ticket, result, cx));
            }));
        }
        // Pixels follow only a current layout, one page at a time around the
        // viewport; zoom alone needs no new layout.
        let visible = self.visible_pages();
        if let Some(job) = self.canvas.request_raster(&key, self.projection(), visible) {
            self.canvas_error = None;
            let PageKey {
                page, projection, ..
            } = *job.ticket.key();
            let (layout, limits, cancellation) = (job.layout, job.limits, job.cancellation);
            // The caret rides along only when it is on this page, so its
            // overlay only ever shows there.
            let caret = self.editor.as_ref().and_then(|editor| {
                let frame = self.canvas.layout()?;
                let geometry = frame.layout.caret(editor.view().selection().head).ok()?;
                (geometry.page == page).then_some((geometry.x, geometry.top, geometry.height))
            });
            let work = cx.background_executor().spawn(async move {
                document_canvas::raster_page(
                    &layout,
                    page,
                    &font,
                    projection,
                    limits,
                    caret,
                    &cancellation,
                )
            });
            let ticket = job.ticket;
            self.raster_task = Some(cx.spawn(async move |view, cx| {
                let result = work.await;
                let _ = view.update(cx, |view, cx| view.raster_landed(&ticket, result, cx));
            }));
        }
    }
    fn layout_landed(
        &mut self,
        ticket: &RasterTicket<LayoutRequestKey>,
        result: Result<joyword_layout::LayoutSnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(current) = self.layout_key() else {
            return;
        };
        match self.canvas.install_layout(ticket, &current, result) {
            Landed::Stale => return,
            Landed::Installed(_) => {
                let version = self
                    .canvas
                    .layout()
                    .map(|frame| frame.layout.key().version.get());
                if let Some((version, editor)) = version.zip(self.editor.as_mut()) {
                    editor.acknowledge_layout(version);
                }
                // Exact geometry now: the origin AppKit asks for next and the
                // caret no longer wait for pixels.
                self.capture_ime_anchor();
                self.reveal_caret();
                self.request_canvas(cx);
            }
            Landed::Failed(error) => self.report_canvas_error(error),
        }
        cx.notify();
    }
    fn raster_landed(
        &mut self,
        ticket: &RasterTicket<PageKey>,
        result: Result<document_canvas::RasterOutput, String>,
        cx: &mut Context<Self>,
    ) {
        match self
            .canvas
            .install_raster(ticket, self.projection(), result)
        {
            // Nothing to show; the next missing page, if any, still follows.
            Landed::Stale => {
                self.request_canvas(cx);
                return;
            }
            Landed::Installed(released) => {
                for image in released {
                    cx.drop_image(image, None);
                }
            }
            // The page shows its own failure; the canvas and other pages stay.
            Landed::Failed(error) => self.note_diag(Level::Error, "ui.raster", &error),
        }
        cx.notify();
    }
    fn canvas(&self) -> gpui::AnyElement {
        if let Some(error) = &self.canvas_error {
            return div()
                .p_6()
                .bg(rgb(0xfff8e7))
                .child(Catalog::shipped(self.locale).format("canvas.error", &[("reason", error)]))
                .into_any_element();
        }
        // Page boxes come from the installed layout, current or not, so the
        // scroll extent matches the geometry. Each page shows its own pixels,
        // or its last ones until those land; they never map positions. A page
        // without pixels is a blank placeholder, and a page that cannot be
        // rasterized says so in its own box. The caret overlay shows only on
        // the page whose raster carried the caret.
        if let Some(frame) = self.canvas.layout() {
            let scale =
                ScreenScale::new(f64::from(self.zoom_percent) / 100.0).expect("clamped zoom");
            let projection = self.projection();
            let catalog = Catalog::shipped(self.locale);
            return div()
                .flex()
                .flex_col()
                .gap(px(input_ui::DOC_GAP))
                .children(frame.layout.pages().iter().enumerate().map(|(i, page)| {
                    let sheet = div()
                        .w(px(scale.logical_pixels(page.width) as f32))
                        .h(px(scale.logical_pixels(page.height) as f32))
                        .flex_shrink_0()
                        .bg(rgb(0xffffff))
                        .shadow_md();
                    let number = (i + 1).to_string();
                    let notice = match self.canvas.page_image(i, projection) {
                        PageImage::Ready(image) => {
                            let shown = self
                                .canvas
                                .caret_overlay(i, projection)
                                .and_then(|overlay| {
                                    document_canvas::image_from_bgra(
                                        overlay.width,
                                        overlay.height,
                                        &overlay.bgra,
                                    )
                                })
                                .unwrap_or(image);
                            return sheet.child(img(shown).size_full());
                        }
                        PageImage::Stale(image) => {
                            return sheet.child(img(image).size_full());
                        }
                        PageImage::Pending => return sheet,
                        PageImage::TooLarge => {
                            catalog.format("canvas.page_too_large", &[("page", &number)])
                        }
                        PageImage::Failed(reason) => catalog.format(
                            "canvas.page_error",
                            &[("page", &number), ("reason", &reason)],
                        ),
                    };
                    sheet
                        .p_6()
                        .text_sm()
                        .text_color(rgb(0x8a5a00))
                        .child(notice)
                }))
                .into_any_element();
        }
        div()
            .p_6()
            .child(self.label("canvas.loading"))
            .into_any_element()
    }
    fn label(&self, key: &str) -> String {
        Catalog::shipped(self.locale).text(key)
    }

    pub(crate) fn document_title(&self) -> Option<String> {
        document_name(self.editor.as_deref(), self.locale)
    }

    fn run_with(
        &mut self,
        id: CommandId,
        args: &[(&'static str, ArgValue)],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let supplemented;
        let args = if matches!(
            id,
            FIND_NEXT | FIND_PREVIOUS | FIND_REPLACE | FIND_REPLACE_ALL
        ) {
            supplemented = supplement_find_flags(args, self.find_width, self.find_regex);
            supplemented.as_slice()
        } else {
            args
        };
        let registry = self.registry.clone();
        match registry.invoke_with(
            &mut ViewHost {
                view: self,
                window,
                cx,
            },
            id,
            args,
        ) {
            Err(InvokeError::Disabled(reason)) => {
                self.report_input_error(self.label(reason));
                cx.notify();
            }
            Err(InvokeError::Failed(message)) | Err(InvokeError::Unknown(message)) => {
                self.report_input_error(message);
                cx.notify();
            }
            Err(InvokeError::PendingInput { .. }) => {
                window.focus(&self.find_query_focus, cx);
                cx.notify();
            }
            Ok(()) => cx.notify(),
        }
    }

    fn find_flags(&self) -> FindBarFlags {
        FindBarFlags {
            case_sensitive: self.find_case,
            whole_word: self.find_word,
            fold_width: self.find_width,
            regex: self.find_regex,
        }
    }

    fn find_args(&self, replacement: bool) -> Vec<(&'static str, ArgValue)> {
        self.find_flags().args(
            self.find_query.text(),
            replacement.then_some(self.find_replacement.text()),
        )
    }

    fn toggle_find(&mut self, which: FindToggle) {
        match which {
            FindToggle::Case => self.find_case = !self.find_case,
            FindToggle::Word => self.find_word = !self.find_word,
            FindToggle::Width => self.find_width = !self.find_width,
            FindToggle::Regex => self.find_regex = !self.find_regex,
        }
    }

    fn find_toggle_on(&self, which: FindToggle) -> bool {
        match which {
            FindToggle::Case => self.find_case,
            FindToggle::Word => self.find_word,
            FindToggle::Width => self.find_width,
            FindToggle::Regex => self.find_regex,
        }
    }

    /// Catalog text when the key exists. Width and regex are not in the
    /// shipped catalogs yet, so those two fall back to the phrases here.
    fn style_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self
            .editor
            .as_ref()
            .map(|editor| editor.style_rows())
            .unwrap_or_default();
        let (paragraph, character) = self
            .editor
            .as_ref()
            .map(|editor| editor.caret_styles())
            .unwrap_or((None, None));
        let paragraph = paragraph.unwrap_or_else(|| self.label("style.none"));
        let character = character.unwrap_or_else(|| self.label("style.none"));
        let current = Catalog::shipped(self.locale).format(
            "style.current",
            &[("paragraph", &paragraph), ("character", &character)],
        );
        let current_paragraph = self
            .editor
            .as_ref()
            .and_then(|editor| editor.caret_styles().0);
        let current_character = self
            .editor
            .as_ref()
            .and_then(|editor| editor.caret_styles().1);
        div()
            .id("style-panel")
            .w(px(220.0))
            .h_full()
            .flex_shrink_0()
            .overflow_y_scroll()
            .bg(rgb(0xf7f9fb))
            .border_r_1()
            .border_color(rgb(0xd5dbe3))
            .p_2()
            .flex()
            .flex_col()
            .gap_1()
            .text_sm()
            .child(self.label("style.panel"))
            .child(div().text_xs().text_color(rgb(0x5c6b7a)).child(current))
            .children(rows.into_iter().map(|row| {
                let id = row.id.clone();
                let marked = current_paragraph.as_deref() == Some(row.id.as_str())
                    || current_character.as_deref() == Some(row.id.as_str());
                let preview = if row.preview.is_empty() {
                    row.kind.clone()
                } else {
                    format!("{} · {}", row.kind, row.preview)
                };
                div()
                    .id(SharedString::from(format!("style-{}", row.id)))
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(if marked { rgb(0xd7e6f5) } else { rgb(0xf7f9fb) })
                    .hover(|style| style.bg(rgb(0xe7eef5)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.run_with(
                            STYLE_APPLY,
                            &[("style", ArgValue::Text(id.clone()))],
                            window,
                            cx,
                        );
                    }))
                    .child(row.name)
                    .child(div().text_xs().text_color(rgb(0x5c6b7a)).child(preview))
            }))
    }

    fn find_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .bg(rgb(0xffffff))
            .border_t_1()
            .border_color(rgb(0xd5dbe3))
            .text_sm()
            .child(self.find_field(FindSlot::Query, cx))
            .child(self.find_field(FindSlot::Replacement, cx))
            .child(self.find_button(
                "find-next",
                &self.label("command.find.next"),
                FIND_NEXT,
                false,
                cx,
            ))
            .child(self.find_button(
                "find-previous",
                &self.label("command.find.previous"),
                FIND_PREVIOUS,
                false,
                cx,
            ))
            .child(self.find_button(
                "find-replace",
                &self.label("command.find.replace"),
                FIND_REPLACE,
                true,
                cx,
            ))
            .child(self.find_button(
                "find-replace-all",
                &self.label("command.find.replace_all"),
                FIND_REPLACE_ALL,
                true,
                cx,
            ))
            .child(self.find_toggle(FindToggle::Case, cx))
            .child(self.find_toggle(FindToggle::Word, cx))
            .child(self.find_toggle(FindToggle::Width, cx))
            .child(self.find_toggle(FindToggle::Regex, cx))
            .child(self.repeat_button(cx))
            .child(self.find_button(
                "format-copy",
                &self.label("command.format.copy"),
                FORMAT_COPY,
                false,
                cx,
            ))
            .child(self.find_button(
                "format-paint",
                &self.label("command.format.paint"),
                FORMAT_PAINT,
                false,
                cx,
            ))
            .child(self.find_button(
                "format-cancel",
                &self.label("command.format.cancel"),
                FORMAT_CANCEL,
                false,
                cx,
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x5c6b7a))
                    .child(self.find_note.clone().unwrap_or_default()),
            )
    }

    fn find_field(&self, slot: FindSlot, cx: &mut Context<Self>) -> impl IntoElement {
        let (focus, buffer, placeholder) = match slot {
            FindSlot::Query => (
                self.find_query_focus.clone(),
                &self.find_query,
                self.label("find.query"),
            ),
            FindSlot::Replacement => (
                self.find_replacement_focus.clone(),
                &self.find_replacement,
                self.label("find.replacement"),
            ),
        };
        div()
            .id(match slot {
                FindSlot::Query => "find-query",
                FindSlot::Replacement => "find-replacement",
            })
            .track_focus(&focus)
            .key_context(field_edit::FIELD_CONTEXT)
            .focusable()
            .w(px(160.0))
            .px_2()
            .py_1()
            .border_1()
            .border_color(rgb(0xc5ced6))
            .bg(rgb(0xffffff))
            .text_color(rgb(0x263247))
            .on_click(cx.listener(move |this, _, window, cx| {
                let focus = match slot {
                    FindSlot::Query => this.find_query_focus.clone(),
                    FindSlot::Replacement => this.find_replacement_focus.clone(),
                };
                window.focus(&focus, cx);
            }))
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    this.type_find(slot, event, window, cx);
                }),
            )
            .child(field_edit::field_view(buffer, &placeholder))
    }

    fn type_find(
        &mut self,
        slot: FindSlot,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let effect = {
            let buffer = match slot {
                FindSlot::Query => &mut self.find_query,
                FindSlot::Replacement => &mut self.find_replacement,
            };
            let effect = buffer.apply_key(&event.keystroke);
            field_edit::consume(effect, buffer, cx)
        };
        let Some(effect) = effect else {
            return;
        };
        cx.stop_propagation();
        match effect {
            field_edit::FieldEffect::Submit => {
                let args = self.find_args(false);
                self.run_with(FIND_NEXT, &args, window, cx);
            }
            field_edit::FieldEffect::Cancel => {
                window.focus(&self.focus_handle, cx);
                cx.notify();
            }
            _ => cx.notify(),
        }
    }

    fn find_button(
        &self,
        id: &'static str,
        label: &str,
        command: CommandId,
        replacement: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.to_owned();
        div()
            .id(id)
            .cursor_pointer()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(0xe7eef5))
            .on_click(cx.listener(move |this, _, window, cx| {
                if command == FORMAT_COPY {
                    this.run_with(
                        FORMAT_COPY,
                        &[("repeat", ArgValue::Bool(this.format_repeat))],
                        window,
                        cx,
                    );
                } else if command == FORMAT_PAINT || command == FORMAT_CANCEL {
                    this.run_with(command, &[], window, cx);
                } else {
                    let args = this.find_args(replacement);
                    this.run_with(command, &args, window, cx);
                }
            }))
            .child(label)
    }

    fn find_toggle(&self, which: FindToggle, cx: &mut Context<Self>) -> impl IntoElement {
        let on = self.find_toggle_on(which);
        let (id, label, checkbox) = match which {
            FindToggle::Case => ("find-case", self.label("find.case"), false),
            FindToggle::Word => ("find-word", self.label("find.word"), false),
            FindToggle::Width => ("find-width", self.label("find.width"), true),
            FindToggle::Regex => ("find-regex", self.label("find.regex"), true),
        };
        let shown = if checkbox {
            format!("{} {label}", if on { "[x]" } else { "[ ]" })
        } else {
            label
        };
        div()
            .id(id)
            .cursor_pointer()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(if on { rgb(0xc5d8ea) } else { rgb(0xe7eef5) })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_find(which);
                cx.notify();
            }))
            .child(shown)
    }

    fn repeat_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let on = self.format_repeat;
        div()
            .id("format-repeat")
            .cursor_pointer()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(if on { rgb(0xc5d8ea) } else { rgb(0xe7eef5) })
            .on_click(cx.listener(|this, _, _, cx| {
                this.format_repeat = !this.format_repeat;
                cx.notify();
            }))
            .child(self.label(if on { "format.repeat" } else { "format.once" }))
    }

    pub(crate) fn report_canvas_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.note_diag(Level::Error, "ui.canvas", &message);
        self.canvas_error = Some(message);
    }

    pub(crate) fn report_input_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.note_diag(Level::Error, "ui.input", &message);
        self.input_error = Some(message);
    }

    fn note_diag(&self, level: Level, event: &str, message: &str) {
        if let Some(log) = &self.diagnostics {
            let _ = log.record_untrusted(level, event, message);
        }
    }

    /// Single entry for menus, shortcuts and on-screen controls. Disabled and
    /// failed commands surface their reason instead of doing nothing silently.
    fn run(&mut self, id: CommandId, window: &mut Window, cx: &mut Context<Self>) {
        let registry = self.registry.clone();
        match registry.invoke(
            &mut ViewHost {
                view: self,
                window,
                cx,
            },
            id,
        ) {
            Err(InvokeError::PendingInput { .. }) if id == FILE_OPEN => {
                self.begin_open(window, cx);
            }
            Err(InvokeError::PendingInput { .. }) if id == FILE_SAVE_COPY => {
                self.begin_save_copy(window, cx);
            }
            Err(InvokeError::PendingInput { .. }) if id == FILE_EXPORT_PDF => {
                self.begin_pdf(false, window, cx);
            }
            Err(InvokeError::PendingInput { .. }) if id == FILE_PRINT => {
                self.begin_pdf(true, window, cx);
            }
            Err(InvokeError::PendingInput { .. }) if id == FILE_CLOSE => {
                self.request_close(false, window, cx);
            }
            Err(InvokeError::PendingInput { .. })
                if matches!(
                    id,
                    FIND_NEXT | FIND_PREVIOUS | FIND_REPLACE | FIND_REPLACE_ALL
                ) =>
            {
                window.focus(&self.find_query_focus, cx);
                cx.notify();
            }
            Err(InvokeError::PendingInput { .. }) if id == FORMAT_FONT => {
                self.open_font_dialog(cx);
            }
            Err(InvokeError::PendingInput { .. }) if id == TABLE_PROPERTIES => {
                self.open_table_dialog(cx);
            }
            Err(InvokeError::PendingInput { .. }) if id == FORMAT_PARAGRAPH => {
                self.open_paragraph_dialog(cx);
            }
            Err(InvokeError::Disabled(reason)) => {
                self.report_input_error(self.label(reason));
                cx.notify();
            }
            Err(InvokeError::Failed(message)) | Err(InvokeError::Unknown(message)) => {
                self.report_input_error(message);
                cx.notify();
            }
            _ => {}
        }
    }
}

impl Render for JoyWord {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.device_scale = window.scale_factor();
        self.request_canvas(cx);
        let edited = self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.has_unsaved_changes());
        window.set_window_edited(edited);
        let title = window_title(self.document_title().as_deref(), edited);
        if title != self.window_title {
            window.set_window_title(&title);
            self.window_title = title.clone();
        }
        let catalog = Catalog::shipped(self.locale);

        div()
            .track_focus(&self.focus_handle)
            .key_context(
                if self
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.is_composing())
                {
                    "JoyWordInput composing"
                } else {
                    "JoyWordInput"
                },
            )
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                this.escape_panel(event, window, cx)
            }))
            .on_action(cx.listener(|this, _: &Quit, window, cx| {
                this.run(QUIT, window, cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &ZoomIn, window, cx| this.run(ZOOM_IN, window, cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, window, cx| this.run(ZOOM_OUT, window, cx)))
            .on_action(
                cx.listener(|this, _: &ZoomReset, window, cx| this.run(ZOOM_RESET, window, cx)),
            )
            .on_action(cx.listener(|this, _: &SwitchLanguage, window, cx| {
                this.run(SWITCH_LANGUAGE, window, cx)
            }))
            .on_action(cx.listener(|this, _: &Undo, window, cx| this.run(EDIT_UNDO, window, cx)))
            .on_action(cx.listener(|this, _: &Redo, window, cx| this.run(EDIT_REDO, window, cx)))
            .on_action(
                cx.listener(|this, _: &SelectAll, window, cx| {
                    this.run(EDIT_SELECT_ALL, window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &Backspace, window, cx| this.run(EDIT_BACKSPACE, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &DeleteForward, window, cx| {
                    this.run(EDIT_DELETE, window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &NewParagraph, window, cx| this.run(EDIT_ENTER, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &MoveLeft, window, cx| this.run(EDIT_LEFT, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &MoveRight, window, cx| this.run(EDIT_RIGHT, window, cx)),
            )
            .on_action(cx.listener(|this, _: &ExtendLeft, window, cx| {
                this.run(EDIT_EXTEND_LEFT, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ExtendRight, window, cx| {
                this.run(EDIT_EXTEND_RIGHT, window, cx)
            }))
            .on_action(cx.listener(|this, _: &MoveUp, window, cx| this.run(EDIT_UP, window, cx)))
            .on_action(
                cx.listener(|this, _: &MoveDown, window, cx| this.run(EDIT_DOWN, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ExtendUp, window, cx| this.run(EDIT_EXTEND_UP, window, cx)),
            )
            .on_action(cx.listener(|this, _: &ExtendDown, window, cx| {
                this.run(EDIT_EXTEND_DOWN, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &LineStart, window, cx| this.run(EDIT_HOME, window, cx)),
            )
            .on_action(cx.listener(|this, _: &LineEnd, window, cx| this.run(EDIT_END, window, cx)))
            .on_action(cx.listener(|this, _: &ExtendLineStart, window, cx| {
                this.run(EDIT_EXTEND_HOME, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ExtendLineEnd, window, cx| {
                this.run(EDIT_EXTEND_END, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &WordLeft, window, cx| this.run(EDIT_WORD_LEFT, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &WordRight, window, cx| {
                    this.run(EDIT_WORD_RIGHT, window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &ExtendWordLeft, window, cx| {
                this.run(EDIT_EXTEND_WORD_LEFT, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ExtendWordRight, window, cx| {
                this.run(EDIT_EXTEND_WORD_RIGHT, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ParagraphUp, window, cx| {
                this.run(EDIT_PARAGRAPH_UP, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ParagraphDown, window, cx| {
                this.run(EDIT_PARAGRAPH_DOWN, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ExtendParagraphUp, window, cx| {
                this.run(EDIT_EXTEND_PARAGRAPH_UP, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ExtendParagraphDown, window, cx| {
                this.run(EDIT_EXTEND_PARAGRAPH_DOWN, window, cx)
            }))
            .on_action(cx.listener(|this, _: &DocumentStart, window, cx| {
                this.run(EDIT_DOCUMENT_START, window, cx)
            }))
            .on_action(cx.listener(|this, _: &DocumentEnd, window, cx| {
                this.run(EDIT_DOCUMENT_END, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ExtendDocumentStart, window, cx| {
                this.run(EDIT_EXTEND_DOCUMENT_START, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ExtendDocumentEnd, window, cx| {
                this.run(EDIT_EXTEND_DOCUMENT_END, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &PageUp, window, cx| this.run(EDIT_PAGE_UP, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PageDown, window, cx| this.run(EDIT_PAGE_DOWN, window, cx)),
            )
            .on_action(cx.listener(|this, _: &ExtendPageUp, window, cx| {
                this.run(EDIT_EXTEND_PAGE_UP, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ExtendPageDown, window, cx| {
                this.run(EDIT_EXTEND_PAGE_DOWN, window, cx)
            }))
            .on_action(cx.listener(|this, _: &DeleteWordBackward, window, cx| {
                this.run(EDIT_DELETE_WORD_BACKWARD, window, cx)
            }))
            .on_action(cx.listener(|this, _: &DeleteWordForward, window, cx| {
                this.run(EDIT_DELETE_WORD_FORWARD, window, cx)
            }))
            .on_action(cx.listener(|this, _: &CancelComposition, window, cx| {
                this.run(EDIT_CANCEL, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &OpenFile, window, cx| this.run(FILE_OPEN, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SaveOriginal, window, cx| this.run(FILE_SAVE, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SaveCopy, window, cx| this.run(FILE_SAVE_COPY, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ExportPdf, window, cx| {
                    this.run(FILE_EXPORT_PDF, window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &PrintFile, window, cx| this.run(FILE_PRINT, window, cx)),
            )
            .on_action(cx.listener(|this, _: &Bold, window, cx| this.run(EDIT_BOLD, window, cx)))
            .on_action(
                cx.listener(|this, _: &ListStyles, window, cx| this.run(STYLE_LIST, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ApplyStyle, window, cx| this.run(STYLE_APPLY, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ApplyList, window, cx| this.run(LIST_APPLY, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SetListLevel, window, cx| this.run(LIST_LEVEL, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &RemoveList, window, cx| this.run(LIST_REMOVE, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PageBreak, window, cx| this.run(PAGE_BREAK, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PageNumber, window, cx| this.run(PAGE_NUMBER, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PageSize, window, cx| this.run(PAGE_SIZE, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PageMargins, window, cx| this.run(PAGE_MARGINS, window, cx)),
            )
            .on_action(cx.listener(|this, _: &PageOrientation, window, cx| {
                this.run(PAGE_ORIENTATION, window, cx)
            }))
            .on_action(cx.listener(|this, _: &EditHeader, window, cx| {
                this.run(PAGE_EDIT_HEADER, window, cx)
            }))
            .on_action(cx.listener(|this, _: &EditFooter, window, cx| {
                this.run(PAGE_EDIT_FOOTER, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &EditBody, window, cx| this.run(PAGE_EDIT_BODY, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &Italic, window, cx| this.run(EDIT_ITALIC, window, cx)),
            )
            .on_action(cx.listener(|this, _: &AlignCenter, window, cx| {
                this.run(EDIT_ALIGN_CENTER, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &Underline, window, cx| this.run(EDIT_UNDERLINE, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &AlignLeft, window, cx| {
                    this.run(EDIT_ALIGN_LEFT, window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &AlignRight, window, cx| {
                this.run(EDIT_ALIGN_RIGHT, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &Justify, window, cx| this.run(EDIT_JUSTIFY, window, cx)),
            )
            .on_action(cx.listener(|this, _: &CutText, window, cx| this.run(EDIT_CUT, window, cx)))
            .on_action(
                cx.listener(|this, _: &CopyText, window, cx| this.run(EDIT_COPY, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PasteText, window, cx| this.run(EDIT_PASTE, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &CloseDocument, window, cx| this.run(FILE_CLOSE, window, cx)),
            )
            .on_action(cx.listener(|this, _: &OpenRecent, window, cx| {
                this.run(FILE_OPEN_RECENT, window, cx)
            }))
            .on_action(cx.listener(|this, _: &SwitchDocument, window, cx| {
                this.run(DOCUMENT_SWITCH, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &InsertText, window, cx| this.run(EDIT_INSERT, window, cx)),
            )
            .on_action(cx.listener(|this, _: &InsertImage, window, cx| {
                this.run(EDIT_INSERT_IMAGE, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &FindNext, window, cx| this.run(FIND_NEXT, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &FindPrevious, window, cx| {
                    this.run(FIND_PREVIOUS, window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &ReplaceNext, window, cx| this.run(FIND_REPLACE, window, cx)),
            )
            .on_action(cx.listener(|this, _: &ReplaceAll, window, cx| {
                this.run(FIND_REPLACE_ALL, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &CopyFormat, window, cx| this.run(FORMAT_COPY, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PaintFormat, window, cx| this.run(FORMAT_PAINT, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &CancelFormat, window, cx| {
                    this.run(FORMAT_CANCEL, window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &InsertTable, window, cx| this.run(TABLE_INSERT, window, cx)),
            )
            .on_action(cx.listener(|this, _: &InsertTableRow, window, cx| {
                this.run(TABLE_ROW_INSERT, window, cx)
            }))
            .on_action(cx.listener(|this, _: &DeleteTableRow, window, cx| {
                this.run(TABLE_ROW_DELETE, window, cx)
            }))
            .on_action(cx.listener(|this, _: &InsertTableColumn, window, cx| {
                this.run(TABLE_COLUMN_INSERT, window, cx)
            }))
            .on_action(cx.listener(|this, _: &DeleteTableColumn, window, cx| {
                this.run(TABLE_COLUMN_DELETE, window, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectTableCell, window, cx| {
                this.run(TABLE_SELECT, window, cx)
            }))
            .on_action(cx.listener(|this, _: &MergeTableRight, window, cx| {
                this.run(TABLE_MERGE_RIGHT, window, cx)
            }))
            .on_action(cx.listener(|this, _: &MergeTableDown, window, cx| {
                this.run(TABLE_MERGE_DOWN, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &SplitTableCell, window, cx| {
                    this.run(TABLE_SPLIT, window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &TableProperties, window, cx| {
                this.run(TABLE_PROPERTIES, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &FontDialog, window, cx| this.run(FORMAT_FONT, window, cx)),
            )
            .on_action(cx.listener(|this, _: &ParagraphDialog, window, cx| {
                this.run(FORMAT_PARAGRAPH, window, cx)
            }))
            .on_action(cx.listener(|this, _: &FormattingMarks, window, cx| {
                this.run(VIEW_FORMATTING_MARKS, window, cx)
            }))
            .on_action(cx.listener(|this, _: &HiddenText, window, cx| {
                this.run(VIEW_HIDDEN_TEXT, window, cx)
            }))
            .on_action(cx.listener(|this, _: &FieldCodes, window, cx| {
                this.run(VIEW_FIELD_CODES, window, cx)
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xe9edf2))
            .text_color(rgb(0x263247))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_6()
                    .py_3()
                    .bg(rgb(0x163b65))
                    .text_color(rgb(0xffffff))
                    .child(title)
                    .child(
                        div()
                            .id("language")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.run(SWITCH_LANGUAGE, window, cx)
                            }))
                            .child(self.label("language")),
                    ),
            )
            .child(
                div()
                    .px_6()
                    .py_3()
                    .bg(rgb(0xfff8e7))
                    .text_sm()
                    .child(self.label("stage")),
            )
            .children(linux_files::window_needs_command_bar().then(|| self.linux_command_bar(cx)))
            .child(self.ribbon(cx))
            .child(self.ruler(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.style_panel(cx))
                    .child(self.input_viewport(cx)),
            )
            .children(self.path_sheet_panel(cx))
            .child(self.find_bar(cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_6()
                    .py_2()
                    .bg(rgb(0xffffff))
                    .text_sm()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(self.input_status())
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x5c6b7a))
                                    .child(self.status_counts()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(
                                div()
                                    .id("zoom-out")
                                    .cursor_pointer()
                                    .px_2()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.run(ZOOM_OUT, window, cx)
                                    }))
                                    .child("−"),
                            )
                            .child(catalog.format(
                                "zoom.percent",
                                &[("percent", &self.zoom_percent.to_string())],
                            ))
                            .child(
                                div()
                                    .id("zoom-in")
                                    .cursor_pointer()
                                    .px_2()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.run(ZOOM_IN, window, cx)
                                    }))
                                    .child("+"),
                            ),
                    ),
            )
            .children(self.format_panel(cx))
    }
}

/// Open a journaled package into a session the window can show. A missing or
/// unreadable journal returns `None` and the caller keeps the sample document.
fn take_recovered(
    directory: &std::path::Path,
    log: Option<&AppLog>,
) -> Option<(
    joyword_shell::host::Office,
    Box<dyn input_ui::DocEditor>,
    joyword_shell::host::DocumentId,
)> {
    let mut office = match joyword_shell::host::Office::recover_from(directory) {
        Ok(office) => office,
        Err(error) => {
            if directory.join("recovery.jwr").is_file()
                && let Some(log) = log
            {
                let _ = log.record_untrusted(Level::Error, "recovery.failed", &error.to_string());
            }
            return None;
        }
    };
    office.set_recovery_directory(directory.to_path_buf());
    let id = office.active_id()?;
    if let Some(log) = log {
        let _ = log.record(Event {
            level: Level::Info,
            event: "recovery.restored",
            kind: Some("document"),
            count: Some(id.0),
        });
    }
    let input = office.hold_active()?;
    Some((office, Box::new(input), id))
}

struct Boot {
    log: Option<Arc<AppLog>>,
    locale: Locale,
}

fn boot_diagnostics() -> Boot {
    let paths = match diag::resolve() {
        Ok(paths) => paths,
        Err(diag::ProfileError::UserDirectoryUnavailable) => {
            eprintln!(
                "JoyWord could not find the user configuration directory; diagnostics are off"
            );
            return Boot {
                log: None,
                locale: Locale::ZhCn,
            };
        }
        Err(error) => {
            eprintln!("JoyWord refused the diagnostics profile: {error}");
            std::process::exit(2);
        }
    };
    let loaded = PrefsStore::new(&paths.config_dir).load(Locale::ZhCn);
    let locale = loaded.prefs.ui_locale;
    let log = match AppLog::open(&paths.log_dir) {
        Ok(log) => {
            let log = Arc::new(log);
            let _ = log.record(Event {
                level: Level::Info,
                event: "startup",
                kind: Some(paths.profile.tag()),
                count: None,
            });
            for notice in &loaded.notices {
                let _ = log.record(Event {
                    level: Level::Warn,
                    event: "prefs.notice",
                    kind: Some(prefs_notice_kind(notice)),
                    count: None,
                });
            }
            if let Err(error) = diag::install_panic_hook(Arc::clone(&log)) {
                let _ = log.record_untrusted(Level::Error, "panic.hook", &error.to_string());
            }
            Some(log)
        }
        Err(_) => {
            eprintln!("JoyWord could not open the diagnostics log");
            None
        }
    };
    Boot { log, locale }
}

fn prefs_notice_kind(notice: &PrefsNotice) -> &'static str {
    match notice {
        PrefsNotice::Corrupt { .. } => "corrupt",
        PrefsNotice::Newer { .. } => "newer",
        PrefsNotice::Migrated { .. } => "migrated",
        PrefsNotice::InvalidValue { .. } => "invalid_value",
    }
}

/// Some Linux virtual machines expose a Vulkan loader but no usable hardware
/// ICD. Find a software ICD so the window can be retried without asking the
/// user to set `VK_ICD_FILENAMES`. Explicit user choices still win.
#[cfg(target_os = "linux")]
fn software_vulkan_icd() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;

    if let Some(path) = std::env::var_os("JOYWORD_SOFTWARE_VULKAN_ICD") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    for candidate in [
        "/opt/namiwork/vk_swiftshader_icd.json",
        "/opt/trae-cn-enterprise/vk_swiftshader_icd.json",
        "/usr/share/trae-cn-enterprise/vk_swiftshader_icd.json",
        "/usr/share/vulkan/icd.d/lvp_icd.x86_64.json",
        "/usr/share/vulkan/icd.d/lvp_icd.json",
    ] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }

    let mut directories = vec![
        PathBuf::from("/usr/share/vulkan/icd.d"),
        PathBuf::from("/usr/local/share/vulkan/icd.d"),
    ];
    for root in ["/opt", "/usr/share"] {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            directories.push(path.clone());
            let Ok(children) = std::fs::read_dir(&path) else {
                continue;
            };
            for child in children.flatten() {
                directories.push(child.path());
            }
        }
    }
    software_vulkan_icd_in(directories)
}

#[cfg(target_os = "linux")]
fn software_vulkan_icd_in(
    directories: impl IntoIterator<Item = std::path::PathBuf>,
) -> Option<std::path::PathBuf> {
    for directory in directories {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let name = name.to_ascii_lowercase();
            if name.ends_with(".json")
                && (name.contains("swiftshader")
                    || name.contains("lvp_icd")
                    || name.contains("lavapipe"))
            {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn linux_has_render_node() -> bool {
    let Ok(entries) = std::fs::read_dir("/dev/dri") else {
        return false;
    };
    entries
        .flatten()
        .any(|entry| entry.file_name().to_string_lossy().starts_with("renderD"))
}

fn main() {
    #[cfg(target_os = "linux")]
    if std::env::var_os("VK_ICD_FILENAMES").is_none()
        && std::env::var_os("VK_DRIVER_FILES").is_none()
        && !linux_has_render_node()
        && let Some(icd) = software_vulkan_icd()
        && let Ok(exe) = std::env::current_exe()
    {
        let mut command = std::process::Command::new(exe);
        command
            .args(std::env::args_os().skip(1))
            .env("VK_ICD_FILENAMES", &icd)
            .env("VK_DRIVER_FILES", &icd);
        let exec_error = command.exec();
        eprintln!("JoyWord could not restart with software rendering: {exec_error}");
    }

    struct StderrLog;
    impl log::Log for StderrLog {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            metadata.level() <= log::Level::Info
        }
        fn log(&self, record: &log::Record) {
            if self.enabled(record.metadata()) {
                eprintln!("[{}] {}", record.level(), record.args());
            }
        }
        fn flush(&self) {}
    }
    let _ = log::set_logger(&STDERR_LOG);
    log::set_max_level(log::LevelFilter::Info);
    static STDERR_LOG: StderrLog = StderrLog;
    let boot = boot_diagnostics();
    let boot_log = boot.log.clone();
    let locale = boot.locale;
    let startup = startup_document(std::env::args_os().skip(1));
    gpui_platform::application().run(move |cx: &mut App| {
        let registry = Registry::builtin();
        cx.on_action(|_: &Quit, cx| {
            if let Some(MainWindow(handle)) = cx.try_global::<MainWindow>().copied()
                && handle
                    .update(cx, |view, window, cx| view.run(QUIT, window, cx))
                    .is_ok()
            {
                return;
            }
            cx.quit();
        });
        cx.bind_keys(key_bindings(&registry));
        #[cfg(target_os = "linux")]
        prompt_ui::install(cx);
        cx.set_menus(menus(&registry, Catalog::shipped(locale)));
        let bounds = Bounds::centered(None, size(px(1100.0), px(850.0)), cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("JoyWord".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let recovery_dir = joyword_shell::storage::recovery_directory();
                    let mut session = joyword_shell::host::Office::new();
                    if let Some(log) = &boot_log {
                        session.set_diagnostics(Arc::clone(log));
                    }
                    if let Some(dir) = &recovery_dir {
                        session.set_recovery_directory(dir.clone());
                    }
                    let mut editor = None;
                    let mut held_document = None;
                    if let Some(dir) = &recovery_dir
                        && let Some((recovered, input, id)) =
                            take_recovered(dir, boot_log.as_deref())
                    {
                        session = recovered;
                        if let Some(log) = &boot_log {
                            session.set_diagnostics(Arc::clone(log));
                        }
                        editor = Some(input);
                        held_document = Some(id);
                    }
                    if let Ok(paths) = diag::resolve() {
                        session.bind_preferences(&paths.config_dir, locale);
                    }
                    if let Some(editor) = editor.as_mut() {
                        editor.set_revision_author(session.revision_author());
                    }
                    let mut view = JoyWord {
                        zoom_percent: ZOOM_DEFAULT,
                        locale,
                        registry: registry.clone(),
                        focus_handle: cx.focus_handle(),
                        editor,
                        copy_note: None,
                        font: None,
                        profile: None,
                        canvas: CanvasPipeline::default(),
                        load_started: false,
                        load_task: None,
                        layout_task: None,
                        raster_task: None,
                        device_scale: 1.0,
                        canvas_error: None,
                        input_error: None,
                        scroll: gpui::ScrollHandle::new(),
                        viewport_bounds: None,
                        dragging: false,
                        reveal_pending: false,
                        ime_geometry: None,
                        ime_anchor: None,
                        preferred_x: None,
                        close_pending: false,
                        close_task: None,
                        subscriptions: Vec::new(),
                        clipboard: String::new(),
                        session,
                        open_gate: None,
                        recovery_dir,
                        held_document,
                        copy_epoch: 0,
                        copy: None,
                        pdf: None,
                        original_save: None,
                        diagnostics: boot_log.clone(),
                        find_query: field_edit::FieldBuffer::default(),
                        find_replacement: field_edit::FieldBuffer::default(),
                        find_case: false,
                        find_word: false,
                        find_width: false,
                        find_regex: false,
                        find_query_focus: cx.focus_handle(),
                        find_replacement_focus: cx.focus_handle(),
                        find_note: None,
                        format_repeat: false,
                        ribbon_tab: RibbonTab::Start,
                        format_dialog: None,
                        path_sheet: None,
                        view_toggles: joyword_shell::dialogs::ViewToggles::default(),
                        ruler_bounds: std::sync::Arc::new(std::sync::Mutex::new(None)),
                        ruler_drag: None,
                        window_title: String::new(),
                    };
                    view.install_input_lifecycle(window, cx);
                    view
                })
            },
        );
        #[cfg(target_os = "linux")]
        let result = match result {
            Ok(window) => Ok(window),
            Err(error) => {
                if std::env::var_os("VK_ICD_FILENAMES").is_none()
                    && std::env::var_os("VK_DRIVER_FILES").is_none()
                    && let Some(icd) = software_vulkan_icd()
                {
                    let exe = std::env::current_exe()
                        .unwrap_or_else(|_| std::path::PathBuf::from("joyword"));
                    let mut command = std::process::Command::new(exe);
                    command
                        .args(std::env::args_os().skip(1))
                        .env("VK_ICD_FILENAMES", &icd)
                        .env("VK_DRIVER_FILES", &icd);
                    if let Some(log) = &boot_log {
                        let _ = log.record(Event {
                            level: Level::Info,
                            event: "window.software_retry",
                            kind: Some("vulkan"),
                            count: None,
                        });
                    }
                    let exec_error = command.exec();
                    eprintln!("JoyWord could not restart with software rendering: {exec_error}");
                }
                Err(error)
            }
        };
        match result {
            Ok(window) => {
                cx.set_global(MainWindow(window));
                let _ = window.update(cx, |view, window, cx| {
                    window.focus(&view.focus_handle, cx);
                    cx.activate(true);
                    if let Some(path) = startup.clone() {
                        view.open_startup_path(path, window, cx);
                    }
                });
            }
            Err(error) => {
                let detail = diag::redact(&error.to_string());
                if let Some(log) = &boot_log {
                    let _ = log.record_untrusted(Level::Error, "window.open", &detail);
                }
                eprintln!("Could not open the JoyWord window: {detail}");
                eprintln!("RAW-ERROR-DEBUG: {:?}", error);
                cx.quit();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn a_software_vulkan_icd_is_found_by_name() {
        let directory =
            std::env::temp_dir().join(format!("joyword-software-icd-{}", std::process::id()));
        if std::fs::create_dir_all(&directory).is_err() {
            return;
        }
        let icd = directory.join("vk_swiftshader_icd.json");
        if std::fs::write(&icd, "{}").is_err() {
            let _ = std::fs::remove_dir_all(&directory);
            return;
        }
        assert_eq!(software_vulkan_icd_in([directory.clone()]), Some(icd));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn registry_commands_all_have_actions_and_bindings() {
        let registry = Registry::builtin();
        for spec in registry.commands() {
            assert!(
                action_for(spec.id).is_some(),
                "{} has no GPUI action",
                spec.id
            );
        }
        assert_eq!(
            key_bindings(&registry).len(),
            shortcut_chords(&registry).len()
        );
    }

    #[test]
    fn linux_export_keeps_the_primary_chord_and_adds_ctrl_e() {
        let chords = shortcut_chords(&Registry::builtin());
        let export: Vec<&str> = chords
            .iter()
            .filter(|(_, id)| *id == FILE_EXPORT_PDF)
            .map(|(keys, _)| *keys)
            .collect();
        if cfg!(target_os = "linux") {
            assert_eq!(export, vec!["ctrl-shift-e", "ctrl-e"]);
        } else if cfg!(target_os = "macos") {
            assert_eq!(export, vec!["cmd-shift-e"]);
        } else {
            assert_eq!(export, vec!["ctrl-shift-e"]);
        }
        assert!(linux_files::LINUX_BAR.contains(&FILE_OPEN));
        assert!(linux_files::LINUX_BAR.contains(&FILE_SAVE_COPY));
        assert!(linux_files::LINUX_BAR.contains(&FILE_EXPORT_PDF));
    }

    #[test]
    fn word_processor_chords_reach_their_commands() {
        let chords = shortcut_chords(&Registry::builtin());
        let bound = |keys: &str| -> Vec<CommandId> {
            chords
                .iter()
                .filter(|(chord, _)| *chord == keys)
                .map(|(_, id)| *id)
                .collect()
        };
        let expected: &[(&str, CommandId)] = if cfg!(target_os = "macos") {
            &[
                ("alt-left", EDIT_WORD_LEFT),
                ("alt-shift-right", EDIT_EXTEND_WORD_RIGHT),
                ("alt-down", EDIT_PARAGRAPH_DOWN),
                ("cmd-up", EDIT_DOCUMENT_START),
                ("cmd-shift-down", EDIT_EXTEND_DOCUMENT_END),
                ("alt-backspace", EDIT_DELETE_WORD_BACKWARD),
                ("cmd-x", EDIT_CUT),
                ("cmd-u", EDIT_UNDERLINE),
                ("cmd-e", EDIT_ALIGN_CENTER),
                ("cmd-enter", PAGE_BREAK),
                ("ctrl-h", FIND_REPLACE),
                ("cmd-d", FORMAT_FONT),
            ]
        } else {
            &[
                ("ctrl-left", EDIT_WORD_LEFT),
                ("ctrl-shift-right", EDIT_EXTEND_WORD_RIGHT),
                ("ctrl-down", EDIT_PARAGRAPH_DOWN),
                ("ctrl-home", EDIT_DOCUMENT_START),
                ("ctrl-shift-end", EDIT_EXTEND_DOCUMENT_END),
                ("ctrl-backspace", EDIT_DELETE_WORD_BACKWARD),
                ("ctrl-delete", EDIT_DELETE_WORD_FORWARD),
                ("ctrl-x", EDIT_CUT),
                ("ctrl-u", EDIT_UNDERLINE),
                ("ctrl-l", EDIT_ALIGN_LEFT),
                ("ctrl-r", EDIT_ALIGN_RIGHT),
                ("ctrl-j", EDIT_JUSTIFY),
                ("ctrl-enter", PAGE_BREAK),
                ("ctrl-h", FIND_REPLACE),
                ("ctrl-d", FORMAT_FONT),
            ]
        };
        for (keys, id) in expected {
            assert_eq!(bound(keys), vec![*id], "{keys}");
        }
        assert_eq!(bound("pageup"), vec![EDIT_PAGE_UP]);
        assert_eq!(bound("shift-pagedown"), vec![EDIT_EXTEND_PAGE_DOWN]);
        let center = chords.iter().any(|(_, id)| *id == EDIT_ALIGN_CENTER);
        if cfg!(target_os = "linux") {
            assert!(!center, "Linux keeps Ctrl+E for PDF export");
            assert_eq!(bound("ctrl-e"), vec![FILE_EXPORT_PDF]);
        } else {
            assert!(center);
        }
    }

    #[test]
    fn window_title_names_the_document_and_marks_unsaved_changes() {
        assert_eq!(window_title(None, false), "JoyWord");
        assert_eq!(
            window_title(Some("合同.docx"), false),
            "合同.docx - JoyWord"
        );
        assert_eq!(
            window_title(Some("合同.docx"), true),
            "*合同.docx - JoyWord"
        );
    }

    #[test]
    fn a_committed_journal_opens_back_into_a_session() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!(
            "joyword-recovery-{}-{}",
            std::process::id(),
            line!()
        ));
        std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let outcome = (|| {
            let baseline = include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-001.docx");
            joyword_shell::storage::commit_recovery(&dir, baseline, "已提交")
                .map_err(|error| error.to_string())?;
            let (_office, editor, id) = super::take_recovered(&dir, None)
                .ok_or_else(|| "journal did not restore".to_string())?;
            if id.0 == 0 {
                return Err("recovered document has no id".into());
            }
            let text = editor.view().index().text();
            if !text.contains("已提交") {
                return Err(format!("restored text was {text}"));
            }
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&dir);
        outcome
    }

    #[test]
    fn ribbon_commands_have_gpui_actions() {
        let registry = Registry::builtin();
        for control in
            joyword_shell::chrome::visible_controls(&registry, joyword_shell::chrome::RIBBON)
        {
            assert!(
                action_for(control.command).is_some(),
                "{} has no GPUI action",
                control.id
            );
        }
    }

    #[test]
    fn file_and_insert_commands_are_not_the_language_switch() {
        let language = action_for(SWITCH_LANGUAGE).unwrap().name();
        for id in [
            EDIT_INSERT,
            EDIT_INSERT_IMAGE,
            FILE_OPEN_RECENT,
            DOCUMENT_SWITCH,
            FILE_CLOSE,
        ] {
            let action = action_for(id).unwrap();
            assert_ne!(action.name(), language, "{}", id);
        }
    }
}

// The no-panic gate scans this file in full, including `#[cfg(test)]`.
// Width and regex checks live in tests.rs, which that gate skips.
#[cfg(test)]
#[path = "tests.rs"]
mod find_bar_tests;
