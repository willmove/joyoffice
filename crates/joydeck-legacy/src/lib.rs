//! One-time legacy reader. These DTOs never become editor state in JoyDeck v2.
#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "Pinned read-only DTO/format snapshot retains source-compatible validation helpers"
)]
#[path = "model/lib.rs"]
mod model;
use model::{
    chart, design, diagram, identity, interop, media, motion, playback, preset, review, show,
    table, text,
};
mod chart_package;
mod diagram_package;
mod document;
mod motion_package;
mod native;
mod pptx;
mod pptx_write;
mod preset_pptx;
mod table_pptx;

use serde::Serialize;
use std::{collections::BTreeMap, sync::Arc};
use std::{
    io::{Cursor, Write},
    sync::atomic::AtomicBool,
};

#[derive(Serialize)]
pub struct Conversion {
    #[serde(skip)]
    pub pptx: Vec<u8>,
    pub schema_minor: u64,
    pub source_ids: Vec<SourceIds>,
    pub warnings: Vec<String>,
    #[serde(skip)]
    pub supplement: Supplement,
}
/// Validated, read-only migration output. It is never a second editor/session.
pub struct Supplement {
    pub threads: Vec<model::review::Thread>,
    pub accessibility: BTreeMap<String, model::review::Access>,
    pub reading_order: BTreeMap<String, Vec<String>>,
    pub clips: Vec<model::playback::Clip>,
    pub assets: BTreeMap<String, Arc<model::playback::Asset>>,
    pub links: BTreeMap<String, String>,
    pub order: Vec<String>,
    pub loop_show: bool,
}
pub type SourceIds = (usize, String, Vec<(String, usize)>);
pub fn convert(bytes: &[u8], cancel: &AtomicBool) -> Result<Conversion, String> {
    let mut parts = document::read_package_bytes(bytes, cancel)?;
    native::validate_editable(&parts)?;
    let manifest: serde_json::Value =
        serde_json::from_str(document::part(&parts, "manifest.json")?)
            .map_err(|e| e.to_string())?;
    let deck = document::decode_parts(&parts, cancel)?;
    let mut warnings = pptx_write::losses(&deck);
    warnings.retain(|w| {
        !w.contains("媒体仅在JoyDeck原生文件")
            && !w.contains("评论与阅读顺序仅在JoyDeck")
            && !w.contains("对象放映链接在JoyDeck")
    });
    warnings.push("One-time v1 conversion resolves legacy design/fields to public OOXML; exact old archive, source package and IDs are retained in the v2 migration attachment. Native inheritance is not reconstructed.".into());
    document::check_cancel(cancel)?;
    parts = pptx_write::generate(&deck)?;
    // Only public OOXML is fed to the new reader; never trust the old private model snapshot.
    parts.remove("customXml/joydeck.xml");
    if let Some(rels) = parts.get_mut("ppt/_rels/presentation.xml.rels") {
        let xml = String::from_utf8(rels.clone()).map_err(|e| e.to_string())?;
        let start = xml
            .find("<Relationship Id=\"snapshot\"")
            .ok_or("missing snapshot relationship")?;
        let end = xml[start..]
            .find("/>")
            .ok_or("invalid snapshot relationship")?
            + start
            + 2;
        *rels = format!("{}{}", &xml[..start], &xml[end..]).into_bytes();
    }
    let source_ids = deck
        .slides
        .iter()
        .enumerate()
        .map(|(i, s)| {
            (
                i + 1,
                s.id.clone(),
                s.objects
                    .iter()
                    .enumerate()
                    .map(|(j, o)| (o.id.clone(), j + 2))
                    .collect(),
            )
        })
        .collect();
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in parts {
        document::check_cancel(cancel)?;
        z.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .map_err(|e| e.to_string())?;
        z.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    let pptx = z.finish().map_err(|e| e.to_string())?.into_inner();
    if pptx.len() > 64 << 20 {
        return Err("converted PPTX exceeds 64 MiB".into());
    }
    Ok(Conversion {
        pptx,
        schema_minor: manifest["schemaMinor"]
            .as_u64()
            .ok_or("missing schemaMinor")?,
        source_ids,
        warnings,
        supplement: Supplement {
            threads: deck.review.threads.clone(),
            accessibility: deck.review.accessibility.clone(),
            reading_order: deck.review.reading_order.clone(),
            clips: deck.playback.clips.clone(),
            assets: deck.playback.assets.clone(),
            links: deck.presentation.links.clone(),
            order: deck.presentation.order.clone(),
            loop_show: deck.presentation.loop_show,
        },
    })
}

pub fn sniff(bytes: &[u8]) -> bool {
    let Ok(z) = joyoffice_opc::ZipArchive::new(
        bytes,
        &joyoffice_opc::Budget {
            max_part_bytes: 8 << 20,
            max_total_bytes: 128 << 20,
            max_entries: 4096,
            ..Default::default()
        },
    ) else {
        return false;
    };
    z.entries()
        .iter()
        .enumerate()
        .find(|(_, e)| e.name == "mimetype" && e.uncompressed_size < 128)
        .is_some_and(|(i, _)| {
            z.read(i)
                .is_ok_and(|v| v == b"application/vnd.joydeck.presentation+zip")
        })
}
