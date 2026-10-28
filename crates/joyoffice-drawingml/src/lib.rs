//! DrawingML shapes, text boxes, and geometries (ECMA-376 Part 1 §20).

use quick_xml::events::Event;
use quick_xml::reader::Reader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawingBudget {
    pub max_shapes: usize,
    pub max_depth: usize,
}

impl Default for DrawingBudget {
    fn default() -> Self {
        Self {
            max_shapes: 256,
            max_depth: 32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrawingError {
    Xml(String),
    BudgetExceeded(String),
    Malformed(String),
}

impl std::fmt::Display for DrawingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Xml(e) => write!(f, "XML error: {e}"),
            Self::BudgetExceeded(e) => write!(f, "DrawingML budget exceeded: {e}"),
            Self::Malformed(e) => write!(f, "Malformed DrawingML: {e}"),
        }
    }
}

impl std::error::Error for DrawingError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeGeometry {
    Rectangle,
    RoundRectangle,
    Ellipse,
    Line,
    TextBox,
    Custom(String),
}

impl ShapeGeometry {
    pub fn from_prst(name: &str) -> Self {
        match name {
            "rect" => Self::Rectangle,
            "roundRect" => Self::RoundRectangle,
            "ellipse" => Self::Ellipse,
            "line" => Self::Line,
            "textBox" => Self::TextBox,
            other => Self::Custom(other.to_string()),
        }
    }

