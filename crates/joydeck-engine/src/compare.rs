//! Offline differences use stable IDs; accepting an object is a guarded, ordinary undoable edit.
use crate::{
    document::{Deck, Result},
    edit::Command,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Difference {
    pub kind: String,
    pub slide: Option<String>,
    pub object: Option<String>,
    pub fields: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub base_sha256: String,
    pub reference_sha256: String,
    pub differences: Vec<Difference>,
    pub truncated: bool,
}
pub fn fingerprint(d: &Deck) -> Result<String> {
    let mut deck = d.clone();
    deck.source = None;
    deck.resources.clear();
    for asset in deck.playback.assets.values_mut() {
        std::sync::Arc::make_mut(asset).bytes.clear();
    }
    let resources = d.resources.keys().collect::<Vec<_>>();
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(deck, resources)).map_err(|e| e.to_string())?)
    ))
}
fn fields(a: Value, b: Value) -> Vec<String> {
    let (Some(a), Some(b)) = (a.as_object(), b.as_object()) else {
        return vec![];
    };
    a.keys()
        .chain(b.keys())
        .filter(|key| a.get(*key) != b.get(*key))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn asset_metadata(d: &Deck) -> BTreeMap<&str, Value> {
    d.playback
        .assets
        .iter()
        .map(|(id, asset)| {
            (
                id.as_str(),
                json!({"extension":asset.extension,"info":asset.info}),
            )
        })
        .collect()
}
pub fn compare(a: &Deck, b: &Deck) -> Result<Report> {
    a.validate()?;
    b.validate()?;
    if [a, b].into_iter().any(|d| {
        d.slides
            .iter()
            .flat_map(|s| &s.objects)
            .any(|o| o.id.is_empty())
    }) {
        return Err("文稿含缺少稳定ID的对象；请先在编辑器打开并另存为原生文稿后比较".into());
    }
    let mut report = Report {
        base_sha256: fingerprint(a)?,
        reference_sha256: fingerprint(b)?,
        differences: vec![],
        truncated: false,
    };
    let mut push =
        |kind: &str, slide: Option<String>, object: Option<String>, fields: Vec<String>| {
            if report.differences.len() < 4096 {
                report.differences.push(Difference {
                    kind: kind.into(),
                    slide,
                    object,
                    fields,
                });
            } else {
                report.truncated = true;
            }
        };
    let global = |d: &Deck| json!({"title":d.title,"width":d.width,"height":d.height,"design":d.design,"organization":d.organization,"review":d.review,"presentation":d.presentation,"playbackAssets":asset_metadata(d),"clips":d.playback.clips,"resources":d.resources.keys().collect::<Vec<_>>(),"fieldDate":d.field_date});
    let changed = fields(global(a), global(b));
    if !changed.is_empty() {
        push("document", None, None, changed);
    }
    let aid: Vec<_> = a.slides.iter().map(|s| &s.id).collect();
    let bid: Vec<_> = b.slides.iter().map(|s| &s.id).collect();
    if aid != bid {
        push("page.order", None, None, vec!["order".into()]);
    }
    let ma: BTreeMap<_, _> = a.slides.iter().map(|s| (&s.id, s)).collect();
    let mb: BTreeMap<_, _> = b.slides.iter().map(|s| (&s.id, s)).collect();
    for id in ma
        .keys()
        .chain(mb.keys())
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
    {
        let (Some(sa), Some(sb)) = (ma.get(id), mb.get(id)) else {
            push(
                if ma.contains_key(id) {
                    "page.deleted"
                } else {
                    "page.added"
                },
                Some(id.clone()),
                None,
                vec![],
            );
            continue;
        };
        let summary = |s: &crate::document::Slide| -> Result<Value> {
            let mut v = serde_json::to_value(s).map_err(|e| e.to_string())?;
            v.as_object_mut()
                .ok_or("页面摘要不是对象")?
                .remove("objects");
            Ok(v)
        };
        let changed = fields(summary(sa)?, summary(sb)?);
        if !changed.is_empty() {
            push("page", Some(id.clone()), None, changed);
        }
        if sa.objects.iter().map(|o| &o.id).collect::<Vec<_>>()
            != sb.objects.iter().map(|o| &o.id).collect::<Vec<_>>()
        {
            push("object.order", Some(id.clone()), None, vec!["order".into()]);
        }
        let oa: BTreeMap<_, _> = sa.objects.iter().map(|o| (&o.id, o)).collect();
        let ob: BTreeMap<_, _> = sb.objects.iter().map(|o| (&o.id, o)).collect();
        for oid in oa
            .keys()
            .chain(ob.keys())
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
        {
            match (oa.get(oid), ob.get(oid)) {
                (Some(a), Some(b)) => {
                    let f = fields(
                        serde_json::to_value(a).map_err(|e| e.to_string())?,
                        serde_json::to_value(b).map_err(|e| e.to_string())?,
                    );
                    if !f.is_empty() {
                        push("object.changed", Some(id.clone()), Some(oid.clone()), f);
                    }
                }
                (Some(_), None) => push(
                    "object.deleted",
                    Some(id.clone()),
                    Some(oid.clone()),
                    vec![],
                ),
                (None, Some(_)) => {
                    push("object.added", Some(id.clone()), Some(oid.clone()), vec![])
                }
                _ => {}
            }
        }
    }
    Ok(report)
}
pub fn accept_object(
    base: &Deck,
    reference: &Deck,
    report: &Report,
    index: usize,
) -> Result<Command> {
    if fingerprint(base)? != report.base_sha256
        || fingerprint(reference)? != report.reference_sha256
    {
        return Err("比较后文稿已改变，请重新比较".into());
    }
    let row = report.differences.get(index).ok_or("比较条目不存在")?;
    let slide = row.slide.as_ref().ok_or("只支持逐对象接受更改")?;
    let object = row.object.as_ref().ok_or("只支持逐对象接受更改")?;
    let page = base
        .slides
        .iter()
        .position(|s| &s.id == slide)
        .ok_or("目标页面不存在")?;
    let old = base.slides[page]
        .objects
        .iter()
        .position(|o| &o.id == object);
    let new = reference
        .slides
        .iter()
        .find(|s| &s.id == slide)
        .and_then(|s| s.objects.iter().find(|o| &o.id == object));
    // Resource/layout dependencies must already exist in the target. Session validation is atomic.
    match (row.kind.as_str(), old, new) {
        ("object.changed", Some(i), Some(o)) => Ok(Command::ReplaceObject(page, i, o.clone())),
        ("object.added", None, Some(o)) => {
            let objects = &base.slides[page].objects;
            let reference_objects = &reference
                .slides
                .iter()
                .find(|s| &s.id == slide)
                .ok_or("参考页面不存在")?
                .objects;
            let position = reference_objects
                .iter()
                .position(|v| &v.id == object)
                .ok_or("参考对象不存在")?;
            // Anchor against shared neighbours so accepting additions in any order
            // preserves their reference layers without reordering existing objects.
            let following = reference_objects[position + 1..]
                .iter()
                .find_map(|v| objects.iter().position(|old| old.id == v.id));
            let target = following
                .or_else(|| {
                    reference_objects[..position]
                        .iter()
                        .rev()
                        .find_map(|v| objects.iter().position(|old| old.id == v.id).map(|i| i + 1))
                })
                .unwrap_or(objects.len());
            Ok(Command::Batch(vec![
                Command::AdoptObject(page, o.clone()),
                Command::ReorderObjects(page, vec![objects.len()], target),
            ]))
        }
        ("object.deleted", Some(i), None) => Ok(Command::DeleteObject(page, i)),
        _ => Err("条目不能作为单个对象更改接受".into()),
    }
}

/// Opt-in image dependency transfer. Layout/theme, page and review merges stay
/// separate; a failing object edit also rolls back all transferred resources.
pub fn accept_object_with_resources(
    base: &Deck,
    reference: &Deck,
    report: &Report,
    index: usize,
) -> Result<Command> {
    let command = accept_object(base, reference, report, index)?;
    let row = report.differences.get(index).ok_or("比较条目不存在")?;
    let object = reference
        .slides
        .iter()
        .find(|s| Some(&s.id) == row.slide.as_ref())
        .and_then(|s| {
            s.objects
                .iter()
                .find(|o| Some(&o.id) == row.object.as_ref())
        });
    let Some(image) = object.and_then(|o| o.image.as_ref()) else {
        return Ok(command);
    };
    if base.resources.contains_key(&image.resource) {
        return Ok(command);
    }
    let resource = reference
        .resources
        .get(&image.resource)
        .ok_or("参考图片资源不存在")?;
    Ok(Command::Batch(vec![
        Command::MergeResources(BTreeMap::from([(image.resource.clone(), resource.clone())])),
        command,
    ]))
}
