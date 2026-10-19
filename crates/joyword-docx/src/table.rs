//! Typed `w:tbl` subset (ECMA-376 Part 1 §17.4 tables, §17.4.58 `tblGrid`,
//! §17.4.66 `tr`, §17.4.64 `tc`, §17.4.17 `gridSpan`, §17.4.84 `vMerge`,
//! §17.4.73 `tcW`, §17.3.4 borders, §17.4.43 `tblCellMar`, §17.4.68 `tcMar`,
//! §17.4.6 `cantSplit`, §17.4.49 `tblHeader`, §17.4.52 `tblLayout`,
//! §17.4.28 `jc`, §17.4.50 `tblInd`, §17.18.2 border styles `nil` / `single` /
//! `double` / `dotted` / `dashed` / `thick`).
//!
//! A table is mapped only when every element and attribute is represented and
//! placed. Nested tables, `trHeight`, `vAlign`, revisions, bookmarks,
//! `tblLook`, theme tint or shade, and any other border style return `None`.
//! The caller preserves the whole `w:tbl` byte range.
//! This parser allocates no identifiers and records no fragments, so a
//! refusal leaves the document untouched.

use std::ops::Range;
use std::sync::Arc;

use joyword_model::{
    BlockSeq, BorderEdge, BorderKind, Cell, CellBorders, CellMargins, Color, Indent, Measure,
    OBJECT_MARKER, ParaProps, Paragraph, Row, RunProps, Shading, Spacing, StyleId, Table,
    TableAlign, TableLayoutKind, TableWidth, ThemeColorSlot, VerticalMerge,
};

use crate::import::Importer;
use crate::ns::W;
use crate::props::{self, Findings};
use crate::xml::{Attribute, Element, Node, XML_NAMESPACE, is_xml_space};
use joyoffice_opc::PartName;

struct DraftParagraph {
    style: Option<StyleId>,
    props: ParaProps,
    runs: Vec<(String, RunProps)>,
    span: Range<usize>,
}

struct CellProps {
    span: u32,
    merge: VerticalMerge,
    width: Option<Measure>,
    borders: CellBorders,
    shading: Option<Shading>,
    margin: Option<CellMargins>,
}

struct DraftCell {
    span: u32,
    merge: VerticalMerge,
    width: Option<Measure>,
    borders: CellBorders,
    shading: Option<Shading>,
    margin: Option<CellMargins>,
    paragraphs: Vec<DraftParagraph>,
}

struct DraftRow {
    before: u32,
    after: u32,
    header: bool,
    cant_split: bool,
    cells: Vec<DraftCell>,
}

struct DraftTable {
    style: Option<StyleId>,
    width: Option<TableWidth>,
    layout: Option<TableLayoutKind>,
    alignment: Option<TableAlign>,
    indent: Option<i64>,
    cell_margin: Option<CellMargins>,
    grid: Vec<Measure>,
    rows: Vec<DraftRow>,
}

struct TableProps {
    style: Option<StyleId>,
    width: Option<TableWidth>,
    layout: Option<TableLayoutKind>,
    alignment: Option<TableAlign>,
    indent: Option<i64>,
    cell_margin: Option<CellMargins>,
}

/// Maps `element` when it is entirely inside the subset. `None` means the
/// caller must preserve the original element.
pub(crate) fn import_table(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
) -> Option<Table> {
    let draft = parse_table(element)?;
    build(importer, part, element, draft)
}

