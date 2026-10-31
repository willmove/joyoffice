//! Independent read-only model and bounded container reader. Units are points.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
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
pub fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err("读取已取消".into())
    } else {
        Ok(())
    }
}

/// Never extracts to disk and accounts for bytes even for unused parts.
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
