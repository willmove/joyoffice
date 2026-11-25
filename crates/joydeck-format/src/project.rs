//! Read and write the model archive used by historical DeckCraft v1 files and JoyDeck projects.

use std::collections::{BTreeMap, HashSet};
use std::io::{Cursor, Read, Write};
use std::sync::Arc;

use joydeck_model::chart::ChartPart;
use joydeck_model::{Master, Presentation, Shape, ShapeKind};
use serde::{Deserialize, Serialize};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{EXTENSION, FormatError, MIMETYPE, Result, VERSION, finite::Finite, repair};

const METADATA: &str = "presentation.json";
const MEDIA: &str = "media";
const CHARTS: &str = "chart-parts";
const ENTRY_LIMIT: u64 = 2 << 30;
const DECODE_STACK: usize = 16 << 20;

#[derive(Clone, Copy)]
pub(super) struct Budget {
    pub(super) total: u64,
    pub(super) items: usize,
}

const LIMITS: Budget = Budget {
    total: 4 << 30,
    items: 100_000,
};

#[derive(Serialize, Deserialize)]
pub(super) struct Manifest {
    pub(super) format: String,
    pub(super) version: u32,
    pub(super) generator: String,
    pub(super) presentation: Presentation,
}

pub fn save(presentation: &Presentation) -> Result<Vec<u8>> {
    let mut model = presentation.clone();
    let mut sidecars = Vec::new();
    visit_charts(&mut model, |part| {
        let name = format!("{CHARTS}/{}", sidecars.len());
        part.native_entry = Some(name.clone());
        sidecars.push((
            name,
            part.content_type.clone(),
            std::mem::take(&mut part.bytes),
        ));
        Ok(())
    })?;
    let metadata = serde_json::to_vec(&Finite(&Manifest {
        format: EXTENSION.into(),
        version: VERSION,
        generator: format!("JoyDeck {}", env!("CARGO_PKG_VERSION")),
        presentation: model,
    }))
    .map_err(write_error)?;

    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    append(&mut archive, "mimetype", MIMETYPE.as_bytes(), true)?;
    append(&mut archive, METADATA, &metadata, false)?;
    for item in &presentation.media {
        if !item.data.is_empty() || item.link.is_none() {
            append(
                &mut archive,
                &format!("{MEDIA}/{}", item.id.0),
                &item.data,
                compressed_media(&item.content_type),
            )?;
        }
    }
    for (name, content_type, bytes) in sidecars {
        append(&mut archive, &name, &bytes, compressed_media(&content_type))?;
    }
    Ok(archive.finish().map_err(write_error)?.into_inner())
}

fn write_error(error: impl std::fmt::Display) -> FormatError {
    FormatError::Write(error.to_string())
}

fn append(
    out: &mut ZipWriter<Cursor<Vec<u8>>>,
    path: &str,
    data: &[u8],
    store: bool,
) -> Result<()> {
    let method = if store {
        CompressionMethod::Stored
    } else {
        CompressionMethod::Deflated
    };
    out.start_file(
        path,
        SimpleFileOptions::default().compression_method(method),
    )
    .map_err(write_error)?;
    out.write_all(data).map_err(write_error)
}

fn compressed_media(content_type: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "image/jpeg",
        "image/png",
        "video/",
        "audio/mpeg",
        "audio/mp4",
    ];
    PREFIXES
        .iter()
        .any(|prefix| content_type.starts_with(prefix))
        || content_type.contains("zip")
}

enum Members<'a> {
    Zip(ZipArchive<Cursor<&'a [u8]>>),
    Checked(BTreeMap<String, Vec<u8>>),
}

