//! JoyDeck v2: explicit capabilities, model payload and exact source package for preservation.
use crate::{Cancellation, Document, document::MAX_NATIVE_INPUT};
use joydeck_engine::{EditorSnapshot, Origins};
use joyoffice_opc::{Budget, Method, ZipArchive};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

const MIME: &[u8] = b"application/vnd.joyoffice.joydeck+zip";
const MANIFEST: &str = "manifest.json";
const MODEL: &str = "model.deckcraft";
const SOURCE: &str = "source.package";
const MODEL_BACKUP: &str = "retained-original-model.deckcraft";
const REPORT: &str = "migration/report.json";
const MAX_MANIFEST: usize = 1 << 20;
const MAX_TOTAL: u64 = 128 << 20;
const MAX_ENTRIES: u64 = 4096;
const FEATURES: &[&str] = &["model-v1", "source-package-v1", "origin-bindings-v1"];
#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub required_features: Vec<String>,
    pub source_format: String,
    pub origins: Origins,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
pub struct Loaded {
    pub model: joydeck_model::Presentation,
    pub source: Vec<u8>,
    pub manifest: Manifest,
}

pub(crate) struct DecodedProject {
    pub model: joydeck_model::Presentation,
    pub source: Arc<Document>,
    pub manifest: Manifest,
    pub migration_warnings: Vec<String>,
}

pub(crate) struct ProjectOrigin {
    pub source: Arc<Document>,
    pub bindings: Origins,
    pub migration_warnings: Vec<String>,
}

fn directory(bytes: &[u8]) -> Result<ZipArchive<'_>, String> {
    if bytes.len() as u64 > MAX_NATIVE_INPUT {
        return Err("native input exceeds 64 MiB".into());
    }
    ZipArchive::new(
        bytes,
        &Budget {
            max_entries: MAX_ENTRIES,
            max_part_bytes: MAX_NATIVE_INPUT,
            max_total_bytes: MAX_TOTAL,
            ..Default::default()
        },
    )
    .map_err(|e| format!("invalid native package: {e:?}"))
}

