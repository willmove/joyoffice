//! Independent transactional editing kernel; no UI types.
use crate::document::{Deck, MAX_PART, Object, Result, Slide};
use std::{
    collections::VecDeque,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
static IDS: AtomicU64 = AtomicU64::new(1);
#[derive(Clone)]
pub enum Command {
    Playback(crate::playback::Playback),
    Review(crate::review::Action),
    Motion(crate::motion::Presentation),
    AddPage,
    Table(usize, String, crate::table::Action),
    Design(crate::design_commands::Action),
    Paste(usize, crate::media::Transfer),
    ImportImage(usize, Option<usize>, crate::media::Resource),
    Batch(Vec<Command>),
    ReplaceMany(usize, Vec<Object>),
    ReorderPages(Vec<usize>, usize),
    DeletePages(Vec<usize>),
    DuplicatePages(Vec<usize>),
    HidePages(Vec<usize>),
    PageSize(f32, f32),
    ReorderObjects(usize, Vec<usize>, usize),
    DuplicatePage(usize),
    DeletePage(usize),
    AddObject(usize, Object),
    AdoptObject(usize, Object),
    ReplaceObject(usize, usize, Object),
    DeleteObject(usize, usize),
}
pub struct CommandSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub undoable: bool,
}
pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        id: "media.edit",
        label: "媒体参数",
        undoable: true,
    },
    CommandSpec {
        id: "review.edit",
        label: "审阅与文稿无障碍",
        undoable: true,
    },
    CommandSpec {
        id: "presentation.motion",
        label: "动画与放映设置",
        undoable: true,
    },
    CommandSpec {
        id: "table.edit",
        label: "编辑表格",
        undoable: true,
    },
    CommandSpec {
        id: "document.design",
        label: "文稿设计与组织",
        undoable: true,
    },
    CommandSpec {
        id: "page.add",
        label: "新增页",
        undoable: true,
    },
    CommandSpec {
        id: "page.copy",
        label: "复制页",
        undoable: true,
    },
    CommandSpec {
        id: "page.delete",
        label: "删除页",
        undoable: true,
    },
    CommandSpec {
        id: "page.reorder",
        label: "重排页面",
        undoable: true,
    },
    CommandSpec {
        id: "page.visibility",
        label: "隐藏页面",
        undoable: true,
    },
    CommandSpec {
        id: "page.size",
        label: "页面尺寸",
        undoable: true,
    },
    CommandSpec {
        id: "object.add",
        label: "创建对象",
        undoable: true,
    },
    CommandSpec {
        id: "object.update",
        label: "修改对象",
        undoable: true,
    },
    CommandSpec {
        id: "object.delete",
        label: "删除对象",
        undoable: true,
    },
    CommandSpec {
        id: "object.reorder",
        label: "调整图层",
        undoable: true,
    },
    CommandSpec {
        id: "transaction.batch",
        label: "复合事务",
        undoable: true,
    },
];
impl Command {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Table(..) => "table.edit",
            Self::Design(..) => "document.design",
            Self::Paste(..) => "object.add",
            Self::ImportImage(..) => "object.add",
            Self::Playback(..) => "media.edit",
            Self::Review(..) => "review.edit",
            Self::Motion(..) => "presentation.motion",
            Self::AddPage => "page.add",
            Self::DuplicatePage(_) | Self::DuplicatePages(_) => "page.copy",
            Self::DeletePage(_) | Self::DeletePages(_) => "page.delete",
            Self::ReorderPages(..) => "page.reorder",
            Self::HidePages(_) => "page.visibility",
            Self::PageSize(..) => "page.size",
            Self::AddObject(..) | Self::AdoptObject(..) => "object.add",
            Self::ReplaceObject(..) | Self::ReplaceMany(..) => "object.update",
            Self::DeleteObject(..) => "object.delete",
            Self::ReorderObjects(..) => "object.reorder",
            Self::Batch(_) => "transaction.batch",
        }
    }
}
struct Snapshot {
    deck: Arc<Deck>,
    bytes: usize,
}
pub struct Session {
    pub id: u64,
    pub deck: Arc<Deck>,
    pub revision: u64,
    pub saved: Option<Arc<Deck>>,
    undo: VecDeque<Snapshot>,
    redo: Vec<Snapshot>,
    bytes: usize,
    next_id: u64,
}
impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
impl Session {
    pub fn new() -> Self {
        Self::from_deck(
            Deck {
                playback: Default::default(),
                review: Default::default(),
                presentation: Default::default(),
                source: None,
                resources: Default::default(),
                design: Default::default(),
                organization: Default::default(),
                field_date: String::new(),
                title: "未命名文稿".into(),
                width: 960.,
                height: 540.,
                slides: vec![Slide {
                    hidden: false,
                    layout: None,
                    inherit_background: false,
                    notes: String::new(),
                    section: None,
                    id: "page-1".into(),
                    title: "第1页".into(),
                    background: "#ffffff".into(),
                    objects: vec![],
                }],
                warnings: vec![],
            },
            true,
        )
    }
    pub fn from_deck(mut deck: Deck, clean: bool) -> Self {
        normalize_ids(&mut deck);
        let bytes = serde_json::to_vec(&deck).map_or(MAX_PART, |b| b.len());
        let deck = Arc::new(deck);
        Self {
            id: IDS.fetch_add(1, Ordering::Relaxed),
            saved: clean.then(|| deck.clone()),
            deck,
            revision: 0,
            undo: VecDeque::new(),
            redo: vec![],
            next_id: 1,
            bytes,
        }
    }
    pub fn dirty(&self) -> bool {
        self.saved
            .as_ref()
            .is_none_or(|s| !Arc::ptr_eq(s, &self.deck))
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn mark_saved(&mut self, snapshot: Arc<Deck>) {
        self.saved = Some(snapshot);
    }
    pub fn enabled(&self, command: &Command) -> Result<()> {
        let mut draft = (*self.deck).clone();
        let mut next = self.next_id;
        apply(&mut draft, command.clone(), &mut next, 0)?;
        crate::motion::reconcile(&mut draft);
        crate::review::reconcile(&mut draft);
        crate::playback::reconcile(&mut draft);
        draft.validate()
    }
    pub fn execute(&mut self, target: u64, command: Command) -> Result<()> {
        if target != self.id {
            return Err("文档身份已改变".into());
        }
        let mut d = (*self.deck).clone();
        let mut next_id = self.next_id;
        apply(&mut d, command, &mut next_id, 0)?;
        crate::motion::reconcile(&mut d);
        crate::review::reconcile(&mut d);
        crate::playback::reconcile(&mut d);
        crate::media::collect_unused(&mut d);
        d.validate()?;
        if d == *self.deck {
            return Ok(());
        }
        let bytes = serde_json::to_vec(&d).map_err(|e| e.to_string())?.len();
        if bytes > MAX_PART {
            return Err("模型超过8 MiB预算".into());
        }
        self.undo.push_back(Snapshot {
            deck: self.deck.clone(),
            bytes: self.bytes,
        });
        self.next_id = next_id;
        self.deck = Arc::new(d);
        self.bytes = bytes;
        self.redo.clear();
        self.limit_history();
        self.revision += 1;
        Ok(())
    }
    fn limit_history(&mut self) {
        while self.undo.len() + self.redo.len() > 100
            || self
                .undo
                .iter()
                .chain(self.redo.iter())
                .map(|s| s.bytes)
                .sum::<usize>()
                > 32 * 1024 * 1024
        {
            if self.undo.pop_front().is_none() {
                if self.redo.is_empty() {
                    break;
                }
                self.redo.remove(0);
            }
        }
    }
    pub fn undo(&mut self) {
        if let Some(d) = self.undo.pop_back() {
            self.redo.push(Snapshot {
                deck: self.deck.clone(),
                bytes: self.bytes,
            });
            self.deck = d.deck;
            self.bytes = d.bytes;
            self.revision += 1;
            self.limit_history();
        }
    }
    pub fn redo(&mut self) {
        if let Some(d) = self.redo.pop() {
            self.undo.push_back(Snapshot {
                deck: self.deck.clone(),
                bytes: self.bytes,
            });
            self.deck = d.deck;
            self.bytes = d.bytes;
            self.revision += 1;
            self.limit_history();
        }
    }
}

fn normalize_ids(d: &mut Deck) {
    let mut ids = std::collections::BTreeSet::new();
    for s in &d.slides {
        for o in &s.objects {
            if !o.id.is_empty() {
                ids.insert(o.id.clone());
            }
        }
    }
    for s in &mut d.slides {
        for o in &mut s.objects {
            if o.id.is_empty() {
                loop {
                    let id = format!("object-{}", IDS.fetch_add(1, Ordering::Relaxed));
                    if ids.insert(id.clone()) {
                        o.id = id;
                        break;
                    }
                }
            }
        }
    }
}
fn indices(ids: &[usize], len: usize) -> Result<Vec<usize>> {
    let mut out = ids.to_vec();
    out.sort_unstable();
    if out.is_empty()
        || out.last().is_some_and(|i| *i >= len)
        || out.windows(2).any(|w| w[0] == w[1])
    {
        return Err("选择为空、重复或已失效".into());
    }
    Ok(out)
}
fn reorder<T: Clone>(items: &mut Vec<T>, ids: Vec<usize>, target: usize) -> Result<()> {
    let ids = indices(&ids, items.len())?;
    if target > items.len() {
        return Err("重排位置超限".into());
    }
    let moved: Vec<_> = ids.iter().map(|i| items[*i].clone()).collect();
    let insert = target - ids.iter().filter(|i| **i < target).count();
    for i in ids.iter().rev() {
        items.remove(*i);
    }
    items.splice(insert..insert, moved);
    Ok(())
}
fn apply(d: &mut Deck, c: Command, next: &mut u64, depth: usize) -> Result<()> {
    if depth > 16 {
        return Err("复合命令嵌套超限".into());
    }
    match c {
        Command::Playback(p) => {
            p.validate(d)?;
            d.playback = p;
        }
        Command::Review(a) => crate::review::apply(d, a)?,
        Command::Motion(value) => {
            value.validate(d)?;
            d.presentation = value;
        }
        Command::Table(page, id, action) => {
            // Resolve inherited cell formatting before merging so rich text keeps its source style.
            if let crate::table::Action::Merge(range) = &action {
                let object = d
                    .slides
                    .get(page)
                    .and_then(|s| s.objects.iter().find(|o| o.id == id))
                    .ok_or("表格已不存在")?;
                let table = object.table.as_ref().ok_or("对象不是表格")?;
                let mut fills = vec![];
                // Bounds are checked before iteration to keep hostile ranges bounded.
                table.index(range.top, range.left)?;
                table.index(range.bottom, range.right)?;
                for r in range.top..=range.bottom {
                    for c in range.left..=range.right {
                        let index = table.index(r, c)?;
                        let cell = &table.cells[index];
                        if !cell.text.is_empty() && cell.runs.is_empty() {
                            fills.push((
                                index,
                                crate::text::Run {
                                    start: 0,
                                    end: cell.text.len(),
                                    style: crate::table::resolve(d, page, table, r, c).text,
                                },
                            ));
                        }
                    }
                }
                let table = d.slides[page]
                    .objects
                    .iter_mut()
                    .find(|o| o.id == id)
                    .unwrap()
                    .table
                    .as_mut()
                    .unwrap();
                for (i, run) in fills {
                    table.cells[i].runs = vec![run];
                }
            }
            let o = d
                .slides
                .get_mut(page)
                .and_then(|s| s.objects.iter_mut().find(|o| o.id == id))
                .ok_or("表格已不存在")?;
            let t = o.table.as_mut().ok_or("对象不是表格")?;
            t.apply(action)?;
            o.width = t.columns.iter().sum();
            o.height = t.rows.iter().sum();
        }
        Command::Batch(list) => {
            if list.len() > 1000 {
                return Err("复合命令数量超限".into());
            }
            for c in list {
                apply(d, c, next, depth + 1)?;
            }
        }
        Command::Design(action) => {
            crate::design_commands::apply(d, action, next)?;
            normalize_ids(d);
        }
        Command::Paste(p, transfer) => {
            transfer.validate()?;
            for (id, r) in transfer.resources {
                d.resources.entry(id).or_insert(r);
            }
            for mut o in transfer.objects {
                if o.morph_key.as_ref().is_some_and(|key| {
                    d.slides[p]
                        .objects
                        .iter()
                        .any(|o| o.morph_key.as_ref() == Some(key))
                }) {
                    o.morph_key = None;
                }
                o.x += 16.;
                o.y += 16.;
                apply(d, Command::AddObject(p, o), next, depth + 1)?;
            }
        }
        Command::ImportImage(p, replace, resource) => {
            resource.validate()?;
            let id = resource.id();
            let mut o = if let Some(i) = replace {
                d.slides
                    .get(p)
                    .and_then(|s| s.objects.get(i))
                    .filter(|o| o.kind == "image")
                    .ok_or("请选择图片")?
                    .clone()
            } else {
                let mut o = object("image");
                let scale = (480. / resource.width as f32)
                    .min(300. / resource.height as f32)
                    .min(1.);
                o.width = resource.width as f32 * scale;
                o.height = resource.height as f32 * scale;
                o
            };
            o.image = Some(crate::media::ImageRef {
                resource: id.clone(),
                crop: [0., 0., 1., 1.],
            });
            d.resources.entry(id).or_insert_with(|| Arc::new(resource));
            apply(
                d,
                if let Some(i) = replace {
                    Command::ReplaceObject(p, i, o)
                } else {
                    Command::AddObject(p, o)
                },
                next,
                depth + 1,
            )?;
        }
        Command::AddPage => {
            let mut page = d.slides[0].clone();
            page.objects.clear();
            page.notes.clear();
            page.section = None;
            page.hidden = false;
            *next = next.checked_add(1).ok_or("ID耗尽")?;
            while d.slides.iter().any(|s| s.id == format!("page-{next}")) {
                *next = next.checked_add(1).ok_or("ID耗尽")?;
            }
            page.id = format!("page-{next}");
            page.title = format!("第{}页", d.slides.len() + 1);
            d.slides.push(page);
        }
        Command::DuplicatePage(i) => apply(d, Command::DuplicatePages(vec![i]), next, depth + 1)?,
        Command::DeletePage(i) => apply(d, Command::DeletePages(vec![i]), next, depth + 1)?,
        Command::DuplicatePages(ids) => {
            let mut copies = vec![];
            for i in indices(&ids, d.slides.len())? {
                let mut s = d.slides[i].clone();
                *next = next.checked_add(1).ok_or("ID耗尽")?;
                while d.slides.iter().any(|s| s.id == format!("page-{next}")) {
                    *next = next.checked_add(1).ok_or("ID耗尽")?;
                }
                s.id = format!("page-{next}");
                for o in &mut s.objects {
                    o.id.clear();
                }
                copies.push((i, d.slides.len()));
                d.slides.push(s);
            }
            normalize_ids(d);
            for (old, new) in copies {
                let source = &d.slides[old];
                let target = &d.slides[new];
                if let Some(mut t) = d.presentation.slides.get(&source.id).cloned() {
                    for e in &mut t.effects {
                        if let Some(i) = source.objects.iter().position(|o| o.id == e.target) {
                            e.target = target.objects[i].id.clone();
                        }
                    }
                    d.presentation.slides.insert(target.id.clone(), t);
                }
                for (a, b) in source.objects.iter().zip(&target.objects) {
                    if let Some(dest) = d.presentation.links.get(&a.id).cloned() {
                        let dest = if dest == source.id {
                            target.id.clone()
                        } else {
                            dest
                        };
                        d.presentation.links.insert(b.id.clone(), dest);
                    }
                }
            }
        }
        Command::DeletePages(ids) => {
            let ids = indices(&ids, d.slides.len())?;
            if ids.len() >= d.slides.len() {
                return Err("至少保留一页".into());
            }
            for i in ids.iter().rev() {
                d.slides.remove(*i);
            }
        }
        Command::HidePages(ids) => {
            for i in indices(&ids, d.slides.len())? {
                d.slides[i].hidden = !d.slides[i].hidden;
            }
        }
        Command::ReorderPages(ids, target) => reorder(&mut d.slides, ids, target)?,
        Command::PageSize(w, h) => {
            d.width = w;
            d.height = h;
        }
        Command::AddObject(p, mut o) => {
            o.id.clear();
            d.slides.get_mut(p).ok_or("页面不存在")?.objects.push(o);
            normalize_ids(d);
        }
        Command::AdoptObject(p, o) => {
            if o.id.is_empty()
                || d.slides
                    .iter()
                    .flat_map(|s| &s.objects)
                    .any(|v| v.id == o.id)
            {
                return Err("参考对象ID为空或已在目标文稿使用".into());
            }
            d.slides.get_mut(p).ok_or("页面不存在")?.objects.push(o);
        }
        Command::ReplaceObject(p, i, mut o) => {
            crate::table::resize(&mut o);
            crate::design_commands::prepare_update(d, p, &mut o)?;
            let old = d
                .slides
                .get_mut(p)
                .and_then(|s| s.objects.get_mut(i))
                .ok_or("对象不存在")?;
            o.id = old.id.clone();
            *old = o;
        }
        Command::ReplaceMany(p, mut objects) => {
            for o in &mut objects {
                crate::table::resize(o);
                crate::design_commands::prepare_update(d, p, o)?;
            }
            let slide = d.slides.get_mut(p).ok_or("页面不存在")?;
            let mut seen = std::collections::BTreeSet::new();
            for o in objects {
                if o.id.is_empty() || !seen.insert(o.id.clone()) {
                    return Err("无效对象ID".into());
                }
                let old = slide
                    .objects
                    .iter_mut()
                    .find(|v| v.id == o.id)
                    .ok_or("对象已不存在")?;
                *old = o;
            }
        }
        Command::DeleteObject(p, i) => {
            let s = d.slides.get_mut(p).ok_or("页面不存在")?;
            if i >= s.objects.len() {
                return Err("对象不存在".into());
            }
            s.objects.remove(i);
        }
        Command::ReorderObjects(p, ids, target) => reorder(
            &mut d.slides.get_mut(p).ok_or("页面不存在")?.objects,
            ids,
            target,
        )?,
    }
    Ok(())
}
pub fn object(kind: &str) -> Object {
    Object {
        text_align: "left".into(),
        morph_key: None,
        chart: None,
        diagram: None,
        table: None,
        image: None,
        binding: None,
        field: None,
        id: String::new(),
        rotation: 0.,
        opacity: 1.,
        stroke: "#202b3a".into(),
        stroke_width: 0.,
        kind: kind.into(),
        x: 80.,
        y: 80.,
        width: 240.,
        height: 100.,
        fill: "#27b6a7".into(),
        text: if kind == "text" {
            "输入文字".into()
        } else {
            String::new()
        },
        font_size: 24.,
        runs: vec![],
    }
}
/// Same-directory temporary file; replacement is the commit point.
pub fn save(path: &Path, deck: &Deck) -> Result<()> {
    save_impl(path, deck, true)
}
pub fn save_new(path: &Path, deck: &Deck) -> Result<()> {
    save_impl(path, deck, false)
}
fn save_impl(path: &Path, deck: &Deck, replace: bool) -> Result<()> {
    deck.validate()?;
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("joydeck"))
    {
        return Err("只能保存为 .joydeck".into());
    }
    let mut model = deck.clone();
    model.resources.clear();
    model.source = None;
    for asset in model.playback.assets.values_mut() {
        Arc::make_mut(asset).bytes.clear();
    }
    let bytes = serde_json::to_vec(&model).map_err(|e| e.to_string())?;
    let manifest = serde_json::to_vec(&serde_json::json!({"format":"joydeck","schemaMajor":1,"schemaMinor":10,"minReader":11,"requiredFeatures":["object-transform-v1","rich-text-v1","image-resource-v1","design-v1","pptx-source-v1","table-v1","charts-v1","motion-v1","review-v1","media-v1"],"document":"document.json","source":deck.source.is_some(),"media":deck.playback.assets.keys().collect::<Vec<_>>(),"resources":deck.resources.keys().map(|id| format!("resources/{id}.png")).collect::<Vec<_>>()})).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_PART {
        return Err("模型超过读取预算".into());
    }
    let temp = path.with_file_name(format!(
        ".joydeck-{}-{}.tmp",
        std::process::id(),
        IDS.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        let mut z = zip::ZipWriter::new(f);
        let opts =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in [
            (
                "mimetype",
                b"application/vnd.joydeck.presentation+zip".as_slice(),
            ),
            ("manifest.json", manifest.as_slice()),
            ("document.json", bytes.as_slice()),
        ] {
            z.start_file(name, opts).map_err(|e| e.to_string())?;
            z.write_all(data).map_err(|e| e.to_string())?;
        }
        for (id, resource) in &deck.resources {
            z.start_file(format!("resources/{id}.png"), opts)
                .map_err(|e| e.to_string())?;
            z.write_all(&resource.png).map_err(|e| e.to_string())?;
        }
        for (id, asset) in &deck.playback.assets {
            z.start_file(format!("media/{id}.bin"), opts)
                .map_err(|e| e.to_string())?;
            z.write_all(&asset.bytes).map_err(|e| e.to_string())?;
        }
        if let Some(source) = &deck.source {
            let mut meta = (**source).clone();
            meta.parts.clear();
            z.start_file("source/metadata.json", opts)
                .map_err(|e| e.to_string())?;
            z.write_all(&serde_json::to_vec(&meta).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            for (name, data) in &source.parts {
                z.start_file(format!("source/package/{name}"), opts)
                    .map_err(|e| e.to_string())?;
                z.write_all(data).map_err(|e| e.to_string())?;
            }
        }
        let mut f = z.finish().map_err(|e| e.to_string())?;
        f.flush().map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        drop(f);
        if replace {
            std::fs::rename(&temp, path).map_err(|e| e.to_string())?;
        } else {
            std::fs::hard_link(&temp, path).map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(&temp);
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}
pub fn open_editable(path: &Path) -> Result<Deck> {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let parts = crate::document::read_package(path, &cancel)?;
    let manifest: serde_json::Value =
        serde_json::from_str(crate::document::part(&parts, "manifest.json")?)
            .map_err(|e| e.to_string())?;
    if !matches!(manifest["schemaMinor"].as_u64(), Some(0..=10)) {
        return Err("新版本格式只能只读查看".into());
    }
    let empty = vec![];
    let resources = if manifest.get("resources").is_none() {
        &empty
    } else {
        manifest["resources"].as_array().ok_or("无效资源索引")?
    };
    if parts.len()
        != 3 + resources.len()
            + parts.keys().filter(|n| n.starts_with("media/")).count()
            + if manifest["source"] == true {
                1 + parts
                    .keys()
                    .filter(|n| n.starts_with("source/package/"))
                    .count()
            } else {
                0
            }
    {
        return Err("未知部件：请使用只读查看器打开".into());
    }
    let raw: serde_json::Value =
        serde_json::from_str(crate::document::part(&parts, "document.json")?)
            .map_err(|e| e.to_string())?;
    fn keys(v: &serde_json::Value, allowed: &[&str]) -> Result<()> {
        if v.as_object()
            .ok_or("无效对象")?
            .keys()
            .any(|k| !allowed.contains(&k.as_str()))
        {
            Err("未知字段：请使用只读查看器打开".into())
        } else {
            Ok(())
        }
    }
    keys(
        &raw,
        &[
            "title",
            "width",
            "height",
            "slides",
            "design",
            "organization",
            "fieldDate",
            "presentation",
            "review",
            "playback",
        ],
    )?;
    for s in raw["slides"].as_array().ok_or("缺少页面")? {
        keys(
            s,
            &[
                "id",
                "title",
                "background",
                "objects",
                "hidden",
                "layout",
                "inherit_background",
                "notes",
                "section",
            ],
        )?;
    }
    crate::document::open(path, &cancel)
}
pub fn recovery_path() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state")))
    };
    base.unwrap_or_else(std::env::temp_dir)
        .join("JoyDeck/recovery")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rollback_identity_and_history() {
        let mut s = Session::new();
        let before = s.deck.clone();
        let mut o = object("rect");
        o.width = f32::NAN;
        assert!(s.execute(s.id, Command::AddObject(0, o)).is_err());
        assert!(Arc::ptr_eq(&before, &s.deck));
        assert!(s.execute(s.id + 1, Command::AddPage).is_err());
        s.execute(s.id, Command::AddPage).unwrap();
        s.undo();
        assert!(Arc::ptr_eq(&before, &s.deck));
        s.redo();
        assert_eq!(s.deck.slides.len(), 2);
        assert!(s.execute(s.id, Command::DeletePage(10)).is_err());
    }
    #[test]
    fn save_reopen_and_stale_save() {
        let mut s = Session::new();
        let path = std::env::temp_dir().join(format!("joydeck-m2-{}.joydeck", std::process::id()));
        save(&path, &s.deck).unwrap();
        let d = open_editable(&path).unwrap();
        assert_eq!(d.slides.len(), 1);
        let old = s.deck.clone();
        s.execute(s.id, Command::AddPage).unwrap();
        s.mark_saved(old);
        assert!(s.dirty());
        save(&path, &s.deck).unwrap();
        assert_eq!(open_editable(&path).unwrap().slides.len(), 2);
        let bytes = std::fs::read(&path).unwrap();
        let mut invalid = (*s.deck).clone();
        invalid.width = f32::NAN;
        assert!(save(&path, &invalid).is_err());
        assert_eq!(bytes, std::fs::read(&path).unwrap());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn save_new_never_replaces_existing_target() {
        let s = Session::new();
        let path =
            std::env::temp_dir().join(format!("joydeck-no-clobber-{}.joydeck", std::process::id()));
        std::fs::write(&path, b"keep existing contents").unwrap();
        assert!(save_new(&path, &s.deck).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"keep existing contents");
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn history_cap_and_new_branch_clear_redo() {
        let mut s = Session::new();
        for _ in 0..120 {
            s.execute(s.id, Command::AddObject(0, object("rect")))
                .unwrap();
        }
        assert!(s.undo.len() <= 100);
        s.undo();
        assert!(s.can_redo());
        s.execute(s.id, Command::AddPage).unwrap();
        assert!(!s.can_redo());
        let mut empty = Session::new();
        assert!(empty.execute(empty.id, Command::DeletePage(0)).is_err());
        assert!(!empty.dirty());
    }
    #[test]
    fn versioned_style_and_ids_roundtrip() {
        let mut s = Session::new();
        let mut o = object("arrow");
        o.rotation = 37.;
        o.opacity = 0.4;
        o.stroke_width = 3.;
        s.execute(s.id, Command::AddObject(0, o)).unwrap();
        let id = s.deck.slides[0].objects[0].id.clone();
        s.execute(s.id, Command::DuplicatePage(0)).unwrap();
        assert_ne!(s.deck.slides[1].objects[0].id, id);
        assert!(s.enabled(&Command::DeletePages(vec![0, 1])).is_err());
        let path =
            std::env::temp_dir().join(format!("joydeck-style-{}.joydeck", std::process::id()));
        save(&path, &s.deck).unwrap();
        let loaded = open_editable(&path).unwrap();
        assert_eq!(&*s.deck, &loaded);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn legacy_upgrade_and_id_overflow_are_checked() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/native-20.joydeck");
        let legacy = open_editable(&path).unwrap();
        let s = Session::from_deck(legacy, true);
        let ids = s
            .deck
            .slides
            .iter()
            .flat_map(|s| s.objects.iter().map(|o| o.id.as_str()))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(), 120);
        assert!(!ids.contains(""));
        let mut s = Session::new();
        s.next_id = u64::MAX;
        let old = s.deck.clone();
        assert!(s.execute(s.id, Command::AddPage).is_err());
        assert!(Arc::ptr_eq(&old, &s.deck));
    }
    #[test]
    fn history_budget_includes_redo_and_large_to_small_edits() {
        let mut s = Session::new();
        s.execute(s.id, Command::AddObject(0, object("text")))
            .unwrap();
        for i in 0..24 {
            let mut o = s.deck.slides[0].objects[0].clone();
            o.text = if i % 2 == 0 {
                "x".repeat(3 * 1024 * 1024)
            } else {
                "small".into()
            };
            s.execute(s.id, Command::ReplaceObject(0, 0, o)).unwrap();
            assert!(
                s.undo
                    .iter()
                    .chain(s.redo.iter())
                    .map(|v| v.bytes)
                    .sum::<usize>()
                    <= 32 * 1024 * 1024
            );
        }
        for _ in 0..24 {
            s.undo();
            assert!(
                s.undo
                    .iter()
                    .chain(s.redo.iter())
                    .map(|v| v.bytes)
                    .sum::<usize>()
                    <= 32 * 1024 * 1024
            );
        }
        for _ in 0..24 {
            s.redo();
            assert!(
                s.undo
                    .iter()
                    .chain(s.redo.iter())
                    .map(|v| v.bytes)
                    .sum::<usize>()
                    <= 32 * 1024 * 1024
            );
        }
    }
}
