//! GPUI-independent display list. Text consumes the CPU layout's exact face and glyph outlines;
//! consumers must not shape it again. Unsupported paints remain explicit CPU object tiles.
use std::{collections::BTreeMap, sync::Arc, time::Instant};

use joydeck_model::{Presentation, Shape, ShapeId, ShapeKind, resolve};
use kurbo::{Affine, BezPath, Point, Rect, Shape as _};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{Image, RenderOpts, SlidePart};

const BUDGET: usize = 48 << 20;
const MAX_PIXELS: usize = 16_000_000;

#[derive(Clone, Debug)]
pub struct Path {
    pub path: BezPath,
    pub color: [u8; 4],
    pub even_odd: bool,
}

/// Retains the exact font bytes, collection face and variable location through FontFace.
#[derive(Clone)]
pub struct GlyphSource {
    pub face: Arc<joydeck_fonts::FontFace>,
    pub glyph: u32,
    pub paragraph: usize,
    pub chars: (usize, usize),
}

pub struct Geometry {
    pub paths: Vec<Path>,
    pub glyphs: Vec<GlyphSource>,
    pub bitmap: Option<Image>,
    pub bounds: Rect,
    pub fallback: Option<String>,
    pub bytes: usize,
}

pub struct Node {
    pub id: Option<ShapeId>,
    pub origin: Point,
    pub geometry: Arc<Geometry>,
}

pub struct Scene {
    pub font_revision: u64,
    pub width: f64,
    pub height: f64,
    pub nodes: Vec<Node>,
    pub stats: Stats,
}
impl Scene {
    pub fn fonts_current(&self) -> bool {
        self.font_revision == joydeck_fonts::FontDb::global().revision()
    }
}

#[derive(Clone, Default, Debug, Serialize)]
pub struct Stats {
    pub builds: u64,
    pub hits: u64,
    pub misses: u64,
    pub cpu_tiles: u64,
    pub backdrop_rasters: u64,
    pub native_paths: usize,
    pub glyphs: usize,
    pub cache_bytes: usize,
    pub build_micros: u64,
}

struct Entry {
    geometry: Arc<Geometry>,
    tick: u64,
}

#[derive(Default)]
pub struct Cache {
    entries: BTreeMap<[u8; 32], Entry>,
    fallbacks: BTreeMap<[u8; 32], String>,
    tick: u64,
    pub stats: Stats,
}

fn digest(value: &impl Serialize) -> Result<[u8; 32], String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    Ok(Sha256::digest(bytes).into())
}

impl Cache {
    pub fn clear(&mut self) {
        self.entries.clear();
        self.fallbacks.clear();
        self.stats.cache_bytes = 0;
    }

    fn get(&mut self, key: [u8; 32]) -> Option<Arc<Geometry>> {
        let entry = self.entries.get_mut(&key)?;
        entry.tick = self.tick;
        self.stats.hits += 1;
        Some(entry.geometry.clone())
    }

