#[cfg(feature = "validation")]
#[path = "authoring_smoke.rs"]
mod authoring_smoke;
#[path = "authoring_ui.rs"]
mod authoring_ui;
#[cfg(feature = "validation")]
#[path = "beta_smoke.rs"]
mod beta_smoke;
#[path = "canvas_text.rs"]
mod canvas_text;
#[path = "chrome.rs"]
mod chrome;
#[cfg(feature = "validation")]
#[path = "delivery_smoke.rs"]
mod delivery_smoke;
#[path = "delivery_ui.rs"]
mod delivery_ui;
#[cfg(feature = "validation")]
#[path = "extended_smoke.rs"]
mod extended_smoke;
#[path = "extended_ui.rs"]
mod extended_ui;
#[path = "maintenance_ui.rs"]
mod maintenance_ui;
#[cfg(feature = "validation")]
#[path = "media_smoke.rs"]
mod media_smoke;
#[cfg(feature = "validation")]
#[path = "render_bench.rs"]
mod render_bench;
#[path = "ribbon_menus.rs"]
mod ribbon_menus;
#[path = "scene_canvas.rs"]
mod scene_canvas;
#[path = "table_demo.rs"]
mod table_demo;
#[cfg(feature = "validation")]
#[path = "table_smoke.rs"]
mod table_smoke;
#[path = "table_ui.rs"]
mod table_ui;
#[cfg(feature = "validation")]
#[path = "editor_validation.rs"]
mod validation;
#[cfg(feature = "validation")]
#[path = "workspace_smoke.rs"]
mod workspace_smoke;
#[path = "workspace_ui.rs"]
mod workspace_ui;
use gpui_kit::base::Disableable;
use gpui_kit::component::{
    WindowExt,
    button::Button,
    input::{Input, InputState, Textarea, TextareaState},
};
use gpui_kit::*;
use joydeck::{
    document::{Deck, Object},
    edit::{self, Command},
    geometry::{self, Mode, Rect},
    preview,
    storage::{self, Fingerprint, Record, Recovery},
    workspace::SharedSession as Session,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
actions!(
    editor,
    [
        Undo,
        Redo,
        Save,
        Close,
        CancelGesture,
        CellNext,
        CellPrevious,
        OpenFile,
        SaveAs,
        NewDocument,
        NewSlide,
        ShowFirst,
        ShowCurrent
    ]
);
struct Gesture {
    image_before: Option<Arc<RenderImage>>,
    handle: (i8, i8),
    shift: bool,
    center: bool,
    base: Vec<Object>,
    start: (f32, f32),
    end: (f32, f32),
    mode: Option<Mode>,
    scale: f32,
}

impl Gesture {
    fn result(&self) -> std::result::Result<Vec<Object>, String> {
        if (self.end.0 - self.start.0).hypot(self.end.1 - self.start.1) * self.scale < 3. {
            return Ok(self.base.clone());
        }
        let Some(mode) = self.mode else {
            return Ok(vec![]);
        };
        if mode == Mode::Scale {
            let mut objects = geometry::resize(
                &self.base,
                self.handle,
                (self.end.0 - self.start.0, self.end.1 - self.start.1),
                self.shift,
                self.center,
            )?;
            for o in &mut objects {
                joydeck::table::resize(o);
            }
            return Ok(objects);
        }
        let mut end = self.end;
        if self.shift && mode == Mode::Move {
            if (end.0 - self.start.0).abs() > (end.1 - self.start.1).abs() {
                end.1 = self.start.1;
            } else {
                end.0 = self.start.0;
            }
        }
        if self.shift && mode == Mode::Rotate {
            let r = geometry::union(&self.base);
            let (cx, cy) = (r.x + r.w / 2., r.y + r.h / 2.);
            let start = (self.start.1 - cy).atan2(self.start.0 - cx);
            let delta = ((end.1 - cy).atan2(end.0 - cx) - start).to_degrees();
            let angle = start + (delta / 15.).round() * 15f32.to_radians();
            end = (cx + 100. * angle.cos(), cy + 100. * angle.sin());
        }
        geometry::transform(&self.base, mode, self.start, end)
    }
}
fn recovery_root() -> PathBuf {
    if std::env::args().any(|s| {
        s == "--beta-smoke"
            || s == "--extended-smoke"
            || s == "--authoring-smoke"
            || s == "--delivery-smoke"
            || s == "--workspace-smoke"
            || s == "--media-smoke"
            || s == "--editor-smoke"
            || s == "--text-smoke"
            || s == "--ime-smoke"
            || s == "--table-ime-smoke"
            || s == "--table-smoke"
            || s == "--table-demo"
            || s == "--scene-smoke"
    }) {
        static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        ROOT.get_or_init(|| {
            std::env::temp_dir().join(format!(
                "joydeck-native-test-recovery-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ))
        })
        .clone()
    } else {
        edit::recovery_path()
    }
}
struct Editor {
    preferences: joydeck::preferences::Preferences,
    comparison_path: Entity<InputState>,
    comparison: Option<(joydeck::compare::Report, Arc<Deck>)>,
    comparison_pick: usize,
    control_service: Option<joydeck::control::Service>,
    extended_fields: Vec<Entity<InputState>>,
    thread_pick: usize,
    clip_pick: usize,
    review_findings: Vec<(String, Option<String>, String)>,
    av_player: Option<joydeck::playback::Player>,
    av_image: Option<Arc<RenderImage>>,
    av_slide: Option<String>,
    av_document_id: u64,
    av_seconds: f64,
    av_sequence: u64,
    av_task: Option<Task<()>>,
    restore_focus_after_show: bool,
    inspector_open: bool,
    chart_grid: Vec<Vec<Entity<InputState>>>,
    author_data: Entity<TextareaState>,
    author_fields: Vec<Entity<InputState>>,
    motion_fields: Vec<Entity<InputState>>,
    effect_pick: usize,
    focus_initialized: bool,
    ribbon: String,
    zoom: f32,
    last_scale: f32,
    show_notes: bool,
    table_fields: Vec<Entity<InputState>>,
    table_pick: Option<table_ui::Pick>,
    cell_edit: Option<table_ui::Target>,
    table_resize: Option<table_ui::Resize>,
    table_drag: bool,
    delivery_panel: bool,
    delivery_fields: Vec<Entity<InputState>>,
    delivery_cancel: Arc<AtomicBool>,
    delivery_busy: bool,
    workspace_panel: bool,
    doc_fields: Vec<Entity<InputState>>,
    notes_input: Entity<TextareaState>,
    doc_page: String,
    notes_base: String,
    chosen_layout: String,
    search_at: usize,
    workspace_poll: Option<Task<()>>,
    accelerated: bool,
    scene: Option<joydeck::scene::Scene>,
    scene_key: Option<(u64, u64, usize, u64)>,
    scene_busy: bool,
    scene_cancel: Arc<AtomicBool>,
    scene_task: Option<Task<()>>,
    scene_cache: Arc<std::sync::Mutex<joydeck::scene::SceneCache>>,
    gpu_cache: scene_canvas::GpuCache,
    text_edit: Option<joydeck::text::TextEdit>,
    ime_mark_calls: usize,
    ime_commit_calls: usize,
    text_lines: Vec<canvas_text::CanvasLine>,
    text_layout_key: Option<(Object, u32, Bounds<Pixels>)>,
    text_shape_count: usize,
    text_bounds: Option<Bounds<Pixels>>,
    text_anchor: Option<usize>,
    session: Session,
    selection: BTreeSet<String>,
    page_selection: BTreeSet<usize>,
    gesture: Option<Gesture>,
    canvas_origin: (f32, f32),
    visual_revision: u64,
    render_busy: bool,
    page: usize,
    selected: Option<usize>,
    fields: Vec<Entity<InputState>>,
    path: Entity<InputState>,
    media_path: Entity<InputState>,
    crop_fields: Vec<Entity<InputState>>,
    message: String,
    image: Option<Arc<RenderImage>>,
    pending: Option<(u64, u64, usize)>,
    render: Option<Task<()>>,
    io: Option<Task<()>>,
    saving: bool,
    focus: FocusHandle,
    recovery: Recovery,
    disk: Option<(PathBuf, Fingerprint)>,
    restored: Option<(Recovery, Record)>,
    close_after_save: Option<AnyWindowHandle>,
    checkpoint: Instant,
    page_drag: Option<Vec<usize>>,
    activation: Option<Subscription>,
    text_blur: Option<Subscription>,
    thumbs: BTreeMap<usize, Arc<RenderImage>>,
    thumb_key: (u64, u64),
    thumb_busy: bool,
    thumb_errors: BTreeSet<usize>,
    thumb_task: Option<Task<()>>,
    recovery_task: Option<Task<()>>,
}
impl Editor {
    fn new(w: &mut Window, cx: &mut Context<Self>) -> Self {
        let (preferences, notice) = joydeck::preferences::load(&joydeck::preferences::root())
            .unwrap_or_else(|e| (joydeck::preferences::Preferences::default(), Some(e)));
        let comparison_path = cx.new(|cx| InputState::new(w, cx).placeholder("参考文稿的完整路径"));
        let fields = [
            "80",
            "80",
            "240",
            "100",
            "#27b6a7",
            "输入文字",
            "0",
            "1",
            "#202b3a",
            "0",
            "960",
            "540",
            "sans-serif",
            "#202b3a",
        ]
        .iter()
        .map(|v| {
            cx.new(|cx| {
                let mut s = InputState::new(w, cx);
                s.set_value(*v, w, cx);
                s
            })
        })
        .collect();
        let path =
            cx.new(|cx| InputState::new(w, cx).placeholder("打开/另存为的完整 .joydeck 路径"));
        let media_path =
            cx.new(|cx| InputState::new(w, cx).placeholder("PNG / JPEG 完整路径（按内容识别）"));
        let crop_fields = ["0", "0", "1", "1"]
            .iter()
            .map(|v| {
                cx.new(|cx| {
                    let mut state = InputState::new(w, cx);
                    state.set_value(*v, w, cx);
                    state
                })
            })
            .collect();
        let doc_fields = [
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "#087f8c",
            "新名称",
            "",
            "40",
            "#ffffff",
        ]
        .iter()
        .map(|v| {
            cx.new(|cx| {
                let mut s = InputState::new(w, cx);
                s.set_value(*v, w, cx);
                s
            })
        })
        .collect();
        let delivery_fields = ["", "", "144"]
            .iter()
            .map(|v| {
                cx.new(|cx| {
                    let mut s = InputState::new(w, cx);
                    s.set_value(*v, w, cx);
                    s
                })
            })
            .collect();
        let notes_input = cx.new(|cx| TextareaState::new(w, cx).auto_grow(3, 8));
        let focus = cx.focus_handle();
        focus.focus(w, cx);
        let session = Session::new();
        let recovery = Recovery::new(recovery_root());
        let table_fields = ["4", "5", "44", "112", "6", "1", "#d8dfec", "18", "#ede9fe"]
            .iter()
            .map(|v| {
                cx.new(|cx| {
                    let mut s = InputState::new(w, cx);
                    s.set_value(*v, w, cx);
                    s
                })
            })
            .collect();
        let author_data = cx.new(|cx| TextareaState::new(w, cx).auto_grow(5, 14));
        let author_fields = [
            "",
            "",
            "",
            "",
            "",
            "",
            "#4472c4,#ed7d31,#a5a5a5,#ffc000",
            "",
            "28",
            "1",
        ]
        .iter()
        .map(|v| {
            cx.new(|cx| {
                let mut f = InputState::new(w, cx);
                f.set_value(*v, w, cx);
                f
            })
        })
        .collect();
        let motion_fields = ["600", "0", "0"]
            .iter()
            .map(|v| {
                cx.new(|cx| {
                    let mut f = InputState::new(w, cx);
                    f.set_value(*v, w, cx);
                    f
                })
            })
            .collect();
        let extended_fields = ["本机作者", "", "", "", "0", "5", "1", "0", "", "", ""]
            .iter()
            .map(|v| {
                cx.new(|cx| {
                    let mut f = InputState::new(w, cx);
                    f.set_value(*v, w, cx);
                    f
                })
            })
            .collect();
        Self {
            comparison_path,
            comparison: None,
            comparison_pick: 0,
            control_service: None,
            extended_fields,
            thread_pick: 0,
            clip_pick: 0,
            review_findings: vec![],
            av_player: None,
            av_image: None,
            av_slide: None,
            av_document_id: 0,
            av_seconds: 0.,
            av_sequence: 0,
            av_task: None,
            restore_focus_after_show: false,
            inspector_open: preferences.inspector,
            chart_grid: vec![],
            author_data,
            author_fields,
            motion_fields,
            effect_pick: 0,
            focus_initialized: false,
            ribbon: "开始".into(),
            zoom: preferences.zoom,
            last_scale: 1.,
            show_notes: preferences.notes,
            table_fields,
            table_pick: None,
            cell_edit: None,
            table_resize: None,
            table_drag: false,
            delivery_panel: false,
            delivery_fields,
            delivery_cancel: Arc::new(AtomicBool::new(false)),
            delivery_busy: false,
            notes_input,
            doc_page: String::new(),
            notes_base: String::new(),
            workspace_panel: false,
            doc_fields,
            chosen_layout: String::new(),
            search_at: 0,
            workspace_poll: None,
            accelerated: !std::env::args().any(|s| s == "--cpu-canvas"),
            scene: None,
            scene_key: None,
            scene_busy: false,
            scene_cancel: Arc::new(AtomicBool::new(false)),
            scene_task: None,
            scene_cache: scene_canvas::new_cache(),
            gpu_cache: scene_canvas::GpuCache::default(),
            text_edit: None,
            ime_mark_calls: 0,
            ime_commit_calls: 0,
            text_lines: vec![],
            text_layout_key: None,
            text_shape_count: 0,
            text_bounds: None,
            text_anchor: None,
            selection: BTreeSet::new(),
            page_selection: BTreeSet::from([0]),
            gesture: None,
            canvas_origin: (0., 0.),
            visual_revision: 1,
            render_busy: false,
            session,
            page: 0,
            selected: None,
            fields,
            path,
            media_path,
            crop_fields,
            message: notice.unwrap_or_else(|| "就绪 · 双击文字编辑，拖动对象调整布局".into()),
            preferences,
            image: None,
            pending: None,
            render: None,
            io: None,
            saving: false,
            focus,
            recovery,
            disk: None,
            restored: None,
            close_after_save: None,
            checkpoint: Instant::now(),
            page_drag: None,
            activation: None,
            text_blur: None,
            thumbs: BTreeMap::new(),
            thumb_key: (0, 0),
            thumb_busy: false,
            thumb_errors: BTreeSet::new(),
            thumb_task: None,
            recovery_task: None,
        }
    }
    fn prepare_show(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.restore_focus_after_show = true;
        self.focus.focus(w, cx);
    }
    fn renew_recovery(&mut self) {
        self.recovery = Recovery::new(recovery_root());
        self.disk = None;
        self.restored = None;
        self.close_after_save = None;
        self.checkpoint = Instant::now();
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.page = self.page.min(self.session.deck.slides.len() - 1);
        self.page_selection
            .retain(|i| *i < self.session.deck.slides.len());
        if self.page_selection.is_empty() {
            self.page_selection.insert(self.page);
        }
        self.selection.retain(|id| {
            self.session.deck.slides[self.page]
                .objects
                .iter()
                .any(|o| &o.id == id)
        });
        self.selected = self.session.deck.slides[self.page]
            .objects
            .iter()
            .position(|o| self.selection.contains(&o.id));
        if let Some(p) = self.table_pick.clone() {
            if let Some((_, t)) = self.table_target() {
                let focus = t.owner(
                    p.focus.0.min(t.rows.len().saturating_sub(1)),
                    p.focus.1.min(t.columns.len().saturating_sub(1)),
                );
                let anchor = t.owner(
                    p.anchor.0.min(t.rows.len().saturating_sub(1)),
                    p.anchor.1.min(t.columns.len().saturating_sub(1)),
                );
                if let (Ok(focus), Ok(anchor), Some(pick)) =
                    (focus, anchor, self.table_pick.as_mut())
                {
                    pick.focus = focus;
                    pick.anchor = anchor;
                } else {
                    self.table_pick = None;
                }
            } else {
                self.table_pick = None;
            }
        }
        self.visual_revision += 1;
        self.pending = None;
        self.image = None;
        self.message = if self.session.dirty() {
            "有未保存的修改".into()
        } else {
            "无未保存修改".into()
        };
        self.recover(cx);
        cx.notify();
    }
    fn command(&mut self, c: Command, cx: &mut Context<Self>) {
        if self.sync_document(cx) {
            self.message = "其他窗口已修改文稿，请重试操作".into();
            return;
        }
        self.finish_text(cx);
        self.gesture = None;
        self.page_drag = None;
        let page_id = self.session.deck.slides[self.page].id.clone();
        let command_id = c.id();
        eprintln!("M2 command={command_id} document={}", self.session.id);
        match self.session.execute(self.session.id, c) {
            Ok(()) => {
                if let Some(p) = self
                    .session
                    .deck
                    .slides
                    .iter()
                    .position(|s| s.id == page_id)
                {
                    self.page = p;
                }
                self.changed(cx);
            }
            Err(e) => {
                self.message = e;
                cx.notify();
            }
        }
    }
    fn import_media(
        &mut self,
        bytes: Option<Vec<u8>>,
        replace: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_text(cx);
        if self.io.is_some() || self.saving {
            self.message = "文件任务正在执行".into();
            cx.notify();
            return;
        }
        let path = PathBuf::from(self.media_path.read(cx).value().to_string());
        let target = if replace { self.selected } else { None };
        if replace && target.is_none() {
            self.message = "请选择图片".into();
            cx.notify();
            return;
        }
        let (id, page, revision) = (self.session.id, self.page, self.session.revision);
        self.message = "正在导入图片…".into();
        self.io = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if let Some(bytes) = bytes {
                        joydeck::media::Resource::import(&bytes)
                    } else {
                        joydeck::media::read(&path)
                    }
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.io = None;
                s.sync_document(cx);
                if s.session.id != id || s.session.revision != revision || s.page != page {
                    s.message = "文档已改变，图片导入已取消".into();
                    cx.notify();
                    return;
                }
                match result {
                    Ok(r) => {
                        s.command(Command::ImportImage(page, target, r), cx);
                        s.message = "图片已导入；裁剪保留原图".into();
                    }
                    Err(e) => s.message = e,
                }
                cx.notify();
            });
        }));
        self.focus.focus(w, cx);
        cx.notify();
    }
    fn crop_media(&mut self, reset: bool, cx: &mut Context<Self>) {
        let Some(index) = self.selected else {
            return;
        };
        let mut o = self.session.deck.slides[self.page].objects[index].clone();
        let Some(image) = o.image.as_mut() else {
            self.message = "请选择图片".into();
            cx.notify();
            return;
        };
        let mut crop = [0., 0., 1., 1.];
        if !reset {
            for (i, field) in self.crop_fields.iter().enumerate() {
                let Ok(v) = field.read(cx).value().parse::<f32>() else {
                    self.message = "裁剪需要 x/y/宽/高 四个数字，范围0–1".into();
                    cx.notify();
                    return;
                };
                crop[i] = v;
            }
        }
        image.crop = crop;
        self.command(Command::ReplaceObject(self.page, index, o), cx);
    }
    fn copy_objects(&mut self, cx: &mut Context<Self>) {
        match joydeck::media::Transfer::capture(&self.session.deck, self.page, &self.selection)
            .and_then(|t| serde_json::to_string(&t).map_err(|e| e.to_string()))
        {
            Ok(data) => cx.write_to_clipboard(ClipboardItem::new_string_with_metadata(
                "JoyDeck 对象".into(),
                data,
            )),
            Err(e) => self.message = e,
        }
        cx.notify();
    }
    fn paste_media(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            self.message = "剪贴板为空或当前平台无法读取".into();
            cx.notify();
            return;
        };
        if let Some(data) = item
            .metadata()
            .filter(|s| s.len() <= joydeck::document::MAX_PART)
        {
            match serde_json::from_str::<joydeck::media::Transfer>(data) {
                Ok(t) => {
                    self.command(Command::Paste(self.page, t), cx);
                    return;
                }
                Err(_) => { /* External metadata: continue to standard formats. */ }
            }
        }
        for entry in item.entries() {
            if let ClipboardEntry::Image(image) = entry {
                self.import_media(Some(image.bytes.clone()), false, w, cx);
                return;
            }
            if let ClipboardEntry::ExternalPaths(paths) = entry
                && let Some(path) = paths.paths().first()
            {
                self.media_path.update(cx, |s, cx| {
                    s.set_value(path.to_string_lossy().to_string(), w, cx)
                });
                self.import_media(None, false, w, cx);
                return;
            }
        }
        if let Some(value) = item.text() {
            let mut o = edit::object("text");
            o.text = value;
            self.command(Command::AddObject(self.page, o), cx);
        } else {
            self.message = "剪贴板无受支持的 PNG/JPEG、文字或内部对象".into();
            cx.notify();
        }
    }
    fn recover(&mut self, cx: &mut Context<Self>) {
        if !self.session.dirty() {
            self.recovery_task = None;
            return;
        }
        let service = self.recovery.clone();
        let deck = self.session.deck.clone();
        let rev = self.session.revision;
        let id = self.session.id;
        let source = self.disk.as_ref().map(|v| v.0.clone());
        let fp = self.disk.as_ref().map(|v| v.1.clone());
        let delay = if self.checkpoint.elapsed() > Duration::from_secs(60) {
            Duration::ZERO
        } else {
            Duration::from_secs(1)
        };
        self.recovery_task = Some(cx.spawn(async move |view, cx| {
            cx.background_executor().timer(delay).await;
            let result = cx
                .background_executor()
                .spawn(async move { service.write(rev, &deck, source, fp) })
                .await;
            let _ = view.update(cx, |s, cx| {
                if s.session.id != id {
                    return;
                }
                match result {
                    Ok(()) => s.checkpoint = Instant::now(),
                    Err(e) => s.message = format!("恢复快照未写入：{e}"),
                }
                cx.notify();
            });
        }));
    }
    fn history(&mut self, redo: bool, w: &mut Window, cx: &mut Context<Self>) {
        self.finish_text(cx);
        self.cancel_gesture(&CancelGesture, w, cx);
        if (redo && !self.session.can_redo()) || (!redo && !self.session.can_undo()) {
            return;
        }
        let page = self.session.deck.slides[self.page].id.clone();
        let pages: Vec<_> = self
            .page_ids()
            .into_iter()
            .map(|i| self.session.deck.slides[i].id.clone())
            .collect();
        if redo {
            self.session.redo();
        } else {
            self.session.undo();
        }
        if let Some(p) = self.session.deck.slides.iter().position(|s| s.id == page) {
            self.page = p;
        }
        self.page_selection = self
            .session
            .deck
            .slides
            .iter()
            .enumerate()
            .filter(|(_, s)| pages.contains(&s.id))
            .map(|(i, _)| i)
            .collect();
        self.changed(cx);
        self.sync_fields(w, cx);
        if matches!(self.ribbon.as_str(), "图表" | "图示" | "动画") {
            self.sync_author(w, cx);
        }
    }
    fn undo(&mut self, _: &Undo, w: &mut Window, cx: &mut Context<Self>) {
        self.history(false, w, cx);
    }
    fn redo(&mut self, _: &Redo, w: &mut Window, cx: &mut Context<Self>) {
        self.history(true, w, cx);
    }
    fn select(&mut self, i: usize, w: &mut Window, cx: &mut Context<Self>) {
        self.finish_text(cx);
        self.selection =
            BTreeSet::from([self.session.deck.slides[self.page].objects[i].id.clone()]);
        self.selected = Some(i);
        let effective = self.effective_objects();
        let o = &effective[i];
        for (field, value) in self.fields.iter().zip([
            format!("{:.2}", o.x),
            format!("{:.2}", o.y),
            format!("{:.2}", o.width),
            format!("{:.2}", o.height),
            o.fill.clone(),
            o.text.clone(),
            o.rotation.to_string(),
            o.opacity.to_string(),
            o.stroke.clone(),
            o.stroke_width.to_string(),
        ]) {
            field.update(cx, |s, cx| s.set_value(value, w, cx));
        }
        if let Some(image) = &o.image {
            for (field, value) in self.crop_fields.iter().zip(image.crop) {
                field.update(cx, |s, cx| s.set_value(value.to_string(), w, cx));
            }
        }
        if !matches!(self.ribbon.as_str(), "幻灯片放映" | "切换") {
            self.sync_author(w, cx);
        }
        self.focus.focus(w, cx);
        cx.notify();
    }
    fn apply(&mut self, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            return;
        }
        let values: Vec<_> = self
            .fields
            .iter()
            .map(|s| s.read(cx).value().to_string())
            .collect();
        let mut out = Vec::new();
        for o in &self.effective_objects() {
            if !self.selection.contains(&o.id) {
                continue;
            }
            let mut o = o.clone();
            let parse = |index: usize, current: f32| -> std::result::Result<f32, String> {
                if values[index].is_empty() {
                    Ok(current)
                } else {
                    values[index]
                        .parse()
                        .map_err(|_| "属性必须是有限数字".into())
                }
            };
            let result = (|| {
                o.x = parse(0, o.x)?;
                o.y = parse(1, o.y)?;
                o.width = parse(2, o.width)?;
                o.height = parse(3, o.height)?;
                if !values[4].is_empty() {
                    o.fill = values[4].clone();
                }
                if self.selection.len() == 1 || !values[5].is_empty() {
                    if o.text != values[5] {
                        o.runs.clear();
                    }
                    o.text = values[5].clone();
                }
                o.rotation = parse(6, o.rotation)?;
                o.opacity = parse(7, o.opacity)?;
                if !values[8].is_empty() {
                    o.stroke = values[8].clone();
                }
                o.stroke_width = parse(9, o.stroke_width)?;
                Ok::<_, String>(())
            })();
            if let Err(e) = result {
                self.message = e;
                cx.notify();
                return;
            }
            out.push(o);
        }
        self.command(Command::ReplaceMany(self.page, out), cx);
    }
    fn sync_fields(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        for (i, value) in [
            (10, self.session.deck.width.to_string()),
            (11, self.session.deck.height.to_string()),
        ] {
            self.fields[i].update(cx, |s, cx| s.set_value(value, w, cx));
        }
        let objects = self.selected_objects();
        if objects.is_empty() {
            return;
        }
        if objects.len() == 1 && objects[0].kind == "text" {
            let style = objects[0]
                .runs
                .first()
                .map(|r| r.style.clone())
                .unwrap_or_else(|| joydeck::text::Style::for_object(&objects[0]));
            self.fields[12].update(cx, |f, cx| f.set_value(style.family, w, cx));
            self.fields[13].update(cx, |f, cx| f.set_value(style.color, w, cx));
            self.author_fields[8].update(cx, |f, cx| f.set_value(style.size.to_string(), w, cx));
        }
        let values = |o: &Object| {
            vec![
                format!("{:.2}", o.x),
                format!("{:.2}", o.y),
                format!("{:.2}", o.width),
                format!("{:.2}", o.height),
                o.fill.clone(),
                o.text.clone(),
                o.rotation.to_string(),
                o.opacity.to_string(),
                o.stroke.clone(),
                o.stroke_width.to_string(),
            ]
        };
        let all: Vec<_> = objects.iter().map(values).collect();
        for (i, f) in self.fields.iter().take(10).enumerate() {
            let value = if all.iter().all(|v| v[i] == all[0][i]) {
                all[0][i].clone()
            } else {
                String::new()
            };
            f.update(cx, |s, cx| s.set_value(value, w, cx));
        }
    }
    fn select_multi(&mut self, i: usize, extend: bool, w: &mut Window, cx: &mut Context<Self>) {
        self.finish_text(cx);
        let old = self.selection.clone();
        self.select(i, w, cx);
        if extend {
            let id = self.session.deck.slides[self.page].objects[i].id.clone();
            self.selection = old;
            if !self.selection.insert(id.clone()) {
                self.selection.remove(&id);
            }
        }
        self.selected = self.session.deck.slides[self.page]
            .objects
            .iter()
            .position(|o| self.selection.contains(&o.id));
        self.sync_fields(w, cx);
        cx.notify();
    }
    fn layer(&mut self, front: bool, cx: &mut Context<Self>) {
        let objects = &self.session.deck.slides[self.page].objects;
        let ids: Vec<_> = objects
            .iter()
            .enumerate()
            .filter(|(_, o)| self.selection.contains(&o.id))
            .map(|(i, _)| i)
            .collect();
        let target = if front { objects.len() } else { 0 };
        self.command(Command::ReorderObjects(self.page, ids, target), cx);
    }
    fn delete_selection(&mut self, cx: &mut Context<Self>) {
        let mut ids: Vec<_> = self.session.deck.slides[self.page]
            .objects
            .iter()
            .enumerate()
            .filter(|(_, o)| self.selection.contains(&o.id))
            .map(|(i, _)| i)
            .collect();
        ids.reverse();
        self.command(
            Command::Batch(
                ids.into_iter()
                    .map(|i| Command::DeleteObject(self.page, i))
                    .collect(),
            ),
            cx,
        );
    }
    fn page_size(&mut self, cx: &mut Context<Self>) {
        match (
            self.fields[10].read(cx).value().parse::<f32>(),
            self.fields[11].read(cx).value().parse::<f32>(),
        ) {
            (Ok(w), Ok(h)) => self.command(Command::PageSize(w, h), cx),
            _ => {
                self.message = "页面宽高必须为1–4096点的有限数字".into();
                cx.notify();
            }
        }
    }
    fn selected_objects(&self) -> Vec<Object> {
        self.effective_objects()
            .iter()
            .filter(|o| self.selection.contains(&o.id))
            .cloned()
            .collect()
    }
    fn cancel_gesture(&mut self, _: &CancelGesture, _: &mut Window, cx: &mut Context<Self>) {
        if self.table_resize.take().is_some() {
            self.visual_revision += 1;
            self.pending = None;
            cx.notify();
        }
        self.table_drag = false;
        if let Some(t) = &mut self.text_edit {
            if t.marked.is_some() {
                t.cancel();
                cx.notify();
            } else {
                self.finish_text(cx);
            }
            return;
        }
        if self.table_pick.take().is_some() {
            cx.notify();
        }
        self.page_drag = None;
        if let Some(g) = self.gesture.take() {
            self.image = g.image_before;
            self.visual_revision += 1;
            self.pending = None;
            cx.notify();
        }
    }
    fn down(&mut self, e: &MouseDownEvent, scale: f32, w: &mut Window, cx: &mut Context<Self>) {
        if self.text_pointer(e, w, cx) {
            return;
        }
        let p = (
            (f32::from(e.position.x) - self.canvas_origin.0) / scale,
            (f32::from(e.position.y) - self.canvas_origin.1) / scale,
        );
        let base = self.selected_objects();
        let r = if base.is_empty() {
            None
        } else {
            Some(geometry::union(&base))
        };
        let radius = 10. / scale;
        let mut handle = (1, 1);
        let mode = r.and_then(|r| {
            let handles = [
                (-1, -1),
                (0, -1),
                (1, -1),
                (-1, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ];
            if let Some((h, _)) = handles
                .into_iter()
                .map(|h| {
                    let x = r.x + r.w * (f32::from(h.0) + 1.) / 2.;
                    let y = r.y + r.h * (f32::from(h.1) + 1.) / 2.;
                    (h, (p.0 - x).hypot(p.1 - y))
                })
                .filter(|(_, d)| *d < radius)
                .min_by(|a, b| a.1.total_cmp(&b.1))
            {
                handle = h;
                Some(Mode::Scale)
            } else if (p.0 - r.x - r.w / 2.).hypot(p.1 - r.y + 24. / scale) < radius {
                Some(Mode::Rotate)
            } else {
                None
            }
        });
        if mode.is_none()
            && let Some(i) = self
                .effective_objects()
                .iter()
                .rposition(|o| geometry::hit(o, p.0, p.1))
            && self.table_down(i, p, e, scale, w, cx)
        {
            return;
        }
        self.table_pick = None;
        if let Some(mode) = mode {
            self.gesture = Some(Gesture {
                image_before: self.image.clone(),
                handle: (1, 1),
                shift: e.modifiers.shift,
                center: e.modifiers.alt,
                base,
                start: p,
                end: p,
                mode: Some(mode),
                scale,
            });
        } else if let Some(i) = self
            .effective_objects()
            .iter()
            .rposition(|o| geometry::hit(o, p.0, p.1))
        {
            if e.modifiers.shift
                || e.modifiers.control
                || !self
                    .selection
                    .contains(&self.session.deck.slides[self.page].objects[i].id)
            {
                self.select_multi(i, e.modifiers.shift || e.modifiers.control, w, cx);
            }
            if e.click_count >= 2 && self.session.deck.slides[self.page].objects[i].kind == "text" {
                self.begin_text(i, w, cx);
                return;
            }
            self.gesture = Some(Gesture {
                image_before: self.image.clone(),
                handle: (1, 1),
                shift: e.modifiers.shift,
                center: e.modifiers.alt,
                base: self.selected_objects(),
                start: p,
                end: p,
                mode: Some(Mode::Move),
                scale,
            });
        } else {
            if !e.modifiers.shift && !e.modifiers.control {
                self.selection.clear();
                self.selected = None;
            }
            self.gesture = Some(Gesture {
                image_before: self.image.clone(),
                handle: (1, 1),
                shift: e.modifiers.shift,
                center: e.modifiers.alt,
                base: Vec::new(),
                start: p,
                end: p,
                mode: None,
                scale,
            });
        }
        if let Some(g) = &mut self.gesture {
            g.handle = handle;
        }
        self.focus.focus(w, cx);
        cx.notify();
    }
    fn move_pointer(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.table_move(e, cx) {
            return;
        }
        if self.text_edit.is_some() {
            self.text_move(e, cx);
            return;
        }
        if let Some(g) = &mut self.gesture {
            if !e.dragging() {
                let image = g.image_before.clone();
                self.gesture = None;
                self.image = image;
            } else {
                g.shift = e.modifiers.shift;
                g.center = e.modifiers.alt;
                g.end = (
                    (f32::from(e.position.x) - self.canvas_origin.0) / g.scale,
                    (f32::from(e.position.y) - self.canvas_origin.1) / g.scale,
                );
            }
            self.visual_revision += 1;
            self.pending = None;
            cx.notify();
        }
    }
    fn end_gesture(&mut self, cx: &mut Context<Self>) {
        let Some(g) = self.gesture.take() else {
            return;
        };
        if (g.start.0 - g.end.0).hypot(g.start.1 - g.end.1) * g.scale < 3. {
            self.image = g.image_before;
            self.visual_revision += 1;
            self.pending = None;
            cx.notify();
            return;
        }
        if g.mode.is_some() {
            match g.result() {
                Ok(v) => self.command(Command::ReplaceMany(self.page, v), cx),
                Err(e) => self.message = e,
            }
        } else {
            let r = Rect {
                x: g.start.0.min(g.end.0),
                y: g.start.1.min(g.end.1),
                w: (g.end.0 - g.start.0).abs(),
                h: (g.end.1 - g.start.1).abs(),
            };
            for o in &self.session.deck.slides[self.page].objects {
                let b = geometry::bounds(o);
                if r.contains(b.x, b.y) && r.contains(b.x + b.w, b.y + b.h) {
                    self.selection.insert(o.id.clone());
                }
            }
            self.selected = self.session.deck.slides[self.page]
                .objects
                .iter()
                .position(|o| self.selection.contains(&o.id));
        }
        self.visual_revision += 1;
        self.pending = None;
        cx.notify();
    }
    fn display_objects(&self) -> Vec<Object> {
        let mut objects = self.effective_objects();
        if let Some(g) = &self.gesture
            && g.mode.is_some()
            && let Ok(changes) = g.result()
        {
            for o in changes {
                if let Some(old) = objects.iter_mut().find(|old| old.id == o.id) {
                    *old = o;
                }
            }
        }
        if let Some(g) = &self.table_resize
            && let Some(o) = objects.iter_mut().find(|o| o.id == g.base.id)
            && let Ok(resized) = g.result()
        {
            *o = resized;
        }
        objects
    }
    fn display_deck(&self) -> Arc<Deck> {
        if let Some(g) = &self.table_resize {
            let mut d = (*self.session.deck).clone();
            if let Some(o) = d.slides[self.page]
                .objects
                .iter_mut()
                .find(|o| o.id == g.base.id)
                && let Ok(resized) = g.result()
            {
                *o = resized;
            }
            return Arc::new(d);
        }
        if let Some(g) = &self.gesture
            && g.mode.is_some()
            && let Ok(objects) = g.result()
        {
            let mut d = (*self.session.deck).clone();
            for o in objects {
                if let Some(old) = d.slides[self.page]
                    .objects
                    .iter_mut()
                    .find(|v| v.id == o.id)
                {
                    *old = o;
                }
            }
            return Arc::new(d);
        }
        self.session.deck.clone()
    }
    fn page_click(&mut self, p: usize, shift: bool, ctrl: bool, cx: &mut Context<Self>) {
        self.finish_text(cx);
        self.gesture = None;
        if shift {
            for i in self.page.min(p)..=self.page.max(p) {
                self.page_selection.insert(i);
            }
        } else if ctrl {
            if !self.page_selection.insert(p) {
                self.page_selection.remove(&p);
            }
        } else {
            self.page_selection = BTreeSet::from([p]);
        }
        self.page = p;
        self.selection.clear();
        self.selected = None;
        self.pending = None;
        self.image = None;
        self.visual_revision += 1;
        cx.notify();
    }
    fn page_ids(&self) -> Vec<usize> {
        if self.page_selection.is_empty() {
            vec![self.page]
        } else {
            self.page_selection.iter().copied().collect()
        }
    }
    fn page_command(&mut self, which: &str, cx: &mut Context<Self>) {
        let ids = self.page_ids();
        let target = if which == "up" {
            ids[0].saturating_sub(1)
        } else {
            (ids[ids.len() - 1] + 2).min(self.session.deck.slides.len())
        };
        let command = match which {
            "copy" => Command::DuplicatePages(ids),
            "delete" => Command::DeletePages(ids),
            "hide" => Command::HidePages(ids),
            _ => Command::ReorderPages(ids, target),
        };
        let selected: Vec<_> = self
            .page_ids()
            .into_iter()
            .map(|p| self.session.deck.slides[p].id.clone())
            .collect();
        self.command(command, cx);
        self.page_selection = self
            .session
            .deck
            .slides
            .iter()
            .enumerate()
            .filter(|(_, s)| selected.contains(&s.id))
            .map(|(i, _)| i)
            .collect();
        if let Some(i) = self.page_selection.first() {
            self.page = *i;
        }
        cx.notify();
    }
    fn save(&mut self, _: &Save, w: &mut Window, cx: &mut Context<Self>) {
        self.finish_text(cx);
        self.cancel_gesture(&CancelGesture, w, cx);
        let path = if self.path.read(cx).value().trim().is_empty() {
            self.session.file().map(|v| v.0).unwrap_or_default()
        } else {
            PathBuf::from(self.path.read(cx).value().trim())
        };
        if path.as_os_str().is_empty() {
            self.file_dialog("save", w, cx);
            return;
        }
        let expected = self
            .disk
            .as_ref()
            .filter(|(p, _)| p == &path)
            .map(|(_, f)| f.clone());
        self.save_to(path, expected, w.window_handle(), cx);
    }
    fn save_to(
        &mut self,
        path: PathBuf,
        expected: Option<Fingerprint>,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if self.saving {
            return;
        }
        if path.as_os_str().is_empty() {
            self.message = "请填写完整 .joydeck 路径".into();
            self.close_after_save = None;
            cx.notify();
            return;
        }
        self.sync_document(cx);
        let Some(lease) = self.session.try_save() else {
            self.message = "其他窗口正在保存此文稿".into();
            cx.notify();
            return;
        };
        let snapshot = self.session.deck.clone();
        let deck = snapshot.clone();
        let id = self.session.id;
        let rev = self.session.revision;
        let recovery = self.recovery.clone();
        let restored = self.restored.clone();
        let dest = path.clone();
        self.saving = true;
        self.message = "正在处理文件，请稍候…".into();
        cx.notify();
        self.io = Some(cx.spawn(async move |view, cx| {
            let _lease = lease;
            let result = cx
                .background_executor()
                .spawn(async move {
                    if expected.is_none() && dest.exists() {
                        return Ok((None, Some(storage::fingerprint(&dest)?), None));
                    }
                    let fp = storage::save(&dest, &deck, expected)?;
                    let mut warning = recovery.clean(rev).err();
                    if let Some((service, record)) = restored
                        && let Err(e) = service.clean_restored(&record)
                    {
                        warning = Some(e);
                    }
                    Ok::<_, String>((Some(fp), None, warning))
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.saving = false;
                s.sync_document(cx);
                if s.session.id != id {
                    return;
                }
                match result {
                    Ok((Some(fp), _, warning)) => {
                        s.remember_path(path.clone(), cx);
                        s.restored = None;
                        s.disk = Some((path.clone(), fp));
                        s.session.set_file(s.disk.clone());
                        s.session.mark_saved(snapshot);
                        s.message = if s.session.dirty() {
                            "快照已保存，后续修改尚未保存".into()
                        } else {
                            "已保存".into()
                        };
                        if let Some(warning) = warning {
                            s.message.push_str(&format!("；恢复清理未完成：{warning}"));
                        }
                        if !s.session.dirty()
                            && let Some(close) = s.close_after_save.take()
                        {
                            cx.spawn(async move |_, cx| {
                                let _ = close.update(cx, |_, w, _| w.remove_window());
                            })
                            .detach();
                        } else {
                            s.close_after_save = None;
                        }
                    }
                    Ok((_, Some(fp), _)) => {
                        let path = path.clone();
                        cx.spawn(async move |view, cx| {
                            let _ = window.update(cx, |_, w, cx| {
                                let _ =
                                    view.update(cx, |s, cx| s.confirm_overwrite(path, fp, w, cx));
                            });
                        })
                        .detach();
                    }
                    Err(e) => {
                        s.message = format!("保存失败：{e}；可使用新路径另存");
                        s.close_after_save = None;
                    }
                    _ => {}
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn confirm_overwrite(
        &mut self,
        path: PathBuf,
        fp: Fingerprint,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let weak = cx.entity().downgrade();
        let id = self.session.id;
        w.open_dialog(cx, move |d, _, _| {
            let cancel = weak.clone();
            let weak = weak.clone();
            let path = path.clone();
            let fp = fp.clone();
            let dismiss = cancel.clone();
            d.on_close(move |_, _, cx| {
                let _ = dismiss.update(cx, |s, _| s.close_after_save = None);
            })
            .title("目标文件已存在")
            .child("确认替换此目标？提交前会再次检查是否被其他程序修改。")
            .child(
                Button::new("overwrite-confirm")
                    .label("覆盖此目标")
                    .on_click(move |_, w, cx| {
                        let close = weak
                            .update(cx, |s, _| s.close_after_save.take())
                            .unwrap_or(None);
                        w.close_dialog(cx);
                        let _ = weak.update(cx, |s, cx| {
                            s.close_after_save = close;
                            if s.session.id == id {
                                s.save_to(path.clone(), Some(fp.clone()), w.window_handle(), cx);
                            }
                        });
                    }),
            )
            .child(Button::new("overwrite-cancel").label("取消").on_click({
                let weak = cancel.clone();
                move |_, w, cx| {
                    w.close_dialog(cx);
                    let _ = weak.update(cx, |s, _| s.close_after_save = None);
                }
            }))
        });
    }
    fn open(&mut self, _recover: bool, w: &mut Window, cx: &mut Context<Self>) {
        self.finish_text(cx);
        if self.session.dirty() {
            self.message = "当前有未保存修改，请先保存或新建时明确放弃".into();
            cx.notify();
            return;
        }
        if self.saving {
            return;
        }
        self.gesture = None;
        let path = PathBuf::from(self.path.read(cx).value().trim());
        let source = path.clone();
        let handle = w.window_handle();
        let id = self.session.id;
        let rev = self.session.revision;
        self.saving = true;
        self.message = "正在处理文件，请稍候…".into();
        cx.notify();
        self.io = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { storage::open(&source) })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.saving = false;
                s.sync_document(cx);
                if s.session.id != id || s.session.revision != rev {
                    s.message = "打开期间发生编辑，已丢弃载入结果".into();
                    cx.notify();
                    return;
                }
                match result {
                    Ok((d, fp)) => {
                        s.remember_path(path.clone(), cx);
                        s.session = Session::from_deck(d, true);
                        s.renew_recovery();
                        s.disk = Some((path, fp));
                        s.session.set_file(s.disk.clone());
                        s.page = 0;
                        s.page_selection = BTreeSet::from([0]);
                        s.selection.clear();
                        s.selected = None;
                        s.pending = None;
                        s.image = None;
                        s.visual_revision += 1;
                        s.message = "已打开原生文稿".into();
                        cx.spawn(async move |view, cx| {
                            let _ = handle.update(cx, |_, w, cx| {
                                let _ = view.update(cx, |s, cx| s.sync_fields(w, cx));
                            });
                        })
                        .detach();
                    }
                    Err(e) => s.message = format!("打开失败：{e}"),
                }
                cx.notify();
            });
        }));
    }
    fn discard_and_close(&mut self, window: AnyWindowHandle, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.recovery_task = None;
        self.close_after_save = None;
        let service = self.recovery.clone();
        let restored = self.restored.take();
        let id = self.session.id;
        let revision = self.session.revision;
        self.saving = true;
        self.message = "正在清理本会话恢复记录…".into();
        self.io = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    service.discard()?;
                    if let Some((r, record)) = restored {
                        r.clean_restored(&record)?;
                    }
                    Ok::<_, String>(())
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.saving = false;
                s.sync_document(cx);
                if s.session.id != id {
                    return;
                }
                if result.is_ok() && s.session.revision == revision {
                    cx.spawn(async move |_, cx| {
                        let _ = window.update(cx, |_, w, _| w.remove_window());
                    })
                    .detach();
                } else {
                    s.recovery = Recovery::new(recovery_root());
                    s.recover(cx);
                    s.message = match result {
                        Err(e) => format!("恢复清理失败，窗口保留：{e}"),
                        Ok(()) => "放弃请求后又有修改，窗口保留".into(),
                    };
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn close(&mut self, _: &Close, w: &mut Window, cx: &mut Context<Self>) {
        self.finish_text(cx);
        self.cancel_gesture(&CancelGesture, w, cx);
        if self.saving {
            self.message = "正在读写文件，请稍候".into();
            cx.notify();
            return;
        }
        if !self.session.dirty() {
            w.remove_window();
            return;
        }
        let weak = cx.entity().downgrade();
        w.open_dialog(cx, move |d, _, _| {
            let a = weak.clone();
            let b = weak.clone();
            d.title("有未保存修改")
                .child("保存成功后关闭；失败或后续又有修改时保持窗口打开。")
                .child(
                    Button::new("save-close")
                        .label("保存后关闭")
                        .on_click(move |_, w, cx| {
                            w.close_dialog(cx);
                            let _ = a.update(cx, |s, cx| {
                                s.close_after_save = Some(w.window_handle());
                                s.save(&Save, w, cx);
                            });
                        }),
                )
                .child(Button::new("discard-close").label("放弃并关闭").on_click(
                    move |_, w, cx| {
                        w.close_dialog(cx);
                        let _ = b.update(cx, |s, cx| s.discard_and_close(w.window_handle(), cx));
                    },
                ))
                .child(
                    Button::new("cancel-close")
                        .label("取消关闭")
                        .on_click(|_, w, cx| w.close_dialog(cx)),
                )
        });
    }
    fn recovery_list(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let service = self.recovery.clone();
        let handle = w.window_handle();
        self.io = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { service.list() })
                .await;
            let _ = handle.update(cx, |_, w, cx| {
                let _ = view.update(cx, |s, cx| match result {
                    Ok(rows) => {
                        let weak = cx.entity().downgrade();
                        let rows = Arc::new(rows);
                        w.open_dialog(cx, move |d, _, _| {
                            let rows = rows.clone();
                            let weak = weak.clone();
                            d.title("恢复列表 · 恢复为未保存副本")
                                .w(px(620.))
                                .child(format!(
                                    "{} 个快照 · 活跃实例不可恢复 · 损坏项已隔离",
                                    rows.len()
                                ))
                                .child(
                                    uniform_list(
                                        "recovery-list",
                                        rows.len(),
                                        move |range, _, _| {
                                            range
                                                .map(|i| {
                                                    let e = &rows[i];
                                                    let record = e.record.clone();
                                                    let weak = weak.clone();
                                                    div()
                                                        .h(px(124.))
                                                        .p_2()
                                                        .child(format!(
                                                            "{} · 修订 {}\n{}",
                                                            e.record.session,
                                                            e.record.revision,
                                                            e.record
                                                                .source
                                                                .as_ref()
                                                                .map(|p| p.display().to_string())
                                                                .unwrap_or_else(
                                                                    || "未命名文稿".into()
                                                                )
                                                        ))
                                                        .child(e.problem.clone().unwrap_or_else(
                                                            || {
                                                                if e.active {
                                                                    "另一实例正在使用".into()
                                                                } else {
                                                                    "可恢复".into()
                                                                }
                                                            },
                                                        ))
                                                        .child(
                                                            Button::new(("restore", i))
                                                                .label("恢复此副本")
                                                                .disabled(
                                                                    e.active || e.problem.is_some(),
                                                                )
                                                                .on_click(move |_, w, cx| {
                                                                    let _ =
                                                                        weak.update(cx, |s, cx| {
                                                                            s.restore(
                                                                                record.clone(),
                                                                                w,
                                                                                cx,
                                                                            )
                                                                        });
                                                                }),
                                                        )
                                                })
                                                .collect::<Vec<_>>()
                                        },
                                    )
                                    .h(px(380.)),
                                )
                        })
                    }
                    Err(e) => {
                        s.message = e;
                        cx.notify();
                    }
                });
            });
        }));
    }
    fn restore(&mut self, record: Record, w: &mut Window, cx: &mut Context<Self>) {
        if self.session.dirty() || self.saving {
            self.message = "请先处理当前未保存修改再恢复".into();
            w.close_dialog(cx);
            cx.notify();
            return;
        }
        w.close_dialog(cx);
        let handle = w.window_handle();
        let service = self.recovery.clone();
        let restored = (service.clone(), record.clone());
        let id = self.session.id;
        let rev = self.session.revision;
        self.saving = true;
        self.message = "正在处理文件，请稍候…".into();
        cx.notify();
        self.io = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { service.restore(&record) })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.saving = false;
                s.sync_document(cx);
                if s.session.id != id || s.session.revision != rev {
                    return;
                }
                match result {
                    Ok(d) => {
                        s.session = Session::from_deck(d, false);
                        s.renew_recovery();
                        s.restored = Some(restored);
                        s.page = 0;
                        s.selection.clear();
                        s.page_selection = BTreeSet::from([0]);
                        s.selected = None;
                        s.pending = None;
                        s.image = None;
                        s.visual_revision += 1;
                        s.message = "已恢复为未保存副本，请另存".into();
                        cx.spawn(async move |view, cx| {
                            let _ = handle.update(cx, |_, w, cx| {
                                let _ = view.update(cx, |s, cx| {
                                    s.path.update(cx, |p, cx| p.set_value("", w, cx));
                                    s.sync_fields(w, cx);
                                });
                            });
                        })
                        .detach();
                    }
                    Err(e) => s.message = e,
                }
                cx.notify();
            });
        }));
    }
    fn reset(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.finish_text(cx);
        let weak = cx.entity().downgrade();
        w.open_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            dialog
                .title("新建文稿")
                .child("确认放弃当前未保存内容并新建？此会话恢复快照也会清理。")
                .child(Button::new("reset-confirm").label("放弃并新建").on_click(
                    move |_, w, cx| {
                        let _ = weak.update(cx, |s, cx| {
                            if s.saving {
                                return;
                            }
                            let service = s.recovery.clone();
                            s.recovery_task = None;
                            let restored = s.restored.take();
                            cx.background_executor()
                                .spawn(async move {
                                    let _ = service.discard();
                                    if let Some((r, record)) = restored {
                                        let _ = r.clean_restored(&record);
                                    }
                                })
                                .detach();
                            s.path.update(cx, |p, cx| p.set_value("", w, cx));
                            s.session = Session::new();
                            s.renew_recovery();
                            s.page = 0;
                            s.changed(cx);
                            s.sync_fields(w, cx);
                        });
                        w.close_dialog(cx);
                    },
                ))
        });
    }
    fn request_thumbs(&mut self, range: std::ops::Range<usize>, cx: &mut Context<Self>) {
        let key = (self.session.id, self.session.revision);
        if self.thumb_key != key {
            self.thumbs.clear();
            self.thumb_errors.clear();
            self.thumb_key = key;
        }
        if self.thumb_busy {
            return;
        }
        let pages: Vec<_> = range
            .filter(|p| !self.thumbs.contains_key(p) && !self.thumb_errors.contains(p))
            .take(8)
            .collect();
        if pages.is_empty() {
            return;
        }
        self.thumb_busy = true;
        let deck = self.session.deck.clone();
        self.thumb_task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    pages
                        .into_iter()
                        .map(|p| (p, preview::render(&deck, p, 140, &AtomicBool::new(false))))
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.thumb_busy = false;
                if s.thumb_key != key {
                    cx.notify();
                    return;
                }
                for (p, r) in result {
                    match r {
                        Ok(r) => {
                            if s.thumbs.len() >= 32
                                && let Some(k) =
                                    s.thumbs.keys().copied().max_by_key(|k| k.abs_diff(p))
                            {
                                s.thumbs.remove(&k);
                            }
                            if let Some(pixels) =
                                image::RgbaImage::from_raw(r.width, r.height, r.bgra)
                            {
                                s.thumbs.insert(
                                    p,
                                    Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])),
                                );
                            } else {
                                s.thumb_errors.insert(p);
                            }
                        }
                        Err(_) => {
                            s.thumb_errors.insert(p);
                        }
                    }
                }
                cx.notify();
            });
        }));
    }
    fn request_render(&mut self, cx: &mut Context<Self>) {
        if self.accelerated {
            self.request_scene(cx);
            return;
        }
        let key = (self.session.id, self.visual_revision, self.page);
        if self.render_busy {
            return;
        }
        if self.pending == Some(key) {
            return;
        }
        self.pending = Some(key);
        self.render_busy = true;
        self.gpu_cache.stats.full_rasters += 1;
        let mut deck = (*self.display_deck()).clone();
        self.suppress_active_text(&mut deck);
        self.render = Some(cx.spawn(async move |view, cx| {
            let r = cx
                .background_executor()
                .spawn(async move { preview::render(&deck, key.2, 1200, &AtomicBool::new(false)) })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.sync_document(cx);
                s.render_busy = false;
                if s.pending != Some(key) {
                    cx.notify();
                    return;
                }
                match r {
                    Ok(r) => {
                        if let Some(pixels) = image::RgbaImage::from_raw(r.width, r.height, r.bgra)
                        {
                            s.image =
                                Some(Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])));
                        } else {
                            s.message = "预览像素与画布尺寸不匹配".into();
                        }
                    }
                    Err(e) => s.message = e,
                }
                cx.notify();
            });
        }));
    }
}
impl Render for Editor {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_workspace(w, cx)
    }
}