    pub fn to_prst(&self) -> &str {
        match self {
            Self::Rectangle => "rect",
            Self::RoundRectangle => "roundRect",
            Self::Ellipse => "ellipse",
            Self::Line => "line",
            Self::TextBox => "rect", // text boxes standardly use rect geometry
            Self::Custom(s) => s.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fill {
    None,
    Solid(String), // Hex RGB, e.g. "4F81BD"
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub color: Option<String>,
    pub width_pt: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawingShape {
    pub id: u32,
    pub name: String,
    pub geometry: ShapeGeometry,
    pub width_pt: f64,
    pub height_pt: f64,
    pub fill: Fill,
    pub stroke: Stroke,
    pub textbox_content: Option<String>,
}

impl DrawingShape {
    /// Render shape as OpenXML `<w:drawing>` XML string.
    pub fn to_drawing_xml(&self) -> String {
        let cx_emu = (self.width_pt * 12700.0) as u64;
        let cy_emu = (self.height_pt * 12700.0) as u64;
        let prst = self.geometry.to_prst();

        let mut out = format!(
            concat!(
                r#"<w:drawing xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" "#,
                r#"xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" "#,
                r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
                r#"xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">"#,
                r#"<wp:inline distT="0" distB="0" distL="0" distR="0">"#,
                r#"<wp:extent cx="{}" cy="{}"/>"#,
                r#"<wp:docPr id="{}" name="{}"/>"#,
                r#"<a:graphic>"#,
                r#"<a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">"#,
                r#"<wps:wsp>"#,
                r#"<wps:spPr>"#,
                r#"<a:prstGeom prst="{}"><a:avLst/></a:prstGeom>"#,
            ),
            cx_emu,
            cy_emu,
            self.id,
            quick_xml::escape::escape(&self.name),
            prst
        );

        match &self.fill {
            Fill::None => out.push_str("<a:noFill/>"),
            Fill::Solid(hex) => {
                out.push_str(&format!(
                    "<a:solidFill><a:srgbClr val=\"{hex}\"/></a:solidFill>"
                ));
            }
        }

        if let Some(color) = &self.stroke.color {
            let width_emu = (self.stroke.width_pt.unwrap_or(1.0) * 12700.0) as u64;
            out.push_str(&format!(
                r#"<a:ln w="{}"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:ln>"#,
                width_emu, color
            ));
        }

        out.push_str("</wps:spPr>");

        if let Some(tb) = &self.textbox_content {
            out.push_str("<wps:txbx><w:txbxContent>");
            out.push_str(tb);
            out.push_str("</w:txbxContent></wps:txbx>");
        }

        out.push_str(concat!(
            "</wps:wsp>",
            "</a:graphicData>",
            "</a:graphic>",
            "</wp:inline>",
            "</w:drawing>"
        ));

        out
    }
}

/// Parse a `<w:drawing>` XML string to extract shapes and textboxes.
pub fn parse_drawing_xml(
    xml: &str,
    budget: &DrawingBudget,
) -> Result<Vec<DrawingShape>, DrawingError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut shapes = Vec::new();
    let mut depth = 0;

    let mut current_id: u32 = 1;
    let mut current_name = "Shape".to_string();
    let mut current_width_pt = 100.0;
    let mut current_height_pt = 100.0;
    let mut current_geom = ShapeGeometry::Rectangle;
    let mut current_fill = Fill::None;
    let mut current_stroke = Stroke {
        color: None,
        width_pt: None,
    };
    let mut current_txbx = None;
    let mut in_ln = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                if depth > budget.max_depth {
                    return Err(DrawingError::BudgetExceeded("max depth exceeded".into()));
                }

                let e_name = e.name();
                let local = local_name(e_name.as_ref());
                match local {
                    b"ln" => {
                        in_ln = true;
                    }
                    b"extent" => {
                        parse_extent(e, &mut current_width_pt, &mut current_height_pt);
                    }
                    b"docPr" => {
                        parse_doc_pr(e, &mut current_id, &mut current_name);
                    }
                    b"prstGeom" => {
                        parse_prst_geom(e, &mut current_geom);
                    }
                    b"srgbClr" => {
                        if in_ln {
                            parse_srgb_clr_stroke(e, &mut current_stroke);
                        } else {
                            parse_srgb_clr(e, &mut current_fill);
                        }
                    }
                    b"txbxContent" => {
                        let end_name = e.name().as_ref().to_vec();
                        let mut txbx_buf = String::new();
                        loop {
                            match reader.read_event() {
                                Ok(Event::Text(ref t)) => {
                                    let s = std::str::from_utf8(t.as_ref())
                                        .map_err(|e| DrawingError::Xml(e.to_string()))?;
                                    txbx_buf.push_str(s);
                                }
                                Ok(Event::End(ref end_e)) if end_e.name().as_ref() == end_name => {
                                    break;
                                }
                                Ok(Event::Eof) => break,
                                Err(err) => return Err(DrawingError::Xml(err.to_string())),
                                _ => {}
                            }
                        }
                        current_txbx = Some(txbx_buf);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let e_name = e.name();
                let local = local_name(e_name.as_ref());
                match local {
                    b"extent" => {
                        parse_extent(e, &mut current_width_pt, &mut current_height_pt);
                    }
                    b"docPr" => {
                        parse_doc_pr(e, &mut current_id, &mut current_name);
                    }
                    b"prstGeom" => {
                        parse_prst_geom(e, &mut current_geom);
                    }
                    b"srgbClr" => {
                        if in_ln {
                            parse_srgb_clr_stroke(e, &mut current_stroke);
                        } else {
                            parse_srgb_clr(e, &mut current_fill);
                        }
                    }
                    b"wsp" => {
                        shapes.push(DrawingShape {
                            id: current_id,
                            name: current_name.clone(),
                            geometry: current_geom.clone(),
                            width_pt: current_width_pt,
                            height_pt: current_height_pt,
                            fill: current_fill.clone(),
                            stroke: current_stroke.clone(),
                            textbox_content: current_txbx.take(),
                        });
                        if shapes.len() > budget.max_shapes {
                            return Err(DrawingError::BudgetExceeded("max shapes exceeded".into()));
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                depth = depth.saturating_sub(1);
                let e_name = e.name();
                let local = local_name(e_name.as_ref());
                if local == b"ln" {
                    in_ln = false;
                }
                if local == b"wsp" {
                    shapes.push(DrawingShape {
                        id: current_id,
                        name: current_name.clone(),
                        geometry: current_geom.clone(),
                        width_pt: current_width_pt,
                        height_pt: current_height_pt,
                        fill: current_fill.clone(),
                        stroke: current_stroke.clone(),
                        textbox_content: current_txbx.take(),
                    });
                    if shapes.len() > budget.max_shapes {
                        return Err(DrawingError::BudgetExceeded("max shapes exceeded".into()));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DrawingError::Xml(e.to_string())),
            _ => {}
        }
    }

    Ok(shapes)
}

fn parse_extent(e: &quick_xml::events::BytesStart<'_>, w: &mut f64, h: &mut f64) {
    for attr in e.attributes().flatten() {
        let k = local_name(attr.key.as_ref());
        if k == b"cx"
            && let Ok(val) = std::str::from_utf8(&attr.value)
            && let Ok(emu) = val.parse::<u64>()
        {
            *w = emu as f64 / 12700.0;
        } else if k == b"cy"
            && let Ok(val) = std::str::from_utf8(&attr.value)
            && let Ok(emu) = val.parse::<u64>()
        {
            *h = emu as f64 / 12700.0;
        }
    }
}

fn parse_doc_pr(e: &quick_xml::events::BytesStart<'_>, id: &mut u32, name: &mut String) {
    for attr in e.attributes().flatten() {
        let k = local_name(attr.key.as_ref());
        if k == b"id"
            && let Ok(val) = std::str::from_utf8(&attr.value)
            && let Ok(parsed_id) = val.parse::<u32>()
        {
            *id = parsed_id;
        } else if k == b"name"
            && let Ok(val) = std::str::from_utf8(&attr.value)
        {
            *name = val.to_string();
        }
    }
}

fn parse_prst_geom(e: &quick_xml::events::BytesStart<'_>, geom: &mut ShapeGeometry) {
    for attr in e.attributes().flatten() {
        if local_name(attr.key.as_ref()) == b"prst"
            && let Ok(val) = std::str::from_utf8(&attr.value)
        {
            *geom = ShapeGeometry::from_prst(val);
        }
    }
}

fn parse_srgb_clr(e: &quick_xml::events::BytesStart<'_>, fill: &mut Fill) {
    for attr in e.attributes().flatten() {
        if local_name(attr.key.as_ref()) == b"val"
            && let Ok(val) = std::str::from_utf8(&attr.value)
        {
            *fill = Fill::Solid(val.to_string());
        }
    }
}

fn parse_srgb_clr_stroke(e: &quick_xml::events::BytesStart<'_>, stroke: &mut Stroke) {
    for attr in e.attributes().flatten() {
        if local_name(attr.key.as_ref()) == b"val"
            && let Ok(val) = std::str::from_utf8(&attr.value)
        {
            stroke.color = Some(val.to_string());
        }
    }
}

fn local_name(qname: &[u8]) -> &[u8] {
    if let Some(idx) = qname.iter().position(|&b| b == b':') {
        &qname[idx + 1..]
    } else {
        qname
    }
}
