//! ECMA-376 DrawingML diagrams, text transforms and Office Math; MS-OLEDS packages.
//! Native XML is authoritative. Our bounded extension retains editable layout/source hints.
use crate::{
    write::{Out, Rels, XML_DECL},
    xml::{El, esc, esc_attr},
};
use joysheet_model::{DrawingObject, MathExpr, Shape, ShapeFormat, SmartArtLayout};
use std::fmt::Write as _;
const META: &str = "urn:joyoffice:drawing-object:v1";
const DGM: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

pub fn metadata(shape: &Shape) -> String {
    metadata_with_spid(shape, None)
}
pub fn metadata_with_spid(shape: &Shape, spid: Option<u32>) -> String {
    let Some(object) = &shape.format.object else { return String::new() };
    let mut object = object.clone();
    if let DrawingObject::Embedded { data, .. } = &mut object {
        *data = std::sync::Arc::new(Vec::new());
    }
    let compat=spid.map(|id|format!("<a:ext uri=\"{{63B3BB69-23CF-44E3-9099-C40C66FF867C}}\"><a14:compatExt xmlns:a14=\"http://schemas.microsoft.com/office/drawing/2010/main\" spid=\"_x0000_s{id}\"/></a:ext>")).unwrap_or_default();
    match serde_json::to_string(&object) {
        Ok(json) => {
            format!("<a:extLst><a:ext uri=\"{META}\"><joy:object xmlns:joy=\"{META}\">{}</joy:object></a:ext>{compat}</a:extLst>", esc(&json))
        }
        Err(_) => String::new(),
    }
}
pub fn read_metadata(obj: &El) -> Option<DrawingObject> {
    let e = obj
        .children
        .iter()
        .find(|e| e.name.starts_with("nv"))?
        .child("cNvPr")?
        .child("extLst")?
        .kids("ext")
        .find(|e| e.attr("uri") == Some(META))?
        .child("object")?;
    if e.text.len() > 131072 {
        return None;
    }
    serde_json::from_str(&e.text).ok()
}

pub fn math_xml(e: &MathExpr) -> String {
    match e {
        MathExpr::Text(t) => format!("<m:r><m:rPr><m:sty m:val=\"p\"/></m:rPr><m:t>{}</m:t></m:r>", esc(t)),
        MathExpr::Row(v) => v.iter().map(math_xml).collect(),
        MathExpr::Fraction(a, b) => format!("<m:f><m:fPr/><m:num>{}</m:num><m:den>{}</m:den></m:f>", math_xml(a), math_xml(b)),
        MathExpr::Radical(a) => format!("<m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e>{}</m:e></m:rad>", math_xml(a)),
        MathExpr::Superscript(a, b) => format!("<m:sSup><m:e>{}</m:e><m:sup>{}</m:sup></m:sSup>", math_xml(a), math_xml(b)),
        MathExpr::Subscript(a, b) => format!("<m:sSub><m:e>{}</m:e><m:sub>{}</m:sub></m:sSub>", math_xml(a), math_xml(b)),
    }
}
pub fn equation_body(e: &MathExpr) -> String {
    format!(
        "<xdr:txBody><a:bodyPr/><a:lstStyle/><a:p><a:pPr algn=\"ctr\"/><a14:m xmlns:a14=\"http://schemas.microsoft.com/office/drawing/2010/main\"><m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\">{}</m:oMath></a14:m><a:endParaRPr lang=\"en-US\"/></a:p></xdr:txBody>",
        math_xml(e)
    )
}
fn math_read(e: &El, depth: usize, budget: &mut usize) -> Option<MathExpr> {
    if depth > 32 || *budget == 0 {
        return None;
    }
    *budget -= 1;
    let row = |e: &El, b: &mut usize| -> Option<MathExpr> {
        Some(MathExpr::Row(e.children.iter().filter(|c| !c.name.ends_with("Pr")).map(|c| math_read(c, depth + 1, b)).collect::<Option<Vec<_>>>()?))
    };
    Some(match e.name.as_str() {
        "r" => MathExpr::Text(e.child("t")?.text.clone()),
        "f" => MathExpr::Fraction(Box::new(row(e.child("num")?, budget)?), Box::new(row(e.child("den")?, budget)?)),
        "rad" => MathExpr::Radical(Box::new(row(e.child("e")?, budget)?)),
        "sSup" => MathExpr::Superscript(Box::new(row(e.child("e")?, budget)?), Box::new(row(e.child("sup")?, budget)?)),
        "sSub" => MathExpr::Subscript(Box::new(row(e.child("e")?, budget)?), Box::new(row(e.child("sub")?, budget)?)),
        "oMath" | "oMathPara" | "m" => return row(e, budget),
        _ => return None,
    })
}
pub fn read_equation(obj: &El) -> Option<MathExpr> {
    let math = obj.child("txBody")?.kids("p").find_map(|p| p.child("m"))?;
    math_read(math, 0, &mut 8192).map(MathExpr::normalized)
}