fn valid_member(name: &str) -> bool {
    !name.contains(['\\', ':', '\0'])
        && name.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

pub fn parts(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    parts_cancellable(bytes, &Cancellation::default())
}
pub(crate) fn parts_cancellable(
    bytes: &[u8],
    cancel: &Cancellation,
) -> Result<BTreeMap<String, Vec<u8>>, String> {
    cancel.check()?;
    let archive = directory(bytes)?;
    let indices = index_members(&archive, cancel)?;
    read_members(&archive, indices, cancel)
}

fn index_members(
    archive: &ZipArchive,
    cancel: &Cancellation,
) -> Result<BTreeMap<String, usize>, String> {
    let mut indices = BTreeMap::new();
    for (i, entry) in archive.entries().iter().enumerate() {
        cancel.check()?;
        if entry.is_symlink() {
            return Err("symlink native package entries are unsupported".into());
        }
        if !valid_member(&entry.name) || indices.insert(entry.name.clone(), i).is_some() {
            return Err("invalid/duplicate native part path".into());
        }
    }
    Ok(indices)
}

fn read_members(
    archive: &ZipArchive,
    indices: BTreeMap<String, usize>,
    cancel: &Cancellation,
) -> Result<BTreeMap<String, Vec<u8>>, String> {
    indices
        .into_iter()
        .map(|(name, index)| {
            cancel.check()?;
            archive
                .read(index)
                .map(|bytes| (name, bytes))
                .map_err(|e| format!("invalid native part: {e:?}"))
        })
        .collect()
}

pub fn sniff(bytes: &[u8]) -> bool {
    if !bytes.starts_with(b"PK") {
        return false;
    }
    let Ok(archive) = directory(bytes) else {
        return false;
    };
    archive
        .entries()
        .iter()
        .enumerate()
        .find(|(_, e)| e.name == "mimetype" && e.uncompressed_size < 128)
        .is_some_and(|(i, _)| archive.read(i).is_ok_and(|s| s == MIME))
}

pub fn load(bytes: &[u8], cancel: &Cancellation) -> Result<Loaded, String> {
    let project = decode(bytes, cancel)?;
    Ok(Loaded {
        model: project.model,
        source: project.source.original.to_vec(),
        manifest: project.manifest,
    })
}

pub(crate) fn decode(bytes: &[u8], cancel: &Cancellation) -> Result<DecodedProject, String> {
    let archive = directory(bytes)?;
    let mut indices = index_members(&archive, cancel)?;
    let mime = indices
        .remove("mimetype")
        .filter(|&index| archive.entries()[index].uncompressed_size == MIME.len() as u64)
        .map(|index| archive.read(index))
        .transpose()
        .map_err(|e| format!("invalid native part: {e:?}"))?;
    if mime.as_deref() != Some(MIME) {
        return Err("not a JoyDeck package".into());
    }
    let manifest_index = indices.remove(MANIFEST).ok_or("missing native manifest")?;
    if archive.entries()[manifest_index].uncompressed_size > MAX_MANIFEST as u64 {
        return Err("native manifest exceeds 1 MiB".into());
    }
    let manifest_bytes = archive
        .read(manifest_index)
        .map_err(|e| format!("invalid native part: {e:?}"))?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes).map_err(|e| e.to_string())?;
    if manifest.format != "joydeck"
        || manifest.version != 2
        || manifest
            .required_features
            .iter()
            .any(|f| !FEATURES.contains(&f.as_str()))
    {
        return Err("unsupported JoyDeck version/required capability; file kept unchanged".into());
    }
    let mut entries = read_members(&archive, indices, cancel)?;
    let model = entries.remove(MODEL).ok_or("missing model payload")?;
    let source = entries.remove(SOURCE).ok_or("missing source package")?;
    if sniff(&model) || sniff(&source) {
        return Err("recursive native sources are not supported".into());
    }
    let doc = Document::from_bytes(MODEL.into(), model, cancel)?;
    let base = Document::from_bytes("original source".into(), source, cancel)?;
    if base.info.format != manifest.source_format {
        return Err("source format/binding mismatch".into());
    }
    for (slide_id, origin) in &manifest.origins {
        cancel.check()?;
        let slide = doc
            .presentation
            .slide(*slide_id)
            .ok_or("native binding refers to a missing slide")?;
        let original = base
            .presentation
            .slide(origin.slide)
            .ok_or("native binding refers to a missing source slide")?;
        if origin
            .shapes
            .iter()
            .any(|(id, source)| slide.shape(*id).is_none() || original.shape(*source).is_none())
        {
            return Err("native binding refers to a missing shape".into());
        }
    }
    let migration_warnings = if let Some(bytes) = entries.remove(REPORT) {
        let report: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        report
            .get("warnings")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|warning| warning.as_str().map(str::to_owned))
            .collect()
    } else {
        Vec::new()
    };
    Ok(DecodedProject {
        model: doc.presentation,
        source: Arc::new(base),
        manifest,
        migration_warnings,
    })
}
pub fn encode(
    snapshot: &EditorSnapshot,
    source: &[u8],
    source_format: &str,
    template: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    let mut entries = if let Some(bytes) = template.filter(|b| sniff(b)) {
        parts(bytes)?
    } else {
        BTreeMap::new()
    };
    let extra = entries
        .get(MANIFEST)
        .and_then(|b| serde_json::from_slice::<Manifest>(b).ok())
        .map(|m| m.extra)
        .unwrap_or_default();
    if !entries.contains_key(MODEL_BACKUP)
        && let Some(original) = entries.get(MODEL).cloned()
    {
        entries.insert(MODEL_BACKUP.into(), original);
    }
    let manifest = Manifest {
        extra,
        format: "joydeck".into(),
        version: 2,
        required_features: FEATURES.iter().map(|s| s.to_string()).collect(),
        source_format: source_format.into(),
        origins: snapshot.origins.clone(),
    };
    entries.insert(
        MANIFEST.into(),
        serde_json::to_vec(&manifest).map_err(|e| e.to_string())?,
    );
    entries.insert(
        MODEL.into(),
        joydeck_format::save(&snapshot.model).map_err(|e| e.to_string())?,
    );
    entries.insert(SOURCE.into(), source.to_vec());
    pack(entries)
}

pub(crate) fn pack(mut entries: BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>, String> {
    entries.remove("mimetype");
    if entries.len() as u64 >= MAX_ENTRIES
        || entries.values().any(|b| b.len() as u64 > MAX_NATIVE_INPUT)
        || MIME.len() as u64 + entries.values().map(|b| b.len() as u64).sum::<u64>() > MAX_TOTAL
    {
        return Err("project/recovery payload exceeds native part budget".into());
    }
    if entries
        .get(MANIFEST)
        .is_some_and(|b| b.len() > MAX_MANIFEST)
    {
        return Err("native manifest exceeds 1 MiB".into());
    }
    let mut members = vec![("mimetype", MIME, Method::Stored)];
    for (name, bytes) in &entries {
        if !valid_member(name) {
            return Err("invalid native part path".into());
        }
        members.push((name, bytes, Method::Deflated));
    }
    let bytes = joyoffice_opc::write_archive(&members).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_NATIVE_INPUT {
        return Err("native output exceeds 64 MiB".into());
    }
    Ok(bytes)
}
