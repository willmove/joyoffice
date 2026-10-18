//! In-memory text editing backed by the engine and unified page scene.
//! Menus, shortcuts and on-screen buttons come from the joyword-shell command
//! registry and run through its single entry point (T106).
mod chrome_ui;
mod document_canvas;
mod input_ui;

use document_canvas::{CanvasPipeline, Landed, LayoutFrame, LayoutRequestKey, PageImage, PageKey};
use gpui::{
    Action, App, Bounds, Context, FocusHandle, KeyBinding, Menu, MenuItem, SharedString, Window,
    WindowBounds, WindowOptions, actions, div, img, prelude::*, px, rgb, size,
};
use joyword_foundation::ScreenScale;
use joyword_layout::{LoadedFont, SingleFontProfile};
use joyword_render::Projection;
use joyword_render::requests::RasterTicket;
use joyword_shell::chrome::RibbonTab;
use joyword_shell::command::*;
use joyword_shell::diag::{self, AppLog, Event, Level};
use joyword_shell::input::{InputLimits, TextInput};
use joyword_shell::l10n::{Catalog, Locale};
use joyword_shell::prefs::{PrefsNotice, PrefsStore};
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
        CancelComposition,
        OpenFile,
        OpenRecent,
        SwitchDocument,
        InsertText,
        SaveOriginal,
        SaveCopy,
        Bold,
        Italic,
        ListStyles,
        ApplyStyle,
        ApplyList,
        SetListLevel,
        RemoveList,
        AlignCenter,
        PageBreak,
        PageSize,
        PageMargins,
        PageOrientation,
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
        SplitTableCell
    ]
);

const ZOOM_MIN: u16 = 50;
const ZOOM_MAX: u16 = 150;
const ZOOM_DEFAULT: u16 = 80;

/// GPUI action for a registered command. Every registry entry must map;
/// `registry_commands_all_have_actions` checks it.
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
        EDIT_CANCEL => Box::new(CancelComposition),
        FILE_OPEN => Box::new(OpenFile),
        FILE_OPEN_RECENT => Box::new(OpenRecent),
        DOCUMENT_SWITCH => Box::new(SwitchDocument),
        FILE_SAVE => Box::new(SaveOriginal),
        FILE_SAVE_COPY => Box::new(SaveCopy),
        EDIT_BOLD => Box::new(Bold),
        EDIT_ITALIC => Box::new(Italic),
        STYLE_LIST => Box::new(ListStyles),
        STYLE_APPLY => Box::new(ApplyStyle),
        LIST_APPLY => Box::new(ApplyList),
        LIST_LEVEL => Box::new(SetListLevel),
        LIST_REMOVE => Box::new(RemoveList),
        PAGE_BREAK => Box::new(PageBreak),
        PAGE_SIZE => Box::new(PageSize),
        PAGE_MARGINS => Box::new(PageMargins),
        PAGE_ORIENTATION => Box::new(PageOrientation),
        EDIT_ALIGN_CENTER => Box::new(AlignCenter),
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

