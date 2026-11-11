//! Native ink tools. Pointer gestures commit through the same session as typing.
use super::ribbon::Tool;
use super::*;
use joyoffice_ui::IconName as I;
use joyword_shell::authoring::DrawMode;
use serde_json::json;

impl JoyWord {
    pub(super) fn draw_ribbon(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tr = |zh, en| self.shell.tr(zh, en);
        let draw = self.canvas.reader.as_ref().map(|r| &r.engine().view.draw);
        let mode = draw.map(|d| d.mode).unwrap_or_default();
        let direct = |id, tool, command, args, selected| {
            self.ribbon_tool(id, tool, self.format_ready(command), false)
                .selected(selected)
                .on_click(cx.listener(move |v, _, w, cx| v.run_format(command, &args, w, cx)))
        };
        let input = direct(
            "draw-input",
            Tool::new("绘图", "Draw", I::Pencil),
            "draw.pen",
            json!({}),
            mode.pen().is_some(),
        );
        let undo = div()
            .flex()
            .flex_col()
            .child(
                self.ribbon_tool(
                    "draw-undo",
                    Tool::compact("撤销", "Undo", I::Undo).width(62.),
                    self.canvas.reader.as_ref().is_some_and(|r| r.can_undo()),
                    false,
                )
                .on_click(cx.listener(|v, _, w, cx| v.execute("edit.undo", w, cx))),
            )
            .child(
                self.ribbon_tool(
                    "draw-redo",
                    Tool::compact("重做", "Redo", I::Redo).width(62.),
                    self.canvas.reader.as_ref().is_some_and(|r| r.can_redo()),
                    false,
                )
                .on_click(cx.listener(|v, _, w, cx| v.execute("edit.redo", w, cx))),
            );
        let mut pens = div()
            .flex()
            .items_center()
            .gap(px(2.))
            .child(direct(
                "draw-select",
                Tool::new("选择", "Select", I::MousePointer2).width(44.),
                "draw.select",
                json!({}),
                mode == DrawMode::Select,
            ))
            .child(direct(
                "draw-lasso",
                Tool::new("套索选择", "Lasso Select", I::LassoSelect).width(58.),
                "draw.lasso",
                json!({}),
                mode == DrawMode::Lasso,
            ))
            .child(direct(
                "draw-eraser",
                Tool::new("橡皮擦", "Eraser", I::Eraser).width(48.),
                "draw.eraser",
                json!({}),
                mode == DrawMode::Eraser,
            ));
        for (id, zh, en, command, kind, color, width, icon) in [
            (
                "draw-black",
                "黑色笔",
                "Black Pen",
                "draw.pen",
                "pen",
                0x222222,
                1.5,
                I::PenTool,
            ),
            (
                "draw-red",
                "红色笔",
                "Red Pen",
                "draw.pen",
                "pen",
                0xc00000,
                1.5,
                I::PenTool,
            ),
            (
                "draw-pencil",
                "铅笔",
                "Pencil",
                "draw.pencil",
                "pencil",
                0x737373,
                1.,
                I::Pencil,
            ),
            (
                "draw-highlight",
                "荧光笔",
                "Highlighter",
                "draw.highlighter",
                "highlighter",
                0xffff00,
                12.,
                I::Highlighter,
            ),
            (
                "draw-blue",
                "蓝色笔",
                "Blue Pen",
                "draw.pen",
                "pen",
                0x0070c0,
                1.5,
                I::PenTool,
            ),
            (
                "draw-green",
                "绿色笔",
                "Green Pen",
                "draw.pen",
                "pen",
                0x008050,
                1.5,
                I::PenTool,
            ),
        ] {
            let selected = mode.pen().is_some_and(|tool| {
                tool.name() == kind
                    && draw.is_some_and(|d| d.settings(tool).color.hex() == format!("{color:06X}"))
            });
            pens = pens.child(direct(
                id,
                Tool::new(zh, en, icon).width(48.).tint(color),
                command,
                json!({"color":format!("{color:06X}"),"width":width}),
                selected,
            ));
        }
        if let Some(draw) = draw {
            for (index, pen) in draw.pens.iter().enumerate() {
                let command = match pen.kind.name() {
                    "pencil" => "draw.pencil",
                    "highlighter" => "draw.highlighter",
                    _ => "draw.pen",
                };
                let tint = u32::from_str_radix(&pen.color.hex(), 16).unwrap_or(0x222222);
                let args = json!({"color":pen.color.hex(),"width":pen.width});
                pens = pens.child(
                    self.ribbon_tool(
                        ("draw-custom-pen", index),
                        Tool::new("自定义笔", "Custom Pen", I::PenTool)
                            .width(48.)
                            .tint(tint),
                        self.format_ready(command),
                        false,
                    )
                    .tooltip(format!("{} · {} pt", pen.color.hex(), pen.width))
                    .selected(
                        mode.pen() == Some(pen.kind)
                            && draw.settings(pen.kind)
                                == joyword_shell::authoring::PenSettings {
                                    color: pen.color,
                                    width: pen.width,
                                },
                    )
                    .on_click(cx.listener(move |v, _, w, cx| v.run_format(command, &args, w, cx))),
                );
            }
        }
        pens = pens
            .child(
                self.ribbon_tool(
                    "draw-add-pen",
                    Tool::new("添加笔", "Add Pen", I::Plus).width(48.),
                    self.format_ready("draw.addPen"),
                    true,
                )
                .on_click(cx.listener(|v, _, w, cx| v.add_pen_form(w, cx))),
            )
            .child(
                self.ribbon_tool(
                    "draw-pen-options",
                    Tool::compact("颜色和粗细", "Color and Width", I::Settings2).width(96.),
                    mode.pen().is_some(),
                    true,
                )
                .on_click(cx.listener(|v, _, w, cx| v.pen_options_form(w, cx))),
            );
        let ruler = self
            .ribbon_tool(
                "draw-ruler",
                Tool::new("标尺", "Ruler", I::Ruler),
                false,
                false,
            )
            .tooltip(tr(
                "绘图直尺尚未实现",
                "The drawing ruler is not available yet",
            ));
        let background = self.format_choices(
            "draw-background-menu",
            self.ribbon_tool(
                "draw-background",
                Tool::new("设置背景格式", "Format Background", I::Grid2x2).width(72.),
                self.format_ready("view.gridlines"),
                true,
            ),
            "view.gridlines",
            vec![
                (tr("无", "None").into(), json!({"value":false})),
                (tr("网格线", "Gridlines").into(), json!({"value":true})),
            ],
            cx,
        );
        let convert = div()
            .flex()
            .child(direct(
                "draw-to-shape",
                Tool::new("将墨迹转换为形状", "Ink to Shape", I::Shapes).width(74.),
                "draw.inkToShape",
                json!({}),
                draw.is_some_and(|d| d.ink_to_shape),
            ))
            .child(self.ribbon_tool(
                "draw-to-math",
                Tool::new("将墨迹转换为数学公式", "Ink to Math", I::Sigma).width(74.),
                false,
                false,
            ));
        let canvas = self.ribbon_tool(
            "draw-canvas",
            Tool::new("画布", "Drawing Canvas", I::SquareDashed).width(56.),
            false,
            false,
        );
        let replay = self
            .ribbon_tool(
                "draw-replay",
                Tool::new("墨迹重播", "Ink Replay", I::Play).width(58.),
                false,
                false,
            )
            .tooltip(tr(
                "墨迹时间轴播放尚未接入桌面",
                "Ink timeline playback is not available yet",
            ));
        let help = self
            .ribbon_tool(
                "draw-help",
                Tool::new("墨迹帮助", "Ink Help", I::CircleQuestionMark).width(58.),
                true,
                false,
            )
            .on_click(cx.listener(|v, _, w, cx| v.show_help(true, w, cx)));
        div()
            .flex()
            .flex_shrink_0()
            .child(self.ribbon_centered_group(
                "draw-input-group",
                tr("输入模式", "Input Mode"),
                input,
            ))
            .child(self.ribbon_centered_group("draw-undo-group", tr("撤消", "Undo"), undo))
            .child(self.ribbon_centered_group(
                "draw-tools-group",
                tr("绘图工具", "Drawing Tools"),
                pens,
            ))
            .child(self.ribbon_centered_group("draw-ruler-group", tr("模具", "Stencil"), ruler))
            .child(self.ribbon_centered_group("draw-edit-group", tr("编辑", "Editing"), background))
            .child(self.ribbon_centered_group("draw-convert-group", tr("转换", "Convert"), convert))
            .child(self.ribbon_centered_group("draw-canvas-group", tr("插入", "Insert"), canvas))
            .child(self.ribbon_centered_group("draw-replay-group", tr("重播", "Replay"), replay))
            .child(self.ribbon_centered_group("draw-help-group", tr("帮助", "Help"), help))
    }