    fn put(&mut self, key: [u8; 32], geometry: Geometry) -> Result<Arc<Geometry>, String> {
        if geometry.bytes > BUDGET {
            return Err("scene object exceeds 48 MiB resource budget".into());
        }
        while self.stats.cache_bytes.saturating_add(geometry.bytes) > BUDGET {
            let Some(old) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.tick)
                .map(|(k, _)| *k)
            else {
                break;
            };
            if let Some(entry) = self.entries.remove(&old) {
                self.stats.cache_bytes =
                    self.stats.cache_bytes.saturating_sub(entry.geometry.bytes);
            }
        }
        self.stats.misses += 1;
        self.stats.cache_bytes += geometry.bytes;
        let geometry = Arc::new(geometry);
        self.entries.insert(
            key,
            Entry {
                geometry: geometry.clone(),
                tick: self.tick,
            },
        );
        Ok(geometry)
    }

    /// One worker builds at a time. Cancellation is checked between objects; obsolete results
    /// are also rejected by the consumer's document/revision/page/scale key.
    pub fn build(
        &mut self,
        pres: &Presentation,
        page: usize,
        scale: f64,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Scene, String> {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for _ in 0..4 {
                let revision = joydeck_fonts::FontDb::global().revision();
                let scene = self.build_inner(pres, page, scale, cancelled)?;
                if revision == joydeck_fonts::FontDb::global().revision() {
                    return Ok(scene);
                }
            }
            Err("font fallback changed repeatedly while preparing scene; retry CPU view".into())
        }));
        match result {
            Ok(value) => value,
            Err(_) => {
                self.clear();
                Err("scene preparation panicked; cache cleared, use CPU view".into())
            }
        }
    }
    fn build_inner(
        &mut self,
        pres: &Presentation,
        page: usize,
        scale: f64,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Scene, String> {
        if !scale.is_finite() || !(0.05..=8.0).contains(&scale) {
            return Err("scene scale outside 0.05..8".into());
        }
        let started = Instant::now();
        let font_revision = joydeck_fonts::FontDb::global().revision();
        let slide = pres.slides.get(page).ok_or("scene page outside document")?;
        let ctx = resolve::Ctx::for_slide(pres, slide).ok_or("scene master missing")?;
        self.tick += 1;
        // Media data is skipped by model serialization. Include byte digests so replacement,
        // embedded fonts and reuse of media IDs cannot return stale cached pictures/glyphs.
        let media: Vec<_> = pres
            .media
            .iter()
            .map(|m| {
                (
                    m.id,
                    &m.name,
                    &m.content_type,
                    &m.link,
                    Sha256::digest(m.data.as_slice()).to_vec(),
                )
            })
            .collect();
        let environment = digest(&(
            &pres.masters,
            &pres.slide_size,
            &pres.default_text_style,
            &pres.header_footer,
            &pres.embedded_fonts,
            &pres.table_styles,
            &media,
            slide.layout,
            &slide.color_map,
            pres.slide_number(page),
            font_revision,
        ))?;
        let backdrop_key = digest(&(
            environment,
            &slide.background,
            slide.show_master_shapes,
            scale.to_bits(),
        ))?;
        let backdrop = if let Some(g) = self.get(backdrop_key) {
            g
        } else {
            check_image_size(pres.slide_size.width, pres.slide_size.height, scale)?;
            let image = crate::render_slide_part(
                pres,
                page,
                SlidePart::Backdrop,
                &RenderOpts {
                    scale,
                    ..Default::default()
                },
            );
            self.stats.backdrop_rasters += 1;
            self.put(
                backdrop_key,
                bitmap(
                    image,
                    Rect::new(0., 0., pres.slide_size.width, pres.slide_size.height),
                    None,
                ),
            )?
        };
        let mut nodes = vec![Node {
            id: None,
            origin: Point::ORIGIN,
            geometry: backdrop,
        }];
        for shape in slide.shapes.iter().filter(|s| !s.hidden) {
            if cancelled() {
                return Err("scene cancelled".into());
            }
            let mut uses_background = false;
            joydeck_model::walk(std::slice::from_ref(shape), &mut |s, _| {
                uses_background |= matches!(
                    resolve::fill(&ctx, s).0,
                    Some(joydeck_model::Fill::Background)
                );
            });
            if uses_background {
                return Err("background-dependent fill requires CPU view".into());
            }
            let x = resolve::xfrm(&ctx, shape);
            if !x.is_finite() {
                return Err("nonfinite scene transform".into());
            }
            let origin = Point::new(x.x, x.y);
            let mut local = shape.clone();
            let mut normalized = x;
            normalized.x = 0.;
            normalized.y = 0.;
            local.xfrm = Some(normalized);
            let key = digest(&(environment, &local, scale.to_bits()))?;
            let phase = (
                (origin.x * scale).rem_euclid(1.),
                (origin.y * scale).rem_euclid(1.),
            );
            let phase = (
                (phase.0 * 1_000_000.).round() as u32,
                (phase.1 * 1_000_000.).round() as u32,
            );
            let tile_key = digest(&(key, phase))?;
            let geometry = if let Some(g) = self.get(key).or_else(|| self.get(tile_key)) {
                g
            } else {
                // Build against the same inheritance context, with a normalized object origin.
                let mut isolated = pres.clone();
                let local_slide = Arc::make_mut(&mut isolated.slides[page]);
                local_slide.shapes = vec![local.clone()];
                let (g, resource_key) = match self
                    .fallbacks
                    .get(&key)
                    .cloned()
                    .map_or_else(|| native(&isolated, page, &local), Err)
                {
                    Ok(g) => (g, key),
                    Err(reason) => {
                        if reason.contains("inherited background") {
                            return Err("background-dependent fill requires CPU view".into());
                        }
                        if self.fallbacks.len() > 4096 {
                            self.fallbacks.clear();
                        }
                        self.fallbacks.insert(key, reason.clone());
                        self.stats.cpu_tiles += 1;
                        let offset = Point::new(
                            f64::from(phase.0) / 1_000_000. / scale,
                            f64::from(phase.1) / 1_000_000. / scale,
                        );
                        let mut phased = local.clone();
                        if let Some(x) = &mut phased.xfrm {
                            x.x = offset.x;
                            x.y = offset.y;
                        }
                        Arc::make_mut(&mut isolated.slides[page]).shapes = vec![phased.clone()];
                        let mut tile = object_tile(&isolated, page, &phased, scale, reason)?;
                        tile.bounds = tile.bounds - offset.to_vec2();
                        (tile, tile_key)
                    }
                };
                self.put(resource_key, g)?
            };
            nodes.push(Node {
                id: Some(shape.id),
                origin,
                geometry,
            });
            if nodes.iter().map(|n| n.geometry.bytes).sum::<usize>() > BUDGET {
                return Err("visible scene exceeds 48 MiB; use CPU view or reduce zoom".into());
            }
        }
        self.stats.builds += 1;
        self.stats.native_paths = nodes.iter().map(|n| n.geometry.paths.len()).sum();
        self.stats.glyphs = nodes.iter().map(|n| n.geometry.glyphs.len()).sum();
        self.stats.build_micros = started.elapsed().as_micros() as u64;
        Ok(Scene {
            font_revision,
            width: pres.slide_size.width,
            height: pres.slide_size.height,
            nodes,
            stats: self.stats.clone(),
        })
    }
}

