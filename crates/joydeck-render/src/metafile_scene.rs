//! Replay a projected metafile scene with the same paint and clip stack at every nesting level.

use std::{collections::HashMap, sync::Arc};

use joydeck_metafile::{Item, Picture, PlacedItem, Tile};
use kurbo::{Affine, BezPath, Rect, Shape};
use vello_cpu::{Pixmap, RenderContext, Resources, peniko};

use super::metafile::{draw_text, rgba};

const MAX_RASTER_SIDE: u32 = 8192;
const MIN_PATTERN_PIXEL: f64 = 1.5;
const MAX_SCENE_WORK: usize = 1_000_000;

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
    ctx.push_clip_layer(&viewport.to_path(0.1));
    let mut playback = Playback {
        tiles: HashMap::new(),
        remaining: MAX_SCENE_WORK,
    };
    playback.play(&mut ctx, picture, picture.fit(viewport, [0.0; 4])?, 0);
    ctx.set_fill_rule(peniko::Fill::NonZero);
    ctx.pop_layer();
    ctx.flush();

    let mut pixels = vec![0u8; usize::from(w) * usize::from(h) * 4];
    ctx.render(
        vello_cpu::PixmapMut::new(w, h, &mut pixels)?,
        &mut Resources::new(),
    );
    for [r, g, b, alpha] in pixels.as_chunks_mut::<4>().0 {
        let a = u32::from(*alpha);
        if (1..255).contains(&a) {
            for channel in [r, g, b] {
                *channel = ((u32::from(*channel) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    image::RgbaImage::from_raw(u32::from(w), u32::from(h), pixels)
}

struct Playback {
    tiles: HashMap<*const Tile, PreparedTile>,
    remaining: usize,
}

struct PreparedTile {
    pixels: Option<Arc<Pixmap>>,
    average: peniko::Color,
}

impl Playback {
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
                ctx.set_fill_rule(peniko::Fill::NonZero);
                ctx.push_clip_layer(outline);
            }
            self.paint(ctx, &picture.items, operation, depth);
            for _ in 0..mask_count {
                ctx.pop_layer();
            }
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
                paint_image(ctx, texture, photo_sampler(), placement.transform, |ctx| {
                    ctx.fill_path(&outline)
                });
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
                inside(ctx, Some(&clip), |ctx| {
                    self.play(ctx, picture, placement.transform, depth + 1)
                });
            }
        }
    }
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
