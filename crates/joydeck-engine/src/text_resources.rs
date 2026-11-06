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

/// Private resource namespace for a complete text editing draft. No model writes
/// occur until `finish` is executed. Document-local links must already be scoped.
#[derive(Clone, Debug)]
pub struct Draft {
    slide: joydeck_model::SlideId,
    target: TextTarget,
    expected: TextBody,
    body: TextBody,
    resources: Vec<Resource>,
}

impl Draft {
    pub fn begin(snapshot: &EditorSnapshot, target: TextTarget) -> Result<Self, String> {
        let body = authoring::body(&snapshot.model, snapshot.selection.page, &target)?;
        let resources = capture(&snapshot.model, &body)?;
        let slide = snapshot.model.slides[snapshot.selection.page].id;
        Ok(Self {
            slide,
            target,
            expected: body.clone(),
            body,
            resources,
        })
    }

    pub fn body(&self) -> &TextBody {
        &self.body
    }

    /// Paste a separately scoped fragment at a UTF-8 byte range using the existing
    /// rich-text insertion semantics (including its paragraph formatting rules).
    /// On failure the entire draft is unchanged. Returned offset is the new caret.
    pub fn paste(
        &mut self,
        range: std::ops::Range<usize>,
        fragment: &TextBody,
        resources: &[Resource],
    ) -> Result<usize, String> {
        validate(fragment, resources)?;
        let value = self.body.text();
        if range.start > range.end
            || range.end > value.len()
            || !value.is_char_boundary(range.start)
            || !value.is_char_boundary(range.end)
        {
            return Err("invalid UTF-8 replacement range".into());
        }
        let range = text::field_range(&self.body, range);
        if value
            .len()
            .saturating_sub(range.len())
            .saturating_add(fragment.text().len())
            > 65536
        {
            return Err("text body exceeds editing budget".into());
        }
        let mut staged = self.resources.clone();
        let mut used: BTreeSet<_> = staged.iter().map(|r| r.id).collect();
        let mut mapping = BTreeMap::new();
        for resource in resources {
            let id = if let Some(existing) = staged
                .iter()
                .find(|r| r.content_type == resource.content_type && r.data == resource.data)
            {
                existing.id
            } else {
                let mut next = 1u32;
                while used.contains(&MediaId(next)) {
                    next = next.checked_add(1).ok_or("draft media ID exhausted")?;
                }
                let id = MediaId(next);
                let mut copy = resource.clone();
                copy.id = id;
                staged.push(copy);
                used.insert(id);
                id
            };
            mapping.insert(resource.id, id);
        }
        let mut fragment = fragment.clone();
        media_refs::visit_mut::text(&mut fragment, &mut |id| {
            if let Some(mapped) = mapping.get(id) {
                *id = *mapped;
            }
        });
        let mut body = self.body.clone();
        let a = text::pos(&value, range.start);
        let b = text::pos(&value, range.end);
        joydeck_model::edit::delete(&mut body, a, b);
        let caret = joydeck_model::edit::insert_body(&mut body, a, &fragment);
        let retained = refs(&body);
        staged.retain(|resource| retained.contains(&resource.id));
        validate(&body, &staged)?;
        let at = text::byte_pos(&body.text(), caret);
        self.body = body;
        self.resources = staged;
        Ok(at)
    }

    /// Retains the original target guard, even after several pastes. Execute once
    /// to publish the entire draft as one undoable edit; drop to cancel.
    pub fn finish(self) -> Command {
        Command::SetTextWithResources {
            slide: self.slide,
            target: self.target,
            expected: Box::new(self.expected),
            body: Box::new(self.body),
            resources: self.resources,
        }
    }
}
