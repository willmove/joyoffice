//! Replay a projected metafile scene with the same paint and clip stack at every nesting level.

use std::{collections::HashMap, sync::Arc};

use joydeck_metafile::{ImageComposition, Item, Picture, PlacedItem, RasterTransfer, Tile};
use kurbo::{Affine, BezPath, Point, Rect, Shape};
use vello_cpu::{Pixmap, RenderContext, Resources, peniko};

use super::metafile::{draw_text, rgba};

const MAX_RASTER_SIDE: u32 = 8192;
const MIN_PATTERN_PIXEL: f64 = 1.5;
const MAX_SCENE_WORK: usize = 1_000_000;
const MAX_TRANSFER_PIXELS: usize = 80_000_000;
const MAX_DESTINATION_READS: usize = 64;

pub(super) fn rasterize(picture: &Picture, side: u32) -> Option<image::RgbaImage> {
    if !picture.has_size() {
        return None;
    }
    let span = side.clamp(16, MAX_RASTER_SIDE);
    let (width, height) = (f64::from(picture.width), f64::from(picture.height));
    let scale = f64::from(span) / width.max(height);
    let w = (width * scale)
        .round()
        .clamp(1.0, f64::from(MAX_RASTER_SIDE)) as u16;
    let h = (height * scale)
        .round()
        .clamp(1.0, f64::from(MAX_RASTER_SIDE)) as u16;
    let mut ctx = RenderContext::new_with(
        w,
        h,
        vello_cpu::RenderSettings {
            num_threads: 0,
            ..Default::default()
        },
    );
    let viewport = Rect::new(0.0, 0.0, f64::from(w), f64::from(h));
    let mut playback = Playback {
        tiles: HashMap::new(),
        remaining: MAX_SCENE_WORK,
        remaining_pixels: MAX_TRANSFER_PIXELS,
        remaining_reads: MAX_DESTINATION_READS,
        viewport,
        masks: Vec::new(),
    };
    playback.push_mask(&mut ctx, Arc::new(viewport.to_path(0.1)));
    playback.play(&mut ctx, picture, picture.fit(viewport, [0.0; 4])?, 0);
    ctx.set_fill_rule(peniko::Fill::NonZero);
    ctx.pop_layer();
    ctx.flush();

    let mut pixels = vec![0u8; usize::from(w) * usize::from(h) * 4];
    ctx.render(
        vello_cpu::PixmapMut::new(w, h, &mut pixels)?,
        &mut Resources::new(),
    );
    for pixel in pixels.as_chunks_mut::<4>().0 {
        *pixel = straight(*pixel);
    }
    image::RgbaImage::from_raw(u32::from(w), u32::from(h), pixels)
}

struct Playback {
    tiles: HashMap<*const Tile, PreparedTile>,
    remaining: usize,
    remaining_pixels: usize,
    remaining_reads: usize,
    viewport: Rect,
    masks: Vec<Arc<BezPath>>,
}

struct PreparedTile {
    pixels: Option<Arc<Pixmap>>,
    average: peniko::Color,
}

impl Playback {
    fn push_mask(&mut self, ctx: &mut RenderContext, mask: Arc<BezPath>) {
        ctx.set_fill_rule(peniko::Fill::NonZero);
        ctx.push_clip_layer(&mask);
        self.masks.push(mask);
    }

    fn pop_masks(&mut self, ctx: &mut RenderContext, count: usize) {
        for _ in 0..count {
            self.masks.pop();
            ctx.pop_layer();
        }
    }

