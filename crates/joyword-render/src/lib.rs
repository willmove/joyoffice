//! CPU consumer of the same immutable EMU scenes as PDF (ARCHITECTURE §6.2).
//! No shaping, line breaking, system font lookup or document mutation occurs
//! here. Monochrome, unhinted outlines use the exact supplied font bytes/face.
pub mod cache;
pub mod requests;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use joyword_foundation::Emu;
use joyword_layout::{FontKey, LoadedFont, PageScene, SceneImage};

pub use cache::{NeighborhoodCache, NeighborhoodRequest};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::TableProvider;
use skrifa::{GlyphId, MetadataProvider, Tag};
use tiny_skia::{
    BlendMode, FillRule, FilterQuality, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Transform,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projection {
    dpi: f64,
    zoom: f64,
}
impl Projection {
    pub fn new(dpi: f64, zoom: f64) -> Result<Self, RasterError> {
        if !dpi.is_finite()
            || !zoom.is_finite()
            || !(1.0..=1200.0).contains(&dpi)
            || !(0.1..=5.0).contains(&zoom)
        {
            return Err(RasterError::InvalidProjection);
        }
        Ok(Self { dpi, zoom })
    }
    pub fn dpi(self) -> f64 {
        self.dpi
    }
    pub fn zoom(self) -> f64 {
        self.zoom
    }
    pub fn pixels(self, emu: Emu) -> f64 {
        emu.raw() as f64 / Emu::PER_INCH as f64 * self.dpi * self.zoom
    }
}

/// Caret pixels painted into a copy of one cached page. The returned buffer is
/// the overlay; `page_bgra` is not modified, so a cache hit stays valid.
pub fn paint_caret_overlay(
    page_bgra: &[u8],
    width: u32,
    height: u32,
    projection: Projection,
    caret_x: Emu,
    caret_top: Emu,
    caret_height: Emu,
) -> Vec<u8> {
    let width = width as usize;
    let height = height as usize;
    let mut overlay = page_bgra.to_vec();
    if width == 0 || height == 0 {
        return overlay;
    }
    let x = projection
        .pixels(caret_x)
        .round()
        .clamp(0.0, (width - 1) as f64) as usize;
    let y0 = projection
        .pixels(caret_top)
        .round()
        .clamp(0.0, (height - 1) as f64) as usize;
    let y1 = projection
        .pixels(Emu::new(caret_top.raw().saturating_add(caret_height.raw())))
        .round()
        .clamp(y0 as f64, height as f64) as usize;
    for y in y0..y1.max(y0 + 1).min(height) {
        let index = (y * width + x) * 4;
        if index + 3 < overlay.len() {
            overlay[index] = 0x20;
            overlay[index + 1] = 0x40;
            overlay[index + 2] = 0xe0;
            overlay[index + 3] = 0xff;
        }
    }
    overlay
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RasterLimit {
    Dimensions,
    Pixels,
    Fonts,
    FontBytes,
    Runs,
    Glyphs,
    Outlines,
    Commands,
    WorkPixels,
    EdgeWork,
}
#[derive(Clone, Copy, Debug)]
pub struct RasterLimits {
    pub max_dimension: u32,
    pub max_pixels: u64,
    pub max_fonts: usize,
    pub max_font_bytes: usize,
    pub max_runs: usize,
    pub max_glyphs: usize,
    pub max_outlines: usize,
    pub max_commands: usize,
    pub max_work_pixels: u64,
    pub max_edge_work: u64,
}
impl Default for RasterLimits {
    fn default() -> Self {
        Self {
            max_dimension: 8192,
            max_pixels: 16_777_216,
            max_fonts: 64,
            max_font_bytes: 256 << 20,
            max_runs: 100_000,
            max_glyphs: 2_000_000,
            max_outlines: 8192,
            max_commands: 1_000_000,
            max_work_pixels: 268_435_456,
            max_edge_work: 33_554_432,
        }
    }
}
impl RasterLimits {
    fn capped(self) -> Self {
        let h = Self::default();
        Self {
            max_dimension: self.max_dimension.min(h.max_dimension),
            max_pixels: self.max_pixels.min(h.max_pixels),
            max_fonts: self.max_fonts.min(h.max_fonts),
            max_font_bytes: self.max_font_bytes.min(h.max_font_bytes),
            max_runs: self.max_runs.min(h.max_runs),
            max_glyphs: self.max_glyphs.min(h.max_glyphs),
            max_outlines: self.max_outlines.min(h.max_outlines),
            max_commands: self.max_commands.min(h.max_commands),
            max_work_pixels: self.max_work_pixels.min(h.max_work_pixels),
            max_edge_work: self.max_edge_work.min(h.max_edge_work),
        }
    }
}
#[derive(Clone, Default)]
pub struct RasterCancellation(Arc<AtomicBool>);
impl RasterCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    fn check(&self) -> Result<(), RasterError> {
        if self.is_cancelled() {
            Err(RasterError::Cancelled)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RasterError {
    InvalidProjection,
    InvalidPage,
    InvalidGeometry,
    InvalidFont(FontKey),
    DuplicateFont(FontKey),
    MissingFont(FontKey),
    UnsupportedFont(FontKey),
    InvalidGlyph {
        font: FontKey,
        glyph: u16,
    },
    Outline {
        font: FontKey,
        glyph: u16,
    },
    /// A scene image could not be decoded. The text does not echo bytes.
    Image(&'static str),
    LimitExceeded(RasterLimit),
    Allocation,
    Cancelled,
    Png,
}
impl fmt::Display for RasterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "page raster: {self:?}")
    }
}
impl std::error::Error for RasterError {}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RasterStats {
    pub glyphs: usize,
    pub outlines: usize,
    pub commands: usize,
    pub work_pixels: u64,
    pub edge_work: u64,
}
/// Opaque RGBA8 page. Font alpha is composited over white, so the output is
/// also suitable for image APIs expecting straight (not premultiplied) alpha.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RasterPage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    stats: RasterStats,
}
impl RasterPage {
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
    pub fn stats(&self) -> RasterStats {
        self.stats
    }
    /// GPUI's RenderImage consumes BGRA. Keep channel conversion explicit.
    pub fn into_bgra(mut self) -> Vec<u8> {
        for p in self.rgba.as_chunks_mut::<4>().0 {
            p.swap(0, 2);
        }
        self.rgba
    }
    pub fn png(&self) -> Result<Vec<u8>, RasterError> {
        tiny_skia::PixmapRef::from_bytes(&self.rgba, self.width, self.height)
            .ok_or(RasterError::InvalidPage)?
            .encode_png()
            .map_err(|_| RasterError::Png)
    }
}

struct Pen<'a> {
    builder: PathBuilder,
    commands: &'a mut usize,
    limit: usize,
    failure: Option<RasterError>,
    cancellation: &'a RasterCancellation,
}
impl Pen<'_> {
    fn accept(&mut self, coordinates: &[f32]) -> bool {
        if self.failure.is_some() {
            return false;
        }
        if self.cancellation.is_cancelled() {
            self.failure = Some(RasterError::Cancelled);
            return false;
        }
        if *self.commands >= self.limit {
            self.failure = Some(RasterError::LimitExceeded(RasterLimit::Commands));
            return false;
        }
        if coordinates
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
        {
            self.failure = Some(RasterError::InvalidGeometry);
            return false;
        }
        *self.commands += 1;
        true
    }
}
impl OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.accept(&[x, y]) {
            self.builder.move_to(x, -y);
        }
    }
    fn line_to(&mut self, x: f32, y: f32) {
        if self.accept(&[x, y]) {
            self.builder.line_to(x, -y);
        }
    }
    fn quad_to(&mut self, x: f32, y: f32, ex: f32, ey: f32) {
        if self.accept(&[x, y, ex, ey]) {
            self.builder.quad_to(x, -y, ex, -ey);
        }
    }
    fn curve_to(&mut self, x: f32, y: f32, x2: f32, y2: f32, ex: f32, ey: f32) {
        if self.accept(&[x, y, x2, y2, ex, ey]) {
            self.builder.cubic_to(x, -y, x2, -y2, ex, -ey);
        }
    }
    fn close(&mut self) {
        if self.accept(&[]) {
            self.builder.close();
        }
    }
}
fn limit(condition: bool, which: RasterLimit) -> Result<(), RasterError> {
    if condition {
        Err(RasterError::LimitExceeded(which))
    } else {
        Ok(())
    }
}
fn checked_position(a: Emu, b: Emu, projection: Projection) -> Result<f32, RasterError> {
    let raw = a
        .raw()
        .checked_add(b.raw())
        .ok_or(RasterError::InvalidGeometry)?;
    let px = projection.pixels(Emu::new(raw));
    if !px.is_finite() || px.abs() > 1_000_000.0 {
        return Err(RasterError::InvalidGeometry);
    }
    Ok(px as f32)
}

