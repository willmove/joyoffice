//! Right-click context menus for slide rail, slide sorter, and slide canvas.
use super::*;
use gpui_kit::component::{
    Icon,
    menu::{PopupMenu, PopupMenuItem},
};
use joydeck_engine::{AlignEdge, Command, ContentAction, DistributeAxis};
use joydeck_model::ShapeKind;
use joyoffice_ui::IconName as I;

fn menu_item(
    view: &WeakEntity<Viewer>,
    label: &'static str,
    icon: I,
    enabled: bool,
    action: impl Fn(&mut Viewer, &mut Window, &mut Context<Viewer>) + 'static,
) -> PopupMenuItem {
    let view = view.clone();
    PopupMenuItem::new(label)
        .icon(icon)
        .disabled(!enabled)
        .on_click(move |_, w, cx| {
            let _ = view.update(cx, |s, cx| action(s, w, cx));
        })
}

impl Viewer {
    pub(super) fn select_slide_for_context(
        &mut self,
        page: usize,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.composing() || self.loading {
            return;
        }
        self.commit_notes(w, cx);
        self.end_inline(cx);
        if self.slide_selected(page) {
            self.focus.focus(w, cx);
            return;
        }
        let Some(id) = self
            .editor
            .as_ref()
            .and_then(|e| e.session.model().slides.get(page))
            .map(|s| s.id)
        else {
            return;
        };
        self.select(page, cx);
        self.slide_selection = vec![id];
        self.slide_anchor = Some(id);
        self.focus.focus(w, cx);
        cx.notify();
    }

    pub(super) fn build_slide_context_menu(
        view: WeakEntity<Self>,
        page: usize,
        menu: PopupMenu,
        w: &mut Window,
        cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        let state = view
            .read_with(cx, |s, cx| {
                let hidden = s
                    .editor
                    .as_ref()
                    .and_then(|e| e.session.model().slides.get(page))
                    .is_some_and(|sl| sl.hidden);
                let layouts = s
                    .editor
                    .as_ref()
                    .and_then(|e| {
                        let p = e.session.model();
                        let sl = p.slides.get(page)?;
                        let (m, _) = p.master_for(sl)?;
                        Some((
                            sl.layout,
                            m.layouts
                                .iter()
                                .map(|l| (l.id, l.name.clone()))
                                .collect::<Vec<_>>(),
                        ))
                    })
                    .unwrap_or_default();
                (
                    s.basic_edit_unavailable(BasicEdit::NewSlide, cx).is_none(),
                    s.basic_edit_unavailable(BasicEdit::DuplicateSlides, cx)
                        .is_none(),
                    s.basic_edit_unavailable(BasicEdit::DeleteSlides, cx)
                        .is_none(),
                    s.editor.is_some() && !s.loading && !s.composing(),
                    hidden,
                    layouts,
                )
            })
            .ok();
        let Some((can_new, can_dup, can_del, can_edit, hidden, (current_layout, layouts))) = state
        else {
            return menu;
        };

        let locale = joyoffice_ui::current_locale(cx);
        let t = |key: &'static str| crate::l10n::t(locale, key);
        let mut menu = menu
            .item(menu_item(
                &view,
                t("menu.new_slide"),
                I::FilePlus,
                can_new,
                |s, w, cx| s.run_basic_edit(BasicEdit::NewSlide, w, cx),
            ))
            .item(menu_item(
                &view,
                t("menu.duplicate_slide"),
                I::CopyPlus,
                can_dup,
                |s, w, cx| s.run_basic_edit(BasicEdit::DuplicateSlides, w, cx),
            ))
            .item(menu_item(
                &view,
                t("menu.delete_slide"),
                I::Trash,
                can_del,
                |s, w, cx| s.run_basic_edit(BasicEdit::DeleteSlides, w, cx),
            ))
            .separator()
            .item(menu_item(
                &view,
                if hidden {
                    t("ctx.unhide_slide")
                } else {
                    t("ctx.hide_slide")
                },
                I::EyeOff,
                can_edit,
                |s, w, cx| s.toggle_hidden_slides(w, cx),
            ));

        if !layouts.is_empty() {
            let layout_view = view.clone();
            menu = menu.submenu_with_icon(
                Some(Icon::new(I::PanelsTopLeft)),
                t("home.layout"),
                w,
                cx,
                move |mut sub, _, _| {
                    for (id, name) in &layouts {
                        let id = *id;
                        let v = layout_view.clone();
                        let label = if name.trim().is_empty() {
                            let id_s = id.0.to_string();
                            crate::l10n::tf(locale, "ctx.layout_num", &[("num", &id_s)])
                        } else {
                            name.clone()
                        };
                        sub = sub.item(
                            PopupMenuItem::new(label)
                                .checked(id == current_layout)
                                .disabled(!can_edit)
                                .on_click(move |_, w, cx| {
                                    let _ = v.update(cx, |s, cx| {
                                        s.dispatch(
                                            Command::Content {
                                                action: ContentAction::Layout { id },
                                            },
                                            w,
                                            cx,
                                        );
                                    });
                                }),
                        );
                    }
                    sub
                },
            );
        }

        menu.item(menu_item(
            &view,
            t("ctx.format_bg"),
            I::Palette,
            can_edit,
            |s, w, cx| s.content_form(content_ui::Form::Background, w, cx),
        ))
        .separator()
        .item(menu_item(
            &view,
            t("home.move_slides"),
            I::Move,
            can_edit,
            |s, w, cx| s.organize_dialog(w, cx),
        ))
    }