fn key_bindings(registry: &Registry) -> Vec<KeyBinding> {
    let platform = Platform::current();
    registry
        .commands()
        .iter()
        .filter_map(|spec| {
            let keys = spec.shortcut.for_platform(platform)?;
            Some(match spec.id {
                QUIT => KeyBinding::new(keys, Quit, None),
                ZOOM_IN => KeyBinding::new(keys, ZoomIn, None),
                ZOOM_OUT => KeyBinding::new(keys, ZoomOut, None),
                ZOOM_RESET => KeyBinding::new(keys, ZoomReset, None),
                SWITCH_LANGUAGE => KeyBinding::new(keys, SwitchLanguage, None),
                EDIT_UNDO => KeyBinding::new(keys, Undo, Some("JoyWordInput")),
                EDIT_REDO => KeyBinding::new(keys, Redo, Some("JoyWordInput")),
                EDIT_SELECT_ALL => KeyBinding::new(keys, SelectAll, Some("JoyWordInput")),
                EDIT_BACKSPACE => {
                    KeyBinding::new(keys, Backspace, Some("JoyWordInput && !composing"))
                }
                EDIT_DELETE => {
                    KeyBinding::new(keys, DeleteForward, Some("JoyWordInput && !composing"))
                }
                EDIT_ENTER => {
                    KeyBinding::new(keys, NewParagraph, Some("JoyWordInput && !composing"))
                }
                EDIT_LEFT => KeyBinding::new(keys, MoveLeft, Some("JoyWordInput && !composing")),
                EDIT_RIGHT => KeyBinding::new(keys, MoveRight, Some("JoyWordInput && !composing")),
                EDIT_EXTEND_LEFT => {
                    KeyBinding::new(keys, ExtendLeft, Some("JoyWordInput && !composing"))
                }
                EDIT_EXTEND_RIGHT => {
                    KeyBinding::new(keys, ExtendRight, Some("JoyWordInput && !composing"))
                }
                EDIT_UP => KeyBinding::new(keys, MoveUp, Some("JoyWordInput && !composing")),
                EDIT_DOWN => KeyBinding::new(keys, MoveDown, Some("JoyWordInput && !composing")),
                EDIT_EXTEND_UP => {
                    KeyBinding::new(keys, ExtendUp, Some("JoyWordInput && !composing"))
                }
                EDIT_EXTEND_DOWN => {
                    KeyBinding::new(keys, ExtendDown, Some("JoyWordInput && !composing"))
                }
                EDIT_HOME => KeyBinding::new(keys, LineStart, Some("JoyWordInput && !composing")),
                EDIT_END => KeyBinding::new(keys, LineEnd, Some("JoyWordInput && !composing")),
                EDIT_EXTEND_HOME => {
                    KeyBinding::new(keys, ExtendLineStart, Some("JoyWordInput && !composing"))
                }
                EDIT_EXTEND_END => {
                    KeyBinding::new(keys, ExtendLineEnd, Some("JoyWordInput && !composing"))
                }
                EDIT_CANCEL => KeyBinding::new(keys, CancelComposition, Some("JoyWordInput")),
                FILE_OPEN => KeyBinding::new(keys, OpenFile, None),
                FILE_SAVE => KeyBinding::new(keys, SaveOriginal, None),
                FILE_SAVE_COPY => KeyBinding::new(keys, SaveCopy, None),
                EDIT_BOLD => KeyBinding::new(keys, Bold, Some("JoyWordInput && !composing")),
                EDIT_ITALIC => KeyBinding::new(keys, Italic, Some("JoyWordInput && !composing")),
                EDIT_COPY => KeyBinding::new(keys, CopyText, Some("JoyWordInput")),
                EDIT_PASTE => KeyBinding::new(keys, PasteText, Some("JoyWordInput && !composing")),
                FILE_CLOSE => KeyBinding::new(keys, CloseDocument, None),
                FIND_NEXT => KeyBinding::new(keys, FindNext, Some("JoyWordInput")),
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
    original_save: Option<input_ui::OriginalSavePhase>,
    diagnostics: Option<Arc<AppLog>>,
    find_query: String,
    find_replacement: String,
    find_case: bool,
    find_word: bool,
    find_query_focus: FocusHandle,
    find_replacement_focus: FocusHandle,
    find_note: Option<String>,
    format_repeat: bool,
    ribbon_tab: RibbonTab,
}

#[derive(Clone, Copy)]
enum FindSlot {
    Query,
    Replacement,
}

/// View-level commands, executed with access to the view's context.
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
        Some(self.canvas.key(
            self.editor.as_ref()?.view(),
            self.font.as_ref()?,
            self.profile?,
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
            let previous = self.canvas.previous_layout(&font.key(), profile);
            let work = cx.background_executor().spawn(async move {
                document_canvas::layout_view_incremental(
                    &input,
                    &font,
                    profile,
                    &cancellation,
                    &cache,
                    previous.as_deref(),
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

    fn run_with(
        &mut self,
        id: CommandId,
        args: &[(&'static str, ArgValue)],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    fn find_args(&self, replacement: bool) -> Vec<(&'static str, ArgValue)> {
        let mut args = vec![
            ("query", ArgValue::Text(self.find_query.clone())),
            ("case", ArgValue::Bool(self.find_case)),
            ("word", ArgValue::Bool(self.find_word)),
        ];
        if replacement {
            args.push(("replacement", ArgValue::Text(self.find_replacement.clone())));
        }
        args
    }

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
            .child(self.toggle_button(
                "find-case",
                &self.label("find.case"),
                self.find_case,
                true,
                cx,
            ))
            .child(self.toggle_button(
                "find-word",
                &self.label("find.word"),
                self.find_word,
                false,
                cx,
            ))
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
        let (focus, text, placeholder) = match slot {
            FindSlot::Query => (
                self.find_query_focus.clone(),
                self.find_query.clone(),
                self.label("find.query"),
            ),
            FindSlot::Replacement => (
                self.find_replacement_focus.clone(),
                self.find_replacement.clone(),
                self.label("find.replacement"),
            ),
        };
        let shown = if text.is_empty() { placeholder } else { text };
        let faded = match slot {
            FindSlot::Query => self.find_query.is_empty(),
            FindSlot::Replacement => self.find_replacement.is_empty(),
        };
        div()
            .id(match slot {
                FindSlot::Query => "find-query",
                FindSlot::Replacement => "find-replacement",
            })
            .track_focus(&focus)
            .focusable()
            .w(px(160.0))
            .px_2()
            .py_1()
            .border_1()
            .border_color(rgb(0xc5ced6))
            .bg(rgb(0xffffff))
            .text_color(if faded { rgb(0x8896a3) } else { rgb(0x263247) })
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
            .child(shown)
    }

    fn type_find(
        &mut self,
        slot: FindSlot,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mods = &event.keystroke.modifiers;
        if mods.control || mods.alt || mods.platform || mods.function {
            return;
        }
        if event.keystroke.key == "enter" {
            cx.stop_propagation();
            let args = self.find_args(false);
            self.run_with(FIND_NEXT, &args, window, cx);
            return;
        }
        if event.keystroke.key == "escape" {
            cx.stop_propagation();
            window.focus(&self.focus_handle, cx);
            cx.notify();
            return;
        }
        let text = match slot {
            FindSlot::Query => &mut self.find_query,
            FindSlot::Replacement => &mut self.find_replacement,
        };
        if event.keystroke.key == "backspace" {
            text.pop();
        } else if let Some(ch) = event.keystroke.key_char.as_deref() {
            if ch.chars().any(char::is_control) {
                return;
            }
            text.push_str(ch);
        } else {
            return;
        }
        cx.stop_propagation();
        cx.notify();
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

    fn toggle_button(
        &self,
        id: &'static str,
        label: &str,
        on: bool,
        case: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.to_owned();
        div()
            .id(id)
            .cursor_pointer()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(if on { rgb(0xc5d8ea) } else { rgb(0xe7eef5) })
            .on_click(cx.listener(move |this, _, _, cx| {
                if case {
                    this.find_case = !this.find_case;
                } else {
                    this.find_word = !this.find_word;
                }
                cx.notify();
            }))
            .child(label)
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
            .child(if on { "Repeat" } else { "Once" })
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
        window.set_window_edited(
            self.editor
                .as_ref()
                .is_some_and(|editor| editor.has_unsaved_changes()),
        );
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
                cx.listener(|this, _: &PageSize, window, cx| this.run(PAGE_SIZE, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PageMargins, window, cx| this.run(PAGE_MARGINS, window, cx)),
            )
            .on_action(cx.listener(|this, _: &PageOrientation, window, cx| {
                this.run(PAGE_ORIENTATION, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &Italic, window, cx| this.run(EDIT_ITALIC, window, cx)),
            )
            .on_action(cx.listener(|this, _: &AlignCenter, window, cx| {
                this.run(EDIT_ALIGN_CENTER, window, cx)
            }))
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
                    .child("JoyWord · JoyOffice")
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
            .child(self.ribbon(cx))
            .child(self.ruler())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.style_panel(cx))
                    .child(self.input_viewport(cx)),
            )
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

fn main() {
    let boot = boot_diagnostics();
    let boot_log = boot.log.clone();
    let locale = boot.locale;
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
        cx.set_menus(menus(&registry, Catalog::shipped(locale)));
        let bounds = Bounds::centered(None, size(px(1100.0), px(850.0)), cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("JoyWord · Preview".into()),
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
                        original_save: None,
                        diagnostics: boot_log.clone(),
                        find_query: String::new(),
                        find_replacement: String::new(),
                        find_case: false,
                        find_word: false,
                        find_query_focus: cx.focus_handle(),
                        find_replacement_focus: cx.focus_handle(),
                        find_note: None,
                        format_repeat: false,
                        ribbon_tab: RibbonTab::Start,
                    };
                    view.install_input_lifecycle(window, cx);
                    view
                })
            },
        );
        match result {
            Ok(window) => {
                cx.set_global(MainWindow(window));
                let _ = window.update(cx, |view, window, cx| {
                    window.focus(&view.focus_handle, cx);
                    cx.activate(true);
                });
            }
            Err(error) => {
                let detail = diag::redact(&error.to_string());
                if let Some(log) = &boot_log {
                    let _ = log.record_untrusted(Level::Error, "window.open", &detail);
                }
                eprintln!("Could not open the JoyWord window: {detail}");
                cx.quit();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let with_keys = registry
            .commands()
            .iter()
            .filter(|spec| spec.shortcut.for_platform(Platform::current()).is_some())
            .count();
        assert_eq!(key_bindings(&registry).len(), with_keys);
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
