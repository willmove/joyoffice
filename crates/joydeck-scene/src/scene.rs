//! Immutable, window-independent scene. Geometry/layout and CPU reference share
//! the pinned SVG shaper; GPUI consumes these exact outlines, never re-shapes text.
use crate::{
    document::{Deck, Object, Result, Slide, check_cancel},
    geometry, preview,
};
use resvg::{tiny_skia as sk, usvg};
use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

pub const CACHE_BUDGET: usize = 64 * 1024 * 1024;
#[derive(Clone)]
pub struct PaintPath {
    pub data: Arc<sk::Path>,
    pub rgba: [u8; 4],
    pub even_odd: bool,
}
#[derive(Clone)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    /// Premultiplied RGBA; converted to straight BGRA at the GPUI image boundary.
    pub rgba: Arc<Vec<u8>>,
}
impl Bitmap {
    /// GPUI's image shader premultiplies samples; supply straight BGRA once.
    pub fn gpui_bgra(&self) -> Vec<u8> {
        let mut bytes = (*self.rgba).clone();
        for p in bytes.as_chunks_mut::<4>().0 {
            let a = p[3] as u32;
            for c in &mut p[..3] {
                *c = (*c as u32 * 255 + a / 2)
                    .checked_div(a)
                    .unwrap_or(0)
                    .min(255) as u8;
            }
            p.swap(0, 2);
        }
        bytes
    }
}
#[derive(Clone)]
pub struct GlyphPrimitive {
    pub path: Arc<sk::Path>,
    pub position: (f32, f32),
    pub rgba: [u8; 4],
    pub glyph_id: u16,
}
pub struct Geometry {
    pub paths: Vec<PaintPath>,
    pub glyph_masks: Vec<GlyphPrimitive>,
    pub fallback: Option<Bitmap>,
    pub reason: Option<String>,
    pub width: f32,
    pub height: f32,
    pub offset: (f32, f32),
    pub clip: Option<[f32; 4]>,
    pub glyphs: usize,
    pub missing_glyphs: usize,
    pub bytes: usize,
    tree: Option<usvg::Tree>,
}
#[derive(Clone)]
pub struct Node {
    pub id: String,
    pub position: (f32, f32),
    pub geometry: Arc<Geometry>,
}
impl Node {
    /// Bounded backend image primitive; importing/persisting media is a separate layer.
    pub fn image(
        id: String,
        position: (f32, f32),
        size: (f32, f32),
        bitmap: Bitmap,
    ) -> Result<Self> {
        let bytes = (bitmap.width as usize)
            .checked_mul(bitmap.height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or("image dimensions overflow")?;
        if bitmap.width == 0
            || bitmap.height == 0
            || bitmap.width > 4096
            || bitmap.height > 4096
            || bytes > CACHE_BUDGET
            || bytes != bitmap.rgba.len()
            || ![position.0, position.1, size.0, size.1]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 10000.)
            || size.0 <= 0.
            || size.1 <= 0.
        {
            return Err("invalid/budget-exceeding scene image".into());
        }
        Ok(Self {
            id,
            position,
            geometry: Arc::new(Geometry {
                paths: vec![],
                glyph_masks: vec![],
                fallback: Some(bitmap),
                reason: None,
                width: size.0,
                height: size.1,
                offset: (0., 0.),
                clip: None,
                glyphs: 0,
                missing_glyphs: 0,
                bytes,
                tree: None,
            }),
        })
    }
}
#[derive(Clone)]
pub struct Scene {
    pub document: u64,
    pub revision: u64,
    pub slide: String,
    pub width: f32,
    pub height: f32,
    pub background: [u8; 4],
    pub nodes: Vec<Node>,
}
#[derive(Clone, Default, serde::Serialize)]
pub struct Stats {
    pub builds: u64,
    pub hits: u64,
    pub evictions: u64,
    pub bytes: usize,
    pub build_micros: u64,
    pub local_rasters: u64,
}
pub struct SceneCache {
    budget: usize,
    entries: BTreeMap<String, (u64, Arc<Geometry>)>,
    clock: u64,
    pub stats: Stats,
}
impl Default for SceneCache {
    fn default() -> Self {
        Self::with_budget(CACHE_BUDGET)
    }
}
fn color(value: &str, opacity: f32) -> [u8; 4] {
    let n = u32::from_str_radix(&value[1..], 16).unwrap_or(0);
    [
        (n >> 16) as u8,
        (n >> 8) as u8,
        n as u8,
        (opacity * 255.).round() as u8,
    ]
}
fn paint_color(paint: &usvg::Paint, opacity: f32) -> Result<[u8; 4]> {
    match paint {
        usvg::Paint::Color(c) => Ok([c.red, c.green, c.blue, (opacity * 255.).round() as u8]),
        _ => Err("gradient/pattern subtree".into()),
    }
}
#[derive(Default)]
struct Extracted {
    paths: Vec<PaintPath>,
    masks: Vec<GlyphPrimitive>,
    glyphs: usize,
    missing: usize,
    glyph_paths: BTreeMap<String, Arc<sk::Path>>,
}
struct Outline(sk::PathBuilder);
impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, y);
    }
    fn quad_to(&mut self, x: f32, y: f32, a: f32, b: f32) {
        self.0.quad_to(x, y, a, b);
    }
    fn curve_to(&mut self, x: f32, y: f32, a: f32, b: f32, c: f32, d: f32) {
        self.0.cubic_to(x, y, a, b, c, d);
    }
    fn close(&mut self) {
        self.0.close();
    }
}
fn extract_text(t: &usvg::Text, out: &mut Extracted, db: &usvg::fontdb::Database) -> Result<()> {
    for span in t.layouted() {
        if !span.visible {
            continue;
        }
        if span.stroke.is_some()
            || span.underline.is_some()
            || span.overline.is_some()
            || span.line_through.is_some()
        {
            return Err("decorated glyph subtree".into());
        }
        let Some(fill) = &span.fill else {
            continue;
        };
        let rgba = paint_color(fill.paint(), fill.opacity().get())?;
        for g in &span.positioned_glyphs {
            out.glyphs += 1;
            if g.id.0 == 0 {
                out.missing += 1;
            }
            let transform = t.abs_transform().pre_concat(g.outline_transform());
            let mut local = transform;
            local.tx = 0.;
            local.ty = 0.;
            let key = format!("{:?}:{}:{:?}", g.font, g.id.0, local);
            let path = if let Some(path) = out.glyph_paths.get(&key) {
                Some(path.clone())
            } else {
                let result = db
                    .with_face_data(g.font, |data, index| {
                        let face = ttf_parser::Face::parse(data, index).map_err(|_| "font face")?;
                        let glyph_id = ttf_parser::GlyphId(g.id.0);
                        if face.is_color_glyph(glyph_id)
                            || face.glyph_svg_image(glyph_id).is_some()
                            || face.glyph_raster_image(glyph_id, u16::MAX).is_some()
                        {
                            // COLR fonts can also supply monochrome outlines;
                            // retaining only that outline would lose the color.
                            return Err("colored/bitmap glyph subtree");
                        }
                        let mut builder = Outline(sk::PathBuilder::new());
                        let bounds = face.outline_glyph(glyph_id, &mut builder);
                        if bounds.is_none() && g.text.chars().all(char::is_whitespace) {
                            return Ok(None);
                        }
                        if bounds.is_none() {
                            return Err("colored/bitmap glyph subtree");
                        }
                        Ok(builder
                            .0
                            .finish()
                            .and_then(|path| path.transform(local))
                            .map(Arc::new))
                    })
                    .ok_or("font source unavailable")??;
                if let Some(path) = &result {
                    out.glyph_paths.insert(key, path.clone());
                }
                result
            };
            if let Some(path) = path {
                out.masks.push(GlyphPrimitive {
                    path,
                    position: (transform.tx, transform.ty),
                    rgba,
                    glyph_id: g.id.0,
                });
            }
        }
    }
    Ok(())
}
fn extract(
    group: &usvg::Group,
    out: &mut Extracted,
    db: &usvg::fontdb::Database,
    allow_clip: bool,
) -> Result<()> {
    if group.opacity().get() != 1.
        || group.mask().is_some()
        || !group.filters().is_empty()
        || group.blend_mode() != usvg::BlendMode::Normal
        || (group.clip_path().is_some() && !allow_clip)
    {
        return Err("isolated opacity/clip/filter subtree".into());
    }
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => extract(g, out, db, allow_clip)?,
            usvg::Node::Text(t) => extract_text(t, out, db)?,
            usvg::Node::Path(p) if p.is_visible() => {
                if p.paint_order() != usvg::PaintOrder::FillAndStroke {
                    return Err("paint order".into());
                }
                if let Some(fill) = p.fill() {
                    let data = p
                        .data()
                        .clone()
                        .transform(p.abs_transform())
                        .ok_or("singular path")?;
                    out.paths.push(PaintPath {
                        data: Arc::new(data),
                        rgba: paint_color(fill.paint(), fill.opacity().get())?,
                        even_odd: fill.rule() == usvg::FillRule::EvenOdd,
                    });
                }
                if let Some(stroke) = p.stroke()
                    && let Some(data) = p
                        .data()
                        .stroke(&stroke.to_tiny_skia(), 1.)
                        .and_then(|path| path.transform(p.abs_transform()))
                {
                    out.paths.push(PaintPath {
                        data: Arc::new(data),
                        rgba: paint_color(stroke.paint(), stroke.opacity().get())?,
                        even_odd: false,
                    });
                }
            }
            usvg::Node::Path(_) => (),
            usvg::Node::Image(_) => return Err("embedded image subtree".into()),
        }
    }
    Ok(())
}
impl SceneCache {
    pub fn with_budget(bytes: usize) -> Self {
        Self {
            budget: bytes.min(CACHE_BUDGET),
            entries: BTreeMap::new(),
            clock: 0,
            stats: Stats::default(),
        }
    }
    pub fn build(
        &mut self,
        deck: &Deck,
        page: usize,
        document: u64,
        revision: u64,
        cancel: &AtomicBool,
    ) -> Result<Scene> {
        let started = Instant::now();
        deck.validate()?;
        let mut visible_bytes = 0usize;
        let slide = crate::design::resolve_slide(deck, page)?;
        preview::check_text_budget(&slide)?;
        let mut nodes = Vec::with_capacity(slide.objects.len());
        for object in &slide.objects {
            check_cancel(cancel)?;
            self.clock += 1;
            let mut local = object.clone();
            local.id.clear();
            local.binding = None;
            local.field = None;
            local.x = 0.;
            local.y = 0.;
            let key = serde_json::to_string(&local).map_err(|e| e.to_string())?;
            let geometry = if let Some((last, geometry)) = self.entries.get_mut(&key) {
                *last = self.clock;
                self.stats.hits += 1;
                geometry.clone()
            } else {
                let geometry = Arc::new(build_geometry(&local, &deck.resources, cancel)?);
                self.stats.builds += 1;
                if geometry.fallback.is_some() {
                    self.stats.local_rasters += 1;
                }
                while self.stats.bytes + geometry.bytes + key.len() > self.budget
                    && !self.entries.is_empty()
                {
                    let victim = self
                        .entries
                        .iter()
                        .min_by_key(|(_, (time, _))| *time)
                        .map(|(key, _)| key.clone())
                        .ok_or("场景缓存逐出候选缺失")?;
                    let (_, entry) = self.entries.remove(&victim).ok_or("场景缓存项已不存在")?;
                    self.stats.bytes -= entry.bytes + victim.len();
                    self.stats.evictions += 1;
                }
                if geometry.bytes + key.len() <= self.budget {
                    self.stats.bytes += geometry.bytes + key.len();
                    self.entries.insert(key, (self.clock, geometry.clone()));
                }
                geometry
            };
            visible_bytes = visible_bytes.saturating_add(geometry.bytes);
            if visible_bytes > 128 * 1024 * 1024 {
                return Err("visible scene exceeds 128 MiB budget".into());
            }
            nodes.push(Node {
                id: object.id.clone(),
                position: (object.x, object.y),
                geometry,
            });
        }
        self.stats.build_micros += started.elapsed().as_micros() as u64;
        Ok(Scene {
            document,
            revision,
            slide: slide.id.clone(),
            width: deck.width,
            height: deck.height,
            background: color(&slide.background, 1.),
            nodes,
        })
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.stats.bytes = 0;
    }
}
fn build_geometry(
    object: &Object,
    resources: &BTreeMap<String, Arc<crate::media::Resource>>,
    cancel: &AtomicBool,
) -> Result<Geometry> {
    let bounds = geometry::bounds(object);
    let margin = object.stroke_width / 2. + 2.;
    let offset = (bounds.x - margin, bounds.y - margin);
    let width = (bounds.w + margin * 2.).ceil().max(1.);
    let height = (bounds.h + margin * 2.).ceil().max(1.);
    let mut local = object.clone();
    local.x = -offset.0;
    local.y = -offset.1;
    let deck = Deck {
        playback: Default::default(),
        review: Default::default(),
        presentation: Default::default(),
        source: None,
        resources: resources.clone(),
        design: Default::default(),
        organization: Default::default(),
        field_date: String::new(),
        title: String::new(),
        width,
        height,
        warnings: vec![],
        slides: vec![Slide {
            hidden: false,
            layout: None,
            inherit_background: false,
            notes: String::new(),
            section: None,
            id: "local".into(),
            title: String::new(),
            background: "#ffffff".into(),
            objects: vec![local],
        }],
    };
    // Omit the generated page background; object subtrees remain isolated.
    let mut svg = preview::svg(&deck, 0, cancel)?;
    let start = svg
        .find("<rect width=\"100%\"")
        .ok_or("background missing")?;
    let end = start + svg[start..].find("/>").ok_or("background malformed")? + 2;
    svg.replace_range(start..end, "");
    let mut geometry = geometry_from_svg(
        &svg,
        width,
        height,
        offset,
        object.kind == "text" && object.rotation == 0. && object.stroke_width == 0.,
        cancel,
    )?;
    if geometry.clip.is_some() {
        geometry.clip = Some([-offset.0, -offset.1, object.width, object.height]);
    }
    Ok(geometry)
}
pub fn geometry_from_svg(
    svg: &str,
    width: f32,
    height: f32,
    offset: (f32, f32),
    text_clip: bool,
    cancel: &AtomicBool,
) -> Result<Geometry> {
    check_cancel(cancel)?;
    let tree = usvg::Tree::from_str(svg, &preview::options()).map_err(|e| e.to_string())?;
    let mut extracted = Extracted::default();
    let result = extract(tree.root(), &mut extracted, tree.fontdb(), text_clip);
    let Extracted {
        mut paths,
        mut masks,
        glyphs,
        missing: missing_glyphs,
        ..
    } = extracted;
    let reason = result.err();
    let fallback = if reason.is_some() {
        paths.clear();
        masks.clear();
        let scale = (2048. / width.max(height)).min(1.);
        let mut pixmap = sk::Pixmap::new(
            (width * scale).ceil().max(1.) as u32,
            (height * scale).ceil().max(1.) as u32,
        )
        .ok_or("local raster allocation")?;
        resvg::render(
            &tree,
            sk::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        Some(Bitmap {
            width: pixmap.width(),
            height: pixmap.height(),
            rgba: Arc::new(pixmap.take()),
        })
    } else {
        None
    };
    check_cancel(cancel)?;
    let bytes = paths
        .iter()
        .map(|p| p.data.points().len() * 8 + p.data.verbs().len())
        .sum::<usize>()
        * 2
        + glyphs * 512
        + fallback.as_ref().map_or(0, |b| b.rgba.len())
        + svg.len() * 3;
    Ok(Geometry {
        paths,
        glyph_masks: masks,
        fallback,
        reason,
        width,
        height,
        offset,
        clip: text_clip.then_some([
            -offset.0,
            -offset.1,
            width + offset.0 * 2.,
            height + offset.1 * 2.,
        ]),
        glyphs,
        missing_glyphs,
        bytes,
        tree: Some(tree),
    })
}
impl Scene {
    /// Full-page CPU reference, including the exact SVG shaping used for outlines.
    pub fn raster(&self, width: u32, cancel: &AtomicBool) -> Result<preview::Raster> {
        let scale = (width.clamp(1, 2048) as f32 / self.width).min(2048. / self.height);
        let w = (self.width * scale).ceil().max(1.) as u32;
        let h = (self.height * scale).ceil().max(1.) as u32;
        let mut pixmap = sk::Pixmap::new(w, h).ok_or("scene raster allocation")?;
        pixmap.fill(sk::Color::from_rgba8(
            self.background[0],
            self.background[1],
            self.background[2],
            255,
        ));
        for node in &self.nodes {
            check_cancel(cancel)?;
            let g = &node.geometry;
            let ts = sk::Transform::from_scale(scale, scale)
                .pre_translate(node.position.0 + g.offset.0, node.position.1 + g.offset.1);
            if let Some(tree) = &g.tree {
                resvg::render(tree, ts, &mut pixmap.as_mut());
            } else if let Some(bitmap) = &g.fallback {
                let image = sk::PixmapRef::from_bytes(&bitmap.rgba, bitmap.width, bitmap.height)
                    .ok_or("invalid scene bitmap")?;
                let transform = ts.pre_scale(
                    g.width / bitmap.width as f32,
                    g.height / bitmap.height as f32,
                );
                pixmap.draw_pixmap(
                    0,
                    0,
                    image,
                    &sk::PixmapPaint {
                        quality: sk::FilterQuality::Bilinear,
                        ..Default::default()
                    },
                    transform,
                    None,
                );
            }
        }
        let mut bgra = pixmap.take();
        for p in bgra.as_chunks_mut::<4>().0 {
            p.swap(0, 2);
        }
        Ok(preview::Raster {
            width: w,
            height: h,
            bgra,
        })
    }
    pub fn coverage(&self) -> (usize, usize, usize) {
        (
            self.nodes
                .iter()
                .filter(|n| n.geometry.fallback.is_none())
                .count(),
            self.nodes
                .iter()
                .filter(|n| n.geometry.fallback.is_some())
                .count(),
            self.nodes.iter().map(|n| n.geometry.glyphs).sum(),
        )
    }
}