pub fn open(cx: &mut App) -> Result<()> {
    open_session(cx, None)
}
/// Opens in its own editor; repeated requests for the same saved file activate its existing view.
pub fn open_path(path: PathBuf, cx: &mut App) -> Result<()> {
    let path = path.canonicalize()?;
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "joydeck" | "pptx" | "potx" | "ppsx") {
        return Err(std::io::Error::other("不支持的文稿类型").into());
    }
    if let Some(registry) = cx.try_global::<Registry>() {
        let existing = registry.0.iter().find_map(|(handle, view)| {
            view.upgrade().and_then(|view| {
                view.read(cx)
                    .session
                    .file()
                    .filter(|(p, _)| p.canonicalize().ok().as_ref() == Some(&path))
                    .map(|_| *handle)
            })
        });
        if let Some(handle) = existing {
            handle.update(cx, |_, w, _| w.activate_window())?;
            return Ok(());
        }
    }
    open(cx)?;
    let (handle, view) = cx
        .global::<Registry>()
        .0
        .last()
        .cloned()
        .ok_or_else(|| std::io::Error::other("打开文稿后未找到编辑窗口"))?;
    handle.update(cx, |_, w, cx| {
        let _ = view.update(cx, |s, cx| {
            if extension == "joydeck" {
                s.path.update(cx, |f, cx| {
                    f.set_value(path.to_string_lossy().to_string(), w, cx)
                });
                s.open(false, w, cx);
            } else {
                s.delivery_fields[0].update(cx, |f, cx| {
                    f.set_value(path.to_string_lossy().to_string(), w, cx)
                });
                s.delivery("import", w, cx);
            }
        });
    })?;
    Ok(())
}
fn open_session(cx: &mut App, shared: Option<Session>) -> Result<()> {
    let preferences = joydeck::preferences::load(&joydeck::preferences::root())
        .map(|p| p.0)
        .unwrap_or_default();
    cx.bind_keys([
        KeyBinding::new("cmd-n", NewDocument, Some("Editor")),
        KeyBinding::new("cmd-shift-n", NewSlide, Some("Editor")),
        KeyBinding::new("f5", ShowFirst, Some("Editor")),
        KeyBinding::new("shift-f5", ShowCurrent, Some("Editor")),
        KeyBinding::new("escape", CancelGesture, Some("Editor")),
        KeyBinding::new("tab", CellNext, Some("Editor && !Input")),
        KeyBinding::new("shift-tab", CellPrevious, Some("Editor && !Input")),
        KeyBinding::new("cmd-z", Undo, Some("Editor && !Input")),
        KeyBinding::new("cmd-shift-z", Redo, Some("Editor && !Input")),
        KeyBinding::new("cmd-s", Save, Some("Editor")),
        KeyBinding::new("cmd-o", OpenFile, Some("Editor")),
        KeyBinding::new("ctrl-o", OpenFile, Some("Editor")),
        KeyBinding::new("cmd-shift-s", SaveAs, Some("Editor")),
        KeyBinding::new("cmd-w", Close, Some("Editor")),
        KeyBinding::new("ctrl-z", Undo, Some("Editor && !Input")),
        KeyBinding::new("ctrl-y", Redo, Some("Editor && !Input")),
        KeyBinding::new("ctrl-shift-s", SaveAs, Some("Editor")),
        KeyBinding::new("ctrl-s", Save, Some("Editor")),
        KeyBinding::new("ctrl-w", Close, Some("Editor")),
    ]);
    let (handle, view) = gpui_kit::open_window(
        WindowOptions {
            titlebar: Some(TitlebarOptions {
                appears_transparent: true,
                traffic_light_position: Some(point(px(12.), px(9.))),
                ..Default::default()
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(preferences.width as f32), px(preferences.height as f32)),
                cx,
            ))),
            ..Default::default()
        },
        cx,
        |w, cx| {
            w.set_window_title("JoyDeck — 演示文稿");
            let entity = cx.new(|cx| {
                let mut editor = Editor::new(w, cx);
                if let Some(shared) = shared {
                    editor.session = shared;
                    editor.disk = editor.session.file();
                }
                editor
            });
            entity.update(cx, |s, cx| {
                s.workspace_poll = Some(cx.spawn(async move |view, cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(100))
                            .await;
                        if view
                            .update(cx, |s, cx| {
                                s.sync_document(cx);
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                }));
                s.text_blur = Some(cx.on_blur(&s.focus.clone(), w, |s, _, cx| {
                    if let Some(t) = &mut s.text_edit {
                        t.cancel();
                    }
                    s.publish_text(cx);
                }));
                s.activation = Some(cx.observe_window_activation(w, |s, w, cx| {
                    if w.is_window_active() && s.restore_focus_after_show {
                        s.focus.focus(w, cx);
                        s.restore_focus_after_show = false;
                    }
                    if !w.is_window_active() {
                        if let Some(t) = &mut s.text_edit {
                            t.cancel();
                            s.publish_text(cx);
                            s.text_anchor = None;
                        } else {
                            s.cancel_gesture(&CancelGesture, w, cx);
                        }
                        s.page_drag = None;
                    }
                }))
            });
            let weak = entity.downgrade();
            w.on_window_should_close(cx, move |w, cx| {
                weak.update(cx, |s, cx| {
                    if s.session.dirty() || s.saving || s.text_edit.is_some() {
                        s.close(&Close, w, cx);
                        false
                    } else {
                        true
                    }
                })
                .unwrap_or(true)
            });
            entity
        },
    )?;
    if cx.try_global::<Registry>().is_none() {
        cx.set_global(Registry::default());
    }
    cx.global_mut::<Registry>()
        .0
        .retain(|(_, view)| view.upgrade().is_some());
    cx.global_mut::<Registry>()
        .0
        .push((handle, view.downgrade()));
    #[cfg(feature = "validation")]
    if std::env::args().any(|s| s == "--workspace-smoke") && cx.global::<Registry>().0.len() == 1 {
        workspace_smoke::run(handle, view.clone(), cx);
    }
    #[cfg(feature = "validation")]
    if std::env::args().any(|s| s == "--delivery-smoke") && cx.global::<Registry>().0.len() == 1 {
        delivery_smoke::run(handle, view.clone(), cx);
    }
    #[cfg(feature = "validation")]
    if std::env::args().any(|s| s == "--extended-smoke") {
        extended_smoke::run(handle, view.clone(), cx);
    }
    #[cfg(feature = "validation")]
    if std::env::args().any(|s| s == "--beta-smoke") && cx.global::<Registry>().0.len() == 1 {
        beta_smoke::run(handle, view.clone(), cx);
    }
    #[cfg(feature = "validation")]
    if std::env::args().any(|s| s == "--authoring-smoke") {
        authoring_smoke::run(handle, view.clone(), cx);
    }
    #[cfg(feature = "validation")]
    if std::env::args().any(|s| s == "--table-smoke") {
        table_smoke::run(handle, view.clone(), cx);
    }
    if std::env::args().any(|s| s == "--authoring-demo") && cx.global::<Registry>().0.len() == 1 {
        let _ = handle.update(cx, |_, w, cx| {
            view.update(cx, |s, cx| {
                s.session = Session::from_deck(joydeck::authoring_demo::deck(), false);
                s.renew_recovery();
                s.page = 0;
                s.selection.clear();
                s.selected = None;
                s.ribbon = "开始".into();
                s.sync_fields(w, cx);
                s.changed(cx);
            })
        });
    }
    if std::env::args().any(|s| s == "--table-demo") && cx.global::<Registry>().0.len() == 1 {
        let _ = handle.update(cx, |_, w, cx| {
            view.update(cx, |s, cx| table_demo::seed(s, w, cx))
        });
    }
    #[cfg(feature = "validation")]
    if std::env::args().any(|s| s == "--media-smoke") {
        media_smoke::run(handle, view.clone(), cx);
    }
    #[cfg(feature = "validation")]
    if std::env::args().any(|s| s == "--scene-smoke") {
        scene_canvas::smoke(handle, view.clone(), cx);
    }
    #[cfg(feature = "validation")]
    if std::env::args().any(|s| s == "--ime-smoke" || s == "--table-ime-smoke") {
        validation::ime_smoke(handle, view.clone(), cx);
    }
    #[cfg(feature = "validation")]
    if std::env::args().any(|s| s == "--text-smoke") {
        validation::text_smoke(handle, view.clone(), cx);
    }
    #[cfg(feature = "validation")]
    if std::env::args().any(|s| s == "--editor-smoke") && cx.global::<Registry>().0.len() == 1 {
        validation::smoke(handle, view, cx);
    }
    Ok(())
}
#[derive(Default)]
struct Registry(Vec<(AnyWindowHandle, WeakEntity<Editor>)>);
impl Global for Registry {}
pub fn quit(cx: &mut App) {
    let entries = cx
        .try_global::<Registry>()
        .map(|r| r.0.clone())
        .unwrap_or_default();
    for (handle, weak) in entries {
        let dirty = weak
            .read_with(cx, |s, _| s.session.dirty() || s.saving)
            .unwrap_or(false);
        if dirty {
            let _ = handle.update(cx, |_, w, cx| {
                w.activate_window();
                let _ = weak.update(cx, |s, cx| s.close(&Close, w, cx));
            });
            return;
        }
    }
    cx.quit();
}

#[cfg(feature = "validation")]
pub fn open_benchmark(cx: &mut App) {
    render_bench::open(cx);
}