    pub(super) fn build_canvas_context_menu(
        view: WeakEntity<Self>,
        menu: PopupMenu,
        w: &mut Window,
        cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        let state = view
            .read_with(cx, |s, cx| {
                let sel_len = s
                    .editor
                    .as_ref()
                    .map_or(0, |e| e.session.selection().shapes.len());
                let is_basic_shape = s.editor.as_ref().is_some_and(|e| {
                    sel_len == 1
                        && e.session
                            .selection()
                            .shapes
                            .first()
                            .and_then(|id| e.session.model().slides.get(s.page)?.shape(*id))
                            .is_some_and(|sh| matches!(sh.kind, ShapeKind::Shape))
                });
                let can_edit_text = s.editor.as_ref().is_some_and(|e| {
                    sel_len == 1
                        && e.session
                            .selection()
                            .shapes
                            .first()
                            .and_then(|id| e.session.model().slides.get(s.page)?.shape(*id))
                            .is_some_and(|sh| {
                                matches!(sh.kind, ShapeKind::Shape | ShapeKind::Table(_))
                            })
                });
                let can_doc = s.editor.is_some() && !s.loading && !s.composing();
                (
                    sel_len,
                    is_basic_shape,
                    can_edit_text,
                    can_doc,
                    s.basic_edit_unavailable(BasicEdit::Duplicate, cx).is_none(),
                    s.basic_edit_unavailable(BasicEdit::Delete, cx).is_none(),
                    s.basic_edit_unavailable(BasicEdit::BringForward, cx)
                        .is_none(),
                    s.basic_edit_unavailable(BasicEdit::SendBackward, cx)
                        .is_none(),
                    s.basic_edit_unavailable(BasicEdit::NewSlide, cx).is_none(),
                    s.can_edit_outline(),
                    s.can_align(),
                    s.can_distribute(),
                    s.can_group(),
                    s.can_ungroup(),
                    s.inspector,
                )
            })
            .ok();

        let Some((
            sel_len,
            is_basic_shape,
            can_edit_text,
            can_doc,
            can_dup,
            can_del,
            can_fwd,
            can_bwd,
            can_new_slide,
            can_outline,
            can_align,
            can_dist,
            can_group,
            can_ungroup,
            inspector_open,
        )) = state
        else {
            return menu;
        };

        let locale = joyoffice_ui::current_locale(cx);
        let t = |key: &'static str| crate::l10n::t(locale, key);
        if sel_len == 0 {
            return menu
                .item(menu_item(
                    &view,
                    t("home.paste"),
                    I::ClipboardPaste,
                    can_doc,
                    |s, w, cx| s.paste_grid(w, cx),
                ))
                .separator()
                .item(menu_item(
                    &view,
                    t("menu.new_slide"),
                    I::FilePlus,
                    can_new_slide,
                    |s, w, cx| s.run_basic_edit(BasicEdit::NewSlide, w, cx),
                ))
                .item(menu_item(
                    &view,
                    t("ctx.format_bg"),
                    I::Palette,
                    can_doc,
                    |s, w, cx| s.content_form(content_ui::Form::Background, w, cx),
                ))
                .separator()
                .item(menu_item(
                    &view,
                    if inspector_open {
                        t("ctx.hide_selection_pane")
                    } else {
                        t("ctx.show_selection_pane")
                    },
                    I::PanelRight,
                    true,
                    |s, _, cx| {
                        s.inspector = !s.inspector;
                        cx.notify();
                    },
                ));
        }

        let align_view = view.clone();
        let mut menu = menu
            .item(menu_item(
                &view,
                t("home.cut"),
                I::Scissors,
                can_doc,
                |s, w, cx| s.clipboard_copy(true, w, cx),
            ))
            .item(menu_item(
                &view,
                t("home.copy"),
                I::Copy,
                can_doc,
                |s, w, cx| s.clipboard_copy(false, w, cx),
            ))
            .item(menu_item(
                &view,
                t("home.paste"),
                I::ClipboardPaste,
                can_doc,
                |s, w, cx| s.paste_grid(w, cx),
            ))
            .separator()
            .item(menu_item(
                &view,
                t("order.duplicate"),
                I::CopyPlus,
                can_dup,
                |s, w, cx| s.run_basic_edit(BasicEdit::Duplicate, w, cx),
            ))
            .item(menu_item(
                &view,
                t("common.delete"),
                I::Trash,
                can_del,
                |s, w, cx| s.run_basic_edit(BasicEdit::Delete, w, cx),
            ))
            .separator()
            .item(menu_item(
                &view,
                t("order.bring_forward"),
                I::BringToFront,
                can_fwd,
                |s, w, cx| s.run_basic_edit(BasicEdit::BringForward, w, cx),
            ))
            .item(menu_item(
                &view,
                t("order.send_backward"),
                I::SendToBack,
                can_bwd,
                |s, w, cx| s.run_basic_edit(BasicEdit::SendBackward, w, cx),
            ));

        if can_group {
            menu = menu.item(menu_item(
                &view,
                t("group.button"),
                I::Box,
                true,
                |s, w, cx| s.group_selected(w, cx),
            ));
        }
        if can_ungroup {
            menu = menu.item(menu_item(
                &view,
                t("group.ungroup"),
                I::SquareSplitHorizontal,
                true,
                |s, w, cx| s.ungroup_selected(w, cx),
            ));
        }

        menu = menu.submenu_with_icon(
            Some(Icon::new(I::LayoutGrid)),
            t("align.group_and_distribute"),
            w,
            cx,
            move |sub, _, _| {
                let t = |key: &'static str| crate::l10n::t(locale, key);
                let align_item = |label: &'static str, icon: I, edge: AlignEdge| {
                    let v = align_view.clone();
                    PopupMenuItem::new(label)
                        .icon(icon)
                        .disabled(!can_align)
                        .on_click(move |_, w, cx| {
                            let _ = v.update(cx, |s, cx| s.align_selected(edge, w, cx));
                        })
                };
                let dist_item = |label: &'static str, icon: I, axis: DistributeAxis| {
                    let v = align_view.clone();
                    PopupMenuItem::new(label)
                        .icon(icon)
                        .disabled(!can_dist)
                        .on_click(move |_, w, cx| {
                            let _ = v.update(cx, |s, cx| s.distribute_selected(axis, w, cx));
                        })
                };
                sub.item(align_item(
                    t("align.left"),
                    I::TextAlignStart,
                    AlignEdge::Left,
                ))
                .item(align_item(
                    t("align.center_h"),
                    I::TextAlignCenter,
                    AlignEdge::CenterH,
                ))
                .item(align_item(
                    t("align.right"),
                    I::TextAlignEnd,
                    AlignEdge::Right,
                ))
                .item(align_item(t("align.top"), I::ArrowUpToLine, AlignEdge::Top))
                .item(align_item(
                    t("align.middle_v"),
                    I::Columns2,
                    AlignEdge::MiddleV,
                ))
                .item(align_item(
                    t("align.bottom"),
                    I::ArrowLeftToLine,
                    AlignEdge::Bottom,
                ))
                .separator()
                .item(dist_item(
                    t("align.distribute_h"),
                    I::MoveHorizontal,
                    DistributeAxis::Horizontal,
                ))
                .item(dist_item(
                    t("align.distribute_v"),
                    I::GalleryVerticalEnd,
                    DistributeAxis::Vertical,
                ))
            },
        );

        menu = menu.separator();
        if can_edit_text {
            menu = menu.item(menu_item(
                &view,
                t("ctx.edit_text"),
                I::TextCursorInput,
                can_doc,
                |s, w, cx| s.begin_selected_text(w, cx),
            ));
        }
        if is_basic_shape {
            menu = menu.item(menu_item(
                &view,
                t("ctx.shape_fill"),
                I::PaintBucket,
                can_doc,
                |s, w, cx| s.content_form(content_ui::Form::ShapeFill, w, cx),
            ));
        }
        if can_outline {
            menu = menu.item(menu_item(
                &view,
                t("ctx.shape_outline"),
                I::PenLine,
                can_doc,
                |s, w, cx| s.content_form(content_ui::Form::ShapeLine, w, cx),
            ));
        }
        menu.item(menu_item(
            &view,
            t("pane.size_pos_btn"),
            I::Move,
            can_doc,
            |s, w, cx| s.geometry_dialog(w, cx),
        ))
    }
}
