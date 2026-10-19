//! Ribbon, horizontal ruler and status counts (T205). Buttons call the
//! command registry. The ruler shows the current paragraph and does not edit it.
use crate::{JoyWord, ZOOM_MAX, ZOOM_MIN};
use gpui::{Bounds, Context, Pixels, Window, canvas, div, fill, point, prelude::*, px, rgb, size};
use joyword_foundation::Emu;
use joyword_model::TabAlign;
use joyword_shell::chrome::{
    self, RibbonArgs, RibbonControl, RibbonInput, RibbonTab, RulerModel, RulerTab, icon_cells,
    ruler_scale, status_line,
};
use joyword_shell::command::{Availability, CommandId, ZOOM_IN, ZOOM_OUT};
use joyword_shell::l10n::Catalog;

const INK: u32 = 0x163b65;
const INK_MUTED: u32 = 0x9aa7b2;

impl JoyWord {
    pub(crate) fn ribbon(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = chrome::visible_controls(&self.registry, chrome::RIBBON);
        let on_tab = chrome::controls_on(self.ribbon_tab, shown.iter().copied());
        let grouped = chrome::groups(&on_tab);
        div()
            .flex()
            .flex_col()
            .bg(rgb(0xf7f9fb))
            .border_b_1()
            .border_color(rgb(0xd5dbe3))
            .child(self.ribbon_tabs(cx))
            .child(
                div()
                    .id("ribbon-row")
                    .flex()
                    .items_end()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .overflow_x_scroll()
                    .children(
                        grouped
                            .into_iter()
                            .map(|(group, controls)| self.ribbon_group(group, &controls, cx)),
                    ),
            )
    }

    fn ribbon_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .gap_1()
            .px_3()
            .pt_2()
            .children(RibbonTab::ALL.into_iter().map(|tab| {
                let selected = self.ribbon_tab == tab;
                div()
                    .id(tab.label_key())
                    .px_3()
                    .py_1()
                    .cursor_pointer()
                    .rounded_t_sm()
                    .bg(if selected {
                        rgb(0xffffff)
                    } else {
                        rgb(0xf7f9fb)
                    })
                    .text_color(if selected { rgb(INK) } else { rgb(0x5c6b7a) })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.ribbon_tab = tab;
                        cx.notify();
                    }))
                    .child(self.label(tab.label_key()))
            }))
    }

    fn ribbon_group(
        &self,
        group: &'static str,
        controls: &[&RibbonControl],
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .id(group)
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .px_2()
            .border_r_1()
            .border_color(rgb(0xd5dbe3))
            .child(
                div().flex().gap_1().children(
                    controls
                        .iter()
                        .map(|control| self.ribbon_button(control, cx)),
                ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x5c6b7a))
                    .child(self.label(group)),
            )
            .into_any_element()
    }

    fn ribbon_button(&self, control: &RibbonControl, cx: &mut Context<Self>) -> gpui::AnyElement {
        let control = *control;
        let enabled = self.ribbon_enabled(control.command);
        let ink = if enabled { INK } else { INK_MUTED };
        div()
            .id(control.id)
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .rounded_sm()
            .cursor_pointer()
            .bg(if enabled {
                rgb(0xe7eef5)
            } else {
                rgb(0xf4f6f8)
            })
            .text_color(rgb(ink))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.run_ribbon(control, window, cx);
            }))
            .child(drawn_icon(control.icon, ink))
            .child(div().text_xs().child(self.label(control.label)))
            .into_any_element()
    }

    fn ribbon_enabled(&self, id: CommandId) -> bool {
        if matches!(self.input_availability(id), Availability::Disabled(_)) {
            return false;
        }
        if id == ZOOM_IN && self.zoom_percent >= ZOOM_MAX {
            return false;
        }
        if id == ZOOM_OUT && self.zoom_percent <= ZOOM_MIN {
            return false;
        }
        true
    }

    fn run_ribbon(&mut self, control: RibbonControl, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(control.args, RibbonArgs::PromptPath) {
            self.prompt_insert_picture(cx);
            return;
        }
        let call = control.bind(RibbonInput {
            query: &self.find_query,
            replacement: &self.find_replacement,
            case_sensitive: self.find_case,
            whole_word: self.find_word,
            format_repeat: self.format_repeat,
        });
        if call.args.is_empty() {
            self.run(call.command, window, cx);
        } else {
            self.run_with(call.command, &call.args, window, cx);
        }
    }

    pub(crate) fn ruler(&self) -> impl IntoElement {
        let model = self.current_ruler();
        let unresolved = model.unresolved;
        div()
            .flex()
            .items_center()
            .h(px(28.0))
            .bg(rgb(0xffffff))
            .border_b_1()
            .border_color(rgb(0xd5dbe3))
            .child(
                div().relative().flex_1().h_full().child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| paint_ruler(bounds, &model, window),
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                ),
            )
            .children(unresolved.then(|| {
                div()
                    .px_2()
                    .text_xs()
                    .text_color(rgb(0x8a5a00))
                    .child(self.label("ruler.unresolved"))
            }))
    }

    fn current_ruler(&self) -> RulerModel {
        let Some(editor) = self.editor.as_ref() else {
            return RulerModel::default();
        };
        let view = editor.view();
        let head = view.selection().head;
        let document = view.snapshot();
        let Some(paragraph) = document.find_paragraph(head.story, head.paragraph) else {
            return RulerModel::default();
        };
        let resolved = document
            .styles()
            .resolve_paragraph(paragraph.style.as_ref(), paragraph.props.as_ref());
        let tabs = resolved.props.tabs.clone().unwrap_or_default();
        let (interval, unresolved_interval) =
            chrome::default_tab_interval(document.settings().default_tab_stop);
        let mut model = chrome::ruler_model(
            &resolved.props.indent,
            &tabs,
            interval,
            self.content_width_emu(),
        );
        if unresolved_interval {
            model.unresolved = true;
        }
        model
    }

    fn content_width_emu(&self) -> Option<i64> {
        let frame = self.ready_layout()?;
        let count = frame.layout.pages().len();
        if count == 0 {
            return None;
        }
        let index = self
            .editor
            .as_ref()
            .and_then(|editor| {
                frame
                    .layout
                    .caret(editor.view().selection().head)
                    .ok()
                    .map(|caret| caret.page)
            })
            .unwrap_or(0)
            .min(count - 1);
        let geometry = frame.layout.page_geometry().get(index)?;
        let width = geometry
            .width
            .raw()
            .saturating_sub(geometry.left.raw())
            .saturating_sub(geometry.right.raw());
        (width > 0).then_some(width)
    }

    pub(crate) fn status_counts(&self) -> String {
        let census = self
            .editor
            .as_ref()
            .map(|editor| chrome::census(editor.view().index().text()))
            .unwrap_or_default();
        let (page, pages) = self
            .ready_layout()
            .map(|frame| {
                let index = self.editor.as_ref().and_then(|editor| {
                    frame
                        .layout
                        .caret(editor.view().selection().head)
                        .ok()
                        .map(|caret| caret.page)
                });
                chrome::display_page(index, frame.layout.pages().len())
            })
            .unwrap_or((None, None));
        status_line(
            Catalog::shipped(self.locale),
            &chrome::StatusSnapshot {
                page,
                pages,
                census,
            },
        )
    }
}