impl Members<'_> {
    fn size(&mut self, name: &str) -> Option<u64> {
        match self {
            Self::Zip(zip) => zip.by_name(name).ok().map(|part| part.size()),
            Self::Checked(parts) => parts.get(name).map(|bytes| bytes.len() as u64),
        }
    }

    fn contains(&self, name: &str) -> bool {
        match self {
            Self::Zip(zip) => zip.index_for_name(name).is_some(),
            Self::Checked(parts) => parts.contains_key(name),
        }
    }

    fn take(&mut self, name: &str, limit: u64) -> Result<Vec<u8>> {
        match self {
            Self::Zip(zip) => read_entry_max(zip, name, limit),
            Self::Checked(parts) => {
                if parts
                    .get(name)
                    .is_some_and(|bytes| bytes.len() as u64 > limit)
                {
                    return Err(FormatError::Damaged(format!("{name} is too large")));
                }
                parts.remove(name).ok_or_else(|| {
                    FormatError::Damaged(format!("{name}: specified file not found in archive"))
                })
            }
        }
    }
}

pub(super) fn read_entry_max(
    zip: &mut ZipArchive<Cursor<&[u8]>>,
    name: &str,
    max: u64,
) -> Result<Vec<u8>> {
    let damaged = |error: &dyn std::fmt::Display| FormatError::Damaged(format!("{name}: {error}"));
    let member = zip.by_name(name).map_err(|error| damaged(&error))?;
    if member.size() > max {
        return Err(FormatError::Damaged(format!("{name} is too large")));
    }
    let mut bytes = Vec::with_capacity(member.size().min(16 << 20) as usize);
    member
        .take(max.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| damaged(&error))?;
    if bytes.len() as u64 > max {
        return Err(FormatError::Damaged(format!("{name} is too large")));
    }
    Ok(bytes)
}

#[cfg(test)]
pub(super) fn read_entry(zip: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>> {
    read_entry_max(zip, name, ENTRY_LIMIT)
}

pub fn load_checked(bytes: &[u8]) -> Result<(Presentation, Vec<String>)> {
    load_within(bytes, LIMITS)
}

pub(super) fn load_within(bytes: &[u8], budget: Budget) -> Result<(Presentation, Vec<String>)> {
    let archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|e| FormatError::NotOurs(e.to_string()))?;
    decode(Members::Zip(archive), budget, false)
}

/// Use the same reader for members whose lengths and CRCs the host package reader validated.
pub fn load_parts(parts: BTreeMap<String, Vec<u8>>) -> Result<(Presentation, Vec<String>)> {
    decode(Members::Checked(parts), LIMITS, true)
}

fn decode(
    mut members: Members,
    budget: Budget,
    require_version: bool,
) -> Result<(Presentation, Vec<String>)> {
    let json = members
        .take(METADATA, ENTRY_LIMIT)
        .map_err(|_| FormatError::NotOurs("no presentation.json".into()))?;
    if require_version {
        let header: ManifestHeader<'_> = serde_json::from_slice(&json)
            .map_err(|e| FormatError::Damaged(format!("invalid native manifest: {e}")))?;
        if header.format.as_deref() != Some(EXTENSION) || header.version != Some(VERSION) {
            return Err(FormatError::NotOurs(
                "unsupported .deckcraft format/version (only version 1 is supported)".into(),
            ));
        }
    }
    let manifest = std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(DECODE_STACK)
            .spawn_scoped(scope, || crate::legacy_numbers::decode(&json))
            .map_err(|e| FormatError::Damaged(format!("cannot start manifest decoder: {e}")))?
            .join()
            .map_err(|_| FormatError::Damaged("manifest decoder panicked".into()))?
    })?;
    if manifest.format != EXTENSION {
        return Err(FormatError::NotOurs(manifest.format));
    }
    let mut model = manifest.presentation;
    let mut remaining = budget;
    let mut warnings = Vec::new();
    read_media(&mut model, &mut members, &mut remaining, &mut warnings)?;
    read_sidecars(
        &mut model,
        &mut members,
        &mut remaining,
        budget.items,
        &mut warnings,
    )?;
    repair(&mut model);
    Ok((model, warnings))
}

#[derive(Deserialize)]
struct ManifestHeader<'a> {
    #[serde(default, borrow)]
    format: Option<std::borrow::Cow<'a, str>>,
    version: Option<u32>,
}

