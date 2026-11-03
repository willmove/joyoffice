//! Restricted R2 writer. Unchanged ZIP members are copied byte-for-byte (uncompressed).
//! Existing shapes retain their original XML except edited transform/text and cNvPr identity.
use crate::{
    PptxError, opc,
    xml::{El, Node},
};
use joydeck_model::{Presentation, ShapeId, SlideId};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Write},
};
#[path = "preserve_content.rs"]
mod content;
pub struct SlideBinding {
    pub slide: SlideId,
    pub source: SlideId,
    pub shapes: BTreeMap<ShapeId, ShapeId>,
}
fn error(s: impl Into<String>) -> PptxError {
    PptxError::Write(s.into())
}
fn relationship_root(pkg: &opc::Package, part: &str) -> Result<El, PptxError> {
    let path = opc::rels_path(part);
    if pkg.get(&path).is_some() {
        return pkg
            .xml(&path)
            .map(|doc| doc.root)
            .ok_or_else(|| error("unreadable slide relationships"));
    }
    // A slide without resources/layout/notes legitimately has no relationships part.
    Ok(El {
        name: "Relationships".into(),
        attrs: vec![(
            "xmlns".into(),
            "http://schemas.openxmlformats.org/package/2006/relationships".into(),
        )],
        children: vec![],
    })
}
// Structural table edits regenerate rows, so admit only the documented simple subset.
// Ordinary cell text/fill edits retain the surrounding original XML and do not use this gate.
fn simple_table_rows(e: &El) -> bool {
    if e.name
        .split_once(':')
        .is_none_or(|(prefix, _)| prefix != "a")
    {
        return false;
    }
    let attrs: &[&str] = match e.local() {
        "tblGrid" | "tcPr" | "txBody" | "bodyPr" | "lstStyle" | "p" | "solidFill" => &[],
        "gridCol" => &["w"],
        "tr" => &["h"],
        "tc" => &["rowSpan", "gridSpan", "hMerge", "vMerge"],
        "pPr" => &["algn", "lvl"],
        "r" | "br" => &[],
        "rPr" | "endParaRPr" | "defRPr" => {
            &["lang", "altLang", "sz", "b", "i", "dirty", "smtClean"]
        }
        "latin" | "ea" | "cs" => &["typeface", "panose", "pitchFamily", "charset"],
        "srgbClr" | "schemeClr" => &["val"],
        "t" => &["xml:space"],
        _ => return false,
    };
    e.attrs.iter().all(|(k, _)| attrs.contains(&k.as_str())) && e.elements().all(simple_table_rows)
}
fn child_mut<'a>(e: &'a mut El, local: &str) -> Option<&'a mut El> {
    e.elements_mut().find(|e| e.is(local))
}
fn find_mut<'a>(e: &'a mut El, local: &str) -> Option<&'a mut El> {
    if e.is(local) {
        return Some(e);
    }
    for c in e.elements_mut() {
        if let Some(e) = find_mut(c, local) {
            return Some(e);
        }
    }
    None
}
fn set(e: &mut El, key: &str, value: String) {
    if let Some((_, v)) = e.attrs.iter_mut().find(|(k, _)| k == key) {
        *v = value
    } else {
        e.attrs.push((key.into(), value));
    }
}
fn replace(e: &mut El, local: &str, new: Option<El>) {
    let at = e
        .children
        .iter()
        .position(|n| matches!(n,Node::El(e) if e.is(local)));
    e.children
        .retain(|n| !matches!(n,Node::El(e) if e.is(local)));
    if let Some(new) = new {
        e.children.insert(
            at.unwrap_or(e.children.len()).min(e.children.len()),
            Node::El(new),
        );
    }
}
fn tree(e: &El) -> Option<&El> {
    e.path(&["cSld", "spTree"])
}
fn node_id(e: &El) -> Option<ShapeId> {
    e.find("cNvPr").and_then(|e| e.u32("id")).map(ShapeId)
}
fn read_parts(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, PptxError> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| error(e.to_string()))?;
    let mut out = BTreeMap::new();
    for i in 0..z.len() {
        let mut entry = z.by_index(i).map_err(|e| error(e.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().map_err(|e| error(e.to_string()))?.to_string();
        let mut bytes = vec![];
        entry
            .read_to_end(&mut bytes)
            .map_err(|e| error(e.to_string()))?;
        out.insert(name, bytes);
    }
    Ok(out)
}
fn unique(parts: &BTreeMap<String, Vec<u8>>, folder: &str, id: u32) -> String {
    let mut i = 0;
    loop {
        let n = format!("{folder}/joydeck-{id}-{i}.xml");
        if !parts.contains_key(&n) {
            return n;
        }
        i += 1;
    }
}
fn add_override(ct: &mut El, name: &str, kind: &str) {
    ct.children.push(Node::El(El {
        name: "Override".into(),
        attrs: vec![
            ("PartName".into(), format!("/{name}")),
            ("ContentType".into(), kind.into()),
        ],
        children: vec![],
    }));
}
fn rel_element(id: &str, kind: &str, target: &str) -> El {
    El {
        name: "Relationship".into(),
        attrs: vec![
            ("Id".into(), id.into()),
            ("Type".into(), format!("{}{kind}", opc::RT_BASE)),
            ("Target".into(), target.into()),
        ],
        children: vec![],
    }
}
fn namespaces(root: &mut El) -> Result<(), PptxError> {
    for (prefix, ns) in [
        ("p", opc::NS_P),
        ("a", opc::NS_A),
        ("r", opc::NS_R),
        ("c", opc::NS_C),
    ] {
        let key = format!("xmlns:{prefix}");
        if root.attrs.iter().any(|(k, v)| k == &key && v != ns) {
            return Err(error(
                "nonstandard namespace binding; use .joydeck to preserve the original",
            ));
        }
        set(root, &key, ns.into());
    }
    Ok(())
}
pub fn export_preserving(
    original: &[u8],
    base: &Presentation,
    current: &Presentation,
    bindings: &[SlideBinding],
) -> Result<Vec<u8>, PptxError> {
    let mut parts = read_parts(original)?;
    if parts
        .keys()
        .any(|p| p.to_ascii_lowercase().ends_with("vbaproject.bin"))
        || parts
            .get("[Content_Types].xml")
            .is_some_and(|b| String::from_utf8_lossy(b).contains("macroEnabled"))
    {
        return Err(error(
            "macro-enabled Office source cannot be saved as ordinary PPTX; retain the original in a .joydeck project or explicitly regenerate a copy",
        ));
    }
    if parts.keys().any(|p| p.starts_with("_xmlsignatures/")) {
        return Err(error(
            "signed PPTX cannot be edited in place; save a .joydeck project",
        ));
    }
    let pkg = opc::Package::open(original)?;
    let pres_path = pkg.presentation_path()?;
    let mut pres = pkg
        .xml(&pres_path)
        .ok_or_else(|| error("missing presentation part"))?
        .root;
    let rels = pkg.rels(&pres_path);
    let ordered: Vec<_> = pres
        .child("sldIdLst")
        .ok_or_else(|| error("missing slide list"))?
        .children_named("sldId")
        .map(|e| {
            let rid = e
                .attr("r:id")
                .ok_or_else(|| error("missing slide relationship"))?;
            Ok((
                e.clone(),
                rels.target(rid)
                    .ok_or_else(|| error("missing slide target"))?
                    .to_string(),
            ))
        })
        .collect::<Result<_, PptxError>>()?;
    if ordered.len() != base.slides.len() {
        return Err(error("ambiguous original slide binding"));
    }
    let structural = current.slides.iter().map(|s| s.id).collect::<Vec<_>>()
        != base.slides.iter().map(|s| s.id).collect::<Vec<_>>();
    if structural && (pres.find("sectionLst").is_some() || pres.find("custShowLst").is_some()) {
        return Err(error(
            "PPTX slide structure with sections/custom shows needs .joydeck Save As",
        ));
    }
    let generated = crate::export(current)?;
    let gen_pkg = opc::Package::open(&generated)?;
    let mut ct = pkg
        .xml("[Content_Types].xml")
        .ok_or_else(|| error("missing content types"))?
        .root;
    let mut pres_rels = pkg
        .xml(&opc::rels_path(&pres_path))
        .ok_or_else(|| error("missing presentation relationships"))?
        .root;
    content::globals(&pkg, &gen_pkg, base, current, &mut parts, &mut pres)?;
    let mut ids = vec![];
    let mut next_file_id = ordered
        .iter()
        .filter_map(|(e, _)| e.u32("id"))
        .max()
        .unwrap_or(255)
        .checked_add(1)
        .ok_or_else(|| error("slide ID space exhausted"))?;
    for (index, slide) in current.slides.iter().enumerate() {
        let binding = bindings
            .iter()
            .find(|b| b.slide == slide.id)
            .ok_or_else(|| error("new slides must originate from an existing layout"))?;
        let source_index = base
            .slides
            .iter()
            .position(|s| s.id == binding.source)
            .ok_or_else(|| error("missing original slide"))?;
        let source_slide = &base.slides[source_index];
        let (source_id, source_part) = &ordered[source_index];
        let existing = slide.id == source_slide.id;
        let part = if existing {
            source_part.clone()
        } else {
            unique(&parts, "ppt/slides", slide.id.0)
        };
        let mut id = source_id.clone();
        if !existing {
            set(&mut id, "id", next_file_id.to_string());
            next_file_id += 1;
            let mut rid = format!("rIdJoyDeck{}", slide.id.0);
            while pres_rels.elements().any(|e| e.attr("Id") == Some(&rid)) {
                rid.push('x');
            }
            set(&mut id, "r:id", rid.clone());
            pres_rels.children.push(Node::El(rel_element(
                &rid,
                "slide",
                &opc::relative(&pres_path, &part),
            )));
            add_override(
                &mut ct,
                &part,
                "application/vnd.openxmlformats-officedocument.presentationml.slide+xml",
            );
        }
        ids.push(Node::El(id));
        if slide.notes != source_slide.notes && (existing || !slide.notes.is_empty()) {
            if !existing {
                return Err(error(
                    "editing notes while cloning a slide requires .joydeck Save As",
                ));
            }
            let notes = pkg
                .rels(source_part)
                .items
                .into_iter()
                .find(|r| r.kind == "notesSlide")
                .ok_or_else(|| error("no reusable notes part; save .joydeck instead"))?;
            let mut root = pkg
                .xml(&notes.target)
                .ok_or_else(|| error("missing original notes XML"))?
                .root;
            namespaces(&mut root)?;
            let body = child_mut(
                child_mut(&mut root, "cSld").ok_or_else(|| error("missing notes cSld"))?,
                "spTree",
            )
            .ok_or_else(|| error("missing notes spTree"))?
            .elements_mut()
            .find(|s| s.find("ph").is_some_and(|p| p.attr("type") == Some("body")))
            .ok_or_else(|| error("no speaker notes placeholder"))?;
            if slide.notes.is_empty() {
                let tx = child_mut(body, "txBody").ok_or_else(|| error("missing speaker body"))?;
                tx.children
                    .retain(|n| !matches!(n,Node::El(e) if e.is("p")));
                tx.children.push(Node::El(El {
                    name: "a:p".into(),
                    attrs: vec![],
                    children: vec![],
                }));
            } else {
                let generated = gen_pkg
                    .xml(&format!("ppt/notesSlides/notesSlide{}.xml", index + 1))
                    .ok_or_else(|| error("missing generated notes"))?
                    .root;
                let speaker = generated
                    .path(&["cSld", "spTree"])
                    .and_then(|t| {
                        t.elements()
                            .find(|s| s.find("ph").is_some_and(|p| p.attr("type") == Some("body")))
                    })
                    .and_then(|s| s.child("txBody"))
                    .cloned()
                    .ok_or_else(|| error("missing generated speaker body"))?;
                if speaker.find("hlinkClick").is_some() || speaker.find("hlinkMouseOver").is_some()
                {
                    return Err(error("edited notes hyperlinks require .joydeck Save As"));
                }
                replace(body, "txBody", Some(speaker));
            }
            parts.insert(notes.target, root.to_xml().into_bytes());
        }
        if existing && slide.as_ref() == source_slide.as_ref() {
            continue;
        }
        let mut root = pkg
            .xml(source_part)
            .ok_or_else(|| error("invalid source slide XML"))?
            .root;
        namespaces(&mut root)?;
        if !existing {
            replace(&mut root, "timing", None);
        }
        if slide.transition.is_none() && source_slide.transition.is_some() {
            replace(&mut root, "transition", None);
        }
        let deleted: Vec<_> = source_slide
            .shapes
            .iter()
            .filter(|old| {
                !binding.shapes.iter().any(|(new, source)| {
                    *source == old.id && slide.shapes.iter().any(|s| s.id == *new)
                })
            })
            .map(|s| s.id)
            .collect();
        if existing
            && !deleted.is_empty()
            && (root.find("timing").is_some()
                || source_slide
                    .shapes
                    .iter()
                    .any(|s| matches!(s.kind, joydeck_model::ShapeKind::Connector { .. })))
        {
            return Err(error(
                "deleting animated/connected objects needs .joydeck Save As in R2",
            ));
        }
        if let Some(common) = child_mut(&mut root, "cSld") {
            set(common, "name", slide.name.clone());
        }
        let old_tree = tree(&root)
            .ok_or_else(|| error("missing source shape tree"))?
            .clone();
        let generated_root = gen_pkg
            .xml(&format!("ppt/slides/slide{}.xml", index + 1))
            .ok_or_else(|| error("missing generated slide"))?
            .root;
        let gen_tree =
            tree(&generated_root).ok_or_else(|| error("missing generated shape tree"))?;
        if slide.background != source_slide.background {
            let common = child_mut(&mut root, "cSld").ok_or_else(|| error("missing slide cSld"))?;
            replace(
                common,
                "bg",
                generated_root
                    .child("cSld")
                    .and_then(|e| e.child("bg"))
                    .cloned(),
            );
            if let Some(at) = common
                .children
                .iter()
                .position(|n| matches!(n,Node::El(e) if e.is("bg")))
            {
                let bg = common.children.remove(at);
                common.children.insert(0, bg);
            }
        }
        if slide.show_master_shapes != source_slide.show_master_shapes {
            set(
                &mut root,
                "showMasterSp",
                if slide.show_master_shapes { "1" } else { "0" }.into(),
            );
        }
        let mut added_rels = vec![];
        // Any unbound drawing on a touched page is refused rather than silently removed.
        for e in old_tree
            .elements()
            .filter(|e| !matches!(e.local(), "nvGrpSpPr" | "grpSpPr" | "extLst"))
        {
            if node_id(e).is_none_or(|id| !source_slide.shapes.iter().any(|s| s.id == id)) {
                return Err(error(
                    "page contains an unmapped drawing; save .joydeck to keep it",
                ));
            }
        }
        let mut nodes: Vec<Node> = old_tree
            .children
            .iter()
            .filter(|n| !matches!(n,Node::El(e) if node_id(e).is_some()))
            .cloned()
            .collect();
        for shape in &slide.shapes {
            let generated = gen_tree
                .elements()
                .find(|e| node_id(e) == Some(shape.id))
                .ok_or_else(|| error("unexportable shape"))?;
            let mut e = if let Some(source_id) = binding.shapes.get(&shape.id) {
                let old = source_slide
                    .shapes
                    .iter()
                    .find(|s| s.id == *source_id)
                    .ok_or_else(|| error("missing source object"))?;
                let mut e = old_tree
                    .elements()
                    .find(|e| node_id(e) == Some(*source_id))
                    .ok_or_else(|| error("object has no source XML"))?
                    .clone();
                if let Some(nv) = find_mut(&mut e, "cNvPr") {
                    set(nv, "id", shape.id.0.to_string());
                }
                if shape.xfrm != old.xfrm {
                    let name = if e.is("grpSp") {
                        "grpSpPr"
                    } else if e.is("graphicFrame") {
                        "xfrm"
                    } else {
                        "spPr"
                    };
                    let replacement = generated
                        .child(name)
                        .cloned()
                        .ok_or_else(|| error("missing generated transform"))?;
                    if name == "xfrm" {
                        replace(&mut e, "xfrm", Some(replacement));
                    } else {
                        let source = child_mut(&mut e, name)
                            .ok_or_else(|| error("missing original shape properties"))?;
                        let x = replacement
                            .child("xfrm")
                            .cloned()
                            .ok_or_else(|| error("missing generated transform"))?;
                        replace(source, "xfrm", Some(x));
                        // xfrm must precede the geometry/fill children in DrawingML.
                        if let Some(at) = source
                            .children
                            .iter()
                            .position(|n| matches!(n,Node::El(e) if e.is("xfrm")))
                        {
                            let x = source.children.remove(at);
                            source.children.insert(0, x);
                        }
                    }
                }
                if shape.text != old.text {
                    if generated.find("hlinkClick").is_some()
                        || generated.find("hlinkMouseOver").is_some()
                    {
                        return Err(error(
                            "edited hyperlink relationships require .joydeck Save As",
                        ));
                    }
                    replace(&mut e, "txBody", generated.child("txBody").cloned());
                }
                if let (
                    joydeck_model::ShapeKind::Table(before),
                    joydeck_model::ShapeKind::Table(after),
                ) = (&old.kind, &shape.kind)
                    && before != after
                {
                    let source =
                        find_mut(&mut e, "tbl").ok_or_else(|| error("missing source table"))?;
                    let generated = generated
                        .find("tbl")
                        .ok_or_else(|| error("missing generated table"))?;
                    if before.cols.len() == after.cols.len()
                        && before.rows.len() == after.rows.len()
                        && before
                            .rows
                            .iter()
                            .zip(&after.rows)
                            .all(|(a, b)| a.cells.len() == b.cells.len())
                    {
                        if before.cols != after.cols {
                            let grid = child_mut(source, "tblGrid")
                                .ok_or_else(|| error("table grid missing"))?;
                            let new_grid = generated
                                .child("tblGrid")
                                .ok_or_else(|| error("generated grid missing"))?;
                            for (old, new) in grid
                                .elements_mut()
                                .filter(|e| e.is("gridCol"))
                                .zip(new_grid.elements().filter(|e| e.is("gridCol")))
                            {
                                if let Some(w) = new.attr("w") {
                                    set(old, "w", w.into());
                                }
                            }
                        }
                        if before.style != after.style
                            || before.first_row != after.first_row
                            || before.band_row != after.band_row
                        {
                            let new = generated
                                .child("tblPr")
                                .ok_or_else(|| error("generated table properties missing"))?;
                            let old = child_mut(source, "tblPr")
                                .ok_or_else(|| error("table properties missing"))?;
                            for name in ["firstRow", "bandRow"] {
                                old.attrs.retain(|(k, _)| k != name);
                                if let Some(v) = new.attr(name) {
                                    set(old, name, v.into());
                                }
                            }
                            replace(old, "tableStyleId", new.child("tableStyleId").cloned());
                        }
                        // Text/fill edits retain untouched cells, table style and unknown table XML.
                        for (r, row) in source.elements_mut().filter(|e| e.is("tr")).enumerate() {
                            let generated_row = generated
                                .elements()
                                .filter(|e| e.is("tr"))
                                .nth(r)
                                .ok_or_else(|| error("missing generated table row"))?;
                            if before.rows[r].height != after.rows[r].height
                                && let Some(h) = generated_row.attr("h")
                            {
                                set(row, "h", h.into());
                            }
                            for (c, cell) in row.elements_mut().filter(|e| e.is("tc")).enumerate() {
                                let a = before
                                    .cell(r, c)
                                    .ok_or_else(|| error("unmapped source cell"))?;
                                let b = after
                                    .cell(r, c)
                                    .ok_or_else(|| error("unmapped edited cell"))?;
                                let g = generated_row
                                    .elements()
                                    .filter(|e| e.is("tc"))
                                    .nth(c)
                                    .ok_or_else(|| error("missing generated cell"))?;
                                if a.text != b.text {
                                    if g.find("hlinkClick").is_some()
                                        || g.find("hlinkMouseOver").is_some()
                                    {
                                        return Err(error(
                                            "edited cell hyperlinks require .joydeck Save As",
                                        ));
                                    }
                                    replace(cell, "txBody", g.child("txBody").cloned());
                                }
                                if a.fill != b.fill
                                    || a.grid_span != b.grid_span
                                    || a.row_span != b.row_span
                                    || a.h_merge != b.h_merge
                                    || a.v_merge != b.v_merge
                                {
                                    // Merge identity is on a:tc, fill is inside a:tcPr.
                                    for key in ["gridSpan", "rowSpan", "hMerge", "vMerge"] {
                                        cell.attrs.retain(|(k, _)| k != key);
                                        if let Some(v) = g.attr(key) {
                                            set(cell, key, v.into());
                                        }
                                    }
                                    let pr = child_mut(cell, "tcPr")
                                        .ok_or_else(|| error("missing cell properties"))?;
                                    pr.children.retain(|n|!matches!(n,Node::El(e) if matches!(e.local(),"gradFill"|"pattFill"|"noFill"|"blipFill"|"grpFill")));
                                    replace(
                                        pr,
                                        "solidFill",
                                        g.child("tcPr").and_then(|x| x.child("solidFill")).cloned(),
                                    );
                                }
                                if a.borders != b.borders
                                    || a.margins != b.margins
                                    || a.anchor != b.anchor
                                    || a.vertical != b.vertical
                                {
                                    if cell.child("tcPr").is_none() {
                                        replace(
                                            cell,
                                            "tcPr",
                                            Some(El {
                                                name: "a:tcPr".into(),
                                                attrs: vec![],
                                                children: vec![],
                                            }),
                                        );
                                    }
                                    let new = g.child("tcPr").ok_or_else(|| {
                                        error("generated cell properties missing")
                                    })?;
                                    let old = child_mut(cell, "tcPr")
                                        .ok_or_else(|| error("cell properties missing"))?;
                                    for (changed, names) in [
                                        (
                                            a.margins != b.margins,
                                            &["marL", "marR", "marT", "marB"][..],
                                        ),
                                        (a.anchor != b.anchor, &["anchor"][..]),
                                        (a.vertical != b.vertical, &["vert"][..]),
                                    ] {
                                        if changed {
                                            for name in names {
                                                old.attrs.retain(|(k, _)| k != name);
                                                if let Some(v) = new.attr(name) {
                                                    set(old, name, v.into());
                                                }
                                            }
                                        }
                                    }
                                    for (side, name) in
                                        ["lnL", "lnR", "lnT", "lnB"].iter().enumerate()
                                    {
                                        if a.borders[side] != b.borders[side] {
                                            replace(old, name, new.child(name).cloned());
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        // Structural edits of extension-bearing tables are intentionally refused.
                        if source
                            .elements()
                            .filter(|e| e.is("tblGrid") || e.is("tr"))
                            .any(|e| !simple_table_rows(e))
                        {
                            return Err(error(
                                "table extensions require .joydeck Save As for structural edits",
                            ));
                        }
                        replace(source, "tblGrid", generated.child("tblGrid").cloned());
                        source
                            .children
                            .retain(|n| !matches!(n,Node::El(e) if e.is("tr")));
                        source.children.extend(
                            generated
                                .elements()
                                .filter(|e| e.is("tr"))
                                .cloned()
                                .map(Node::El),
                        );
                    }
                }
                e
            } else {
                generated.clone()
            };
            // Generated shapes only reference existing theme or explicit colors; R2 inserts no new media.
            let old = binding
                .shapes
                .get(&shape.id)
                .and_then(|id| source_slide.shapes.iter().find(|s| s.id == *id));
            content::shape(
                &mut e,
                generated,
                old,
                shape,
                &gen_pkg,
                &format!("ppt/slides/slide{}.xml", index + 1),
                &part,
                &mut parts,
                &mut ct,
                &mut added_rels,
            )?;
            if let Some(nv) = find_mut(&mut e, "cNvPr") {
                set(nv, "id", shape.id.0.to_string());
            }
            nodes.push(Node::El(e));
        }
        let target = child_mut(
            child_mut(&mut root, "cSld").ok_or_else(|| error("missing cSld"))?,
            "spTree",
        )
        .ok_or_else(|| error("missing shape tree"))?;
        target.children = nodes;
        parts.insert(part.clone(), root.to_xml().into_bytes());
        if !existing {
            let mut slide_rels = relationship_root(&pkg, source_part)?;
            // Cloning a slide can move it out of a custom directory. Rebase internal targets.
            for rel in pkg
                .rels(source_part)
                .items
                .iter()
                .filter(|rel| !rel.external)
            {
                if let Some(node) = slide_rels
                    .elements_mut()
                    .find(|node| node.attr("Id") == Some(&rel.id))
                {
                    set(node, "Target", opc::relative(&part, &rel.target));
                }
            }
            let notes = pkg
                .rels(source_part)
                .items
                .into_iter()
                .find(|r| r.kind == "notesSlide");
            if slide.notes.is_empty() {
                slide_rels.children.retain(|n|!matches!(n,Node::El(e) if e.attr("Type").is_some_and(|t|t.ends_with("/notesSlide"))));
            } else if let Some(notes) = notes {
                let new_notes = unique(&parts, "ppt/notesSlides", slide.id.0);
                parts.insert(
                    new_notes.clone(),
                    pkg.get(&notes.target)
                        .ok_or_else(|| error("missing notes bytes"))?
                        .to_vec(),
                );
                add_override(
                    &mut ct,
                    &new_notes,
                    "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml",
                );
                if let Some(r) = slide_rels
                    .elements_mut()
                    .find(|r| r.attr("Id") == Some(&notes.id))
                {
                    set(r, "Target", opc::relative(&part, &new_notes));
                }
                let mut notes_rels = pkg
                    .xml(&opc::rels_path(&notes.target))
                    .ok_or_else(|| error("missing notes relationships"))?
                    .root;
                for r in notes_rels.elements_mut() {
                    if r.attr("Type").is_some_and(|t| t.ends_with("/slide")) {
                        set(r, "Target", opc::relative(&new_notes, &part));
                    }
                }
                parts.insert(opc::rels_path(&new_notes), notes_rels.to_xml().into_bytes());
            }
            parts.insert(opc::rels_path(&part), slide_rels.to_xml().into_bytes());
        }
        if !added_rels.is_empty() || slide.layout != source_slide.layout {
            let path = opc::rels_path(&part);
            let mut relationships = if let Some(bytes) = parts.get(&path) {
                crate::xml::parse(bytes)?.root
            } else {
                relationship_root(&pkg, source_part)?
            };
            relationships
                .children
                .extend(added_rels.into_iter().map(Node::El));
            if slide.layout != source_slide.layout {
                let target = content::layout_target(&pkg, base, slide.layout)?;
                let rel = relationships
                    .elements_mut()
                    .find(|r| r.attr("Type").is_some_and(|t| t.ends_with("/slideLayout")))
                    .ok_or_else(|| error("layout relationship missing"))?;
                set(rel, "Target", opc::relative(&part, &target));
            }
            parts.insert(path, relationships.to_xml().into_bytes());
        }
    }
    if structural {
        child_mut(&mut pres, "sldIdLst")
            .ok_or_else(|| error("missing slide list"))?
            .children = ids;
        parts.insert(opc::rels_path(&pres_path), pres_rels.to_xml().into_bytes());
    }
    if structural || current.slide_size != base.slide_size {
        parts.insert(pres_path, pres.to_xml().into_bytes());
    }
    // New image/chart/workbook parts also need content types without changing slide structure.
    if ct
        != pkg
            .xml("[Content_Types].xml")
            .ok_or_else(|| error("content types missing"))?
            .root
    {
        parts.insert("[Content_Types].xml".into(), ct.to_xml().into_bytes());
    }
    let mut z = zip::ZipWriter::new(Cursor::new(vec![]));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in parts {
        z.start_file(name, options)
            .map_err(|e| error(e.to_string()))?;
        z.write_all(&bytes).map_err(|e| error(e.to_string()))?;
    }
    Ok(z.finish().map_err(|e| error(e.to_string()))?.into_inner())
}
