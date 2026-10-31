//! Chart data, diagram text and animation panes. Draft fields never become document state until validated.
use super::*;
use gpui_kit::{
    assets::IconName as I,
    component::{Sizable, button::ButtonVariants},
};
use joydeck::{chart, diagram, motion};
fn caption(s: &str) -> Div {
    div()
        .text_size(px(11.))
        .text_color(rgb(0x687078))
        .child(s.to_owned())
}
fn button(id: &'static str, name: &'static str) -> Button {
    Button::new(id).ghost().small().label(name)
}
impl Editor {
    pub(super) fn author_object(&self) -> Option<(usize, Object)> {
        self.selected.and_then(|i| {
            self.session.deck.slides[self.page]
                .objects
                .get(i)
                .cloned()
                .map(|o| (i, o))
        })
    }
    pub(super) fn insert_authored(&mut self, kind: &str, w: &mut Window, cx: &mut Context<Self>) {
        self.finish_text(cx);
        let o = if ["list", "process", "hierarchy"].contains(&kind) {
            diagram::object(kind)
        } else {
            chart::object(kind)
        };
        self.command(Command::AddObject(self.page, o), cx);
        let i = self.session.deck.slides[self.page].objects.len() - 1;
        self.select(i, w, cx);
        self.ribbon = if kind == "list" || kind == "process" || kind == "hierarchy" {
            "图示"
        } else {
            "图表"
        }
        .into();
        self.sync_author(w, cx);
        cx.notify();
    }
    pub(super) fn sync_author(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, o)) = self.author_object() {
            let text = if let Some(c) = &o.chart {
                c.tsv()
            } else if let Some(d) = &o.diagram {
                d.text()
            } else {
                String::new()
            };
            self.chart_grid.clear();
            if o.chart.is_some() {
                for row in text.lines() {
                    self.chart_grid.push(
                        row.split('\t')
                            .map(|v| {
                                cx.new(|cx| {
                                    let mut f = InputState::new(w, cx);
                                    f.set_value(v, w, cx);
                                    f
                                })
                            })
                            .collect(),
                    );
                }
            }
            self.author_data
                .update(cx, |f, cx| f.set_value(text, w, cx));
            if let Some(c) = &o.chart {
                let vals = [
                    c.title.clone(),
                    c.x_title.clone(),
                    c.y_title.clone(),
                    c.unit.clone(),
                    c.minimum.map_or(String::new(), |v| v.to_string()),
                    c.maximum.map_or(String::new(), |v| v.to_string()),
                    c.colors.join(","),
                ];
                for (f, v) in self.author_fields.iter().zip(vals) {
                    f.update(cx, |f, cx| f.set_value(v, w, cx));
                }
            }
            self.author_fields[7].update(cx, |f, cx| {
                f.set_value(o.morph_key.unwrap_or_default(), w, cx)
            });
        }
    }
    pub(super) fn chart_op(&mut self, op: &str, w: &mut Window, cx: &mut Context<Self>) {
        let Some((i, mut o)) = self.author_object() else {
            return;
        };
        let value = if op == "grid-data" {
            self.chart_grid
                .iter()
                .map(|r| {
                    r.iter()
                        .map(|c| c.read(cx).value().to_string())
                        .collect::<Vec<_>>()
                        .join("\t")
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            self.author_data.read(cx).value().to_string()
        };
        let result = (|| -> std::result::Result<(), String> {
            if let Some(c) = &mut o.chart {
                match op {
                    "data" | "grid-data" => c.paste(&value)?,
                    "style" => {
                        c.title = self.author_fields[0].read(cx).value().to_string();
                        c.x_title = self.author_fields[1].read(cx).value().to_string();
                        c.y_title = self.author_fields[2].read(cx).value().to_string();
                        c.unit = self.author_fields[3].read(cx).value().to_string();
                        let num = |j: usize| -> std::result::Result<Option<f64>, String> {
                            let t = self.author_fields[j].read(cx).value();
                            if t.trim().is_empty() {
                                Ok(None)
                            } else {
                                t.parse()
                                    .map(Some)
                                    .map_err(|_| "坐标轴须为数字或空白（自动）".into())
                            }
                        };
                        c.minimum = num(4)?;
                        c.maximum = num(5)?;
                        c.colors = self.author_fields[6]
                            .read(cx)
                            .value()
                            .split(',')
                            .map(|s| s.trim().into())
                            .collect();
                    }
                    "legend" => c.legend = !c.legend,
                    "grid" => c.grid = !c.grid,
                    "labels" => c.labels = !c.labels,
                    _ => c.kind = op.into(),
                }
                c.validate()?;
            } else if let Some(d) = &mut o.diagram {
                if op == "data" {
                    d.set_text(&value)?
                } else {
                    d.layout = op.into();
                    d.validate()?;
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.command(Command::ReplaceObject(self.page, i, o), cx);
                self.sync_author(w, cx)
            }
            Err(e) => self.message = e,
        };
        cx.notify();
    }
    pub(super) fn convert_diagram(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some((i, o)) = self.author_object().filter(|(_, o)| o.diagram.is_some()) else {
            return;
        };
        let win = w.window_handle();
        let answer = w.prompt(
            PromptLevel::Warning,
            "转换为普通形状？",
            Some("将保留当前外观；图示文本层级、自动布局及此对象的动画目标将移除。可撤销此操作。"),
            &["转换", "取消"],
            cx,
        );
        cx.spawn(async move |view, cx| {
            if let Ok(0) = answer.await {
                let _ = win.update(cx, |_, w, cx| {
                    let _ = view.update(cx, |s, cx| {
                        if s.session.deck.slides[s.page].objects.get(i) != Some(&o) {
                            s.message = "对象已改变，请重新转换".into();
                            return;
                        }
                        match diagram::expand(&o) {
                            Ok(mut children) => {
                                for c in &mut children {
                                    c.id = String::new();
                                }
                                let mut cmds = vec![Command::DeleteObject(s.page, i)];
                                cmds.extend(
                                    children.into_iter().map(|o| Command::AddObject(s.page, o)),
                                );
                                s.command(Command::Batch(cmds), cx);
                                s.sync_fields(w, cx);
                            }
                            Err(e) => s.message = e,
                        }
                    });
                });
            }
        })
        .detach();
    }
    pub(super) fn timing_op(&mut self, op: &str, w: &mut Window, cx: &mut Context<Self>) {
        self.finish_text(cx);
        let mut p = self.session.deck.presentation.clone();
        let id = self.session.deck.slides[self.page].id.clone();
        let t = p.slides.entry(id).or_default();
        if ["none", "fade", "push", "wipe", "morph"].contains(&op) {
            t.transition = op.into();
        } else if op == "apply-all" {
            let current = t.clone();
            for page in &self.session.deck.slides {
                let target = p.slides.entry(page.id.clone()).or_default();
                target.transition = current.transition.clone();
                target.duration_ms = current.duration_ms;
            }
        } else if op == "advance" {
            match self.motion_fields[2].read(cx).value().parse::<u64>() {
                Ok(v) => t.advance_ms = (v > 0).then_some(v),
                Err(_) => {
                    self.message = "自动翻页须为毫秒，0为手动".into();
                    cx.notify();
                    return;
                }
            }
        } else if op == "loop" {
            p.loop_show = !p.loop_show;
        } else if op == "order" {
            let text = self.author_data.read(cx).value();
            let values = text
                .split(',')
                .map(|v| v.trim().parse::<usize>())
                .collect::<std::result::Result<Vec<_>, _>>();
            match values {
                Ok(v) => {
                    let ids = v
                        .into_iter()
                        .map(|i| {
                            self.session
                                .deck
                                .slides
                                .get(i.wrapping_sub(1))
                                .map(|s| s.id.clone())
                        })
                        .collect::<Option<Vec<_>>>();
                    if let Some(ids) = ids {
                        p.order = ids
                    } else {
                        self.message = "页序须为有效页码，例如1,3,2".into();
                        return;
                    }
                }
                Err(_) => {
                    self.message = "页序须为逗号分隔的页码".into();
                    return;
                }
            }
        } else if op == "link" {
            if let Some((_, o)) = self.author_object() {
                let value = self.author_fields[9].read(cx).value().parse::<usize>().ok();
                if let Some(page) =
                    value.and_then(|i| self.session.deck.slides.get(i.wrapping_sub(1)))
                {
                    p.links.insert(o.id, page.id.clone());
                } else {
                    self.message = "链接目标须为有效页码".into();
                    return;
                }
            }
        } else if op == "transition-time" {
            if let Ok(ms) = self.motion_fields[0].read(cx).value().parse::<u64>() {
                t.duration_ms = ms;
            } else {
                self.message = "转场时间须为整数毫秒".into();
                return;
            }
        } else if op == "morph-key" {
            if let Some((i, mut o)) = self.author_object() {
                let key = self.author_fields[7].read(cx).value().to_string();
                o.morph_key = (!key.is_empty()).then_some(key);
                self.command(Command::ReplaceObject(self.page, i, o), cx);
            }
            return;
        } else if op == "paragraph" {
            if let Some((_, o)) = self
                .author_object()
                .filter(|(_, o)| o.kind == "text" && o.rotation == 0.)
            {
                for (i, _) in o.text.split('\n').enumerate() {
                    t.effects.push(motion::Effect {
                        id: format!("paragraph-{}-{i}", self.visual_revision),
                        target: o.id.clone(),
                        paragraph: Some(i),
                        ..Default::default()
                    });
                }
            }
        } else if op == "delete" {
            if self.effect_pick < t.effects.len() {
                t.effects.remove(self.effect_pick);
                self.effect_pick = self.effect_pick.saturating_sub(1);
            }
        } else if op == "up" || op == "down" {
            let to = if op == "up" {
                self.effect_pick.saturating_sub(1)
            } else {
                self.effect_pick + 1
            };
            if to < t.effects.len() {
                t.effects.swap(self.effect_pick, to);
                self.effect_pick = to;
            }
        } else if op == "timing" {
            if let Some(e) = t.effects.get_mut(self.effect_pick) {
                let parsed = (0..2)
                    .map(|i| self.motion_fields[i].read(cx).value().parse::<u64>())
                    .collect::<std::result::Result<Vec<_>, _>>();
                if let Ok(v) = parsed {
                    e.duration_ms = v[0];
                    e.delay_ms = v[1];
                } else {
                    self.message = "时间须为整数毫秒".into();
                    return;
                }
            }
        } else if ["click", "with", "after", "repeat", "reverse", "rewind"].contains(&op) {
            if let Some(e) = t.effects.get_mut(self.effect_pick) {
                match op {
                    "repeat" => e.repeats = if e.repeats == 1 { 2 } else { 1 },
                    "reverse" => e.auto_reverse = !e.auto_reverse,
                    "rewind" => e.rewind = !e.rewind,
                    _ => e.trigger = op.into(),
                }
            }
        } else {
            let Some((_, o)) = self.author_object() else {
                self.message = "请先在画布中选择动画对象".into();
                cx.notify();
                return;
            };
            let id = format!(
                "effect-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            );
            t.effects.push(motion::Effect {
                id,
                target: o.id,
                kind: op.into(),
                ..Default::default()
            });
            self.effect_pick = t.effects.len() - 1;
        }
        self.command(Command::Motion(p), cx);
        w.refresh();
        cx.notify();
    }
    pub(super) fn author_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = div()
            .id("author-pane")
            .w(px(320.))
            .flex_shrink_0()
            .border_l_1()
            .border_color(rgb(0xd3d3d3))
            .bg(rgb(0xfafafa))
            .px_4()
            .py_3()
            .flex()
            .flex_col()
            .gap_3()
            .overflow_y_scroll();
        let chart = self.author_object().is_some_and(|(_, o)| o.chart.is_some());
        let diagram = self
            .author_object()
            .is_some_and(|(_, o)| o.diagram.is_some());
        body = body.child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(self.ribbon.clone()),
        );
        if self.ribbon == "动画" {
            let t = self
                .session
                .deck
                .presentation
                .timing(&self.session.deck.slides[self.page].id);
            body = body
                .child(caption("按列表顺序播放 · 选择一项修改时间"))
                .child(
                    button("paragraph", "按段落淡入")
                        .on_click(cx.listener(|s, _, w, cx| s.timing_op("paragraph", w, cx))),
                );
            for (i, e) in t.effects.iter().enumerate() {
                let text = format!("{}  {} · {} · {}ms", i + 1, e.target, e.kind, e.duration_ms);
                body = body.child(
                    Button::new(("effect", i))
                        .small()
                        .ghost()
                        .label(text)
                        .bg(rgb(if i == self.effect_pick {
                            0xf0dfd9
                        } else {
                            0xfafafa
                        }))
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.effect_pick = i;
                            let t = s
                                .session
                                .deck
                                .presentation
                                .timing(&s.session.deck.slides[s.page].id);
                            if let Some(e) = t.effects.get(i) {
                                for (j, v) in
                                    [(0, e.duration_ms.to_string()), (1, e.delay_ms.to_string())]
                                {
                                    s.motion_fields[j].update(cx, |f, cx| f.set_value(v, w, cx));
                                }
                            }
                            cx.notify();
                        })),
                );
            }
            for (i, name) in ["持续时间（毫秒）", "延迟（毫秒）"].iter().enumerate() {
                body = body
                    .child(caption(name))
                    .child(Input::new(&self.motion_fields[i]).small());
            }
            body = body.child(
                button("effect-time", "应用时间")
                    .on_click(cx.listener(|s, _, w, cx| s.timing_op("timing", w, cx))),
            );
            for row in [
                [
                    ("click", "单击时"),
                    ("with", "与上一项同时"),
                    ("after", "上一项之后"),
                ],
                [("up", "上移"), ("down", "下移"), ("delete", "删除动画")],
                [
                    ("repeat", "重复1/2次"),
                    ("reverse", "自动反向"),
                    ("rewind", "播放后倒回"),
                ],
            ] {
                body = body.child(div().flex().flex_wrap().children(row.into_iter().map(
                    |(op, name)| {
                        button(op, name)
                            .on_click(cx.listener(move |s, _, w, cx| s.timing_op(op, w, cx)))
                    },
                )));
            }
            body = body
                .child(caption("Morph 匹配名称（同页必须唯一）"))
                .child(Input::new(&self.author_fields[7]).small())
                .child(
                    button("morph-key", "设置匹配名称")
                        .on_click(cx.listener(|s, _, w, cx| s.timing_op("morph-key", w, cx))),
                );
        } else if self.ribbon == "幻灯片放映" {
            body = body
                .child(caption("对象链接目标页码"))
                .child(Input::new(&self.author_fields[9]).small())
                .child(
                    button("show-link", "设置所选对象链接")
                        .on_click(cx.listener(|s, _, w, cx| s.timing_op("link", w, cx))),
                )
                .child(caption("自定义页序（逗号分隔，从1开始）"))
                .child(Textarea::new(&self.author_data))
                .child(
                    button("show-order", "应用页序")
                        .on_click(cx.listener(|s, _, w, cx| s.timing_op("order", w, cx))),
                )
                .child(
                    button(
                        "show-loop",
                        if self.session.deck.presentation.loop_show {
                            "循环放映：开"
                        } else {
                            "循环放映：关"
                        },
                    )
                    .on_click(cx.listener(|s, _, w, cx| s.timing_op("loop", w, cx))),
                );
        } else if self.ribbon == "切换" {
            body = body
                .child(caption("转场持续时间（毫秒）"))
                .child(Input::new(&self.motion_fields[0]).small())
                .child(
                    button("transition-time", "应用转场时间")
                        .on_click(cx.listener(|s, _, w, cx| s.timing_op("transition-time", w, cx))),
                )
                .child(caption("自动翻页（毫秒，0为手动）"))
                .child(Input::new(&self.motion_fields[2]).small())
                .child(
                    button("show-advance", "应用自动翻页")
                        .on_click(cx.listener(|s, _, w, cx| s.timing_op("advance", w, cx))),
                )
                .child(
                    button("transition-all", "应用到全部")
                        .on_click(cx.listener(|s, _, w, cx| s.timing_op("apply-all", w, cx))),
                );
        } else if chart || diagram {
            body = body
                .child(caption(if chart {
                    "首行为系列名，Tab分列；空值保留为空。散点为成对X/Y列。"
                } else {
                    "每行一个节点；Tab缩进建立层级（最多8层）。"
                }))
                .child(Textarea::new(&self.author_data))
                .child(
                    button("author-apply", "应用数据")
                        .on_click(cx.listener(|s, _, w, cx| s.chart_op("data", w, cx))),
                );
            if chart {
                let mut grid = div()
                    .id("chart-data-grid")
                    .h(px(180.))
                    .overflow_scroll()
                    .flex()
                    .flex_col()
                    .gap_1();
                for (row_i, row) in self.chart_grid.iter().enumerate() {
                    grid = grid.child(
                        div()
                            .flex()
                            .gap_1()
                            .child(div().w(px(20.)).child((row_i + 1).to_string()))
                            .children(row.iter().map(|cell| Input::new(cell).small().w(px(96.)))),
                    );
                }
                body = body
                    .child(caption("数据网格（Tab切换单元格）"))
                    .child(grid)
                    .child(
                        button("apply-grid", "应用网格")
                            .on_click(cx.listener(|s, _, w, cx| s.chart_op("grid-data", w, cx))),
                    );
                for (i, name) in [
                    "图表标题",
                    "水平轴标题",
                    "垂直轴标题",
                    "单位",
                    "最小值（空白为自动）",
                    "最大值（空白为自动）",
                    "调色板（逗号分隔）",
                ]
                .iter()
                .enumerate()
                {
                    body = body
                        .child(caption(name))
                        .child(Input::new(&self.author_fields[i]).small());
                }
                body = body.child(
                    button("chart-style", "应用图表格式")
                        .on_click(cx.listener(|s, _, w, cx| s.chart_op("style", w, cx))),
                );
                for (op, name) in [
                    ("legend", "显示/隐藏图例"),
                    ("grid", "显示/隐藏网格"),
                    ("labels", "显示/隐藏标签"),
                ] {
                    body = body.child(
                        button(op, name)
                            .on_click(cx.listener(move |s, _, w, cx| s.chart_op(op, w, cx))),
                    );
                }
            } else {
                body = body.child(
                    button("diagram-convert", "转换为普通形状…")
                        .on_click(cx.listener(|s, _, w, cx| s.convert_diagram(w, cx))),
                );
            }
        } else {
            body = body.child(caption("选择图表或图示后编辑数据与布局。"));
        }
        body.into_any_element()
    }
    pub(super) fn author_ribbon(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut r = div().flex().items_start().gap_3();
        let entries: Vec<(&'static str, &'static str, I)> = match self.ribbon.as_str() {
            "切换" => vec![
                ("none", "无", I::Square),
                ("fade", "淡化", I::Layers),
                ("push", "推进", I::ArrowRight),
                ("wipe", "擦除", I::PanelLeft),
                ("morph", "平滑", I::Sparkles),
            ],
            "动画" => vec![
                ("appear", "出现", I::Star),
                ("fade", "淡入", I::Sparkles),
                ("fade-out", "淡出", I::EyeOff),
                ("fly", "飞入", I::ArrowUpRight),
                ("move", "移动", I::Move),
                ("zoom", "缩放", I::Maximize),
                ("spin", "旋转", I::RotateCw),
            ],
            "图表" => vec![
                ("column", "柱形图", I::ChartColumn),
                ("bar", "条形图", I::ChartBar),
                ("line", "折线图", I::ChartLine),
                ("area", "面积图", I::ChartLine),
                ("pie", "饼图", I::ChartPie),
                ("doughnut", "环形图", I::ChartPie),
                ("scatter", "散点图", I::ChartScatter),
            ],
            "图示" => vec![
                ("list", "列表", I::List),
                ("process", "流程", I::ArrowRight),
                ("hierarchy", "层级", I::Network),
            ],
            _ => vec![],
        };
        if self.ribbon == "动画" || self.ribbon == "幻灯片放映" {
            r = r.child(
                button("show-preview", "预览")
                    .icon(I::Play)
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.prepare_show(w, cx);
                        if let Err(e) =
                            crate::slideshow::open_preview(s.session.deck.clone(), s.page, cx)
                        {
                            s.message = e.to_string();
                        }
                    })),
            );
        }
        if self.ribbon == "幻灯片放映" {
            for (op, name) in [
                ("show-first", "从头播放"),
                ("show-current", "从当前幻灯片播放"),
            ] {
                r = r.child(
                    button(op, name)
                        .icon(I::Presentation)
                        .on_click(cx.listener(move |s, _, w, cx| s.delivery(op, w, cx))),
                );
            }
            r = r.child(
                button("rehearse", "排练计时")
                    .icon(I::Timer)
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.prepare_show(w, cx);
                        if let Err(e) =
                            crate::slideshow::open_rehearsal(s.session.deck.clone(), s.page, cx)
                        {
                            s.message = e.to_string();
                        }
                    })),
            );
            r = r.child(
                button("presenter", "演示者视图")
                    .icon(I::PanelsTopLeft)
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.prepare_show(w, cx);
                        if let Err(e) =
                            crate::slideshow::open_presenter(s.session.deck.clone(), s.page, cx)
                        {
                            s.message = e.to_string();
                        }
                    })),
            );
        }
        for (op, name, icon) in entries {
            let motion = self.ribbon == "动画" || self.ribbon == "切换";
            r = r.child(
                Button::new(op)
                    .accessibility_label(name)
                    .tooltip(name)
                    .ghost()
                    .h(px(58.))
                    .w(px(80.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .child(gpui_kit::component::Icon::from(icon).size(px(26.)))
                            .child(name),
                    )
                    .on_click(cx.listener(move |s, _, w, cx| {
                        if motion {
                            s.timing_op(op, w, cx)
                        } else {
                            s.chart_op(op, w, cx)
                        }
                    })),
            );
        }
        r.into_any_element()
    }
}