    fn play(
        &mut self,
        ctx: &mut RenderContext,
        picture: &Picture,
        transform: Affine,
        depth: usize,
    ) {
        if depth > 3 || self.remaining == 0 {
            return;
        }
        for operation in picture.project(transform) {
            let (masks, cost) = match &operation {
                PlacedItem::Path { path, region, .. }
                | PlacedItem::Pattern { path, region, .. } => (region, path.elements().len() + 1),
                PlacedItem::Bitmap { region, .. }
                | PlacedItem::AffineBitmap { region, .. }
                | PlacedItem::Transfer { region, .. }
                | PlacedItem::Text { region, .. }
                | PlacedItem::Nested { region, .. } => (region, 1),
            };
            let cost = masks
                .iter()
                .fold(cost, |sum, mask| sum.saturating_add(mask.elements().len()));
            if cost > self.remaining {
                self.remaining = 0;
                break;
            }
            self.remaining -= cost;
            let mask_count = masks.len();
            for outline in masks {
                self.push_mask(ctx, outline.clone());
            }
            self.paint(ctx, &picture.items, operation, depth);
            self.pop_masks(ctx, mask_count);
        }
    }

    fn paint(
        &mut self,
        ctx: &mut RenderContext,
        items: &[Item],
        operation: PlacedItem,
        depth: usize,
    ) {
        match operation {
            PlacedItem::Path {
                path,
                fill,
                stroke,
                even_odd,
                ..
            } => {
                if let Some(color) = fill {
                    ctx.set_fill_rule(fill_rule(even_odd));
                    ctx.set_paint(rgba(color));
                    ctx.fill_path(&path);
                }
                if let Some((color, width)) = stroke {
                    ctx.set_paint(rgba(color));
                    ctx.set_stroke(
                        kurbo::Stroke::new(width.max(1.0))
                            .with_caps(kurbo::Cap::Round)
                            .with_join(kurbo::Join::Round),
                    );
                    ctx.stroke_path(&path);
                }
            }
            PlacedItem::Bitmap { index, rect, .. } => {
                let Some((w, h, pixels)) = items.get(index).and_then(Item::pixels) else {
                    return;
                };
                let Some(texture) = premul(w, h, pixels) else {
                    return;
                };
                let transform = Affine::translate((rect.x0, rect.y0))
                    * Affine::scale_non_uniform(
                        rect.width() / f64::from(w),
                        rect.height() / f64::from(h),
                    );
                paint_image(ctx, texture, photo_sampler(), transform, |ctx| {
                    ctx.fill_rect(&rect)
                });
            }
            PlacedItem::AffineBitmap {
                index, placement, ..
            } => {
                let Some((w, h, pixels)) = items.get(index).and_then(Item::pixels) else {
                    return;
                };
                let Some(texture) = premul(w, h, pixels) else {
                    return;
                };
                let outline = placement.transform * placement.source.to_path(0.1);
                ctx.set_fill_rule(peniko::Fill::NonZero);
                if matches!(
                    items.get(index),
                    Some(Item::AffineBitmap {
                        composition: ImageComposition::SourceCopy,
                        ..
                    })
                ) {
                    ctx.push_layer(
                        Some(&outline),
                        Some(peniko::BlendMode::new(
                            peniko::Mix::Normal,
                            peniko::Compose::Copy,
                        )),
                        None,
                        None,
                        None,
                    );
                    paint_image(ctx, texture, photo_sampler(), placement.transform, |ctx| {
                        ctx.fill_rect(&placement.bounds())
                    });
                    ctx.pop_layer();
                } else {
                    paint_image(ctx, texture, photo_sampler(), placement.transform, |ctx| {
                        ctx.fill_path(&outline)
                    });
                }
            }
            PlacedItem::Transfer {
                index, transform, ..
            } => {
                let Some(Item::Transfer { transfer, .. }) = items.get(index) else {
                    return;
                };
                self.transfer(ctx, transfer, transform);
            }
            PlacedItem::Pattern {
                index,
                path,
                even_odd,
                tile,
                ..
            } => {
                let Some(Item::Pattern { tile: source, .. }) = items.get(index) else {
                    return;
                };
                let paint = self
                    .tiles
                    .entry(Arc::as_ptr(source))
                    .or_insert_with(|| PreparedTile {
                        pixels: premul(source.width, source.height, &source.rgba),
                        average: average(&source.rgba),
                    });
                ctx.set_fill_rule(fill_rule(even_odd));
                if let Some(texture) = &paint.pixels
                    && (source.sampled || tile.determinant().abs().sqrt() >= MIN_PATTERN_PIXEL)
                {
                    let sampler = peniko::ImageSampler {
                        x_extend: peniko::Extend::Repeat,
                        y_extend: peniko::Extend::Repeat,
                        quality: peniko::ImageQuality::Low,
                        alpha: 1.0,
                    };
                    paint_image(ctx, texture.clone(), sampler, tile, |ctx| {
                        ctx.fill_path(&path)
                    });
                } else {
                    ctx.set_paint(paint.average);
                    ctx.fill_path(&path);
                }
            }
            PlacedItem::Text {
                index,
                transform,
                clip,
                ..
            } => {
                let Some(Item::Text(text)) = items.get(index) else {
                    return;
                };
                let clip = clip.map(|bounds| bounds.to_path(0.1));
                inside(ctx, clip.as_ref(), |ctx| draw_text(ctx, text, transform));
            }
            PlacedItem::Nested {
                index, placement, ..
            } => {
                let Some(Item::Nested { picture, .. }) = items.get(index) else {
                    return;
                };
                let clip = placement.transform * placement.source.to_path(0.1);
                self.push_mask(ctx, Arc::new(clip));
                self.play(ctx, picture, placement.transform, depth + 1);
                self.pop_masks(ctx, 1);
            }
        }
    }

