//! Draft geometry stays in the view until one command commits on release.
use super::*;
use gpui::{Pixels, Point, fill, point, rgba};
use joyword_shell::authoring::{
    InlineObject, ObjectHit, StoryRef, Wrap, table_mouse as table, table_resize as resize,
};
use joyword_shell::editing::PageRect as Rect;
use serde_json::json;
#[derive(Clone)]
enum Kind {
    Picture { hit: ObjectHit, resize: bool },
    TableResize(resize::ResizeTarget),
    TableMove(resize::ResizeTarget),
    TableSelect,
}

pub(super) struct Gesture {
    document: u64,
    revision: u64,
    kind: Kind,
    page: usize,
    end_page: usize,
    start: (f32, f32),
    end: (f32, f32),
    shift: bool,
    ctrl: bool,
}
impl JoyWord {
    fn on_page(&self, page: usize, p: Point<Pixels>) -> Option<(f32, f32)> {
        let b = self.page_bounds.get(&page)?;
        let s = self.canvas.factor(self.shell.zoom);
        Some((
            f32::from(p.x - b.origin.x) / s,
            f32::from(p.y - b.origin.y) / s,
        ))
    }
    fn page_for(&self, p: Point<Pixels>) -> Option<usize> {
        self.page_bounds
            .iter()
            .find(|(_, b)| p.y >= b.top() && p.y <= b.bottom())
            .map(|(i, _)| *i)
    }
    fn gesture(
        &self,
        page: usize,
        at: (f32, f32),
        kind: Kind,
        mods: gpui::Modifiers,
    ) -> Option<Gesture> {
        let r = self.canvas.reader.as_ref()?;
        Some(Gesture {
            document: r.engine().document_id(),
            revision: r.engine().rev(),
            kind,
            page,
            end_page: page,
            start: at,
            end: at,
            shift: mods.shift,
            ctrl: mods.control,
        })
    }
    pub(super) fn pointer_down(
        &mut self,
        page: usize,
        e: &gpui::MouseDownEvent,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.canvas.loading || self.confirm_discard || self.replace_confirm.is_some() {
            return;
        }
        w.focus(&self.focus, cx);
        self.gesture = None;
        self.dragging = true;
        let Some((x, y)) = self.on_page(page, e.position) else {
            return;
        };
        let scale = self.canvas.factor(self.shell.zoom);
        let reach = 4. / scale;
        let Some(r) = self.canvas.reader.as_ref() else {
            return;
        };
        if let Some(hit) = r
            .picture_info()
            .and_then(|p| r.geometry().object(&p.pos, page))
        {
            let b = hit.bounds();
            if (x - (b.x + b.w)).abs() < 8. / scale && (y - (b.y + b.h)).abs() < 8. / scale {
                self.gesture = self.gesture(
                    page,
                    (x, y),
                    Kind::Picture { hit, resize: true },
                    e.modifiers,
                );
                return;
            }
        }
        if let Some(hit) = r.geometry().object_at(page, x, y, reach).filter(|h| {
            matches!(
                r.engine()
                    .doc
                    .para_at(&h.pos())
                    .and_then(|p| p.object_at(h.off)),
                Some(InlineObject::Image { .. })
            )
        }) {
            let pos = hit.pos();
            self.apply_input(w, cx, |r| r.select_object(pos));
            self.tab = 7;
            self.gesture = self.gesture(
                page,
                (x, y),
                Kind::Picture { hit, resize: false },
                e.modifiers,
            );
            return;
        }
        if let Some(mut target) =
            resize::resize_target(r.geometry(), &r.engine().doc, page, x, y, reach)
        {
            if e.click_count >= 2 && matches!(target.edge, resize::Edge::Column(_)) {
                let _ = self.try_edit(
                    "table.fitColumn",
                    &json!({"page":page,"x":x,"y":y,"reach":reach}),
                    w,
                    cx,
                );
                self.dragging = false;
                return;
            }
            target.limit_to_selection(r.engine(), y);
            self.tab = 6;
            self.gesture = self.gesture(page, (x, y), Kind::TableResize(target), e.modifiers);
            cx.notify();
            return;
        }
        if let Some(t) = table::target_at(
            r.geometry(),
            &r.engine().doc,
            StoryRef::Body,
            page,
            x,
            y,
            reach,
        ) {
            let moving = matches!(t.spot, table::Spot::Handle);
            let insertion = matches!(
                t.spot,
                table::Spot::InsertRow(_) | table::Spot::InsertColumn(_)
            );
            let cmd = if insertion {
                "table.insertAt"
            } else {
                "table.selectAt"
            };
            let _ = self.try_edit(
                cmd,
                &json!({"page":page,"x":x,"y":y,"extend":e.modifiers.shift,"reach":reach}),
                w,
                cx,
            );
            self.tab = 6;
            if !insertion {
                self.gesture = self.gesture(
                    page,
                    (x, y),
                    if moving {
                        Kind::TableMove(t.geom)
                    } else {
                        Kind::TableSelect
                    },
                    e.modifiers,
                );
            }
            cx.notify();
            return;
        }
        let was_table = r.table_info().is_some();
        self.mouse_selection(e.position, e.modifiers.shift, e.click_count, w, cx);
        if !was_table
            && self
                .canvas
                .reader
                .as_ref()
                .is_some_and(|r| r.table_info().is_some())
        {
            self.tab = 6;
        }
    }
    pub(super) fn pointer_move(
        &mut self,
        e: &gpui::MouseMoveEvent,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = self.gesture.as_ref().map(|g| g.page) else {
            self.mouse_selection(e.position, true, 1, w, cx);
            return;
        };
        let end_page = self.page_for(e.position).unwrap_or(page);
        let Some(at) = self.on_page(end_page, e.position) else {
            return;
        };
        if let Some(g) = &mut self.gesture {
            g.end_page = end_page;
            g.end = at;
            g.shift = e.modifiers.shift;
            g.ctrl = e.modifiers.control;
        }
        if let Some(g) = &self.gesture
            && matches!(g.kind, Kind::TableSelect)
        {
            let args = json!({"page":g.page,"x":g.start.0,"y":g.start.1,"toX":at.0,"toY":at.1,"extend":g.shift,"reach":4./self.canvas.factor(self.shell.zoom)});
            let _ = self.try_edit("table.selectAt", &args, w, cx);
        }
        cx.notify();
    }
    pub(super) fn pointer_up(
        &mut self,
        e: &gpui::MouseUpEvent,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.dragging {
            return;
        }
        self.dragging = false;
        let Some(g) = self.gesture.take() else {
            self.paint_brush(w, cx);
            return;
        };
        if self
            .viewport_bounds
            .is_some_and(|b| !b.contains(&e.position))
        {
            cx.notify();
            return;
        }
        if self.canvas.reader.as_ref().is_none_or(|r| {
            r.engine().document_id() != g.document || r.engine().rev() != g.revision
        }) {
            cx.notify();
            return;
        }
        let dx = g.end.0 - g.start.0;
        let dy = g.end.1 - g.start.1;
        let moved = dx.abs() + dy.abs() > 2.;
        let reach = 4. / self.canvas.factor(self.shell.zoom);
        match g.kind {
            Kind::TableSelect => self.paint_brush(w, cx),
            Kind::TableResize(_) | Kind::TableMove(_) if !moved => {}
            Kind::TableResize(_) => {
                let _=self.try_edit("table.resizeDrag",&json!({"page":g.page,"x":g.start.0,"y":g.start.1,"toX":g.end.0,"toY":g.end.1,"shift":g.shift,"ctrl":g.ctrl,"reach":reach}),w,cx);
            }
            Kind::TableMove(_) => {
                let _=self.try_edit("table.moveDrag",&json!({"page":g.page,"x":g.start.0,"y":g.start.1,"toX":g.end.0,"toY":g.end.1,"reach":reach}),w,cx);
            }
            Kind::Picture { hit, resize } if moved => {
                let args = if resize {
                    let sx = ((hit.bounds().w + dx) / hit.bounds().w.max(1.)).clamp(0.01, 100.);
                    let sy = ((hit.bounds().h + dy) / hit.bounds().h.max(1.)).clamp(0.01, 100.);
                    let factor = sx.max(sy);
                    json!({"width":(hit.rect.w*if g.shift{sx}else{factor}).clamp(1.,4000.),"height":(hit.rect.h*if g.shift{sy}else{factor}).clamp(1.,4000.),"lockAspect":!g.shift})
                } else if hit.wrap == Wrap::Inline {
                    let pos = self.canvas.reader.as_ref().and_then(|r| {
                        r.geometry()
                            .hit(g.end_page, g.end.0, g.end.1, StoryRef::Body)
                    });
                    let Some(pos) = pos else {
                        return;
                    };
                    json!({"to":pos})
                } else {
                    json!({"page":g.end_page,"x":hit.rect.x+dx,"y":hit.rect.y+dy})
                };
                let cmd = if resize {
                    "picture.size"
                } else if g.ctrl {
                    "arrange.copyTo"
                } else {
                    "arrange.bounds"
                };
                let _ = self.try_edit(cmd, &args, w, cx);
            }
            Kind::Picture { .. } => {}
        }
        cx.notify();
    }
    fn paint_brush(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self
            .canvas
            .reader
            .as_ref()
            .is_some_and(|r| r.engine().painter.is_some())
        {
            let _ = self.try_edit("edit.pasteFormat", &json!({}), w, cx);
        }
    }
    pub(super) fn paint_authoring(&self, page: usize, bounds: Bounds<Pixels>, w: &mut Window) {
        let Some(r) = self.canvas.reader.as_ref() else {
            return;
        };
        let scale = self.canvas.factor(self.shell.zoom);
        let convert = |b: Rect| {
            Bounds::new(
                point(
                    bounds.origin.x + px(b.x * scale),
                    bounds.origin.y + px(b.y * scale),
                ),
                size(px(b.w * scale), px(b.h * scale)),
            )
        };
        let line = |b: Rect, w: &mut Window| {
            w.paint_quad(fill(convert(b), rgb(0x315fa8)));
        };
        let outline = |b: Rect, w: &mut Window| {
            let t = 1.5 / scale;
            for q in [
                Rect::new(b.x, b.y, b.w, t),
                Rect::new(b.x, b.y + b.h - t, b.w, t),
                Rect::new(b.x, b.y, t, b.h),
                Rect::new(b.x + b.w - t, b.y, t, b.h),
            ] {
                line(q, w);
            }
        };
        if let Some(range) = r.engine().cell_range() {
            for (pi, b) in table::cell_rects(r.geometry(), &r.engine().doc, range) {
                if pi == page {
                    w.paint_quad(fill(convert(b), rgba(0x4285f430)));
                }
            }
        }
        if let Some(pic) = r
            .picture_info()
            .and_then(|p| r.geometry().object(&p.pos, page))
            .filter(|h| h.page == page)
        {
            let b = pic.bounds();
            outline(b, w);
            let h = 7. / scale;
            line(Rect::new(b.x + b.w - h / 2., b.y + b.h - h / 2., h, h), w);
        }
        if let Some((tp, _, _)) = r.engine().sel.focus.path.cell() {
            let (a, b) = table::caret_handles(
                r.geometry(),
                &r.engine().doc,
                r.engine().sel.focus.story,
                &tp,
                page,
            );
            for b in [a, b].into_iter().flatten() {
                outline(b, w);
            }
        }
        let Some(g) = &self.gesture else {
            return;
        };
        if page != g.end_page {
            return;
        }
        let (dx, dy) = (g.end.0 - g.start.0, g.end.1 - g.start.1);
        match &g.kind {
            Kind::Picture { hit, resize } => {
                let mut b = hit.bounds();
                if *resize {
                    let sx = ((b.w + dx) / b.w.max(1.)).max(0.01);
                    let sy = ((b.h + dy) / b.h.max(1.)).max(0.01);
                    b.w *= if g.shift { sx } else { sx.max(sy) };
                    b.h *= if g.shift { sy } else { sx.max(sy) };
                } else {
                    b.x += dx;
                    b.y += dy;
                }
                outline(b, w);
            }
            Kind::TableMove(t) => outline(
                Rect::new(
                    t.left() + dx,
                    t.top() + dy,
                    t.right() - t.left(),
                    t.bottom() - t.top(),
                ),
                w,
            ),
            Kind::TableResize(t) => {
                let share = if g.shift {
                    resize::Share::LeftOnly
                } else if g.ctrl {
                    resize::Share::Proportional
                } else {
                    resize::Share::Neighbour
                };
                let plan = t.plan(dx, dy, share);
                let widths = plan.widths.unwrap_or_else(|| t.widths());
                let mut x = t.left() + plan.indent_by.unwrap_or(0.);
                let top = t.top();
                let heights: Vec<f32> = t
                    .rows
                    .iter()
                    .map(|row| {
                        plan.heights
                            .iter()
                            .find(|(i, _)| *i == row.row)
                            .map_or(row.bottom - row.top, |(_, h)| *h)
                    })
                    .collect();
                let height: f32 = heights.iter().sum();
                for width in &widths {
                    line(Rect::new(x, top, 1. / scale, height), w);
                    x += width;
                }
                line(Rect::new(x, top, 1. / scale, height), w);
                let left = t.left() + plan.indent_by.unwrap_or(0.);
                let mut y = top;
                for h in heights {
                    line(Rect::new(left, y, x - left, 1. / scale), w);
                    y += h;
                }
                line(Rect::new(left, y, x - left, 1. / scale), w);
            }
            Kind::TableSelect => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{MouseDownEvent, MouseMoveEvent, MouseUpEvent, TestAppContext};
    use joyword_shell::{
        authoring::{Pos, image_bytes},
        reader::Reader,
    };
    use std::sync::atomic::AtomicBool;

    #[gpui::test]
    fn picture_drag_is_draft_until_release_cancel_and_stale_guard(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(JoyWord::new);
        view.update(cx, |v, cx| {
            let sample = Reader::from_bytes(
                "sample",
                include_bytes!("../../../assets/samples/mixed-media.docx"),
                &AtomicBool::new(false),
            )
            .unwrap();
            let bytes = sample
                .engine()
                .doc
                .media
                .values()
                .find_map(|b| image_bytes(b).ok())
                .unwrap();
            let mut r = Reader::blank(&AtomicBool::new(false)).unwrap();
            r.execute("insert.picture", &bytes).unwrap();
            r.select_object(Pos::body(0, 0)).unwrap();
            r.execute(
                "picture.size",
                &json!({"width":100.,"height":80.,"lockAspect":false}),
            )
            .unwrap();
            let doc = r.engine().document_id();
            v.canvas.reader = Some(r);
            v.install_input(doc, cx);
        });
        cx.update(|w, cx| w.draw(cx).clear(cx));
        let (start, end, depth) = view.read_with(cx, |v, _| {
            let r = v.canvas.reader.as_ref().unwrap();
            let hit = r.geometry().object(&Pos::body(0, 0), 0).unwrap().bounds();
            let b = v.page_bounds[&0];
            let scale = v.canvas.factor(v.shell.zoom);
            let p = point(
                b.origin.x + px((hit.x + hit.w) * scale),
                b.origin.y + px((hit.y + hit.h) * scale),
            );
            (
                p,
                point(p.x + px(20. * scale), p.y + px(16. * scale)),
                r.engine().undo_depth(),
            )
        });
        cx.update(|w, cx| {
            view.update(cx, |v, cx| {
                v.pointer_down(
                    0,
                    &MouseDownEvent {
                        position: start,
                        click_count: 1,
                        ..Default::default()
                    },
                    w,
                    cx,
                );
                assert!(v.gesture.is_some());
                v.pointer_move(
                    &MouseMoveEvent {
                        position: end,
                        ..Default::default()
                    },
                    w,
                    cx,
                );
                let r = v.canvas.reader.as_ref().unwrap();
                assert_eq!(r.engine().undo_depth(), depth);
                assert_eq!(r.picture_info().unwrap().width, 100.);
            })
        });
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |v, _| {
            assert!(v.gesture.is_none());
            assert_eq!(
                v.canvas
                    .reader
                    .as_ref()
                    .unwrap()
                    .picture_info()
                    .unwrap()
                    .width,
                100.
            );
        });
        cx.update(|w, cx| {
            view.update(cx, |v, cx| {
                v.pointer_down(
                    0,
                    &MouseDownEvent {
                        position: start,
                        click_count: 1,
                        ..Default::default()
                    },
                    w,
                    cx,
                );
                v.pointer_move(
                    &MouseMoveEvent {
                        position: end,
                        ..Default::default()
                    },
                    w,
                    cx,
                );
                v.pointer_up(
                    &MouseUpEvent {
                        position: end,
                        ..Default::default()
                    },
                    w,
                    cx,
                );
                let r = v.canvas.reader.as_ref().unwrap();
                assert_eq!(r.engine().undo_depth(), depth + 1);
                assert!((r.picture_info().unwrap().width - 120.).abs() < 0.01);
                assert!((r.picture_info().unwrap().height - 96.).abs() < 0.01);
                v.run_format("edit.undo", &json!({}), w, cx);
                assert_eq!(
                    v.canvas
                        .reader
                        .as_ref()
                        .unwrap()
                        .picture_info()
                        .unwrap()
                        .width,
                    100.
                );
                v.pointer_down(
                    0,
                    &MouseDownEvent {
                        position: start,
                        click_count: 1,
                        ..Default::default()
                    },
                    w,
                    cx,
                );
                v.pointer_move(
                    &MouseMoveEvent {
                        position: end,
                        ..Default::default()
                    },
                    w,
                    cx,
                );
                // An independent version change invalidates a pending mouse edit.
                v.canvas
                    .reader
                    .as_mut()
                    .unwrap()
                    .execute("picture.size", &json!({"width":90.}))
                    .unwrap();
                v.pointer_up(
                    &MouseUpEvent {
                        position: end,
                        ..Default::default()
                    },
                    w,
                    cx,
                );
                assert_eq!(
                    v.canvas
                        .reader
                        .as_ref()
                        .unwrap()
                        .picture_info()
                        .unwrap()
                        .width,
                    90.
                );
            })
        });
    }
    #[gpui::test]
    fn table_border_drag_one_history_step_and_undo(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(JoyWord::new);
        view.update(cx, |v, cx| {
            let mut r = Reader::blank(&AtomicBool::new(false)).unwrap();
            r.execute("insert.table", &json!({"rows":2,"cols":2}))
                .unwrap();
            let doc = r.engine().document_id();
            v.canvas.reader = Some(r);
            v.install_input(doc, cx);
        });
        cx.update(|w, cx| w.draw(cx).clear(cx));
        let (start, end, old, depth) = view.read_with(cx, |v, _| {
            let r = v.canvas.reader.as_ref().unwrap();
            let caret = r.geometry().caret(&r.engine().sel.focus).unwrap();
            let target = (1..600)
                .find_map(|x| {
                    resize::resize_target(
                        r.geometry(),
                        &r.engine().doc,
                        0,
                        x as f32,
                        caret.top + caret.height / 2.,
                        1.,
                    )
                    .filter(|t| t.edge == resize::Edge::Column(1))
                })
                .unwrap();
            let x = target.cols[1];
            let y = caret.top + caret.height / 2.;
            let b = v.page_bounds[&0];
            let scale = v.canvas.factor(v.shell.zoom);
            let p = point(b.origin.x + px(x * scale), b.origin.y + px(y * scale));
            (
                p,
                point(p.x + px(20. * scale), p.y),
                r.engine()
                    .doc
                    .table(StoryRef::Body, &joyword_shell::authoring::DocPath(vec![0]))
                    .unwrap()
                    .grid
                    .clone(),
                r.engine().undo_depth(),
            )
        });
        cx.update(|w, cx| {
            view.update(cx, |v, cx| {
                v.pointer_down(
                    0,
                    &MouseDownEvent {
                        position: start,
                        click_count: 1,
                        ..Default::default()
                    },
                    w,
                    cx,
                );
                assert!(matches!(
                    v.gesture.as_ref().map(|g| &g.kind),
                    Some(Kind::TableResize(_))
                ));
                v.pointer_move(
                    &MouseMoveEvent {
                        position: end,
                        ..Default::default()
                    },
                    w,
                    cx,
                );
                assert_eq!(
                    v.canvas.reader.as_ref().unwrap().engine().undo_depth(),
                    depth
                );
                v.pointer_up(
                    &MouseUpEvent {
                        position: end,
                        ..Default::default()
                    },
                    w,
                    cx,
                );
                let r = v.canvas.reader.as_ref().unwrap();
                assert_eq!(r.engine().undo_depth(), depth + 1);
                assert_ne!(
                    r.engine()
                        .doc
                        .table(StoryRef::Body, &joyword_shell::authoring::DocPath(vec![0]))
                        .unwrap()
                        .grid,
                    old
                );
                v.run_format("edit.undo", &json!({}), w, cx);
                assert_eq!(
                    v.canvas
                        .reader
                        .as_ref()
                        .unwrap()
                        .engine()
                        .doc
                        .table(StoryRef::Body, &joyword_shell::authoring::DocPath(vec![0]))
                        .unwrap()
                        .grid,
                    old
                );
            })
        });
    }
}
