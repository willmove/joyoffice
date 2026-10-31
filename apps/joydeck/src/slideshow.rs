//! Audience and presenter share one monotonic clock and immutable document snapshot.
use gpui_kit::base::Disableable;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::{
    component::{Sizable, button::Button},
    *,
};
use joydeck::{
    document::Deck,
    motion::{self, Blank, Controller},
    show::ShowSnapshot,
};
use std::{
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::{Duration, Instant},
};
struct Runtime {
    snapshot: ShowSnapshot,
    control: Controller,
    closed: bool,
    serial: u64,
    clock_owner: u64,
    presenter_window: Option<AnyWindowHandle>,
}
type Shared = Arc<Mutex<Runtime>>;
type BaseCache =
    Arc<Mutex<std::collections::BTreeMap<(usize, u32), Arc<joydeck::preview::Raster>>>>;
type FrameKey = (usize, usize, u64, u64, u64);
pub(super) struct Slideshow {
    av_player: Option<joydeck::playback::Player>,
    av_image: Option<Arc<RenderImage>>,
    av_clip: Option<String>,
    av_page: Option<usize>,
    av_sequence: u64,
    av_paused: bool,
    pub(super) snapshot: ShowSnapshot,
    shared: Shared,
    image: Option<Arc<RenderImage>>,
    key: Option<FrameKey>,
    busy: bool,
    task: Option<Task<()>>,
    clock: Option<Task<()>>,
    focus: FocusHandle,
    message: String,
    preview: bool,
    pen: bool,
    laser: bool,
    drawing: bool,
    origin: Point<Pixels>,
    scale: f32,
    frames: u64,
    render_ms: u64,
    render_times: Vec<u64>,
    base_cache: BaseCache,
    display_id: Option<DisplayId>,
    display_lost: bool,
}
fn image(r: joydeck::preview::Raster) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::from_raw(r.width, r.height, r.bgra).unwrap(),
    )]))
}
impl Slideshow {
    fn sync_av(&mut self) {
        let page = self.snapshot.page();
        let rt = self.shared.lock().unwrap();
        let paused = rt.control.paused || rt.control.finished;
        drop(rt);
        if self.av_page != Some(page) {
            let current = self.av_clip.as_ref().and_then(|id| {
                self.snapshot
                    .deck
                    .playback
                    .clips
                    .iter()
                    .find(|c| &c.id == id)
            });
            let continuing = current.is_some_and(|c| {
                c.cross_slide && self.snapshot.deck.playback.assets[&c.asset].info.width == 0
            });
            if !continuing {
                self.av_player = None;
                self.av_image = None;
                self.av_clip = None;
            }
            self.av_page = Some(page);
            if !continuing
                && let Some(c) = self
                    .snapshot
                    .deck
                    .playback
                    .clips
                    .iter()
                    .find(|c| c.slide == self.snapshot.deck.slides[page].id && c.autoplay)
            {
                let a = &self.snapshot.deck.playback.assets[&c.asset];
                match joydeck::playback::Player::new(a, c) {
                    Ok(p) => {
                        self.av_player = Some(p);
                        self.av_clip = Some(c.id.clone());
                        self.av_sequence = 0;
                        self.av_paused = false;
                    }
                    Err(e) => self.message = e,
                }
            }
        }
        if let Some(player) = &mut self.av_player {
            if paused != self.av_paused {
                let _ =
                    player.control(serde_json::json!({"command":if paused{"pause"}else{"play"}}));
                self.av_paused = paused;
            }
            let status = player.status.lock().unwrap().clone();
            if let Some(e) = status.error {
                self.message = e;
            }
            if status.frame.is_none() {
                self.av_image = None;
            }
            if status.sequence != self.av_sequence {
                self.av_sequence = status.sequence;
                if let Some(frame) = status.frame {
                    self.av_image = Some(Arc::new(RenderImage::new(vec![image::Frame::new(
                        image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra.clone())
                            .unwrap(),
                    )])));
                }
            }
        }
    }
    pub(super) fn ready(&self) -> bool {
        self.image.is_some()
    }
    fn request(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let rt = self.shared.lock().unwrap();
        let page = rt.snapshot.page();
        let t = rt
            .snapshot
            .deck
            .presentation
            .timing(&rt.snapshot.deck.slides[page].id);
        let end = motion::schedule(&t)
            .iter()
            .filter(|s| s.step == rt.control.step)
            .map(|s| s.end)
            .max()
            .unwrap_or(0);
        let elapsed = rt.control.elapsed.min(end);
        let transition = rt.control.transition_elapsed.min(t.duration_ms);
        let key = (page, rt.control.step, elapsed, transition, rt.serial);
        if self.key == Some(key) {
            return;
        }
        let animated = elapsed < end
            || (rt.control.previous_page.is_some()
                && transition < t.duration_ms
                && t.transition != "none");
        let sampled = rt.control.frame(&rt.snapshot);
        let deck = rt.snapshot.deck.clone();
        let from = rt.control.previous_page;
        let ink = rt.control.ink.clone();
        let cache = self.base_cache.clone();
        drop(rt);
        self.busy = true;
        self.key = Some(key);
        self.task = Some(cx.spawn(async move |view, cx| {
            let now = Instant::now();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut frame = sampled?;
                    motion::append_ink(&mut frame, &ink)?;
                    let width = if animated { 1024 } else { 2048 };
                    let mut r =
                        joydeck::preview::render(&frame, page, width, &AtomicBool::new(false))?;
                    if let Some(from) = from
                        && transition < t.duration_ms
                        && ["fade", "push", "wipe"].contains(&t.transition.as_str())
                    {
                        let cached = cache.lock().unwrap().get(&(from, width)).cloned();
                        let a = if let Some(a) = cached {
                            a
                        } else {
                            let a = Arc::new(joydeck::preview::render(
                                &deck,
                                from,
                                width,
                                &AtomicBool::new(false),
                            )?);
                            let mut cache = cache.lock().unwrap();
                            if cache.len() >= 2 {
                                cache.clear();
                            }
                            if a.bgra.len() <= 32 * 1024 * 1024 {
                                cache.insert((from, width), a.clone());
                            }
                            a
                        };
                        motion::composite(
                            &a,
                            &mut r,
                            &t.transition,
                            transition as f32 / t.duration_ms.max(1) as f32,
                        )?;
                    }
                    Ok::<_, String>(r)
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.busy = false;
                match result {
                    Ok(r) => {
                        s.image = Some(image(r));
                        s.frames += 1;
                        let ms = now.elapsed().as_micros() as u64;
                        s.render_times.push(ms);
                        s.render_ms += ms / 1000;
                        s.message.clear();
                    }
                    Err(e) => s.message = e,
                }
                cx.notify();
            });
        }));
    }
    pub(super) fn navigation(&mut self, key: &str, w: &mut Window, cx: &mut Context<Self>) {
        if key == "escape"
            || (self.shared.lock().unwrap().control.finished
                && ["right", "space", "enter"].contains(&key))
        {
            self.exit(w, cx);
            return;
        }
        if key == "tab" {
            let handle = self.shared.lock().unwrap().presenter_window;
            if let Some(handle) = handle {
                let _ = handle.update(cx, |_, w, _| w.activate_window());
            }
            return;
        }
        let mut r = self.shared.lock().unwrap();
        let Runtime {
            snapshot, control, ..
        } = &mut *r;
        match key {
            "right" | "down" | "pagedown" | "space" | "enter" => control.next(snapshot),
            "left" | "up" | "pageup" | "backspace" => control.previous(snapshot),
            "home" => control.jump(snapshot, 0),
            "end" => control.jump(snapshot, snapshot.pages.len() - 1),
            "b" => {
                control.blank = if control.blank == Some(Blank::Black) {
                    None
                } else {
                    Some(Blank::Black)
                }
            }
            "w" => {
                control.blank = if control.blank == Some(Blank::White) {
                    None
                } else {
                    Some(Blank::White)
                }
            }
            "p" => control.paused = !control.paused,
            "r" => control.restart_current(),
            "i" => {
                self.pen = !self.pen;
                self.laser = false;
            }
            "l" => {
                self.laser = !self.laser;
                self.pen = false;
            }
            "e" => {
                control.ink.retain(|i| i.page != snapshot.page());
                r.serial += 1;
            }
            n if n.parse::<usize>().is_ok() => {
                let n = n.parse::<usize>().unwrap();
                if n > 0 {
                    control.jump(snapshot, n - 1);
                }
            }
            _ => return,
        }
        self.snapshot = r.snapshot.clone();
        drop(r);
        cx.notify();
    }
    fn pointer_click(&mut self, e: &MouseDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let (x, y) = (
            f32::from(e.position.x - self.origin.x) / self.scale,
            f32::from(e.position.y - self.origin.y) / self.scale,
        );
        let mut rt = self.shared.lock().unwrap();
        let page = rt.snapshot.page();
        let hit = rt.control.frame(&rt.snapshot).ok().and_then(|d| {
            d.slides[page]
                .objects
                .iter()
                .rev()
                .find(|o| {
                    o.opacity > 0.
                        && joydeck::geometry::hit(o, x, y)
                        && rt.snapshot.deck.presentation.links.contains_key(&o.id)
                })
                .map(|o| o.id.clone())
        });
        if let Some(id) = hit {
            let Runtime {
                snapshot, control, ..
            } = &mut *rt;
            if let Err(e) = control.follow_link(snapshot, &id) {
                self.message = e;
            }
            self.snapshot = rt.snapshot.clone();
            drop(rt);
            cx.notify();
        } else {
            drop(rt);
            self.navigation("right", w, cx);
        }
    }
    fn exit(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let rt = self.shared.lock().unwrap();
        let has_ink = !rt.control.ink.is_empty();
        drop(rt);
        if !has_ink {
            self.shared.lock().unwrap().closed = true;
            w.remove_window();
            return;
        }
        let choice = w.prompt(
            PromptLevel::Info,
            "是否保留墨迹？",
            Some("保留会另存一份演示文稿副本，不改变编辑中的文稿。"),
            &["保留到副本…", "放弃墨迹", "继续放映"],
            cx,
        );
        let handle = w.window_handle();
        let shared = self.shared.clone();
        cx.spawn(async move |view, cx| match choice.await {
            Ok(1) => {
                shared.lock().unwrap().closed = true;
                let _ = handle.update(cx, |_, w, _| w.remove_window());
            }
            Ok(0) => {
                let path = cx.update(|cx| {
                    cx.prompt_for_new_path(std::path::Path::new("."), Some("放映墨迹.joydeck"))
                });
                if let Ok(Ok(Some(path))) = path.await {
                    let result = {
                        let rt = shared.lock().unwrap();
                        let mut d = (*rt.snapshot.deck).clone();
                        motion::append_ink(&mut d, &rt.control.ink)
                            .and_then(|()| joydeck::edit::save_new(&path, &d))
                    };
                    match result {
                        Ok(()) => {
                            shared.lock().unwrap().closed = true;
                            let _ = handle.update(cx, |_, w, _| w.remove_window());
                        }
                        Err(e) => {
                            let _ = view.update(cx, |s, cx| {
                                s.message = e;
                                cx.notify();
                            });
                        }
                    }
                }
            }
            _ => {}
        })
        .detach();
    }
}
impl Render for Slideshow {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_av();
        self.request(cx);
        let rt = self.shared.lock().unwrap();
        let blank = if rt.control.finished {
            Some(Blank::Black)
        } else {
            rt.control.blank.clone()
        };
        let laser = rt.control.laser;
        let paused = rt.control.paused;
        let finished = rt.control.finished;
        drop(rt);
        let v = w.viewport_size();
        let scale = (f32::from(v.width) / self.snapshot.deck.width).min(
            (f32::from(v.height) - if self.preview { 36. } else { 0. }) / self.snapshot.deck.height,
        );
        self.scale = scale;
        self.origin = point(
            px((f32::from(v.width) - self.snapshot.deck.width * scale) / 2.),
            px((f32::from(v.height) - self.snapshot.deck.height * scale) / 2.),
        );
        div().id("audience").size_full().bg(rgb(if blank==Some(Blank::White){0xffffff}else{0})).text_color(rgb(0xffffff)).relative().flex().items_center().justify_center().track_focus(&self.focus)
            .on_key_down(cx.listener(|s,e:&KeyDownEvent,w,cx|{s.navigation(&e.keystroke.key,w,cx);cx.stop_propagation();}))
            .on_mouse_down(MouseButton::Left,cx.listener(|s,e:&MouseDownEvent,w,cx|{if s.pen {let p=((f32::from(e.position.x-s.origin.x)/s.scale).clamp(0.,s.snapshot.deck.width),(f32::from(e.position.y-s.origin.y)/s.scale).clamp(0.,s.snapshot.deck.height));let mut rt=s.shared.lock().unwrap();if rt.control.ink.len()<256 {let page=rt.snapshot.page();rt.control.ink.push(motion::Ink{page,points:vec![p]});s.drawing=true;}}else if !s.laser{s.pointer_click(e,w,cx);}}))
            .on_mouse_move(cx.listener(|s,e:&MouseMoveEvent,_,cx|{let p=(f32::from(e.position.x-s.origin.x)/s.scale,f32::from(e.position.y-s.origin.y)/s.scale);let mut rt=s.shared.lock().unwrap();if s.laser {rt.control.laser=Some(p);}
if s.drawing {let total:usize=rt.control.ink.iter().map(|i|i.points.len()).sum();if total<2048 {if let Some(i)=rt.control.ink.last_mut(){i.points.push(p);}rt.serial+=1;}}cx.notify();}))
            .on_mouse_up(MouseButton::Left,cx.listener(|s,_,_,cx|{s.drawing=false;cx.notify();}))
            .on_mouse_down(MouseButton::Right,cx.listener(|s,_,w,cx|s.navigation("left",w,cx)))
            .children(if blank.is_none(){self.image.as_ref().map(|i|img(i.clone()).w(px(self.snapshot.deck.width*scale)).h(px(self.snapshot.deck.height*scale)))}else{None})
            .children(self.av_image.as_ref().filter(|_|blank.is_none()).map(|i|img(i.clone()).absolute().left(self.origin.x).top(self.origin.y).w(px(self.snapshot.deck.width*scale)).h(px(self.snapshot.deck.height*scale)).object_fit(ObjectFit::Contain)))
            .children(laser.filter(|_|self.laser && blank.is_none()).map(|(x,y)|div().absolute().left(self.origin.x+px(x*scale-5.)).top(self.origin.y+px(y*scale-5.)).size(px(10.)).rounded_full().bg(rgb(0xff2828))))
            .children((self.preview||paused||finished||!self.message.is_empty()).then(||div().absolute().bottom_0().left_0().right_0().h(px(32.)).px_4().bg(rgb(0x242424)).text_size(px(12.)).child(format!("{} / {}  {}  Space 下一步 · P 暂停 · B/W 黑白屏 · L 激光 · I 墨迹 · E 清除 · Esc 退出  {}",self.snapshot.cursor+1,self.snapshot.pages.len(),if finished{"放映结束"}else if paused{"已暂停"}else{""},self.message))))
    }
}
struct Presenter {
    shared: Shared,
    focus: FocusHandle,
    current: Option<Arc<RenderImage>>,
    next: Option<Arc<RenderImage>>,
    key: Option<usize>,
    task: Option<Task<()>>,
    poll: Option<Task<()>>,
    audience: Entity<Slideshow>,
    audience_window: AnyWindowHandle,
    displays: Vec<DisplayId>,
    message: String,
    seek_input: Entity<InputState>,
}
impl Presenter {
    fn save_rehearsal(&mut self, cx: &mut Context<Self>) {
        let rt = self.shared.lock().unwrap();
        let mut deck = (*rt.snapshot.deck).clone();
        let mut times = rt.control.rehearsed.clone();
        if rt.control.page_elapsed > 0 {
            times.insert(
                deck.slides[rt.snapshot.page()].id.clone(),
                rt.control.page_elapsed.min(3600000),
            );
        }
        drop(rt);
        if times.is_empty() {
            self.message = "尚无排练计时".into();
            cx.notify();
            return;
        }
        for (id, ms) in times {
            deck.presentation.slides.entry(id).or_default().advance_ms = Some(ms);
        }
        let path = cx.prompt_for_new_path(std::path::Path::new("."), Some("排练计时.joydeck"));
        cx.spawn(async move |view, cx| {
            if let Ok(Ok(Some(path))) = path.await {
                let result = cx
                    .background_executor()
                    .spawn(async move { joydeck::edit::save_new(&path, &deck) })
                    .await;
                let _ = view.update(cx, |s, cx| {
                    s.message = result.map_or_else(|e| e, |()| "排练计时副本已保存".into());
                    cx.notify();
                });
            }
        })
        .detach();
    }
}
impl Render for Presenter {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.displays = cx.displays().iter().map(|d| d.id()).collect();
        let r = self.shared.lock().unwrap();
        let page = r.snapshot.page();
        let next = r.snapshot.pages.get(r.snapshot.cursor + 1).copied();
        let deck = r.snapshot.deck.clone();
        let notes = deck.slides[page].notes.clone();
        let time = r.control.total;
        let paused = r.control.paused;
        let cursor = r.snapshot.cursor;
        let count = r.snapshot.pages.len();
        let closed = r.closed;
        let rehearsing = r.control.rehearsing;
        drop(r);
        if self.key != Some(page) {
            self.key = Some(page);
            self.current = None;
            self.next = None;
            self.task = Some(cx.spawn(async move |view, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        let current =
                            joydeck::preview::render(&deck, page, 900, &AtomicBool::new(false))
                                .map(image);
                        let next = next.map(|p| {
                            joydeck::preview::render(&deck, p, 480, &AtomicBool::new(false))
                                .map(image)
                        });
                        (current, next)
                    })
                    .await;
                let _ = view.update(cx, |s, cx| {
                    if s.key == Some(page) {
                        s.current = result.0.ok();
                        s.next = result.1.and_then(|v| v.ok());
                    }
                    cx.notify();
                });
            }));
        }
        let mut controls = div().flex().flex_wrap().gap_2().child(
            Button::new("activate-audience")
                .small()
                .label("查看观众窗口")
                .on_click(cx.listener(|s, _, _, cx| {
                    let _ = s.audience_window.update(cx, |_, w, _| w.activate_window());
                })),
        );
        for (key, name) in [
            ("left", "上一张"),
            ("right", "下一步"),
            ("p", "暂停 / 继续"),
            ("b", "黑屏"),
            ("w", "白屏"),
            ("r", "重新预览"),
        ] {
            controls = controls.child(Button::new(key).small().label(name).on_click(cx.listener(
                move |s, _, _, cx| {
                    let a = s.audience.clone();
                    let _ = s.audience_window.update(cx, |_, w, cx| {
                        a.update(cx, |a, cx| a.navigation(key, w, cx))
                    });
                },
            )));
        }
        controls = controls
            .child(
                Input::new(&self.seek_input)
                    .small()
                    .w(px(80.))
                    .text_color(rgb(0x242424)),
            )
            .child(
                Button::new("seek-time")
                    .small()
                    .label("定位当前动画步(ms)")
                    .on_click(cx.listener(|s, _, _, cx| {
                        match s.seek_input.read(cx).value().parse::<u64>() {
                            Ok(ms) => s.shared.lock().unwrap().control.seek(ms),
                            Err(_) => s.message = "定位时间须为非负整数毫秒".into(),
                        }
                        cx.notify();
                    })),
            );
        controls = controls.child(
            Button::new("swap-displays")
                .small()
                .label("交换显示器")
                .disabled(self.displays.len() < 2)
                .on_click(cx.listener(|s, _, w, cx| {
                    let audience = s.audience.read(cx).display_id;
                    let presenter = w.display(cx).map(|d| d.id());
                    if let Err(e) =
                        open_shared_presenter_on(s.shared.clone(), audience, presenter, cx)
                    {
                        s.message = e.to_string();
                    } else {
                        let _ = s.audience_window.update(cx, |_, w, _| w.remove_window());
                        w.remove_window();
                    }
                })),
        );
        controls = controls.child(
            Button::new("save-rehearsal")
                .small()
                .label("保存排练计时副本…")
                .disabled(!rehearsing)
                .on_click(cx.listener(|s, _, _, cx| s.save_rehearsal(cx))),
        );
        for (i, id) in self.displays.iter().copied().enumerate() {
            controls = controls.child(
                Button::new(("display", i))
                    .small()
                    .label(format!("观众屏幕 {}", i + 1))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        let shared = s.shared.clone();
                        if let Ok((h, v)) = open_runtime(shared, false, Some(id), cx) {
                            let old = s.audience_window;
                            s.audience = v;
                            s.audience_window = h;
                            let _ = old.update(cx, |_, w, _| w.remove_window());
                        }
                    })),
            );
        }
        let preview_height = (f32::from(w.viewport_size().height) - 340.).clamp(200., 460.);
        let aspect = {
            let r = self.shared.lock().unwrap();
            r.snapshot.deck.width / r.snapshot.deck.height
        };
        let current_width = (f32::from(w.viewport_size().width) - 380.)
            .max(100.)
            .min(preview_height * aspect);
        div()
            .size_full()
            .bg(rgb(0x242424))
            .text_color(rgb(0xffffff))
            .p_5()
            .flex()
            .flex_col()
            .gap_4()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                if s.focus.is_focused(w) {
                    let a = s.audience.clone();
                    let key = e.keystroke.key.clone();
                    let _ = s.audience_window.update(cx, |_, w, cx| {
                        a.update(cx, |a, cx| a.navigation(&key, w, cx))
                    });
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(format!("JoyDeck 演示者视图  {} / {}", cursor + 1, count))
                    .child(format!(
                        "{}:{:02} {}",
                        time / 60000,
                        time / 1000 % 60,
                        if paused { "已暂停" } else { "" }
                    )),
            )
            .child(div().text_size(px(12.)).child(self.message.clone()))
            .child(if closed {
                div().child("放映已结束。关闭此窗口返回编辑。")
            } else {
                controls
            })
            .child(
                div()
                    .flex()
                    .gap_5()
                    .w_full()
                    .h(px(preview_height + 26.))
                    .flex_shrink_0()
                    .child(
                        div().flex_1().min_w_0().child("当前幻灯片").children(
                            self.audience
                                .read(cx)
                                .image
                                .clone()
                                .or_else(|| self.current.clone())
                                .map(|i| img(i).w(px(current_width)).h(px(current_width / aspect))),
                        ),
                    )
                    .child(
                        div().w(px(320.)).flex_shrink_0().child("下一张").children(
                            self.next
                                .as_ref()
                                .map(|i| img(i.clone()).w_full().h(px(180.))),
                        ),
                    ),
            )
            .child(
                div()
                    .id("presenter-notes")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .bg(rgb(0x333333))
                    .child("演讲者备注（仅此窗口可见）")
                    .child(div().mt_3().text_size(px(22.)).child(notes)),
            )
    }
}
fn open_runtime(
    shared: Shared,
    preview: bool,
    display: Option<DisplayId>,
    cx: &mut App,
) -> Result<(AnyWindowHandle, Entity<Slideshow>)> {
    let (snapshot, owner) = {
        let mut r = shared.lock().unwrap();
        r.clock_owner += 1;
        (r.snapshot.clone(), r.clock_owner)
    };
    let bounds = Bounds::centered(display, size(px(1200.), px(760.)), cx);
    let (h, v) = gpui_kit::open_window(
        WindowOptions {
            display_id: display,
            window_bounds: Some(if preview {
                WindowBounds::Windowed(bounds)
            } else {
                WindowBounds::Fullscreen(bounds)
            }),
            ..Default::default()
        },
        cx,
        |w, cx| {
            w.set_window_title(if preview {
                "JoyDeck — Animation Preview"
            } else {
                "JoyDeck — Static Slideshow"
            });
            cx.new(|cx| {
                let focus = cx.focus_handle();
                focus.focus(w, cx);
                Slideshow {
                    av_player: None,
                    av_image: None,
                    av_clip: None,
                    av_page: None,
                    av_sequence: 0,
                    av_paused: false,
                    snapshot,
                    shared,
                    image: None,
                    key: None,
                    busy: false,
                    task: None,
                    clock: None,
                    focus,
                    message: "正在准备放映…".into(),
                    preview,
                    pen: false,
                    laser: false,
                    drawing: false,
                    origin: point(px(0.), px(0.)),
                    scale: 1.,
                    frames: 0,
                    render_ms: 0,
                    render_times: vec![],
                    base_cache: Arc::new(Mutex::new(Default::default())),
                    display_id: display.or_else(|| w.display(cx).map(|d| d.id())),
                    display_lost: false,
                }
            })
        },
    )?;
    let weak = v.downgrade();
    let _ = h.update(cx, |_, w, cx| {
        w.on_window_should_close(cx, move |w, cx| {
            let _ = weak.update(cx, |s, cx| s.exit(w, cx));
            false
        })
    });
    v.update(cx, |s, cx| {
        s.clock = Some(cx.spawn(async move |view, cx| {
            let mut last = Instant::now();
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let now = Instant::now();
                let dt = now.duration_since(last).as_millis() as u64;
                last = now;
                let result = view.update(cx, |s, cx| {
                    let mut rt = s.shared.lock().unwrap();
                    if s.display_id
                        .is_some_and(|id| !cx.displays().iter().any(|d| d.id() == id))
                        && !s.display_lost
                    {
                        rt.control.paused = true;
                        s.display_lost = true;
                        s.message = "观众屏幕已断开，已暂停；可在演示者视图重新选择屏幕".into();
                    }
                    if rt.closed || rt.clock_owner != owner {
                        return false;
                    }
                    if dt > 5000 {
                        rt.control.paused = true;
                        s.message = "检测到睡眠/长时间停顿，已暂停；按P继续".into();
                    } else {
                        let Runtime {
                            snapshot, control, ..
                        } = &mut *rt;
                        control.advance(snapshot, dt);
                    }
                    s.snapshot = rt.snapshot.clone();
                    drop(rt);
                    cx.notify();
                    true
                });
                if !matches!(result, Ok(true)) {
                    break;
                }
            }
        }));
    });
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(100))
            .await;
        let _ = h.update(cx, |_, w, _| w.activate_window());
    })
    .detach();
    Ok((h, v))
}
fn runtime(deck: Arc<Deck>, page: usize) -> Result<Shared> {
    let snapshot =
        ShowSnapshot::new(deck, page).map_err(|e| gpui_kit::private::anyhow::anyhow!(e))?;
    Ok(Arc::new(Mutex::new(Runtime {
        snapshot,
        control: Controller::default(),
        closed: false,
        serial: 0,
        clock_owner: 0,
        presenter_window: None,
    })))
}
pub(super) fn open(
    deck: Arc<Deck>,
    page: usize,
    cx: &mut App,
) -> Result<(AnyWindowHandle, Entity<Slideshow>)> {
    open_runtime(runtime(deck, page)?, false, None, cx)
}
pub(super) fn open_preview(
    deck: Arc<Deck>,
    page: usize,
    cx: &mut App,
) -> Result<(AnyWindowHandle, Entity<Slideshow>)> {
    open_runtime(runtime(deck, page)?, true, None, cx)
}
pub(super) fn open_presenter(deck: Arc<Deck>, page: usize, cx: &mut App) -> Result<()> {
    let shared = runtime(deck, page)?;
    open_shared_presenter(shared, cx)
}
pub(super) fn open_rehearsal(deck: Arc<Deck>, page: usize, cx: &mut App) -> Result<()> {
    let shared = runtime(deck, page)?;
    shared.lock().unwrap().control.rehearsing = true;
    open_shared_presenter(shared, cx)
}
fn open_shared_presenter(shared: Shared, cx: &mut App) -> Result<()> {
    let displays = cx.displays();
    open_shared_presenter_on(
        shared,
        displays.first().map(|d| d.id()),
        displays.get(1).map(|d| d.id()),
        cx,
    )
}
fn open_shared_presenter_on(
    shared: Shared,
    presenter_display: Option<DisplayId>,
    audience_display: Option<DisplayId>,
    cx: &mut App,
) -> Result<()> {
    let displays: Vec<_> = cx.displays().iter().map(|d| d.id()).collect();
    let (h, audience) = open_runtime(shared.clone(), displays.len() < 2, audience_display, cx)?;
    let (presenter_handle, v) = gpui_kit::open_window(
        WindowOptions {
            display_id: presenter_display,
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                presenter_display,
                size(px(1300.), px(850.)),
                cx,
            ))),
            ..Default::default()
        },
        cx,
        |w, cx| {
            w.set_window_title("JoyDeck — Presenter View");
            let entity = cx.new(|cx| {
                let focus = cx.focus_handle();
                focus.focus(w, cx);
                Presenter {
                    shared,
                    focus,
                    current: None,
                    next: None,
                    key: None,
                    task: None,
                    poll: None,
                    audience,
                    audience_window: h,
                    displays,
                    message: String::new(),
                    seek_input: cx.new(|cx| {
                        let mut s = InputState::new(w, cx);
                        s.set_value("0", w, cx);
                        s
                    }),
                }
            });
            let weak = entity.downgrade();
            w.on_window_should_close(cx, move |_, cx| {
                let _ = weak.update(cx, |s, cx| {
                    let a = s.audience.clone();
                    let _ = s
                        .audience_window
                        .update(cx, |_, w, cx| a.update(cx, |a, cx| a.exit(w, cx)));
                });
                true
            });
            entity
        },
    )?;
    let _ = h.update(cx, |_, w, _| w.set_window_title("JoyDeck — Audience View"));
    v.update(cx, |s, cx| {
        s.shared.lock().unwrap().presenter_window = Some(presenter_handle);
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
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(200))
            .await;
        let _ = presenter_handle.update(cx, |_, w, _| w.activate_window());
    })
    .detach();
    Ok(())
}
/// Real monotonic timer and rendering acceptance, independently of virtual model tests.
pub(super) fn smoke(
    deck: Arc<Deck>,
    output: std::path::PathBuf,
    parent: AnyWindowHandle,
    parent_focus: FocusHandle,
    cx: &mut App,
) {
    let baseline = deck.clone();
    let shared = runtime(deck, 0).unwrap();
    let (h, v) = open_runtime(shared.clone(), true, None, cx).unwrap();
    cx.spawn(async move|cx|{
        let result:std::result::Result<(),String>=async{
            cx.background_executor().timer(Duration::from_millis(350)).await;
            h.update(cx,|_,w,cx|v.update(cx,|s,cx|s.navigation("right",w,cx))).map_err(|e|e.to_string())?;
            cx.background_executor().timer(Duration::from_millis(900)).await;
            let before=shared.lock().unwrap().control.total;if before<500{return Err("native clock did not advance".into());}
            h.update(cx,|_,w,cx|v.update(cx,|s,cx|{s.navigation("p",w,cx);s.navigation("b",w,cx);})).map_err(|e|e.to_string())?;
            let frozen=shared.lock().unwrap().control.total;cx.background_executor().timer(Duration::from_millis(250)).await;if shared.lock().unwrap().control.total!=frozen{return Err("paused clock advanced".into());}
            h.update(cx,|_,w,cx|v.update(cx,|s,cx|{s.navigation("b",w,cx);s.navigation("w",w,cx);})).map_err(|e|e.to_string())?;if shared.lock().unwrap().control.blank!=Some(Blank::White){return Err("white screen failed".into());}
            h.update(cx,|_,w,cx|v.update(cx,|s,cx|{s.navigation("w",w,cx);s.navigation("p",w,cx);s.navigation("right",w,cx);})).map_err(|e|e.to_string())?;
            cx.background_executor().timer(Duration::from_millis(700)).await;
            if shared.lock().unwrap().snapshot.page()!=1{return Err("effect/page navigation failed".into());}
            let report=h.update(cx,|_,_,cx|{let s=v.read(cx);let mut times=s.render_times.clone();times.sort();serde_json::json!({"renderedFrames":s.frames,"renderP95Ms":times.get(times.len()*95/100).copied().unwrap_or(0)as f64/1000.,"totalRenderMs":s.render_ms,"monotonicPause":true,"blackWhite":true,"snapshotUnchanged":*s.snapshot.deck==*baseline,"displays":cx.displays().iter().map(|d|u64::from(d.id())).collect::<Vec<_>>(),"scope":"native preview timer/render + one physical display; not physical dual-screen/hotplug or end-to-end 60fps certification"})}).map_err(|e|e.to_string())?;
            let before=shared.lock().unwrap().control.total;
            let (second,second_view)=cx.update(|cx|open_runtime(shared.clone(),true,None,cx)).map_err(|e|e.to_string())?;
            cx.background_executor().timer(Duration::from_millis(250)).await;
            let delta=shared.lock().unwrap().control.total.saturating_sub(before);
            if !(100..=400).contains(&delta){return Err(format!("clock handoff delta={delta}"));}
            let _=h.update(cx,|_,w,_|w.remove_window());
            second.update(cx,|_,w,cx|second_view.update(cx,|s,cx|s.navigation("escape",w,cx))).map_err(|e|e.to_string())?;
            parent.update(cx,|_,w,_|w.activate_window()).map_err(|e|e.to_string())?;
            cx.background_executor().timer(Duration::from_millis(250)).await;
            if !parent.update(cx,|_,w,_|parent_focus.is_focused(w)).map_err(|e|e.to_string())?{return Err("editor focus was not restored after show".into());}
            let mut report=report;report["editorFocusRestored"]=serde_json::json!(true);report["clockHandoff"]=serde_json::json!(true);report["handoffDeltaMs"]=serde_json::json!(delta);
            std::fs::write(output.join("native-show.json"),serde_json::to_vec_pretty(&report).unwrap()).map_err(|e|e.to_string())?;
            if shared.lock().unwrap().snapshot.deck!=baseline{return Err("show mutated document".into());}
            Ok(())
        }.await;
        match result{Ok(())=>eprintln!("M10_SMOKE clock_pause_blank_navigation_snapshot=true complete=true"),Err(e)=>eprintln!("M10_SMOKE failed={e}")};cx.update(|cx|cx.quit());
    }).detach();
}
