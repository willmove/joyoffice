//! Apply table edits to DrawingML while retaining untouched rows, cells and metadata.

use joydeck_model::{
    Fill,
    table::{Cell, Table},
};

use super::{El, Node, PptxError, error, fields, links, patch_text};

const MERGES: &[&str] = &["gridSpan", "rowSpan", "hMerge", "vMerge"];
const FILLS: &[&str] = &[
    "noFill",
    "solidFill",
    "gradFill",
    "blipFill",
    "pattFill",
    "grpFill",
];
const LINES: &[&str] = &["lnL", "lnR", "lnT", "lnB", "lnTlToBr", "lnBlToTr"];
const FLAGS: &[&str] = &[
    "firstRow", "firstCol", "lastRow", "lastCol", "bandRow", "bandCol",
];

pub(super) fn register_styles(
    original: &super::opc::Package,
    generated: &super::opc::Package,
    presentation: &str,
    relationships: &mut El,
    parts: &mut super::PartEdits<'_>,
    types: &mut El,
    needed: &std::collections::BTreeSet<String>,
) -> Result<bool, PptxError> {
    use super::{add_override, append_relationships, content, opc, rel_element};
    const GENERATED: &str = "ppt/tableStyles.xml";
    let Some(mut next) = generated.xml(GENERATED) else {
        return Ok(false);
    };
    next.root.children.retain(|child| {
        matches!(child, Node::El(style) if style.is("tblStyle")
        && style.attr("styleId").is_some_and(|id| needed.contains(&id.to_ascii_uppercase())))
    });
    if next.root.child("tblStyle").is_none() {
        return Ok(false);
    }
    let rels = original.rels(presentation);
    if let Some(existing) = rels.first_of("tableStyles").filter(|edge| !edge.external) {
        let mut doc = original
            .xml(&existing.target)
            .ok_or_else(|| error("invalid source table styles"))?;
        let ids: std::collections::HashSet<_> = doc
            .root
            .children_named("tblStyle")
            .filter_map(|style| style.attr("styleId").map(str::to_ascii_uppercase))
            .collect();
        let mut additions = next
            .root
            .children_named("tblStyle")
            .filter(|style| {
                style
                    .attr("styleId")
                    .is_some_and(|id| !ids.contains(&id.to_ascii_uppercase()))
            })
            .peekable();
        if additions.peek().is_some() {
            super::namespaces(&mut doc.root)?;
            doc.root.children.extend(additions.cloned().map(Node::El));
            parts.insert(existing.target.clone(), doc.into_xml());
        }
        return Ok(false);
    }
    let target = if parts.contains_key(GENERATED) {
        content::new_name(parts, GENERATED)
    } else {
        GENERATED.into()
    };
    let mut id = "rIdJoyDeckTableStyles".to_owned();
    while relationships
        .elements()
        .any(|edge| edge.attr("Id") == Some(&id))
    {
        id.push('x');
    }
    append_relationships(
        relationships,
        [rel_element(
            &id,
            "tableStyles",
            &opc::relative(presentation, &target),
        )],
    );
    add_override(
        types,
        &target,
        "application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml",
    );
    parts.insert(target, next.into_xml());
    Ok(true)
}

pub(super) fn patch(
    source: &mut El,
    generated: &El,
    before: &Table,
    after: &Table,
    relationships: &mut links::Relationships<'_>,
    ids: &mut fields::Ids,
) -> Result<(), PptxError> {
    let mut table_properties_changed = false;
    let old_flags = flags(before);
    let new_flags = flags(after);
    for (index, flag) in FLAGS.iter().enumerate() {
        if old_flags[index] != new_flags[index] {
            let generated = required(generated, "tblPr")?;
            let target = properties(source, "tblPr", 0)?;
            copy_attrs(target, generated, &[*flag]);
            table_properties_changed = true;
        }
    }
    if before.style != after.style
        || before.background != after.background
        || table_properties_changed
    {
        let generated = required(generated, "tblPr")?;
        let props = properties(source, "tblPr", 0)?;
        if before.style != after.style || props.child("tableStyleId").is_none() {
            replace_choice(props, generated, &["tableStyleId"], &["extLst"])?;
        }
        if before.background != after.background {
            replace_choice(
                props,
                generated,
                FILLS,
                &["tableStyle", "tableStyleId", "extLst"],
            )?;
        }
    }
    if before.rows.len() != after.rows.len()
        || before.cols.len() != after.cols.len()
        || before
            .rows
            .iter()
            .zip(&after.rows)
            .any(|(a, b)| a.cells.len() != b.cells.len())
    {
        return replace_grid(source, generated, relationships, ids);
    }
    if before.cols != after.cols {
        let source_grid =
            child(source, "tblGrid").ok_or_else(|| error("source table grid missing"))?;
        let generated_grid = required(generated, "tblGrid")?;
        for ((target, next), (old, new)) in children_mut(source_grid, "gridCol")
            .zip(generated_grid.children_named("gridCol"))
            .zip(before.cols.iter().zip(&after.cols))
        {
            if old != new {
                copy_attrs(target, next, &["w"]);
            }
        }
    }
    let mut source_rows = children_mut(source, "tr");
    let mut output_rows = generated.children_named("tr");
    for (before, after) in before.rows.iter().zip(&after.rows) {
        let row = source_rows
            .next()
            .ok_or_else(|| error("source table row missing"))?;
        let next = output_rows
            .next()
            .ok_or_else(|| error("generated table row missing"))?;
        if before.height != after.height {
            copy_attrs(row, next, &["h"]);
        }
        let mut cells = children_mut(row, "tc");
        let mut next_cells = next.children_named("tc");
        for (before, after) in before.cells.iter().zip(&after.cells) {
            let cell = cells
                .next()
                .ok_or_else(|| error("source table cell missing"))?;
            let generated = next_cells
                .next()
                .ok_or_else(|| error("generated table cell missing"))?;
            if before.text != after.text {
                patch_text(
                    cell,
                    generated,
                    Some(&before.text),
                    Some(&after.text),
                    relationships,
                    ids,
                )?;
            }
            patch_cell(cell, generated, before, after)?;
        }
    }
    Ok(())
}

