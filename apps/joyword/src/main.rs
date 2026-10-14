//! In-memory text editing backed by the engine and unified page scene.
//! Menus, shortcuts and on-screen buttons come from the joyword-shell command
//! registry and run through its single entry point (T106).
mod document_canvas;
mod input_ui;

use document_canvas::{CanvasCancellation, CanvasFrame, CanvasRequestKey};
use gpui::{
    Action, App, Application, Bounds, Context, FocusHandle, KeyBinding, Menu, MenuItem, Window,
    WindowBounds, WindowOptions, actions, div, img, prelude::*, px, rgb, size,
};
use joyword_foundation::ScreenScale;
use joyword_layout::{LoadedFont, SingleFontProfile};
use joyword_render::Projection;
use joyword_render::requests::RasterRequestGate;
use joyword_shell::command::*;
use joyword_shell::input::{InputLimits, TextInput};
use joyword_shell::l10n::{Catalog, Locale};
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
        CancelComposition
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
        _ => return None,
    })
}

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
                        })
                    }
                })
                .collect(),
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
    editor: Option<TextInput>,
    font: Option<Arc<LoadedFont>>,
    profile: Option<SingleFontProfile>,
    frame: Option<CanvasFrame>,
    gate: RasterRequestGate<CanvasRequestKey>,
    canvas_cancel: CanvasCancellation,
    load_started: bool,
    load_task: Option<gpui::Task<()>>,
    raster_task: Option<gpui::Task<()>>,
    device_scale: f32,
    canvas_error: Option<String>,
    input_error: Option<String>,
    scroll: gpui::ScrollHandle,
    viewport_bounds: Option<Bounds<gpui::Pixels>>,
    dragging: bool,
    reveal_pending: bool,
    preferred_x: Option<joyword_foundation::Emu>,
    close_pending: bool,
    close_task: Option<gpui::Task<()>>,
    subscriptions: Vec<gpui::Subscription>,
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

    fn execute(&mut self, id: CommandId) -> Result<(), String> {
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
            other => return view.execute_input(other, self.window, self.cx),
        }
        self.cx.notify();
        Ok(())
    }
}

