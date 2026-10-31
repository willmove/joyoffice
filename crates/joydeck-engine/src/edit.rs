//! Independent transactional editing kernel; no UI types.
use crate::document::{Deck, MAX_PART, Object, Result, Slide};
pub use joydeck_model::identity::object;
use joydeck_model::identity::{IDS, normalize_ids};
use std::{
    collections::VecDeque,
    sync::{Arc, atomic::Ordering},
};
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
    /// Supply content-addressed dependencies as part of a containing edit transaction.
    MergeResources(std::collections::BTreeMap<String, Arc<crate::media::Resource>>),
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
            Self::ImportImage(..) | Self::MergeResources(..) => "object.add",
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
        self.prepare(command.clone()).map(|_| ())
    }
    fn prepare(&self, command: Command) -> Result<(Deck, u64, usize)> {
        let mut draft = (*self.deck).clone();
        let mut next = self.next_id;
        apply(&mut draft, command, &mut next, 0, self.id)?;
        crate::motion::reconcile(&mut draft);
        crate::review::reconcile(&mut draft);
        crate::playback::reconcile(&mut draft);
        crate::media::collect_unused(&mut draft);
        draft.validate()?;
        let bytes = serde_json::to_vec(&draft).map_err(|e| e.to_string())?.len();
        if bytes > MAX_PART {
            return Err("模型超过8 MiB预算".into());
        }
        Ok((draft, next, bytes))
    }
    pub fn execute(&mut self, target: u64, command: Command) -> Result<()> {
        if target != self.id {
            return Err("文档身份已改变".into());
        }
        let (d, next_id, bytes) = self.prepare(command)?;
        if d == *self.deck {
            return Ok(());
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
/// Draft validation and commit must allocate the same IDs without consuming a
/// process-global counter. The session publishes this cursor only on commit.
fn normalize_edit_ids(d: &mut Deck, next: &mut u64, namespace: u64) -> Result<()> {
    let mut used = d
        .slides
        .iter()
        .flat_map(|s| &s.objects)
        .filter(|o| !o.id.is_empty())
        .map(|o| o.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    for o in d
        .slides
        .iter_mut()
        .flat_map(|s| &mut s.objects)
        .filter(|o| o.id.is_empty())
    {
        loop {
            *next = next.checked_add(1).ok_or("ID耗尽")?;
            let id = format!("object-{namespace}-{next}");
            if used.insert(id.clone()) {
                o.id = id;
                break;
            }
        }
    }
    Ok(())
}
fn apply(d: &mut Deck, c: Command, next: &mut u64, depth: usize, namespace: u64) -> Result<()> {
    if depth > 16 {
        return Err("复合命令嵌套超限".into());
    }
    match c {
        Command::MergeResources(resources) => {
            for (id, resource) in resources {
                if let Some(old) = d.resources.get(&id) {
                    if old != &resource {
                        return Err("图片资源身份冲突".into());
                    }
                } else {
                    d.resources.insert(id, resource);
                }
            }
        }
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
                    .ok_or("表格对象已不存在")?
                    .table
                    .as_mut()
                    .ok_or("对象不是表格")?;
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
                apply(d, c, next, depth + 1, namespace)?;
            }
        }
        Command::Design(action) => {
            crate::design_commands::apply(d, action, next)?;
            normalize_edit_ids(d, next, namespace)?;
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
                apply(d, Command::AddObject(p, o), next, depth + 1, namespace)?;
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
                namespace,
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
        Command::DuplicatePage(i) => apply(
            d,
            Command::DuplicatePages(vec![i]),
            next,
            depth + 1,
            namespace,
        )?,
        Command::DeletePage(i) => {
            apply(d, Command::DeletePages(vec![i]), next, depth + 1, namespace)?
        }
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
            normalize_edit_ids(d, next, namespace)?;
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
            normalize_edit_ids(d, next, namespace)?;
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

#[cfg(test)]
#[path = "edit/tests.rs"]
mod tests;
