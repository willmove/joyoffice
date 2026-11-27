//! Copy-on-write edits of chart values and their original SpreadsheetML cells.

use std::collections::{BTreeMap, BTreeSet};

use joydeck_model::Chart;

use super::{
    El, Node, PartEdits, PptxError, add_override, child_mut, content, error, find_mut, links, opc,
};

const MAX_WORKBOOK: usize = 32 << 20;
const WORKBOOK_TYPE: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Cell {
    sheet: String,
    address: String,
}

#[derive(Default)]
struct Changes(BTreeMap<Cell, f64>);

impl Changes {
    fn set(&mut self, formula: &str, index: usize, value: f64) -> Result<(), PptxError> {
        let cell = address(formula, index)
            .ok_or_else(|| error("unsupported chart workbook reference; save .joydeck instead"))?;
        if self.0.insert(cell, value).is_some_and(|old| old != value) {
            return Err(error("chart edits disagree on one workbook cell"));
        }
        Ok(())
    }
}

pub(super) fn apply(
    frame: &mut El,
    before: &Chart,
    after: &Chart,
    package: &opc::Package,
    parts: &mut PartEdits<'_>,
    types: &mut El,
    relationships: &mut links::Relationships<'_>,
) -> Result<bool, PptxError> {
    let mut comparable = after.clone();
    comparable.title.clone_from(&before.title);
    if comparable.series.len() != before.series.len() {
        return Ok(false);
    }
    for (series, original) in comparable.series.iter_mut().zip(&before.series) {
        if series.values.len() != original.values.len() {
            return Ok(false);
        }
        series.values.clone_from(&original.values);
    }
    if &comparable != before || before == after {
        return Ok(false);
    }
    let mut candidates = Vec::new();
    frame.find_all("graphicData", &mut candidates);
    let mut references = BTreeMap::new();
    for graphic in candidates {
        let kind = match graphic.attr("uri") {
            Some(opc::NS_C) => "chart",
            Some(crate::chartex_write::NS_CX) => "chartEx",
            _ => continue,
        };
        let edge = graphic
            .child("chart")
            .and_then(|node| node.attr("r:id"))
            .and_then(|id| relationships.source.get(id))
            .filter(|edge| edge.kind == kind && !edge.external)
            .ok_or_else(|| error("chart relationship missing"))?;
        if references.insert(kind, edge).is_some() {
            return Err(error("ambiguous chart references"));
        }
    }
    let primary = if before.kind.is_chartex() {
        "chartEx"
    } else {
        "chart"
    };
    if !references.contains_key(primary) {
        return Ok(false);
    }
    let mut edits = Vec::new();
    for (kind, reference) in references {
        let mut doc = crate::read::load(package, &reference.target)
            .ok_or_else(|| error("original chart part missing"))?;
        if !known_extensions(&doc.root) {
            return Err(error(
                "edited chart has unsupported extensions; save .joydeck to preserve them",
            ));
        }
        let changes = if kind == "chartEx" {
            edit_extended(&mut doc.root, before, after)?
        } else {
            edit_classic(&mut doc.root, before, after)?
        };
        let Some(changes) = changes else {
            return Ok(false);
        };
        edits.push((reference, doc, changes));
    }
    for (reference, doc, changes) in edits {
        let destination = content::new_name(parts, &reference.target);
        let chart_type = package
            .content_type(&reference.target)
            .ok_or_else(|| error("original chart content type missing"))?;
        let rels_path = opc::rels_path(&reference.target);
        if let Some(bytes) = package.get(&rels_path) {
            let bytes = if changes.0.is_empty() {
                bytes.to_vec()
            } else {
                rewrite_workbook(
                    &doc.root,
                    &reference.target,
                    &destination,
                    changes,
                    package,
                    parts,
                    types,
                )?
            };
            parts.insert(opc::rels_path(&destination), bytes);
        } else if !changes.0.is_empty() {
            return Err(error("referenced chart workbook part missing"));
        }
        parts.insert(destination.clone(), doc.into_xml());
        add_override(types, &destination, &chart_type);
        let node = find_chart_binding(frame, &reference.id)
            .ok_or_else(|| error("chart reference missing"))?;
        let binding = node
            .attrs
            .iter_mut()
            .find(|(name, value)| name.ends_with(":id") && *value == reference.id)
            .ok_or_else(|| error("chart relationship attribute missing"))?;
        binding.1 = relationships.add_typed(&reference.ty, &destination, false);
    }
    Ok(true)
}