fn build(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
    draft: DraftTable,
) -> Option<Table> {
    let mut rows = Vec::new();
    for row in draft.rows {
        let mut cells = Vec::new();
        for cell in row.cells {
            let mut blocks = Vec::new();
            for paragraph in cell.paragraphs {
                blocks.push(build_paragraph(importer, part, element, paragraph)?.into());
            }
            cells.push(Cell {
                id: importer.document.ids().node(),
                grid_span: cell.span,
                vertical_merge: cell.merge,
                width: cell.width,
                borders: cell.borders,
                shading: cell.shading,
                margin: cell.margin,
                vertical_align: None,
                blocks: BlockSeq::from_vec(blocks),
                preserved: None,
            });
        }
        rows.push(Row {
            id: importer.document.ids().node(),
            grid_before: row.before,
            grid_after: row.after,
            cells,
            header: row.header,
            cant_split: row.cant_split,
            height: None,
            preserved: None,
        });
    }
    Some(Table {
        id: importer.document.ids().node(),
        grid: draft.grid,
        rows,
        style: draft.style,
        width: draft.width,
        layout: draft.layout,
        alignment: draft.alignment,
        indent: draft.indent,
        cell_margin: draft.cell_margin,
        preserved: None,
        source: Some(Importer::source(part, element)),
    })
}

fn build_paragraph(
    importer: &mut Importer<'_>,
    part: &PartName,
    table: &Element,
    draft: DraftParagraph,
) -> Option<Paragraph> {
    let mut paragraph = Paragraph::new(importer.document.ids().node());
    paragraph.style = draft.style;
    paragraph.props = Arc::new(draft.props);
    if let Some(element) = find_span(table, &draft.span) {
        paragraph.source = Some(Importer::source(part, element));
    }
    for (text, props) in draft.runs {
        paragraph.push_text(&text, None, Arc::new(props)).ok()?;
    }
    Some(paragraph)
}

fn parse_table(element: &Element) -> Option<DraftTable> {
    if !element.is(W, "tbl") || !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut props = TableProps {
        style: None,
        width: None,
        layout: None,
        alignment: None,
        indent: None,
        cell_margin: None,
    };
    let mut saw_pr = false;
    let mut grid = None;
    let mut rows = Vec::new();
    let mut seen_row = false;
    for child in element.elements() {
        if child.is(W, "tblPr") && !seen_row && !saw_pr {
            saw_pr = true;
            props = parse_tbl_pr(child)?;
        } else if child.is(W, "tblGrid") && !seen_row && grid.is_none() {
            grid = Some(parse_grid(child)?);
        } else if child.is(W, "tr") {
            seen_row = true;
            rows.push(parse_row(child)?);
        } else {
            return None;
        }
    }
    let grid = grid?;
    if rows.is_empty() || grid.is_empty() {
        return None;
    }
    Some(DraftTable {
        style: props.style,
        width: props.width,
        layout: props.layout,
        alignment: props.alignment,
        indent: props.indent,
        cell_margin: props.cell_margin,
        grid,
        rows,
    })
}

fn parse_tbl_pr(element: &Element) -> Option<TableProps> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut props = TableProps {
        style: None,
        width: None,
        layout: None,
        alignment: None,
        indent: None,
        cell_margin: None,
    };
    for child in element.elements() {
        if child.is(W, "tblStyle") && props.style.is_none() {
            if !only_word_attributes(child, &["val"]) || !text_is_space(child) {
                return None;
            }
            let name = child.attribute(Some(W), "val")?;
            if name.is_empty() || name.chars().any(|ch| matches!(ch, '<' | '>' | '&' | '"')) {
                return None;
            }
            props.style = Some(StyleId::new(name));
        } else if child.is(W, "tblW") && props.width.is_none() {
            props.width = Some(parse_table_width(child)?);
        } else if child.is(W, "tblLayout") && props.layout.is_none() {
            props.layout = Some(parse_layout(child)?);
        } else if child.is(W, "jc") && props.alignment.is_none() {
            props.alignment = Some(parse_alignment(child)?);
        } else if child.is(W, "tblInd") && props.indent.is_none() {
            props.indent = Some(parse_indent(child)?);
        } else if child.is(W, "tblCellMar") && props.cell_margin.is_none() {
            props.cell_margin = Some(parse_cell_margins(child)?);
        } else {
            return None;
        }
    }
    Some(props)
}

