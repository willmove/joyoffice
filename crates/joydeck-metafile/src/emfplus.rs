//! EMF+ (MS-EMFPLUS): GDI+ drawing records carried in EMF comment records.
//!
//! Most EMF+ files are *dual*: the same picture is also drawn with plain EMF records, which
//! [`crate::emf`] already plays, so this reader ignores their EMF+ part. An *EMF+ only* file has
//! nothing else to draw; for those this reader plays the records:
//! - brushes (solid, hatch, texture, linear and path gradients), pens, paths, images, fonts,
//!   string formats and regions, including objects split across several records;
//! - rectangles, polygons, lines, ellipses, paths and Béziers;
//! - bitmaps (raw pixel formats, and PNG/JPEG/GIF/TIFF/BMP payloads) and strings (DrawString,
//!   DrawDriverString with character codes);
//! - world and page transforms, save/restore and containers;
//! - arcs and pies;
//! - clipping to paths and regions, including non-convex outlines and boolean combinations;
//! - nested metafile images and driver strings with physical glyph indices;
//! - cardinal splines (DrawCurve, DrawClosedCurve, FillClosedCurve), FillRegion, and Clear
//!   (over the picture frame).
//!
//! Every drawing record of MS-EMFPLUS §2.3.4 is played. Image attributes are not drawn.
//! Deliberately ignored: rendering hints (antialias, text
//! hint, interpolation, pixel offset, compositing; the renderer has its own), SetRenderingOrigin
//! (hatch phase), SerializableObject image effects, the terminal-server records SetTSGraphics and
//! SetTSClip, and StrokeFillPath (0x4037), which the specification lists but gives no layout for
//! and GDI+ doesn't write.

use std::sync::Arc;

use crate::bitmap::Rgba;
use crate::bytes::Bytes;
use crate::canvas::{
    BS_NULL, BS_SOLID, Brush, Canvas, LogFont, PS_NULL, Pen, RawTile, Xform, hatch_tile, mul,
};
use crate::canvas::{PlusClip, Raw};
use crate::geom::{Op, P};
use crate::plus_image::{ImageData, image};
use crate::plus_paint::{Brush as Fill, argb, blend as lerp, brush};
use crate::plus_pen::{self, Pen as PlusPen};
use crate::plus_region::{self, Area};
use crate::plus_shapes::{Drawing, Geometry, Paint, floats, path};
use crate::plus_text::{DriverString, FontSpec, Format, Run, unit_px};
use crate::text::ETO_CLIPPED;
use crate::{Item, Picture, Seg};

/// "EMF+" as the comment identifier of an EMR_COMMENT holding EMF+ records.
pub(crate) const EMF_PLUS: u32 = 0x2B46_4D45;
const IDENTITY: Xform = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
/// GDI+ allows 64 objects; ids are a byte.
const MAX_OBJECTS: usize = 64;
const MAX_STACK: usize = 64;
#[derive(Clone)]
enum Obj {
    Brush(Fill),
    Pen(Arc<PlusPen>),
    Path(Vec<Op>),
    Image(Arc<ImageData>),
    Font(FontSpec),
    Format(Format),
    /// A region object in world units.
    Region(Area),
}

struct Saved {
    id: u32,
    world: Xform,
    page: f64,
    clip: Area,
    clip_paint: Option<PlusClip>,
}

pub(crate) struct Plus {
    /// An EMF+ header said the file is EMF+ only, so these records are the picture.
    active: bool,
    dpi: f64,
    world: Xform,
    page: f64,
    /// The logical region and the compiled mask are in device units.
    clip: Area,
    clip_paint: Option<PlusClip>,
    clip_work: usize,
    objects: Vec<Option<Obj>>,
    object_bytes: usize,
    stack: Vec<Saved>,
    objects_pending: crate::plus_objects::Assembly,
    /// The picture frame in device units (`[x, y, w, h]`), what Clear paints.
    pub frame: Option<[f64; 4]>,
}