fn find_chart_binding<'a>(node: &'a mut El, id: &str) -> Option<&'a mut El> {
    if node.is("graphicData")
        && matches!(
            node.attr("uri"),
            Some(opc::NS_C | crate::chartex_write::NS_CX)
        )
    {
        return node
            .elements_mut()
            .find(|child| child.is("chart") && child.attr("r:id") == Some(id));
    }
    node.elements_mut()
        .find_map(|child| find_chart_binding(child, id))
}

fn edit_classic(
    root: &mut El,
    before: &Chart,
    after: &Chart,
) -> Result<Option<Changes>, PptxError> {
    let chart = child_mut(root, "chart").ok_or_else(|| error("chart element missing"))?;
    if before.title != after.title {
        if let (None, Some(text)) = (&before.title, &after.title)
            && chart.child("title").is_none()
        {
            let mut writer = crate::xml::W::frag();
            crate::write::chart::title(&mut writer, text, after);
            let mut title = crate::xml::parse(writer.s.as_bytes())?.root;
            title.set_attr("xmlns:c", opc::NS_C);
            title.set_attr("xmlns:a", opc::NS_A);
            chart.children.insert(0, Node::El(title));
            if let Some(deleted) = child_mut(chart, "autoTitleDeleted") {
                deleted.set_attr("val", "0");
            }
        } else if !title(chart, before, after)? {
            return Ok(None);
        }
    }
    let plot = child_mut(chart, "plotArea").ok_or_else(|| error("chart plot area missing"))?;
    let mut source_series: Vec<_> = plot
        .elements_mut()
        .filter(|group| crate::chart_series::kind_from(group).is_some())
        .flat_map(|group| {
            let prefix = group.prefix().to_owned();
            group
                .elements_mut()
                .filter(move |node| node.is("ser") && node.prefix() == prefix)
        })
        .collect();
    if source_series.len() != before.series.len() {
        return Err(error("ambiguous chart series mapping"));
    }
    if source_series.iter().all(|node| {
        node.child_val("order")
            .is_some_and(|order| order.parse::<u32>().is_ok())
    }) {
        source_series.sort_by_key(|node| node.child("order").and_then(|order| order.u32("val")));
    }
    let mut changes = Changes::default();
    for ((source, old), new) in source_series
        .into_iter()
        .zip(&before.series)
        .zip(&after.series)
    {
        for value in changed_values(&old.values, &new.values) {
            let (index, value) = value?;
            write_point(source, index, value, &mut changes)?;
        }
    }
    Ok(Some(changes))
}

fn changed_values<'a>(
    old: &'a [Option<f64>],
    new: &'a [Option<f64>],
) -> impl Iterator<Item = Result<(usize, f64), PptxError>> + 'a {
    old.iter()
        .zip(new)
        .enumerate()
        .filter(|(_, (before, after))| before != after)
        .map(|(index, (_, value))| {
            value
                .filter(|value| value.is_finite())
                .map(|value| (index, value))
                .ok_or_else(|| error("removing a chart cache value requires .joydeck Save As"))
        })
}