    fn transfer(&mut self, ctx: &mut RenderContext, transfer: &RasterTransfer, transform: Affine) {
        let mut transfer = transfer.clone();
        transfer.map(transform);
        let bounds = transfer.placement.bounds();
        let rect = Rect::new(
            bounds.x0.floor(),
            bounds.y0.floor(),
            bounds.x1.ceil(),
            bounds.y1.ceil(),
        )
        .intersect(self.viewport);
        let (x, y) = (rect.x0 as usize, rect.y0 as usize);
        let (w, h) = (rect.width() as usize, rect.height() as usize);
        let reads = transfer.logic.uses_destination();
        // Vello crops the right and bottom of a snapshot at the supplied dimensions.
        let (prefix_w, prefix_h) = (x + w, y + h);
        let work = w * h + if reads { prefix_w * prefix_h } else { 0 };
        if w == 0 || h == 0 || work > self.remaining_pixels || (reads && self.remaining_reads == 0)
        {
            return;
        }
        self.remaining_pixels -= work;
        let backdrop = if reads {
            self.remaining_reads -= 1;
            self.read_destination(ctx, prefix_w as u16, prefix_h as u16)
        } else {
            Vec::new()
        };
        let backdrop = backdrop.as_chunks::<4>().0;
        let inverse = transfer.placement.transform.inverse();
        let mut patch = vec![0; w * h * 4];
        for (i, pixel) in patch.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (column, row) = (x + i % w, y + i / w);
            let center = Point::new(column as f64 + 0.5, row as f64 + 0.5);
            let destination = backdrop
                .get(row * prefix_w + column)
                .copied()
                .map_or([0; 4], straight);
            if let Some(output) = transfer.paint(inverse * center, center, destination) {
                *pixel = output;
            }
        }
        if let Some(texture) = premul(w as u32, h as u32, &patch) {
            ctx.set_fill_rule(peniko::Fill::NonZero);
            paint_image(
                ctx,
                texture,
                peniko::ImageSampler::default().with_quality(peniko::ImageQuality::Low),
                Affine::translate((rect.x0, rect.y0)),
                |ctx| ctx.fill_rect(&rect),
            );
        }
    }

    fn read_destination(&self, ctx: &mut RenderContext, w: u16, h: u16) -> Vec<u8> {
        let mut pixels = vec![0; usize::from(w) * usize::from(h) * 4];
        let Some(target) = vello_cpu::PixmapMut::new(w, h, &mut pixels) else {
            return Vec::new();
        };
        // Close clip layers to make the recorded prefix readable. Reopen the same clips for
        // subsequent operations, so both the prefix and the resulting write stay clipped.
        for _ in &self.masks {
            ctx.pop_layer();
        }
        ctx.flush();
        ctx.render(target, &mut Resources::new());
        for mask in &self.masks {
            ctx.set_fill_rule(peniko::Fill::NonZero);
            ctx.push_clip_layer(mask);
        }
        pixels
    }
}