impl Plus {
    pub fn new() -> Self {
        Plus {
            active: false,
            dpi: 96.0,
            world: IDENTITY,
            page: 1.0,
            clip: Area::full(),
            clip_paint: None,
            clip_work: plus_region::WORK_BUDGET,
            objects: vec![None; MAX_OBJECTS],
            object_bytes: 0,
            stack: Vec::new(),
            objects_pending: Default::default(),
            frame: None,
        }
    }

    /// Plays the EMF+ records of one EMR_COMMENT payload (after the identifier).
    pub fn comment(&mut self, cv: &mut Canvas, data: Bytes) {
        for entry in crate::records::read(data, 0, crate::records::Format::EmfPlus) {
            if cv.full() {
                break;
            }
            let _ = self.record(cv, entry.kind as u16, entry.flags, entry.data);
        }
    }

    /// World units to device pixels before the canvas's own mapping.
    fn xf(&self) -> Xform {
        mul(self.world, [self.page, 0.0, 0.0, self.page, 0.0, 0.0])
    }

    fn painter(&self) -> Painter {
        Painter {
            xf: self.xf(),
            clip: self.clip_paint.clone(),
        }
    }

    fn record(&mut self, cv: &mut Canvas, typ: u16, flags: u16, r: Bytes) -> Option<()> {
        if typ == 0x4001 {
            // Header: dual files also draw with EMF records, which win.
            self.active = flags & 1 == 0;
            if let Some(d) = r
                .u32(8)
                .map(f64::from)
                .filter(|d| *d > 0.0 && *d < 100_000.0)
            {
                self.dpi = d;
            }
            return Some(());
        }
        if !self.active {
            return None;
        }
        match typ {
            0x4008 => self.object(flags, r),
            0x4009 | 0x4013 | 0x401A..=0x401C | 0x4036 => {
                self.record_image_text_region(cv, typ, flags, r)
            }
            0x4025..=0x4035 => self.record_state(cv, typ, flags, r),
            _ => self.record_shape(cv, typ, flags, r),
        }
    }

    fn record_shape(&self, canvas: &mut Canvas, typ: u16, flags: u16, bytes: Bytes) -> Option<()> {
        let Drawing { paint, geometry } = crate::plus_shapes::drawing(typ, flags, bytes)?;
        let painter = self.painter();
        let paint = match paint {
            Paint::Fill(value) => {
                let brush = self.brush_of(flags, value)?;
                EitherPaint::Brush(brush)
            }
            Paint::Stroke(id) => {
                let pen = self.pen(id)?;
                let scale = if pen.unit <= 2 {
                    1.0
                } else {
                    unit_px(pen.unit, self.dpi) / self.page
                };
                EitherPaint::Pen(pen, pen.width * scale)
            }
        };
        let draw = |canvas: &mut Canvas, path: &[Op]| paint.play(&painter, canvas, path);
        match geometry {
            Geometry::Object(id) => draw(canvas, &self.path(id)?),
            Geometry::Path(path) => draw(canvas, &path),
            Geometry::Rectangles(rectangles) => {
                for rectangle in rectangles {
                    if canvas.full() {
                        break;
                    }
                    draw(canvas, &rect_ops(rectangle));
                }
            }
        }
        Some(())
    }