fn parse_layout(element: &Element) -> Option<TableLayoutKind> {
    if !only_word_attributes(element, &["type"]) || !text_is_space(element) {
        return None;
    }
    match element.attribute(Some(W), "type")? {
        "fixed" => Some(TableLayoutKind::Fixed),
        "autofit" => Some(TableLayoutKind::Autofit),
        _ => None,
    }
}

fn parse_alignment(element: &Element) -> Option<TableAlign> {
    if !only_word_attributes(element, &["val"]) || !text_is_space(element) {
        return None;
    }
    match element.attribute(Some(W), "val")? {
        "left" => Some(TableAlign::Left),
        "center" => Some(TableAlign::Center),
        "right" => Some(TableAlign::Right),
        _ => None,
    }
}

fn parse_indent(element: &Element) -> Option<i64> {
    if !only_word_attributes(element, &["w", "type"]) || !text_is_space(element) {
        return None;
    }
    match element.attribute(Some(W), "type")? {
        "nil" => Some(0),
        "dxa" => {
            let width = integer_attr(element, "w")?;
            (width >= 0).then_some(width)
        }
        _ => None,
    }
}

fn parse_cell_margins(element: &Element) -> Option<CellMargins> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut top = None;
    let mut left = None;
    let mut bottom = None;
    let mut right = None;
    let mut start = None;
    let mut end = None;
    for child in element.elements() {
        let value = parse_margin_edge(child)?;
        let slot = match &*child.local {
            "top" if top.is_none() => &mut top,
            "left" if left.is_none() => &mut left,
            "bottom" if bottom.is_none() => &mut bottom,
            "right" if right.is_none() => &mut right,
            "start" if start.is_none() => &mut start,
            "end" if end.is_none() => &mut end,
            _ => return None,
        };
        *slot = Some(value);
    }
    // `bidiVisual` is outside this subset, so start/end are the left/right
    // edges. Both names together would be ambiguous and stay opaque.
    if (left.is_some() && start.is_some()) || (right.is_some() && end.is_some()) {
        return None;
    }
    // An omitted edge is 0 when the margin element itself is present.
    Some(CellMargins {
        top: top.unwrap_or(0),
        left: left.or(start).unwrap_or(0),
        bottom: bottom.unwrap_or(0),
        right: right.or(end).unwrap_or(0),
    })
}

fn parse_margin_edge(element: &Element) -> Option<i64> {
    if !only_word_attributes(element, &["w", "type"]) || !text_is_space(element) {
        return None;
    }
    match element.attribute(Some(W), "type")? {
        "nil" => Some(0),
        "dxa" => {
            let width = integer_attr(element, "w")?;
            (width >= 0).then_some(width)
        }
        _ => None,
    }
}

fn parse_table_width(element: &Element) -> Option<TableWidth> {
    if !only_word_attributes(element, &["w", "type"]) || !text_is_space(element) {
        return None;
    }
    match element.attribute(Some(W), "type")? {
        "auto" => Some(TableWidth::Auto),
        "dxa" => Some(TableWidth::Dxa(integer_attr(element, "w")?)),
        "pct" => {
            let width = integer_attr(element, "w")?;
            (width > 0).then_some(TableWidth::Pct(width))
        }
        _ => None,
    }
}

fn parse_grid(element: &Element) -> Option<Vec<Measure>> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut columns = Vec::new();
    for child in element.elements() {
        if !child.is(W, "gridCol") || !only_word_attributes(child, &["w"]) || !text_is_space(child)
        {
            return None;
        }
        let width = integer_attr(child, "w")?;
        if width < 0 {
            return None;
        }
        columns.push(Measure::Twips(width));
    }
    (!columns.is_empty()).then_some(columns)
}

