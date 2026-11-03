//! Adapted from JoyOffice apps/joydeck/src/scene_canvas.rs (MIT, pinned in source-map).
//! Uses the current DeckCraft model's display list; no legacy parallel document model.
use gpui_kit::*;
use joydeck_render::scene::{Geometry, Scene};
use std::{collections::BTreeMap, sync::Arc, time::Instant};

const BUDGET: usize = 64 << 20;
#[derive(Default, serde::Serialize)]
pub(super) struct Stats {
    pub frames: u64,
    pub tessellations: u64,
    pub texture_preparations: u64,
    pub texture_bytes: u64,
    pub atlas_removals: u64,
    pub cache_bytes: usize,
    pub draw_micros: u64,
    pub errors: u64,
}
struct Entry {
    _geometry: Arc<Geometry>,
    paths: Vec<(Path<Pixels>, [u8; 4])>,
    image: Option<Arc<RenderImage>>,
    bytes: usize,
    tick: u64,
}
#[derive(Default)]
pub(super) struct GpuCache {
    entries: BTreeMap<usize, Entry>,
    retired: Vec<Arc<RenderImage>>,
    scale: (u32, u32),
    tick: u64,
    pub stats: Stats,
}
impl GpuCache {
    pub fn clear(&mut self, w: &mut Window) {
        for (_, entry) in std::mem::take(&mut self.entries) {
            if let Some(image) = entry.image {
                self.retired.push(image);
            }
        }
        self.stats.cache_bytes = 0;
        self.release(w);
    }
    fn release(&mut self, w: &mut Window) {
        for image in self.retired.drain(..) {
            let _ = w.drop_image(image);
            self.stats.atlas_removals += 1;
        }
    }
    fn prepare(&mut self, g: &Arc<Geometry>, scale: f32) -> Result<usize, String> {
        let key = Arc::as_ptr(g) as usize;
        if let Some(e) = self.entries.get_mut(&key) {
            e.tick = self.tick;
            return Ok(key);
        }
        let mut paths = vec![];
        for p in &g.paths {
            let rule = if p.even_odd {
                FillRule::EvenOdd
            } else {
                FillRule::NonZero
            };
            let mut builder = PathBuilder::fill().with_style(PathStyle::Fill(
                FillOptions::default()
                    .with_fill_rule(rule)
                    .with_tolerance(0.1),
            ));
            let point_of =
                |p: joydeck_geom::Point| point(px(p.x as f32 * scale), px(p.y as f32 * scale));
            for el in p.path.elements() {
                match *el {
                    joydeck_geom::PathEl::MoveTo(p) => builder.move_to(point_of(p)),
                    joydeck_geom::PathEl::LineTo(p) => builder.line_to(point_of(p)),
                    joydeck_geom::PathEl::QuadTo(a, b) => {
                        builder.curve_to(point_of(b), point_of(a))
                    }
                    joydeck_geom::PathEl::CurveTo(a, b, c) => {
                        builder.cubic_bezier_to(point_of(c), point_of(a), point_of(b))
                    }
                    joydeck_geom::PathEl::ClosePath => builder.close(),
                }
            }
            paths.push((builder.build().map_err(|e| e.to_string())?, p.color));
            self.stats.tessellations += 1;
        }
        let image = if let Some(bitmap) = &g.bitmap {
            let mut bgra = bitmap.to_straight();
            for p in bgra.as_chunks_mut::<4>().0 {
                p.swap(0, 2);
            }
            let pixels = image::RgbaImage::from_raw(bitmap.width, bitmap.height, bgra)
                .ok_or("invalid scene pixels")?;
            self.stats.texture_preparations += 1;
            self.stats.texture_bytes += bitmap.pixels.len() as u64;
            Some(Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])))
        } else {
            None
        };
        let bytes = paths
            .iter()
            .map(|(p, _)| p.vertices.len() * std::mem::size_of::<PathVertex<Pixels>>())
            .sum::<usize>()
            + g.bitmap.as_ref().map_or(0, |b| b.pixels.len());
        if bytes > BUDGET {
            return Err("GPUI object exceeds 64 MiB cache budget".into());
        }
        while self.stats.cache_bytes.saturating_add(bytes) > BUDGET {
            let Some(key) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.tick)
                .map(|(k, _)| *k)
            else {
                break;
            };
            if let Some(e) = self.entries.remove(&key) {
                self.stats.cache_bytes = self.stats.cache_bytes.saturating_sub(e.bytes);
                if let Some(image) = e.image {
                    self.retired.push(image);
                }
            }
        }
        self.stats.cache_bytes += bytes;
        self.entries.insert(
            key,
            Entry {
                _geometry: g.clone(),
                paths,
                image,
                bytes,
                tick: self.tick,
            },
        );
        Ok(key)
    }
    pub fn paint(
        &mut self,
        scene: &Scene,
        bounds: Bounds<Pixels>,
        scale: f32,
        moved: &[joydeck_model::ShapeId],
        delta: Point<Pixels>,
        w: &mut Window,
    ) -> Result<(), String> {
        let stamp = (scale.to_bits(), w.scale_factor().to_bits());
        if stamp != self.scale {
            self.clear(w);
            self.scale = stamp;
        }
        self.tick += 1;
        self.release(w);
        let start = Instant::now();
        let result = w.with_content_mask(Some(ContentMask { bounds }), |w| {
            for node in &scene.nodes {
                let key = self.prepare(&node.geometry, scale)?;
                let e = &self.entries[&key];
                let origin = bounds.origin
                    + point(
                        px(node.origin.x as f32 * scale),
                        px(node.origin.y as f32 * scale),
                    )
                    + if node.id.is_some_and(|id| moved.contains(&id)) {
                        delta
                    } else {
                        point(px(0.), px(0.))
                    };
                for (path, c) in &e.paths {
                    let mut translated = path.clone();
                    translated.bounds.origin += origin;
                    for vertex in &mut translated.vertices {
                        vertex.xy_position += origin;
                    }
                    w.paint_path(translated, rgba(u32::from_be_bytes(*c)));
                }
                if let Some(image) = &e.image {
                    let r = node.geometry.bounds;
                    let target = Bounds::new(
                        origin + point(px(r.x0 as f32 * scale), px(r.y0 as f32 * scale)),
                        size(px(r.width() as f32 * scale), px(r.height() as f32 * scale)),
                    );
                    w.paint_image(
                        target,
                        target,
                        Corners::all(px(0.)),
                        image.clone(),
                        0,
                        false,
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
            Ok(())
        });
        self.release(w);
        self.stats.frames += 1;
        self.stats.draw_micros = start.elapsed().as_micros() as u64;
        if result.is_err() {
            self.stats.errors += 1;
        }
        result
    }
}
