//! Window-local GPUI adapter. Derived geometry/images never enter documents.
use super::*;
use joydeck::scene::{self, Scene};
use resvg::tiny_skia::PathSegment;
use std::sync::Mutex;

#[derive(Default, Clone, serde::Serialize)]
pub(super) struct PaintStats {
    pub frames: u64,
    pub paths: u64,
    pub images: u64,
    pub glyph_masks: u64,
    pub fallbacks: u64,
    pub uploads: u64,
    pub atlas_removals: u64,
    pub upload_bytes: u64,
    pub tessellations: u64,
    pub cpu_raster_micros: u64,
    pub resource_prepare_micros: u64,
    pub full_rasters: u64,
    pub paint_micros: u64,
    pub gpu_cache_bytes: usize,
}
struct GlyphImages {
    image: Arc<RenderImage>,
    pixel_bounds: [f32; 4],
    position: (f32, f32),
}
struct Entry {
    _geometry: Arc<scene::Geometry>,
    paths: Vec<(Path<Pixels>, [u8; 4])>,
    image: Option<Arc<RenderImage>>,
    image_inset: (f32, f32),
    glyph_images: Vec<GlyphImages>,
    bytes: usize,
    tick: u64,
}
#[derive(Default)]
pub(super) struct GpuCache {
    entries: BTreeMap<usize, Entry>,
    retired: Vec<Arc<RenderImage>>,
    scale: (u32, u32),
    tick: u64,
    pub stats: PaintStats,
}
impl GpuCache {
    fn retire(&mut self, entry: Entry) {
        let mut images = BTreeMap::new();
        if let Some(image) = entry.image {
            images.insert(Arc::as_ptr(&image) as usize, image);
        }
        for glyph in entry.glyph_images {
            images.insert(Arc::as_ptr(&glyph.image) as usize, glyph.image);
        }
        self.retired.extend(images.into_values());
    }
    pub(super) fn clear(&mut self) {
        for (_, entry) in std::mem::take(&mut self.entries) {
            self.retire(entry);
        }
        self.stats.gpu_cache_bytes = 0;
    }
    pub(super) fn release_images(&mut self, w: &mut Window) {
        for image in self.retired.drain(..) {
            if w.drop_image(image).is_ok() {
                self.stats.atlas_removals += 1;
            }
        }
    }
    fn prepare(
        &mut self,
        node: &scene::Node,
        scale: f32,
        dpi: f32,
    ) -> std::result::Result<usize, String> {
        if self.scale != (scale.to_bits(), dpi.to_bits()) {
            self.clear();
            self.scale = (scale.to_bits(), dpi.to_bits());
        }
        self.tick += 1;
        let key = Arc::as_ptr(&node.geometry) as usize;
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.tick = self.tick;
            return Ok(key);
        }
        let prepared = Instant::now();
        let mut paths = vec![];
        for p in &node.geometry.paths {
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
            let point_of = |p: resvg::tiny_skia::Point| point(px(p.x * scale), px(p.y * scale));
            for segment in p.data.segments() {
                match segment {
                    PathSegment::MoveTo(p) => builder.move_to(point_of(p)),
                    PathSegment::LineTo(p) => builder.line_to(point_of(p)),
                    PathSegment::QuadTo(c, p) => builder.curve_to(point_of(p), point_of(c)),
                    PathSegment::CubicTo(a, b, p) => {
                        builder.cubic_bezier_to(point_of(p), point_of(a), point_of(b))
                    }
                    PathSegment::Close => builder.close(),
                }
            }
            let path = builder
                .build()
                .map_err(|e| format!("GPUI tessellation: {e}"))?;
            paths.push((path, p.rgba));
            self.stats.tessellations += 1;
        }
        let effective = scale * dpi;
        let mut masks: BTreeMap<(usize, u32), (Arc<RenderImage>, [f32; 4])> = BTreeMap::new();
        let mut glyph_images = vec![];
        let mut mask_bytes = 0usize;
        for glyph in &node.geometry.glyph_masks {
            let id = (
                Arc::as_ptr(&glyph.path) as usize,
                u32::from_be_bytes(glyph.rgba),
            );
            if let std::collections::btree_map::Entry::Vacant(entry) = masks.entry(id) {
                let b = glyph.path.bounds();
                let left = (b.left() * effective).floor() - 1.;
                let top = (b.top() * effective).floor() - 1.;
                let width = ((b.right() * effective).ceil() + 2. - left).max(1.) as u32;
                let height = ((b.bottom() * effective).ceil() + 2. - top).max(1.) as u32;
                let bytes = width as usize * height as usize * 4 * 16;
                if bytes > 16 * 1024 * 1024 || mask_bytes + bytes > scene::CACHE_BUDGET {
                    return Err("glyph mask exceeds budget".into());
                }
                let mut sheet = resvg::tiny_skia::Pixmap::new(width * 4, height * 4)
                    .ok_or("glyph allocation")?;
                let mut paint = resvg::tiny_skia::Paint::default();
                let c = glyph.rgba;
                paint.set_color_rgba8(c[0], c[1], c[2], c[3]);
                for y in 0..4 {
                    for x in 0..4 {
                        let ts = resvg::tiny_skia::Transform::from_scale(effective, effective)
                            .post_translate(
                                -left + x as f32 / 4. + (x * width) as f32,
                                -top + y as f32 / 4. + (y * height) as f32,
                            );
                        sheet.fill_path(
                            &glyph.path,
                            &paint,
                            resvg::tiny_skia::FillRule::Winding,
                            ts,
                            None,
                        );
                    }
                }
                let bitmap = scene::Bitmap {
                    width: width * 4,
                    height: height * 4,
                    rgba: Arc::new(sheet.take()),
                };
                let image = Arc::new(RenderImage::new(vec![image::Frame::new(
                    image::RgbaImage::from_raw(width * 4, height * 4, bitmap.gpui_bgra())
                        .ok_or("glyph image")?,
                )]));
                self.stats.uploads += 1;
                self.stats.upload_bytes += bytes as u64;
                mask_bytes += bytes;
                entry.insert((image, [left, top, width as f32, height as f32]));
            }
            let (image, bounds) = &masks[&id];
            glyph_images.push(GlyphImages {
                image: image.clone(),
                pixel_bounds: *bounds,
                position: glyph.position,
            });
        }
        let mut image_bytes = 0;
        let mut image_inset = (0., 0.);
        let image = if let Some(b) = node.geometry.fallback.as_ref() {
            // paint_image snaps its quad to device pixels. Give the texture a
            // transparent guard and crop on an integral quad instead, so that
            // fractional translation does not stretch/shift the source image.
            let g = &node.geometry;
            let pad_x = (2. * b.width as f32 / (g.width * effective)).ceil() as u32;
            let pad_y = (2. * b.height as f32 / (g.height * effective)).ceil() as u32;
            let width = b
                .width
                .checked_add(pad_x.saturating_mul(2))
                .ok_or("image width")?;
            let height = b
                .height
                .checked_add(pad_y.saturating_mul(2))
                .ok_or("image height")?;
            image_bytes = (width as usize)
                .checked_mul(height as usize)
                .and_then(|n| n.checked_mul(4))
                .ok_or("image bytes")?;
            if image_bytes > scene::CACHE_BUDGET {
                return Err("padded image exceeds budget".into());
            }
            let bgra = b.gpui_bgra();
            let mut padded = vec![0; image_bytes];
            for y in 0..b.height as usize {
                let start = ((y + pad_y as usize) * width as usize + pad_x as usize) * 4;
                padded[start..start + b.width as usize * 4].copy_from_slice(
                    &bgra[y * b.width as usize * 4..(y + 1) * b.width as usize * 4],
                );
            }
            image_inset = (
                g.width * pad_x as f32 / b.width as f32,
                g.height * pad_y as f32 / b.height as f32,
            );
            self.stats.uploads += 1;
            self.stats.upload_bytes += image_bytes as u64;
            Some(Arc::new(RenderImage::new(vec![image::Frame::new(
                image::RgbaImage::from_raw(width, height, padded)
                    .ok_or("padded image dimensions invalid")?,
            )])))
        } else {
            None
        };
        let bytes = paths
            .iter()
            .map(|(p, _)| p.vertices.len() * std::mem::size_of::<PathVertex<Pixels>>())
            .sum::<usize>()
            + image_bytes
            + mask_bytes;
        if bytes > scene::CACHE_BUDGET {
            return Err("GPU object exceeds 64 MiB budget".into());
        }
        self.stats.resource_prepare_micros += prepared.elapsed().as_micros() as u64;
        while self.stats.gpu_cache_bytes + bytes > scene::CACHE_BUDGET && !self.entries.is_empty() {
            let Some((&key, _)) = self.entries.iter().min_by_key(|(_, entry)| entry.tick) else {
                return Err("GPU cache state inconsistent".into());
            };
            if let Some(e) = self.entries.remove(&key) {
                self.stats.gpu_cache_bytes -= e.bytes;
                self.retire(e);
            }
        }
        self.stats.gpu_cache_bytes += bytes;
        self.entries.insert(
            key,
            Entry {
                _geometry: node.geometry.clone(),
                paths,
                image,
                image_inset,
                glyph_images,
                bytes,
                tick: self.tick,
            },
        );
        Ok(key)
    }
    pub(super) fn paint(
        &mut self,
        scene: &Scene,
        objects: &[Object],
        skip: Option<&str>,
        bounds: Bounds<Pixels>,
        scale: f32,
        w: &mut Window,
    ) -> std::result::Result<(), String> {
        self.release_images(w);
        let began = Instant::now();
        self.stats.frames += 1;
        w.with_content_mask(Some(ContentMask { bounds }), |w| {
            let c = scene.background;
            w.paint_quad(fill(bounds, rgba(u32::from_be_bytes(c))));
            for node in &scene.nodes {
                if skip == Some(node.id.as_str()) {
                    continue;
                }
                let g = &node.geometry;
                let position = objects
                    .iter()
                    .find(|o| o.id == node.id)
                    .map_or(node.position, |o| (o.x, o.y));
                let origin = point(
                    bounds.left() + px((position.0 + g.offset.0) * scale),
                    bounds.top() + px((position.1 + g.offset.1) * scale),
                );
                let key = self.prepare(node, scale, w.scale_factor())?;
                let entry = &self.entries[&key];
                let clip = g.clip.map(|r| {
                    Bounds::new(
                        origin + point(px(r[0] * scale), px(r[1] * scale)),
                        size(px(r[2] * scale), px(r[3] * scale)),
                    )
                });
                w.with_content_mask(clip.map(|bounds| ContentMask { bounds }), |w| {
                    for (path, color) in &entry.paths {
                        let mut translated = path.clone();
                        translated.bounds.origin += origin;
                        for vertex in &mut translated.vertices {
                            vertex.xy_position += origin;
                        }
                        w.paint_path(translated, rgba(u32::from_be_bytes(*color)));
                        self.stats.paths += 1;
                    }
                    let dpi = w.scale_factor();
                    for glyph in &entry.glyph_images {
                        let x = ((f32::from(origin.x) + glyph.position.0 * scale) * dpi * 4.)
                            .round()
                            / 4.;
                        let y = ((f32::from(origin.y) + glyph.position.1 * scale) * dpi * 4.)
                            .round()
                            / 4.;
                        let ix = x.floor();
                        let iy = y.floor();
                        let phase_x = ((x - ix) * 4.).round();
                        let phase_y = ((y - iy) * 4.).round();
                        let r = glyph.pixel_bounds;
                        let target = Bounds::new(
                            point(px((ix + r[0]) / dpi), px((iy + r[1]) / dpi)),
                            size(px(r[2] / dpi), px(r[3] / dpi)),
                        );
                        let sheet = Bounds::new(
                            target.origin
                                - point(px(phase_x * r[2] / dpi), px(phase_y * r[3] / dpi)),
                            size(px(r[2] * 4. / dpi), px(r[3] * 4. / dpi)),
                        );
                        w.paint_image(
                            target,
                            sheet,
                            Corners::all(px(0.)),
                            glyph.image.clone(),
                            0,
                            false,
                        )
                        .map_err(|e| e.to_string())?;
                        self.stats.glyph_masks += 1;
                    }
                    if let Some(image) = &entry.image {
                        let target = Bounds::from_corners(
                            point(
                                px((f32::from(origin.x) * dpi).floor() / dpi - 1. / dpi),
                                px((f32::from(origin.y) * dpi).floor() / dpi - 1. / dpi),
                            ),
                            point(
                                px(((f32::from(origin.x) + g.width * scale) * dpi).ceil() / dpi
                                    + 1. / dpi),
                                px(
                                    ((f32::from(origin.y) + g.height * scale) * dpi).ceil() / dpi
                                        + 1. / dpi,
                                ),
                            ),
                        );
                        let inset = entry.image_inset;
                        let image_bounds = Bounds::new(
                            origin - point(px(inset.0 * scale), px(inset.1 * scale)),
                            size(
                                px((g.width + inset.0 * 2.) * scale),
                                px((g.height + inset.1 * 2.) * scale),
                            ),
                        );
                        w.paint_image(
                            target,
                            image_bounds,
                            Corners::all(px(0.)),
                            image.clone(),
                            0,
                            false,
                        )
                        .map_err(|e| format!("image upload: {e}"))?;
                        if g.reason.is_some() {
                            self.stats.fallbacks += 1;
                        } else {
                            self.stats.images += 1;
                        }
                    }
                    Ok::<_, String>(())
                })?;
            }
            Ok::<_, String>(())
        })?;
        self.stats.paint_micros += began.elapsed().as_micros() as u64;
        Ok(())
    }
}
impl Editor {
    pub(super) fn request_scene(&mut self, cx: &mut Context<Self>) {
        let geometry_generation = if self.text_edit.is_some()
            || self.table_resize.is_some()
            || self
                .gesture
                .as_ref()
                .is_some_and(|g| g.mode != Some(Mode::Move))
        {
            self.visual_revision
        } else {
            0
        };
        let key = (
            self.session.id,
            self.session.revision,
            self.page,
            geometry_generation,
        );
        if self.scene_busy {
            if self.scene_key != Some(key) {
                self.scene_cancel
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
            return;
        }
        if self.scene_key == Some(key) {
            return;
        }
        self.scene_key = Some(key);
        self.scene_busy = true;
        self.scene_cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.scene_cancel.clone();
        let completed_cancel = cancel.clone();
        let mut deck = (*self.display_deck()).clone();
        self.suppress_active_text(&mut deck);
        let cache = self.scene_cache.clone();
        self.scene_task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    cache
                        .lock()
                        .map_err(|_| "scene cache lock".to_string())?
                        .build(&deck, key.2, key.0, key.1, &cancel)
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.sync_document(cx);
                s.scene_busy = false;
                let current_geometry = if s.text_edit.is_some()
                    || s.table_resize.is_some()
                    || s.gesture
                        .as_ref()
                        .is_some_and(|g| g.mode != Some(Mode::Move))
                {
                    s.visual_revision
                } else {
                    0
                };
                if completed_cancel.load(std::sync::atomic::Ordering::Relaxed)
                    || (s.session.id, s.session.revision, s.page, current_geometry) != key
                {
                    s.scene_key = None;
                    cx.notify();
                    return;
                }
                match result {
                    Ok(scene) => s.scene = Some(scene),
                    Err(e) => {
                        s.accelerated = false;
                        s.pending = None;
                        s.image = None;
                        s.message = format!("场景不可用，已切回CPU：{e}");
                    }
                }
                cx.notify();
            });
        }));
    }
    pub(super) fn paint_scene(
        &mut self,
        bounds: Bounds<Pixels>,
        scale: f32,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.gpu_cache.release_images(w);
        if !self.accelerated {
            return;
        }
        let Some(scene) = &self.scene else {
            return;
        };
        if scene.document != self.session.id
            || scene.slide != self.session.deck.slides[self.page].id
        {
            return;
        }
        let objects = self.display_objects();
        let skip = self.text_edit.as_ref().map(|t| t.object.id.as_str());
        if let Err(e) = self
            .gpu_cache
            .paint(scene, &objects, skip, bounds, scale, w)
        {
            self.accelerated = false;
            self.pending = None;
            self.image = None;
            self.message = format!("绘制失败，已切回CPU：{e}");
            cx.notify();
        }
    }
    pub(super) fn toggle_renderer(&mut self, cx: &mut Context<Self>) {
        self.finish_text(cx);
        self.accelerated = !self.accelerated;
        self.pending = None;
        self.image = None;
        self.scene_key = None;
        cx.notify();
    }
}
pub(super) fn new_cache() -> Arc<Mutex<scene::SceneCache>> {
    Arc::new(Mutex::new(scene::SceneCache::default()))
}

impl Drop for Editor {
    fn drop(&mut self) {
        self.scene_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(feature = "validation")]
#[path = "scene_canvas_validation.rs"]
mod validation;
#[cfg(feature = "validation")]
pub(super) use validation::smoke;
