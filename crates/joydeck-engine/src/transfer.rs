//! Object clipboard transactions. ID/connector/media remapping follows DeckCraft
//! cmd/edit.rs; the typed media walker is reused unchanged apart from namespaces.
use super::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    result::Result,
};

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

pub fn capture(p: &Presentation, page: usize, ids: &[ShapeId]) -> Result<Transfer, String> {
    let slide = p.slides.get(page).ok_or("幻灯片不存在")?;
    let ctx = resolve::Ctx::for_slide(p, slide).ok_or("无法解析当前版式")?;
    let mut shapes: Vec<_> = slide
        .shapes
        .iter()
        .filter(|s| ids.contains(&s.id))
        .cloned()
        .collect();
    if shapes.is_empty() || shapes.len() > 256 {
        return Err("请选择 1–256 个对象".into());
    }
    for s in &mut shapes {
        if !matches!(
            s.kind,
            ShapeKind::Shape
                | ShapeKind::Picture { .. }
                | ShapeKind::Table(_)
                | ShapeKind::Connector { .. }
        ) || s.raw_ext.is_some()
        {
            return Err(
                "当前剪贴板支持形状、文字、图片、连接线和表格；复杂内容请保留在源文稿中".into(),
            );
        }
        s.xfrm = Some(resolve::xfrm(&ctx, s));
        let (fill, _) = resolve::fill(&ctx, s);
        s.fill = fill;
        s.line = Some(resolve::line(&ctx, s).0);
        s.effects = resolve::effects(&ctx, s).0;
        if let Some(mut body) = s.text.clone() {
            body.body = resolve::body(&ctx, s);
            for para in &mut body.paragraphs {
                para.props = resolve::para(&ctx, s, para);
                para.def_props = resolve::run(&ctx, s, para, &para.def_props);
                para.end_props = resolve::run(&ctx, s, para, &para.end_props);
                for index in 0..para.runs.len() {
                    para.runs[index].props = resolve::run(&ctx, s, para, &para.runs[index].props);
                }
            }
            s.text = Some(body);
        }
        s.ph = None;
        s.style = None;
    }
    // Freeze scheme colours and font references before crossing document themes.
    fn bake(v: &mut serde_json::Value, ctx: &resolve::Ctx, depth: usize) {
        if depth > 64 {
            return;
        }
        match v {
            serde_json::Value::Object(o) => {
                if o.get("kind").and_then(|v| v.as_str()) == Some("scheme")
                    && let Ok(c) =
                        serde_json::from_value::<ColorRef>(serde_json::Value::Object(o.clone()))
                    && let Ok(serde_json::Value::Object(rgb)) =
                        serde_json::to_value(ColorRef::rgb(ctx.color(&c, None)))
                {
                    *o = rgb;
                    return;
                }
                for x in o.values_mut() {
                    bake(x, ctx, depth + 1);
                }
            }
            serde_json::Value::Array(a) => {
                for x in a {
                    bake(x, ctx, depth + 1);
                }
            }
            serde_json::Value::String(s) if s.starts_with("+mj-") || s.starts_with("+mn-") => {
                let r = RunProps {
                    font: Some(s.clone()),
                    ..Default::default()
                };
                *s = resolve::font_family(ctx, &r);
            }
            _ => {}
        }
    }
    let mut value = serde_json::to_value(&shapes).map_err(|e| e.to_string())?;
    bake(&mut value, &ctx, 0);
    shapes = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let mut refs = BTreeSet::new();
    super::media_refs::visit::shapes(&shapes, &mut |id| {
        refs.insert(*id);
    });
    let mut media = Vec::new();
    for id in refs {
        let m = p.media(id).ok_or("对象引用的资源丢失")?;
        if m.link.is_some() {
            return Err("链接资源请先嵌入文稿再复制".into());
        }
        media.push(Resource {
            id,
            name: m.name.clone(),
            content_type: m.content_type.clone(),
            data: m.data.as_ref().clone(),
        });
    }
    let clip = Transfer {
        joydeck_objects: 1,
        shapes,
        media,
    };
    validate(&clip)?;
    Ok(clip)
}
fn validate(t: &Transfer) -> Result<(), String> {
    if t.joydeck_objects != 1
        || t.shapes.is_empty()
        || t.shapes.len() > 256
        || t.media.len() > 512
        || t.media.iter().map(|m| m.data.len()).sum::<usize>() > 16 * 1024 * 1024
    {
        return Err("对象剪贴板版本或大小超出限制".into());
    }
    let mut ids = BTreeSet::new();
    for s in &t.shapes {
        if !ids.insert(s.id)
            || !matches!(
                s.kind,
                ShapeKind::Shape
                    | ShapeKind::Picture { .. }
                    | ShapeKind::Table(_)
                    | ShapeKind::Connector { .. }
            )
            || s.raw_ext.is_some()
        {
            return Err("对象剪贴板包含重复 ID 或不支持的内容".into());
        }
        let x = s.xfrm.ok_or("剪贴板对象缺少位置")?;
        for v in [x.x, x.y, x.w, x.h, x.rot] {
            checked(v)?;
        }
        if x.w < 1. || x.h < 1. {
            return Err("剪贴板对象尺寸无效".into());
        }
        if let Some(body) = &s.text {
            text::validate(body)?;
        }
        if let ShapeKind::Table(table) = &s.kind {
            if table.n_rows() > 64 || table.n_cols() > 32 {
                return Err("剪贴板表格超出限制".into());
            }
            for r in &table.rows {
                for c in &r.cells {
                    text::validate(&c.text)?;
                }
            }
        }
    }
    let mut media_ids = BTreeSet::new();
    for m in &t.media {
        if !media_ids.insert(m.id) || m.data.is_empty() {
            return Err("剪贴板资源无效".into());
        }
    }
    let mut missing = false;
    super::media_refs::visit::shapes(&t.shapes, &mut |id| missing |= !media_ids.contains(id));
    if missing {
        return Err("剪贴板缺少对象资源".into());
    }
    Ok(())
}
pub(super) fn paste(st: &mut EditorSnapshot, t: &Transfer) -> Result<(), String> {
    validate(t)?;
    let p = Arc::make_mut(&mut st.model);
    let mut media_map = BTreeMap::new();
    for m in &t.media {
        let id = if let Some(existing) = p
            .media
            .iter()
            .find(|v| v.content_type == m.content_type && v.data.as_ref() == &m.data)
        {
            existing.id
        } else {
            p.add_media(&m.name, &m.content_type, m.data.clone())
                .ok_or("ID 空间已耗尽")?
        };
        media_map.insert(m.id, id);
    }
    let mut shapes = t.shapes.clone();
    let mut map = BTreeMap::new();
    for s in &mut shapes {
        let old = s.id;
        s.id = ShapeId(next_id(p)?);
        map.insert(old, s.id);
        if let Some(x) = &mut s.xfrm {
            x.x += 18.;
            x.y += 18.;
        }
    }
    for s in &mut shapes {
        if let ShapeKind::Connector { start, end } = &mut s.kind {
            for at in [start, end] {
                *at = at.and_then(|(id, site)| map.get(&id).map(|id| (*id, site)));
            }
        }
    }
    super::media_refs::visit_mut::shapes(&mut shapes, &mut |id| {
        if let Some(new) = media_map.get(id) {
            *id = *new;
        }
    });
    st.selection.shapes = shapes.iter().map(|s| s.id).collect();
    Arc::make_mut(&mut p.slides[st.selection.page])
        .shapes
        .extend(shapes);
    Ok(())
}
