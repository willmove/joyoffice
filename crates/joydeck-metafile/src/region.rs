//! Bounded device-space rectangle regions for the GDI clip records.

use crate::bytes::Bytes;
use crate::canvas::Canvas;
use crate::vector_records::emf_rect;

type Rect = [f64; 4];
type Region = Option<Vec<Rect>>;

pub const MAX_CLIP_RECTS: usize = 64;
const MIN_SPAN: f64 = 1e-6;
const RGN_AND: u32 = 1;
const RGN_OR: u32 = 2;
const RGN_XOR: u32 = 3;
const RGN_DIFF: u32 = 4;
const RGN_COPY: u32 = 5;

/// EMR_EXTSELECTCLIPRGN carries a RegionDataHeader followed by RECTL entries.
pub fn play_emf(canvas: &mut Canvas, record: Bytes<'_>) -> Option<()> {
    let declared = record.u32(8)? as usize;
    let mode = record.u32(12)?;
    if declared == 0 {
        canvas.reset_clip();
        return Some(());
    }
    const HEADER: usize = 32;
    const RECT: usize = 16;
    let region = record.tail(16)?;
    if declared < HEADER || region.u32(4)? != 1 {
        return None;
    }
    let count = (region.u32(8)? as usize)
        .min(MAX_CLIP_RECTS)
        .min((declared - HEADER) / RECT);
    let rectangles: Option<Vec<_>> = (0..count)
        .map(|index| emf_rect(region, HEADER + index * RECT))
        .collect();
    canvas.select_clip_rects(&rectangles?, mode);
    Some(())
}

fn nonempty(rect: Rect) -> Option<Rect> {
    (rect.iter().all(|coordinate| coordinate.is_finite())
        && rect[2] - rect[0] > MIN_SPAN
        && rect[3] - rect[1] > MIN_SPAN)
        .then_some(rect)
}

pub fn normalize([l, t, r, b]: Rect) -> Option<Rect> {
    [l, t, r, b]
        .iter()
        .all(|coordinate| coordinate.is_finite())
        .then(|| nonempty([l.min(r), t.min(b), l.max(r), t.max(b)]))
        .flatten()
}

fn overlap([l, t, r, b]: Rect, [x0, y0, x1, y1]: Rect) -> Option<Rect> {
    nonempty([l.max(x0), t.max(y0), r.min(x1), b.min(y1)])
}

/// Split the visible remainder into horizontal outer strips and the two sides of the cut.
fn difference(subject: Rect, cut: Rect) -> impl Iterator<Item = Rect> {
    let [l, t, r, b] = subject;
    let pieces = match overlap(subject, cut) {
        Some([x0, y0, x1, y1]) => [
            [l, t, r, y0],
            [l, y1, r, b],
            [l, y0, x0, y1],
            [x1, y0, r, y1],
        ],
        None => [subject, [0.0; 4], [0.0; 4], [0.0; 4]],
    };
    pieces.into_iter().filter_map(nonempty)
}

fn without(mut visible: Vec<Rect>, cuts: &[Rect]) -> Vec<Rect> {
    for &cut in cuts {
        // Keep applying later cuts even if one operation reaches the rectangle budget.
        visible = visible
            .into_iter()
            .flat_map(|rect| difference(rect, cut))
            .take(MAX_CLIP_RECTS)
            .collect();
    }
    visible
}

pub fn intersect(current: Region, rect: Rect) -> Region {
    let incoming: Vec<_> = normalize(rect).into_iter().collect();
    Some(intersections(current, incoming))
}

fn intersections(current: Region, incoming: Vec<Rect>) -> Vec<Rect> {
    match current {
        None => incoming,
        Some(current) => current
            .into_iter()
            .flat_map(|a| incoming.iter().filter_map(move |&b| overlap(a, b)))
            .take(MAX_CLIP_RECTS)
            .collect(),
    }
}

fn visible_in_surface(current: Region, surface: Option<Rect>) -> Region {
    current.or_else(|| surface.and_then(normalize).map(|surface| vec![surface]))
}

pub fn exclude(current: Region, rect: Rect, surface: Option<Rect>) -> Region {
    match normalize(rect) {
        None => current,
        Some(rect) => visible_in_surface(current, surface).map(|visible| without(visible, &[rect])),
    }
}

/// `None` is the default surface and an empty vector is a fully clipped context.
pub fn combine(current: Region, incoming: Vec<Rect>, mode: u32, surface: Option<Rect>) -> Region {
    let incoming: Vec<_> = incoming
        .into_iter()
        .filter_map(normalize)
        .take(MAX_CLIP_RECTS)
        .collect();
    match mode {
        RGN_COPY => Some(incoming),
        RGN_AND => Some(intersections(current, incoming)),
        RGN_OR => current.map(|visible| {
            visible
                .into_iter()
                .chain(incoming)
                .take(MAX_CLIP_RECTS)
                .collect()
        }),
        RGN_DIFF => visible_in_surface(current, surface).map(|visible| without(visible, &incoming)),
        RGN_XOR => current.map(|visible| {
            let only_in = without(incoming.clone(), &visible);
            without(visible, &incoming)
                .into_iter()
                .chain(only_in)
                .take(MAX_CLIP_RECTS)
                .collect()
        }),
        _ => current,
    }
}

#[cfg(test)]
#[path = "region_tests.rs"]
mod tests;