fn flags(table: &Table) -> [bool; 6] {
    [
        table.first_row,
        table.first_col,
        table.last_row,
        table.last_col,
        table.band_row,
        table.band_col,
    ]
}

fn patch_cell(
    target: &mut El,
    generated: &El,
    before: &Cell,
    after: &Cell,
) -> Result<(), PptxError> {
    if (
        before.grid_span,
        before.row_span,
        before.h_merge,
        before.v_merge,
    ) != (
        after.grid_span,
        after.row_span,
        after.h_merge,
        after.v_merge,
    ) {
        copy_attrs(target, generated, MERGES);
    }
    let changes = [
        before.margins != after.margins,
        before.anchor != after.anchor,
        before.vertical != after.vertical,
    ];
    let lines = before
        .borders
        .iter()
        .chain([&before.diag_down, &before.diag_up])
        .zip(
            after
                .borders
                .iter()
                .chain([&after.diag_down, &after.diag_up]),
        )
        .map(|(a, b)| a != b);
    let fill_changed = before.fill != after.fill;
    if !fill_changed
        && !changes.iter().any(|changed| *changed)
        && !lines.clone().any(|changed| changed)
    {
        return Ok(());
    }
    let next = required(generated, "tcPr")?;
    let props = properties(target, "tcPr", target.children.len())?;
    for (changed, names) in changes.into_iter().zip([
        &["marL", "marR", "marT", "marB"][..],
        &["anchor"],
        &["vert"],
    ]) {
        if changed {
            copy_attrs(props, next, names);
        }
    }
    for (index, (name, changed)) in LINES.iter().zip(lines).enumerate() {
        if changed {
            let later: Vec<_> = LINES[index + 1..]
                .iter()
                .chain(FILLS)
                .copied()
                .chain(["cell3D", "extLst"])
                .collect();
            replace_choice(props, next, &[*name], &later)?;
        }
    }
    if fill_changed {
        if matches!(after.fill, Some(Fill::Picture(_))) {
            return Err(error("image-filled table cells require .joydeck Save As"));
        }
        replace_choice(props, next, FILLS, &["cell3D", "extLst"])?;
    }
    Ok(())
}

fn required<'a>(source: &'a El, local: &str) -> Result<&'a El, PptxError> {
    source
        .child(local)
        .ok_or_else(|| error(format!("generated table {local} missing")))
}

fn children_mut<'a>(parent: &'a mut El, local: &'static str) -> impl Iterator<Item = &'a mut El> {
    let prefix = parent.prefix().to_owned();
    parent
        .elements_mut()
        .filter(move |node| node.is(local) && drawing_namespace(node, &prefix))
}

fn child<'a>(parent: &'a mut El, local: &'static str) -> Option<&'a mut El> {
    children_mut(parent, local).next()
}

fn drawing_namespace(node: &El, parent_prefix: &str) -> bool {
    let prefix = node.prefix();
    let local_binding = node.attrs.iter().find(|(key, _)| {
        key.strip_prefix("xmlns:") == Some(prefix) || (prefix.is_empty() && key == "xmlns")
    });
    local_binding.map_or(prefix == parent_prefix || prefix == "a", |(_, uri)| {
        uri == super::opc::NS_A
    })
}

fn properties<'a>(parent: &'a mut El, local: &str, at: usize) -> Result<&'a mut El, PptxError> {
    let full_name = if parent.prefix().is_empty() {
        local.to_owned()
    } else {
        format!("{}:{local}", parent.prefix())
    };
    let position = parent
        .children
        .iter()
        .position(|part| matches!(part, Node::El(node) if node.is(local) && drawing_namespace(node, parent.prefix())))
        .unwrap_or_else(|| {
            parent.children.insert(
                at,
                Node::El(El {
                    name: full_name,
                    ..El::default()
                }),
            );
            at
        });
    match &mut parent.children[position] {
        Node::El(element) => Ok(element),
        _ => Err(error("missing table property element")),
    }
}