fn edit_extended(
    root: &mut El,
    before: &Chart,
    after: &Chart,
) -> Result<Option<Changes>, PptxError> {
    let chart = child_mut(root, "chart").ok_or_else(|| error("extended chart missing"))?;
    if before.title != after.title && !title(chart, before, after)? {
        return Ok(None);
    }
    let region = chart
        .path(&["plotArea", "plotAreaRegion"])
        .ok_or_else(|| error("extended chart region missing"))?;
    let series: Vec<_> = region
        .children_named("series")
        .filter(|series| {
            series.attr("layoutId") != Some("paretoLine") && series.bool("hidden") != Some(true)
        })
        .map(|series| series.child_val("dataId").unwrap_or("0").to_owned())
        .collect();
    if series.len() != before.series.len() {
        return Err(error("ambiguous extended chart series mapping"));
    }
    let data =
        child_mut(root, "chartData").ok_or_else(|| error("extended chart datasets missing"))?;
    let mut changes = Changes::default();
    for ((id, old), new) in series.iter().zip(&before.series).zip(&after.series) {
        if old.values == new.values {
            continue;
        }
        if series.iter().filter(|other| *other == id).count() != 1 {
            return Err(error(
                "editing a shared chart dataset requires .joydeck Save As",
            ));
        }
        let dataset = data
            .elements_mut()
            .find(|item| item.is("data") && item.attr("id") == Some(id))
            .ok_or_else(|| error("extended chart dataset missing"))?;
        let role = ["val", "size", "colorVal"]
            .into_iter()
            .find(|role| {
                dataset
                    .children_named("numDim")
                    .any(|dimension| dimension.attr("type") == Some(role))
            })
            .or_else(|| {
                dataset
                    .children_named("numDim")
                    .filter_map(|dimension| dimension.attr("type"))
                    .find(|role| *role != "cat")
            })
            .ok_or_else(|| error("extended chart numeric dimension missing"))?
            .to_owned();
        let dimension = dataset
            .children
            .iter_mut()
            .rev()
            .find_map(|child| match child {
                Node::El(dimension)
                    if dimension.is("numDim") && dimension.attr("type") == Some(&role) =>
                {
                    Some(dimension)
                }
                _ => None,
            })
            .ok_or_else(|| error("extended chart numeric dimension missing"))?;
        let formula = dimension.child("f").map(El::text);
        let level = child_mut(dimension, "lvl")
            .ok_or_else(|| error("extended chart value cache missing"))?;
        for value in changed_values(&old.values, &new.values) {
            let (index, value) = value?;
            if let Some(formula) = formula.as_ref() {
                changes.set(formula, index, value)?;
            }
            let point = level
                .elements_mut()
                .find(|point| point.is("pt") && point.u32("idx") == Some(index as u32))
                .ok_or_else(|| error("extended chart cache point missing"))?;
            set_text(point, crate::chart_cache::format_number(value));
        }
    }
    Ok(Some(changes))
}

/// These Office extensions carry appearance or identity, independent of the numeric data.
fn known_extensions(root: &El) -> bool {
    let mut lists = Vec::new();
    root.find_all("extLst", &mut lists);
    lists.into_iter().all(|list| {
        list.elements().all(|extension| {
            matches!(
                extension
                    .attr("uri")
                    .map(str::to_ascii_uppercase)
                    .as_deref(),
                Some(
                    "{CE6537A1-D6FC-4F65-9D91-7224C49458BB}"
                        | "{6F2FDCE9-48DA-4B69-8628-5D25D57E5C99}"
                        | "{C3380CC4-5D6E-409C-BE32-E72D297353CC}"
                )
            )
        })
    })
}

fn title(chart: &mut El, before: &Chart, after: &Chart) -> Result<bool, PptxError> {
    let (Some(old), Some(new)) = (before.title.as_ref(), after.title.as_ref()) else {
        return Ok(false);
    };
    let Some(rich) = child_mut(chart, "title")
        .and_then(|title| child_mut(title, "tx"))
        .and_then(|tx| child_mut(tx, "rich"))
    else {
        return Ok(false);
    };
    let mut text = Vec::new();
    rich.find_all("t", &mut text);
    if text.len() != 1 || text[0].text() != *old {
        return Ok(false);
    }
    let run = find_mut(rich, "t").ok_or_else(|| error("chart title text missing"))?;
    set_text(run, new.clone());
    Ok(true)
}

