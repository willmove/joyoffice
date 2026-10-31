//! Independent read-only model and bounded container reader. Units are points.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

pub type Result<T> = std::result::Result<T, String>;
pub const MAX_FILE: u64 = 64 * 1024 * 1024;
pub const MAX_TOTAL: usize = 128 * 1024 * 1024;
pub const MAX_PART: usize = 8 * 1024 * 1024;
pub const MAX_PAGES: usize = 500;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Deck {
    #[serde(default)]
    pub playback: crate::playback::Playback,
    #[serde(default)]
    pub review: crate::review::Review,
    #[serde(default)]
    pub presentation: crate::motion::Presentation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<std::sync::Arc<crate::interop::SourcePackage>>,
    #[serde(default)]
    pub design: crate::design::Design,
    #[serde(default)]
    pub organization: crate::design::Organization,
    #[serde(default)]
    pub field_date: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub resources: BTreeMap<String, std::sync::Arc<crate::media::Resource>>,
    pub title: String,
    pub width: f32,
    pub height: f32,
    pub slides: Vec<Slide>,
    #[serde(skip)]
    pub warnings: Vec<Warning>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slide {
    #[serde(default)]
    pub layout: Option<String>,
    #[serde(default)]
    pub inherit_background: bool,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub hidden: bool,
    pub id: String,
    pub title: String,
    pub background: String,
    pub objects: Vec<Object>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Object {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub morph_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chart: Option<crate::chart::Chart>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagram: Option<crate::diagram::Diagram>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<crate::table::Table>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<crate::design::Binding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub rotation: f32,
    #[serde(default = "one")]
    pub opacity: f32,
    #[serde(default = "black")]
    pub stroke: String,
    #[serde(default)]
    pub stroke_width: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<crate::media::ImageRef>,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default = "black")]
    pub fill: String,
    #[serde(default)]
    pub text: String,
    #[serde(default = "font_size")]
    pub font_size: f32,
    #[serde(default = "left", skip_serializing_if = "is_left")]
    pub text_align: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<crate::text::Run>,
}
fn left() -> String {
    "left".into()
}
fn is_left(s: &String) -> bool {
    s == "left"
}
fn one() -> f32 {
    1.
}
fn black() -> String {
    "#202b3a".into()
}
fn font_size() -> f32 {
    24.
}
#[derive(Clone, Debug, PartialEq)]
pub struct Warning {
    pub code: &'static str,
    pub page: Option<usize>,
    pub part: String,
    pub message: String,
}
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
pub fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err("读取已取消".into())
    } else {
        Ok(())
    }
}

