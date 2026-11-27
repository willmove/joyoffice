//! Replay a projected metafile scene with the same paint and clip stack at every nesting level.

use std::{collections::HashMap, sync::Arc};

use joydeck_metafile::{
    ImageComposition, Item, Picture, PlacedItem, RasterTransfer, Rop2, Tile, VectorMix,
};
use kurbo::{Affine, BezPath, Point, Rect, Shape};
use vello_cpu::{Pixmap, RenderContext, Resources, peniko};

use super::metafile::{draw_text, rgba};

const MAX_RASTER_SIDE: u32 = 8192;
const MIN_PATTERN_PIXEL: f64 = 1.5;
const MAX_SCENE_WORK: usize = 1_000_000;
const MAX_TRANSFER_PIXELS: usize = 80_000_000;
const MAX_DESTINATION_READS: usize = 64;
const MAX_GLYPH_BRUSH_SAMPLES: usize = 4_000_000;

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
    let mut ctx = context(w, h);
    let viewport = Rect::new(0.0, 0.0, f64::from(w), f64::from(h));
    let mut playback = Playback {
        tiles: HashMap::new(),
        remaining: MAX_SCENE_WORK,
        remaining_pixels: MAX_TRANSFER_PIXELS,
        remaining_reads: MAX_DESTINATION_READS,
        remaining_brush_samples: MAX_GLYPH_BRUSH_SAMPLES,
        viewport,
        direct_clip: uses_destination(picture, 0),
    };
    playback.push_mask(&mut ctx, &viewport.to_path(0.1));
    playback.play(&mut ctx, picture, picture.fit(viewport, [0.0; 4])?, 0);
    playback.pop_mask(&mut ctx);

    let mut pixels = render_pixels(&mut ctx, w, h)?;
    for pixel in pixels.as_chunks_mut::<4>().0 {
        *pixel = straight(*pixel);
    }
    image::RgbaImage::from_raw(u32::from(w), u32::from(h), pixels)
}

fn context(w: u16, h: u16) -> RenderContext {
    RenderContext::new_with(
        w,
        h,
        vello_cpu::RenderSettings {
            num_threads: 0,
            ..Default::default()
        },
    )
}

fn render_pixels(ctx: &mut RenderContext, w: u16, h: u16) -> Option<Vec<u8>> {
    ctx.flush();

    let mut pixels = vec![0u8; usize::from(w) * usize::from(h) * 4];
    ctx.render(
        vello_cpu::PixmapMut::new(w, h, &mut pixels)?,
        &mut Resources::new(),
    );
    Some(pixels)
}

struct Playback {
    tiles: HashMap<*const Tile, PreparedTile>,
    remaining: usize,
    remaining_pixels: usize,
    remaining_reads: usize,
    remaining_brush_samples: usize,
    viewport: Rect,
    direct_clip: bool,
}

struct PreparedTile {
    pixels: Option<Arc<Pixmap>>,
    average: peniko::Color,
}

enum VectorPaint {
    Solid(peniko::Color),
    Tile(Arc<Pixmap>, Affine),
}

impl VectorPaint {
    fn draw(&self, ctx: &mut RenderContext, offset: Affine, draw: impl FnOnce(&mut RenderContext)) {
        match self {
            Self::Solid(color) => {
                ctx.set_paint(*color);
                draw(ctx);
            }
            Self::Tile(pixels, mapping) => {
                let sampler = peniko::ImageSampler {
                    x_extend: peniko::Extend::Repeat,
                    y_extend: peniko::Extend::Repeat,
                    quality: peniko::ImageQuality::Low,
                    alpha: 1.0,
                };
                paint_image(ctx, pixels.clone(), sampler, offset * *mapping, draw);
            }
        }
    }
}

impl Playback {
    fn push_mask(&self, ctx: &mut RenderContext, mask: &BezPath) {
        ctx.set_fill_rule(peniko::Fill::NonZero);
        if self.direct_clip {
            ctx.push_clip_path(mask);
        } else {
            ctx.push_clip_layer(mask);
        }
    }