/// A complete semantic diagram graph with a layout definition and style/color definitions.
pub fn smart_art(shape: &Shape, drawing: u32, id: usize, out: &mut Out, rels: &mut Rels) -> Option<String> {
    let DrawingObject::SmartArt { layout, nodes } = shape.format.object.as_ref()? else { return None };
    let suffix = format!("{drawing}_{id}");
    let (layout_name, algorithm) = match layout {
        SmartArtLayout::Process => ("process", "lin"),
        SmartArtLayout::List => ("list", "lin"),
        SmartArtLayout::Hierarchy => ("hierarchy", "hierRoot"),
        SmartArtLayout::Cycle => ("cycle", "cycle"),
    };
    let layout_id = format!("urn:joyoffice:smartart:{layout_name}:v1");
    let mut data = format!(
        "{XML_DECL}<dgm:dataModel xmlns:dgm=\"{DGM}\" xmlns:a=\"{A}\"><dgm:ptLst><dgm:pt modelId=\"0\" type=\"doc\"><dgm:prSet loTypeId=\"{layout_id}\"/><dgm:spPr/><dgm:t><a:bodyPr/><a:lstStyle/><a:p/></dgm:t></dgm:pt>"
    );
    for (i, node) in nodes.iter().enumerate() {
        let _ = write!(
            data,
            "<dgm:pt modelId=\"{}\" type=\"node\"><dgm:prSet/><dgm:spPr/><dgm:t><a:bodyPr anchor=\"ctr\"/><a:lstStyle/><a:p><a:pPr algn=\"ctr\"/><a:r><a:rPr sz=\"1100\"/><a:t>{}</a:t></a:r></a:p></dgm:t></dgm:pt>",
            i + 1,
            esc(node)
        );
    }
    data.push_str("</dgm:ptLst><dgm:cxnLst>");
    for i in 0..nodes.len() {
        let parent = if *layout == SmartArtLayout::Hierarchy && i > 0 { 1 } else { 0 };
        let _ =
            write!(data, "<dgm:cxn modelId=\"{}\" type=\"parOf\" srcId=\"{parent}\" destId=\"{}\" srcOrd=\"{i}\" destOrd=\"0\"/>", i + 1000, i + 1);
    }
    data.push_str("</dgm:cxnLst><dgm:bg/><dgm:whole/></dgm:dataModel>");
    let dir = if *layout == SmartArtLayout::List { "fromT" } else { "fromL" };
    let layout_xml = format!(
        "{XML_DECL}<dgm:layoutDef xmlns:dgm=\"{DGM}\" xmlns:a=\"{A}\" uniqueId=\"{layout_id}\" minVer=\"{DGM}\"><dgm:title val=\"JoyOffice {layout_name}\"/><dgm:desc val=\"Editable diagram\"/><dgm:catLst/><dgm:layoutNode name=\"root\"><dgm:alg type=\"{algorithm}\"><dgm:param type=\"linDir\" val=\"{dir}\"/></dgm:alg><dgm:presOf axis=\"self\" ptType=\"doc\"/><dgm:forEach name=\"nodes\" axis=\"ch\" ptType=\"node\"><dgm:layoutNode name=\"node\" styleLbl=\"node\"><dgm:alg type=\"tx\"/><dgm:shape type=\"rect\"/><dgm:presOf axis=\"self\" ptType=\"node\"/><dgm:constrLst/><dgm:ruleLst/></dgm:layoutNode></dgm:forEach><dgm:constrLst><dgm:constr type=\"w\" for=\"ch\" forName=\"node\" refType=\"w\" fact=\"0.25\"/><dgm:constr type=\"h\" for=\"ch\" forName=\"node\" refType=\"h\" fact=\"0.4\"/><dgm:constr type=\"primFontSz\" for=\"ch\" forName=\"node\" op=\"equ\" val=\"11\"/></dgm:constrLst><dgm:ruleLst/></dgm:layoutNode></dgm:layoutDef>"
    );
    let style = format!(
        "{XML_DECL}<dgm:styleDef xmlns:dgm=\"{DGM}\" xmlns:a=\"{A}\" uniqueId=\"urn:joyoffice:smartart:style:v1\"><dgm:title val=\"Simple\"/><dgm:desc val=\"Simple\"/><dgm:catLst/><dgm:scene3d><a:camera prst=\"orthographicFront\"/><a:lightRig rig=\"threePt\" dir=\"t\"/></dgm:scene3d><dgm:styleLbl name=\"node\"><dgm:style><a:lnRef idx=\"0\"><a:schemeClr val=\"accent1\"/></a:lnRef><a:fillRef idx=\"1\"><a:schemeClr val=\"accent1\"/></a:fillRef><a:effectRef idx=\"0\"><a:schemeClr val=\"accent1\"/></a:effectRef><a:fontRef idx=\"minor\"><a:schemeClr val=\"lt1\"/></a:fontRef></dgm:style></dgm:styleLbl></dgm:styleDef>"
    );
    let colors = format!(
        "{XML_DECL}<dgm:colorsDef xmlns:dgm=\"{DGM}\" xmlns:a=\"{A}\" uniqueId=\"urn:joyoffice:smartart:colors:v1\"><dgm:title val=\"Accent 1\"/><dgm:desc val=\"Accent 1\"/><dgm:catLst/><dgm:styleLbl name=\"node\"><dgm:fillClrLst meth=\"repeat\"><a:schemeClr val=\"accent1\"/></dgm:fillClrLst><dgm:linClrLst meth=\"repeat\"><a:schemeClr val=\"accent1\"/></dgm:linClrLst><dgm:txFillClrLst meth=\"repeat\"><a:schemeClr val=\"lt1\"/></dgm:txFillClrLst></dgm:styleLbl></dgm:colorsDef>"
    );
    let mut ids = Vec::new();
    for (name, kind, ct, xml) in [
        ("data", "diagramData", "Data", data),
        ("layout", "diagramLayout", "Layout", layout_xml),
        ("quickStyle", "diagramQuickStyle", "Style", style),
        ("colors", "diagramColors", "Colors", colors),
    ] {
        let part = format!("xl/diagrams/{name}{suffix}.xml");
        out.part(&part, Some(&format!("application/vnd.openxmlformats-officedocument.drawingml.diagram{ct}+xml")), xml.into_bytes());
        ids.push(rels.add(kind, &format!("../diagrams/{name}{suffix}.xml")));
    }
    Some(format!(
        "<xdr:graphicFrame macro=\"\"><xdr:nvGraphicFramePr><xdr:cNvPr id=\"{id}\" name=\"SmartArt {id}\">{}</xdr:cNvPr><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr>{}<a:graphic><a:graphicData uri=\"{DGM}\"><dgm:relIds xmlns:dgm=\"{DGM}\" r:dm=\"{}\" r:lo=\"{}\" r:qs=\"{}\" r:cs=\"{}\"/></a:graphicData></a:graphic></xdr:graphicFrame>",
        metadata(shape),
        crate::drawing::xfrm(&shape.anchor, "xdr:xfrm"),
        ids[0],
        ids[1],
        ids[2],
        ids[3]
    ))
}