    fn add_pen_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.fields_form("添加笔（本次会话） / Add Pen (this session)","draw.addPen",vec![("类型 / Kind (pen / pencil / highlighter)","pen".into()),("颜色 / Color (RRGGBB)","0070C0".into()),("粗细 / Width (pt)","1.5".into())],false,|v| {
            if !["pen","pencil","highlighter"].contains(&v[0].trim()){return Err("类型应为 pen、pencil 或 highlighter".into());}
            let color=v[1].trim().trim_start_matches('#');
            if color.len()!=6 || !color.bytes().all(|b|b.is_ascii_hexdigit()){return Err("颜色需要 6 位 RGB / A six-digit RGB color is required".into());}
            Ok(json!({"kind":v[0].trim(),"color":color,"width":joyword_shell::authoring::required_number(&v[2],0.25,30.)?}))
        },w,cx);
    }

    fn pen_options_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some((tool, settings)) = self.canvas.reader.as_ref().and_then(|r| {
            r.engine()
                .view
                .draw
                .mode
                .pen()
                .map(|t| (t, r.engine().view.draw.settings(t)))
        }) else {
            return;
        };
        let command = match tool.name() {
            "pencil" => "draw.pencil",
            "highlighter" => "draw.highlighter",
            _ => "draw.pen",
        };
        self.fields_form("笔的颜色和粗细 / Pen Color and Width",command,vec![("颜色 / Color (RRGGBB)",settings.color.hex()),("粗细 / Width (pt)",settings.width.to_string())],false,|v| {
            let color=v[0].trim().trim_start_matches('#');
            if color.len()!=6 || !color.bytes().all(|b|b.is_ascii_hexdigit()){return Err("颜色需要 6 位 RGB / A six-digit RGB color is required".into());}
            Ok(json!({"color":color,"width":joyword_shell::authoring::required_number(&v[1],0.25,30.)?}))
        },w,cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{MouseDownEvent, MouseMoveEvent, MouseUpEvent, TestAppContext, point};
    use joyword_shell::reader::Reader;
    use std::sync::atomic::AtomicBool;

    #[gpui::test]
    fn native_ink_gestures_preview_cancel_commit_once_and_reject_stale_document(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(JoyWord::new);
        view.update(cx, |v, cx| {
            let r = Reader::blank(&AtomicBool::new(false)).unwrap();
            let id = r.engine().document_id();
            v.canvas.reader = Some(r);
            v.install_input(id, cx);
            v.tab = 10;
        });
        cx.update(|w, cx| {
            view.update(cx, |v, cx| {
                v.run_format("draw.pen", &json!({"color":"C00000","width":2}), w, cx)
            });
            w.draw(cx).clear(cx);
        });
        let (start, end, depth) = view.read_with(cx, |v, _| {
            let bounds = v.page_bounds[&0];
            let scale = v.canvas.factor(v.shell.zoom);
            let start = point(
                bounds.origin.x + px(100. * scale),
                bounds.origin.y + px(100. * scale),
            );
            (
                start,
                point(start.x + px(90. * scale), start.y + px(40. * scale)),
                v.canvas.reader.as_ref().unwrap().engine().undo_depth(),
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
            })
        });
        cx.update(|w, cx| w.draw(cx).clear(cx));
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |v, _| {
            assert!(v.gesture.is_none());
            assert_eq!(
                v.canvas.reader.as_ref().unwrap().engine().undo_depth(),
                depth
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
                assert_eq!(
                    r.engine()
                        .doc
                        .body
                        .iter()
                        .filter_map(|b| b.as_para())
                        .flat_map(|p| p.objects.iter())
                        .filter(|o| o.ink().is_some())
                        .count(),
                    1
                );
                v.run_format("edit.undo", &json!({}), w, cx);
                assert!(
                    v.canvas
                        .reader
                        .as_ref()
                        .unwrap()
                        .engine()
                        .doc
                        .body
                        .iter()
                        .filter_map(|b| b.as_para())
                        .all(|p| p.objects.iter().all(|o| o.ink().is_none()))
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
                v.run_format("text.insert", &json!({"text":"Changed during drag"}), w, cx);
                v.pointer_up(
                    &MouseUpEvent {
                        position: end,
                        ..Default::default()
                    },
                    w,
                    cx,
                );
                assert!(
                    v.canvas
                        .reader
                        .as_ref()
                        .unwrap()
                        .engine()
                        .doc
                        .body
                        .iter()
                        .filter_map(|b| b.as_para())
                        .all(|p| p.objects.iter().all(|o| o.ink().is_none()))
                );
            })
        });
    }
}