fn copy_attrs(target: &mut El, source: &El, keys: &[&str]) {
    for &key in keys {
        if let Some(value) = source
            .attrs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
        {
            target.set_attr(key, value);
        } else {
            target.attrs.retain(|(name, _)| name != key);
        }
    }
}

fn replace_choice(
    target: &mut El,
    source: &El,
    names: &[&str],
    later: &[&str],
) -> Result<(), PptxError> {
    let prefix = target.prefix().to_owned();
    let member = |node: &Node, choices: &[&str]| matches!(node, Node::El(node) if drawing_namespace(node, &prefix) && choices.contains(&node.local()));
    if target
        .children
        .iter()
        .filter(|node| member(node, names))
        .any(|node| matches!(node, Node::El(element) if !plain_style(element, &prefix)))
    {
        return Err(error(
            "edited table style contains opaque formatting; save .joydeck instead",
        ));
    }
    let index = target.children.iter().position(|node| member(node, names));
    target.children.retain(|node| !member(node, names));
    if let Some(next) = source
        .elements()
        .find(|element| names.contains(&element.local()))
    {
        let at = index
            .or_else(|| target.children.iter().position(|node| member(node, later)))
            .unwrap_or(target.children.len());
        target.children.insert(at, Node::El(next.clone()));
    }
    Ok(())
}

fn plain_style(node: &El, prefix: &str) -> bool {
    if !drawing_namespace(node, prefix) {
        return false;
    }
    let attrs: &[&str] = match node.local() {
        "tableStyleId" | "noFill" | "solidFill" | "grpFill" | "fgClr" | "bgClr" | "gsLst"
        | "stretch" | "custDash" | "round" | "bevel" => &[],
        "lnL" | "lnR" | "lnT" | "lnB" | "lnTlToBr" | "lnBlToTr" => &["w", "cap", "cmpd", "algn"],
        "gradFill" => &["flip", "rotWithShape"],
        "pattFill" => &["prst"],
        "blipFill" => &["dpi", "rotWithShape"],
        "blip" => &["r:embed", "r:link", "cstate"],
        "tile" => &["tx", "ty", "sx", "sy", "flip", "algn"],
        "srcRect" | "fillRect" | "fillToRect" | "tileRect" => &["l", "t", "r", "b"],
        "lin" => &["ang", "scaled"],
        "path" => &["path"],
        "gs" => &["pos"],
        "ds" => &["d", "sp"],
        "miter" => &["lim"],
        "headEnd" | "tailEnd" => &["type", "w", "len"],
        "srgbClr" | "schemeClr" | "prstClr" | "prstDash" => &["val"],
        "sysClr" => &["val", "lastClr"],
        "scrgbClr" => &["r", "g", "b"],
        "hslClr" => &["hue", "sat", "lum"],
        name if joydeck_color::ColorTransform::from_xml(name, node.i32("val")).is_some() => {
            &["val"]
        }
        _ => return false,
    };
    node.attrs
        .iter()
        .all(|(name, _)| attrs.contains(&name.as_str()))
        && node.children.iter().all(|child| match child {
            Node::El(child) => plain_style(child, prefix),
            Node::Text(text) => node.is("tableStyleId") || text.trim().is_empty(),
            Node::Markup(_) => false,
        })
}

fn replace_grid(
    source: &mut El,
    generated: &El,
    relationships: &mut links::Relationships<'_>,
    ids: &mut fields::Ids,
) -> Result<(), PptxError> {
    let prefix = source.prefix().to_owned();
    let row = |node: &El| node.prefix() == prefix && matches!(node.local(), "tblGrid" | "tr");
    if source
        .elements()
        .filter(|node| row(node))
        .any(|node| !plain_grid(node, &prefix))
    {
        return Err(error(
            "table extensions require .joydeck Save As for structural edits",
        ));
    }
    let at = source
        .children
        .iter()
        .position(|node| matches!(node, Node::El(e) if row(e)))
        .ok_or_else(|| error("source table grid missing"))?;
    let mut replacement = vec![Node::El(required(generated, "tblGrid")?.clone())];
    for mut row in generated.children_named("tr").cloned() {
        relationships.rebind_generated(&mut row)?;
        ids.reassign_tree(&mut row);
        replacement.push(Node::El(row));
    }
    source
        .children
        .retain(|node| !matches!(node, Node::El(e) if row(e)));
    source.children.splice(at..at, replacement);
    Ok(())
}

fn plain_grid(node: &El, prefix: &str) -> bool {
    if node.prefix() != prefix {
        return false;
    }
    let attrs: &[&str] = match node.local() {
        "tblGrid" | "tcPr" | "txBody" | "bodyPr" | "lstStyle" | "p" | "solidFill" => &[],
        "gridCol" => &["w"],
        "tr" => &["h"],
        "tc" => MERGES,
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
    node.attrs
        .iter()
        .all(|(name, _)| attrs.contains(&name.as_str()))
        && node.children.iter().all(|child| match child {
            Node::El(child) => plain_grid(child, prefix),
            Node::Text(_) => true,
            Node::Markup(_) => false,
        })
}