pub fn read_smart_art(cx: &mut crate::read::Ctx<'_>, obj: &El, part: &str, anchor: joysheet_model::Anchor) -> Result<Option<Shape>, crate::IoError> {
    let Some(ids) = obj.path(&["graphic", "graphicData", "relIds"]) else { return Ok(None) };
    let rels = cx.pkg.rels(part)?;
    let Some(rel) = ids.attr("dm").and_then(|id| rels.iter().find(|r| r.id == id && !r.external)) else { return Ok(None) };
    let target = rel.target.clone();
    let Some(data) = cx.optional_xml(&target)? else { return Ok(None) };
    let nodes: Vec<String> = data
        .child("ptLst")
        .into_iter()
        .flat_map(|l| l.kids("pt"))
        .filter(|p| p.attr("type").is_none_or(|t| t == "node"))
        .take(64)
        .map(|p| {
            p.child("t")
                .into_iter()
                .flat_map(|t| t.kids("p"))
                .flat_map(|p| p.kids("r"))
                .filter_map(|r| r.child("t"))
                .map(|t| t.text.as_str())
                .collect()
        })
        .collect();
    if nodes.is_empty() {
        return Ok(None);
    }
    let layout = match read_metadata(obj) {
        Some(DrawingObject::SmartArt { layout, .. }) => layout,
        _ => SmartArtLayout::Process,
    };
    let text = nodes.join("\n");
    let source = if read_metadata(obj).is_none() {
        let id = obj
            .path(&["nvGraphicFramePr", "cNvPr"])
            .and_then(|p| p.attr_u32("id"))
            .ok_or_else(|| crate::IoError::Format("SmartArt has no object id".into()))?;
        cx.warn("native SmartArt layout is shown with a basic preview; original diagram XML is retained and content is read-only");
        Some(crate::passthrough::drawing_source(cx, part, id)?)
    } else {
        None
    };
    Ok(Some(Shape {
        id: cx.next_object_id(),
        kind: joysheet_model::ShapeKind::TextBox,
        anchor,
        fill: joysheet_model::Color::Auto,
        line: joysheet_model::Color::Auto,
        text,
        format: ShapeFormat { no_line: true, object: Some(DrawingObject::SmartArt { layout, nodes }), source, ..Default::default() },
    }))
}