fn straight([r, g, b, a]: [u8; 4]) -> [u8; 4] {
    if a == 0 || a == 255 {
        return [r, g, b, a];
    }
    let alpha = u32::from(a);
    let unpremul = |channel| ((u32::from(channel) * 255 + alpha / 2) / alpha).min(255) as u8;
    [unpremul(r), unpremul(g), unpremul(b), a]
}

fn photo_sampler() -> peniko::ImageSampler {
    peniko::ImageSampler::default().with_quality(peniko::ImageQuality::Medium)
}

fn fill_rule(even_odd: bool) -> peniko::Fill {
    if even_odd {
        peniko::Fill::EvenOdd
    } else {
        peniko::Fill::NonZero
    }
}

fn inside(ctx: &mut RenderContext, mask: Option<&BezPath>, paint: impl FnOnce(&mut RenderContext)) {
    if let Some(mask) = mask {
        ctx.set_fill_rule(peniko::Fill::NonZero);
        ctx.push_clip_layer(mask);
        paint(ctx);
        ctx.pop_layer();
    } else {
        paint(ctx);
    }
}

fn paint_image(
    ctx: &mut RenderContext,
    texture: Arc<Pixmap>,
    sampler: peniko::ImageSampler,
    transform: Affine,
    paint: impl FnOnce(&mut RenderContext),
) {
    ctx.set_paint(vello_cpu::Image {
        image: vello_cpu::ImageSource::Pixmap(texture),
        sampler,
    });
    ctx.set_paint_transform(transform);
    paint(ctx);
    ctx.reset_paint_transform();
}

/// The average colour of straight RGBA pixels (alpha-weighted).
fn average(px: &[u8]) -> peniko::Color {
    let (mut r, mut g, mut b, mut a, mut n) = (0u64, 0u64, 0u64, 0u64, 0u64);
    for p in px.as_chunks::<4>().0 {
        let w = u64::from(p[3]);
        r += u64::from(p[0]) * w;
        g += u64::from(p[1]) * w;
        b += u64::from(p[2]) * w;
        a += w;
        n += 1;
    }
    if a == 0 || n == 0 {
        return peniko::Color::from_rgba8(0, 0, 0, 0);
    }
    peniko::Color::from_rgba8((r / a) as u8, (g / a) as u8, (b / a) as u8, (a / n) as u8)
}

/// Straight RGBA rows to a premultiplied pixmap.
fn premul(w: u32, h: u32, px: &[u8]) -> Option<Arc<Pixmap>> {
    if w == 0
        || h == 0
        || w > u16::MAX as u32
        || h > u16::MAX as u32
        || px.len() < w as usize * h as usize * 4
    {
        return None;
    }
    let data = px
        .as_chunks::<4>()
        .0
        .iter()
        .take(w as usize * h as usize)
        .map(|p| {
            let a = p[3] as u16;
            let m = |c: u8| ((c as u16 * a + 127) / 255) as u8;
            vello_cpu::color::PremulRgba8 {
                r: m(p[0]),
                g: m(p[1]),
                b: m(p[2]),
                a: p[3],
            }
        })
        .collect();
    Some(Arc::new(Pixmap::from_parts(data, w as u16, h as u16)))
}