    /// Clear, region fill, image and text records.
    fn record_image_text_region(
        &mut self,
        cv: &mut Canvas,
        typ: u16,
        flags: u16,
        r: Bytes,
    ) -> Option<()> {
        use crate::plus_shapes::{Command, command};
        match command(typ, flags, r)? {
            Command::Clear(value) => {
                let color = argb(value);
                let [x, y, w, h] = self.frame?;
                if color[3] != 0 && w > 0.0 && h > 0.0 {
                    let xf = invert(mul(cv.world(), cv.device_affine()))?;
                    Painter { xf, clip: None }.fill(
                        cv,
                        &rect_ops([x, y, x + w, y + h]),
                        &Fill::Solid(color),
                    );
                }
            }
            Command::Region { slot, brush } => {
                let fill = self.brush_of(flags, brush)?;
                let Some(Some(Obj::Region(area))) = self.objects.get(slot) else {
                    return None;
                };
                let domain = cv.clip_surface().and_then(|[l, t, r, b]| {
                    let inverse = invert(mul(self.xf(), cv.device_affine()))?;
                    let world = crate::cliprect::rect_poly([l, t, r, b]);
                    let world: Vec<_> = world
                        .into_iter()
                        .map(|point| apply(inverse, point))
                        .collect();
                    crate::cliprect::pts_bbox(&world)
                });
                let ops = area.fill(domain, &mut self.clip_work)?;
                self.painter().fill(cv, &ops, &fill);
            }
            Command::Image { slot, destination } => {
                return self.draw_image(cv, self.image(slot)?.as_ref(), r, destination);
            }
            Command::Text => return self.draw_string(cv, flags, r),
            Command::DriverText => return self.draw_driver_string(cv, flags, r),
        }
        Some(())
    }

    /// Save / restore, transform and clip records.
    fn record_state(&mut self, cv: &mut Canvas, typ: u16, flags: u16, r: Bytes) -> Option<()> {
        use crate::plus_commands::{State, state};
        match state(typ, flags, r)? {
            State::Save(id) => self.push(id),
            State::Restore(id) => self.pop(id),
            State::Transform(matrix) => self.world = matrix,
            State::Concat(matrix) => return self.apply(matrix, flags),
            State::Page(scale) => {
                let page = scale * unit_px(u32::from(flags & 0xff), self.dpi);
                if (0.0..1.0e6).contains(&page) && page > 0.0 {
                    self.page = page;
                }
            }
            State::ClearClip => self.set_clip(cv, Area::full()),
            State::ClipRectangle(bounds) => {
                let area = self.map_clip(cv, &Area::rectangle(bounds));
                return self.combine(cv, flags, &area);
            }
            State::ClipPath(id) => {
                let ops = self.path(id)?;
                let Some(area) = Area::outline(&ops) else {
                    self.set_clip(cv, Area::empty());
                    return None;
                };
                let area = self.map_clip(cv, &area);
                return self.combine(cv, flags, &area);
            }
            State::ClipRegion(id) => {
                let Some(Some(Obj::Region(region))) = self.objects.get(id) else {
                    return None;
                };
                let area = self.map_clip(cv, region);
                return self.combine(cv, flags, &area);
            }
            State::ClipOffset((dx, dy)) => {
                let [a, b, c, d, ..] = mul(self.xf(), cv.device_affine());
                let (x, y) = (dx * a + dy * c, dx * b + dy * d);
                self.set_clip(cv, self.clip.map([1.0, 0.0, 0.0, 1.0, x, y]));
            }
        }
        Some(())
    }

    fn map_clip(&self, canvas: &Canvas, area: &Area) -> Area {
        area.map(mul(self.xf(), canvas.device_affine()))
    }

    /// Without the `A` flag a transform is applied before the world transform (prepended).
    fn apply(&mut self, m: Xform, flags: u16) -> Option<()> {
        self.world = if flags & 0x2000 != 0 {
            mul(self.world, m)
        } else {
            mul(m, self.world)
        };
        Some(())
    }

    fn push(&mut self, id: u32) {
        if self.stack.len() < MAX_STACK {
            self.stack.push(Saved {
                id,
                world: self.world,
                page: self.page,
                clip: self.clip.clone(),
                clip_paint: self.clip_paint.clone(),
            });
        }
    }

    fn pop(&mut self, id: u32) {
        if let Some(i) = self.stack.iter().rposition(|s| s.id == id) {
            let s = self.stack.swap_remove(i);
            (self.world, self.page, self.clip, self.clip_paint) =
                (s.world, s.page, s.clip, s.clip_paint);
            self.stack.truncate(i);
        }
    }

