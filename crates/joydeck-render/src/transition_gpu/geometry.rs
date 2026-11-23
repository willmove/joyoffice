//! CPU reference coverage with attributes for the GPU's ordered tile compositor.
use crate::transition::{
    PixelAlignedRect, pixel_aligned_rect, sample_x, sample_y, triangle_edge,
    triangle_row_candidates, triangle_weights,
};
use joydeck_anim::{Layer, MeshVertex, layer_mesh};

pub(super) const TILE: u32 = 8;
const MAX_PRIMITIVES: usize = 128_000;
const MAX_ROWS: usize = 1_048_576;
const MAX_TILE_REFERENCES: usize = 4_194_304;
const PIXEL_RECT: u32 = 8;

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Primitive {
    step_x: [f32; 3],
    flags: u32,
    first_row: u32,
    y_start: u32,
    y_end: u32,
    padding: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Row {
    attributes: [f32; 3],
    span: u32,
}

impl Row {
    fn new(start: usize, end: usize, attributes: [f32; 3]) -> Self {
        Self {
            attributes,
            span: start as u32 | ((end as u32) << 16),
        }
    }
}

#[derive(Clone, Copy)]
struct Bounds {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

impl Bounds {
    fn is_empty(self) -> bool {
        self.x0 == self.x1 || self.y0 == self.y1
    }

    fn tiles(self, tiles_x: usize) -> impl Iterator<Item = usize> {
        let side = TILE as usize;
        let x = self.x0 / side..self.x1.div_ceil(side);
        (self.y0 / side..self.y1.div_ceil(side))
            .flat_map(move |y| x.clone().map(move |x| y * tiles_x + x))
    }
}

pub(super) struct FrameGeometry {
    pub(super) primitives: Vec<Primitive>,
    pub(super) rows: Vec<Row>,
    pub(super) tiles: Vec<[u32; 2]>,
    pub(super) references: Vec<u32>,
    bounds: Vec<Bounds>,
    width: usize,
    height: usize,
    covered_work: usize,
}

impl FrameGeometry {
    pub(super) fn prepare<'a>(
        width: u32,
        height: u32,
        layers: impl IntoIterator<Item = (u32, &'a Layer)>,
    ) -> Result<Self, String> {
        let mut frame = Self {
            primitives: Vec::new(),
            rows: Vec::new(),
            tiles: Vec::new(),
            references: Vec::new(),
            bounds: Vec::new(),
            width: width as usize,
            height: height as usize,
            covered_work: 0,
        };
        for (source, layer) in layers {
            if !layer.alpha.is_finite() || layer.alpha <= 0. {
                continue;
            }
            if let Some(rect) = pixel_aligned_rect(layer, frame.width, frame.height, source >= 6) {
                frame.rect(rect, source, layer.alpha as f32)?;
                continue;
            }
            let (vertices, triangles) = layer_mesh(layer);
            if frame.primitives.len() + triangles.len() > MAX_PRIMITIVES {
                return Err("transition exceeds mesh budget".into());
            }
            for indices in triangles {
                frame.triangle(indices.map(|index| vertices[index as usize]), source)?;
            }
        }
        frame.bin(
            width.div_ceil(TILE) as usize,
            height.div_ceil(TILE) as usize,
        )?;
        // Storage bindings need a full element even if no tile refers to one.
        if frame.primitives.is_empty() {
            frame.primitives.push(Primitive::default());
            frame.rows.push(Row::default());
            frame.references.push(0);
        }
        Ok(frame)
    }

    fn begin(&mut self, bounds: Bounds, flags: u32, step_x: [f32; 3]) -> Result<(), String> {
        self.covered_work += (bounds.x1 - bounds.x0) * (bounds.y1 - bounds.y0);
        if self.covered_work > self.width * self.height * 96 {
            return Err("transition exceeds output coverage budget".into());
        }
        if self.primitives.len() == MAX_PRIMITIVES
            || self.rows.len() + bounds.y1 - bounds.y0 > MAX_ROWS
        {
            return Err("transition exceeds span geometry budget".into());
        }
        self.primitives.push(Primitive {
            step_x,
            flags,
            first_row: self.rows.len() as u32,
            y_start: bounds.y0 as u32,
            y_end: bounds.y1 as u32,
            padding: 0,
        });
        self.bounds.push(bounds);
        Ok(())
    }

    fn rect(&mut self, rect: PixelAlignedRect, source: u32, alpha: f32) -> Result<(), String> {
        let bounds = Bounds {
            x0: rect.x.start,
            y0: rect.y.start,
            x1: rect.x.end,
            y1: rect.y.end,
        };
        if bounds.is_empty() {
            return Ok(());
        }
        self.begin(bounds, source | PIXEL_RECT, [1., 0., 0.])?;
        let x = (bounds.x0 as isize + rect.offset[0]) as f32;
        self.rows.extend(rect.y.map(|y| {
            Row::new(
                bounds.x0,
                bounds.x1,
                [x, (y as isize + rect.offset[1]) as f32, alpha],
            )
        }));
        Ok(())
    }

    fn triangle(&mut self, vertices: [MeshVertex; 3], source: u32) -> Result<(), String> {
        let [width, height] = [self.width as f64, self.height as f64];
        let points = vertices.map(|v| [v.pos[0] * width, v.pos[1] * height]);
        let [a, b, c] = points;
        let area = triangle_edge(a, b, c);
        if !area.is_finite() || area.abs() < 1e-12 {
            return Ok(());
        }
        let bounds = Bounds {
            x0: a[0].min(b[0]).min(c[0]).floor().clamp(0., width) as usize,
            y0: a[1].min(b[1]).min(c[1]).floor().clamp(0., height) as usize,
            x1: a[0].max(b[0]).max(c[0]).ceil().clamp(0., width) as usize,
            y1: a[1].max(b[1]).max(c[1]).ceil().clamp(0., height) as usize,
        };
        if bounds.is_empty() {
            return Ok(());
        }
        let attributes = vertices.map(|v| [v.uv[0], v.uv[1], v.a.clamp(0., 1.)]);
        let interpolate = |weights: [f64; 3], attribute: usize| {
            weights[0] * attributes[0][attribute]
                + weights[1] * attributes[1][attribute]
                + weights[2] * attributes[2][attribute]
        };
        let dx_weights = [
            (b[1] - c[1]) / area,
            (c[1] - a[1]) / area,
            (a[1] - b[1]) / area,
        ];
        let step_x = [
            (interpolate(dx_weights, 0) * width) as f32,
            (interpolate(dx_weights, 1) * height) as f32,
            interpolate(dx_weights, 2) as f32,
        ];
        self.begin(bounds, source, step_x)?;
        for y in bounds.y0..bounds.y1 {
            let sy = sample_y(y);
            let mut row = Row::default();
            if let Some((lo, hi)) = triangle_row_candidates(points, sy, self.width) {
                let weights = |x| triangle_weights(points, [sample_x(x), sy], area);
                let covered = |x| weights(x).iter().all(|&weight| weight >= 0.);
                if let Some(start) = (lo..hi).find(|&x| covered(x)) {
                    let end = (start + 1..hi)
                        .rfind(|&x| covered(x))
                        .map_or(start + 1, |x| x + 1);
                    let w = weights(start);
                    row = Row::new(
                        start,
                        end,
                        [
                            (interpolate(w, 0) * width - 0.5) as f32,
                            (interpolate(w, 1) * height - 0.5) as f32,
                            interpolate(w, 2) as f32,
                        ],
                    );
                }
            }
            self.rows.push(row);
        }
        Ok(())
    }

    fn bin(&mut self, tiles_x: usize, tiles_y: usize) -> Result<(), String> {
        let mut tiles = vec![[0, 0u32]; tiles_x * tiles_y];
        let mut total = 0usize;
        for bounds in &self.bounds {
            for tile in bounds.tiles(tiles_x) {
                total += 1;
                if total > MAX_TILE_REFERENCES {
                    return Err("transition exceeds tile budget".into());
                }
                tiles[tile][1] += 1;
            }
        }
        let mut offset = 0;
        for [start, count] in &mut tiles {
            *start = offset;
            offset += *count;
            *count = 0;
        }
        let mut references = vec![0; total];
        for (index, bounds) in self.bounds.iter().enumerate() {
            for tile in bounds.tiles(tiles_x) {
                let [start, count] = &mut tiles[tile];
                references[(*start + *count) as usize] = index as u32;
                *count += 1;
            }
        }
        self.tiles = tiles;
        self.references = references;
        Ok(())
    }
}
