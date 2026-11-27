//! EMF+ region trees and clipping operations for the convex canvas.

use crate::{
    bytes::Bytes,
    cliprect::{clip_polygon, convex, flatten, ops_bbox, pts_bbox, rect_poly},
    device::Xform,
    geom::{Op, P},
    plus_shapes::{MAX_POINTS, floats, path},
};

/// Infinite is `None`. A present zero-vertex polygon is empty.
pub type Area = Option<Vec<P>>;

pub fn parse(input: Bytes<'_>) -> Option<Area> {
    let mut scan = Region {
        bytes: input,
        offset: 8,
        work: MAX_POINTS,
    };
    scan.node(0)
}

struct Region<'a> {
    bytes: Bytes<'a>,
    offset: usize,
    work: usize,
}

impl Region<'_> {
    fn next(&mut self) -> Option<u32> {
        let result = self.bytes.u32(self.offset)?;
        self.offset += 4;
        Some(result)
    }

    fn node(&mut self, depth: usize) -> Option<Area> {
        if depth > 32 {
            return None;
        }
        self.work = self.work.checked_sub(1)?;
        let kind = self.next()?;
        match kind {
            0x1000_0000 => {
                let [x, y, w, h] = floats::<4>(self.bytes, self.offset)?;
                self.offset += 16;
                Some(Some(rect_poly([
                    x.min(x + w),
                    y.min(y + h),
                    x.max(x + w),
                    y.max(y + h),
                ])))
            }
            0x1000_0001 => {
                let length = self.next()? as usize;
                let path_bytes = Bytes(self.bytes.slice(self.offset, length)?);
                self.offset += length;
                self.work = self
                    .work
                    .checked_sub((path_bytes.u32(4)? as usize).min(MAX_POINTS))?;
                Some(Some(outline(&path(path_bytes)?)))
            }
            0x1000_0002 => Some(Some(Vec::new())),
            0x1000_0003 => Some(None),
            mode @ 0..=5 => {
                let left = self.node(depth + 1)?;
                let right = self.node(depth + 1)?;
                Some(combine(left, right, mode))
            }
            _ => None,
        }
    }
}

pub fn outline(path: &[Op]) -> Vec<P> {
    let figures = flatten(path);
    if let [(points, _)] = figures.as_slice()
        && let Some(ring) = convex(points)
    {
        return ring;
    }
    ops_bbox(path).map_or_else(Vec::new, rect_poly)
}

pub fn combine(current: Area, next: Area, mode: u32) -> Area {
    match mode {
        0 => next,
        1 => match (current, next) {
            (None, finite) | (finite, None) => finite,
            (Some(a), Some(b)) if a.len() >= 3 && b.len() >= 3 => {
                Some(convex(&clip_polygon(&a, &b)).unwrap_or_default())
            }
            _ => Some(Vec::new()),
        },
        2 => match (current, next) {
            (None, _) | (_, None) => None,
            (Some(a), Some(b)) if a.len() < 3 => Some(b),
            (Some(a), Some(b)) if b.len() < 3 => Some(a),
            (Some(a), Some(b)) => {
                let [l, t, r, bottom] = pts_bbox(&a)?;
                let [x, y, w, h] = pts_bbox(&b)?;
                Some(rect_poly([l.min(x), t.min(y), r.max(w), bottom.max(h)]))
            }
        },
        // The canvas accepts a single convex boundary. Non-representable subtractive modes
        // retain the preceding area, matching the established playback fallback.
        _ => current,
    }
}

pub fn map(area: Area, [a, b, c, d, x, y]: Xform) -> Area {
    area.map(|vertices| {
        let transformed: Vec<_> = vertices
            .iter()
            .map(|&(u, v)| (a * u + c * v + x, b * u + d * v + y))
            .collect();
        convex(&transformed).unwrap_or_default()
    })
}
