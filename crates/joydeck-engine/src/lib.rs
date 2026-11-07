#![forbid(unsafe_code)]
//! One transaction entry point for desktop and CLI. No file, renderer or window services.
use joydeck_model::{text::RunKind, *};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
mod authoring;
mod content;
mod footer;
pub use footer::{ExistingFooterSummary, existing_footer_summary};
mod history_cost;
mod review;
mod workspace;
pub use review::ReviewAction;
pub use workspace::WorkspaceAction;
pub mod clipboard;
pub mod media_refs;
pub mod transfer;
pub use content::ContentAction;
pub mod formatting;
pub mod text;
pub mod text_resources;
pub use authoring::{CellBorders, CellFormat, CellRange, TableAction, TextEdit, TextTarget};
pub use authoring::{body as text_body, set_body as set_draft_body};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Origin {
    pub slide: SlideId,
    pub shapes: BTreeMap<ShapeId, ShapeId>,
}
pub type Origins = BTreeMap<SlideId, Origin>;
pub fn original_bindings(p: &Presentation) -> Origins {
    p.slides
        .iter()
        .map(|s| {
            let mut shapes = BTreeMap::new();
            walk(&s.shapes, &mut |sh, _| {
                shapes.insert(sh.id, sh.id);
            });
            (
                s.id,
                Origin {
                    slide: s.id,
                    shapes,
                },
            )
        })
        .collect()
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Selection {
    pub page: usize,
    pub shapes: Vec<ShapeId>,
}
impl Selection {
    pub fn checked(
        model: &Presentation,
        page: usize,
        shapes: Vec<ShapeId>,
    ) -> std::result::Result<Self, String> {
        let slide = model.slides.get(page).ok_or("page outside document")?;
        let mut missing: BTreeSet<_> = shapes.iter().copied().collect();
        for shape in &slide.shapes {
            if missing.is_empty() {
                break;
            }
            missing.remove(&shape.id);
        }
        if !missing.is_empty() {
            return Err("select top-level shapes only".into());
        }
        Ok(Self { page, shapes })
    }
}
#[derive(Clone, Debug)]
pub struct EditorSnapshot {
    pub model: Arc<Presentation>,
    pub origins: Origins,
    pub selection: Selection,
    pub content_id: u64,
}
impl EditorSnapshot {
    /// Stage a resolved edit without changing a live session or allocating undo history.
    pub fn stage_edit(&mut self, command: Command) -> std::result::Result<(), String> {
        apply(self, command)?;
        check_model_budget(&self.model)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Command {
    At {
        page: usize,
        shapes: Vec<ShapeId>,
        edit: Box<Command>,
    },
    Workspace {
        action: WorkspaceAction,
    },
    Batch {
        commands: Vec<Command>,
    },
    Content {
        action: ContentAction,
    },
    Review {
        action: ReviewAction,
    },
    AcceptObject {
        page: usize,
        target: Option<ShapeId>,
        replacement: Option<Box<transfer::Transfer>>,
    },
    InsertPicture {
        png: Vec<u8>,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        #[serde(default)]
        replace: bool,
    },
    Select {
        page: usize,
        shapes: Vec<ShapeId>,
    },
    PasteObjects {
        transfer: Box<transfer::Transfer>,
    },
    InsertShape {
        preset: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    InsertRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    InsertText {
        x: f64,
        y: f64,
        text: String,
    },
    Transform {
        dx: f64,
        dy: f64,
        #[serde(default)]
        dw: f64,
        #[serde(default)]
        dh: f64,
        #[serde(default)]
        rotation: f64,
    },
    ReplaceText {
        text: String,
    },
    SetText {
        target: TextTarget,
        body: Box<TextBody>,
    },
    SetTextWithResources {
        slide: SlideId,
        target: TextTarget,
        expected: Box<TextBody>,
        body: Box<TextBody>,
        resources: Vec<transfer::Resource>,
    },
    /// Fill all missing slide footers atomically, preserving existing footers.
    AddAllMissingFooterText {
        text: String,
    },
    /// Add a missing footer only to the selected page, after conservative preflight.
    AddMissingFooterText {
        text: String,
    },
    /// Update existing simple footer placeholders across slides without adding any.
    SetExistingFooterText {
        text: String,
    },
    SetTexts {
        edits: Vec<TextEdit>,
    },
    EditText {
        target: TextTarget,
        start: usize,
        end: usize,
        text: String,
    },
    InsertTable {
        x: f64,
        y: f64,
        rows: usize,
        cols: usize,
        width: f64,
        row_height: f64,
    },
    Table {
        action: TableAction,
    },
    Delete,
    Duplicate,
    BringForward,
    SendBackward,
    NewSlide,
    DuplicateSlide,
    DeleteSlide,
    Undo,
    Redo,
}
impl Command {
    pub fn id(&self) -> &'static str {
        match self {
            Self::At { .. } => "at",
            Self::Workspace { .. } => "workspace",
            Self::Batch { .. } => "batch",
            Self::Content { .. } => "content",
            Self::Review { .. } => "review",
            Self::AcceptObject { .. } => "accept-object",
            Self::InsertPicture { .. } => "insert-picture",
            Self::Select { .. } => "select",
            Self::PasteObjects { .. } => "paste-objects",
            Self::InsertRect { .. } => "insert-rect",
            Self::InsertShape { .. } => "insert-shape",
            Self::InsertText { .. } => "insert-text",
            Self::Transform { .. } => "transform",
            Self::ReplaceText { .. } => "replace-text",
            Self::SetText { .. } => "set-text",
            Self::SetTextWithResources { .. } => "set-text-with-resources",
            Self::SetTexts { .. } => "set-texts",
            Self::AddAllMissingFooterText { .. } => "add-all-missing-footer-text",
            Self::AddMissingFooterText { .. } => "add-missing-footer-text",
            Self::SetExistingFooterText { .. } => "set-existing-footer-text",
            Self::EditText { .. } => "edit-text",
            Self::InsertTable { .. } => "insert-table",
            Self::Table { .. } => "table",
            Self::Delete => "delete",
            Self::Duplicate => "duplicate",
            Self::BringForward => "bring-forward",
            Self::SendBackward => "send-backward",
            Self::NewSlide => "new-slide",
            Self::DuplicateSlide => "duplicate-slide",
            Self::DeleteSlide => "delete-slide",
            Self::Undo => "undo",
            Self::Redo => "redo",
        }
    }
}
pub const COMMANDS: &[&str] = &[
    "at",
    "workspace",
    "batch",
    "content",
    "review",
    "accept-object",
    "insert-picture",
    "select",
    "paste-objects",
    "insert-rect",
    "insert-shape",
    "insert-text",
    "transform",
    "replace-text",
    "set-text",
    "set-texts",
    "edit-text",
    "insert-table",
    "table",
    "delete",
    "duplicate",
    "bring-forward",
    "send-backward",
    "new-slide",
    "duplicate-slide",
    "delete-slide",
    "undo",
    "redo",
];
#[derive(Clone)]
struct Entry {
    state: EditorSnapshot,
    cost: usize,
}
#[derive(Clone)]
pub struct Session {
    state: EditorSnapshot,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    saved: Option<u64>,
    revision: u64,
    next_content: u64,
}
impl Session {
    pub fn new(model: Presentation, origins: Origins, saved: bool) -> Self {
        Self {
            state: EditorSnapshot {
                model: Arc::new(model),
                origins,
                selection: Selection {
                    page: 0,
                    shapes: vec![],
                },
                content_id: 0,
            },
            undo: vec![],
            redo: vec![],
            saved: saved.then_some(0),
            revision: 0,
            next_content: 1,
        }
    }
    pub fn snapshot(&self) -> EditorSnapshot {
        self.state.clone()
    }
    pub fn model(&self) -> &Presentation {
        &self.state.model
    }
    pub fn model_snapshot(&self) -> Arc<Presentation> {
        self.state.model.clone()
    }
    pub fn selection(&self) -> &Selection {
        &self.state.selection
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn dirty(&self) -> bool {
        self.saved != Some(self.state.content_id)
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn mark_saved(&mut self, content_id: u64) {
        self.saved = Some(content_id);
    }
    pub fn select(&mut self, page: usize, shapes: Vec<ShapeId>) -> std::result::Result<(), String> {
        self.state.selection = Selection::checked(self.model(), page, shapes)?;
        Ok(())
    }
    /// Execute against a copy-on-write snapshot, including real history and budget checks.
    /// The live document, selection, save marker and undo/redo stacks remain unchanged.
    pub fn validate(&self, cmd: Command) -> std::result::Result<bool, String> {
        self.validate_from(&self.state.selection, cmd)
    }
    /// Validate a command against a window's selection without changing the shared session.
    pub fn validate_for_selection(
        &self,
        selection: &Selection,
        cmd: Command,
    ) -> std::result::Result<bool, String> {
        let selection = Selection::checked(self.model(), selection.page, selection.shapes.clone())?;
        self.validate_from(&selection, cmd)
    }
    fn validate_from(
        &self,
        selection: &Selection,
        cmd: Command,
    ) -> std::result::Result<bool, String> {
        if let Command::Select { page, shapes } = cmd {
            Selection::checked(
                self.model(),
                page.checked_sub(1).ok_or("pages start at 1")?,
                shapes,
            )?;
            return Ok(false);
        }
        if self.model().slides.get(selection.page).is_none() {
            return Err("no active slide".into());
        }
        match cmd {
            Command::Undo => Ok(self.can_undo()),
            Command::Redo => Ok(self.can_redo()),
            _ => self.prepare_edit(selection, cmd).map(|next| next.is_some()),
        }
    }
    pub fn execute(&mut self, cmd: Command) -> std::result::Result<bool, String> {
        if let Command::Select { page, shapes } = cmd {
            self.select(page.checked_sub(1).ok_or("pages start at 1")?, shapes)?;
            return Ok(false);
        }
        if self
            .state
            .model
            .slides
            .get(self.state.selection.page)
            .is_none()
        {
            return Err("no active slide".into());
        }
        if matches!(cmd, Command::Undo | Command::Redo) {
            let (from, to) = if matches!(cmd, Command::Undo) {
                (&mut self.undo, &mut self.redo)
            } else {
                (&mut self.redo, &mut self.undo)
            };
            let Some(entry) = from.pop() else {
                return Ok(false);
            };
            let cost = history_cost::undo_cost(&self.state.model, &entry.state.model);
            to.push(Entry {
                state: std::mem::replace(&mut self.state, entry.state),
                cost,
            });
            self.revision += 1;
            return Ok(true);
        }
        let Some(mut next) = self.prepare_edit(&self.state.selection, cmd)? else {
            return Ok(false);
        };
        next.content_id = self.next_content;
        self.next_content += 1;
        let cost = history_cost::undo_cost(&self.state.model, &next.model);
        self.undo.push(Entry {
            state: std::mem::replace(&mut self.state, next),
            cost,
        });
        self.redo.clear();
        self.revision += 1;
        let mut cost: usize = self.undo.iter().map(|e| e.cost).sum();
        while self.undo.len() > 1 && (self.undo.len() > 128 || cost > 64 << 20) {
            cost = cost.saturating_sub(self.undo.remove(0).cost);
        }
        Ok(true)
    }

    fn prepare_edit(
        &self,
        selection: &Selection,
        cmd: Command,
    ) -> std::result::Result<Option<EditorSnapshot>, String> {
        let mut next = self.state.clone();
        next.selection = selection.clone();
        apply(&mut next, cmd)?;
        next.origins.retain(|id, origin| {
            if let Some(slide) = next.model.slide(*id) {
                origin.shapes.retain(|id, _| slide.shape(*id).is_some());
                true
            } else {
                false
            }
        });
        if next.model == self.state.model && next.origins == self.state.origins {
            return Ok(None);
        }
        check_model_budget(&next.model)?;
        Ok(Some(next))
    }
}
fn check_model_budget(model: &Presentation) -> std::result::Result<(), String> {
    if model.slides.len() > 1000 {
        return Err("page budget exceeded".into());
    }
    let mut count = 0;
    for slide in &model.slides {
        walk(&slide.shapes, &mut |_, _| count += 1);
    }
    if count > 20_000 {
        return Err("shape budget exceeded".into());
    }
    Ok(())
}
fn checked(v: f64) -> std::result::Result<f64, String> {
    if v.is_finite() && v.abs() <= 32768. {
        Ok(v)
    } else {
        Err("geometry outside finite coordinate budget".into())
    }
}
fn next_id(p: &mut Presentation) -> std::result::Result<u32, String> {
    p.alloc_id().ok_or_else(|| "ID space exhausted".into())
}
fn apply(st: &mut EditorSnapshot, cmd: Command) -> std::result::Result<(), String> {
    if let Command::AddAllMissingFooterText { text } = &cmd {
        return footer::add_all_missing(st, text);
    }
    if let Command::AddMissingFooterText { text } = &cmd {
        return footer::add_missing(st, text);
    }
    if let Command::SetExistingFooterText { text } = &cmd {
        return footer::set_existing(st, text);
    }
    if let Command::At { page, shapes, edit } = cmd {
        if matches!(
            *edit,
            Command::At { .. }
                | Command::Batch { .. }
                | Command::Select { .. }
                | Command::Undo
                | Command::Redo
        ) {
            return Err("targeted edit cannot nest selection/history/target/batch".into());
        }
        let page = page
            .checked_sub(1)
            .filter(|p| *p < st.model.slides.len())
            .ok_or("target page missing")?;
        let unique: std::collections::BTreeSet<_> = shapes.iter().copied().collect();
        if unique.len() != shapes.len()
            || shapes
                .iter()
                .any(|id| !st.model.slides[page].shapes.iter().any(|s| s.id == *id))
        {
            return Err("target shapes missing or duplicated".into());
        }
        st.selection = Selection { page, shapes };
        return apply(st, *edit);
    }
    if let Command::Review { action } = &cmd {
        return review::apply(st, action);
    }
    if let Command::AcceptObject {
        page,
        target,
        replacement,
    } = &cmd
    {
        let page = page
            .checked_sub(1)
            .filter(|p| *p < st.model.slides.len())
            .ok_or("invalid acceptance page")?;
        st.selection.page = page;
        return review::accept(st, *target, replacement.as_deref());
    }
    if let Command::Workspace { action } = &cmd {
        return workspace::apply(st, action);
    }
    if let Command::Batch { commands } = cmd {
        if commands.is_empty()
            || commands.len() > 256
            || commands.iter().any(|c| {
                matches!(
                    c,
                    Command::Batch { .. } | Command::Select { .. } | Command::Undo | Command::Redo
                )
            })
        {
            return Err("batch requires 1–256 non-nested edit commands".into());
        }
        for command in commands {
            apply(st, command)?;
        }
        return Ok(());
    }
    if let Command::SetTextWithResources {
        slide,
        target,
        expected,
        body,
        resources,
    } = &cmd
    {
        return text_resources::apply(st, *slide, target, expected, body, resources);
    }
    if let Command::PasteObjects { transfer } = &cmd {
        return transfer::paste(st, transfer);
    }
    if let Some(result) = content::apply(st, &cmd) {
        return result;
    }
    if let Some(result) = authoring::apply(st, &cmd) {
        return result;
    }
    let page = st.selection.page;
    let p = Arc::make_mut(&mut st.model);
    if matches!(cmd, Command::NewSlide | Command::DuplicateSlide) {
        let old = p
            .slides
            .get(page)
            .ok_or("page outside document")?
            .as_ref()
            .clone();
        let id = SlideId(next_id(p)?);
        let mut slide = old.clone();
        slide.id = id;
        slide.name = format!("Slide {}", p.slides.len() + 1);
        let mut origin = st.origins.get(&old.id).cloned();
        if matches!(cmd, Command::NewSlide) {
            slide.shapes.clear();
            slide.comments.clear();
            slide.hidden = false;
            slide.notes = TextBody::default();
            slide.animations.clear();
            slide.transition = None;
            if let Some(o) = &mut origin {
                o.shapes.clear();
            }
        } else {
            if slide.shapes.iter().any(|s| {
                !matches!(
                    s.kind,
                    ShapeKind::Shape | ShapeKind::Picture { .. } | ShapeKind::Table(_)
                )
            }) {
                return Err(
                    "R2 duplicates simple slides; grouped/opaque/3D slides remain intact".into(),
                );
            }
            let mut cloned_ids = BTreeMap::new();
            for shape in &mut slide.shapes {
                let old = shape.id;
                shape.id = ShapeId(next_id(p)?);
                cloned_ids.insert(old, shape.id);
                shape.tags.retain(|(k, _)| k != "JoyOffice.LegacyId");
                if let Some(o) = &mut origin
                    && let Some(source) = o.shapes.remove(&old)
                {
                    o.shapes.insert(shape.id, source);
                }
            }
            for animation in &mut slide.animations {
                animation.shape = *cloned_ids
                    .get(&animation.shape)
                    .ok_or("animation target cannot be cloned")?;
                animation.trigger = animation
                    .trigger
                    .map(|id| {
                        cloned_ids
                            .get(&id)
                            .copied()
                            .ok_or("animation trigger cannot be cloned")
                    })
                    .transpose()?;
            }
            for (index, comment) in slide.comments.iter_mut().enumerate() {
                comment.id = format!("comment-{}-{index}", id.0);
                comment.shape = comment
                    .shape
                    .map(|id| {
                        cloned_ids
                            .get(&id)
                            .copied()
                            .ok_or("comment anchor cannot be cloned")
                    })
                    .transpose()?;
                for reply in &mut comment.replies {
                    reply.shape = comment.shape;
                }
            }
        }
        slide.tags.retain(|(k, _)| k != "JoyOffice.LegacyId");
        if let Some(origin) = origin {
            st.origins.insert(id, origin);
        }
        p.slides.insert(page + 1, Arc::new(slide));
        st.selection = Selection {
            page: page + 1,
            shapes: vec![],
        };
        return Ok(());
    }
    if matches!(cmd, Command::DeleteSlide) {
        if p.slides.len() <= 1 {
            return Err("keep at least one slide".into());
        }
        let removed = p.slides.remove(page);
        for section in &mut p.sections {
            section.slides.retain(|id| *id != removed.id);
        }
        for show in &mut p.custom_shows {
            show.slides.retain(|id| *id != removed.id);
        }
        st.origins.remove(&removed.id);
        st.selection = Selection {
            page: page.min(p.slides.len() - 1),
            shapes: vec![],
        };
        return Ok(());
    }
    if let Command::InsertRect {
        x,
        y,
        width,
        height,
    }
    | Command::InsertShape {
        x,
        y,
        width,
        height,
        ..
    } = &cmd
    {
        let preset = match &cmd {
            Command::InsertShape { preset, .. } => preset.as_str(),
            _ => "rect",
        };
        if !["rect", "roundRect", "ellipse", "triangle", "chevron"].contains(&preset) {
            return Err("unsupported inserted shape preset".into());
        }
        let (x, y, width, height) = (*x, *y, *width, *height);
        let id = ShapeId(next_id(p)?);
        let (x, y, w, h) = (checked(x)?, checked(y)?, checked(width)?, checked(height)?);
        if w < 1. || h < 1. {
            return Err("shape size must be at least 1 point".into());
        }
        Arc::make_mut(&mut p.slides[page]).shapes.push(Shape {
            id,
            name: preset.into(),
            geom: Geom::Preset {
                name: preset.into(),
                adj: Default::default(),
            },
            xfrm: Some(Xfrm::new(x, y, w, h)),
            fill: Some(Fill::solid(ColorRef::rgb(Rgba::rgb(166, 61, 36)))),
            ..Default::default()
        });
        st.selection.shapes = vec![id];
        return Ok(());
    }
    if let Command::InsertText { x, y, text } = cmd {
        if text.len() > 65536 {
            return Err("text exceeds 64 KiB".into());
        }
        let id = ShapeId(next_id(p)?);
        let mut body = TextBody::from_text(&text);
        for para in &mut body.paragraphs {
            para.def_props = RunProps {
                font: Some("Arial".into()),
                size: Some(24.),
                ..Default::default()
            };
        }
        Arc::make_mut(&mut p.slides[page]).shapes.push(Shape {
            id,
            name: "Text".into(),
            xfrm: Some(Xfrm::new(checked(x)?, checked(y)?, 400., 80.)),
            text: Some(body),
            text_box: true,
            ..Default::default()
        });
        st.selection.shapes = vec![id];
        return Ok(());
    }
    let selected: BTreeSet<_> = st.selection.shapes.iter().copied().collect();
    if p.slides[page]
        .shapes
        .iter()
        .any(|s| s.locked && selected.contains(&s.id))
    {
        return Err("selected object is locked".into());
    }
    if selected.is_empty() {
        return Err("select an object first".into());
    }
    if matches!(cmd, Command::Duplicate) {
        let old = p.slides[page]
            .shapes
            .iter()
            .filter(|s| selected.contains(&s.id))
            .cloned()
            .collect::<Vec<_>>();
        let slide_id = p.slides[page].id;
        let mut copies = vec![];
        for mut shape in old {
            if !matches!(
                shape.kind,
                ShapeKind::Shape | ShapeKind::Picture { .. } | ShapeKind::Table(_)
            ) {
                return Err("duplicate simple shapes only in R2".into());
            }
            let old_id = shape.id;
            shape.id = ShapeId(next_id(p)?);
            let ctx = resolve::Ctx::for_slide(p, &p.slides[page]).ok_or("missing layout")?;
            let mut x = resolve::xfrm(&ctx, &shape);
            x.x += 18.;
            x.y += 18.;
            shape.xfrm = Some(x);
            if let Some(origin) = st.origins.get_mut(&slide_id)
                && let Some(source) = origin.shapes.get(&old_id).copied()
            {
                origin.shapes.insert(shape.id, source);
            }
            copies.push(shape);
        }
        st.selection.shapes = copies.iter().map(|s| s.id).collect();
        Arc::make_mut(&mut p.slides[page]).shapes.extend(copies);
        return Ok(());
    }
    let transforms = {
        let ctx = resolve::Ctx::for_slide(p, &p.slides[page]).ok_or("missing layout")?;
        p.slides[page]
            .shapes
            .iter()
            .filter(|s| matches!(cmd, Command::Transform { .. }) && selected.contains(&s.id))
            .map(|s| (s.id, resolve::xfrm(&ctx, s)))
            .collect::<BTreeMap<_, _>>()
    };
    let slide = Arc::make_mut(&mut p.slides[page]);
    match cmd {
        Command::Delete => {
            slide.shapes.retain(|s| !selected.contains(&s.id));
            slide
                .comments
                .retain(|c| c.shape.is_none_or(|id| !selected.contains(&id)));
            slide.animations.retain(|a| !selected.contains(&a.shape));
            for animation in &mut slide.animations {
                if animation.trigger.is_some_and(|id| selected.contains(&id)) {
                    animation.trigger = None;
                }
            }
            for shape in &mut slide.shapes {
                if let ShapeKind::Connector { start, end } = &mut shape.kind {
                    *start = start.filter(|(id, _)| !selected.contains(id));
                    *end = end.filter(|(id, _)| !selected.contains(id));
                }
            }
            st.selection.shapes.clear();
        }
        Command::BringForward | Command::SendBackward => {
            let forward = matches!(cmd, Command::BringForward);
            let indices: Vec<usize> = if forward {
                (0..slide.shapes.len().saturating_sub(1)).rev().collect()
            } else {
                (1..slide.shapes.len()).collect()
            };
            for i in indices {
                let j = if forward { i + 1 } else { i - 1 };
                if selected.contains(&slide.shapes[i].id) && !selected.contains(&slide.shapes[j].id)
                {
                    slide.shapes.swap(i, j);
                }
            }
        }
        Command::Transform {
            dx,
            dy,
            dw,
            dh,
            rotation,
        } => {
            for v in [dx, dy, dw, dh, rotation] {
                checked(v)?;
            }
            for s in slide.shapes.iter_mut().filter(|s| selected.contains(&s.id)) {
                if s.locked {
                    return Err("object is locked".into());
                }
                let mut x = transforms[&s.id];
                x.x = checked(x.x + dx)?;
                x.y = checked(x.y + dy)?;
                x.w = checked(x.w + dw)?;
                x.h = checked(x.h + dh)?;
                x.rot = (x.rot + rotation).rem_euclid(360.);
                if x.w < 1. || x.h < 1. {
                    return Err("shape size must be at least 1 point".into());
                }
                s.xfrm = Some(x);
            }
        }
        Command::ReplaceText { text } => {
            if text.len() > 65536 || st.selection.shapes.len() != 1 {
                return Err("replace one text object (up to 64 KiB)".into());
            }
            let shape = slide
                .shapes
                .iter_mut()
                .find(|s| s.id == st.selection.shapes[0])
                .ok_or("missing object")?;
            let body = shape
                .text
                .as_mut()
                .filter(|_| matches!(shape.kind, ShapeKind::Shape))
                .ok_or("object is not an editable text shape")?;
            let mut props = joydeck_model::edit::props_at(body, (0, 0));
            props.link = None;
            props.hover = None;
            let mut replacement = TextBody {
                body: body.body.clone(),
                ..Default::default()
            };
            replacement.paragraphs = text
                .replace("\r\n", "\n")
                .replace('\r', "\n")
                .split('\n')
                .map(|line| {
                    let mut para = Paragraph::new(line);
                    para.def_props = body
                        .paragraphs
                        .first()
                        .map(|p| p.def_props.clone())
                        .unwrap_or_default();
                    para.runs = vec![Run {
                        text: line.into(),
                        props: props.clone(),
                        kind: RunKind::Text,
                    }];
                    para
                })
                .collect();
            if replacement.paragraphs.is_empty() {
                replacement.paragraphs.push(Paragraph::default());
            }
            *body = replacement;
        }
        _ => return Err("unsupported transaction".into()),
    }
    Ok(())
}
