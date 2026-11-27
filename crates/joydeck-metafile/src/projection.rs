//! Projection of metafile items into a bounded, ordered scene.

use std::sync::Arc;

use kurbo::{Affine, BezPath, Rect, Shape};

use crate::{ClipRegion, ImagePlacement, Item, Picture, Seg};

pub const MAX_BITMAP_SIDE: u32 = 30_000;
const COORDINATE_LIMIT: f64 = 1e6;
const ELEMENT_LIMIT: usize = 500_000;

pub type Masks = Vec<Arc<BezPath>>;

/// One drawing operation, in the target coordinate space.
pub enum PlacedItem {
    Path {
        path: BezPath,
        fill: Option<[u8; 4]>,
        stroke: Option<([u8; 4], f64)>,
        even_odd: bool,
        region: Masks,
    },
    Bitmap {
        index: usize,
        rect: Rect,
        region: Masks,
    },
    AffineBitmap {
        index: usize,
        placement: ImagePlacement,
        region: Masks,
    },
    Transfer {
        index: usize,
        transform: Affine,
        region: Masks,
    },
    Pattern {
        index: usize,
        path: BezPath,
        even_odd: bool,
        tile: Affine,
        region: Masks,
    },
    Text {
        index: usize,
        transform: Affine,
        clip: Option<Rect>,
        region: Masks,
    },
    Nested {
        index: usize,
        placement: ImagePlacement,
        region: Masks,
    },
}

impl Picture {
    pub fn has_size(&self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width > 0.0 && self.height > 0.0
    }

    /// Fit the cropped part of the picture into a target rectangle.
    pub fn fit(&self, target: Rect, crop: [f32; 4]) -> Option<Affine> {
        if !self.has_size() {
            return None;
        }
        let [left, top, right, bottom] = crop.map(|value| {
            if value.is_finite() {
                f64::from(value.clamp(0.0, 0.95))
            } else {
                0.0
            }
        });
        let (w, h) = (f64::from(self.width), f64::from(self.height));
        let sx = target.width() / (w * (1.0 - left - right).max(0.05));
        let sy = target.height() / (h * (1.0 - top - bottom).max(0.05));
        (sx.is_finite() && sy.is_finite() && sx > 0.0 && sy > 0.0).then(|| {
            Affine::new([
                sx,
                0.0,
                0.0,
                sy,
                target.x0 - left * w * sx,
                target.y0 - top * h * sy,
            ])
        })
    }

    pub fn place(&self, target: Rect, crop: [f32; 4]) -> Vec<PlacedItem> {
        self.fit(target, crop)
            .map_or_else(Vec::new, |transform| self.project(transform))
    }

    /// Project a nested picture into the caller's coordinates, including rotated or mirrored axes.
    pub fn project(&self, transform: Affine) -> Vec<PlacedItem> {
        if !transform.is_finite() || transform.determinant() == 0.0 {
            return Vec::new();
        }
        let [a, b, c, d, ..] = transform.as_coeffs();
        let mut projector = Projector {
            transform,
            stroke_scale: (a.hypot(b) + c.hypot(d)) * 0.5,
            elements: ELEMENT_LIMIT,
            current_mask: None,
        };
        let mut output = Vec::new();
        for (index, item) in self.items.iter().enumerate() {
            if projector.elements == 0 {
                break;
            }
            if let Some(op) = projector.item(index, item) {
                output.push(op);
            }
        }
        output
    }
}

struct Projector {
    transform: Affine,
    stroke_scale: f64,
    elements: usize,
    current_mask: Option<(*const BezPath, Arc<BezPath>)>,
}

