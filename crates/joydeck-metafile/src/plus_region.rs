//! Bounded EMF+ region algebra. Rings describe where coverage differs from the infinite exterior.

use std::sync::Arc;

use kurbo::BezPath;

use crate::{
    bytes::Bytes,
    cliprect::{clip_polygon, convex, flatten, rect_poly},
    device::Xform,
    geom::{Op, P, polygon},
    plus_shapes::{floats, path},
};

const MAX_EDGES: usize = 768;
const MAX_BANDS: usize = 4096;
const MAX_CELLS: usize = 2048;
const EPSILON: f64 = 1e-8;
pub const WORK_BUDGET: usize = 8_000_000;

#[derive(Clone)]
pub struct Area {
    rings: Arc<[Vec<P>]>,
    outside: bool,
    disjoint: bool,
}

impl Area {
    pub fn full() -> Self {
        Self {
            rings: Arc::from([]),
            outside: true,
            disjoint: true,
        }
    }

    pub fn empty() -> Self {
        Self {
            rings: Arc::from([]),
            outside: false,
            disjoint: true,
        }
    }

    pub fn is_full(&self) -> bool {
        self.outside && self.rings.is_empty()
    }

    pub fn vertices(&self) -> usize {
        self.rings.iter().map(Vec::len).sum()
    }

    pub fn rectangle(bounds: [f64; 4]) -> Self {
        if bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
            return Self::empty();
        }
        Self {
            rings: Arc::from([rect_poly(bounds)]),
            outside: false,
            disjoint: true,
        }
    }

    pub fn outline(ops: &[Op]) -> Option<Self> {
        if ops.len() > MAX_EDGES {
            return None;
        }
        let rings: Vec<_> = flatten(ops)
            .into_iter()
            .map(|(ring, _)| ring)
            .filter(|ring| ring.len() >= 3)
            .collect();
        if rings.iter().map(Vec::len).sum::<usize>() > MAX_EDGES {
            return None;
        }
        Some(Self {
            rings: rings.into(),
            outside: false,
            disjoint: false,
        })
    }

    pub fn convex(&self) -> Option<Vec<P>> {
        if self.outside {
            return None;
        }
        match self.rings.as_ref() {
            [] => Some(Vec::new()),
            [ring] => convex(ring),
            _ => None,
        }
    }

    pub fn map(&self, [a, b, c, d, x, y]: Xform) -> Self {
        let rings: Vec<Vec<P>> = self
            .rings
            .iter()
            .map(|ring| {
                ring.iter()
                    .map(|&(u, v)| (a * u + c * v + x, b * u + d * v + y))
                    .collect()
            })
            .collect();
        if rings
            .iter()
            .flatten()
            .any(|p| !p.0.is_finite() || !p.1.is_finite())
        {
            return Self::empty();
        }
        Self {
            outside: self.outside,
            disjoint: self.disjoint,
            rings: rings.into(),
        }
    }

    pub fn fill(&self, domain: Option<[f64; 4]>, work: &mut usize) -> Option<Vec<Op>> {
        let finite;
        let area = if self.outside {
            finite = combine(self, &Self::rectangle(domain?), 1, work)?;
            &finite
        } else {
            self
        };
        Some(
            area.rings
                .iter()
                .flat_map(|ring| polygon(ring, true))
                .collect(),
        )
    }

    // The clip renderer uses non-zero winding. The sweep provides disjoint, consistently
    // oriented cells, including when the source path uses alternate parity and has holes.
    pub fn mask(&self, domain: Option<[f64; 4]>, work: &mut usize) -> Option<BezPath> {
        let finite = if self.outside {
            combine(self, &Self::rectangle(domain?), 1, work)?
        } else if self.disjoint {
            self.clone()
        } else {
            sweep(self, &Self::empty(), 2, work)?
        };
        let mut bez = BezPath::new();
        for ring in finite.rings.iter() {
            let Some(&first) = ring.first() else { continue };
            bez.move_to(first);
            for &p in &ring[1..] {
                bez.line_to(p);
            }
            bez.close_path();
        }
        Some(bez)
    }
}

pub fn parse(input: Bytes<'_>, work: &mut usize) -> Option<Area> {
    let mut scan = Region {
        bytes: input,
        offset: 8,
        work,
    };
    scan.node(0)
}

struct Region<'a, 'w> {
    bytes: Bytes<'a>,
    offset: usize,
    work: &'w mut usize,
}

impl Region<'_, '_> {
    fn next(&mut self) -> Option<u32> {
        let result = self.bytes.u32(self.offset)?;
        self.offset += 4;
        Some(result)
    }

    fn node(&mut self, depth: usize) -> Option<Area> {
        if depth > 32 {
            return None;
        }
        charge(self.work, 1)?;
        match self.next()? {
            0x1000_0000 => {
                let [x, y, w, h] = floats::<4>(self.bytes, self.offset)?;
                self.offset += 16;
                Some(Area::rectangle([
                    x.min(x + w),
                    y.min(y + h),
                    x.max(x + w),
                    y.max(y + h),
                ]))
            }
            0x1000_0001 => {
                let length = self.next()? as usize;
                let path_bytes = Bytes(self.bytes.slice(self.offset, length)?);
                self.offset += length;
                let count = path_bytes.u32(4)? as usize;
                if count > MAX_EDGES {
                    return None;
                }
                charge(self.work, count)?;
                Area::outline(&path(path_bytes)?)
            }
            0x1000_0002 => Some(Area::empty()),
            0x1000_0003 => Some(Area::full()),
            mode @ 1..=5 => {
                let left = self.node(depth + 1)?;
                let right = self.node(depth + 1)?;
                combine(&left, &right, mode, self.work)
            }
            _ => None,
        }
    }
}