/// Headless reference consumer for display-list parity and repeatable cache measurements.
/// This validates scene decomposition; it does not stand in for GPUI screenshot comparisons.
pub fn raster(scene: &Scene, scale: f64) -> Result<Image, String> {
    let (w, h) = check_image_size(scene.width, scene.height, scale)?;
    let mut ctx = vello_cpu::RenderContext::new_with(
        w,
        h,
        vello_cpu::RenderSettings {
            num_threads: 0,
            ..Default::default()
        },
    );
    for node in &scene.nodes {
        let transform = Affine::scale(scale) * Affine::translate(node.origin.to_vec2());
        for path in &node.geometry.paths {
            ctx.set_transform(transform);
            ctx.set_fill_rule(if path.even_odd {
                vello_cpu::peniko::Fill::EvenOdd
            } else {
                vello_cpu::peniko::Fill::NonZero
            });
            let [r, g, b, a] = path.color;
            ctx.set_paint(vello_cpu::peniko::Color::from_rgba8(r, g, b, a));
            ctx.fill_path(&path.path);
        }
        if let Some(image) = &node.geometry.bitmap {
            let pixels = image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| vello_cpu::color::PremulRgba8 {
                    r: p[0],
                    g: p[1],
                    b: p[2],
                    a: p[3],
                })
                .collect();
            let pixmap = Arc::new(vello_cpu::Pixmap::from_parts(
                pixels,
                image.width as u16,
                image.height as u16,
            ));
            let r = node.geometry.bounds;
            ctx.set_transform(
                transform
                    * Affine::translate(r.origin().to_vec2())
                    * Affine::scale_non_uniform(
                        r.width() / f64::from(image.width),
                        r.height() / f64::from(image.height),
                    ),
            );
            ctx.set_paint(vello_cpu::Image {
                image: vello_cpu::ImageSource::Pixmap(pixmap),
                sampler: Default::default(),
            });
            ctx.fill_rect(&Rect::new(
                0.,
                0.,
                f64::from(image.width),
                f64::from(image.height),
            ));
        }
    }
    Ok(crate::Renderer::default().finish(ctx, w, h))
}