fn set_text(node: &mut El, value: String) {
    node.children
        .retain(|child| !matches!(child, Node::Text(_)));
    node.children.insert(0, Node::Text(value));
}

fn write_point(
    series: &mut El,
    index: usize,
    value: f64,
    changes: &mut Changes,
) -> Result<(), PptxError> {
    let values = child_mut(series, "val").ok_or_else(|| error("chart value series missing"))?;
    let cache = if let Some(reference) = child_mut(values, "numRef") {
        let formula = reference
            .child("f")
            .ok_or_else(|| error("chart value formula missing"))?
            .text();
        changes.set(&formula, index, value)?;
        child_mut(reference, "numCache")
    } else {
        child_mut(values, "numLit")
    }
    .ok_or_else(|| error("chart value cache missing"))?;
    if cache
        .child("ptCount")
        .and_then(|node| node.u32("val"))
        .is_none_or(|count| index >= count as usize)
    {
        return Err(error("chart value index outside its cache"));
    }
    let number = crate::chart_cache::format_number(value);
    if let Some(point) = cache
        .elements_mut()
        .find(|node| node.is("pt") && node.u32("idx") == Some(index as u32))
    {
        let cell = child_mut(point, "v").ok_or_else(|| error("chart cache value missing"))?;
        set_text(cell, number);
    } else {
        let prefix = cache.prefix();
        let qualified = |local: &str| {
            if prefix.is_empty() {
                local.into()
            } else {
                format!("{prefix}:{local}")
            }
        };
        let point = El {
            name: qualified("pt"),
            attrs: vec![("idx".into(), index.to_string())],
            children: vec![Node::El(El {
                name: qualified("v"),
                children: vec![Node::Text(number)],
                ..El::default()
            })],
        };
        let at = cache.children.iter().position(|node| {
            matches!(node, Node::El(next) if next.is("extLst") || (next.is("pt") && next.u32("idx").is_some_and(|next| next as usize > index)))
        }).unwrap_or(cache.children.len());
        cache.children.insert(at, Node::El(point));
    }
    Ok(())
}

fn address(formula: &str, index: usize) -> Option<Cell> {
    let (sheet, range) = formula.rsplit_once('!')?;
    let sheet = if let Some(quoted) = sheet.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        quoted.replace("''", "'")
    } else {
        sheet.to_owned()
    };
    if sheet.is_empty() || sheet.contains(['[', ']', ':']) {
        return None;
    }
    let (first, last) = range.split_once(':').unwrap_or((range, range));
    let (column, row) = split_cell(first)?;
    let (last_column, last_row) = split_cell(last)?;
    let row = row.checked_add(index)?;
    (column == last_column && row <= last_row).then(|| Cell {
        sheet,
        address: format!("{column}{row}"),
    })
}

fn split_cell(text: &str) -> Option<(&str, usize)> {
    let text = text.trim_start_matches('$');
    let end = text.bytes().position(|byte| !byte.is_ascii_uppercase())?;
    let (column, row) = text.split_at(end);
    let row: usize = row.trim_start_matches('$').parse().ok()?;
    (!column.is_empty() && column.len() <= 3 && (1..=1_048_576).contains(&row))
        .then_some((column, row))
}

