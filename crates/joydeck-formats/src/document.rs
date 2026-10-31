pub use joydeck_model::document::*;
use serde::Deserialize;
use std::io::Read;
use std::{collections::BTreeMap, sync::atomic::AtomicBool};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    format: String,
    schema_major: u32,
    schema_minor: u32,
    min_reader: u32,
    required_features: Vec<String>,
    document: String,
    #[serde(default)]
    resources: Vec<String>,
    #[serde(default)]
    source: bool,
    #[serde(default)]
    media: Vec<String>,
}
pub fn read_nested_package(bytes: &[u8], cancel: &AtomicBool) -> Result<BTreeMap<String, Vec<u8>>> {
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("嵌入工作簿超过4MiB压缩预算".into());
    }
    read_zip(std::io::Cursor::new(bytes), cancel, 8 * 1024 * 1024, 128)
}
fn read_zip<R: std::io::Read + std::io::Seek>(
    reader: R,
    cancel: &AtomicBool,
    max_total: usize,
    max_entries: usize,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| format!("不是可读的ZIP文稿：{e}"))?;
    if zip.len() > max_entries {
        return Err("ZIP 成员超过 4096 项".into());
    }
    let mut parts = BTreeMap::new();
    let mut total = 0;
    for i in 0..zip.len() {
        check_cancel(cancel)?;
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        let bare = name.trim_end_matches('/');
        if bare.is_empty()
            || bare.contains(['\\', ':', '%'])
            || bare.starts_with('/')
            || bare
                .split('/')
                .any(|x| x.is_empty() || x == "." || x == "..")
            || parts.contains_key(&name)
            || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
        {
            return Err(format!("非法或重复 ZIP 路径：{name}"));
        }
        if entry.size() > MAX_PART as u64 {
            return Err(format!("成员超过 8 MiB：{name}"));
        }
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 32 * 1024];
        loop {
            check_cancel(cancel)?;
            let n = entry
                .read(&mut chunk)
                .map_err(|e| format!("ZIP 读取失败 {name}：{e}"))?;
            if n == 0 {
                break;
            }
            total += n;
            if total > max_total || bytes.len() + n > MAX_PART.min(max_total) {
                return Err("解压预算超限".into());
            }
            bytes.extend_from_slice(&chunk[..n]);
        }
        parts.insert(name, bytes);
    }
    Ok(parts)
}
pub fn part<'a>(parts: &'a BTreeMap<String, Vec<u8>>, name: &str) -> Result<&'a str> {
    std::str::from_utf8(parts.get(name).ok_or_else(|| format!("缺少部件：{name}"))?)
        .map_err(|_| format!("部件不是 UTF-8：{name}"))
}
pub fn decode_parts(parts: &BTreeMap<String, Vec<u8>>, cancel: &AtomicBool) -> Result<Deck> {
    let deck = if parts.contains_key("mimetype") {
        native(parts)?
    } else if parts.contains_key("[Content_Types].xml") {
        crate::pptx::import(parts, cancel)?
    } else {
        return Err("未识别的文稿内容；支持 .joydeck v1 / PPTX".into());
    };
    check_cancel(cancel)?;
    deck.validate()?;
    Ok(deck)
}
fn native(parts: &BTreeMap<String, Vec<u8>>) -> Result<Deck> {
    if part(parts, "mimetype")? != "application/vnd.joydeck.presentation+zip" {
        return Err("原生格式身份不匹配".into());
    }
    let m: Manifest = serde_json::from_str(part(parts, "manifest.json")?)
        .map_err(|e| format!("manifest 无效：{e}"))?;
    if m.format != "joydeck"
        || m.schema_major != 1
        || m.min_reader > 11
        || m.required_features.iter().any(|s| {
            s != "object-transform-v1"
                && s != "rich-text-v1"
                && s != "image-resource-v1"
                && s != "design-v1"
                && s != "pptx-source-v1"
                && s != "table-v1"
                && s != "charts-v1"
                && s != "motion-v1"
                && s != "review-v1"
                && s != "media-v1"
        })
        || m.document != "document.json"
    {
        return Err("不支持的原生版本、入口或必需特性".into());
    }
    let mut deck: Deck = serde_json::from_str(part(parts, &m.document)?)
        .map_err(|e| format!("文档模型无效：{e}"))?;
    if !deck.resources.is_empty() {
        return Err("资源必须存放在独立部件中".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for name in m.resources {
        if !seen.insert(name.clone()) {
            return Err("重复资源索引".into());
        }
        let bytes = parts.get(&name).ok_or("资源部件缺失")?;
        let resource = crate::media::Resource::from_png(bytes)?;
        if name != format!("resources/{}.png", resource.id()) {
            return Err("资源哈希不匹配".into());
        }
        deck.resources
            .insert(resource.id(), std::sync::Arc::new(resource));
    }
    for id in m.media {
        let asset = deck.playback.assets.get_mut(&id).ok_or("媒体索引无效")?;
        if !asset.bytes.is_empty() {
            return Err("媒体不能内联存储".into());
        }
        std::sync::Arc::make_mut(asset).bytes = parts
            .get(&format!("media/{id}.bin"))
            .ok_or("媒体部件缺失")?
            .clone();
    }
    if deck.source.is_some() {
        return Err("源包必须存在独立部件中".into());
    }
    if m.source {
        let mut source: crate::interop::SourcePackage =
            serde_json::from_str(part(parts, "source/metadata.json")?)
                .map_err(|e| e.to_string())?;
        if !source.parts.is_empty() {
            return Err("源包元数据含内联部件".into());
        }
        source.parts = parts
            .iter()
            .filter_map(|(name, bytes)| {
                name.strip_prefix("source/package/")
                    .map(|name| (name.to_owned(), bytes.clone()))
            })
            .collect();
        deck.source = Some(std::sync::Arc::new(source));
        deck.warn("pptx-source",None,"source/metadata.json","包含原始PPTX源包；当前显示是受支持子集。源包写回仅允许唯一单行文字，其他修改需明确有损导出。");
    }
    if m.schema_minor > 10 {
        deck.warn(
            "schema-minor",
            None,
            "manifest.json",
            "较新的可选 schema；仅按 v1 只读显示",
        );
    }
    Ok(deck)
}