fn rgba(c: joydeck_color::Rgba) -> [u8; 4] {
    [c.r, c.g, c.b, c.a]
}

fn native(pres: &Presentation, page: usize, s: &Shape) -> Result<Geometry, String> {
    if !matches!(s.kind, ShapeKind::Shape) {
        return Err(match &s.kind {
            ShapeKind::Picture { .. } => "picture decode/crop/adjust",
            ShapeKind::Table(_) => "table",
            ShapeKind::Chart(_) => "chart",
            ShapeKind::Group { .. } => "group/compositing",
            _ => "complex content",
        }
        .into());
    }
    let ctx = resolve::Ctx::for_slide(pres, &pres.slides[page]).ok_or("master missing")?;
    let x = resolve::xfrm(&ctx, s);
    let (fx, _) = resolve::effects(&ctx, s);
    if fx.is_some_and(|f| !f.is_empty() || f.raw3d.is_some()) {
        return Err("effects/3D".into());
    }
    let (fill, ph) = resolve::fill(&ctx, s);
    let fill_color = match fill {
        None | Some(joydeck_model::Fill::None) => None,
        Some(joydeck_model::Fill::Solid { color }) => Some(ctx.color(&color, ph)),
        Some(joydeck_model::Fill::Background) => return Err("inherited background".into()),
        Some(joydeck_model::Fill::Gradient(_)) => return Err("gradient".into()),
        Some(joydeck_model::Fill::Pattern(_)) => return Err("pattern".into()),
        _ => return Err("picture/group fill".into()),
    };
    let (line, line_ph) = resolve::line(&ctx, s);
    let line_color = match &line.fill {
        None | Some(joydeck_model::Fill::None) => None,
        Some(joydeck_model::Fill::Solid { color }) => Some(ctx.color(color, line_ph)),
        _ => return Err("complex outline paint".into()),
    };
    if line_color.is_some()
        && (line.dash.is_some()
            || line.head.is_some()
            || line.tail.is_some()
            || line.compound.is_some()
            || line.align.is_some())
    {
        return Err("dashed/compound/arrow outline".into());
    }
    let mut paths = vec![];
    let geometry = crate::shape_geometry(s, x.w, x.h);
    for sub in geometry.paths {
        if let Some(c) = fill_color.filter(|_| sub.fill != joydeck_geom::preset::FillMode::None) {
            paths.push(Path {
                path: x.affine() * &sub.path,
                color: rgba(crate::shade(c, sub.fill)),
                even_odd: sub.even_odd,
            });
        }
        if let Some(c) = line_color.filter(|_| sub.stroke) {
            use joydeck_model::style::{LineCap, LineJoin};
            let cap = match line.cap.unwrap_or_default() {
                LineCap::Flat => kurbo::Cap::Butt,
                LineCap::Round => kurbo::Cap::Round,
                LineCap::Square => kurbo::Cap::Square,
            };
            let join = match line.join.unwrap_or_default() {
                LineJoin::Round => kurbo::Join::Round,
                LineJoin::Bevel => kurbo::Join::Bevel,
                LineJoin::Miter => kurbo::Join::Miter,
            };
            let stroke = kurbo::Stroke::new(line.width.unwrap_or(0.75))
                .with_caps(cap)
                .with_join(join)
                .with_miter_limit(f64::from(line.miter_limit.unwrap_or(8)));
            paths.push(Path {
                path: x.affine()
                    * kurbo::stroke(
                        sub.path.iter(),
                        &stroke,
                        &kurbo::StrokeOpts::default(),
                        0.05,
                    ),
                color: rgba(c),
                even_odd: false,
            });
        }
    }
    let mut glyphs = vec![];
    if s.text.as_ref().is_some_and(|t| !t.is_empty()) {
        let placed = crate::editable_text(pres, page, s.id, None).ok_or("text layout missing")?;
        if !placed.layout.decos.is_empty()
            || crate::text_warp(&ctx, s, crate::shape_geometry(s, x.w, x.h).text_rect).is_some()
        {
            return Err("text decoration/warp".into());
        }
        for run in &placed.layout.runs {
            if run.face.is_color()
                || run.fake_bold
                || run.outline.is_some()
                || run.gradient.is_some()
                || run.effects.is_some()
            {
                return Err("color/synthetic/effect text".into());
            }
            let mut path = BezPath::new();
            let skew = if run.fake_italic {
                Affine::new([1., 0., -0.2, 1., 0., 0.])
            } else {
                Affine::IDENTITY
            };
            for &(gid, gx, gy) in &run.glyphs {
                let outline = crate::glyph(&run.face, gid);
                let at = placed.transform
                    * crate::glyph_place(run, gid, gx, gy)
                    * skew
                    * Affine::scale(run.size / run.face.upem.max(1.));
                path.extend(outline.elements().iter().map(|e| at * *e));
                glyphs.push(GlyphSource {
                    face: run.face.clone(),
                    glyph: gid,
                    paragraph: run.para,
                    chars: run.chars,
                });
            }
            let mut c = run.color;
            c.a = (f64::from(c.a) * run.alpha.clamp(0., 1.)).round() as u8;
            paths.push(Path {
                path,
                color: rgba(c),
                even_odd: false,
            });
        }
    }
    let bounds = paths
        .iter()
        .map(|p| p.path.bounding_box())
        .reduce(|a, b| a.union(b))
        .unwrap_or(x.bounds());
    let bytes = paths
        .iter()
        .map(|p| std::mem::size_of_val(p.path.elements()))
        .sum::<usize>()
        + glyphs.len() * std::mem::size_of::<GlyphSource>();
    Ok(Geometry {
        paths,
        glyphs,
        bitmap: None,
        bounds,
        fallback: None,
        bytes,
    })
}

