//! Capture a bounded object tree, resolve its inherited appearance, and stage
//! a single destination edit with fresh object IDs and embedded media.
use crate::{EditorSnapshot, checked, media_refs, text};
use joydeck_model::{MediaId, MediaItem, Presentation, Shape, ShapeId, ShapeKind, Xfrm, resolve};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::Arc,
};

pub(crate) const PASTE_OFFSET: f64 = 18.;
const MAX_OBJECTS: usize = 256;
pub(crate) const MAX_GROUP_DEPTH: usize = 32;
const MAX_RESOURCES: usize = 512;
const MAX_RESOURCE_BYTES: usize = 16 * 1024 * 1024;
const OBJECT_BUDGET: &str = "请选择 1–256 个对象（含组内对象）";
const RESOURCE_BUDGET: &str = "对象剪贴板版本或大小超出限制";
const MISSING_RESOURCE: &str = "剪贴板缺少对象资源";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transfer {
    pub joydeck_objects: u32,
    pub shapes: Vec<Shape>,
    pub media: Vec<Resource>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub id: MediaId,
    pub name: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

pub fn capture(
    presentation: &Presentation,
    page: usize,
    ids: &[ShapeId],
) -> Result<Transfer, String> {
    let slide = presentation.slides.get(page).ok_or("幻灯片不存在")?;
    let appearance = resolve::Ctx::for_slide(presentation, slide).ok_or("无法解析当前版式")?;
    let selection: HashSet<_> = ids.iter().copied().collect();
    let selected: Vec<_> = slide
        .shapes
        .iter()
        .filter(|s| selection.contains(&s.id))
        .collect();
    check_objects(selected.iter().copied(), Some(&appearance))?;
    let mut shapes: Vec<_> = selected.into_iter().cloned().collect();
    for shape in &mut shapes {
        crate::clipboard_appearance::materialize(&appearance, shape);
    }
    let ids: BTreeSet<_> = media_refs::shapes_media(&shapes).into_iter().collect();
    let media = capture_resources(presentation, ids)?;
    Ok(Transfer {
        joydeck_objects: 1,
        shapes,
        media,
    })
}

fn capture_resources(
    presentation: &Presentation,
    ids: BTreeSet<MediaId>,
) -> Result<Vec<Resource>, String> {
    let mut budget = ResourceBudget::new(ids.len())?;
    let sources: Vec<_> = ids
        .into_iter()
        .map(|id| {
            let item = presentation.media(id).ok_or("对象引用的资源丢失")?;
            if item.link.is_some() {
                return Err("链接资源请先嵌入文稿再复制".into());
            }
            budget.charge(item.data.len())?;
            Ok(item)
        })
        .collect::<Result<_, String>>()?;
    Ok(sources
        .into_iter()
        .map(|item| Resource {
            id: item.id,
            name: item.name.clone(),
            content_type: item.content_type.clone(),
            data: (*item.data).clone(),
        })
        .collect())
}

struct ResourceBudget(usize);

impl ResourceBudget {
    fn new(count: usize) -> Result<Self, String> {
        if count > MAX_RESOURCES {
            return Err(RESOURCE_BUDGET.into());
        }
        Ok(Self(MAX_RESOURCE_BYTES))
    }

    fn charge(&mut self, bytes: usize) -> Result<(), String> {
        if bytes == 0 {
            return Err("剪贴板资源无效".into());
        }
        self.0 = self.0.checked_sub(bytes).ok_or(RESOURCE_BUDGET)?;
        Ok(())
    }
}

/// Check before cloning. Return preorder IDs so allocation follows the z-order
/// of the roots and the child order of each group.
fn check_objects<'a>(
    roots: impl DoubleEndedIterator<Item = &'a Shape>,
    source: Option<&resolve::Ctx>,
) -> Result<Vec<ShapeId>, String> {
    let mut ids = Vec::new();
    let mut seen = HashSet::new();
    let mut pending: Vec<_> = roots
        .rev()
        .take(MAX_OBJECTS + 1)
        .map(|shape| (shape, 0))
        .collect();
    if pending.is_empty() || pending.len() > MAX_OBJECTS {
        return Err(OBJECT_BUDGET.into());
    }
    while let Some((shape, depth)) = pending.pop() {
        if ids.len() == MAX_OBJECTS || depth > MAX_GROUP_DEPTH {
            return Err(OBJECT_BUDGET.into());
        }
        if !seen.insert(shape.id) || shape.raw_ext.is_some() || shape.diagram.is_some() {
            return Err("对象剪贴板包含重复 ID 或不支持的内容".into());
        }
        ids.push(shape.id);
        let transform = source
            .map(|ctx| resolve::xfrm(ctx, shape))
            .or(shape.xfrm)
            .ok_or("剪贴板对象缺少位置")?;
        check_transform(transform, 1.)?;
        if let Some(body) = &shape.text {
            text::validate(body)?;
        }
        match &shape.kind {
            ShapeKind::Shape | ShapeKind::Picture { .. } | ShapeKind::Connector { .. } => {}
            ShapeKind::Group { children, child } => {
                check_transform(*child, f64::MIN_POSITIVE)?;
                if ids.len() + pending.len() + children.len() > MAX_OBJECTS {
                    return Err(OBJECT_BUDGET.into());
                }
                pending.extend(children.iter().rev().map(|s| (s, depth + 1)));
            }
            ShapeKind::Table(table) => {
                if table.n_rows() > 64 || table.n_cols() > 32 {
                    return Err("剪贴板表格超出限制".into());
                }
                for row in &table.rows {
                    if row.cells.len() > 32 {
                        return Err("剪贴板表格超出限制".into());
                    }
                    for cell in &row.cells {
                        text::validate(&cell.text)?;
                    }
                }
            }
            _ => return Err(
                "当前剪贴板支持形状、文字、图片、连接线、表格和普通组；请将复杂内容留在源文稿中"
                    .into(),
            ),
        }
    }
    Ok(ids)
}

