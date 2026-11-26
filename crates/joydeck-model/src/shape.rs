//! Shape records and the bounded shape tree used by editing, inspection and ID allocation.
//!
//! The serialized DrawingML-facing fields retain the original DeckCraft model's wire format.

use serde::{Deserialize, Serialize};

use crate::{
    Chart, Diagram, Effects, Fill, InkStroke, Line, MediaClip, MediaId, ShapeId, ShapeStyle, Table,
    TextBody, Xfrm, style, text,
};

/// Placeholder types.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhType {
    Title,
    CtrTitle,
    SubTitle,
    #[default]
    Body,
    Obj,
    Chart,
    Table,
    ClipArt,
    Diagram,
    Media,
    Picture,
    Date,
    Footer,
    SlideNum,
    Header,
    SlideImage,
}

impl PhType {
    pub fn xml(self) -> &'static str {
        match self {
            PhType::Title => "title",
            PhType::CtrTitle => "ctrTitle",
            PhType::SubTitle => "subTitle",
            PhType::Body => "body",
            PhType::Obj => "obj",
            PhType::Chart => "chart",
            PhType::Table => "tbl",
            PhType::ClipArt => "clipArt",
            PhType::Diagram => "dgm",
            PhType::Media => "media",
            PhType::Picture => "pic",
            PhType::Date => "dt",
            PhType::Footer => "ftr",
            PhType::SlideNum => "sldNum",
            PhType::Header => "hdr",
            PhType::SlideImage => "sldImg",
        }
    }
    pub fn from_xml(s: &str) -> Self {
        match s {
            "title" => PhType::Title,
            "ctrTitle" => PhType::CtrTitle,
            "subTitle" => PhType::SubTitle,
            "obj" => PhType::Obj,
            "chart" => PhType::Chart,
            "tbl" => PhType::Table,
            "clipArt" => PhType::ClipArt,
            "dgm" => PhType::Diagram,
            "media" => PhType::Media,
            "pic" => PhType::Picture,
            "dt" => PhType::Date,
            "ftr" => PhType::Footer,
            "sldNum" => PhType::SlideNum,
            "hdr" => PhType::Header,
            "sldImg" => PhType::SlideImage,
            _ => PhType::Body,
        }
    }
    pub fn is_title(self) -> bool {
        matches!(self, PhType::Title | PhType::CtrTitle)
    }
    /// The prompt shown in an empty placeholder in Normal view.
    pub fn prompt(self) -> &'static str {
        match self {
            PhType::Title | PhType::CtrTitle => "Click to add title",
            PhType::SubTitle => "Click to add subtitle",
            PhType::Body | PhType::Obj => "Click to add text",
            PhType::Chart => "Click icon to add chart",
            PhType::Table => "Click icon to add table",
            PhType::Diagram => "Click icon to add SmartArt graphic",
            PhType::Media => "Click icon to add media",
            PhType::Picture | PhType::ClipArt => "Click icon to add picture",
            PhType::Date => "Date",
            PhType::Footer => "Footer",
            PhType::SlideNum => "‹#›",
            PhType::Header => "Header",
            PhType::SlideImage => "",
        }
    }
    /// Footer-row placeholders are not shown on slides unless the deck turns them on.
    pub fn is_footer_kind(self) -> bool {
        matches!(
            self,
            PhType::Date | PhType::Footer | PhType::SlideNum | PhType::Header
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Placeholder {
    pub kind: PhType,
    pub idx: u32,
    /// `vert` orientation flag.
    pub vertical: bool,
    /// `quarter`, `half`, `full` size hint.
    pub size: Option<String>,
    pub has_custom_prompt: bool,
}

/// Shape geometry: a preset name with adjust values, or a custom path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Geom {
    Preset {
        name: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        adj: Vec<f64>,
    },
    /// Custom geometry: sub-paths in their own coordinate space (w, h) scaled to the shape.
    Custom { paths: Vec<CustomPath> },
}

impl Default for Geom {
    fn default() -> Self {
        Self::preset("rect")
    }
}

impl Geom {
    pub fn preset(name: &str) -> Self {
        Self::Preset {
            name: name.into(),
            adj: Vec::new(),
        }
    }
    /// Whether `name` is a supported ECMA-376 preset geometry in `joydeck-geom`.
    pub fn is_supported_preset(name: &str) -> bool {
        name != "textBox" && joydeck_geom::preset::info(name).is_some()
    }
    /// Whether `name` is an open/line-only preset that needs a visible stroke.
    pub fn is_open_preset(name: &str) -> bool {
        joydeck_geom::preset::is_line_like(name)
            || joydeck_geom::preset::build(name, 100.0, 100.0, &[]).is_some_and(|g| g.is_open())
    }
    pub fn set_callout_vertex(&mut self, vertex: u8, x: i32, y: i32) -> bool {
        let Self::Preset { name, adj } = self else {
            return false;
        };
        joydeck_geom::preset::LineCallout::named(name)
            .is_some_and(|callout| callout.set_vertex(adj, vertex, x, y))
    }

    pub fn preset_name(&self) -> Option<&str> {
        match self {
            Geom::Preset { name, .. } => Some(name),
            Geom::Custom { .. } => None,
        }
    }
}

/// One sub-path of a custom geometry: SVG-like commands in path units.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomPath {
    pub w: f64,
    pub h: f64,
    /// `M x y`, `L x y`, `C x1 y1 x2 y2 x y`, `Q x1 y1 x y`, `Z`.
    pub d: String,
    pub fill: joydeck_geom::preset::FillMode,
    pub stroke: bool,
}

