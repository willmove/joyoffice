//! Audience/presenter lifecycle, clock and owned media workers adapted from JoyOffice slideshow.
use super::*;
use gpui_kit::component::Sizable;
use joydeck_shell::{
    Cancellation,
    show::Player,
    show_media::{Control, Media},
    show_render::{RasterCache, RenderTarget},
};
use std::{sync::Mutex, time::Duration};

#[path = "slideshow_frames.rs"]
mod frames;

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
    image_target: Option<RenderTarget>,
    image_animated: bool,
    prepare_total: u128,
    prepare_peak: u128,
    prepare_count: u64,
    retired: Vec<Arc<RenderImage>>,
    requests: frames::Requests,
    raster_cache: Arc<Mutex<RasterCache>>,
    task: Option<Task<()>>,
    clock: Option<Task<()>>,
    bounds_observer: Option<Subscription>,
    #[cfg(target_os = "windows")]
    viewport: frames::Viewport,
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
    #[cfg(feature = "validation")]
    trace_transitions: bool,
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
                    "right" | "down" | "space" | "enter" | "pagedown" => "next",
                    "left" | "up" | "backspace" | "pageup" => "previous",
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
    fn request(&mut self, target: RenderTarget, cx: &mut Context<Self>) {
        let Ok(r) = self.shared.lock() else { return };
        if r.closed {
            return;
        }
        let model = r.player.clone();
        let control = r.media.clone();
        let animated = model.animated();
        let mut key = model.frame_key();
        let media_on = r.media_enabled();
        if media_on && !model.paused && r.status["paused"] != true {
            key.3 = key.3.max((model.total * 24.) as u64);
        }
        let font_revision = joydeck_fonts::FontDb::global().environment_revision();
        let key = frames::Key {
            state: key,
            target,
            fonts: font_revision,
        };
        if !self.requests.request(key) {
            return;
        }
        drop(r);
        #[cfg(feature = "validation")]
        let trace = self
            .trace_transitions
            .then(|| {
                let tr = model.model.slides[model.state.slide].transition.as_ref()?;
                Some((
                    model.state.slide + 1,
                    tr.kind.clone(),
                    model.transition_progress()?,
                ))
            })
            .flatten();
        let media = self.media.clone();
        let cache = self.raster_cache.clone();
        let cancel = self.cancel.clone();
        self.task = Some(cx.spawn(async move |view, cx| {
            let started = Instant::now();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut cache = cache.lock().map_err(|_| "show raster cache lock poisoned")?;
                    let mut raster = cache.render(&model, target, media_on, &cancel)?;
                    let (renders, hits) = (cache.renders, cache.hits);
                    drop(cache);
                    let mut media = media
                        .lock()
                        .map_err(|_| "media state lock poisoned".to_string())?;
                    let status = media.composite(&model, &control, &mut raster);
                    if status.is_err() {
                        media.clear();
                    }
                    cancel.check()?;
                    Ok::<_, String>((image(raster)?, status, renders, hits))
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                if s.cancel.check().is_err() {
                    return;
                }
                let current = s.shared.lock().ok().map(|r| r.player.frame_key());
                let content_current = current.is_some_and(|k|
                    (k.0, k.1, k.2) == (key.state.0, key.state.1, key.state.2))
                    && joydeck_fonts::FontDb::global().environment_revision() == font_revision;
                if !s.requests.complete(key, content_current) {
                    cx.notify();
                    return;
                }
                match result {
                    Ok((image, status, renders, hits)) => {
                        let resized = s.image_target != Some(target);
                        s.image_target = Some(target);
                        let settled = s.image_animated && !animated;
                        s.image_animated = animated;
                        if let Some(old) = s.image.replace(image) {
                            s.retired.push(old);
                        }
                        s.frames += 1;
                        let micros = started.elapsed().as_micros();
                        #[cfg(feature = "validation")]
                        if let Some((slide, kind, progress)) = &trace {
                            eprintln!("JoyDeck show_transition_frame slide={slide} effect={kind} progress={progress:.8} prepare_us={micros} pixels={}x{}", target.width, target.height);
                        }
                        s.prepare_total += micros;
                        s.prepare_peak = s.prepare_peak.max(micros);
                        s.prepare_count += 1;
                        if resized || settled || s.frames.is_multiple_of(30) {
                            eprintln!("JoyDeck show_frame pixels={}x{} scale={:.4} prepare_us={} frames={} rasters={renders} cache_hits={hits}",
                                target.width, target.height, target.scale(), started.elapsed().as_micros(), s.frames);
                            eprintln!("JoyDeck show_prepare samples={} mean_us={} peak_us={} (worker preparation, excludes GPU presentation)",
                                s.prepare_count, s.prepare_total / u128::from(s.prepare_count), s.prepare_peak);
                            s.prepare_total = 0;
                            s.prepare_peak = 0;
                            s.prepare_count = 0;
                        }
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
        #[cfg(target_os = "windows")]
        if let Some((width, height)) = self.viewport.update(
            (f32::from(viewport.width), f32::from(viewport.height)),
            w.scale_factor(),
            w.is_fullscreen(),
        ) {
            eprintln!(
                "JoyDeck show_dpi_refit logical={width}x{height} dpi={}",
                w.scale_factor()
            );
            w.resize(size(px(width), px(height)));
            cx.notify();
        }
        let scale = (f32::from(viewport.width) / width)
            .min((f32::from(viewport.height) - 42.) / height)
            .max(0.01);
        self.origin = point(
            px((f32::from(viewport.width) - width * scale) / 2.),
            px((f32::from(viewport.height) - 42. - height * scale) / 2.),
        );
        self.scale = scale;
        match RenderTarget::fit(
            (f64::from(width), f64::from(height)),
            (
                f64::from(viewport.width),
                (f64::from(viewport.height) - 42.).max(1.),
            ),
            f64::from(w.scale_factor()),
        ) {
            Ok(target) => self.request(target, cx),
            Err(error) => self.message = error,
        }
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

type PresenterCurrentKey = ((u64, usize, usize, u64), RenderTarget, u64);
type PresenterNextKey = (usize, RenderTarget, u64);

struct Presenter {
    shared: Shared,
    audience: AnyWindowHandle,
    focus: FocusHandle,
    poll: Option<Task<()>>,
    cancel: Cancellation,
    current_image: Option<Arc<RenderImage>>,
    current_key: Option<PresenterCurrentKey>,
    current_in_flight: Option<PresenterCurrentKey>,
    current_task: Option<Task<()>>,
    next_image: Option<Arc<RenderImage>>,
    next_key: Option<PresenterNextKey>,
    next_in_flight: Option<PresenterNextKey>,
    next_task: Option<Task<()>>,
    retired: Vec<Arc<RenderImage>>,
}
impl Drop for Presenter {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
impl Presenter {
    fn exit(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.cancel.cancel();
        if let Ok(mut r) = self.shared.lock() {
            r.closed = true;
            r.presenter = None;
        }
        let _ = self.audience.update(cx, |_, w, _| w.remove_window());
        w.remove_window();
    }
    fn navigation(&mut self, key: &str, w: &mut Window, cx: &mut Context<Self>) {
        if key == "escape" {
            self.exit(w, cx);
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
                    "right" | "down" | "space" | "enter" | "pagedown" => "next",
                    "left" | "up" | "backspace" | "pageup" => "previous",
                    "p" => "pause",
                    "b" => "black",
                    "w" => "white",
                    _ => return,
                };
                let old = r.player.state.slide;
                if let Ok(q) = r.player.navigate(action, None) {
                    quit = q;
                }
                r.changed_page(old);
            }
        }
        if quit {
            self.exit(w, cx);
        } else {
            cx.notify();
        }
    }
    fn request_previews(
        &mut self,
        current_target: RenderTarget,
        next_target: RenderTarget,
        cx: &mut Context<Self>,
    ) {
        let Ok(r) = self.shared.lock() else { return };
        if r.closed {
            return;
        }
        let model = r.player.clone();
        let next_page = r.player.state.peek_next(&r.player.model);
        drop(r);

        let fonts = joydeck_fonts::FontDb::global().environment_revision();
        let curr_key = (model.frame_key(), current_target, fonts);
        if self.current_key != Some(curr_key) && self.current_in_flight != Some(curr_key) {
            self.current_in_flight = Some(curr_key);
            let cancel = self.cancel.clone();
            let model_for_curr = model.clone();
            self.current_task = Some(cx.spawn(async move |view, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        let raster = model_for_curr.frame(current_target.scale(), &cancel)?;
                        image(raster)
                    })
                    .await;
                let _ = view.update(cx, |s, cx| {
                    if s.current_in_flight == Some(curr_key) {
                        s.current_in_flight = None;
                    }
                    if s.cancel.check().is_err() {
                        return;
                    }
                    if let Ok(img) = result {
                        if let Some(old) = s.current_image.replace(img) {
                            s.retired.push(old);
                        }
                        s.current_key = Some(curr_key);
                        cx.notify();
                    }
                });
            }));
        }

        if let Some(page) = next_page {
            let n_key = (page, next_target, fonts);
            if self.next_key != Some(n_key) && self.next_in_flight != Some(n_key) {
                self.next_in_flight = Some(n_key);
                let cancel = self.cancel.clone();
                self.next_task = Some(cx.spawn(async move |view, cx| {
                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            let raster = model.preview_slide(page, next_target.scale(), &cancel)?;
                            image(raster)
                        })
                        .await;
                    let _ = view.update(cx, |s, cx| {
                        if s.next_in_flight == Some(n_key) {
                            s.next_in_flight = None;
                        }
                        if s.cancel.check().is_err() {
                            return;
                        }
                        if let Ok(img) = result {
                            if let Some(old) = s.next_image.replace(img) {
                                s.retired.push(old);
                            }
                            s.next_key = Some(n_key);
                            cx.notify();
                        }
                    });
                }));
            }
        } else {
            if let Some(old) = self.next_image.take() {
                self.retired.push(old);
            }
            self.next_key = None;
            self.next_in_flight = None;
        }
    }
}
impl Focusable for Presenter {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for Presenter {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for old in self.retired.drain(..) {
            let _ = w.drop_image(old);
        }
        let (slide_w, slide_h, status, notes, next_label, has_next, media, blank, ended) = self
            .shared
            .lock()
            .map(|r| {
                let next_idx = r.player.state.peek_next(&r.player.model);
                let next_title = next_idx
                    .map(|p| {
                        let title = r.player.model.slides[p].title();
                        if title.trim().is_empty() {
                            format!("第 {} 页", p + 1)
                        } else {
                            format!("第 {} 页 · {title}", p + 1)
                        }
                    })
                    .unwrap_or_else(|| "放映结束".into());
                (
                    r.player.model.slide_size.width.max(1.),
                    r.player.model.slide_size.height.max(1.),
                    format!(
                        "第 {} / {} 页 · 步骤 {} · 已用 {:.1} 秒 · {}",
                        r.player.state.slide + 1,
                        r.player.model.slides.len(),
                        r.player.state.step,
                        r.player.total,
                        if r.player.paused {
                            "已暂停"
                        } else {
                            "放映中"
                        }
                    ),
                    r.player.model.slides[r.player.state.slide].notes_text(),
                    next_title,
                    next_idx.is_some(),
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
                    r.player.blank,
                    r.player.state.ended,
                )
            })
            .unwrap_or((
                960.,
                540.,
                String::new(),
                String::new(),
                "放映结束".into(),
                false,
                "未播放".into(),
                None,
                true,
            ));