fn check_transform(x: Xfrm, minimum_extent: f64) -> Result<(), String> {
    for v in [x.x, x.y, x.w, x.h, x.rot] {
        checked(v)?;
    }
    if x.w < minimum_extent || x.h < minimum_extent {
        return Err("剪贴板对象尺寸无效".into());
    }
    Ok(())
}

/// A resource plan owns only new allocations. Match MIME type and exact bytes,
/// choose the first identical embedded item, and never match an external link.
pub(crate) struct MediaImport {
    pub(crate) ids: BTreeMap<MediaId, MediaId>,
    new: Vec<MediaItem>,
    next_id: u32,
}

impl MediaImport {
    pub(crate) fn prepare(
        presentation: &Presentation,
        resources: &[Resource],
    ) -> Result<Self, String> {
        let mut budget = ResourceBudget::new(resources.len())?;
        let mut classes = HashSet::new();
        let mut source_ids = HashSet::new();
        for resource in resources {
            budget.charge(resource.data.len())?;
            if !source_ids.insert(resource.id) {
                return Err("剪贴板资源无效".into());
            }
            classes.insert((resource.content_type.as_str(), resource.data.len()));
        }
        let mut content = HashMap::new();
        for item in &presentation.media {
            let mime = item.content_type.as_str();
            if item.link.is_none() && classes.contains(&(mime, item.data.len())) {
                content
                    .entry((mime, item.data.as_slice()))
                    .or_insert(item.id);
            }
        }
        let mut plan = Self {
            ids: BTreeMap::new(),
            new: Vec::new(),
            next_id: presentation.next_id,
        };
        for resource in resources {
            let key = (resource.content_type.as_str(), resource.data.as_slice());
            let id = if let Some(&id) = content.get(&key) {
                id
            } else {
                let id = MediaId(plan.allocate()?);
                plan.new.push(MediaItem {
                    id,
                    name: resource.name.clone(),
                    content_type: resource.content_type.clone(),
                    data: Arc::new(resource.data.clone()),
                    link: None,
                });
                content.insert(key, id);
                id
            };
            plan.ids.insert(resource.id, id);
        }
        Ok(plan)
    }

    fn allocate(&mut self) -> Result<u32, String> {
        self.next_id = self.next_id.max(1).checked_add(1).ok_or("ID 空间已耗尽")?;
        Ok(self.next_id)
    }

    pub(crate) fn install(self, model: &mut Presentation) {
        model.next_id = self.next_id;
        model.media.extend(self.new);
    }
}

pub(super) fn paste(
    snapshot: &mut EditorSnapshot,
    transfer: &Transfer,
    replacement_id: Option<ShapeId>,
) -> Result<(), String> {
    if transfer.joydeck_objects != 1 {
        return Err(RESOURCE_BUDGET.into());
    }
    let page = snapshot.selection.page;
    if snapshot.model.slides.get(page).is_none() {
        return Err("幻灯片不存在".into());
    }
    let order = check_objects(transfer.shapes.iter(), None)?;
    let mut media = MediaImport::prepare(&snapshot.model, &transfer.media)?;
    if media_refs::shapes_media(&transfer.shapes)
        .iter()
        .any(|id| !media.ids.contains_key(id))
    {
        return Err(MISSING_RESOURCE.into());
    }
    let objects: BTreeMap<_, _> = order
        .into_iter()
        .enumerate()
        .map(|(index, id)| {
            let allocated = ShapeId(media.allocate()?);
            Ok((
                id,
                if index == 0 {
                    replacement_id.unwrap_or(allocated)
                } else {
                    allocated
                },
            ))
        })
        .collect::<Result<_, String>>()?;
    let mut forest = transfer.shapes.clone();
    for root in &mut forest {
        if let Some(transform) = &mut root.xfrm {
            transform.x += PASTE_OFFSET;
            transform.y += PASTE_OFFSET;
        }
        rebind_objects(root, &objects);
    }
    media_refs::visit_mut::shapes(&mut forest, &mut |id| {
        *id = media.ids[id];
    });

    let model = Arc::make_mut(&mut snapshot.model);
    media.install(model);
    snapshot.selection.shapes = forest.iter().map(|s| s.id).collect();
    Arc::make_mut(&mut model.slides[page]).shapes.extend(forest);
    Ok(())
}

fn rebind_objects(shape: &mut Shape, ids: &BTreeMap<ShapeId, ShapeId>) {
    shape.id = ids[&shape.id];
    match &mut shape.kind {
        ShapeKind::Group { children, .. } => {
            for child in children {
                rebind_objects(child, ids);
            }
        }
        ShapeKind::Connector { start, end } => {
            for endpoint in [start, end] {
                *endpoint = endpoint.and_then(|(id, site)| Some((*ids.get(&id)?, site)));
            }
        }
        _ => {}
    }
}