fn parse_row(element: &Element) -> Option<DraftRow> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut before = 0u32;
    let mut after = 0u32;
    let mut header = false;
    let mut cant_split = false;
    let mut saw_pr = false;
    let mut cells = Vec::new();
    for child in element.elements() {
        if child.is(W, "trPr") && !saw_pr && cells.is_empty() {
            saw_pr = true;
            (before, after, header, cant_split) = parse_tr_pr(child)?;
        } else if child.is(W, "tc") {
            cells.push(parse_cell(child)?);
        } else {
            return None;
        }
    }
    (!cells.is_empty()).then_some(DraftRow {
        before,
        after,
        header,
        cant_split,
        cells,
    })
}

fn parse_tr_pr(element: &Element) -> Option<(u32, u32, bool, bool)> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut before = None;
    let mut after = None;
    let mut header = None;
    let mut cant_split = None;
    for child in element.elements() {
        if child.is(W, "gridBefore") && before.is_none() {
            before = Some(decimal_val(child)?);
        } else if child.is(W, "gridAfter") && after.is_none() {
            after = Some(decimal_val(child)?);
        } else if child.is(W, "cantSplit") && cant_split.is_none() {
            cant_split = Some(on_off_element(child)?);
        } else if child.is(W, "tblHeader") && header.is_none() {
            header = Some(on_off_element(child)?);
        } else {
            return None;
        }
    }
    Some((
        before.unwrap_or(0),
        after.unwrap_or(0),
        header.unwrap_or(false),
        cant_split.unwrap_or(false),
    ))
}

fn parse_cell(element: &Element) -> Option<DraftCell> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut span = 1u32;
    let mut merge = VerticalMerge::None;
    let mut width = None;
    let mut borders = CellBorders::default();
    let mut shading = None;
    let mut margin = None;
    let mut saw_pr = false;
    let mut paragraphs = Vec::new();
    for child in element.elements() {
        if child.is(W, "tcPr") && !saw_pr && paragraphs.is_empty() {
            saw_pr = true;
            let props = parse_tc_pr(child)?;
            span = props.span;
            merge = props.merge;
            width = props.width;
            borders = props.borders;
            shading = props.shading;
            margin = props.margin;
        } else if child.is(W, "p") {
            paragraphs.push(parse_paragraph(child)?);
        } else {
            return None;
        }
    }
    if paragraphs.is_empty() {
        return None;
    }
    Some(DraftCell {
        span,
        merge,
        width,
        borders,
        shading,
        margin,
        paragraphs,
    })
}

fn parse_tc_pr(element: &Element) -> Option<CellProps> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut span = None;
    let mut merge = None;
    let mut width = None;
    let mut borders = None;
    let mut shading = None;
    let mut margin = None;
    for child in element.elements() {
        if child.is(W, "gridSpan") && span.is_none() {
            let value = decimal_val(child)?;
            if value == 0 {
                return None;
            }
            span = Some(value);
        } else if child.is(W, "vMerge") && merge.is_none() {
            merge = Some(parse_vmerge(child)?);
        } else if child.is(W, "tcW") && width.is_none() {
            width = Some(parse_cell_width(child)?);
        } else if child.is(W, "tcBorders") && borders.is_none() {
            borders = Some(parse_borders(child)?);
        } else if child.is(W, "shd") && shading.is_none() {
            shading = Some(parse_shading(child)?);
        } else if child.is(W, "tcMar") && margin.is_none() {
            margin = Some(parse_cell_margins(child)?);
        } else {
            return None;
        }
    }
    Some(CellProps {
        span: span.unwrap_or(1),
        merge: merge.unwrap_or(VerticalMerge::None),
        width,
        borders: borders.unwrap_or_default(),
        shading,
        margin,
    })
}

fn parse_vmerge(element: &Element) -> Option<VerticalMerge> {
    if !only_word_attributes(element, &["val"]) || !text_is_space(element) {
        return None;
    }
    // ECMA-376 §17.4.84: an omitted `w:val` means `continue`.
    match element.attribute(Some(W), "val") {
        Some("restart") => Some(VerticalMerge::Restart),
        None | Some("continue") => Some(VerticalMerge::Continue),
        Some(_) => None,
    }
}