    fn set_clip(&mut self, canvas: &Canvas, area: Area) {
        self.clip_paint = if area.is_full() {
            None
        } else if let Some(ring) = area.convex() {
            Some(PlusClip::Convex(ring.into()))
        } else {
            Some(PlusClip::Outline(Arc::new(
                area.mask(canvas.clip_surface(), &mut self.clip_work)
                    .unwrap_or_default(),
            )))
        };
        self.clip = area;
    }

    fn combine(&mut self, canvas: &Canvas, flags: u16, new: &Area) -> Option<()> {
        let mode = u32::from((flags >> 8) & 0xf);
        let result = plus_region::combine(&self.clip, new, mode, &mut self.clip_work);
        if mode <= 5 {
            self.set_clip(canvas, result.unwrap_or_else(Area::empty));
        }
        Some(())
    }

    fn object(&mut self, flags: u16, r: Bytes) -> Option<()> {
        let decoded = self.objects_pending.push(flags, r)?;
        let object = parse_object(
            decoded.kind,
            Bytes(decoded.payload.as_ref()),
            &mut self.clip_work,
        );
        self.store(decoded.slot, object);
        Some(())
    }

    fn store(&mut self, id: usize, obj: Option<Obj>) {
        let Some(slot) = self.objects.get_mut(id) else {
            return;
        };
        if let Some(old) = slot.take() {
            self.object_bytes = self.object_bytes.saturating_sub(Self::obj_cost(&old));
        }
        if let Some(new_obj) = obj {
            let cost = Self::obj_cost(&new_obj);
            if self.object_bytes.saturating_add(cost) <= crate::MAX_BYTES {
                self.object_bytes += cost;
                *slot = Some(new_obj);
            }
        }
    }

    fn obj_cost(obj: &Obj) -> usize {
        match obj {
            Obj::Image(img) => match img.as_ref() {
                ImageData::Bitmap(rgba) => rgba.px.len(),
                ImageData::Meta(pic) => pic.items.len().saturating_mul(128),
            },
            Obj::Brush(Fill::Texture { img, .. }) => img.px.len(),
            Obj::Pen(pen) => pen.brush.cost(),
            Obj::Path(ops) => ops.len().saturating_mul(std::mem::size_of::<Op>()),
            Obj::Region(area) => area.vertices().saturating_mul(std::mem::size_of::<P>()),
            _ => 0,
        }
    }

    fn path(&self, id: usize) -> Option<Vec<Op>> {
        match self.objects.get(id)? {
            Some(Obj::Path(ops)) => Some(ops.clone()),
            _ => None,
        }
    }

    fn pen(&self, id: usize) -> Option<&PlusPen> {
        match self.objects.get(id)? {
            Some(Obj::Pen(pen)) => Some(pen.as_ref()),
            _ => None,
        }
    }

    fn image(&self, id: usize) -> Option<Arc<ImageData>> {
        match self.objects.get(id)? {
            Some(Obj::Image(img)) => Some(img.clone()),
            _ => None,
        }
    }

    /// The brush of a fill record: an ARGB value with the `S` flag, else a brush object.
    fn brush_of(&self, flags: u16, v: u32) -> Option<Fill> {
        if flags & 0x8000 != 0 {
            return Some(Fill::Solid(argb(v)));
        }
        match self.objects.get(v as usize)? {
            Some(Obj::Brush(f)) => Some(f.clone()),
            _ => None,
        }
    }