fn drawn_icon(id: &str, ink: u32) -> gpui::AnyElement {
    let cells = icon_cells(id);
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            for (column, row) in &cells {
                let origin = point(
                    bounds.origin.x + bounds.size.width * (f32::from(*column) / 16.0),
                    bounds.origin.y + bounds.size.height * (f32::from(*row) / 16.0),
                );
                let step_x = bounds.size.width / px(16.0);
                let step_y = bounds.size.height / px(16.0);
                window.paint_quad(fill(
                    Bounds {
                        origin,
                        size: size(px(step_x.max(1.0)), px(step_y.max(1.0))),
                    },
                    rgb(ink),
                ));
            }
        },
    )
    .w(px(16.0))
    .h(px(16.0))
    .into_any_element()
}

fn paint_ruler(bounds: Bounds<Pixels>, model: &RulerModel, window: &mut Window) {
    let scale = ruler_scale(model);
    window.paint_quad(fill(
        Bounds {
            origin: point(
                bounds.origin.x,
                bounds.origin.y + bounds.size.height - px(1.0),
            ),
            size: size(bounds.size.width, px(1.0)),
        },
        rgb(0xd5dbe3),
    ));
    if model.content_width.is_some() {
        let mut inch = Emu::PER_INCH;
        while inch < scale {
            paint_tick(bounds, scale, inch, px(8.0), 0xc5ced6, window);
            let next = inch.saturating_add(Emu::PER_INCH);
            if next <= inch {
                break;
            }
            inch = next;
        }
    }
    for tab in &model.tabs {
        paint_tab(bounds, scale, tab, window);
    }
    if let Some(position) = model.left {
        paint_block(bounds, scale, position, false, INK, window);
    }
    if let Some(position) = model.first_line {
        paint_block(bounds, scale, position, true, 0x1f5b9b, window);
    }
    if let Some(position) = model.right {
        paint_block(bounds, scale, position, false, 0xc47b2b, window);
    }
}

fn paint_tab(bounds: Bounds<Pixels>, scale: i64, tab: &RulerTab, window: &mut Window) {
    let color = if tab.custom { 0x1f5b9b } else { 0xc5ced6 };
    let height = match tab.align {
        TabAlign::Bar => bounds.size.height,
        TabAlign::Center | TabAlign::Decimal | TabAlign::Num => px(10.0),
        TabAlign::Right => px(6.0),
        TabAlign::Left | TabAlign::Clear => px(6.0),
    };
    let top = match tab.align {
        TabAlign::Bar => bounds.origin.y,
        TabAlign::Right => bounds.origin.y + px(2.0),
        _ => bounds.origin.y + bounds.size.height - height - px(1.0),
    };
    let x = emu_x(bounds, scale, tab.position);
    window.paint_quad(fill(
        Bounds {
            origin: point(x, top),
            size: size(px(1.5), height),
        },
        rgb(color),
    ));
}

fn paint_tick(
    bounds: Bounds<Pixels>,
    scale: i64,
    emu: i64,
    height: Pixels,
    color: u32,
    window: &mut Window,
) {
    let x = emu_x(bounds, scale, emu);
    window.paint_quad(fill(
        Bounds {
            origin: point(x, bounds.origin.y),
            size: size(px(1.0), height),
        },
        rgb(color),
    ));
}

fn paint_block(
    bounds: Bounds<Pixels>,
    scale: i64,
    emu: i64,
    top: bool,
    color: u32,
    window: &mut Window,
) {
    let x = emu_x(bounds, scale, emu);
    let y = if top {
        bounds.origin.y + px(2.0)
    } else {
        bounds.origin.y + bounds.size.height - px(8.0)
    };
    window.paint_quad(fill(
        Bounds {
            origin: point(x - px(3.0), y),
            size: size(px(6.0), px(6.0)),
        },
        rgb(color),
    ));
}

fn emu_x(bounds: Bounds<Pixels>, scale: i64, emu: i64) -> Pixels {
    let t = (emu as f32 / scale.max(1) as f32).clamp(0.0, 1.0);
    bounds.origin.x + bounds.size.width * t
}
