//! JoyDeck v2: explicit capabilities, model payload and exact source package for preservation.
use crate::{Cancellation, Document, document::MAX_INPUT};
use joydeck_engine::{EditorSnapshot, Origins};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
};
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
pub fn parts(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    parts_cancellable(bytes, &Cancellation::default())
}
pub(crate) fn parts_cancellable(
    bytes: &[u8],
    cancel: &Cancellation,
) -> Result<BTreeMap<String, Vec<u8>>, String> {
    cancel.check()?;
    if bytes.len() as u64 > MAX_INPUT {
        return Err("native input exceeds 64 MiB".into());
    }
    let budget = joyoffice_opc::Budget {
        max_entries: 4096,
        max_part_bytes: 64 << 20,
        max_total_bytes: 128 << 20,
        ..Default::default()
    };
    let z = joyoffice_opc::ZipArchive::new(bytes, &budget)
        .map_err(|e| format!("invalid native package: {e:?}"))?;
    let mut entries = BTreeMap::new();
    for (i, e) in z.entries().iter().enumerate() {
        cancel.check()?;
        if e.name.contains(['\\', ':'])
            || e.name
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
            || entries.contains_key(&e.name)
        {
            return Err("invalid/duplicate native part path".into());
        }
        entries.insert(
            e.name.clone(),
            z.read(i)
                .map_err(|e| format!("invalid native part: {e:?}"))?,
        );
    }
    Ok(entries)
}
pub fn sniff(bytes: &[u8]) -> bool {
    if !bytes.starts_with(b"PK") || bytes.len() as u64 > MAX_INPUT {
        return false;
    }
    let budget = joyoffice_opc::Budget {
        max_entries: 4096,
        max_part_bytes: 64 << 20,
        max_total_bytes: 128 << 20,
        ..Default::default()
    };
    let Ok(archive) = joyoffice_opc::ZipArchive::new(bytes, &budget) else {
        return false;
    };
    archive
        .entries()
        .iter()
        .enumerate()
        .find(|(_, e)| e.name == "mimetype" && e.uncompressed_size < 128)
        .is_some_and(|(i, _)| {
            archive
                .read(i)
                .is_ok_and(|s| s == b"application/vnd.joyoffice.joydeck+zip")
        })
}
pub fn load(bytes: &[u8], cancel: &Cancellation) -> Result<Loaded, String> {
    let mut entries = parts_cancellable(bytes, cancel)?;
    if entries.remove("mimetype").as_deref() != Some(b"application/vnd.joyoffice.joydeck+zip") {
        return Err("not a JoyDeck package".into());
    }
    let manifest_bytes = entries
        .remove("manifest.json")
        .ok_or("missing native manifest")?;
    if manifest_bytes.len() > 1 << 20 {
        return Err("native manifest exceeds 1 MiB".into());
    }
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
    let model = entries
        .remove("model.deckcraft")
        .ok_or("missing model payload")?;
    let source = entries
        .remove("source.package")
        .ok_or("missing source package")?;
    if sniff(&model) || sniff(&source) {
        return Err("recursive native sources are not supported".into());
    }
    let doc = Document::from_bytes("model.deckcraft".into(), model, cancel)?;
    let base = Document::from_bytes("source.package".into(), source.clone(), cancel)?;
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
    Ok(Loaded {
        model: doc.presentation,
        source,
        manifest,
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
        .get("manifest.json")
        .and_then(|b| serde_json::from_slice::<Manifest>(b).ok())
        .map(|m| m.extra)
        .unwrap_or_default();
    if !entries.contains_key("retained-original-model.deckcraft")
        && let Some(original) = entries.get("model.deckcraft").cloned()
    {
        entries.insert("retained-original-model.deckcraft".into(), original);
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
        "manifest.json".into(),
        serde_json::to_vec(&manifest).map_err(|e| e.to_string())?,
    );
    entries.insert(
        "model.deckcraft".into(),
        joydeck_format::save(&snapshot.model).map_err(|e| e.to_string())?,
    );
    entries.insert("source.package".into(), source.to_vec());
    pack(entries)
}

pub(crate) fn pack(mut entries: BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>, String> {
    entries.remove("mimetype");
    if entries.len() + 1 > 4096
        || entries.values().any(|b| b.len() as u64 > MAX_INPUT)
        || entries.values().map(|b| b.len() as u64).sum::<u64>() > 128 << 20
    {
        return Err("project/recovery payload exceeds native part budget".into());
    }
    if entries
        .get("manifest.json")
        .is_some_and(|b| b.len() > 1 << 20)
    {
        return Err("native manifest exceeds 1 MiB".into());
    }
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    z.start_file(
        "mimetype",
        opts.compression_method(zip::CompressionMethod::Stored),
    )
    .map_err(|e| e.to_string())?;
    z.write_all(b"application/vnd.joyoffice.joydeck+zip")
        .map_err(|e| e.to_string())?;
    for (name, bytes) in entries {
        z.start_file(name, opts).map_err(|e| e.to_string())?;
        z.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    let bytes = z.finish().map_err(|e| e.to_string())?.into_inner();
    if bytes.len() as u64 > MAX_INPUT {
        return Err("native output exceeds 64 MiB".into());
    }
    Ok(bytes)
}