impl Projector {
    fn item(&mut self, index: usize, item: &Item) -> Option<PlacedItem> {
        self.charge(1)?;
        let transform = self.transform;
        Some(match item {
            Item::Path {
                segs,
                fill,
                stroke,
                even_odd,
                region,
            } => {
                let path = self.path(segs)?;
                let region = self.masks(region.as_ref())?;
                PlacedItem::Path {
                    path,
                    fill: *fill,
                    stroke: stroke.map(|(color, w)| (color, f64::from(w) * self.stroke_scale)),
                    even_odd: *even_odd,
                    region,
                }
            }
            Item::Pattern {
                segs,
                even_odd,
                tile,
                region,
            } => {
                let path = self.path(segs)?;
                let region = self.masks(region.as_ref())?;
                let tile = transform
                    * Affine::translate((f64::from(tile.origin[0]), f64::from(tile.origin[1])))
                    * Affine::scale_non_uniform(f64::from(tile.cell[0]), f64::from(tile.cell[1]));
                let det = tile.determinant();
                if !det.is_finite() || det.abs() <= 1e-12 {
                    return None;
                }
                PlacedItem::Pattern {
                    index,
                    path,
                    even_odd: *even_odd,
                    tile,
                    region,
                }
            }
            Item::Text(text) => {
                let transform = transform * Affine::new(text.transform);
                if !transform
                    .as_coeffs()
                    .iter()
                    .all(|v| v.is_finite() && v.abs() <= COORDINATE_LIMIT)
                {
                    return None;
                }
                let mut region = self.masks(text.region.as_ref())?;
                let mut clip = None;
                if let Some([x0, y0, x1, y1]) = text.clip.map(|rect| rect.map(f64::from)) {
                    let source = Rect::new(x0, y0, x1, y1);
                    let bounds = self.transform.transform_rect_bbox(source);
                    if !visible(bounds) {
                        return None;
                    }
                    if crate::image_placement::axis_aligned(self.transform) {
                        clip = Some(bounds);
                    } else {
                        region.push(Arc::new(self.transform * source.to_path(0.1)));
                    }
                }
                PlacedItem::Text {
                    index,
                    transform,
                    clip,
                    region,
                }
            }
            Item::Bitmap {
                rect,
                width,
                height,
                region,
                ..
            } => {
                let [x, y, w, h] = rect.map(f64::from);
                if w <= 0.0 || h <= 0.0 || *width == 0 || *height == 0 {
                    return None;
                }
                let region = self.masks(region.as_ref())?;
                if crate::image_placement::axis_aligned(transform)
                    && transform.as_coeffs()[0] > 0.0
                    && transform.as_coeffs()[3] > 0.0
                {
                    let rect = transform.transform_rect_bbox(Rect::new(x, y, x + w, y + h));
                    if !visible(rect) {
                        return None;
                    }
                    PlacedItem::Bitmap {
                        index,
                        rect,
                        region,
                    }
                } else {
                    let source = Rect::new(0.0, 0.0, f64::from(*width), f64::from(*height));
                    let mapping = transform
                        * Affine::new([w / source.width(), 0.0, 0.0, h / source.height(), x, y]);
                    let placement = ImagePlacement {
                        transform: mapping,
                        source,
                    };
                    if !placement.valid() || !visible(placement.bounds()) {
                        return None;
                    }
                    PlacedItem::AffineBitmap {
                        index,
                        placement,
                        region,
                    }
                }
            }
            Item::AffineBitmap {
                placement, region, ..
            }
            | Item::Nested {
                placement, region, ..
            } => {
                let placement = placement.map(transform);
                if !placement.valid() || !visible(placement.bounds()) {
                    return None;
                }
                let region = self.masks(region.as_ref())?;
                if matches!(item, Item::Nested { .. }) {
                    PlacedItem::Nested {
                        index,
                        placement,
                        region,
                    }
                } else {
                    PlacedItem::AffineBitmap {
                        index,
                        placement,
                        region,
                    }
                }
            }
            Item::Transfer { transfer, region } => {
                let placement = transfer.placement.map(transform);
                if !placement.valid() || !visible(placement.bounds()) {
                    return None;
                }
                PlacedItem::Transfer {
                    index,
                    transform,
                    region: self.masks(region.as_ref())?,
                }
            }
        })
    }

    fn charge(&mut self, amount: usize) -> Option<()> {
        if amount > self.elements {
            self.elements = 0;
            return None;
        }
        self.elements -= amount;
        Some(())
    }