    /// DrawImage and DrawImagePoints: the source rectangle (pixels) cut from the image, drawn
    /// into world rectangle `dst` = `[x, y, w, h]`.
    fn draw_image(&self, cv: &mut Canvas, img: &ImageData, r: Bytes, dst: [f64; 4]) -> Option<()> {
        let img = match img {
            ImageData::Bitmap(b) => b,
            ImageData::Meta(pic) => {
                self.painter().picture(cv, dst, pic);
                return Some(());
            }
        };
        let [sx, sy, sw, sh] = floats::<4>(r, 8)?;
        let src = if r.u32(4)? == 2 {
            [sx, sy, sw, sh].map(|v| v.round().clamp(-1.0e9, 1.0e9) as i64)
        } else {
            [0; 4]
        };
        let (part, dst) = crate::bitmap::clip_blit(img, src, dst)?;
        self.painter().image(cv, dst, part);
        Some(())
    }

    fn font(&self, id: usize) -> Option<FontSpec> {
        match self.objects.get(id)? {
            Some(Obj::Font(f)) => Some(f.clone()),
            _ => None,
        }
    }

    fn draw_string(&self, canvas: &mut Canvas, flags: u16, bytes: Bytes) -> Option<()> {
        let font = self
            .font(usize::from(flags & 0xff))?
            .logical(self.dpi, self.page);
        let color = self.brush_of(flags, bytes.u32(0)?)?.color();
        let format = match self.objects.get(bytes.u32(4)? as usize) {
            Some(Some(Obj::Format(format))) => *format,
            _ => Format::default(),
        };
        let (runs, clip) = crate::plus_text::layout(bytes, format, f64::from(-font.font.height))?;
        self.play_text(canvas, font, color, runs, clip, IDENTITY);
        Some(())
    }

    fn draw_driver_string(&self, canvas: &mut Canvas, flags: u16, bytes: Bytes) -> Option<()> {
        let font = self
            .font(usize::from(flags & 0xff))?
            .logical(self.dpi, self.page);
        let color = self.brush_of(flags, bytes.u32(0)?)?.color();
        let (draw, xf) = crate::plus_text::driver(bytes, -font.font.height)?;
        match draw {
            DriverString::Text(runs) => self.play_text(canvas, font, color, runs, None, xf),
            DriverString::Glyphs { at, indexed } => {
                let painter = self.painter();
                painter.begin(canvas, xf);
                canvas.set_text_style(color, font, 24);
                canvas.indexed_text(at, indexed);
                painter.end(canvas);
            }
        }
        Some(())
    }

    fn play_text(
        &self,
        canvas: &mut Canvas,
        font: LogFont,
        color: [u8; 4],
        runs: Vec<Run>,
        clip: Option<[f64; 4]>,
        xf: Xform,
    ) {
        let painter = self.painter();
        for run in runs {
            if canvas.full() {
                break;
            }
            painter.text(
                canvas,
                run.at,
                run.text,
                run.advances,
                TextStyle {
                    color,
                    font: font.clone(),
                    align: run.align,
                    clip,
                },
                xf,
            );
        }
    }
}

enum EitherPaint<'a> {
    Brush(Fill),
    Pen(&'a PlusPen, f64),
}

impl EitherPaint<'_> {
    fn play(&self, painter: &Painter, canvas: &mut Canvas, ops: &[Op]) {
        match self {
            Self::Brush(brush) => painter.fill(canvas, ops, brush),
            Self::Pen(pen, width) => painter.stroke(canvas, ops, pen, *width),
        }
    }
}

struct TextStyle {
    color: [u8; 4],
    font: LogFont,
    align: u32,
    /// World clip rectangle `[l, t, r, b]`.
    clip: Option<[f64; 4]>,
}

/// Draws through the canvas with EMF+'s transform, clip and its own pen or brush, then puts the
/// canvas's GDI state back.
struct Painter {
    xf: Xform,
    clip: Option<PlusClip>,
}

impl Painter {
    fn begin(&self, cv: &mut Canvas, pre: Xform) {
        let world = cv.world();
        cv.save_dc();
        cv.modify_world(mul(mul(pre, self.xf), world), 4);
        cv.set_clip(self.clip.clone());
    }

    fn end(&self, cv: &mut Canvas) {
        cv.restore_dc(-1);
        cv.set_clip(None);
    }