        let vp = w.viewport_size();
        let vp_w = f64::from(vp.width).max(640.);
        let vp_h = f64::from(vp.height).max(480.);
        let right_w = 340.0_f64.min((vp_w * 0.36).max(260.));
        let left_w = (vp_w - right_w - 64.).max(260.);
        let left_h = (vp_h - 190.).max(180.);
        let next_box_w = (right_w - 24.).max(200.);
        let next_box_h = (next_box_w * slide_h / slide_w).clamp(110., 210.);
        let dpi = f64::from(w.scale_factor()).max(1.);

        let curr_scale = (left_w / slide_w).min(left_h / slide_h).max(0.05);
        let curr_disp_w = (slide_w * curr_scale) as f32;
        let curr_disp_h = (slide_h * curr_scale) as f32;

        let next_scale = (next_box_w / slide_w).min(next_box_h / slide_h).max(0.05);
        let next_disp_w = (slide_w * next_scale) as f32;
        let next_disp_h = (slide_h * next_scale) as f32;

        if let (Ok(ct), Ok(nt)) = (
            RenderTarget::fit((slide_w, slide_h), (left_w, left_h), dpi),
            RenderTarget::fit((slide_w, slide_h), (next_box_w, next_box_h), dpi),
        ) {
            self.request_previews(ct, nt, cx);
        }

