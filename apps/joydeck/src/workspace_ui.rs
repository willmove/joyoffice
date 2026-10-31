use super::*;
use joydeck::{
    design,
    design_commands::{self, Action, PageTransfer},
};
impl Editor {
    pub(super) fn effective_objects(&self) -> Vec<Object> {
        design::resolve_slide(&self.session.deck, self.page)
            .map(|mut s| {
                for o in &mut s.objects {
                    if let Some(raw) = self.session.deck.slides[self.page]
                        .objects
                        .iter()
                        .find(|raw| raw.id == o.id)
                    {
                        o.runs = raw.runs.clone();
                    }
                }
                s.objects
            })
            .unwrap_or_else(|_| self.session.deck.slides[self.page].objects.clone())
    }
    pub(super) fn sync_document(&mut self, cx: &mut Context<Self>) -> bool {
        let page = self
            .session
            .deck
            .slides
            .get(self.page)
            .map(|s| s.id.clone());
        let changed = self.session.sync();
        self.disk = self.session.file();
        if changed {
            self.text_edit = None;
            self.cell_edit = None;
            self.table_pick = None;
            self.table_resize = None;
            self.text_anchor = None;
            self.gesture = None;
            self.page_drag = None;
            if let Some(index) =
                page.and_then(|id| self.session.deck.slides.iter().position(|s| s.id == id))
            {
                self.page = index;
            }
            self.changed(cx);
            if !self.session.dirty() {
                let recovery = self.recovery.clone();
                let revision = self.session.revision;
                cx.background_executor()
                    .spawn(async move {
                        let _ = recovery.clean(revision);
                    })
                    .detach();
            }
            self.message = "已同步共享文稿；当前窗口选择独立保留，未提交的编辑草稿已取消".into();
        }
        changed
    }
    pub(super) fn action(&mut self, a: Action, cx: &mut Context<Self>) {
        self.command(Command::Design(a), cx);
    }
    pub(super) fn sync_doc_fields(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let d = &self.session.deck;
        let s = &d.slides[self.page];
        self.doc_page = s.id.clone();
        self.notes_base = s.notes.clone();
        self.notes_input
            .update(cx, |f, cx| f.set_value(s.notes.clone(), w, cx));
        let section = s
            .section
            .as_ref()
            .and_then(|id| d.organization.sections.get(id))
            .cloned()
            .unwrap_or_default();
        for (i, value) in [
            (0, d.title.clone()),
            (1, d.organization.author.clone()),
            (2, d.organization.subject.clone()),
            (3, s.notes.clone()),
            (4, section),
            (9, d.field_date.clone()),
        ] {
            self.doc_fields[i].update(cx, |f, cx| f.set_value(value, w, cx));
        }
    }
    pub(super) fn design_op(&mut self, op: &str, cx: &mut Context<Self>) {
        let d = &self.session.deck;
        let mut design = d.design.clone();
        let get = |i: usize| self.doc_fields[i].read(cx).value().to_string();
        let chosen = if design.layouts.contains_key(&self.chosen_layout) {
            self.chosen_layout.clone()
        } else {
            design.layouts.keys().next().cloned().unwrap_or_default()
        };
        let name = get(8);
        match op {
            "starter" => {
                if !design.layouts.is_empty() {
                    self.message = "已有设计，请使用复制/编辑操作".into();
                    cx.notify();
                    return;
                }
                design = design::starter();
            }
            "apply" | "reset" => {
                self.action(Action::ApplyLayout(self.page, chosen, op == "reset"), cx);
                return;
            }
            "delete-layout" => {
                self.action(Action::DeleteLayout(chosen, None), cx);
                return;
            }
            "copy-layout" => {
                if let Some(l) = design.layouts.get(&chosen).cloned() {
                    let id = unique("layout", design.layouts.keys());
                    let mut l = l;
                    l.name = name;
                    design.layouts.insert(id.clone(), l);
                    self.chosen_layout = id;
                }
            }
            "rename-layout" => {
                if let Some(l) = design.layouts.get_mut(&chosen) {
                    l.name = name;
                }
            }
            "copy-master" => {
                if let Some(l) = design.layouts.get(&chosen).cloned() {
                    let mut m = design.masters[&l.master].clone();
                    let theme = design.themes[&m.theme].clone();
                    let tid = unique("theme", design.themes.keys());
                    let mid = unique("master", design.masters.keys());
                    let lid = unique("layout", design.layouts.keys());
                    m.theme = tid.clone();
                    m.name = name.clone();
                    let mut l = l;
                    l.master = mid.clone();
                    l.name = name;
                    design.themes.insert(tid, theme);
                    design.masters.insert(mid, m);
                    design.layouts.insert(lid.clone(), l);
                    self.chosen_layout = lid;
                }
            }
            "rename-master" => {
                if let Some(l) = design.layouts.get(&chosen)
                    && let Some(m) = design.masters.get_mut(&l.master)
                {
                    m.name = name;
                }
            }
            "delete-master" => {
                if let Some(l) = design.layouts.get(&chosen) {
                    design.masters.remove(&l.master);
                }
            }
            "theme" | "master-title" => {
                if let Some(l) = design.layouts.get(&chosen) {
                    let Some(m) = design.masters.get_mut(&l.master) else {
                        self.message = "版式引用的母版不存在".into();
                        cx.notify();
                        return;
                    };
                    if op == "theme" {
                        let Some(t) = design.themes.get_mut(&m.theme) else {
                            self.message = "母版引用的主题不存在".into();
                            cx.notify();
                            return;
                        };
                        t.colors.insert("accent".into(), get(7));
                        t.colors.insert("background".into(), get(11));
                        t.fonts
                            .insert("body".into(), self.fields[12].read(cx).value().to_string());
                    } else {
                        let Ok(size) = get(10).parse() else {
                            self.message = "标题字号必须为1–512".into();
                            cx.notify();
                            return;
                        };
                        for p in &mut m.placeholders {
                            if p.kind == "title" {
                                p.style.size = Some(size);
                            }
                        }
                    }
                }
            }
            "fields" => {
                if let Some(l) = design.layouts.get_mut(&chosen) {
                    for (kind, x) in [("number", 800.), ("date", 60.)] {
                        if !l.placeholders.iter().any(|p| p.kind == kind) {
                            l.placeholders.push(design::Placeholder {
                                kind: kind.into(),
                                index: 0,
                                style: design::Layer {
                                    frame: Some([x, d.height - 40., 100., 30.]),
                                    fill: Some("@accent".into()),
                                    size: Some(14.),
                                    family: Some("@body".into()),
                                },
                            });
                        }
                    }
                }
            }
            _ => {}
        }
        self.action(Action::SetDesign(design), cx);
    }
    pub(super) fn notes(&mut self, cx: &mut Context<Self>) {
        let slide = &self.session.deck.slides[self.page];
        if slide.id != self.doc_page || slide.notes != self.notes_base {
            self.message = "备注已在其他窗口或页面改变，请刷新后重新编辑".into();
            cx.notify();
            return;
        }
        let notes = self.notes_input.read(cx).value().to_string();
        self.action(Action::Notes(self.page, notes.clone()), cx);
        if self.session.deck.slides[self.page].notes == notes {
            self.notes_base = notes;
        }
    }
    pub(super) fn properties(&mut self, cx: &mut Context<Self>) {
        let get = |i: usize| self.doc_fields[i].read(cx).value().to_string();
        let mut org = self.session.deck.organization.clone();
        org.author = get(1);
        org.subject = get(2);
        self.action(Action::Properties(get(0), org, get(9)), cx);
    }
    pub(super) fn section(&mut self, delete: bool, cx: &mut Context<Self>) {
        if delete {
            if let Some(id) = self.session.deck.slides[self.page].section.clone() {
                self.action(Action::DeleteSection(id), cx);
            }
            return;
        }
        let name = self.doc_fields[4].read(cx).value().to_string();
        let id = self.session.deck.slides[self.page]
            .section
            .clone()
            .unwrap_or_else(|| unique("section", self.session.deck.organization.sections.keys()));
        self.action(Action::Section(self.page_ids(), id, name), cx);
    }
    pub(super) fn find_next(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let value = self.doc_fields[5].read(cx).value().to_string();
        match design_commands::find(&self.session.deck, &value) {
            Ok(hits) if !hits.is_empty() => {
                let hit = &hits[self.search_at % hits.len()];
                self.search_at += 1;
                self.page_click(hit.page, false, false, cx);
                if let Some(i) = self.session.deck.slides[self.page]
                    .objects
                    .iter()
                    .position(|o| o.id == hit.object)
                {
                    self.select(i, w, cx);
                    self.begin_text(i, w, cx);
                    if let Some(t) = &mut self.text_edit {
                        t.selection = hit.range.clone();
                    }
                    self.message = format!(
                        "匹配 {}/{}",
                        (self.search_at - 1) % hits.len() + 1,
                        hits.len()
                    );
                }
            }
            Ok(_) => self.message = "没有匹配文字".into(),
            Err(e) => self.message = e,
        }
        cx.notify();
    }
    pub(super) fn replace_all(&mut self, font: bool, cx: &mut Context<Self>) {
        let from = self.doc_fields[5].read(cx).value().to_string();
        let to = self.doc_fields[6].read(cx).value().to_string();
        self.action(
            if font {
                Action::ReplaceFont(from, to)
            } else {
                Action::ReplaceAll(from, to)
            },
            cx,
        );
    }
    pub(super) fn copy_pages(&mut self, cx: &mut Context<Self>) {
        self.finish_text(cx);
        match PageTransfer::capture(&self.session.deck, &self.page_ids())
            .and_then(|p| serde_json::to_string(&p).map_err(|e| e.to_string()))
        {
            Ok(data) if data.len() <= joydeck::document::MAX_PART => {
                cx.write_to_clipboard(ClipboardItem::new_string_with_metadata(
                    "JoyDeck 页面".into(),
                    data,
                ));
                self.message = "已复制页面与独立设计/媒体快照".into();
            }
            Ok(_) => self.message = "页面剪贴板超过8 MiB预算".into(),
            Err(e) => self.message = e,
        }
        cx.notify();
    }
    pub(super) fn paste_pages(&mut self, target: bool, cx: &mut Context<Self>) {
        let result = cx
            .read_from_clipboard()
            .and_then(|i| i.metadata().cloned())
            .filter(|s| s.len() <= joydeck::document::MAX_PART)
            .ok_or("剪贴板没有受支持的页面快照".to_string())
            .and_then(|s| serde_json::from_str::<PageTransfer>(&s).map_err(|e| e.to_string()));
        match result {
            Ok(p) => {
                let theme = if target {
                    self.session.deck.design.themes.keys().next().cloned()
                } else {
                    None
                };
                if target && theme.is_none() {
                    self.message = "目标文稿没有主题，请先创建设计".into();
                    cx.notify();
                    return;
                }
                self.action(Action::PastePages(self.page + 1, p, theme), cx);
            }
            Err(e) => {
                self.message = e;
                cx.notify();
            }
        }
    }
    pub(super) fn workspace_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut panel = div().flex().flex_col().gap_2().child("文稿组织与设计");
        for (op, label) in [
            ("starter", "创建原创设计"),
            ("apply", "应用选中版式"),
            ("reset", "重置版式格式"),
            ("copy-layout", "复制版式"),
            ("rename-layout", "重命名版式"),
            ("delete-layout", "删除版式（在用时拒绝）"),
            ("copy-master", "复制母版及主题"),
            ("rename-master", "重命名母版"),
            ("delete-master", "删除母版（引用保护）"),
            ("theme", "应用主题颜色/字体/背景"),
            ("master-title", "应用母版标题字号"),
            ("fields", "添加页码/日期占位符"),
        ] {
            panel = panel.child(
                Button::new(op)
                    .label(label)
                    .on_click(cx.listener(move |s, _, _, cx| s.design_op(op, cx))),
            );
        }
        for (id, l) in &self.session.deck.design.layouts {
            let id = id.clone();
            let name = format!(
                "{} · {}",
                l.name,
                self.session
                    .deck
                    .design
                    .masters
                    .get(&l.master)
                    .map_or("?", |m| m.name.as_str())
            );
            panel = panel.child(
                Button::new(SharedString::from(format!("choose-{id}")))
                    .label(name)
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.chosen_layout = id.clone();
                        s.message = format!("已选择版式 {id}");
                        cx.notify();
                    })),
            );
        }
        panel = panel.child(
            Button::new("refresh-doc-fields")
                .label("刷新文稿字段")
                .on_click(cx.listener(|s, _, w, cx| s.sync_doc_fields(w, cx))),
        );
        for (i, label) in [
            "文稿标题",
            "作者",
            "主题说明",
            "当前页备注（应用后提交）",
            "节名称",
            "查找文字 / 原字体",
            "替换文字 / 新字体",
            "主题 accent #RRGGBB",
            "母版/版式新名称",
            "字段日期（固定文本）",
            "母版标题字号",
            "主题背景 #RRGGBB",
        ]
        .iter()
        .enumerate()
        {
            if i == 3 {
                panel = panel.child(div().child(*label).child(Textarea::new(&self.notes_input)));
            } else {
                panel = panel.child(div().child(*label).child(Input::new(&self.doc_fields[i])));
            }
        }
        panel = panel
            .child(
                Button::new("properties")
                    .label("应用文档属性与日期")
                    .on_click(cx.listener(|s, _, _, cx| s.properties(cx))),
            )
            .child(
                Button::new("notes")
                    .label("应用当前页备注")
                    .on_click(cx.listener(|s, _, _, cx| s.notes(cx))),
            )
            .child(
                Button::new("section")
                    .label("创建/重命名节并归入选中页")
                    .on_click(cx.listener(|s, _, _, cx| s.section(false, cx))),
            )
            .child(
                Button::new("delete-section")
                    .label("删除当前节，保留页面")
                    .on_click(cx.listener(|s, _, _, cx| s.section(true, cx))),
            )
            .child(
                Button::new("find-next")
                    .label("查找下一个")
                    .on_click(cx.listener(|s, _, w, cx| s.find_next(w, cx))),
            )
            .child(
                Button::new("replace-all")
                    .label("全部替换（一次撤销）")
                    .on_click(cx.listener(|s, _, _, cx| s.replace_all(false, cx))),
            )
            .child(
                Button::new("replace-font")
                    .label("替换字体（一次撤销）")
                    .on_click(cx.listener(|s, _, _, cx| s.replace_all(true, cx))),
            )
            .child(
                Button::new("copy-pages")
                    .label("复制所选页面")
                    .on_click(cx.listener(|s, _, _, cx| s.copy_pages(cx))),
            )
            .child(
                Button::new("paste-pages-source")
                    .label("粘贴页面：源格式")
                    .on_click(cx.listener(|s, _, _, cx| s.paste_pages(false, cx))),
            )
            .child(
                Button::new("paste-pages-target")
                    .label("粘贴页面：目标主题")
                    .on_click(cx.listener(|s, _, _, cx| s.paste_pages(true, cx))),
            );
        panel = panel.child("大纲导航（点击切页）");
        for (i, page) in self.session.deck.slides.iter().enumerate() {
            let section = page
                .section
                .as_ref()
                .and_then(|id| self.session.deck.organization.sections.get(id))
                .map_or("", String::as_str);
            let text = page
                .objects
                .iter()
                .filter(|o| !o.text.is_empty())
                .map(|o| o.text.as_str())
                .collect::<Vec<_>>()
                .join(" · ");
            panel = panel.child(
                Button::new(("outline", i))
                    .label(format!(
                        "{} {section} {} {}",
                        i + 1,
                        page.title,
                        text.chars().take(40).collect::<String>()
                    ))
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.page_click(i, false, false, cx);
                        s.sync_doc_fields(w, cx);
                        s.focus.focus(w, cx);
                    })),
            );
        }
        panel.into_any_element()
    }
}
fn unique<'a>(prefix: &str, ids: impl Iterator<Item = &'a String>) -> String {
    let ids: BTreeSet<_> = ids.cloned().collect();
    let mut candidate = format!("{prefix}-{}", ids.len());
    while ids.contains(&candidate) {
        candidate.push('-');
    }
    candidate
}