    fn no_pen(cv: &mut Canvas) {
        cv.set_pen(Pen {
            style: PS_NULL,
            width: 0.0,
            color: [0, 0, 0, 0],
        });
    }

    fn fill(&self, cv: &mut Canvas, ops: &[Op], fill: &Fill) {
        self.fill_path(cv, ops, fill, true);
    }

    fn fill_path(&self, cv: &mut Canvas, ops: &[Op], fill: &Fill, alternate: bool) {
        self.begin(cv, IDENTITY);
        cv.set_polyfill(if alternate { 1 } else { 2 });
        Self::no_pen(cv);
        match fill {
            Fill::Solid(c) => {
                cv.set_brush(Brush::new(BS_SOLID, *c));
                cv.shape(ops, true, false);
            }
            Fill::Hatch { style, fg, bg } => match hatch_tile(*style, *fg, Some(*bg)) {
                Some(px) => cv.tile_shape(
                    ops,
                    Arc::new(RawTile {
                        w: 8,
                        h: 8,
                        px,
                        cell: cv.pattern_px,
                        sampled: false,
                    }),
                ),
                // GDI+ has many more hatch styles than GDI: those fill with the blend of both colours.
                None => {
                    cv.set_brush(Brush::new(BS_SOLID, lerp(*fg, *bg, 0.5)));
                    cv.shape(ops, true, false);
                }
            },
            _ => {
                // Device point to brush space: invert brush space → world → device.
                let tf = if fill.device_space() {
                    fill.xf()
                } else {
                    mul(fill.xf(), cv.device_affine())
                };
                if let Some(inv) = invert(tf) {
                    cv.shade_shape(ops, &|p| fill.sample(apply(inv, p)));
                }
            }
        }
        self.end(cv);
    }

    fn stroke(&self, cv: &mut Canvas, ops: &[Op], pen: &PlusPen, width: f64) {
        if !pen.simple {
            if let Some(outline) = pen.outline(ops, width) {
                self.fill_path(cv, &outline, &pen.brush, false);
            }
            return;
        }
        self.begin(cv, IDENTITY);
        cv.set_pen(Pen {
            style: 0,
            width,
            color: pen.brush.color(),
        });
        cv.set_brush(Brush::new(BS_NULL, [0, 0, 0, 0]));
        cv.shape(ops, false, true);
        self.end(cv);
    }

    fn image(&self, cv: &mut Canvas, dst: [f64; 4], img: Rgba) {
        self.begin(cv, IDENTITY);
        cv.bitmap(dst, img);
        self.end(cv);
    }

