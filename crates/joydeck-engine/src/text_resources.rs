//! Staged text plus embedded picture resources. No UI, I/O or content execution.
use crate::{Command, EditorSnapshot, TextTarget, authoring, media_refs, text, transfer::Resource};
use joydeck_model::{MediaId, Presentation, TextBody};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

fn refs(body: &TextBody) -> BTreeSet<MediaId> {
    let mut ids = BTreeSet::new();
    media_refs::visit::text(body, &mut |id| {
        ids.insert(*id);
    });
    ids
}

/// Capture only referenced embedded images; source and destination stay unchanged.
/// Document-local links must be scoped separately by the clipboard caller.
pub fn capture(source: &Presentation, body: &TextBody) -> Result<Vec<Resource>, String> {
    text::validate(body)?;
    let ids = refs(body);
    if ids.len() > 512 {
        return Err("too many text image resources".into());
    }
    let mut total = 0usize;
    let mut resources = Vec::new();
    for id in ids {
        let media = source.media(id).ok_or("text image resource missing")?;
        if media.link.is_some() {
            return Err("linked text images must be embedded first".into());
        }
        total = total
            .checked_add(media.data.len())
            .ok_or("text image budget exceeded")?;
        if total > 16 * 1024 * 1024 {
            return Err("text image budget exceeded".into());
        }
        resources.push(Resource {
            id,
            name: media.name.clone(),
            content_type: media.content_type.clone(),
            data: media.data.as_ref().clone(),
        });
    }
    validate(body, &resources)?;
    Ok(resources)
}

fn validate(body: &TextBody, resources: &[Resource]) -> Result<(), String> {
    text::validate(body)?;
    if resources.len() > 512 {
        return Err("too many text image resources".into());
    }
    let mut total = 0usize;
    let mut ids = BTreeSet::new();
    for resource in resources {
        total = total
            .checked_add(resource.data.len())
            .ok_or("text image budget exceeded")?;
        if total > 16 * 1024 * 1024
            || resource.data.is_empty()
            || !resource.content_type.starts_with("image/")
            || !ids.insert(resource.id)
        {
            return Err("invalid or excessive text image resources".into());
        }
    }
    if ids != refs(body) {
        return Err("text image resources missing or unreferenced".into());
    }
    Ok(())
}

/// Build a pending command without changing the model/history. Cancelling means
/// dropping this command. Pass the complete final draft, not just the insertion.
/// Every media reference in that draft belongs to the supplied resource ID space.
pub fn prepare(
    snapshot: &EditorSnapshot,
    target: TextTarget,
    body: TextBody,
    resources: Vec<Resource>,
) -> Result<Command, String> {
    validate(&body, &resources)?;
    let slide = snapshot
        .model
        .slides
        .get(snapshot.selection.page)
        .ok_or("missing slide")?
        .id;
    let expected = authoring::body(&snapshot.model, snapshot.selection.page, &target)?;
    Ok(Command::SetTextWithResources {
        slide,
        target,
        expected: Box::new(expected),
        body: Box::new(body),
        resources,
    })
}

pub(crate) fn apply(
    snapshot: &mut EditorSnapshot,
    slide: joydeck_model::SlideId,
    target: &TextTarget,
    expected: &TextBody,
    body: &TextBody,
    resources: &[Resource],
) -> Result<(), String> {
    validate(body, resources)?;
    let page = snapshot.selection.page;
    if snapshot.model.slides.get(page).map(|s| s.id) != Some(slide)
        || authoring::body(&snapshot.model, page, target)? != *expected
    {
        return Err("text resource draft is stale".into());
    }
    let p = Arc::make_mut(&mut snapshot.model);
    let mut mapping = BTreeMap::new();
    for resource in resources {
        let id = p
            .add_media(
                &resource.name,
                &resource.content_type,
                resource.data.clone(),
            )
            .ok_or("media ID exhausted")?;
        mapping.insert(resource.id, id);
    }
    let mut remapped = body.clone();
    media_refs::visit_mut::text(&mut remapped, &mut |id| {
        if let Some(mapped) = mapping.get(id) {
            *id = *mapped;
        }
    });
    authoring::set_body(p, page, target, remapped)
}