pub fn embedded_package(name: &str, data: &[u8]) -> Result<Vec<u8>, crate::IoError> {
    // Package native format: flags, display name, source path, reserved, temp path, payload.
    let ansi: Vec<u8> = name.chars().map(|c| if c.is_ascii() { c as u8 } else { b'?' }).collect();
    let mut native = Vec::new();
    native.extend_from_slice(&2u16.to_le_bytes());
    for _ in 0..2 {
        native.extend_from_slice(&ansi);
        native.push(0);
    }
    native.extend_from_slice(&[0, 0, 3, 0]);
    // Package's command/path is a length-prefixed ANSI string (including NUL),
    // unlike the preceding NUL-terminated label and source path.
    native.extend_from_slice(&((ansi.len() + 1) as u32).to_le_bytes());
    native.extend_from_slice(&ansi);
    native.push(0);
    native.extend_from_slice(&(data.len() as u32).to_le_bytes());
    native.extend_from_slice(data);
    // Package2's command, display label and source path use UTF-16 character
    // counts without a terminator. Flag 3 above requires this Unicode suffix.
    let unicode: Vec<u16> = name.encode_utf16().collect();
    for _ in 0..3 {
        native.extend_from_slice(&(unicode.len() as u32).to_le_bytes());
        for c in &unicode {
            native.extend_from_slice(&c.to_le_bytes());
        }
    }
    let mut stream = (native.len() as u32).to_le_bytes().to_vec();
    stream.extend_from_slice(&native);
    let mut cfb = joyoffice_cfb::CfbWriter::new();
    cfb.add_stream("\u{1}Ole10Native", stream);
    // MS-OLEDS 2.3.7/2.3.8: the OLE host needs class/clipboard metadata when
    // persisting the embedded package, in addition to its native payload.
    let package_clsid = [0x0c, 0, 3, 0, 0, 0, 0, 0, 0xc0, 0, 0, 0, 0, 0, 0, 0x46];
    let mut comp = Vec::new();
    comp.extend_from_slice(&0xfffe0001u32.to_le_bytes());
    comp.extend_from_slice(&0x00000a03u32.to_le_bytes());
    comp.extend_from_slice(&0xffffffffu32.to_le_bytes());
    comp.extend_from_slice(&package_clsid);
    comp.extend_from_slice(&12u32.to_le_bytes());
    comp.extend_from_slice(b"OLE Package\0");
    comp.extend_from_slice(&0u32.to_le_bytes()); // No registered clipboard format.
    comp.extend_from_slice(&8u32.to_le_bytes());
    comp.extend_from_slice(b"Package\0");
    comp.extend_from_slice(&0x71b239f4u32.to_le_bytes());
    for _ in 0..3 {
        comp.extend_from_slice(&0u32.to_le_bytes());
    }
    cfb.add_stream("\u{1}CompObj", comp);
    let mut bytes = cfb.build().map_err(|e| crate::IoError::Format(e.to_string()))?;
    // CLSID_Package in the root directory; sector address is stored in the CFB header.
    let sector = u32::from_le_bytes(bytes[48..52].try_into().map_err(|_| crate::IoError::Format("CFB header".into()))?) as usize;
    let offset = (sector + 1) * 512 + 80;
    if let Some(clsid) = bytes.get_mut(offset..offset + 16) {
        clsid.copy_from_slice(&package_clsid);
    }
    Ok(bytes)
}
pub fn unpack_embedded(bytes: &[u8]) -> Option<Vec<u8>> {
    let cfb = crate::cfb::Compound::open(bytes).ok()?;
    let s = cfb.root_stream("\u{1}Ole10Native").ok()??;
    if s.len() > 34 * 1024 * 1024 {
        return None;
    }
    let mut at = 6usize;
    for _ in 0..2 {
        at += s.get(at..)?.iter().position(|b| *b == 0)? + 1;
    }
    at += 4;
    let length = u32::from_le_bytes(s.get(at..at + 4)?.try_into().ok()?) as usize;
    if length > 0 && length <= 65536 && s.get(at + 4..at.checked_add(4 + length)?)?.last() == Some(&0) {
        at += 4 + length;
    } else {
        // Compatibility with the first JoySheet Package writer, which omitted
        // that length. Existing local files must retain their original payload.
        at += s.get(at..)?.iter().position(|b| *b == 0)? + 1;
    }
    let n = u32::from_le_bytes(s.get(at..at + 4)?.try_into().ok()?) as usize;
    at += 4;
    (n <= 32 * 1024 * 1024).then(|| s.get(at..at.checked_add(n)?).map(<[u8]>::to_vec)).flatten()
}