    fn pop_mask(&self, ctx: &mut RenderContext) {
        if self.direct_clip {
            ctx.pop_clip_path();
        } else {
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
                self.push_mask(ctx, outline);
            }
            self.paint(ctx, &picture.items, operation, depth);
            for _ in 0..mask_count {
                self.pop_mask(ctx);
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
                mix,
                ..
            } => {
                let fill = fill.map(|color| VectorPaint::Solid(rgba(color)));
                self.vector(ctx, &path, even_odd, fill, stroke, mix);
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
                let sampler = if matches!(
                    items.get(index),
                    Some(Item::AffineBitmap { nearest: true, .. })
                ) {
                    photo_sampler().with_quality(peniko::ImageQuality::Low)
                } else {
                    photo_sampler()
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
                    source_copy(ctx, |ctx| {
                        paint_image(ctx, texture, sampler, placement.transform, |ctx| {
                            ctx.fill_path(&outline)
                        });
                    });
                } else {
                    paint_image(ctx, texture, sampler, placement.transform, |ctx| {
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
                stroke,
                mix,
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
                let paint = if let Some(texture) = &paint.pixels
                    && (mix != VectorMix::SourceOver
                        || source.sampled
                        || tile.determinant().abs().sqrt() >= MIN_PATTERN_PIXEL)
                {
                    VectorPaint::Tile(texture.clone(), tile)
                } else {
                    VectorPaint::Solid(paint.average)
                };
                self.vector(ctx, &path, even_odd, Some(paint), stroke, mix);
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
                if let Some(bounds) = clip {
                    self.push_mask(ctx, &bounds.to_path(0.1));
                }
                draw_text(
                    ctx,
                    text,
                    transform,
                    self.viewport,
                    &mut self.remaining_brush_samples,
                );
                if clip.is_some() {
                    self.pop_mask(ctx);
                }
            }
            PlacedItem::Nested {
                index, placement, ..
            } => {
                let Some(Item::Nested { picture, .. }) = items.get(index) else {
                    return;
                };
                let clip = placement.transform * placement.source.to_path(0.1);
                // Native GDI+ plays the child directly into its parent's DC. The child's
                // mix records apply where it draws. Its undrawn area keeps the backdrop.
                self.push_mask(ctx, &clip);
                self.play(ctx, picture, placement.transform, depth + 1);
                self.pop_mask(ctx);
            }
        }
    }

    fn stroke_outline(&mut self, path: &BezPath, style: &kurbo::Stroke) -> Option<BezPath> {
        if path.elements().len() > self.remaining.min(10_000) {
            return None;
        }
        let outline = kurbo::stroke(path.iter(), style, &kurbo::StrokeOpts::default(), 0.25);
        self.remaining = self.remaining.checked_sub(outline.elements().len())?;
        Some(outline)
    }

    fn vector(
        &mut self,
        ctx: &mut RenderContext,
        path: &BezPath,
        odd: bool,
        fill: Option<VectorPaint>,
        stroke: Option<([u8; 4], f64)>,
        mix: VectorMix,
    ) {
        let stroke = stroke.map(|(color, width)| {
            let style = kurbo::Stroke::new(width.max(1.0))
                .with_caps(kurbo::Cap::Round)
                .with_join(kurbo::Join::Round);
            (rgba(color), style)
        });
        if mix == VectorMix::SourceOver {
            if let Some(fill) = fill {
                ctx.set_fill_rule(fill_rule(odd));
                fill.draw(ctx, Affine::IDENTITY, |ctx| ctx.fill_path(path));
            }
            if let Some((color, style)) = stroke {
                ctx.set_paint(color);
                ctx.set_stroke(style);
                ctx.stroke_path(path);
            }
            return;
        }
        let stroke =
            stroke.and_then(|(color, style)| Some((color, self.stroke_outline(path, &style)?)));
        if let VectorMix::Raster(rop) = mix {
            self.vector_rop(ctx, path, odd, fill, stroke, rop);
            return;
        }
        if let Some(fill) = fill {
            ctx.set_fill_rule(fill_rule(odd));
            source_copy(ctx, |ctx| {
                fill.draw(ctx, Affine::IDENTITY, |ctx| ctx.fill_path(path))
            });
        }
        if let Some((color, outline)) = stroke {
            ctx.set_fill_rule(peniko::Fill::NonZero);
            source_copy(ctx, |ctx| {
                ctx.set_paint(color);
                ctx.fill_path(&outline);
            });
        }
    }

    fn vector_rop(
        &mut self,
        ctx: &mut RenderContext,
        path: &BezPath,
        odd: bool,
        fill: Option<VectorPaint>,
        stroke: Option<(peniko::Color, BezPath)>,
        rop: Rop2,
    ) {
        let path_bounds = path.bounding_box();
        let bounds = stroke.as_ref().map_or(path_bounds, |(_, outline)| {
            outline.bounding_box().union(path_bounds)
        });
        let Some(window) = RasterWindow::new(bounds, self.viewport) else {
            return;
        };
        let Some(prefix) = self.charge_transfer(window, rop.reads_destination(), 2) else {
            return;
        };
        let offset = Affine::translate((-window.rect.x0, -window.rect.y0));
        let mut layer = context(window.w as u16, window.h as u16);
        if let Some(fill) = fill {
            layer.set_fill_rule(fill_rule(odd));
            fill.draw(&mut layer, offset, |layer| {
                layer.fill_path(&(offset * path))
            });
        }
        // The pen owns the overlap. One GDI shape applies its mix once per pixel.
        if let Some((color, outline)) = stroke {
            layer.set_fill_rule(peniko::Fill::NonZero);
            layer.set_paint(color);
            layer.fill_path(&(offset * outline));
        }
        let Some(mut pixels) = render_pixels(&mut layer, window.w as u16, window.h as u16) else {
            return;
        };
        let backdrop = prefix.and_then(|(w, h)| render_pixels(ctx, w, h));
        for (index, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let [x, y] = window.point(index);
            let before = snapshot_pixel(backdrop.as_deref(), prefix, x, y).unwrap_or_default();
            *pixel = if pixel[3] == 0 {
                [0; 4]
            } else {
                rop.paint(straight(*pixel), before).unwrap_or_default()
            };
        }
        window.paint(ctx, &pixels);
    }

    fn transfer(&mut self, ctx: &mut RenderContext, transfer: &RasterTransfer, transform: Affine) {
        let mut transfer = transfer.clone();
        transfer.map(transform);
        let Some(window) = RasterWindow::new(transfer.placement.bounds(), self.viewport) else {
            return;
        };
        let Some(prefix) = self.charge_transfer(window, transfer.logic.uses_destination(), 1)
        else {
            return;
        };
        let backdrop = prefix.and_then(|(w, h)| render_pixels(ctx, w, h));
        let get = |x, y| snapshot_pixel(backdrop.as_deref(), prefix, x, y);
        let inverse = transfer.placement.transform.inverse();
        let mut patch = vec![0; window.w * window.h * 4];
        for (index, pixel) in patch.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let [column, row] = window.point(index);
            let center = Point::new(column as f64 + 0.5, row as f64 + 0.5);
            let destination = get(column, row).unwrap_or_default();
            if let Some(output) = transfer.paint(inverse * center, center, destination) {
                *pixel = output;
            }
        }
        window.paint(ctx, &patch);
    }

    fn charge_transfer(
        &mut self,
        window: RasterWindow,
        read: bool,
        passes: usize,
    ) -> Option<Option<(u16, u16)>> {
        // Vello crops the right and bottom of a snapshot at the supplied dimensions.
        let prefix = read.then_some((window.rect.x1 as u16, window.rect.y1 as u16));
        let work = window.w * window.h * passes
            + prefix.map_or(0, |(w, h)| usize::from(w) * usize::from(h));
        if work > self.remaining_pixels || (prefix.is_some() && self.remaining_reads == 0) {
            return None;
        }
        self.remaining_pixels -= work;
        self.remaining_reads -= usize::from(prefix.is_some());
        Some(prefix)
    }
}

#[derive(Clone, Copy)]
struct RasterWindow {
    rect: Rect,
    w: usize,
    h: usize,
}

impl RasterWindow {
    fn new(bounds: Rect, viewport: Rect) -> Option<Self> {
        if !bounds.is_finite() {
            return None;
        }
        let rect = Rect::new(
            bounds.x0.floor(),
            bounds.y0.floor(),
            bounds.x1.ceil(),
            bounds.y1.ceil(),
        )
        .intersect(viewport);
        let (w, h) = (rect.width() as usize, rect.height() as usize);
        (w != 0 && h != 0).then_some(Self { rect, w, h })
    }