fn rewrite_workbook(
    chart: &El,
    original: &str,
    destination: &str,
    changes: Changes,
    package: &opc::Package,
    parts: &mut PartEdits<'_>,
    types: &mut El,
) -> Result<Vec<u8>, PptxError> {
    let chart_rels = package.rels(original);
    let embedded = chart
        .child("externalData")
        .or_else(|| chart.path(&["chartData", "externalData"]))
        .and_then(|node| node.attr("r:id"))
        .and_then(|id| chart_rels.get(id))
        .filter(|edge| !edge.external && edge.kind == "package")
        .ok_or_else(|| error("chart has no editable embedded workbook"))?;
    if package.content_type(&embedded.target).as_deref() != Some(WORKBOOK_TYPE) {
        return Err(error(
            "chart embedded workbook is not an ordinary XLSX package",
        ));
    }
    let bytes = package
        .get(&embedded.target)
        .filter(|bytes| bytes.len() <= MAX_WORKBOOK)
        .ok_or_else(|| error("embedded chart workbook missing or too large"))?;
    let workbook = crate::archive::read_editable(
        bytes,
        &joyoffice_opc::Budget {
            max_entries: 1024,
            max_part_bytes: MAX_WORKBOOK as u64,
            max_total_bytes: 128 << 20,
            ..Default::default()
        },
    )?;
    if workbook
        .members()
        .any(|(name, ..)| name.starts_with("_xmlsignatures/"))
    {
        return Err(error("embedded workbook has signatures"));
    }
    let edited = write_sheets(&workbook, changes)?;
    let new_workbook = content::new_name(parts, &embedded.target);
    parts.insert(new_workbook.clone(), edited);
    add_override(types, &new_workbook, WORKBOOK_TYPE);
    let mut relations = package
        .xml(&opc::rels_path(original))
        .ok_or_else(|| error("chart relationships missing"))?;
    let edge = relations
        .root
        .elements_mut()
        .find(|edge| edge.attr("Id") == Some(&embedded.id))
        .ok_or_else(|| error("workbook relationship missing"))?;
    edge.set_attr(
        "Target",
        &crate::relationships::rebase(destination, &new_workbook, &embedded.target_raw),
    );
    Ok(relations.into_xml())
}

fn write_sheets(workbook: &opc::Package, changes: Changes) -> Result<Vec<u8>, PptxError> {
    let main = workbook
        .rels("")
        .items
        .into_iter()
        .find(|edge| edge.kind == "officeDocument" && !edge.external)
        .ok_or_else(|| error("workbook root relationship missing"))?;
    let doc = crate::read::load(workbook, &main.target)
        .ok_or_else(|| error("embedded workbook XML missing"))?;
    let sheets = doc
        .root
        .child("sheets")
        .ok_or_else(|| error("embedded workbook sheets missing"))?;
    let mut pending = BTreeMap::<String, BTreeMap<String, f64>>::new();
    for (cell, value) in changes.0 {
        pending
            .entry(cell.sheet)
            .or_default()
            .insert(cell.address, value);
    }
    let links = workbook.rels(&main.target);
    let mut changed = PartEdits::new(workbook);
    let mut used = BTreeSet::new();
    for (name, mut updates) in pending {
        let sheet = sheets
            .children_named("sheet")
            .find(|sheet| sheet.attr("name") == Some(&name))
            .and_then(|sheet| crate::presentation_parts::target(sheet, &links, "worksheet"))
            .ok_or_else(|| error("referenced workbook sheet missing"))?;
        if !used.insert(opc::norm(sheet)) {
            return Err(error("ambiguous workbook sheet binding"));
        }
        let mut doc =
            crate::read::load(workbook, sheet).ok_or_else(|| error("worksheet XML missing"))?;
        let data =
            child_mut(&mut doc.root, "sheetData").ok_or_else(|| error("worksheet data missing"))?;
        for cell in data
            .elements_mut()
            .filter(|row| row.is("row"))
            .flat_map(El::elements_mut)
            .filter(|cell| cell.is("c"))
        {
            let Some(value) = cell.attr("r").and_then(|address| updates.remove(address)) else {
                continue;
            };
            if cell.child("f").is_some() || cell.attr("t").is_some_and(|kind| kind != "n") {
                return Err(error(
                    "editing a formula or nonnumeric chart cell requires .joydeck Save As",
                ));
            }
            let target =
                child_mut(cell, "v").ok_or_else(|| error("numeric workbook cell is empty"))?;
            set_text(target, crate::chart_cache::format_number(value));
        }
        if !updates.is_empty() {
            return Err(error("referenced workbook cell missing"));
        }
        changed.insert(sheet.into(), doc.into_xml());
    }
    changed.finish()
}
