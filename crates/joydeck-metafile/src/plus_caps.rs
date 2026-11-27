//! Custom EMF+ line cap records, evaluated at the endpoints in pen space.

use kurbo::{Affine, BezPath, Point, Shape, Stroke, StrokeOpts, Vec2};

use crate::{
    bytes::Bytes,
    plus_pen::{TOLERANCE, bez, cap, join},
    plus_shapes::{floats, path},
};

const MAX_CAP_POINTS: usize = 512;

pub(crate) enum CustomCap {
    Outline(PathCap),
    Arrow(Arrow),
}

pub(crate) struct PathCap {
    base: u32,
    inset: f64,
    scale: f64,
    fill: Option<BezPath>,
    stroke: Option<(BezPath, Stroke)>,
}

pub(crate) struct Arrow {
    width: f64,
    height: f64,
    inset: f64,
    filled: bool,
    scale: f64,
}

impl CustomCap {
    pub fn read(bytes: Bytes<'_>) -> Option<Self> {
        match bytes.u32(4)? {
            0 => {
                let flags = bytes.u32(8)?;
                if flags == 0 || flags & !3 != 0 {
                    return None;
                }
                let [inset] = floats(bytes, 16)?;
                let [limit, scale] = floats(bytes, 32)?;
                let mut at = 56;
                let fill = if flags & 1 != 0 {
                    Some(read_path(bytes, &mut at)?)
                } else {
                    None
                };
                let stroke = if flags & 2 != 0 {
                    let path = read_path(bytes, &mut at)?;
                    Some((
                        path,
                        Stroke::new(1.0)
                            .with_start_cap(cap(bytes.u32(20)?))
                            .with_end_cap(cap(bytes.u32(24)?))
                            .with_join(join(bytes.u32(28)?))
                            .with_miter_limit(limit.clamp(1.0, 100.0)),
                    ))
                } else {
                    None
                };
                Some(Self::Outline(PathCap {
                    base: bytes.u32(12)?,
                    inset: inset.clamp(0.0, 100.0),
                    scale: scale.clamp(0.0, 100.0),
                    fill,
                    stroke,
                }))
            }
            1 => {
                let [width, height, inset] = floats(bytes, 8)?;
                let [scale] = floats(bytes, 40)?;
                Some(Self::Arrow(Arrow {
                    width: width.clamp(0.0, 100.0),
                    height: height.clamp(0.0, 100.0),
                    inset: inset.clamp(0.0, height.max(0.0)),
                    filled: bytes.u32(20)? != 0,
                    scale: scale.clamp(0.0, 100.0),
                }))
            }
            _ => None,
        }
    }

    pub fn base(&self) -> u32 {
        match self {
            Self::Outline(path) => path.base,
            Self::Arrow(_) => 0,
        }
    }

    pub fn inset(&self, pen_width: f64) -> f64 {
        match self {
            Self::Outline(path) => path.inset * path.scale * pen_width,
            Self::Arrow(_) => 0.0,
        }
    }

    pub fn paint(&self, anchor: Point, outward: Vec2, width: f64) -> BezPath {
        match self {
            Self::Outline(custom) => {
                let width = width * custom.scale;
                let normal = Vec2::new(-outward.y, outward.x);
                let transform = Affine::new([
                    normal.x * width,
                    normal.y * width,
                    outward.x * width,
                    outward.y * width,
                    anchor.x,
                    anchor.y,
                ]);
                let mut outline = BezPath::new();
                if let Some(fill) = &custom.fill {
                    outline.extend(transform * fill);
                }
                if let Some((line, style)) = &custom.stroke {
                    let stroke = transform
                        * kurbo::stroke(line.iter(), style, &StrokeOpts::default(), TOLERANCE);
                    extend(&mut outline, stroke);
                }
                outline
            }
            Self::Arrow(arrow) => {
                let normal = Vec2::new(-outward.y, outward.x);
                let local =
                    |x: f64, y: f64| anchor + (outward * x + normal * y) * (width * arrow.scale);
                let mut shape = BezPath::new();
                shape.move_to(anchor);
                shape.line_to(local(-arrow.height, -arrow.width / 2.0));
                shape.line_to(local(arrow.inset - arrow.height, 0.0));
                shape.line_to(local(-arrow.height, arrow.width / 2.0));
                shape.close_path();
                if arrow.filled {
                    shape
                } else {
                    kurbo::stroke(
                        shape.iter(),
                        &Stroke::new(width),
                        &StrokeOpts::default(),
                        TOLERANCE,
                    )
                }
            }
        }
    }
}

fn read_path(bytes: Bytes<'_>, offset: &mut usize) -> Option<BezPath> {
    let length = usize::try_from(bytes.i32(*offset)?).ok()?;
    let path_bytes = Bytes(bytes.slice(offset.checked_add(4)?, length)?);
    *offset += 4 + length;
    if path_bytes.u32(4)? as usize > MAX_CAP_POINTS {
        return None;
    }
    let result = bez(&path(path_bytes)?);
    let bounds = result.control_box();
    (bounds.is_finite()
        && [bounds.x0, bounds.y0, bounds.x1, bounds.y1]
            .iter()
            .all(|value| value.abs() <= 1000.0))
    .then_some(result)
}

/// Align fill orientation before joining independently authored decorations to a stroke.
pub fn extend(outline: &mut BezPath, addition: BezPath) {
    if outline.area().is_sign_positive() != addition.area().is_sign_positive()
        && !outline.is_empty()
    {
        outline.extend(addition.reverse_subpaths());
    } else {
        outline.extend(addition);
    }
}