/// Never extracts to disk and accounts for bytes even for unused parts.
pub fn read_package(path: &Path, cancel: &AtomicBool) -> Result<BTreeMap<String, Vec<u8>>> {
    let file = File::open(path).map_err(|e| format!("无法打开文件：{e}"))?;
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_FILE {
        return Err("压缩文件超过 M1 的 64 MiB 上限".into());
    }
    read_zip(file, cancel, MAX_TOTAL, 4096)
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
pub fn open(path: &Path, cancel: &AtomicBool) -> Result<Deck> {
    let parts = read_package(path, cancel)?;
    let is_native = parts.contains_key("mimetype");
    let mut deck = if is_native {
        native(&parts)?
    } else if parts.contains_key("[Content_Types].xml") {
        crate::pptx::import(&parts, cancel)?
    } else {
        return Err("未识别的文稿内容；支持 .joydeck v1 / PPTX".into());
    };
    check_cancel(cancel)?;
    deck.validate()?;
    crate::preview::diagnose_fonts(&mut deck);
    if !path
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case(if is_native { "joydeck" } else { "pptx" }))
    {
        deck.warn("extension", None, "", "扩展名与内容不一致；按容器身份读取");
    }
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
impl Deck {
    pub fn warn(&mut self, code: &'static str, page: Option<usize>, part: &str, message: &str) {
        self.warnings.push(Warning {
            code,
            page,
            part: part.into(),
            message: message.into(),
        });
    }
    pub fn validate(&self) -> Result<()> {
        if let Some(source) = &self.source {
            source.validate()?;
        }
        crate::motion::Presentation::validate(&self.presentation, self)?;
        crate::design::validate(self)?;
        self.review.validate(self)?;
        self.playback.validate(self)?;
        crate::media::validate_resources(self)?;
        let mut ids = std::collections::BTreeSet::new();
        if !self.width.is_finite()
            || !self.height.is_finite()
            || self.width < 1.
            || self.height < 1.
            || self.width > 4096.
            || self.height > 4096.
        {
            return Err("页面尺寸必须在 1–4096 点范围内".into());
        }
        if self.slides.is_empty() || self.slides.len() > MAX_PAGES {
            return Err("页数须为 1–500".into());
        }
        let mut objects = 0;
        let mut cells = 0;
        let mut points = 0;
        let mut object_ids = std::collections::BTreeSet::new();
        for s in &self.slides {
            if s.id.is_empty() || !ids.insert(&s.id) || !color(&s.background) {
                return Err("页面 ID 重复/为空或背景无效".into());
            }
            objects += s.objects.len();
            if objects > 50000 || s.objects.len() > 10000 {
                return Err("对象数量超限".into());
            }
            for o in &s.objects {
                if !matches!(o.text_align.as_str(), "left" | "center" | "right") {
                    return Err("文字对齐无效".into());
                }
                if let Some(c) = &o.chart {
                    c.validate()?;
                    points += c.categories.len() * c.series.len();
                    if points > 16384 {
                        return Err("文稿图表点数超过16384预算".into());
                    }
                    if o.kind != "chart" {
                        return Err("图表对象类型不匹配".into());
                    }
                } else if o.kind == "chart" {
                    return Err("缺少图表模型".into());
                }
                if let Some(d) = &o.diagram {
                    d.validate()?;
                    if o.kind != "diagram" {
                        return Err("图示对象类型不匹配".into());
                    }
                } else if o.kind == "diagram" {
                    return Err("缺少图示模型".into());
                }
                crate::table::validate(o)?;
                cells += o.table.as_ref().map_or(0, |t| t.cells.len());
                if cells > 16384 {
                    return Err("文稿表格总单元格超过16384预算".into());
                }
                crate::media::validate_image(self, o)?;
                crate::text::validate(&o.text, &o.runs)?;
                if (!o.id.is_empty() && !object_ids.insert(&o.id))
                    || !o.rotation.is_finite()
                    || o.rotation.abs() > 36000.
                    || !o.opacity.is_finite()
                    || !(0. ..=1.).contains(&o.opacity)
                    || !color(&o.stroke)
                    || !o.stroke_width.is_finite()
                    || !(0. ..=100.).contains(&o.stroke_width)
                {
                    return Err("无效对象ID/旋转/透明度/描边".into());
                }
                if ![o.x, o.y, o.width, o.height, o.font_size]
                    .iter()
                    .all(|v| v.is_finite() && v.abs() <= 10000.)
                    || o.width < 0.
                    || o.height < 0.
                    || !(1. ..=512.).contains(&o.font_size)
                    || !color(&o.fill)
                {
                    return Err(format!("页面 {} 包含无效几何、字号或颜色", s.id));
                }
                if ![
                    "chart",
                    "diagram",
                    "table",
                    "image",
                    "rect",
                    "ellipse",
                    "text",
                    "line",
                    "placeholder",
                    "arrow",
                    "roundRect",
                ]
                .contains(&o.kind.as_str())
                {
                    return Err(format!("不支持的原生对象类型：{}", o.kind));
                }
            }
        }
        Ok(())
    }
}
pub fn color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|c| c.is_ascii_hexdigit())
}