impl Default for CustomPath {
    fn default() -> Self {
        CustomPath {
            w: 0.0,
            h: 0.0,
            d: String::new(),
            fill: joydeck_geom::preset::FillMode::Norm,
            stroke: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ShapeKind {
    /// An autoshape or text box.
    #[default]
    Shape,
    Picture {
        fill: style::PictureFill,
    },
    Group {
        children: Vec<Shape>,
        /// Child coordinate space.
        child: Xfrm,
    },
    Connector {
        start: Option<(ShapeId, u32)>,
        end: Option<(ShapeId, u32)>,
    },
    Table(Table),
    Chart(Box<Chart>),
    Media(MediaClip),
    Ink {
        strokes: Vec<InkStroke>,
    },
    /// A 3-D model (`p:contentPart` with a `model3d` relationship, usually a `.glb`).
    /// `poster` is the fallback picture an `mc:AlternateContent` choice carried for older readers.
    Model3d {
        media: MediaId,
        #[serde(default)]
        poster: Option<MediaId>,
    },
    /// Something we read but don't model (SmartArt without fallback, OLE…): its XML is kept and its
    /// fallback picture, if any, is shown.
    Opaque {
        xml: String,
        preview: Option<MediaId>,
        label: String,
        /// The parts its XML points to (SmartArt data, an OLE embedding…), so a save writes them
        /// back: (relationship id used in `xml`, relationship type, the part as a media item).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        parts: Vec<(String, String, MediaId)>,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Shape {
    pub id: ShapeId,
    pub name: String,
    /// `None`: placeholder inheriting its box from the layout/master.
    pub xfrm: Option<Xfrm>,
    pub kind: ShapeKind,
    pub geom: Geom,
    pub ph: Option<Placeholder>,
    /// `None`: inherited (placeholder chain or style); `Some(Fill::None)`: no fill.
    pub fill: Option<Fill>,
    pub line: Option<Line>,
    pub effects: Option<Effects>,
    pub style: Option<ShapeStyle>,
    pub text: Option<TextBody>,
    pub hidden: bool,
    pub locked: bool,
    /// A DrawingML text box (`txBox="1"`).
    pub text_box: bool,
    /// Alt text.
    pub descr: String,
    pub title: String,
    pub decorative: bool,
    pub click: Option<text::Hyperlink>,
    pub hover: Option<text::Hyperlink>,
    /// Add-in tags (`p:custDataLst` → `ppt/tags/tagN.xml`): name/value pairs kept for round trip.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<(String, String)>,
    /// Unmodelled XML kept for round-trip (extension lists etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_ext: Option<String>,
    /// On a SmartArt group: the diagram's text outline, from which its shapes are regenerated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagram: Option<Diagram>,
}

impl Shape {
    /// A stand-in shape for laying out one table cell's `body`: a table, so the cell's text takes
    /// the master's other-text style as PowerPoint's does.
    pub fn table_cell(body: text::TextBody) -> Shape {
        Shape {
            text: Some(body),
            kind: ShapeKind::Table(Table::default()),
            ..Default::default()
        }
    }
    pub fn is_placeholder(&self) -> bool {
        self.ph.is_some()
    }
    pub fn ph_type(&self) -> Option<PhType> {
        self.ph.as_ref().map(|p| p.kind)
    }
    pub fn is_group(&self) -> bool {
        self.group_members().is_some()
    }
    pub fn children(&self) -> &[Shape] {
        self.group_members().unwrap_or_default()
    }

    fn group_members(&self) -> Option<&[Shape]> {
        if let ShapeKind::Group { children, .. } = &self.kind {
            Some(children)
        } else {
            None
        }
    }
    pub fn children_mut(&mut self) -> Option<&mut Vec<Shape>> {
        match &mut self.kind {
            ShapeKind::Group { children, .. } => Some(children),
            _ => None,
        }
    }
    /// Switch autoshapes to an editable preset; stale guides from the old geometry are discarded.
    pub fn set_basic_preset(&mut self, name: &str) -> bool {
        if !matches!(self.kind, ShapeKind::Shape) || !Geom::is_supported_preset(name) {
            return false;
        }
        self.geom = Geom::preset(name);
        if Geom::is_open_preset(name) && self.line.is_none() {
            let color = match &self.fill {
                Some(Fill::Solid { color }) => color.clone(),
                _ => style::ColorRef::scheme(joydeck_color::SchemeSlot::Accent1),
            };
            self.line = Some(Line::solid(color, 2.0));
        }
        true
    }

    /// Change a line callout guide on an autoshape.
    pub fn set_callout_vertex(&mut self, vertex: u8, x: i32, y: i32) -> bool {
        matches!(self.kind, ShapeKind::Shape) && self.geom.set_callout_vertex(vertex, x, y)
    }

    /// Is this a line-like shape (no area)?
    pub fn is_line(&self) -> bool {
        matches!(self.kind, ShapeKind::Connector { .. })
            || self
                .geom
                .preset_name()
                .is_some_and(joydeck_geom::preset::is_line_like)
    }
    pub fn kind_name(&self) -> &'static str {
        match &self.kind {
            ShapeKind::Shape if self.ph.is_some() => "placeholder",
            ShapeKind::Shape if self.text_box => "textBox",
            ShapeKind::Shape => "shape",
            ShapeKind::Picture { .. } => "picture",
            ShapeKind::Group { .. } => "group",
            ShapeKind::Connector { .. } => "connector",
            ShapeKind::Table(_) => "table",
            ShapeKind::Chart(_) => "chart",
            ShapeKind::Media(m) if m.video => "video",
            ShapeKind::Media(_) => "audio",
            ShapeKind::Ink { .. } => "ink",
            ShapeKind::Model3d { .. } => "model",
            ShapeKind::Opaque { .. } => "object",
        }
    }
}

// Native documents and older migration inputs can contain deeper trees than the PPTX importer.
const MAX_DEPTH: usize = 64;

fn visit<'a, T>(
    shapes: &'a [Shape],
    parent: Option<ShapeId>,
    depth: usize,
    accept: &mut impl FnMut(&'a Shape, Option<ShapeId>, usize) -> Option<T>,
) -> Option<T> {
    for shape in shapes {
        if let Some(found) = accept(shape, parent, depth) {
            return Some(found);
        }
        if depth < MAX_DEPTH
            && let Some(children) = shape.group_members()
            && let Some(found) = visit(children, Some(shape.id), depth + 1, accept)
        {
            return Some(found);
        }
    }
    None
}

/// Walk the shape tree in drawing order, visiting each group before its children.
pub fn walk<'a>(shapes: &'a [Shape], f: &mut dyn FnMut(&'a Shape, usize)) {
    visit::<()>(shapes, None, 0, &mut |shape, _, depth| {
        f(shape, depth);
        None
    });
}

pub fn find_shape(shapes: &[Shape], id: ShapeId) -> Option<&Shape> {
    visit(shapes, None, 0, &mut |shape, _, _| {
        (shape.id == id).then_some(shape)
    })
}

pub fn find_shape_mut(shapes: &mut [Shape], id: ShapeId) -> Option<&mut Shape> {
    fn find(shapes: &mut [Shape], id: ShapeId, budget: usize) -> Option<&mut Shape> {
        for shape in shapes {
            if shape.id == id {
                return Some(shape);
            }
            if budget > 0
                && let Some(children) = shape.children_mut()
                && let Some(found) = find(children, id, budget - 1)
            {
                return Some(found);
            }
        }
        None
    }
    find(shapes, id, MAX_DEPTH)
}

/// The drawing-order list that directly contains `id`.
pub fn sibling_list_mut(shapes: &mut Vec<Shape>, id: ShapeId) -> Option<&mut Vec<Shape>> {
    fn find(shapes: &mut Vec<Shape>, id: ShapeId, budget: usize) -> Option<&mut Vec<Shape>> {
        if shapes.iter().any(|shape| shape.id == id) {
            return Some(shapes);
        }
        if budget == 0 {
            return None;
        }
        shapes
            .iter_mut()
            .filter_map(Shape::children_mut)
            .find_map(|children| find(children, id, budget - 1))
    }
    find(shapes, id, MAX_DEPTH)
}

/// `Some(None)` identifies a top-level shape and `None` identifies an absent shape.
pub fn parent_of(shapes: &[Shape], id: ShapeId) -> Option<Option<ShapeId>> {
    visit(shapes, None, 0, &mut |shape, parent, _| {
        (shape.id == id).then_some(parent)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_lookup_order_and_depth_agree_across_read_and_edit() {
        let shape = |id| Shape {
            id: ShapeId(id),
            ..Shape::default()
        };
        let mut tree = shape(MAX_DEPTH as u32 + 1);
        for id in (0..=MAX_DEPTH as u32).rev() {
            tree = Shape {
                kind: ShapeKind::Group {
                    children: vec![tree],
                    child: Xfrm::new(0.0, 0.0, 10.0, 10.0),
                },
                ..shape(id)
            };
        }
        let mut shapes = vec![tree, shape(100)];
        let mut drawing_order = Vec::new();
        walk(&shapes, &mut |node, depth| {
            drawing_order.push((node.id.0, depth))
        });
        let mut expected: Vec<_> = (0..=MAX_DEPTH).map(|id| (id as u32, id)).collect();
        expected.push((100, 0));
        assert_eq!(drawing_order, expected);
        let last = ShapeId(MAX_DEPTH as u32);
        let hidden = ShapeId(last.0 + 1);
        assert_eq!(parent_of(&shapes, last), Some(Some(ShapeId(last.0 - 1))));
        assert_eq!(parent_of(&shapes, ShapeId(100)), Some(None));
        assert!(parent_of(&shapes, hidden).is_none());
        assert!(find_shape(&shapes, hidden).is_none());
        assert!(find_shape_mut(&mut shapes, hidden).is_none());
        assert!(sibling_list_mut(&mut shapes, hidden).is_none());
        assert_eq!(
            find_shape_mut(&mut shapes, last).map(|node| node.id),
            Some(last)
        );
        assert_eq!(
            sibling_list_mut(&mut shapes, last).map(|nodes| nodes.len()),
            Some(1)
        );
    }

    #[test]
    fn preset_edit_replaces_guides_without_losing_identity_appearance_or_extension() {
        let mut shape = Shape {
            id: ShapeId(7),
            geom: Geom::Preset {
                name: "roundRect".into(),
                adj: vec![20000.0],
            },
            fill: Some(Fill::None),
            raw_ext: Some("<p:extLst/>".into()),
            ..Default::default()
        };
        let before = shape.clone();
        assert!(!shape.set_basic_preset("custom-invalid"));
        assert_eq!(shape, before);
        assert!(shape.set_basic_preset("ellipse"));
        assert_eq!(
            shape,
            Shape {
                geom: Geom::preset("ellipse"),
                ..before
            }
        );
    }
}
