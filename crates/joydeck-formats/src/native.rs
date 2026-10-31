use joydeck_model::document::{Deck, MAX_PART, Result};
use std::{io::Write, sync::Arc};
pub fn encode(deck: &Deck) -> Result<Vec<u8>> {
    deck.validate()?;
    let mut model = deck.clone();
    model.resources.clear();
    model.source = None;
    for asset in model.playback.assets.values_mut() {
        Arc::make_mut(asset).bytes.clear();
    }
    let bytes = serde_json::to_vec(&model).map_err(|e| e.to_string())?;
    let manifest = serde_json::to_vec(&serde_json::json!({"format":"joydeck","schemaMajor":1,"schemaMinor":10,"minReader":11,"requiredFeatures":["object-transform-v1","rich-text-v1","image-resource-v1","design-v1","pptx-source-v1","table-v1","charts-v1","motion-v1","review-v1","media-v1"],"document":"document.json","source":deck.source.is_some(),"media":deck.playback.assets.keys().collect::<Vec<_>>(),"resources":deck.resources.keys().map(|id| format!("resources/{id}.png")).collect::<Vec<_>>()})).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_PART {
        return Err("模型超过读取预算".into());
    }
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in [
        (
            "mimetype",
            b"application/vnd.joydeck.presentation+zip".as_slice(),
        ),
        ("manifest.json", manifest.as_slice()),
        ("document.json", bytes.as_slice()),
    ] {
        z.start_file(name, opts).map_err(|e| e.to_string())?;
        z.write_all(data).map_err(|e| e.to_string())?;
    }
    for (id, resource) in &deck.resources {
        z.start_file(format!("resources/{id}.png"), opts)
            .map_err(|e| e.to_string())?;
        z.write_all(&resource.png).map_err(|e| e.to_string())?;
    }
    for (id, asset) in &deck.playback.assets {
        z.start_file(format!("media/{id}.bin"), opts)
            .map_err(|e| e.to_string())?;
        z.write_all(&asset.bytes).map_err(|e| e.to_string())?;
    }
    if let Some(source) = &deck.source {
        let mut meta = (**source).clone();
        meta.parts.clear();
        z.start_file("source/metadata.json", opts)
            .map_err(|e| e.to_string())?;
        z.write_all(&serde_json::to_vec(&meta).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        for (name, data) in &source.parts {
            z.start_file(format!("source/package/{name}"), opts)
                .map_err(|e| e.to_string())?;
            z.write_all(data).map_err(|e| e.to_string())?;
        }
    }
    Ok(z.finish().map_err(|e| e.to_string())?.into_inner())
}
pub fn validate_editable(parts: &std::collections::BTreeMap<String, Vec<u8>>) -> Result<()> {
    let manifest: serde_json::Value =
        serde_json::from_str(crate::document::part(parts, "manifest.json")?)
            .map_err(|e| e.to_string())?;
    if !matches!(manifest["schemaMinor"].as_u64(), Some(0..=10)) {
        return Err("新版本格式只能只读查看".into());
    }
    let empty = vec![];
    let resources = if manifest.get("resources").is_none() {
        &empty
    } else {
        manifest["resources"].as_array().ok_or("无效资源索引")?
    };
    if parts.len()
        != 3 + resources.len()
            + parts.keys().filter(|n| n.starts_with("media/")).count()
            + if manifest["source"] == true {
                1 + parts
                    .keys()
                    .filter(|n| n.starts_with("source/package/"))
                    .count()
            } else {
                0
            }
    {
        return Err("未知部件：请使用只读查看器打开".into());
    }
    let raw: serde_json::Value =
        serde_json::from_str(crate::document::part(parts, "document.json")?)
            .map_err(|e| e.to_string())?;
    fn keys(v: &serde_json::Value, allowed: &[&str]) -> Result<()> {
        if v.as_object()
            .ok_or("无效对象")?
            .keys()
            .any(|k| !allowed.contains(&k.as_str()))
        {
            Err("未知字段：请使用只读查看器打开".into())
        } else {
            Ok(())
        }
    }
    keys(
        &raw,
        &[
            "title",
            "width",
            "height",
            "slides",
            "design",
            "organization",
            "fieldDate",
            "presentation",
            "review",
            "playback",
        ],
    )?;
    for s in raw["slides"].as_array().ok_or("缺少页面")? {
        keys(
            s,
            &[
                "id",
                "title",
                "background",
                "objects",
                "hidden",
                "layout",
                "inherit_background",
                "notes",
                "section",
            ],
        )?;
    }
    Ok(())
}