    fn point(self, index: usize) -> [usize; 2] {
        [
            self.rect.x0 as usize + index % self.w,
            self.rect.y0 as usize + index / self.w,
        ]
    }

    fn paint(self, ctx: &mut RenderContext, pixels: &[u8]) {
        if let Some(texture) = premul(self.w as u32, self.h as u32, pixels) {
            ctx.set_fill_rule(peniko::Fill::NonZero);
            paint_image(
                ctx,
                texture,
                peniko::ImageSampler::default().with_quality(peniko::ImageQuality::Low),
                Affine::translate((self.rect.x0, self.rect.y0)),
                |ctx| ctx.fill_rect(&self.rect),
            );
        }
    }
}

fn snapshot_pixel(
    pixels: Option<&[u8]>,
    size: Option<(u16, u16)>,
    x: usize,
    y: usize,
) -> Option<[u8; 4]> {
    let (w, h) = size?;
    let w = usize::from(w);
    if x >= w || y >= usize::from(h) {
        return None;
    }
    pixels?
        .as_chunks::<4>()
        .0
        .get(y * w + x)
        .copied()
        .map(straight)
}

pub(super) fn source_copy(ctx: &mut RenderContext, fill: impl FnOnce(&mut RenderContext)) {
    ctx.set_blend_mode(peniko::BlendMode::new(
        peniko::Mix::Normal,
        peniko::Compose::Copy,
    ));
    fill(ctx);
    ctx.set_blend_mode(peniko::BlendMode::default());
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

/// Destination operations need the real backdrop even inside child clips. Pictures that only
/// paint source-over keep grouped edge coverage for ordinary clip stacks.
fn uses_destination(picture: &Picture, depth: u8) -> bool {
    depth <= 3
        && picture.items.iter().any(|item| match item {
            Item::Path { mix, .. } | Item::Pattern { mix, .. } => *mix != VectorMix::SourceOver,
            Item::Text(text) => text.composition == ImageComposition::SourceCopy,
            Item::AffineBitmap { composition, .. } => *composition == ImageComposition::SourceCopy,
            Item::Transfer { transfer, .. } => transfer.logic.uses_destination(),
            Item::Nested { picture, .. } => uses_destination(picture, depth + 1),
            Item::Bitmap { .. } => false,
        })
}

pub(super) fn paint_image(
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
pub(super) fn premul(w: u32, h: u32, px: &[u8]) -> Option<Arc<Pixmap>> {
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