pub fn read_embedded(
    cx: &mut crate::read::Ctx<'_>,
    root: &El,
    rels: &[crate::package::Rel],
    sheet: &mut joysheet_model::Sheet,
) -> Result<(), crate::IoError> {
    for ole in crate::xml::office_children(root).into_iter().filter(|e| e.name == "oleObject") {
        let Some(id) = ole.attr_u32("shapeId") else { continue };
        let Some(shape) = sheet
            .shapes
            .iter_mut()
            .find(|s| s.format.package_shape_id == Some(id) && matches!(s.format.object, Some(DrawingObject::Embedded { .. })))
        else {
            continue;
        };
        let Some(rel) = ole.attr("id").and_then(|id| rels.iter().find(|r| r.id == id && !r.external)) else { continue };
        let bytes = cx.pkg.read_limited(&rel.target, 34 * 1024 * 1024)?;
        if let Some(data) = bytes.as_deref().and_then(unpack_embedded) {
            if let Some(DrawingObject::Embedded { data: d, .. }) = &mut shape.format.object {
                *d = std::sync::Arc::new(data);
            }
        } else {
            shape.format.object = None;
            cx.warn("embedded package could not be edited; original OLE content retained");
        }
    }
    Ok(())
}

pub const OLE_PREVIEW: &[u8] = include_bytes!("../assets/embedded-file.png");
// Public ECMA-376 VML picture geometry. A rectangle masquerading as type 75
// is converted to a custom shape by Excel and clips the image on reopening.
pub const OLE_SHAPE_TYPE: &str = include_str!("../assets/ole-picture-type.xml");

pub fn ole_properties(sheet: &joysheet_model::Sheet, shape: &Shape, image_id: &str) -> String {
    let points = crate::drawing::anchor_points(sheet, &shape.anchor).replace("xdr:from", "from").replace("xdr:to", "to");
    let flags = match shape.anchor.mode {
        joysheet_model::AnchorMode::Absolute => "",
        joysheet_model::AnchorMode::MoveOnly => " moveWithCells=\"1\"",
        joysheet_model::AnchorMode::MoveAndSize => " moveWithCells=\"1\" sizeWithCells=\"1\"",
    };
    format!(
        "<objectPr defaultSize=\"0\" r:id=\"{image_id}\"><anchor{flags} xmlns:xdr=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\">{points}</anchor></objectPr>"
    )
}

