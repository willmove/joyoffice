//! Resolve points and visual caret stops from the shaped line's grapheme boxes.

use std::collections::BTreeMap;

use super::{Affinity, LineInfo, Point, Pos, TextLayout};

#[derive(Clone, Copy)]
pub(super) struct GlyphBox {
    pub(super) left: f64,
    pub(super) right: f64,
    pub(super) start: usize,
    end: usize,
    rtl: bool,
}

impl GlyphBox {
    fn midpoint(self) -> f64 {
        (self.left + self.right) / 2.0
    }

    fn edge(self, left: bool) -> (f64, usize, Affinity) {
        let x = if left { self.left } else { self.right };
        if left != self.rtl {
            (x, self.start, Affinity::Downstream)
        } else {
            (x, self.end, Affinity::Upstream)
        }
    }
}

pub(super) fn visual_boxes(line: &LineInfo) -> Vec<GlyphBox> {
    let mut boxes = Vec::new();
    let mut start = 0;
    let len = line.end.saturating_sub(line.start);
    while start < len {
        let mut end = start + 1;
        while end < len && line.boundary.get(end) == Some(&false) {
            end += 1;
        }
        if let Some(edges) = line.edges.get(start..end) {
            let (left, right) = edges.iter().fold(
                (f64::INFINITY, f64::NEG_INFINITY),
                |(left, right), &(a, b)| (left.min(a), right.max(b)),
            );
            if left <= right {
                boxes.push(GlyphBox {
                    left,
                    right,
                    start: line.start + start,
                    end: line.start + end,
                    rtl: line.odd(start),
                });
            }
        }
        start = end;
    }
    boxes.sort_by(|a, b| a.left.total_cmp(&b.left));
    boxes
}

pub(super) fn visual_stops(line: &LineInfo) -> Vec<(f64, usize, Affinity)> {
    let boxes = visual_boxes(line);
    let mut stops: Vec<(f64, usize, Affinity)> = Vec::with_capacity(boxes.len() * 2);
    for rect in boxes {
        for stop in [rect.edge(true), rect.edge(false)] {
            if stops.last().is_none_or(|previous| {
                !((previous.0 - stop.0).abs() < 0.01 && previous.1 == stop.1)
            }) {
                stops.push(stop);
            }
        }
    }
    if stops.is_empty() {
        stops.push((
            line.caret_x.first().copied().unwrap_or_default(),
            line.start,
            Affinity::Downstream,
        ));
    }
    stops
}

struct NearbyLine<'a> {
    line: &'a LineInfo,
    across: f64,
    down: f64,
}

impl<'a> NearbyLine<'a> {
    fn measure(line: &'a LineInfo, at: Point) -> Self {
        let (left, right) = line.extent;
        let left = left.min(line.caret_x.first().copied().unwrap_or(left));
        let right = right.max(line.caret_x.last().copied().unwrap_or(right));
        Self {
            line,
            across: (left - at.x).max(at.x - right).max(0.0),
            down: (line.top - at.y).max(at.y - line.bottom).max(0.0),
        }
    }

    fn closer_in_column(&self, other: &Self) -> bool {
        self.down
            .total_cmp(&other.down)
            .then_with(|| self.across.total_cmp(&other.across))
            .is_lt()
    }

    fn distance(&self) -> f64 {
        self.down.hypot(self.across)
    }
}

pub(super) fn hit(layout: &TextLayout, point: Point) -> (Pos, Affinity) {
    let mut columns = BTreeMap::new();
    for line in &layout.lines {
        let nearest = NearbyLine::measure(line, point);
        match columns.entry(line.column) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(nearest);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if nearest.closer_in_column(entry.get()) {
                    entry.insert(nearest);
                }
            }
        }
    }
    let Some(line) = columns
        .into_values()
        .min_by(|a, b| a.distance().total_cmp(&b.distance()))
        .map(|candidate| candidate.line)
    else {
        return (Pos::default(), Affinity::Downstream);
    };

    hit_line(line, point.x)
}

pub(super) fn hit_line(line: &LineInfo, x: f64) -> (Pos, Affinity) {
    let boxes = visual_boxes(line);
    let (_, ch, affinity) = match edge_at(&boxes, x) {
        Some(stop) => stop,
        None => (0.0, line.start, Affinity::Downstream),
    };
    (
        Pos {
            para: line.para,
            ch,
        },
        affinity,
    )
}

fn edge_at(boxes: &[GlyphBox], x: f64) -> Option<(f64, usize, Affinity)> {
    let covered = boxes
        .iter()
        .find(|entry| x >= entry.left && x <= entry.right && entry.right > entry.left);
    if let Some(entry) = covered {
        return Some(entry.edge(x < entry.midpoint()));
    }
    let first = boxes.first()?;
    if x < first.left {
        return Some(first.edge(true));
    }
    let last = boxes.last()?;
    if x > last.right {
        return Some(last.edge(false));
    }
    let nearest = boxes.iter().min_by(|a, b| {
        (a.midpoint() - x)
            .abs()
            .total_cmp(&(b.midpoint() - x).abs())
    })?;
    Some(nearest.edge(x < nearest.midpoint()))
}
