//! Audience/presenter lifecycle, clock and owned media workers adapted from JoyOffice slideshow.
use super::*;
use joydeck_shell::{
    Cancellation,
    show::Player,
    show_media::{Control, Media},
};
use std::{sync::Mutex, time::Duration};

struct Runtime {
    player: Player,
    closed: bool,
    presenter: Option<AnyWindowHandle>,
    media: Control,
    status: serde_json::Value,
}
type Shared = Arc<Mutex<Runtime>>;
impl Runtime {
    fn media_enabled(&self) -> bool {
        self.media.enabled.unwrap_or_else(|| {
            self.player.model.slides[self.player.state.slide]
                .shapes
                .iter()
                .any(|s| matches!(&s.kind,joydeck_model::ShapeKind::Media(c) if c.autoplay))
        })
    }
    fn changed_page(&mut self, old: usize) {
        if self.player.state.slide != old {
            self.media = Control {
                muted: self.media.muted,
                ..Default::default()
            };
            self.status = serde_json::Value::Null;
        }
    }
}
struct Audience {
    shared: Shared,
    focus: FocusHandle,
    image: Option<Arc<RenderImage>>,
    retired: Vec<Arc<RenderImage>>,
    key: Option<(u64, usize, usize, u64)>,
    font_revision: u64,
    busy: bool,
    task: Option<Task<()>>,
    clock: Option<Task<()>>,
    media: Arc<Mutex<Media>>,
    cancel: Cancellation,
    message: String,
    origin: Point<Pixels>,
    scale: f32,
    laser: bool,
    pen: bool,
    drawing: bool,
    pointer: Point<Pixels>,
    ink: Vec<Vec<(f32, f32)>>,
    frames: u64,
}
impl Drop for Audience {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
fn image(mut raster: joydeck_render::Image) -> std::result::Result<Arc<RenderImage>, String> {
    for p in raster.pixels.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
    }
    let pixels = image::RgbaImage::from_raw(raster.width, raster.height, raster.pixels)
        .ok_or("show image byte count mismatch")?;
    Ok(Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])))
}
impl Audience {
    fn exit(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.cancel.cancel();
        let presenter = self.shared.lock().ok().and_then(|mut r| {
            r.closed = true;
            r.presenter.take()
        });
        if let Some(h) = presenter {
            let _ = h.update(cx, |_, w, _| w.remove_window());
        }
        eprintln!("R7 show_closed frames={}", self.frames);
        w.remove_window();
    }
    fn navigation(&mut self, key: &str, w: &mut Window, cx: &mut Context<Self>) {
        if key == "escape" {
            self.exit(w, cx);
            return;
        }
        if key == "f11" {
            w.toggle_fullscreen();
            return;
        }
        if key == "l" {
            self.laser = !self.laser;
            cx.notify();
            return;
        }
        if key == "i" {
            self.pen = !self.pen;
            cx.notify();
            return;
        }
        if key == "e" {
            self.ink.clear();
            cx.notify();
            return;
        }
        let mut quit = false;
        if let Ok(mut r) = self.shared.lock() {
            if key == "m" {
                r.media.enabled = Some(!r.media_enabled());
                r.player.serial += 1;
            } else if key == "j" || key == "k" {
                let seconds = (r.status["seconds"].as_f64().unwrap_or(0.)
                    + if key == "j" { -5. } else { 5. })
                .clamp(0., 3600.);
                r.player.serial += 1;
                r.media.seek = Some((r.player.serial, seconds));
            } else if key == "v" {
                r.media.muted = !r.media.muted;
                r.player.serial += 1;
            } else {
                let action = match key {
                    "right" | "space" | "enter" | "pagedown" => "next",
                    "left" | "backspace" | "pageup" => "previous",
                    "p" => "pause",
                    "b" => "black",
                    "w" => "white",
                    _ => return,
                };
                let old = r.player.state.slide;
                match r.player.navigate(action, None) {
                    Ok(q) => quit = q,
                    Err(e) => self.message = e,
                }
                if r.player.state.slide != old {
                    r.changed_page(old);
                    self.ink.clear();
                }
            }
            eprintln!("R7 show_state {}", r.player.status());
        }
        if quit {
            self.exit(w, cx);
        } else {
            cx.notify();
        }
    }
    fn request(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Ok(r) = self.shared.lock() else { return };
        if r.closed {
            return;
        }
        let model = r.player.clone();
        let control = r.media.clone();
        let mut key = model.frame_key();
        let media_on = r.media_enabled();
        if media_on && !model.paused && r.status["paused"] != true {
            key.3 = (model.total * 10.) as u64;
        }
        let font_revision = joydeck_fonts::FontDb::global().environment_revision();
        if self.key == Some(key) && self.font_revision == font_revision {
            return;
        }
        drop(r);
        self.key = Some(key);
        self.font_revision = font_revision;
        self.busy = true;
        let media = self.media.clone();
        let cancel = self.cancel.clone();
        self.task = Some(cx.spawn(async move |view, cx| {
            let started = Instant::now();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut raster = model.frame(1., &cancel)?;
                    let mut media = media
                        .lock()
                        .map_err(|_| "media state lock poisoned".to_string())?;
                    let status = media.composite(&model, &control, &mut raster);
                    if status.is_err() {
                        media.clear();
                    }
                    cancel.check()?;
                    Ok::<_, String>((image(raster)?, status))
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.busy = false;
                if s.cancel.check().is_err() {
                    return;
                }
                let current = s.shared.lock().ok().map(|r| r.player.frame_key());
                if !current.is_some_and(|k| (k.0, k.1, k.2) == (key.0, key.1, key.2))
                    || joydeck_fonts::FontDb::global().environment_revision() != font_revision
                {
                    s.key = None;
                    cx.notify();
                    return;
                }
                match result {
                    Ok((image, status)) => {
                        if let Some(old) = s.image.replace(image) {
                            s.retired.push(old);
                        }
                        s.frames += 1;
                        if let Ok(mut r) = s.shared.lock() {
                            match status {
                                Ok(value) => r.status = value,
                                Err(e) => {
                                    s.message = e;
                                    r.media.enabled = Some(false);
                                }
                            }
                        }
                        if s.frames == 1 {
                            eprintln!(
                                "R7 show_first_frame prepare_us={}",
                                started.elapsed().as_micros()
                            );
                        }
                    }
                    Err(e) => s.message = e,
                }
                cx.notify();
            });
        }));
    }
}
impl Focusable for Audience {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for Audience {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for old in self.retired.drain(..) {
            let _ = w.drop_image(old);
        }
        self.request(cx);
        let (width, height, page, step, paused, blank, ended) = self
            .shared
            .lock()
            .map(|r| {
                (
                    r.player.model.slide_size.width as f32,
                    r.player.model.slide_size.height as f32,
                    r.player.state.slide + 1,
                    r.player.state.step,
                    r.player.paused,
                    r.player.blank,
                    r.player.state.ended,
                )
            })
            .unwrap_or((960., 540., 0, 0, true, None, true));
        let viewport = w.viewport_size();
        let scale = (f32::from(viewport.width) / width)
            .min((f32::from(viewport.height) - 42.) / height)
            .max(0.01);
        self.origin = point(
            px((f32::from(viewport.width) - width * scale) / 2.),
            px((f32::from(viewport.height) - 42. - height * scale) / 2.),
        );
        self.scale = scale;
        let ink = self.ink.clone();
        let origin = self.origin;
        div().id("joydeck-audience").size_full().bg(rgb(if blank==Some(true){0xffffff}else{0})).text_color(rgb(0xffffff)).relative().track_focus(&self.focus).key_context("Slideshow")
            .on_key_down(cx.listener(|s,e:&KeyDownEvent,w,cx|{s.navigation(&e.keystroke.key,w,cx);cx.stop_propagation();}))
            .on_mouse_down(MouseButton::Left,cx.listener(|s,e:&MouseDownEvent,_,cx|{
                if s.pen {if s.ink.len()<256 {s.ink.push(vec![(f32::from(e.position.x-s.origin.x)/s.scale,f32::from(e.position.y-s.origin.y)/s.scale)]);s.drawing=true;}}
                else if !s.laser {
                    let opened = s.shared.lock().ok().and_then(|mut r| {
                        let old = r.player.state.slide;
                        let result = r.player.click(
                            f64::from(f32::from(e.position.x - s.origin.x) / s.scale),
                            f64::from(f32::from(e.position.y - s.origin.y) / s.scale),
                        );
                        r.changed_page(old);
                        result.ok().flatten()
                    });
                    if let Some(url) = opened {
                        cx.open_url(&url);
                    }
                }
                cx.notify();
            }))
            .on_mouse_move(cx.listener(|s,e:&MouseMoveEvent,_,cx|{s.pointer=e.position;
                if s.drawing && s.ink.iter().map(Vec::len).sum::<usize>()<2048 && let Some(line)=s.ink.last_mut(){line.push((f32::from(e.position.x-s.origin.x)/s.scale,f32::from(e.position.y-s.origin.y)/s.scale));}
                if s.laser||s.drawing{cx.notify();}}))
            .on_mouse_up(MouseButton::Left,cx.listener(|s,_,_,cx|{s.drawing=false;cx.notify();}))
            .children(self.image.as_ref().filter(|_|blank.is_none()&&!ended).map(|i|img(i.clone()).absolute().left(self.origin.x).top(self.origin.y).w(px(width*scale)).h(px(height*scale))))
            .children(ended.then(||div().absolute().top(px(100.)).left(px(100.)).child("放映结束 · Esc 退出")))
            .child(canvas(|_,_,_|(),move|_,_,w,_|{if blank.is_none() && !ended {for line in &ink {let mut path=PathBuilder::stroke(px(2.));for(i,(x,y))in line.iter().enumerate(){let p=point(origin.x+px(x*scale),origin.y+px(y*scale));if i==0{path.move_to(p);}else{path.line_to(p);}}if let Ok(p)=path.build(){w.paint_path(p,rgb(0xff3535));}}}}).absolute().size_full())
            .children((self.laser && blank.is_none() && !ended).then(||div().absolute().left(self.pointer.x-px(5.)).top(self.pointer.y-px(5.)).size(px(10.)).rounded_full().bg(rgb(0xff3030))))
            .child(div().absolute().bottom_0().left_0().right_0().min_h(px(36.)).px_3().bg(rgb(0x222222)).text_xs().child(format!("第 {page} 页 · 步骤 {step} {} Space 下一步 · P 暂停 · B/W 黑白 · L 激光 · I 墨迹 · E 清除 · M 媒体 · V 静音 · F11 全屏 · Esc 退出  {}",if paused{"已暂停"}else{""},self.message)))
    }
}