fn charge(work: &mut usize, n: usize) -> Option<()> {
    *work = work.checked_sub(n)?;
    Some(())
}

fn operation(mode: u32, a: bool, b: bool) -> bool {
    match mode {
        1 => a && b,
        2 => a || b,
        3 => a != b,
        4 => a && !b,
        5 => b && !a,
        _ => b,
    }
}

pub fn combine(a: &Area, b: &Area, mode: u32, work: &mut usize) -> Option<Area> {
    if mode == 0 {
        return Some(b.clone());
    }
    if mode > 5 {
        return None;
    }
    if a.rings.is_empty() || b.rings.is_empty() {
        let (area, op) = if a.rings.is_empty() {
            (
                b,
                [
                    operation(mode, a.outside, false),
                    operation(mode, a.outside, true),
                ],
            )
        } else {
            (
                a,
                [
                    operation(mode, false, b.outside),
                    operation(mode, true, b.outside),
                ],
            )
        };
        return Some(if op[0] == op[1] {
            if op[0] { Area::full() } else { Area::empty() }
        } else {
            Area {
                outside: op[usize::from(area.outside)],
                disjoint: area.disjoint,
                rings: area.rings.clone(),
            }
        });
    }
    if mode == 1
        && let (Some(left), Some(right)) = (a.convex(), b.convex())
    {
        charge(work, left.len().saturating_mul(right.len()))?;
        let polygon = clip_polygon(&left, &right);
        return Some(match convex(&polygon) {
            Some(ring) => Area {
                outside: false,
                disjoint: true,
                rings: Arc::from([ring]),
            },
            None => Area::empty(),
        });
    }
    sweep(a, b, mode, work)
}

#[derive(Clone, Copy)]
struct Edge {
    from: P,
    to: P,
    owner: usize,
}

impl Edge {
    fn x_at(self, y: f64) -> f64 {
        self.from.0 + (self.to.0 - self.from.0) * ((y - self.from.1) / (self.to.1 - self.from.1))
    }

    fn crossing(self, other: Self) -> Option<f64> {
        let low = self.from.1.max(other.from.1);
        let high = self.to.1.min(other.to.1);
        if high <= low {
            return None;
        }
        let a = self.x_at(low) - other.x_at(low);
        let b = self.x_at(high) - other.x_at(high);
        if a.abs() <= EPSILON || b.abs() <= EPSILON || a.is_sign_positive() == b.is_sign_positive()
        {
            return None;
        }
        Some(low + (high - low) * (a / (a - b)))
    }
}

fn sweep(a: &Area, b: &Area, mode: u32, budget: &mut usize) -> Option<Area> {
    let mut edges = Vec::new();
    let mut heights = Vec::new();
    for (owner, area) in [a, b].into_iter().enumerate() {
        for ring in area.rings.iter() {
            for (&p, &q) in ring.iter().zip(ring.iter().cycle().skip(1)) {
                if (p.1 - q.1).abs() <= EPSILON {
                    continue;
                }
                let (from, to) = if p.1 < q.1 { (p, q) } else { (q, p) };
                heights.extend([from.1, to.1]);
                edges.push(Edge { from, to, owner });
            }
        }
    }
    if edges.len() > MAX_EDGES {
        return None;
    }
    charge(budget, edges.len() * edges.len())?;
    for (index, edge) in edges.iter().enumerate() {
        for &other in &edges[index + 1..] {
            if let Some(y) = edge.crossing(other) {
                heights.push(y);
                if heights.len() > MAX_BANDS {
                    return None;
                }
            }
        }
    }
    heights.sort_unstable_by(f64::total_cmp);
    heights.dedup_by(|x, y| (*x - *y).abs() <= EPSILON);
    charge(budget, heights.len() * edges.len())?;
    let exterior = [a.outside, b.outside];
    let outside = operation(mode, exterior[0], exterior[1]);
    let mut rings = Vec::new();
    let mut active = Vec::new();
    for band in heights.windows(2) {
        let [top, bottom] = [band[0], band[1]];
        let mid = top + (bottom - top) / 2.0;
        active.clear();
        active.extend(
            edges
                .iter()
                .filter(|e| e.from.1 < mid && mid < e.to.1)
                .map(|&e| (e.x_at(mid), e)),
        );
        active.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        let mut inside = exterior;
        let mut start: Option<Edge> = None;
        let mut at = 0;
        while at < active.len() {
            let (x, edge) = active[at];
            let covered = operation(mode, inside[0], inside[1]) != outside;
            while let Some(&(next, e)) = active.get(at) {
                if (x - next).abs() > EPSILON {
                    break;
                }
                inside[e.owner] = !inside[e.owner];
                at += 1;
            }
            match (covered, operation(mode, inside[0], inside[1]) != outside) {
                (false, true) => start = Some(edge),
                (true, false) if let Some(left) = start.take() => {
                    let ring = vec![
                        (left.x_at(top), top),
                        (edge.x_at(top), top),
                        (edge.x_at(bottom), bottom),
                        (left.x_at(bottom), bottom),
                    ];
                    rings.push(ring);
                    if rings.len() > MAX_CELLS {
                        return None;
                    }
                }
                _ => {}
            }
        }
    }
    Some(Area {
        outside,
        disjoint: true,
        rings: rings.into(),
    })
}
