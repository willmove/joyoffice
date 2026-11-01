//! Document surface and selection overlays share the same scale and bounds.
use super::ui::*;
use super::*;
impl Editor {
    pub(super) fn slide_surface(&self, scale: f32, cx: &mut Context<Self>) -> AnyElement {
        let objects = self.display_objects();
        let width = self.session.deck.width * scale;
        let height = self.session.deck.height * scale;
        div()
            .relative()
            .w(px(width))
            .h(px(height))
            .bg(rgb(0xffffff))
            .children(
                self.image
                    .as_ref()
                    .filter(|_| !self.accelerated)
                    .map(|i| img(i.clone()).w(px(width)).h(px(height))),
            )
            .id("slide-surface")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, e: &MouseDownEvent, w, cx| s.down(e, scale, w, cx)),
            )
            .child({
                let weak = cx.entity().downgrade();
                let paint_weak = weak.clone();
                canvas(
                    move |bounds, _, cx| {
                        let _ = weak.update(cx, |s, _| {
                            s.canvas_origin =
                                (f32::from(bounds.origin.x), f32::from(bounds.origin.y))
                        });
                    },
                    move |bounds, _, w, cx| {
                        let _ = paint_weak.update(cx, |s, cx| {
                            s.paint_scene(bounds, scale, w, cx);
                            s.paint_text(bounds, scale, w, cx)
                        });
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full()
            })
            .children(
                objects
                    .iter()
                    .filter(|o| self.selection.contains(&o.id))
                    .map(|o| {
                        let r = geometry::bounds(o);
                        div()
                            .absolute()
                            .left(px(r.x * scale))
                            .top(px(r.y * scale))
                            .w(px(r.w * scale))
                            .h(px(r.h * scale))
                            .border_1()
                            .border_color(rgb(ACCENT))
                    }),
            )
            .children({
                let selected: Vec<_> = objects
                    .iter()
                    .filter(|o| self.selection.contains(&o.id))
                    .cloned()
                    .collect();
                if selected.is_empty() {
                    Vec::new()
                } else {
                    let r = geometry::union(&selected);
                    let mut handles = Vec::new();
                    for (hx, hy) in [
                        (-1, -1),
                        (0, -1),
                        (1, -1),
                        (-1, 0),
                        (1, 0),
                        (-1, 1),
                        (0, 1),
                        (1, 1),
                    ] {
                        handles.push(
                            div()
                                .absolute()
                                .left(px((r.x + r.w * (hx as f32 + 1.) / 2.) * scale - 5.))
                                .top(px((r.y + r.h * (hy as f32 + 1.) / 2.) * scale - 5.))
                                .w(px(10.))
                                .h(px(10.))
                                .bg(rgb(ACCENT)),
                        );
                    }
                    handles.push(
                        div()
                            .absolute()
                            .left(px((r.x + r.w / 2.) * scale - 5.))
                            .top(px(r.y * scale - 29.))
                            .w(px(10.))
                            .h(px(10.))
                            .rounded_full()
                            .bg(rgb(ACCENT)),
                    );
                    handles
                }
            })
            .children(self.gesture.as_ref().filter(|g| g.mode.is_none()).map(|g| {
                div()
                    .absolute()
                    .left(px(g.start.0.min(g.end.0) * scale))
                    .top(px(g.start.1.min(g.end.1) * scale))
                    .w(px((g.end.0 - g.start.0).abs() * scale))
                    .h(px((g.end.1 - g.start.1).abs() * scale))
                    .border_1()
                    .border_color(rgb(ACCENT))
            }))
            .children(self.table_overlays(scale))
            .shadow_lg()
            .into_any_element()
    }
    fn table_overlays(&self, scale: f32) -> Vec<Div> {
        let Some((o, t)) = self.table_target() else {
            return vec![];
        };
        if o.rotation != 0. {
            return vec![];
        }
        let Some(p) = self.table_pick.as_ref() else {
            return vec![];
        };
        let Ok(range) = t.selection_range(p.anchor, p.focus) else {
            return vec![];
        };
        let mut seen = BTreeSet::new();
        let mut result = vec![];
        for r in range.top..=range.bottom {
            for c in range.left..=range.right {
                if let Ok((rr, cc)) = t.owner(r, c)
                    && seen.insert((rr, cc))
                    && let Ok([x, y, w, h]) = t.bounds(rr, cc)
                {
                    result.push(
                        div()
                            .absolute()
                            .left(px((o.x + x) * scale))
                            .top(px((o.y + y) * scale))
                            .w(px(w * scale))
                            .h(px(h * scale))
                            .border_2()
                            .border_color(rgb(ACCENT)),
                    );
                }
            }
        }
        result
    }
}