impl Drop for JoyWord {
    fn drop(&mut self) {
        self.canvas_cancel.cancel();
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
    fn current_key(&self) -> Option<CanvasRequestKey> {
        Some(CanvasRequestKey {
            revision: self.editor.as_ref()?.view().revision(),
            font: self.font.as_ref()?.key(),
            profile: self.profile?,
            projection: self.projection(),
        })
    }
    fn ready_frame(&self) -> Option<&CanvasFrame> {
        let key = self.current_key()?;
        self.frame.as_ref().filter(|f| f.key == key)
    }
    fn drop_frame(&mut self, cx: &mut Context<Self>) {
        if let Some(frame) = self.frame.take() {
            for image in frame.images {
                cx.drop_image(image, None);
            }
        }
    }
    fn request_canvas(&mut self, cx: &mut Context<Self>) {
        if !self.load_started {
            self.load_started = true;
            let job = cx
                .background_executor()
                .spawn(async { document_canvas::load_source() });
            self.load_task = Some(cx.spawn(async move |view, cx| {
                let result = job.await;
                let _ = view.update(cx, |view, cx| {
                    match result {
                        Ok(source) => {
                            let document_canvas::CanvasSource {
                                session,
                                layout,
                                font,
                                profile,
                            } = source;
                            if let Err(error) =
                                layout.ensure_current(&session.snapshot(), &font, profile)
                            {
                                view.canvas_error = Some(error.to_string());
                                cx.notify();
                                return;
                            }
                            match TextInput::new(session, InputLimits::default()) {
                                Ok(editor) => {
                                    view.editor = Some(editor);
                                    view.font = Some(Arc::new(font));
                                    view.profile = Some(profile);
                                }
                                Err(e) => view.canvas_error = Some(e.to_string()),
                            }
                        }
                        Err(e) => view.canvas_error = Some(e),
                    }
                    cx.notify();
                });
            }));
        }
        let Some(key) = self.current_key() else {
            return;
        };
        if self.ready_frame().is_some() {
            return;
        }
        let Some(ticket) = self.gate.request(key.clone()) else {
            return;
        };
        self.canvas_cancel.cancel();
        self.canvas_cancel = CanvasCancellation::default();
        let cancellation = self.canvas_cancel.clone();
        let font = self.font.clone().expect("current key requires font");
        let input = self
            .editor
            .as_ref()
            .expect("current key requires editor")
            .view()
            .clone();
        self.canvas_error = None;
        let job = cx.background_executor().spawn(async move {
            document_canvas::render_view(&input, &font, key.profile, key.projection, &cancellation)
        });
        self.raster_task = Some(cx.spawn(async move |view, cx| {
            let result = job.await;
            let _ = view.update(cx, |view, cx| {
                let Some(current) = view.current_key() else {
                    return;
                };
                if !view.gate.accepts(&ticket, &current) {
                    return;
                }
                view.drop_frame(cx);
                match result {
                    Ok(frame) if frame.key == current => {
                        view.frame = Some(frame);
                        view.reveal_caret();
                    }
                    Ok(_) => return,
                    Err(error) => view.canvas_error = Some(error),
                }
                cx.notify();
            });
        }));
    }
    fn canvas(&self) -> gpui::AnyElement {
        if let Some(error) = &self.canvas_error {
            return div()
                .p_6()
                .bg(rgb(0xfff8e7))
                .child(Catalog::shipped(self.locale).format("canvas.error", &[("reason", error)]))
                .into_any_element();
        }
        if let Some(frame) = self.ready_frame() {
            let scale =
                ScreenScale::new(f64::from(self.zoom_percent) / 100.0).expect("clamped zoom");
            return div()
                .flex()
                .flex_col()
                .gap(px(input_ui::DOC_GAP))
                .children(
                    frame
                        .layout
                        .pages()
                        .iter()
                        .zip(&frame.images)
                        .map(|(page, image)| {
                            div()
                                .w(px(scale.logical_pixels(page.width) as f32))
                                .h(px(scale.logical_pixels(page.height) as f32))
                                .flex_shrink_0()
                                .bg(rgb(0xffffff))
                                .shadow_md()
                                .child(img(image.clone()).size_full())
                        }),
                )
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

    /// Single entry for menus, shortcuts and on-screen controls. Disabled
    /// commands (zoom limits) do nothing.
    fn run(&mut self, id: CommandId, window: &mut Window, cx: &mut Context<Self>) {
        let registry = self.registry.clone();
        let _ = registry.invoke(
            &mut ViewHost {
                view: self,
                window,
                cx,
            },
            id,
        );
    }
}

impl Render for JoyWord {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.device_scale = window.scale_factor();
        self.request_canvas(cx);
        window.set_window_edited(
            self.editor
                .as_ref()
                .is_some_and(TextInput::has_unsaved_changes),
        );
        let catalog = Catalog::shipped(self.locale);

        div()
            .track_focus(&self.focus_handle)
            .key_context(
                if self.editor.as_ref().is_some_and(TextInput::is_composing) {
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
            .child(self.input_viewport(cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_6()
                    .py_2()
                    .bg(rgb(0xffffff))
                    .text_sm()
                    .child(self.input_status())
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

fn main() {
    Application::new().run(|cx: &mut App| {
        let registry = Registry::builtin();
        let locale = Locale::ZhCn;
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
                    let mut view = JoyWord {
                        zoom_percent: ZOOM_DEFAULT,
                        locale,
                        registry: registry.clone(),
                        focus_handle: cx.focus_handle(),
                        editor: None,
                        font: None,
                        profile: None,
                        frame: None,
                        gate: RasterRequestGate::default(),
                        canvas_cancel: CanvasCancellation::default(),
                        load_started: false,
                        load_task: None,
                        raster_task: None,
                        device_scale: 1.0,
                        canvas_error: None,
                        input_error: None,
                        scroll: gpui::ScrollHandle::new(),
                        viewport_bounds: None,
                        dragging: false,
                        reveal_pending: false,
                        preferred_x: None,
                        close_pending: false,
                        close_task: None,
                        subscriptions: Vec::new(),
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
                    window.focus(&view.focus_handle);
                    cx.activate(true);
                });
            }
            Err(error) => {
                eprintln!("Could not open the JoyWord window: {error}");
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
}