fn parse_cell_width(element: &Element) -> Option<Measure> {
    if !only_word_attributes(element, &["w", "type"]) || !text_is_space(element) {
        return None;
    }
    match element.attribute(Some(W), "type")? {
        "auto" => Some(Measure::Auto),
        "dxa" => {
            let width = integer_attr(element, "w")?;
            (width >= 0).then_some(Measure::Twips(width))
        }
        "pct" => {
            let width = integer_attr(element, "w")?;
            (width > 0).then_some(Measure::PercentFiftieths(width))
        }
        _ => None,
    }
}

fn parse_borders(element: &Element) -> Option<CellBorders> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut borders = CellBorders::default();
    for child in element.elements() {
        let edge = parse_edge(child)?;
        let slot = match &*child.local {
            "top" if borders.top.is_none() => &mut borders.top,
            "left" if borders.left.is_none() => &mut borders.left,
            "bottom" if borders.bottom.is_none() => &mut borders.bottom,
            "right" if borders.right.is_none() => &mut borders.right,
            _ => return None,
        };
        *slot = Some(edge);
    }
    Some(borders)
}

fn parse_edge(element: &Element) -> Option<BorderEdge> {
    if !element.namespace.as_deref().is_some_and(|ns| ns == W)
        || !only_word_attributes(element, &["val", "sz", "color", "themeColor"])
        || !text_is_space(element)
    {
        return None;
    }
    let kind = match element.attribute(Some(W), "val")? {
        "nil" => BorderKind::Nil,
        "single" => BorderKind::Single,
        "double" => BorderKind::Double,
        "dotted" => BorderKind::Dotted,
        "dashed" => BorderKind::Dashed,
        "thick" => BorderKind::Thick,
        _ => return None,
    };
    let size_eighths = match element.attribute(Some(W), "sz") {
        None => 4,
        Some(value) => value.parse().ok()?,
    };
    // A theme slot wins over a cached hex. Tint and shade are not in the
    // allowed attribute list, so those borders keep the table opaque.
    let color = if let Some(name) = element.attribute(Some(W), "themeColor") {
        Color::Theme(ThemeColorSlot::from_word_name(name)?)
    } else {
        match element.attribute(Some(W), "color") {
            None | Some("auto") => Color::Auto,
            Some(value) => rgb_color(value)?,
        }
    };
    Some(BorderEdge {
        kind,
        size_eighths,
        color,
    })
}

fn parse_shading(element: &Element) -> Option<Shading> {
    if !only_word_attributes(element, &["val", "color", "fill"]) || !text_is_space(element) {
        return None;
    }
    if element.attribute(Some(W), "val")? != "clear" {
        return None;
    }
    if let Some(color) = element.attribute(Some(W), "color")
        && color != "auto"
    {
        return None;
    }
    let fill = match element.attribute(Some(W), "fill")? {
        "auto" => Color::Auto,
        value => rgb_color(value)?,
    };
    Some(Shading { fill })
}

fn parse_paragraph(element: &Element) -> Option<DraftParagraph> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut style = None;
    let mut props = ParaProps::default();
    let mut runs = Vec::new();
    let mut saw_ppr = false;
    for child in element.elements() {
        if child.is(W, "pPr") && !saw_ppr && runs.is_empty() {
            saw_ppr = true;
            (style, props) = parse_ppr(child)?;
        } else if child.is(W, "r") {
            runs.push(parse_run(child)?);
        } else {
            return None;
        }
    }
    Some(DraftParagraph {
        style,
        props,
        runs,
        span: element.span.clone(),
    })
}

fn find_span<'a>(element: &'a Element, span: &Range<usize>) -> Option<&'a Element> {
    if element.span == *span {
        return Some(element);
    }
    element.elements().find_map(|child| find_span(child, span))
}

