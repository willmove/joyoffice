use super::*;
use gpui_kit::component::Sizable;
use joydeck::{
    playback,
    review::{self, Action},
};
fn label(s: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(12.))
        .text_color(rgb(0x555555))
        .child(s.into())
}
fn b(id: &'static str, text: &'static str) -> Button {
    Button::new(id).label(text).small()
}
impl Editor {
    pub(super) fn extended_ribbon(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .gap_2()
            .items_center()
            .child(label(match self.ribbon.as_str() {
                "审阅" => "评论、回复与文稿检查",
                "媒体" => "PCM/WAV · MP3 · H.264/AAC MP4（macOS）",
                _ => "PDF · 大纲 · 静态 GIF/MP4 · 标题字幕 VTT · POTX/PPSX",
            }))
            .child(
                b("extended-pane", "打开操作面板").on_click(cx.listener(|_, _, _, cx| cx.notify())),
            )
            .into_any_element()
    }
    pub(super) fn extended_review(&mut self, op: &str, cx: &mut Context<Self>) {
        self.finish_text(cx);
        let fields: Vec<_> = self
            .extended_fields
            .iter()
            .map(|f| f.read(cx).value().to_string())
            .collect();
        let slide = self.session.deck.slides[self.page].id.clone();
        let selected = self
            .selected
            .and_then(|i| self.session.deck.slides[self.page].objects.get(i))
            .map(|o| o.id.clone());
        let thread = self
            .session
            .deck
            .review
            .threads
            .iter()
            .filter(|t| t.slide == slide)
            .nth(self.thread_pick)
            .cloned();
        let action = match op {
            "add" => Some(Action::Add {
                id: format!("comment-{}-{}", self.session.id, self.session.revision),
                slide,
                object: selected,
                author: fields[0].clone(),
                text: fields[1].clone(),
            }),
            "reply" => thread.as_ref().map(|t| Action::Reply {
                id: t.id.clone(),
                author: fields[0].clone(),
                text: fields[1].clone(),
            }),
            "resolve" => thread.as_ref().map(|t| Action::Resolve {
                id: t.id.clone(),
                resolved: !t.resolved,
            }),
            "alt" | "decorative" => selected.map(|object| Action::Accessibility {
                object,
                alt: fields[2].clone(),
                decorative: op == "decorative",
            }),
            "order" => {
                let mut objects: Vec<_> = self.session.deck.slides[self.page]
                    .objects
                    .iter()
                    .map(|o| o.id.clone())
                    .collect();
                if let Some(id) = selected {
                    objects.retain(|o| o != &id);
                    objects.insert(0, id);
                }
                Some(Action::Order { slide, objects })
            }
            "check" => {
                let findings = review::check(&self.session.deck);
                self.review_findings = findings
                    .into_iter()
                    .map(|f| (f.slide, f.object, f.message))
                    .collect();
                self.message = format!("检查完成：{}项；未修改文稿", self.review_findings.len());
                cx.notify();
                return;
            }
            _ => None,
        };
        if let Some(a) = action {
            self.command(Command::Review(a), cx);
        } else {
            self.message = "请先选择评论或对象".into();
            cx.notify();
        }
    }
    pub(super) fn import_av(&mut self, cx: &mut Context<Self>) {
        let window = cx.active_window();
        let path = PathBuf::from(self.extended_fields[3].read(cx).value().trim());
        let revision = self.session.revision;
        let id = self.session.id;
        let slide = self.session.deck.slides[self.page].id.clone();
        self.message = "正在后台探测媒体…".into();
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { playback::import(&path) })
                .await;
            let _ = view.update(cx, |s, cx| {
                if s.session.id != id || s.session.revision != revision {
                    s.message = "文稿已改变，忽略媒体导入结果".into();
                    cx.notify();
                    return;
                }
                match result {
                    Ok(asset) => {
                        let key = asset.id();
                        let mut p = s.session.deck.playback.clone();
                        let end = asset.info.duration;
                        p.assets.insert(key.clone(), Arc::new(asset));
                        p.clips.push(playback::Clip {
                            id: format!("clip-{id}-{revision}"),
                            slide,
                            asset: key,
                            start: 0.,
                            end,
                            volume: 1.,
                            looping: false,
                            autoplay: false,
                            cross_slide: false,
                        });
                        s.command(Command::Playback(p), cx);
                        s.clip_pick = s.session.deck.playback.clips.len().saturating_sub(1);
                        if let Some(window) = window {
                            cx.spawn(async move |view, cx| {
                                let _ = window.update(cx, |_, w, cx| {
                                    let _ = view.update(cx, |s, cx| s.sync_clip_fields(w, cx));
                                });
                            })
                            .detach();
                        }
                    }
                    Err(e) => {
                        s.message = e;
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
    pub(super) fn sync_clip_fields(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(c) = self.session.deck.playback.clips.get(self.clip_pick) {
            for (j, v) in [c.start.to_string(), c.end.to_string(), c.volume.to_string()]
                .iter()
                .enumerate()
            {
                self.extended_fields[j + 4].update(cx, |f, cx| f.set_value(v, w, cx));
            }
        }
    }
    pub(super) fn media_action(&mut self, op: &str, cx: &mut Context<Self>) {
        if op == "import" {
            self.import_av(cx);
            return;
        }
        if op == "stop" {
            self.av_player = None;
            self.av_image = None;
            cx.notify();
            return;
        }
        if op == "pause" || op == "play" || op == "seek" {
            let seconds = self.extended_fields[7]
                .read(cx)
                .value()
                .parse::<f64>()
                .unwrap_or(0.);
            if let Some(p) = &mut self.av_player
                && let Err(e) = p.control(serde_json::json!({"command":op,"seconds":seconds}))
            {
                self.message = e;
            }
            cx.notify();
            return;
        }
        let Some(c) = self
            .session
            .deck
            .playback
            .clips
            .get(self.clip_pick)
            .cloned()
        else {
            self.message = "先导入或选择媒体".into();
            cx.notify();
            return;
        };
        if op == "preview" {
            self.av_player = None;
            self.av_image = None;
            let a = self.session.deck.playback.assets[&c.asset].clone();
            let mut playing = c.clone();
            playing.autoplay = true;
            match playback::Player::new(&a, &playing) {
                Ok(p) => {
                    self.av_document_id = self.session.id;
                    self.av_player = Some(p);
                    self.av_slide = Some(c.slide);
                    self.av_sequence = 0;
                    self.poll_av(cx);
                }
                Err(e) => self.message = e,
            }
            cx.notify();
            return;
        }
        let mut p = self.session.deck.playback.clone();
        let clip = &mut p.clips[self.clip_pick];
        match op {
            "params" => {
                let values =
                    [4, 5, 6].map(|i| self.extended_fields[i].read(cx).value().parse::<f64>());
                if let [Ok(start), Ok(end), Ok(volume)] = values {
                    clip.start = start;
                    clip.end = end;
                    clip.volume = volume as f32;
                } else {
                    self.message = "裁切/音量需要有效数值".into();
                    cx.notify();
                    return;
                }
            }
            "loop" => clip.looping = !clip.looping,
            "auto" => clip.autoplay = !clip.autoplay,
            "cross" => clip.cross_slide = !clip.cross_slide,
            "delete" => {
                p.clips.remove(self.clip_pick);
            }
            _ => return,
        }
        self.command(Command::Playback(p), cx);
    }
    fn poll_av(&mut self, cx: &mut Context<Self>) {
        self.av_task = Some(cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let Ok(keep) = view.update(cx, |s, cx| {
                    let Some(player) = &s.av_player else {
                        return false;
                    };
                    let status = player.status.lock().unwrap().clone();
                    if let Some(e) = &status.error {
                        s.message = format!("媒体错误：{e}");
                    }
                    if status.frame.is_none() {
                        s.av_image = None;
                    }
                    if status.sequence != s.av_sequence {
                        s.av_sequence = status.sequence;
                        if let Some(frame) = status.frame {
                            let image = image::RgbaImage::from_raw(
                                frame.width,
                                frame.height,
                                frame.bgra.clone(),
                            )
                            .unwrap();
                            s.av_image =
                                Some(Arc::new(RenderImage::new(vec![image::Frame::new(image)])));
                        }
                    }
                    s.av_seconds = status.seconds;
                    cx.notify();
                    true
                }) else {
                    break;
                };
                if !keep {
                    break;
                }
            }
        }));
    }
    fn toggle_control(&mut self, cx: &mut Context<Self>) {
        if self.control_service.take().is_some() {
            self.message = "本地控制已关闭".into();
            cx.notify();
            return;
        }
        let reads = self.extended_fields[9].read(cx).value().to_string();
        let writes = self.extended_fields[10].read(cx).value().to_string();
        let make = || -> joydeck::document::Result<joydeck::control::Service> {
            let mut d = joydeck::automation::Dispatcher::new(
                if reads.is_empty() {
                    vec![]
                } else {
                    vec![reads.into()]
                },
                if writes.is_empty() {
                    vec![]
                } else {
                    vec![writes.into()]
                },
            )?;
            d.session = self.session.clone();
            let path = std::env::temp_dir().join(format!(
                "joydeck-control-{}-{}.json",
                std::process::id(),
                self.session.id
            ));
            joydeck::control::Service::start(d, &path, true)
        };
        match make() {
            Ok(service) => {
                self.message = format!("本机控制已开启；凭据文件 {}", service.descriptor.display());
                self.control_service = Some(service);
            }
            Err(e) => self.message = e,
        }
        cx.notify();
    }
    pub(super) fn extended_export(&mut self, kind: &str, cx: &mut Context<Self>) {
        if self.delivery_busy {
            return;
        }
        let path = PathBuf::from(self.extended_fields[8].read(cx).value().trim());
        let d = self.session.deck.clone();
        let kind = kind.to_owned();
        let cancel = Arc::new(AtomicBool::new(false));
        self.delivery_cancel = cancel.clone();
        self.delivery_busy = true;
        self.message = "正在导出…".into();
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let pages: Vec<_> = d
                        .slides
                        .iter()
                        .enumerate()
                        .filter_map(|(i, s)| (!s.hidden).then_some(i))
                        .collect();
                    match kind.as_str() {
                        "vtt" => {
                            let text = joydeck::video::captions(&d, &pages, &Default::default())?;
                            joydeck::export::atomic(&path, text.as_bytes(), None, &cancel)
                        }
                        "pdf" => joydeck::output::vector_pdf(&path, &d, &pages, &cancel),
                        "mp4" => joydeck::video::export(
                            &path,
                            &d,
                            &pages,
                            &joydeck::video::Options::default(),
                            &cancel,
                        )
                        .map(|_| ()),
                        "print" => joydeck::output::print(&d, &pages, &cancel),
                        "potx" | "ppsx" => joydeck::output::presentation(&path, &d, &kind, &cancel),
                        _ => joydeck::output::other(&path, &d, &pages, &kind, &cancel),
                    }
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.delivery_busy = false;
                s.message = match result {
                    Ok(()) => "操作已完成；文稿未修改".into(),
                    Err(e) => e,
                };
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn extended_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut pane = div()
            .id("extended-pane-scroll")
            .w_full()
            .h_full()
            .overflow_y_scroll()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .text_size(px(13.))
            .bg(rgb(0xffffff));
        if self.ribbon == "审阅" {
            pane = pane
                .child(label("离线作者"))
                .child(Input::new(&self.extended_fields[0]).small())
                .child(label("评论 / 回复正文"))
                .child(Input::new(&self.extended_fields[1]).small());
            for (id, text, op) in [
                ("comment-add", "添加页/对象评论", "add"),
                ("comment-reply", "回复所选线程", "reply"),
                ("comment-resolve", "解决 / 重新打开", "resolve"),
            ] {
                pane = pane.child(
                    b(id, text).on_click(cx.listener(move |s, _, _, cx| s.extended_review(op, cx))),
                );
            }
            let slide = &self.session.deck.slides[self.page].id;
            for (i, t) in self
                .session
                .deck
                .review
                .threads
                .iter()
                .filter(|t| &t.slide == slide)
                .enumerate()
            {
                pane = pane.child(
                    Button::new(("thread", i))
                        .label(format!(
                            "{} {}",
                            if i == self.thread_pick { "●" } else { "○" },
                            if t.resolved { "已解决" } else { "待处理" }
                        ))
                        .small()
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.thread_pick = i;
                            cx.notify();
                        })),
                );
                for m in &t.messages {
                    pane = pane.child(
                        div()
                            .p_2()
                            .bg(rgb(0xf5f5f5))
                            .child(format!("{}：{}", m.author, m.text)),
                    );
                }
            }
            pane = pane
                .child(label("选中对象的替代文字"))
                .child(Input::new(&self.extended_fields[2]).small());
            for (id, text, op) in [
                ("review-alt", "应用替代文字", "alt"),
                ("review-decor", "标记为装饰对象", "decorative"),
                ("review-order", "选中对象优先阅读", "order"),
                ("review-check", "检查整个文稿", "check"),
            ] {
                pane = pane.child(
                    b(id, text).on_click(cx.listener(move |s, _, _, cx| s.extended_review(op, cx))),
                );
            }
            for (i, (slide, object, message)) in self.review_findings.iter().enumerate() {
                let page = self.session.deck.slides.iter().position(|p| &p.id == slide);
                let location = object.as_deref().unwrap_or("页面");
                let slide = slide.clone();
                let object = object.clone();
                pane = pane.child(
                    Button::new(("finding", i))
                        .label(format!(
                            "第{}页 · {location}：{message}",
                            page.unwrap_or(0) + 1
                        ))
                        .small()
                        .on_click(cx.listener(move |s, _, w, cx| {
                            if let Some(p) =
                                s.session.deck.slides.iter().position(|p| p.id == slide)
                            {
                                s.page = p;
                                if let Some(i) = object.as_ref().and_then(|id| {
                                    s.session.deck.slides[p]
                                        .objects
                                        .iter()
                                        .position(|o| &o.id == id)
                                }) {
                                    s.select(i, w, cx);
                                }
                                s.ribbon = "开始".into();
                                s.changed(cx);
                            }
                        })),
                );
            }
        } else if self.ribbon == "媒体" {
            pane = pane
                .child(label("音视频完整路径（单资源≤8MiB）"))
                .child(Input::new(&self.extended_fields[3]).small())
                .child(
                    b("av-import", "导入媒体")
                        .on_click(cx.listener(|s, _, _, cx| s.media_action("import", cx))),
                );
            for (i, c) in self.session.deck.playback.clips.iter().enumerate() {
                pane = pane.child(
                    Button::new(("clip", i))
                        .label(format!(
                            "{} {}",
                            if i == self.clip_pick { "●" } else { "○" },
                            c.id
                        ))
                        .small()
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.clip_pick = i;
                            let c = &s.session.deck.playback.clips[i];
                            let values =
                                [c.start.to_string(), c.end.to_string(), c.volume.to_string()];
                            for (j, v) in values.iter().enumerate() {
                                s.extended_fields[j + 4].update(cx, |f, cx| f.set_value(v, w, cx));
                            }
                            cx.notify();
                        })),
                );
            }
            if let Some(im) = &self.av_image {
                pane = pane.child(
                    img(im.clone())
                        .w_full()
                        .h(px(165.))
                        .flex_shrink_0()
                        .object_fit(ObjectFit::Contain),
                );
            }
            pane = pane.child(label(format!("播放位置：{:.2}s", self.av_seconds)));
            for (i, text) in [
                (4, "裁切起点（秒）"),
                (5, "裁切终点（秒）"),
                (6, "音量 0–1"),
                (7, "跳转到（秒）"),
            ] {
                pane = pane
                    .child(label(text))
                    .child(Input::new(&self.extended_fields[i]).small());
            }
            for (id, text, op) in [
                ("av-params", "应用裁切/音量", "params"),
                ("av-preview", "重新预览", "preview"),
                ("av-play", "播放", "play"),
                ("av-pause", "暂停", "pause"),
                ("av-seek", "跳转", "seek"),
                ("av-stop", "停止", "stop"),
                ("av-loop", "切换循环", "loop"),
                ("av-auto", "切换自动播放", "auto"),
                ("av-cross", "切换跨页音频", "cross"),
                ("av-delete", "删除媒体", "delete"),
            ] {
                pane = pane.child(
                    b(id, text).on_click(cx.listener(move |s, _, _, cx| s.media_action(op, cx))),
                );
            }
        } else {
            pane = pane
                .child(label("输出到不存在的新文件完整路径"))
                .child(Input::new(&self.extended_fields[8]).small())
                .child(label(
                    "GIF 为每页1秒的静态页序列。POTX/PPSX继承PPTX损失范围，评论和媒体仅原生保存。",
                ));
            for (id, text, kind) in [
                ("out-pdf", "矢量 PDF", "pdf"),
                ("out-print", "系统打印（macOS）", "print"),
                ("out-outline", "纯文字大纲", "outline"),
                ("out-rtf", "RTF 大纲", "rtf"),
                ("out-gif", "静态页 GIF", "gif"),
                ("out-mp4", "静态页 MP4（无音轨/动画）", "mp4"),
                ("out-vtt", "页面标题字幕 VTT（每页2秒）", "vtt"),
                ("out-potx", "POTX 模板（有损）", "potx"),
                ("out-ppsx", "PPSX 放映（有损）", "ppsx"),
            ] {
                pane = pane.child(
                    b(id, text)
                        .on_click(cx.listener(move |s, _, _, cx| s.extended_export(kind, cx))),
                );
            }
            pane = pane
                .child(
                    b("output-cancel", "取消正在进行的导出").on_click(cx.listener(
                        |s, _, _, cx| {
                            s.delivery_cancel
                                .store(true, std::sync::atomic::Ordering::Relaxed);
                            cx.notify();
                        },
                    )),
                )
                .child(label("自动化读取根目录（空=禁止）"))
                .child(Input::new(&self.extended_fields[9]).small())
                .child(label("自动化写入根目录（空=禁止）"))
                .child(Input::new(&self.extended_fields[10]).small())
                .child(
                    b(
                        "control-toggle",
                        if self.control_service.is_some() {
                            "关闭本机自动化"
                        } else {
                            "开启本机自动化"
                        },
                    )
                    .on_click(cx.listener(|s, _, _, cx| s.toggle_control(cx))),
                );
        }
        pane.into_any_element()
    }
}