fn check_image_size(w: f64, h: f64, scale: f64) -> Result<(u16, u16), String> {
    let (w, h) = ((w * scale).ceil(), (h * scale).ceil());
    if !w.is_finite()
        || !h.is_finite()
        || !(1.0..=8192.).contains(&w)
        || !(1.0..=8192.).contains(&h)
        || w * h > MAX_PIXELS as f64
    {
        return Err(
            "scene tile exceeds 8192 side / 16 MP budget; use CPU view or reduce zoom".into(),
        );
    }
    Ok((w as u16, h as u16))
}

fn bitmap(image: Image, bounds: Rect, fallback: Option<String>) -> Geometry {
    Geometry {
        bytes: image.pixels.len(),
        bitmap: Some(image),
        paths: vec![],
        glyphs: vec![],
        bounds,
        fallback,
    }
}

fn object_tile(
    pres: &Presentation,
    page: usize,
    s: &Shape,
    scale: f64,
    reason: String,
) -> Result<Geometry, String> {
    let ctx = resolve::Ctx::for_slide(pres, &pres.slides[page]).ok_or("master missing")?;
    let x = resolve::xfrm(&ctx, s);
    let (line, _) = resolve::line(&ctx, s);
    let (effects, _) = resolve::effects(&ctx, s);
    let pad = crate::extent::spill(Some(&line), effects.as_ref(), x.w, x.h).max(8.);
    let mut bounds = x.bounds().inflate(pad, pad);
    bounds = Rect::new(
        (bounds.x0 * scale).floor() / scale,
        (bounds.y0 * scale).floor() / scale,
        (bounds.x1 * scale).ceil() / scale,
        (bounds.y1 * scale).ceil() / scale,
    );
    let mut renderer = crate::Renderer::default();
    // First pass discovers real painted extents, including overflowing text and group children.
    for pass in 0..2 {
        let (w, h) = check_image_size(bounds.width(), bounds.height(), scale)?;
        let opts = RenderOpts {
            scale,
            ..Default::default()
        };
        let f = crate::renderer::Frame {
            pres,
            opts: &opts,
            fields: crate::SlideFields::new(
                pres.slide_number(page),
                pres.header_footer.footer_text.clone(),
                &opts,
            ),
        };
        let mut target = vello_cpu::RenderContext::new_with(
            w,
            h,
            vello_cpu::RenderSettings {
                num_threads: 0,
                ..Default::default()
            },
        );
        renderer.drawn = None;
        renderer.shape(
            &mut target,
            &f,
            &ctx,
            s,
            Affine::scale(scale) * Affine::translate(-bounds.origin().to_vec2()),
            0,
        );
        let drawn = renderer.drawn.map(|r| {
            Rect::new(
                bounds.x0 + r.x0 / scale,
                bounds.y0 + r.y0 / scale,
                bounds.x0 + r.x1 / scale,
                bounds.y0 + r.y1 / scale,
            )
        });
        if pass == 0
            && let Some(r) = drawn
            && bounds.union(r).area() > bounds.area() + 0.01
        {
            bounds = bounds.union(r).inflate(2. / scale, 2. / scale);
            bounds = Rect::new(
                (bounds.x0 * scale).floor() / scale,
                (bounds.y0 * scale).floor() / scale,
                (bounds.x1 * scale).ceil() / scale,
                (bounds.y1 * scale).ceil() / scale,
            );
            continue;
        }
        // Match image dimensions exactly to avoid stretching ceil-rounded edge pixels.
        bounds.x1 = bounds.x0 + f64::from(w) / scale;
        bounds.y1 = bounds.y0 + f64::from(h) / scale;
        return Ok(bitmap(renderer.finish(target, w, h), bounds, Some(reason)));
    }
    Err("object extent did not converge".into())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test assertions"
)]
mod tests {
    use super::*;
    use joydeck_model::{ColorRef, Fill, Line, Rgba, TextBody};
    fn deck() -> Presentation {
        let mut p = Presentation::default();
        Arc::make_mut(&mut p.slides[0]).shapes = vec![Shape {
            id: ShapeId(901),
            xfrm: Some(joydeck_geom::Xfrm::new(54., 66., 240., 120.)),
            fill: Some(Fill::solid(ColorRef::rgb(Rgba::rgb(26, 120, 208)))),
            line: Some(Line::none()),
            text: Some(TextBody::from_text("Scene / 中文")),
            ..Default::default()
        }];
        p
    }
    // Other renderer tests discover system fonts concurrently. Cache identity assertions
    // require two observations of the same font environment, just as the desktop cache does.
    fn stable_pair(
        cache: &mut Cache,
        before: &Presentation,
        after: &Presentation,
    ) -> (Scene, Scene) {
        for _ in 0..128 {
            let a = cache.build(before, 0, 1., &|| false).unwrap();
            let b = cache.build(after, 0, 1., &|| false).unwrap();
            if a.font_revision == b.font_revision {
                return (a, b);
            }
        }
        panic!("font environment did not stabilize for cache identity check")
    }
    #[test]
    fn translation_reuses_geometry_and_exact_glyphs() {
        let mut p = deck();
        let before = p.clone();
        let mut cache = Cache::default();
        Arc::make_mut(&mut p.slides[0]).shapes[0]
            .xfrm
            .as_mut()
            .unwrap()
            .x += 12.;
        let (first, moved) = stable_pair(&mut cache, &before, &p);
        assert!(!first.nodes[1].geometry.paths.is_empty());
        assert!(!first.nodes[1].geometry.glyphs.is_empty());
        for glyph in &first.nodes[1].geometry.glyphs {
            assert!(!glyph.face.data().is_empty());
        }
        assert!(Arc::ptr_eq(
            &first.nodes[1].geometry,
            &moved.nodes[1].geometry
        ));
        assert_eq!(cache.stats.misses, first.stats.misses);
        assert_eq!(cache.stats.cpu_tiles, 0);
        assert_eq!(cache.stats.backdrop_rasters, first.stats.backdrop_rasters);
    }
    #[test]
    fn scene_reference_matches_cpu_and_does_not_reshape() {
        let p = deck();
        let s = Cache::default().build(&p, 0, 1., &|| false).unwrap();
        let actual = raster(&s, 1.).unwrap();
        let expected = crate::render_slide(&p, 0, &RenderOpts::default());
        let bad = actual
            .pixels
            .iter()
            .zip(&expected.pixels)
            .filter(|(a, b)| a.abs_diff(**b) > 12)
            .count();
        assert!(
            bad < actual.pixels.len() / 100,
            "scene differs in {bad} channels"
        );
    }
    #[test]
    fn resources_scale_theme_and_edit_invalidate_but_notes_do_not() {
        let mut p = deck();
        let mut cache = Cache::default();
        let before = p.clone();
        Arc::make_mut(&mut p.slides[0]).notes = TextBody::from_text("notes only");
        let (first, notes) = stable_pair(&mut cache, &before, &p);
        assert!(Arc::ptr_eq(
            &first.nodes[1].geometry,
            &notes.nodes[1].geometry
        ));
        let dpi = cache.build(&p, 0, 2., &|| false).unwrap();
        assert!(!Arc::ptr_eq(
            &notes.nodes[1].geometry,
            &dpi.nodes[1].geometry
        ));
        Arc::make_mut(&mut p.masters[0]).theme.fonts.minor.latin = "Arial".into();
        let theme = cache.build(&p, 0, 1., &|| false).unwrap();
        assert!(!Arc::ptr_eq(
            &notes.nodes[1].geometry,
            &theme.nodes[1].geometry
        ));
        assert!(cache.build(&p, 0, 1., &|| true).is_err());
        assert!(cache.build(&p, 0, f64::NAN, &|| false).is_err());
        cache.clear();
        assert_eq!(cache.stats.cache_bytes, 0);
    }
    #[test]
    fn gradient_is_a_local_cached_tile_and_background_fill_requires_cpu_view() {
        use joydeck_model::style::{Gradient, GradientShape, GradientStop};
        let mut p = deck();
        Arc::make_mut(&mut p.slides[0]).shapes[0].text = None;
        Arc::make_mut(&mut p.slides[0]).shapes[0].fill = Some(Fill::Gradient(Gradient {
            stops: vec![
                GradientStop {
                    pos: 0.,
                    color: ColorRef::rgb(Rgba::rgb(20, 90, 180)),
                },
                GradientStop {
                    pos: 1.,
                    color: ColorRef::rgb(Rgba::rgb(180, 220, 250)),
                },
            ],
            shape: GradientShape::Linear {
                angle: 45.,
                scaled: false,
            },
            flip: None,
            rotate_with_shape: true,
            tile: [0.; 4],
        }));
        let mut cache = Cache::default();
        let first = cache.build(&p, 0, 1., &|| false).unwrap();
        assert_eq!(
            first.nodes[1].geometry.fallback.as_deref(),
            Some("gradient")
        );
        assert!(first.nodes[1].geometry.bitmap.as_ref().unwrap().width < 400);
        let actual = raster(&first, 1.).unwrap();
        let expected = crate::render_slide(&p, 0, &RenderOpts::default());
        assert!(
            actual
                .pixels
                .iter()
                .zip(&expected.pixels)
                .all(|(a, b)| a.abs_diff(*b) <= 4)
        );
        let before = p.clone();
        Arc::make_mut(&mut p.slides[0]).shapes[0]
            .xfrm
            .as_mut()
            .unwrap()
            .x += 12.;
        let (cached, moved) = stable_pair(&mut cache, &before, &p);
        assert!(Arc::ptr_eq(
            &cached.nodes[1].geometry,
            &moved.nodes[1].geometry
        ));
        assert_eq!(cache.stats.cpu_tiles, cached.stats.cpu_tiles);
        Arc::make_mut(&mut p.slides[0]).shapes[0].fill = Some(Fill::Background);
        assert!(cache.build(&p, 0, 1., &|| false).is_err());
    }
}