fn read_media(
    model: &mut Presentation,
    members: &mut Members,
    budget: &mut Budget,
    warnings: &mut Vec<String>,
) -> Result<()> {
    budget.items = budget.items.checked_sub(model.media.len()).ok_or_else(|| {
        FormatError::Damaged(format!(
            "{} media items (at most {})",
            model.media.len(),
            budget.items
        ))
    })?;
    let mut ids = HashSet::new();
    model.media.retain(|media| ids.insert(media.id));
    for media in &mut model.media {
        let name = format!("{MEDIA}/{}", media.id.0);
        if media.link.is_some() && !members.contains(&name) {
            continue;
        }
        let declared = members.size(&name);
        if declared.is_some_and(|size| size > budget.total) {
            return Err(FormatError::Damaged(
                "embedded media is larger than DeckCraft can open".into(),
            ));
        }
        match members.take(&name, budget.total.min(ENTRY_LIMIT)) {
            Ok(bytes) => {
                budget.total -= bytes.len() as u64;
                media.data = Arc::new(bytes);
            }
            Err(error) => {
                // A corrupt optional entry can have consumed work before the checksum failed.
                budget.total = budget.total.saturating_sub(declared.unwrap_or_default());
                warnings.push(error.to_string());
            }
        }
    }
    Ok(())
}

fn read_sidecars(
    model: &mut Presentation,
    members: &mut Members,
    budget: &mut Budget,
    max_items: usize,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let mut used = HashSet::new();
    visit_charts(model, |part| {
        budget.items = budget.items.checked_sub(1).ok_or_else(|| {
            FormatError::Damaged(format!("media and chart parts exceed {max_items} items"))
        })?;
        let Some(name) = part.native_entry.take() else {
            let size = part.bytes.len() as u64;
            if size > ENTRY_LIMIT.min(budget.total) {
                return Err(FormatError::Damaged(
                    "inline chart part is larger than DeckCraft can open".into(),
                ));
            }
            budget.total -= size;
            return Ok(());
        };
        let serial = name
            .strip_prefix("chart-parts/")
            .and_then(|s| s.parse::<usize>().ok());
        if !serial.is_some_and(|serial| name == format!("{CHARTS}/{serial}"))
            || !used.insert(name.clone())
            || !part.bytes.is_empty()
        {
            return Err(FormatError::Damaged(
                "invalid or ambiguous chart part reference".into(),
            ));
        }
        if !members.contains(&name) {
            warnings.push(format!("{name}: missing chart part"));
            return Ok(());
        }
        if members.size(&name).is_some_and(|size| size > budget.total) {
            return Err(FormatError::Damaged(
                "embedded media and chart parts are larger than DeckCraft can open".into(),
            ));
        }
        part.bytes = members.take(&name, ENTRY_LIMIT.min(budget.total))?;
        budget.total -= part.bytes.len() as u64;
        Ok(())
    })
}

fn visit_charts(
    model: &mut Presentation,
    mut visit: impl FnMut(&mut ChartPart) -> Result<()>,
) -> Result<()> {
    let mut scan = |shapes: &mut [Shape]| -> Result<()> {
        let mut todo = vec![shapes.iter_mut()];
        while let Some(mut branch) = todo.pop() {
            for shape in branch.by_ref() {
                match &mut shape.kind {
                    ShapeKind::Group { children, .. } => todo.push(children.iter_mut()),
                    ShapeKind::Chart(chart) => {
                        for part in &mut chart.kept.parts {
                            visit(part)?;
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    };
    for slide in &mut model.slides {
        scan(&mut Arc::make_mut(slide).shapes)?;
    }
    for master in model
        .masters
        .iter_mut()
        .chain(model.notes_master.iter_mut())
        .chain(model.handout_master.iter_mut())
        .map(Arc::make_mut)
    {
        let Master {
            shapes, layouts, ..
        } = master;
        scan(shapes)?;
        for layout in layouts {
            scan(&mut layout.shapes)?;
        }
    }
    Ok(())
}