    /// A nested metafile's picture stretched into world rectangle `dst` = `[x, y, w, h]`.
    fn picture(&self, cv: &mut Canvas, [x, y, w, h]: [f64; 4], pic: &Picture) {
        self.begin(cv, IDENTITY);
        let (sx, sy) = (w / f64::from(pic.width), h / f64::from(pic.height));
        let m = mul([sx, 0.0, 0.0, sy, x, y], cv.device_affine());
        let scale = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
        let pt = |a: f32, b: f32| apply(m, (f64::from(a), f64::from(b)));
        let ops = |segs: &[Seg]| -> Vec<Op> {
            segs.iter()
                .map(|s| match *s {
                    Seg::Move(a, b) => Op::M(pt(a, b)),
                    Seg::Line(a, b) => Op::L(pt(a, b)),
                    Seg::Cubic(a, b, c, d, e, f) => Op::C(pt(a, b), pt(c, d), pt(e, f)),
                    Seg::Close => Op::Z,
                })
                .collect()
        };
        // The nested picture's own clip regions are not carried over: each item takes the
        // canvas's current clip.
        for item in &pic.items {
            if cv.full() {
                break;
            }
            let raw = match item {
                Item::Path {
                    segs,
                    fill,
                    stroke,
                    even_odd,
                    ..
                } => Raw::Path {
                    ops: ops(segs),
                    fill: *fill,
                    stroke: stroke.map(|(c, w)| (c, f64::from(w) * scale)),
                    even_odd: *even_odd,
                    region: None,
                },
                Item::Pattern {
                    segs,
                    even_odd,
                    tile,
                    ..
                } => Raw::Pattern {
                    ops: ops(segs),
                    even_odd: *even_odd,
                    tile: Arc::new(RawTile {
                        w: tile.width as usize,
                        h: tile.height as usize,
                        px: tile.rgba.clone(),
                        cell: f64::from(tile.cell[0]) * scale,
                        sampled: tile.sampled,
                    }),
                    region: None,
                },
                Item::Bitmap {
                    rect,
                    width,
                    height,
                    rgba,
                    ..
                } => {
                    let (a, b) = (
                        pt(rect[0], rect[1]),
                        pt(rect[0] + rect[2], rect[1] + rect[3]),
                    );
                    let mut img = Rgba {
                        w: *width as usize,
                        h: *height as usize,
                        bottom_up: false,
                        px: rgba.clone(),
                    };
                    crate::bitmap::flip(&mut img, a.0 > b.0, a.1 > b.1);
                    Raw::Bitmap {
                        rect: [
                            a.0.min(b.0),
                            a.1.min(b.1),
                            (b.0 - a.0).abs(),
                            (b.1 - a.1).abs(),
                        ],
                        img,
                        region: None,
                    }
                }
                Item::Text(t) => {
                    let mut t = t.clone();
                    t.transform = mul(t.transform, m);
                    if let Some([x0, y0, x1, y1]) = t.clip {
                        let (p, q) = (pt(x0, y0), pt(x1, y1));
                        t.clip = Some([
                            p.0.min(q.0) as f32,
                            p.1.min(q.1) as f32,
                            p.0.max(q.0) as f32,
                            p.1.max(q.1) as f32,
                        ]);
                    }
                    Raw::Text {
                        text: t,
                        region: None,
                    }
                }
            };
            cv.push_device(raw);
        }
        self.end(cv);
    }

    fn text(
        &self,
        cv: &mut Canvas,
        at: P,
        text: String,
        dx: Vec<f32>,
        style: TextStyle,
        pre: Xform,
    ) {
        self.begin(cv, pre);
        cv.set_text_style(style.color, style.font, style.align);
        let (opts, rect) = match style.clip {
            Some(c) => (ETO_CLIPPED, Some(c)),
            None => (0, None),
        };
        cv.text(at, text, dx, opts, rect);
        self.end(cv);
    }
}

fn apply(m: Xform, p: P) -> P {
    (
        p.0 * m[0] + p.1 * m[2] + m[4],
        p.0 * m[1] + p.1 * m[3] + m[5],
    )
}

fn invert(m: Xform) -> Option<Xform> {
    let det = m[0] * m[3] - m[1] * m[2];
    if !det.is_finite() || det.abs() < 1e-12 {
        return None;
    }
    let (a, b, c, d) = (m[3] / det, -m[1] / det, -m[2] / det, m[0] / det);
    Some([a, b, c, d, -(m[4] * a + m[5] * c), -(m[4] * b + m[5] * d)])
}

/// ARGB (0xAARRGGBB) to RGBA.
fn parse_object(kind: u16, record: Bytes, region_work: &mut usize) -> Option<Obj> {
    match kind {
        1 => brush(record).map(Obj::Brush),
        2 => plus_pen::read(record).map(|pen| Obj::Pen(Arc::new(pen))),
        3 => path(record).map(Obj::Path),
        4 => plus_region::parse(record, region_work).map(Obj::Region),
        5 => image(record).map(|image| Obj::Image(Arc::new(image))),
        6 => FontSpec::read(record).map(Obj::Font),
        7 => Format::read(record).map(Obj::Format),
        _ => None,
    }
}

fn rect_ops([l, t, r, b]: [f64; 4]) -> Vec<Op> {
    crate::geom::rect(l, t, r, b)
}
