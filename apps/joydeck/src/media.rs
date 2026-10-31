//! Bounded, content-addressed image resources. No file paths survive import.
use crate::document::{Deck, MAX_PART, Object, Result};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{Cursor, Read},
    path::Path,
};

pub const MAX_DIMENSION: u32 = 4096;
pub const MAX_PIXELS: u64 = 4 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRef {
    pub resource: String,
    /// Normalized source rectangle: x, y, width, height.
    pub crop: [f32; 4],
}
fn decode(bytes: &[u8]) -> Result<DynamicImage> {
    if bytes.is_empty() || bytes.len() > MAX_PART {
        return Err("图片超过8 MiB编码预算或为空".into());
    }
    let format = image::guess_format(bytes).map_err(|_| "只支持 PNG / JPEG 图片内容")?;
    if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
        return Err("只支持 PNG / JPEG 图片内容".into());
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err("图片超过4M像素解码预算".into());
    }
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    image.apply_orientation(orientation);
    Ok(image)
}
impl Resource {
    pub fn import(bytes: &[u8]) -> Result<Self> {
        let image = decode(bytes)?;
        let mut png = Cursor::new(Vec::new());
        image
            .write_to(&mut png, ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let resource = Self {
            width: image.width(),
            height: image.height(),
            png: png.into_inner(),
        };
        resource.validate()?;
        Ok(resource)
    }
    pub fn read(path: &Path) -> Result<Self> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(MAX_PART as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Self::import(&bytes)
    }
    pub fn from_png(bytes: &[u8]) -> Result<Self> {
        if image::guess_format(bytes).ok() != Some(ImageFormat::Png) {
            return Err("资源必须为PNG".into());
        }
        let image = decode(bytes)?;
        Ok(Self {
            width: image.width(),
            height: image.height(),
            png: bytes.to_vec(),
        })
    }
    pub fn id(&self) -> String {
        format!("{:x}", Sha256::digest(&self.png))
    }
    pub fn validate(&self) -> Result<()> {
        if self.width == 0
            || self.height == 0
            || self.width > MAX_DIMENSION
            || self.height > MAX_DIMENSION
            || u64::from(self.width) * u64::from(self.height) > MAX_PIXELS
            || self.png.len() > MAX_PART
            || !self.png.starts_with(b"\x89PNG\r\n\x1a\n")
        {
            return Err("无效图片资源或预算超限".into());
        }
        let decoded = decode(&self.png)?;
        if (decoded.width(), decoded.height()) != (self.width, self.height) {
            return Err("资源尺寸不匹配".into());
        }
        Ok(())
    }
}
pub fn validate_resources(deck: &Deck) -> Result<()> {
    let mut total = 0;
    for (id, r) in &deck.resources {
        r.validate()?;
        if id != &r.id() {
            return Err("资源身份不匹配".into());
        }
        total += r.png.len();
        if total > 32 * 1024 * 1024 {
            return Err("资源总预算超过32 MiB".into());
        }
    }
    Ok(())
}
pub fn validate_image(deck: &Deck, o: &Object) -> Result<()> {
    match (&o.image, o.kind.as_str()) {
        (Some(i), "image") => {
            let [x, y, w, h] = i.crop;
            if !deck.resources.contains_key(&i.resource)
                || !i.crop.iter().all(|v| v.is_finite())
                || x < 0.
                || y < 0.
                || w <= 0.
                || h <= 0.
                || x + w > 1.000001
                || y + h > 1.000001
                || o.width <= 0.
                || o.height <= 0.
            {
                return Err("图片引用或裁剪区域无效".into());
            }
        }
        (None, "image") | (Some(_), _) => return Err("图片对象与资源不匹配".into()),
        _ => {}
    }
    Ok(())
}
pub fn collect_unused(deck: &mut Deck) {
    let used: BTreeSet<_> = deck
        .slides
        .iter()
        .flat_map(|s| &s.objects)
        .filter_map(|o| o.image.as_ref().map(|i| i.resource.clone()))
        .collect();
    deck.resources.retain(|id, _| used.contains(id));
}
pub(crate) fn svg(deck: &Deck, o: &Object, index: usize) -> Result<String> {
    use base64::Engine;
    let i = o.image.as_ref().ok_or("缺少图片引用")?;
    let r = deck.resources.get(&i.resource).ok_or("缺少图片资源")?;
    let [x, y, w, h] = i.crop;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&r.png);
    // Nested SVG viewBox crops in source coordinates without changing stored PNG.
    Ok(format!(
        r#"<svg x="{}" y="{}" width="{}" height="{}" viewBox="{} {} {} {}" preserveAspectRatio="none" overflow="hidden"><image id="image{index}" width="{}" height="{}" href="data:image/png;base64,{encoded}"/></svg>"#,
        o.x,
        o.y,
        o.width,
        o.height,
        x * r.width as f32,
        y * r.height as f32,
        w * r.width as f32,
        h * r.height as f32,
        r.width,
        r.height
    ))
}

/// Self-contained clipboard payload: independent of the source session/history.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transfer {
    pub version: u32,
    pub objects: Vec<Object>,
    pub resources: std::collections::BTreeMap<String, std::sync::Arc<Resource>>,
}
impl Transfer {
    pub fn capture(deck: &Deck, page: usize, ids: &BTreeSet<String>) -> Result<Self> {
        let mut copy = deck.clone();
        let mut slide = crate::design::resolve_slide(deck, page)?;
        slide.layout = None;
        slide.inherit_background = false;
        slide.section = None;
        for o in &mut slide.objects {
            crate::table::freeze(deck, page, o);
            o.binding = None;
            o.field = None;
        }
        copy.slides = vec![slide];
        copy.slides[0].objects.retain(|o| ids.contains(&o.id));
        collect_unused(&mut copy);
        let transfer = Self {
            version: 1,
            objects: copy.slides.remove(0).objects,
            resources: copy.resources,
        };
        transfer.validate()?;
        Ok(transfer)
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 || self.objects.is_empty() || self.objects.len() > 1000 {
            return Err("不支持的对象剪贴板版本或数量".into());
        }
        let mut d = crate::edit::Session::new().deck.as_ref().clone();
        d.resources = self.resources.clone();
        d.slides[0].objects = self.objects.clone();
        d.validate()
    }
}
