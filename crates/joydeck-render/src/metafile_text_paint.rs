//! Apply an EMF+ brush to a complete glyph run, including its transparent coverage.

use joydeck_metafile::{ImageComposition, Text};
use kurbo::{Affine, BezPath, Point, Rect, Shape};
use vello_cpu::{RenderContext, peniko};

use crate::{
    metafile::rgba,
    metafile_scene::{paint_image, premul, source_copy},
};

const MAX_OUTLINE_ELEMENTS: usize = 250_000;
const MAX_SAMPLE_SIDE: f64 = 256.0;

pub(super) struct GlyphPaint {
    outline: Option<BezPath>,
}

impl GlyphPaint {
    pub fn new(text: &Text) -> Self {
        let group = text.brush.is_some() || text.composition == ImageComposition::SourceCopy;
        Self {
            outline: group.then(BezPath::new),
        }
    }

    pub fn path(&mut self, ctx: &mut RenderContext, path: &BezPath) {
        if let Some(outline) = &mut self.outline {
            if outline.elements().len() + path.elements().len() <= MAX_OUTLINE_ELEMENTS {
                outline.extend(path.iter());
            }
        } else {
            ctx.fill_path(path);
        }
    }

    pub fn stroke(&mut self, ctx: &mut RenderContext, path: &BezPath, stroke: kurbo::Stroke) {
        if self.outline.is_some() {
            let outline = kurbo::stroke(path.iter(), &stroke, &kurbo::StrokeOpts::default(), 0.25);
            self.path(ctx, &outline);
        } else {
            ctx.set_stroke(stroke);
            ctx.stroke_path(path);
        }
    }

    pub fn finish(
        self,
        ctx: &mut RenderContext,
        text: &Text,
        transform: Affine,
        viewport: Rect,
        budget: &mut usize,
    ) {
        let Some(path) = self.outline.filter(|path| !path.elements().is_empty()) else {
            return;
        };
        let mut draw = |ctx: &mut RenderContext| {
            if let Some(brush) = &text.brush {
                // The glyph layout transform can vary even within a single text record.
                // The shader is anchored to the picture rather than to a glyph.
                let picture = transform * Affine::new(text.transform).inverse();
                if picture.is_finite() && picture.determinant() != 0.0 {
                    if let Some(tile) = brush.texture()
                        && draw_texture(ctx, &path, picture, tile, budget).is_some()
                    {
                        return;
                    } else if let Some((tile, mapping)) = brush.hatch() {
                        if let Some(texture) = premul(8, 8, tile) {
                            let sampler = peniko::ImageSampler {
                                x_extend: peniko::Extend::Repeat,
                                y_extend: peniko::Extend::Repeat,
                                quality: peniko::ImageQuality::Low,
                                alpha: 1.0,
                            };
                            paint_image(ctx, texture, sampler, picture * mapping, |ctx| {
                                ctx.fill_path(&path)
                            });
                            return;
                        }
                    } else if sample_brush(ctx, &path, brush, picture, viewport, budget).is_some() {
                        return;
                    }
                }
            }
            ctx.set_paint(rgba(text.color));
            ctx.fill_path(&path);
        };
        ctx.set_fill_rule(peniko::Fill::NonZero);
        if text.composition == ImageComposition::SourceCopy {
            source_copy(ctx, draw);
        } else {
            draw(ctx);
        }
    }
}

fn draw_texture(
    ctx: &mut RenderContext,
    path: &BezPath,
    picture: Affine,
    tile: joydeck_metafile::TextTexture<'_>,
    budget: &mut usize,
) -> Option<()> {
    let (mut w, mut h) = (tile.width, tile.height);
    let mut mapping = tile.texture_to_picture;
    let extend = |mirrored| {
        if mirrored {
            peniko::Extend::Reflect
        } else {
            peniko::Extend::Repeat
        }
    };
    let mut sampler = peniko::ImageSampler {
        x_extend: extend(tile.x_mirrored),
        y_extend: extend(tile.y_mirrored),
        quality: peniko::ImageQuality::Medium,
        alpha: 1.0,
    };
    let border;
    let bytes = if tile.clamp {
        let row = w as usize * 4;
        w += 2;
        h += 2;
        *budget = budget.checked_sub(w as usize * h as usize)?;
        let stride = w as usize * 4;
        // A transparent border makes Pad sample transparency outside the brush image.
        let mut rgba = vec![0; stride * h as usize];
        for (dest, source) in rgba
            .chunks_exact_mut(stride)
            .skip(1)
            .zip(tile.rgba.chunks_exact(row))
        {
            dest[4..4 + row].copy_from_slice(source);
        }
        border = rgba;
        mapping *= Affine::translate((-1.0, -1.0));
        sampler.x_extend = peniko::Extend::Pad;
        sampler.y_extend = peniko::Extend::Pad;
        &border
    } else {
        *budget = budget.checked_sub(w as usize * h as usize)?;
        tile.rgba
    };
    let texture = premul(w, h, bytes)?;
    paint_image(ctx, texture, sampler, picture * mapping, |ctx| {
        ctx.fill_path(path)
    });
    Some(())
}

fn sample_brush(
    ctx: &mut RenderContext,
    path: &BezPath,
    brush: &joydeck_metafile::TextBrush,
    picture: Affine,
    viewport: Rect,
    budget: &mut usize,
) -> Option<()> {
    let rect = path.bounding_box().intersect(viewport);
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return Some(());
    }
    let step = (rect.width().max(rect.height()) / MAX_SAMPLE_SIDE).max(1.0);
    let w = (rect.width() / step).ceil() as u32;
    let h = (rect.height() / step).ceil() as u32;
    let size = w as usize * h as usize;
    *budget = budget.checked_sub(size)?;
    let pixels_to_canvas = Affine::translate((rect.x0, rect.y0)) * Affine::scale(step);
    let sample = brush.picture_to_brush() * picture.inverse() * pixels_to_canvas;
    if !sample.is_finite() {
        return None;
    }
    let mut pixels = Vec::with_capacity(size * 4);
    for y in 0..h {
        for x in 0..w {
            pixels.extend_from_slice(
                &brush.sample(sample * Point::new(f64::from(x) + 0.5, f64::from(y) + 0.5)),
            );
        }
    }
    let texture = premul(w, h, &pixels)?;
    let sampler = peniko::ImageSampler::default().with_quality(peniko::ImageQuality::Medium);
    paint_image(ctx, texture, sampler, pixels_to_canvas, |ctx| {
        ctx.fill_path(path)
    });
    Some(())
}