struct Presenter {
    shared: Shared,
    audience: AnyWindowHandle,
    focus: FocusHandle,
    poll: Option<Task<()>>,
}
impl Focusable for Presenter {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for Presenter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (status, notes, next, media) = self
            .shared
            .lock()
            .map(|r| {
                (
                    format!(
                        "第 {} 页 · 步骤 {} · 已用 {:.1} 秒 · {}",
                        r.player.state.slide + 1,
                        r.player.state.step,
                        r.player.total,
                        if r.player.paused {
                            "已暂停"
                        } else {
                            "放映中"
                        }
                    ),
                    r.player.model.slides[r.player.state.slide].notes_text(),
                    r.player
                        .state
                        .peek_next(&r.player.model)
                        .map(|p| {
                            let title = r.player.model.slides[p].title();
                            if title.trim().is_empty() {
                                format!("第 {} 页", p + 1)
                            } else {
                                title
                            }
                        })
                        .unwrap_or_else(|| "放映结束".into()),
                    if r.status.is_null() || r.status["playing"] == false {
                        "未播放".into()
                    } else {
                        format!(
                            "{} · {} · {:.1} 秒",
                            if r.status["paused"] == true {
                                "已暂停"
                            } else {
                                "播放中"
                            },
                            if r.media.muted { "静音" } else { "有声" },
                            r.status["seconds"].as_f64().unwrap_or(0.)
                        )
                    },
                )
            })
            .unwrap_or_default();
        let mut buttons = div().flex().flex_wrap().gap_2();
        for (label, action) in [
            ("下一步", "next"),
            ("上一步", "previous"),
            ("暂停 / 继续", "pause"),
            ("黑屏", "black"),
            ("白屏", "white"),
        ] {
            buttons = buttons.child(Button::new(label).label(label).on_click(cx.listener(
                move |s, _, _, cx| {
                    if let Ok(mut r) = s.shared.lock() {
                        let old = r.player.state.slide;
                        let _ = r.player.navigate(action, None);
                        r.changed_page(old);
                    }
                    cx.notify();
                },
            )));
        }
        let shared = self.shared.clone();
        for (label, action) in [
            ("媒体开关", 0),
            ("静音开关", 1),
            ("后退 5 秒", 2),
            ("前进 5 秒", 3),
        ] {
            buttons = buttons.child(Button::new(label).label(label).on_click(cx.listener(
                move |s, _, _, cx| {
                    if let Ok(mut r) = s.shared.lock() {
                        r.player.serial += 1;
                        match action {
                            0 => r.media.enabled = Some(!r.media_enabled()),
                            1 => r.media.muted = !r.media.muted,
                            _ => {
                                r.media.seek = Some((
                                    r.player.serial,
                                    (r.status["seconds"].as_f64().unwrap_or(0.)
                                        + if action == 2 { -5. } else { 5. })
                                    .clamp(0., 3600.),
                                ))
                            }
                        }
                    }
                    cx.notify();
                },
            )));
        }
        buttons = buttons.child(
            Button::new("exit-show")
                .label("结束放映")
                .on_click(cx.listener(move |s, _, w, cx| {
                    if let Ok(mut r) = shared.lock() {
                        r.closed = true;
                    }
                    let _ = s.audience.update(cx, |_, w, _| w.remove_window());
                    w.remove_window();
                })),
        );
        div()
            .id("presenter")
            .size_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0x202a30))
            .text_color(rgb(0xffffff))
            .track_focus(&self.focus)
            .child(div().text_lg().child("JoyDeck · 演示者视图"))
            .child(buttons)
            .child(div().text_sm().child(status))
            .child(format!("下一页：{next}"))
            .child(
                div()
                    .id("speaker-notes")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_4()
                    .bg(rgb(0x303d45))
                    .child("演讲者备注（仅此窗口）")
                    .child(div().mt_3().child(notes)),
            )
            .child(div().text_xs().child(format!("媒体状态：{media}")))
    }
}
pub(super) fn open(
    model: Arc<joydeck_model::Presentation>,
    page: Option<usize>,
    fullscreen: bool,
    presenter: bool,
    cx: &mut App,
) -> std::result::Result<(), String> {
    let player = Player::new(model, page, None)?;
    eprintln!("R7 show_start {}", player.status());
    let shared = Arc::new(Mutex::new(Runtime {
        player,
        closed: false,
        presenter: None,
        media: Control::default(),
        status: serde_json::Value::Null,
    }));
    let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
    let runtime = shared.clone();
    let (handle, view) = gpui_kit::open_window(
        WindowOptions {
            app_id: Some("joydeck".into()),
            window_bounds: Some(if fullscreen {
                WindowBounds::Fullscreen(bounds)
            } else {
                WindowBounds::Windowed(bounds)
            }),
            ..Default::default()
        },
        cx,
        move |w, cx| {
            w.set_window_title("JoyDeck · 放映");
            cx.new(|cx| {
                let focus = cx.focus_handle();
                focus.focus(w, cx);
                Audience {
                    shared: runtime,
                    focus,
                    image: None,
                    retired: vec![],
                    key: None,
                    font_revision: 0,
                    busy: false,
                    task: None,
                    clock: None,
                    media: Arc::new(Mutex::new(Media::default())),
                    cancel: Cancellation::default(),
                    message: String::new(),
                    origin: Point::default(),
                    scale: 1.,
                    laser: false,
                    pen: false,
                    drawing: false,
                    pointer: Point::default(),
                    ink: vec![],
                    frames: 0,
                }
            })
        },
    )
    .map_err(|e| e.to_string())?;
    let weak = view.downgrade();
    let _ = handle.update(cx, |_, w, cx| {
        w.on_window_should_close(cx, move |w, cx| {
            let closed = shared_closed(&weak, cx);
            if closed {
                return true;
            }
            let _ = weak.update(cx, |s, cx| s.exit(w, cx));
            false
        })
    });
    view.update(cx, |s, cx| {
        s.clock = Some(cx.spawn(async move |view, cx| {
            let mut last = Instant::now();
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let now = Instant::now();
                let dt = now.duration_since(last).as_secs_f64();
                last = now;
                let live = view.update(cx, |s, cx| {
                    let Ok(mut r) = s.shared.lock() else {
                        return false;
                    };
                    if r.closed {
                        return false;
                    }
                    if dt > 5. {
                        r.player.paused = true;
                        s.message = "长时间停顿，放映已暂停；按 P 继续".into();
                    } else {
                        let old = r.player.state.slide;
                        let _ = r.player.advance(dt);
                        r.changed_page(old);
                    }
                    drop(r);
                    cx.notify();
                    true
                });
                if !matches!(live, Ok(true)) {
                    break;
                }
            }
        }));
    });
    if presenter {
        let runtime = shared.clone();
        let (h, v) = gpui_kit::open_window(
            WindowOptions {
                app_id: Some("joydeck".into()),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(900.), px(650.)),
                    cx,
                ))),
                ..Default::default()
            },
            cx,
            move |w, cx| {
                w.set_window_title("JoyDeck · 演示者视图");
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    focus.focus(w, cx);
                    Presenter {
                        shared: runtime,
                        audience: handle,
                        focus,
                        poll: None,
                    }
                })
            },
        )
        .map_err(|e| e.to_string())?;
        if let Ok(mut r) = shared.lock() {
            r.presenter = Some(h);
        }
        let runtime = shared.clone();
        let _ = h.update(cx, |_, w, cx| {
            w.on_window_should_close(cx, move |_, cx| {
                if let Ok(mut r) = runtime.lock()
                    && !r.closed
                {
                    r.closed = true;
                    drop(r);
                    let _ = handle.update(cx, |_, w, _| w.remove_window());
                }
                true
            })
        });
        v.update(cx, |s, cx| {
            s.poll = Some(cx.spawn(async move |view, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    if view.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
            }));
        });
    }
    Ok(())
}
fn shared_closed(view: &WeakEntity<Audience>, cx: &mut App) -> bool {
    view.update(cx, |s, _| s.shared.lock().map_or(true, |r| r.closed))
        .unwrap_or(true)
}