/// Pixel size `rasterize` gives `scene` at `projection`, refused for page
/// geometry, dimensions and total pixels exactly as `rasterize` refuses it.
/// Nothing is allocated or drawn, so callers can budget pages up front.
pub fn raster_size(
    scene: &PageScene,
    projection: Projection,
    limits: RasterLimits,
) -> Result<(u32, u32), RasterError> {
    let limits = limits.capped();
    if scene.width.raw() <= 0 || scene.height.raw() <= 0 {
        return Err(RasterError::InvalidPage);
    }
    let w = projection.pixels(scene.width).ceil();
    let h = projection.pixels(scene.height).ceil();
    limit(
        w > f64::from(limits.max_dimension) || h > f64::from(limits.max_dimension),
        RasterLimit::Dimensions,
    )?;
    let (width, height) = (w as u32, h as u32);
    if width == 0 || height == 0 {
        return Err(RasterError::InvalidPage);
    }
    limit(
        u64::from(width) * u64::from(height) > limits.max_pixels,
        RasterLimit::Pixels,
    )?;
    Ok((width, height))
}

fn paint_image(
    pixmap: &mut Pixmap,
    image: &SceneImage,
    projection: Projection,
) -> Result<(), RasterError> {
    let decoded =
        joyword_model::decode_rgba(image.bytes.as_ref(), &joyword_model::ImageLimits::default())
            .map_err(|_| RasterError::Image("image decode failed"))?;
    let expected = (decoded.width as usize)
        .checked_mul(decoded.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(RasterError::Image("image decode failed"))?;
    if decoded.rgba.len() != expected || decoded.width == 0 || decoded.height == 0 {
        return Err(RasterError::Image("image decode failed"));
    }
    let mut rgba = decoded.rgba;
    for pixel in rgba.as_chunks_mut::<4>().0 {
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = ((u32::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    let size = tiny_skia::IntSize::from_wh(decoded.width, decoded.height)
        .ok_or(RasterError::Image("image decode failed"))?;
    let source = Pixmap::from_vec(rgba, size).ok_or(RasterError::Image("image decode failed"))?;
    let dest_w = projection.pixels(image.width) as f32;
    let dest_h = projection.pixels(image.height) as f32;
    let x = projection.pixels(image.x) as f32;
    let y = projection.pixels(image.y) as f32;
    if ![dest_w, dest_h, x, y].iter().all(|value| value.is_finite())
        || dest_w <= 0.0
        || dest_h <= 0.0
    {
        return Err(RasterError::Image("image extent rejected"));
    }
    let transform = Transform::from_row(
        dest_w / decoded.width as f32,
        0.0,
        0.0,
        dest_h / decoded.height as f32,
        x,
        y,
    );
    pixmap.draw_pixmap(
        0,
        0,
        source.as_ref(),
        &PixmapPaint {
            opacity: 1.0,
            blend_mode: BlendMode::SourceOver,
            quality: FilterQuality::Nearest,
        },
        transform,
        None,
    );
    Ok(())
}

/// All work is private until success. An error or cancellation never yields a
/// partial page. Request budgets can only lower the fixed hard ceilings.
pub fn rasterize(
    scene: &PageScene,
    fonts: &[LoadedFont],
    projection: Projection,
    limits: RasterLimits,
    cancellation: &RasterCancellation,
) -> Result<RasterPage, RasterError> {
    cancellation.check()?;
    let limits = limits.capped();
    let (width, height) = raster_size(scene, projection, limits)?;
    limit(fonts.len() > limits.max_fonts, RasterLimit::Fonts)?;
    limit(
        scene.runs.len().saturating_add(scene.images.len()) > limits.max_runs,
        RasterLimit::Runs,
    )?;
    let mut by_key = BTreeMap::new();
    let mut font_bytes = 0usize;
    for font in fonts {
        cancellation.check()?;
        font_bytes = font_bytes.saturating_add(font.data().len());
        limit(font_bytes > limits.max_font_bytes, RasterLimit::FontBytes)?;
        if by_key.insert(font.key(), font).is_some() {
            return Err(RasterError::DuplicateFont(font.key()));
        }
    }
    let mut count = 0usize;
    for run in &scene.runs {
        if run.glyphs.is_empty() && run.text.is_empty() {
            continue;
        }
        count = count.saturating_add(run.glyphs.len());
        limit(count > limits.max_glyphs, RasterLimit::Glyphs)?;
        let font = by_key
            .get(&run.font)
            .ok_or(RasterError::MissingFont(run.font))?;
        let face = font.font_ref();
        // A monochrome base outline is not a correct replacement for a color
        // glyph or bitmap/SVG font. Refuse these until their paint is supported.
        // Legacy EBDT strikes (e.g. SimSun) may accompany scalable outlines.
        // This explicitly unhinted outline profile uses the scalable glyphs,
        // just as the PDF consumer does. Bitmap-only faces still fail below
        // when an outline cannot be obtained; colored paint stays refused.
        if [b"COLR", b"CBDT", b"sbix", b"SVG "]
            .iter()
            .any(|t| face.data_for_tag(Tag::new(t)).is_some())
        {
            return Err(RasterError::UnsupportedFont(run.font));
        }
        let glyph_count = face
            .maxp()
            .map_err(|_| RasterError::InvalidFont(run.font))?
            .num_glyphs();
        let px_size = projection.pixels(run.size);
        if run.size.raw() <= 0 || !px_size.is_finite() || px_size > 4096.0 {
            return Err(RasterError::InvalidGeometry);
        }
        if run.text.is_empty() != run.glyphs.is_empty()
            || run.glyphs.first().is_some_and(|g| g.cluster != 0)
        {
            return Err(RasterError::InvalidGeometry);
        }
        let mut previous_cluster = 0;
        let mut pen = 0i64;
        for glyph in &run.glyphs {
            cancellation.check()?;
            if glyph.glyph_id >= glyph_count {
                return Err(RasterError::InvalidGlyph {
                    font: run.font,
                    glyph: glyph.glyph_id,
                });
            }
            if glyph.cluster >= run.text.len() || !run.text.is_char_boundary(glyph.cluster) {
                return Err(RasterError::InvalidGeometry);
            }
            if glyph.cluster < previous_cluster || glyph.x.raw() != pen || glyph.advance.raw() < 0 {
                return Err(RasterError::InvalidGeometry);
            }
            previous_cluster = glyph.cluster;
            pen = pen
                .checked_add(glyph.advance.raw())
                .ok_or(RasterError::InvalidGeometry)?;
            checked_position(run.origin_x, glyph.x, projection)?;
            checked_position(
                run.origin_x,
                glyph
                    .x
                    .checked_add(glyph.x_offset)
                    .ok_or(RasterError::InvalidGeometry)?,
                projection,
            )?;
            let y = run
                .baseline_y
                .raw()
                .checked_sub(glyph.y_offset.raw())
                .ok_or(RasterError::InvalidGeometry)?;
            checked_position(Emu::new(y), Emu::new(0), projection)?;
        }
    }
    let mut pixmap = Pixmap::new(width, height).ok_or(RasterError::Allocation)?;
    pixmap.fill(tiny_skia::Color::WHITE);
    for rect in &scene.rects {
        let x = projection.pixels(rect.x);
        let y = projection.pixels(rect.y);
        let w = projection.pixels(rect.width);
        let h = projection.pixels(rect.height);
        if ![x, y, w, h].iter().all(|value| value.is_finite()) || w <= 0.0 || h <= 0.0 {
            continue;
        }
        let mut builder = PathBuilder::new();
        builder.move_to(x as f32, y as f32);
        builder.line_to((x + w) as f32, y as f32);
        builder.line_to((x + w) as f32, (y + h) as f32);
        builder.line_to(x as f32, (y + h) as f32);
        builder.close();
        if let Some(path) = builder.finish() {
            let mut fill = Paint::default();
            fill.set_color_rgba8(rect.red, rect.green, rect.blue, 255);
            fill.anti_alias = false;
            pixmap.fill_path(&path, &fill, FillRule::Winding, Transform::identity(), None);
        }
    }
    for image in &scene.images {
        paint_image(&mut pixmap, image, projection)?;
    }
    let mut paint = Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    paint.anti_alias = true;
    let mut cache: BTreeMap<(FontKey, i64, u16), (Option<Path>, usize)> = BTreeMap::new();
    let mut stats = RasterStats::default();
    for run in &scene.runs {
        if run.glyphs.is_empty() && run.text.is_empty() {
            continue;
        }
        let font = by_key[&run.font];
        let outlines = font.font_ref().outline_glyphs();
        for glyph in &run.glyphs {
            cancellation.check()?;
            let key = (run.font, run.size.raw(), glyph.glyph_id);
            if !cache.contains_key(&key) {
                limit(cache.len() >= limits.max_outlines, RasterLimit::Outlines)?;
                let outline = outlines
                    .get(GlyphId::new(u32::from(glyph.glyph_id)))
                    .ok_or(RasterError::Outline {
                        font: run.font,
                        glyph: glyph.glyph_id,
                    })?;
                let before_commands = stats.commands;
                let mut pen = Pen {
                    builder: PathBuilder::new(),
                    commands: &mut stats.commands,
                    limit: limits.max_commands,
                    failure: None,
                    cancellation,
                };
                let drawn = outline.draw(
                    DrawSettings::unhinted(
                        Size::new(projection.pixels(run.size) as f32),
                        LocationRef::default(),
                    ),
                    &mut pen,
                );
                if let Some(error) = pen.failure {
                    return Err(error);
                }
                drawn.map_err(|_| RasterError::Outline {
                    font: run.font,
                    glyph: glyph.glyph_id,
                })?;
                cache.insert(
                    key,
                    (pen.builder.finish(), stats.commands - before_commands),
                );
                stats.outlines += 1;
            }
            stats.glyphs += 1;
            let (path, commands) = &cache[&key];
            let Some(path) = path else { continue }; // e.g. a space
            let x = checked_position(
                run.origin_x,
                glyph
                    .x
                    .checked_add(glyph.x_offset)
                    .ok_or(RasterError::InvalidGeometry)?,
                projection,
            )?;
            let y = projection.pixels(Emu::new(
                run.baseline_y
                    .raw()
                    .checked_sub(glyph.y_offset.raw())
                    .ok_or(RasterError::InvalidGeometry)?,
            )) as f32;
            let bounds = path.bounds();
            let left = (bounds.left() + x).floor().max(0.0).min(width as f32);
            let right = (bounds.right() + x).ceil().max(0.0).min(width as f32);
            let top = (bounds.top() + y).floor().max(0.0).min(height as f32);
            let bottom = (bounds.bottom() + y).ceil().max(0.0).min(height as f32);
            let work = ((right - left).max(0.0) as u64) * ((bottom - top).max(0.0) as u64);
            stats.work_pixels = stats.work_pixels.saturating_add(work);
            limit(
                stats.work_pixels > limits.max_work_pixels,
                RasterLimit::WorkPixels,
            )?;
            // Cached paths still incur edge processing on every fill. Charge
            // their complexity times visible scan rows, not just pixel area.
            if work != 0 {
                stats.edge_work = stats.edge_work.saturating_add(
                    (*commands as u64).saturating_mul((bottom - top).max(0.0) as u64),
                );
                limit(
                    stats.edge_work > limits.max_edge_work,
                    RasterLimit::EdgeWork,
                )?;
            }
            if work != 0 {
                let em = projection.pixels(run.size) as f32;
                let passes = if run.bold { 2 } else { 1 };
                for pass in 0..passes {
                    let dx = if pass == 0 { 0.0 } else { em * 0.04 };
                    let transform = if run.italic {
                        // Shear about the baseline so italic leans right.
                        Transform::from_row(1.0, 0.0, -0.2, 1.0, x + dx, y)
                    } else {
                        Transform::from_translate(x + dx, y)
                    };
                    pixmap.fill_path(path, &paint, FillRule::Winding, transform, None);
                }
            }
        }
    }
    cancellation.check()?;
    Ok(RasterPage {
        width,
        height,
        rgba: pixmap.take(),
        stats,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bgra_conversion_swaps_only_red_and_blue() {
        let page = RasterPage {
            width: 2,
            height: 1,
            rgba: vec![11, 22, 33, 255, 44, 55, 66, 255],
            stats: RasterStats::default(),
        };
        assert_eq!(page.into_bgra(), [33, 22, 11, 255, 66, 55, 44, 255]);
    }
}