        let mut buttons = div().flex().flex_wrap().items_center().gap_2();
        for (label, action) in [
            ("上一步", "previous"),
            ("下一步", "next"),
            ("暂停 / 继续", "pause"),
            ("黑屏", "black"),
            ("白屏", "white"),
        ] {
            buttons = buttons.child(
                Button::new(label)
                    .small()
                    .label(label)
                    .on_click(cx.listener(move |s, _, _, cx| {
                        if let Ok(mut r) = s.shared.lock() {
                            let old = r.player.state.slide;
                            let _ = r.player.navigate(action, None);
                            r.changed_page(old);
                        }
                        cx.notify();
                    })),
            );
        }
        for (label, action) in [
            ("媒体开关", 0),
            ("静音开关", 1),
            ("后退 5 秒", 2),
            ("前进 5 秒", 3),
        ] {
            buttons = buttons.child(
                Button::new(label)
                    .small()
                    .label(label)
                    .on_click(cx.listener(move |s, _, _, cx| {
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
                    })),
            );
        }
        buttons = buttons.child(
            Button::new("exit-show")
                .small()
                .label("结束放映")
                .on_click(cx.listener(|s, _, w, cx| s.exit(w, cx))),
        );

        let current_badge = if ended {
            Some("放映结束")
        } else if blank == Some(true) {
            Some("白屏中")
        } else if blank == Some(false) {
            Some("黑屏中")
        } else {
            None
        };