fn parse_ppr(element: &Element) -> Option<(Option<StyleId>, ParaProps)> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut findings = Findings::default();
    let parsed = props::para(element, &mut findings);
    if !findings.complete || parsed.mark.is_some() || parsed.section.is_some() {
        return None;
    }
    let props = parsed.props;
    let plain = props.indent == Indent::default()
        && props.spacing == Spacing::default()
        && props.keep_next.is_none()
        && props.keep_lines.is_none()
        && props.widow_control.is_none()
        && props.page_break_before.is_none()
        && props.snap_to_grid.is_none()
        && props.outline_level.is_none()
        && props.numbering.is_none()
        && props.tabs.is_none()
        && props.preserved.is_none();
    plain.then_some((parsed.style, props))
}

fn parse_run(element: &Element) -> Option<(String, RunProps)> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut props = RunProps::default();
    let mut text = String::new();
    let mut saw_rpr = false;
    for child in element.elements() {
        if child.is(W, "rPr") && !saw_rpr && text.is_empty() {
            saw_rpr = true;
            props = parse_rpr(child)?;
        } else if child.is(W, "t") && child.elements().next().is_none() && t_attributes_ok(child) {
            let piece = child.text();
            if piece.contains('\u{FFFC}') || piece.contains(OBJECT_MARKER) {
                return None;
            }
            text.push_str(&piece);
        } else if child.is(W, "tab") && only_word_attributes(child, &[]) && text_is_space(child) {
            text.push('\t');
        } else {
            return None;
        }
    }
    Some((text, props))
}

fn parse_rpr(element: &Element) -> Option<RunProps> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut findings = Findings::default();
    let (props, style) = props::run(element, &mut findings);
    if !findings.complete || style.is_some() || props.preserved.is_some() {
        return None;
    }
    let allowed = props.strike.is_none()
        && props.hidden.is_none()
        && props.underline.is_none()
        && props.fonts == joyword_model::FontSlots::default()
        && props.languages == joyword_model::Languages::default()
        && props
            .size
            .is_none_or(|size| matches!(size, Measure::HalfPoints(_)))
        && props
            .color
            .is_none_or(|color| matches!(color, Color::Auto | Color::Rgb(_, _, _)));
    allowed.then_some(props)
}

fn t_attributes_ok(element: &Element) -> bool {
    element.attributes.iter().all(|attribute| {
        attribute.namespace.as_deref() == Some(XML_NAMESPACE) && attribute.local.as_ref() == "space"
    })
}

fn decimal_val(element: &Element) -> Option<u32> {
    if !only_word_attributes(element, &["val"]) || !text_is_space(element) {
        return None;
    }
    let value = integer_attr(element, "val")?;
    u32::try_from(value).ok()
}

fn on_off_element(element: &Element) -> Option<bool> {
    if !only_word_attributes(element, &["val"]) || !text_is_space(element) {
        return None;
    }
    props::on_off(element.attribute(Some(W), "val"))
}

fn integer_attr(element: &Element, name: &str) -> Option<i64> {
    element.attribute(Some(W), name)?.parse().ok()
}

fn rgb_color(value: &str) -> Option<Color> {
    if value.len() != 6 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    let number = u32::from_str_radix(value, 16).ok()?;
    Some(Color::Rgb(
        (number >> 16) as u8,
        (number >> 8) as u8,
        number as u8,
    ))
}

fn only_word_attributes(element: &Element, allowed: &[&str]) -> bool {
    element
        .attributes
        .iter()
        .all(|attribute| word_allowed(attribute, allowed))
}

fn word_allowed(attribute: &Attribute, allowed: &[&str]) -> bool {
    attribute.namespace.as_deref() == Some(W)
        && allowed.iter().any(|name| attribute.local.as_ref() == *name)
}

fn text_is_space(element: &Element) -> bool {
    element.children.iter().all(|node| match node {
        Node::Element(_) => true,
        Node::Text(text) => text.chars().all(is_xml_space),
    })
}