    fn path(&mut self, segments: &[Seg]) -> Option<BezPath> {
        self.charge(segments.len())?;
        let transform = self.transform;
        let point = |x, y| transform * kurbo::Point::new(f64::from(x), f64::from(y));
        let mut path = BezPath::new();
        let mut start = (0.0, 0.0);
        let mut active = false;
        for &segment in segments {
            if !active && matches!(segment, Seg::Line(..) | Seg::Cubic(..)) {
                path.move_to(point(start.0, start.1));
                active = true;
            }
            match segment {
                Seg::Move(x, y) => {
                    path.move_to(point(x, y));
                    start = (x, y);
                    active = true;
                }
                Seg::Line(x, y) => path.line_to(point(x, y)),
                Seg::Cubic(x1, y1, x2, y2, x, y) => {
                    path.curve_to(point(x1, y1), point(x2, y2), point(x, y))
                }
                Seg::Close => {
                    path.close_path();
                    active = false;
                }
            }
        }
        in_range(path.bounding_box()).then_some(path)
    }

    fn masks(&mut self, region: Option<&ClipRegion<f32>>) -> Option<Masks> {
        let mut masks = Vec::new();
        if let Some(region) = region {
            self.append_masks(region, &mut masks, 0)?;
        }
        Some(masks)
    }

    fn append_masks(
        &mut self,
        region: &ClipRegion<f32>,
        masks: &mut Masks,
        depth: usize,
    ) -> Option<()> {
        if depth > 8 {
            return None;
        }
        let path = match region {
            ClipRegion::Rectangles(rects) => {
                self.charge(rects.len().saturating_mul(5))?;
                let mut outline = BezPath::new();
                for &[x0, y0, x1, y1] in rects {
                    let rect =
                        Rect::new(f64::from(x0), f64::from(y0), f64::from(x1), f64::from(y1));
                    if visible(self.transform.transform_rect_bbox(rect)) {
                        outline.extend((self.transform * rect.to_path(0.1)).iter());
                    }
                }
                Arc::new(outline)
            }
            ClipRegion::Outline(outline) => {
                if let Some((key, old)) = &self.current_mask
                    && *key == Arc::as_ptr(outline)
                {
                    masks.push(old.clone());
                    return Some(());
                }
                self.charge(outline.elements().len())?;
                let path = Arc::new(self.transform * outline.as_ref());
                if !path.is_empty() && in_range(path.bounding_box()) {
                    self.current_mask = Some((Arc::as_ptr(outline), path.clone()));
                }
                path
            }
            ClipRegion::Intersection(parts) => {
                self.append_masks(&parts.0, masks, depth + 1)?;
                self.append_masks(&parts.1, masks, depth + 1)?;
                return Some(());
            }
        };
        if path.is_empty() || !in_range(path.bounding_box()) {
            return None;
        }
        masks.push(path);
        Some(())
    }
}

impl Item {
    /// Borrow the bitmap plane if its dimensions and storage match the renderer limits.
    pub fn pixels(&self) -> Option<(u32, u32, &[u8])> {
        let (w, h, px) = match self {
            Self::Bitmap {
                width,
                height,
                rgba,
                ..
            }
            | Self::AffineBitmap {
                width,
                height,
                rgba,
                ..
            } => (*width, *height, rgba),
            _ => return None,
        };
        drawable(w, h, px).then_some((w, h, px))
    }

    /// Move the pixel buffer out of an independently owned bitmap item.
    pub fn take_pixels(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        let (w, h, px) = match self {
            Self::Bitmap {
                width,
                height,
                rgba,
                ..
            }
            | Self::AffineBitmap {
                width,
                height,
                rgba,
                ..
            } => (*width, *height, std::mem::take(rgba)),
            _ => return None,
        };
        drawable(w, h, &px).then_some((w, h, px))
    }
}

fn drawable(width: u32, height: u32, data: &[u8]) -> bool {
    (1..=MAX_BITMAP_SIDE).contains(&width)
        && (1..=MAX_BITMAP_SIDE).contains(&height)
        && data.len()
            == (width as usize)
                .saturating_mul(height as usize)
                .saturating_mul(4)
}

fn visible(rect: Rect) -> bool {
    in_range(rect) && rect.width() > 1e-6 && rect.height() > 1e-6
}
fn in_range(rect: Rect) -> bool {
    [rect.x0, rect.y0, rect.x1, rect.y1]
        .iter()
        .all(|x| x.is_finite() && x.abs() <= COORDINATE_LIMIT)
}