        let current_preview = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("当前幻灯片"),
                    )
                    .children(current_badge.map(|badge| {
                        div()
                            .px_2()
                            .py(px(2.))
                            .rounded(px(4.))
                            .bg(rgb(0xd9480f))
                            .text_xs()
                            .child(badge)
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(rgb(0x3a4a54))
                    .bg(rgb(0x10161a))
                    .overflow_hidden()
                    .child(if let Some(img_ref) = &self.current_image {
                        div()
                            .relative()
                            .w(px(curr_disp_w))
                            .h(px(curr_disp_h))
                            .border_1()
                            .border_color(rgb(0x4b5e6b))
                            .child(img(img_ref.clone()).w(px(curr_disp_w)).h(px(curr_disp_h)))
                            .into_any_element()
                    } else {
                        div()
                            .text_sm()
                            .text_color(rgb(0x9fb0ba))
                            .child("正在渲染当前页预览…")
                            .into_any_element()
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0xb8c7d0))
                    .child(format!("媒体状态：{media}")),
            );

        let next_preview_card = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded(px(6.))
            .border_1()
            .border_color(rgb(0x33424c))
            .bg(rgb(0x26323a))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xd4e0e7))
                    .truncate()
                    .child(format!("下一张幻灯片：{next_label}")),
            )
            .child(
                div()
                    .h(px(next_box_h as f32 + 8.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .bg(rgb(0x10161a))
                    .border_1()
                    .border_color(rgb(0x3a4a54))
                    .overflow_hidden()
                    .child(if !has_next {
                        div()
                            .text_sm()
                            .text_color(rgb(0x9fb0ba))
                            .child("已是最后一张 · 放映结束")
                            .into_any_element()
                    } else if let Some(img_ref) = &self.next_image {
                        img(img_ref.clone())
                            .w(px(next_disp_w))
                            .h(px(next_disp_h))
                            .into_any_element()
                    } else {
                        div()
                            .text_xs()
                            .text_color(rgb(0x9fb0ba))
                            .child("正在渲染下一页预览…")
                            .into_any_element()
                    }),
            );

        let notes_display = if notes.trim().is_empty() {
            "（本页无演讲者备注）".to_string()
        } else {
            notes
        };
        let notes_card = div()
            .id("speaker-notes")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_3()
            .rounded(px(6.))
            .border_1()
            .border_color(rgb(0x33424c))
            .bg(rgb(0x26323a))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xb8c7d0))
                    .child("演讲者备注（仅此窗口）"),
            )
            .child(div().mt_2().text_sm().child(notes_display));

        div()
            .id("presenter")
            .size_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0x1b242a))
            .text_color(rgb(0xffffff))
            .track_focus(&self.focus)
            .key_context("Slideshow")
            .on_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                s.navigation(&e.keystroke.key, w, cx);
                cx.stop_propagation();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("JoyDeck · 演示者视图"),
                    )
                    .child(
                        div()
                            .px_2()
                            .py(px(3.))
                            .rounded(px(4.))
                            .bg(rgb(0x2b3942))
                            .text_xs()
                            .child(status),
                    ),
            )
            .child(buttons)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap_4()
                    .child(current_preview)
                    .child(
                        div()
                            .w(px(right_w as f32))
                            .flex_shrink_0()
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(next_preview_card)
                            .child(notes_card),
                    ),
            )
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
            window_bounds: Some(if fullscreen {
                WindowBounds::Fullscreen(bounds)
            } else {
                WindowBounds::Windowed(bounds)
            }),
            ..joyoffice_ui::product_window_options(joyoffice_ui::Brand::Deck)
        },
        cx,
        move |w, cx| {
            w.set_window_title("JoyDeck · 放映");
            cx.new(|cx| {
                let focus = cx.focus_handle();
                focus.focus(w, cx);
                let mut audience = Audience {
                    shared: runtime,
                    focus,
                    image: None,
                    image_target: None,
                    image_animated: false,
                    prepare_total: 0,
                    prepare_peak: 0,
                    prepare_count: 0,
                    retired: vec![],
                    requests: frames::Requests::default(),
                    raster_cache: Arc::new(Mutex::new(RasterCache::for_audience())),
                    task: None,
                    clock: None,
                    bounds_observer: None,
                    #[cfg(target_os = "windows")]
                    viewport: frames::Viewport::default(),
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
                    #[cfg(feature = "validation")]
                    trace_transitions: std::env::var_os("JOYDECK_TRACE_TRANSITIONS").is_some(),
                };
                audience.bounds_observer =
                    Some(cx.observe_window_bounds(w, |_: &mut Audience, _, cx| {
                        cx.notify();
                    }));
                audience
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
            let mut fonts = joydeck_fonts::FontDb::global().environment_revision();
            let mut clock = frames::Clock::default();
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
                    let media_tick =
                        (r.media_enabled() && !r.player.paused && r.status["paused"] != true)
                            .then_some((r.player.total * 24.) as u64);
                    let changed = clock.changed(r.player.frame_key(), media_tick);
                    drop(r);
                    let current_fonts = joydeck_fonts::FontDb::global().environment_revision();
                    if changed || current_fonts != fonts || dt > 5. {
                        cx.notify();
                    }
                    fonts = current_fonts;
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
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1120.), px(680.)),
                    cx,
                ))),
                ..joyoffice_ui::product_window_options(joyoffice_ui::Brand::Deck)
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
                        cancel: Cancellation::default(),
                        current_image: None,
                        current_key: None,
                        current_in_flight: None,
                        current_task: None,
                        next_image: None,
                        next_key: None,
                        next_in_flight: None,
                        next_task: None,
                        retired: vec![],
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