pub fn ole_vml(sheet: &joysheet_model::Sheet, shape: &Shape, id: u32, image_id: &str) -> String {
    let a = &shape.anchor;
    let x = sheet.col_left(a.cell.col) + a.dx as f64;
    let y = sheet.row_top(a.cell.row) + a.dy as f64;
    let (c, r) = (sheet.col_at(x + a.width as f64), sheet.row_at(y + a.height as f64));
    format!(
        "<v:shape id=\"_x0000_s{id}\" type=\"#_x0000_t75\" style=\"position:absolute;margin-left:{}pt;margin-top:{}pt;width:{}pt;height:{}pt;z-index:1\" filled=\"f\" stroked=\"f\"><v:imagedata o:relid=\"{image_id}\" o:title=\"{}\"/><v:textbox inset=\"0,{}pt,0,0\"><div style=\"text-align:center;font-size:9pt\">{}</div></v:textbox><x:ClientData ObjectType=\"Pict\"><x:MoveWithCells/><x:Anchor>{},{},{},{},{},{},{},{}</x:Anchor><x:CF>Pict</x:CF><x:AutoPict/></x:ClientData></v:shape>",
        x * 0.75,
        y * 0.75,
        a.width * 0.75,
        a.height * 0.75,
        esc_attr(&shape.text),
        (a.height - 22.).max(0.) * 0.75,
        esc(&shape.text),
        a.cell.col,
        a.dx.round() as i32,
        a.cell.row,
        a.dy.round() as i32,
        c,
        (x + a.width as f64 - sheet.col_left(c)).round() as i32,
        r,
        (y + a.height as f64 - sheet.row_top(r)).round() as i32
    )
}

pub fn owned_ole_ids(sheet: &joysheet_model::Sheet) -> std::collections::HashSet<u32> {
    sheet
        .shapes
        .iter()
        .filter(|s| matches!(s.format.object, Some(DrawingObject::Embedded { .. })))
        .filter_map(|s| s.format.package_shape_id)
        .collect()
}

pub fn strip_owned_ole(xml: &str, ids: &std::collections::HashSet<u32>) -> String {
    use quick_xml::{Reader, events::Event};
    let mut reader = Reader::from_str(xml);
    let mut cuts = Vec::new();
    let mut open = None;
    let mut depth = 0usize;
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                depth += 1;
                if e.local_name().as_ref() == b"oleObject"
                    && e.attributes().flatten().any(|a| {
                        a.key.as_ref() == b"shapeId"
                            && std::str::from_utf8(&a.value).ok().and_then(|s| s.parse::<u32>().ok()).is_some_and(|id| ids.contains(&id))
                    })
                {
                    open = Some((start, depth));
                }
            }
            Ok(Event::Empty(e)) if e.local_name().as_ref() == b"oleObject" => {
                if e.attributes().flatten().any(|a| {
                    a.key.as_ref() == b"shapeId"
                        && std::str::from_utf8(&a.value).ok().and_then(|s| s.parse::<u32>().ok()).is_some_and(|id| ids.contains(&id))
                }) {
                    cuts.push((start, reader.buffer_position() as usize));
                }
            }
            Ok(Event::End(_)) => {
                if let Some((start, d)) = open
                    && d == depth
                {
                    cuts.push((start, reader.buffer_position() as usize));
                    open = None;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let mut xml = xml.to_owned();
    for (a, b) in cuts.into_iter().rev() {
        xml.replace_range(a..b, "");
    }
    xml
}

/// Remove only successfully modelled OLE entries from pass-through; unrelated native objects
/// and their original namespace declarations remain byte-for-byte intact.
pub fn consume_owned_ole(sheet: &mut joysheet_model::Sheet) {
    let ids = owned_ole_ids(sheet);
    if let Some(p) = sheet.preserved.as_mut() {
        for fragment in std::sync::Arc::make_mut(p).elements.iter_mut().filter(|e| e.slot == "oleObjects") {
            fragment.xml = strip_owned_ole(&fragment.xml, &ids);
        }
    }
    for shape in &mut sheet.shapes {
        shape.format.package_shape_id = None;
    }
}

pub fn merge_ole_objects(kept: &str, extra: &str) -> String {
    use quick_xml::{Reader, Writer, events::Event};
    if extra.is_empty() {
        return kept.into();
    }
    let mut reader = Reader::from_str(kept);
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::End(e)) if e.local_name().as_ref() == b"oleObjects" => {
                let mut out = kept.to_owned();
                out.insert_str(start, extra);
                return out;
            }
            Ok(Event::Empty(e)) if e.local_name().as_ref() == b"oleObjects" => {
                let mut writer = Writer::new(Vec::new());
                let end = e.to_end().into_owned();
                if writer.write_event(Event::Start(e)).is_ok() {
                    writer.get_mut().extend_from_slice(extra.as_bytes());
                    if writer.write_event(Event::End(end)).is_ok()
                        && let Ok(xml) = String::from_utf8(writer.into_inner())
                    {
                        let mut out = kept.to_owned();
                        out.replace_range(start..reader.buffer_position() as usize, &xml);
                        return out;
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    format!("{kept}<oleObjects>{extra}</oleObjects>")
}
