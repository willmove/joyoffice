//! Typed `w:tbl` subset (ECMA-376 Part 1 §17.4 tables, §17.4.58 `tblGrid`,
//! §17.4.66 `tr`, §17.4.64 `tc`, §17.4.17 `gridSpan`, §17.4.84 `vMerge`,
//! §17.4.73 `tcW`, §17.3.4 borders, §17.4.43 `tblCellMar`, §17.4.68 `tcMar`,
//! §17.4.6 `cantSplit`, §17.4.49 `tblHeader`, §17.4.52 `tblLayout`,
//! §17.4.28 `jc`, §17.4.50 `tblInd`, §17.4.56 `tblLook`, §17.4.80 `trHeight`,
//! §17.4.83 `vAlign`, §17.18.2 border styles `nil` / `single` / `double` /
//! `dotted` / `dashed` / `thick`, §17.18.37 `ST_HeightRule`).
//!
//! A table is mapped only when every element and attribute is represented and
//! placed. One nested `w:tbl` inside a cell is part of this subset when that
//! inner table meets the same rules. A second nested table, `vAlign` `both`,
//! a non-positive `exact` row height, structural revisions (`w:cellIns`,
//! `w:cellDel`, `w:cellMerge`, `w:tblGridChange`), bookmarks, theme tint or
//! shade, and any other border style return `None` for the whole outer
//! table. A `w:tblPrChange`, `w:trPrChange` or `w:tcPrChange` whose previous
//! properties are fully representable is typed as a pending property
//! revision; anything else about the change refuses the whole table. A plain
//! `w:ins`, `w:del`, `w:moveFrom` or `w:moveTo` around typed runs is a text
//! revision on those runs.
//! A cell paragraph may carry simple `w:hyperlink` runs (text and tabs,
//! `r:id` and/or `w:anchor`), the same subset the body parser maps.
//! `w:tblLook`, `top` / `center` / `bottom` and `auto` / `atLeast` / positive
//! `exact` are stored. The caller preserves the whole `w:tbl` byte range.
//! This parser allocates no identifiers and records no fragments, so a
//! refusal leaves the document untouched.

use std::ops::Range;
use std::sync::Arc;

use joyword_model::{
    BlockSeq, BorderEdge, BorderKind, Cell, CellBorders, CellChangeProps, CellMargins, Color,
    Hyperlink, Indent, Measure, OBJECT_MARKER, ParaProps, Paragraph, PreviousTableProps,
    RevisionKind, Row, RowChangeProps, RowHeight, RowHeightRule, RunProps, Shading, Spacing, Style,
    StyleId, Table, TableAlign, TableChangeProps, TableCondition, TableConditional, TableFormat,
    TableLayoutKind, TableLook, TablePropsRevision, TableWidth, TextRevision, ThemeColorSlot,
    VerticalAlign, VerticalMerge,
};

use crate::import::Importer;
use crate::ns::W;
use crate::props::{self, Findings};
use crate::xml::{Attribute, Element, Node, XML_NAMESPACE, is_xml_space};
use joyoffice_opc::PartName;

struct DraftRun {
    text: String,
    props: RunProps,
    link: Option<Arc<Hyperlink>>,
    revision: Option<TextRevision>,
}

struct DraftParagraph {
    style: Option<StyleId>,
    props: ParaProps,
    runs: Vec<DraftRun>,
    span: Range<usize>,
}

struct CellProps {
    span: u32,
    merge: VerticalMerge,
    width: Option<Measure>,
    borders: CellBorders,
    shading: Option<Shading>,
    margin: Option<CellMargins>,
    vertical_align: Option<VerticalAlign>,
    props_revision: Option<TablePropsRevision>,
}

struct RowProps {
    before: u32,
    after: u32,
    header: bool,
    cant_split: bool,
    height: Option<RowHeight>,
}

enum DraftBlock {
    Paragraph(DraftParagraph),
    Table(DraftTable),
}

struct DraftCell {
    span: u32,
    merge: VerticalMerge,
    width: Option<Measure>,
    borders: CellBorders,
    shading: Option<Shading>,
    margin: Option<CellMargins>,
    vertical_align: Option<VerticalAlign>,
    blocks: Vec<DraftBlock>,
    props_revision: Option<TablePropsRevision>,
}

/// Body tables are depth 0. A table inside a cell is depth 1. Depth 2 stays
/// outside this slice, so the whole outer table remains opaque.
const MAX_NESTED_DEPTH: u8 = 1;

struct DraftRow {
    before: u32,
    after: u32,
    header: bool,
    cant_split: bool,
    height: Option<RowHeight>,
    cells: Vec<DraftCell>,
    props_revision: Option<TablePropsRevision>,
}

struct DraftTable {
    style: Option<StyleId>,
    width: Option<TableWidth>,
    layout: Option<TableLayoutKind>,
    alignment: Option<TableAlign>,
    indent: Option<i64>,
    look: Option<TableLook>,
    cell_margin: Option<CellMargins>,
    grid: Vec<Measure>,
    rows: Vec<DraftRow>,
    span: Range<usize>,
    props_revision: Option<TablePropsRevision>,
}

struct TableProps {
    style: Option<StyleId>,
    width: Option<TableWidth>,
    layout: Option<TableLayoutKind>,
    alignment: Option<TableAlign>,
    indent: Option<i64>,
    look: Option<TableLook>,
    cell_margin: Option<CellMargins>,
}

impl TableProps {
    fn previous(self) -> TableChangeProps {
        TableChangeProps {
            style: self.style,
            width: self.width,
            layout: self.layout,
            alignment: self.alignment,
            indent: self.indent,
            look: self.look,
            cell_margin: self.cell_margin,
        }
    }
}

/// Maps `element` when it is entirely inside the subset. `None` means the
/// caller must preserve the original element.
pub(crate) fn import_table(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
) -> Option<Table> {
    let draft = parse_table(importer, part, element, 0)?;
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
            for block in cell.blocks {
                match block {
                    DraftBlock::Paragraph(paragraph) => {
                        blocks.push(build_paragraph(importer, part, element, paragraph)?.into());
                    }
                    DraftBlock::Table(nested) => {
                        blocks.push(build(importer, part, element, nested)?.into());
                    }
                }
            }
            if blocks.is_empty() {
                return None;
            }
            if let Some(revision) = &cell.props_revision {
                importer.document.note_revision_id(revision.id);
            }
            cells.push(Cell {
                id: importer.document.ids().node(),
                grid_span: cell.span,
                vertical_merge: cell.merge,
                width: cell.width,
                borders: cell.borders,
                shading: cell.shading,
                margin: cell.margin,
                vertical_align: cell.vertical_align,
                blocks: BlockSeq::from_vec(blocks),
                preserved: None,
                props_revision: cell.props_revision.map(Arc::new),
            });
        }
        if let Some(revision) = &row.props_revision {
            importer.document.note_revision_id(revision.id);
        }
        rows.push(Row {
            id: importer.document.ids().node(),
            grid_before: row.before,
            grid_after: row.after,
            cells,
            header: row.header,
            cant_split: row.cant_split,
            height: row.height,
            preserved: None,
            props_revision: row.props_revision.map(Arc::new),
        });
    }
    if let Some(revision) = &draft.props_revision {
        importer.document.note_revision_id(revision.id);
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
        look: draft.look,
        cell_margin: draft.cell_margin,
        preserved: None,
        props_revision: draft.props_revision.map(Arc::new),
        source: Some(Importer::source(
            part,
            find_span(element, &draft.span).unwrap_or(element),
        )),
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
    for run in draft.runs {
        match run.revision {
            Some(revision) => {
                importer.document.note_revision_id(revision.id);
                paragraph
                    .push_text_marked(&run.text, None, Arc::new(run.props), revision)
                    .ok()?;
            }
            None => paragraph
                .push_text(&run.text, None, Arc::new(run.props))
                .ok()?,
        }
        if run.link.is_some() {
            paragraph.set_last_run_link(run.link);
        }
    }
    Some(paragraph)
}

fn parse_table(
    importer: &Importer<'_>,
    part: &PartName,
    element: &Element,
    depth: u8,
) -> Option<DraftTable> {
    if !element.is(W, "tbl") || !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut props = TableProps {
        style: None,
        width: None,
        layout: None,
        alignment: None,
        indent: None,
        look: None,
        cell_margin: None,
    };
    let mut props_revision = None;
    let mut saw_pr = false;
    let mut grid = None;
    let mut rows = Vec::new();
    let mut seen_row = false;
    for child in element.elements() {
        if child.is(W, "tblPr") && !seen_row && !saw_pr {
            saw_pr = true;
            (props, props_revision) = parse_tbl_pr(child)?;
        } else if child.is(W, "tblGrid") && !seen_row && grid.is_none() {
            grid = Some(parse_grid(child)?);
        } else if child.is(W, "tr") {
            seen_row = true;
            rows.push(parse_row(importer, part, child, depth)?);
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
        look: props.look,
        cell_margin: props.cell_margin,
        grid,
        rows,
        span: element.span.clone(),
        props_revision,
    })
}

fn parse_tbl_pr(element: &Element) -> Option<(TableProps, Option<TablePropsRevision>)> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut props = TableProps {
        style: None,
        width: None,
        layout: None,
        alignment: None,
        indent: None,
        look: None,
        cell_margin: None,
    };
    let mut change = None;
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
        } else if child.is(W, "tblLook") && props.look.is_none() {
            props.look = Some(parse_look(child)?);
        } else if child.is(W, "tblCellMar") && props.cell_margin.is_none() {
            props.cell_margin = Some(parse_cell_margins(child)?);
        } else if child.is(W, "tblPrChange") && change.is_none() {
            change = Some(parse_props_change(child, PropsChangeLevel::Table)?);
        } else {
            return None;
        }
    }
    Some((props, change))
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

/// `w:tblLook`. A present attribute overrides the same bit of `w:val`.
/// An omitted attribute is false when the element is present. An invalid
/// value refuses the table.
fn parse_look(element: &Element) -> Option<TableLook> {
    const FLAGS: &[(&str, u16)] = &[
        ("firstRow", 0x0020),
        ("lastRow", 0x0040),
        ("firstColumn", 0x0080),
        ("lastColumn", 0x0100),
        ("noHBand", 0x0200),
        ("noVBand", 0x0400),
    ];
    if !only_word_attributes(
        element,
        &[
            "val",
            "firstRow",
            "lastRow",
            "firstColumn",
            "lastColumn",
            "noHBand",
            "noVBand",
        ],
    ) || !text_is_space(element)
    {
        return None;
    }
    let bits = match element.attribute(Some(W), "val") {
        None => 0u16,
        Some(value) => u16::from_str_radix(value, 16).ok()?,
    };
    let mut flags = [false; 6];
    for (index, (_, mask)) in FLAGS.iter().enumerate() {
        flags[index] = bits & mask != 0;
    }
    for (index, (name, _)) in FLAGS.iter().enumerate() {
        if let Some(value) = element.attribute(Some(W), name) {
            flags[index] = match value {
                "true" | "1" | "on" => true,
                "false" | "0" | "off" => false,
                _ => return None,
            };
        }
    }
    Some(TableLook {
        first_row: flags[0],
        last_row: flags[1],
        first_column: flags[2],
        last_column: flags[3],
        no_h_band: flags[4],
        no_v_band: flags[5],
    })
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

fn parse_row(
    importer: &Importer<'_>,
    part: &PartName,
    element: &Element,
    depth: u8,
) -> Option<DraftRow> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut before = 0u32;
    let mut after = 0u32;
    let mut header = false;
    let mut cant_split = false;
    let mut height = None;
    let mut props_revision = None;
    let mut saw_pr = false;
    let mut cells = Vec::new();
    for child in element.elements() {
        if child.is(W, "trPr") && !saw_pr && cells.is_empty() {
            saw_pr = true;
            let (row, change) = parse_tr_pr(child)?;
            before = row.before;
            after = row.after;
            header = row.header;
            cant_split = row.cant_split;
            height = row.height;
            props_revision = change;
        } else if child.is(W, "tc") {
            cells.push(parse_cell(importer, part, child, depth)?);
        } else {
            return None;
        }
    }
    (!cells.is_empty()).then_some(DraftRow {
        before,
        after,
        header,
        cant_split,
        height,
        cells,
        props_revision,
    })
}

fn parse_tr_pr(element: &Element) -> Option<(RowProps, Option<TablePropsRevision>)> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut before = None;
    let mut after = None;
    let mut header = None;
    let mut cant_split = None;
    let mut height = None;
    let mut change = None;
    for child in element.elements() {
        if child.is(W, "gridBefore") && before.is_none() {
            before = Some(decimal_val(child)?);
        } else if child.is(W, "gridAfter") && after.is_none() {
            after = Some(decimal_val(child)?);
        } else if child.is(W, "cantSplit") && cant_split.is_none() {
            cant_split = Some(on_off_element(child)?);
        } else if child.is(W, "tblHeader") && header.is_none() {
            header = Some(on_off_element(child)?);
        } else if child.is(W, "trHeight") && height.is_none() {
            height = Some(parse_row_height(child)?);
        } else if child.is(W, "trPrChange") && change.is_none() {
            change = Some(parse_props_change(child, PropsChangeLevel::Row)?);
        } else {
            return None;
        }
    }
    let row = RowProps {
        before: before.unwrap_or(0),
        after: after.unwrap_or(0),
        header: header.unwrap_or(false),
        cant_split: cant_split.unwrap_or(false),
        height,
    };
    Some((row, change))
}

/// `w:trHeight` (ECMA-376 §17.4.80). An omitted `w:hRule` is `auto`
/// (§17.18.37), and `auto` does not use `w:val` when the row is placed.
/// An omitted `w:val` is stored as 0. `atLeast` refuses a negative value.
/// `exact` refuses a missing or non-positive value so this slice does not
/// invent a zero-height row.
fn parse_row_height(element: &Element) -> Option<RowHeight> {
    if !only_word_attributes(element, &["val", "hRule"]) || !text_is_space(element) {
        return None;
    }
    let rule = match element.attribute(Some(W), "hRule") {
        None | Some("auto") => RowHeightRule::Auto,
        Some("atLeast") => RowHeightRule::AtLeast,
        Some("exact") => RowHeightRule::Exact,
        Some(_) => return None,
    };
    let twips = match element.attribute(Some(W), "val") {
        None => 0,
        Some(_) => integer_attr(element, "val")?,
    };
    match rule {
        RowHeightRule::Auto => {}
        RowHeightRule::AtLeast if twips >= 0 => {}
        RowHeightRule::Exact if twips > 0 => {}
        RowHeightRule::AtLeast | RowHeightRule::Exact => return None,
    }
    Some(RowHeight { twips, rule })
}

fn parse_cell(
    importer: &Importer<'_>,
    part: &PartName,
    element: &Element,
    depth: u8,
) -> Option<DraftCell> {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return None;
    }
    let mut span = 1u32;
    let mut merge = VerticalMerge::None;
    let mut width = None;
    let mut borders = CellBorders::default();
    let mut shading = None;
    let mut margin = None;
    let mut vertical_align = None;
    let mut props_revision = None;
    let mut saw_pr = false;
    let mut blocks = Vec::new();
    for child in element.elements() {
        if child.is(W, "tcPr") && !saw_pr && blocks.is_empty() {
            saw_pr = true;
            let props = parse_tc_pr(child)?;
            span = props.span;
            merge = props.merge;
            width = props.width;
            borders = props.borders;
            shading = props.shading;
            margin = props.margin;
            vertical_align = props.vertical_align;
            props_revision = props.props_revision;
        } else if child.is(W, "p") {
            blocks.push(DraftBlock::Paragraph(parse_paragraph(
                importer, part, child,
            )?));
        } else if child.is(W, "tbl") {
            // ECMA-376 §17.4.64 allows a nested `w:tbl` as cell content.
            // Only one level is typed. A deeper table, or an inner table
            // outside the subset, refuses the whole outer table.
            if depth >= MAX_NESTED_DEPTH {
                return None;
            }
            blocks.push(DraftBlock::Table(parse_table(
                importer,
                part,
                child,
                depth + 1,
            )?));
        } else {
            return None;
        }
    }
    if blocks.is_empty() {
        return None;
    }
    Some(DraftCell {
        span,
        merge,
        width,
        borders,
        shading,
        margin,
        vertical_align,
        blocks,
        props_revision,
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
    let mut vertical_align = None;
    let mut change = None;
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
        } else if child.is(W, "vAlign") && vertical_align.is_none() {
            vertical_align = Some(parse_valign(child)?);
        } else if child.is(W, "tcPrChange") && change.is_none() {
            change = Some(parse_props_change(child, PropsChangeLevel::Cell)?);
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
        vertical_align,
        props_revision: change,
    })
}

/// Which level a `w:*PrChange` records previous properties for.
enum PropsChangeLevel {
    Table,
    Row,
    Cell,
}

/// Parses one `w:tblPrChange`, `w:trPrChange` or `w:tcPrChange` into a typed
/// [`TablePropsRevision`] holding the pre-change properties. `None` means the
/// change is outside the subset (illegal tracking attributes, more than one
/// previous-property child, or a previous property that is not fully
/// representable); the caller then refuses the whole table, which stays
/// preserved. Nothing is dropped silently.
fn parse_props_change(element: &Element, level: PropsChangeLevel) -> Option<TablePropsRevision> {
    let (id, author, date) = crate::revision::track_attrs(element, false)?;
    if !text_is_space(element) {
        return None;
    }
    let child_name = match level {
        PropsChangeLevel::Table => "tblPr",
        PropsChangeLevel::Row => "trPr",
        PropsChangeLevel::Cell => "tcPr",
    };
    let mut children = element.elements();
    let previous = match children.next() {
        None => None,
        Some(child) => {
            // §17.13.5: exactly one optional previous-property child.
            if !child.is(W, child_name) || children.next().is_some() {
                return None;
            }
            Some(child)
        }
    };
    let previous = match level {
        PropsChangeLevel::Table => PreviousTableProps::Table(match previous {
            None => TableChangeProps::default(),
            Some(tbl_pr) => parse_tbl_pr(tbl_pr)?.0.previous(),
        }),
        PropsChangeLevel::Row => PreviousTableProps::Row(match previous {
            None => RowChangeProps::default(),
            Some(tr_pr) => {
                let (row, change) = parse_tr_pr(tr_pr)?;
                // A nested `w:trPrChange` inside the previous row properties
                // is out of subset.
                if change.is_some() {
                    return None;
                }
                RowChangeProps {
                    grid_before: row.before,
                    grid_after: row.after,
                    header: row.header,
                    cant_split: row.cant_split,
                    height: row.height,
                }
            }
        }),
        PropsChangeLevel::Cell => PreviousTableProps::Cell(match previous {
            None => CellChangeProps::default(),
            Some(tc_pr) => {
                let props = parse_tc_pr(tc_pr)?;
                // A nested `w:tcPrChange` inside the previous cell properties
                // is out of subset.
                if props.props_revision.is_some() {
                    return None;
                }
                CellChangeProps {
                    grid_span: props.span,
                    vertical_merge: props.merge,
                    width: props.width,
                    borders: props.borders,
                    shading: props.shading,
                    margin: props.margin,
                    vertical_align: props.vertical_align,
                }
            }
        }),
    };
    Some(TablePropsRevision {
        id,
        author,
        date,
        previous,
    })
}

/// `w:vAlign` (ECMA-376 §17.4.83). `both` is vertical justification and stays
/// outside this subset, so the whole table remains opaque. An omitted `w:val`
/// is also refused: this slice does not substitute `top`.
fn parse_valign(element: &Element) -> Option<VerticalAlign> {
    if !only_word_attributes(element, &["val"]) || !text_is_space(element) {
        return None;
    }
    match element.attribute(Some(W), "val")? {
        "top" => Some(VerticalAlign::Top),
        "center" => Some(VerticalAlign::Center),
        "bottom" => Some(VerticalAlign::Bottom),
        _ => None,
    }
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
        Color::theme(ThemeColorSlot::from_word_name(name)?)
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

fn parse_paragraph(
    importer: &Importer<'_>,
    part: &PartName,
    element: &Element,
) -> Option<DraftParagraph> {
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
            let (text, props) = parse_run(child, false)?;
            runs.push(DraftRun {
                text,
                props,
                link: None,
                revision: None,
            });
        } else if child.is(W, "hyperlink") {
            // The same simple subset the body parser maps: text and tab runs
            // under `r:id` and/or `w:anchor`. A complex wrapper refuses the
            // whole table, which stays preserved instead.
            let link = crate::hyperlink::mapped(importer, part, child)?;
            for run_element in child.elements() {
                let (text, props) = parse_run(run_element, false)?;
                runs.push(DraftRun {
                    text,
                    props,
                    link: Some(Arc::new(link.clone())),
                    revision: None,
                });
            }
        } else {
            let kind = crate::revision::revision_kind(child.local.as_ref())?;
            let mark = crate::revision::cell_text_revision(child, kind)?;
            let deleted = matches!(kind, RevisionKind::Delete | RevisionKind::MoveFrom);
            for run in child.elements() {
                if run.is(W, "proofErr") {
                    continue;
                }
                if !run.is(W, "r") {
                    return None;
                }
                let (text, props) = parse_run(run, deleted)?;
                runs.push(DraftRun {
                    text,
                    props,
                    link: None,
                    revision: Some(mark.clone()),
                });
            }
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

fn parse_run(element: &Element, deleted: bool) -> Option<(String, RunProps)> {
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
        } else if child.is(W, if deleted { "delText" } else { "t" })
            && child.elements().next().is_none()
            && t_attributes_ok(child)
        {
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

/// Table-style children this slice resolves. `false` means something was
/// left unparsed; known bold, shading and borders are still stored.
pub(crate) fn read_style_table(style: &mut Style, element: &Element) -> bool {
    match &*element.local {
        "tblPr" => fill_format(&mut style.table, element, true),
        "tcPr" => fill_format(&mut style.table, element, false),
        "tblStylePr" => read_conditional(style, element),
        _ => false,
    }
}

fn read_conditional(style: &mut Style, element: &Element) -> bool {
    if !only_word_attributes(element, &["type"]) || !text_is_space(element) {
        return false;
    }
    let condition = match element.attribute(Some(W), "type") {
        Some("firstRow") => TableCondition::FirstRow,
        Some("lastRow") => TableCondition::LastRow,
        Some("firstCol") => TableCondition::FirstColumn,
        Some("lastCol") => TableCondition::LastColumn,
        Some("band1Horz") => TableCondition::Band1Horizontal,
        Some("band2Horz") => TableCondition::Band2Horizontal,
        Some("band1Vert") => TableCondition::Band1Vertical,
        Some("band2Vert") => TableCondition::Band2Vertical,
        _ => return false,
    };
    let mut format = TableFormat::default();
    let mut complete = true;
    for child in element.elements() {
        if child.namespace.as_deref() != Some(W) {
            complete = false;
            continue;
        }
        let known = match &*child.local {
            "rPr" => read_bold(&mut format, child),
            "tblPr" => fill_format(&mut format, child, true),
            "tcPr" => fill_format(&mut format, child, false),
            _ => false,
        };
        complete &= known;
    }
    style
        .table_conditionals
        .push(TableConditional { condition, format });
    complete
}

fn fill_format(format: &mut TableFormat, element: &Element, table_properties: bool) -> bool {
    if !only_word_attributes(element, &[]) || !text_is_space(element) {
        return false;
    }
    let mut complete = true;
    for child in element.elements() {
        if child.namespace.as_deref() != Some(W) {
            complete = false;
            continue;
        }
        let known = if table_properties {
            match &*child.local {
                "tblBorders" => match parse_borders(child) {
                    Some(borders) => {
                        overlay_borders(&mut format.borders, borders);
                        true
                    }
                    None => false,
                },
                "shd" => match parse_shading(child) {
                    Some(shading) => {
                        format.shading = Some(shading);
                        true
                    }
                    None => false,
                },
                _ => false,
            }
        } else {
            match &*child.local {
                "tcBorders" => match parse_borders(child) {
                    Some(borders) => {
                        overlay_borders(&mut format.borders, borders);
                        true
                    }
                    None => false,
                },
                "shd" => match parse_shading(child) {
                    Some(shading) => {
                        format.shading = Some(shading);
                        true
                    }
                    None => false,
                },
                _ => false,
            }
        };
        complete &= known;
    }
    complete
}

fn overlay_borders(base: &mut CellBorders, layer: CellBorders) {
    if layer.top.is_some() {
        base.top = layer.top;
    }
    if layer.left.is_some() {
        base.left = layer.left;
    }
    if layer.bottom.is_some() {
        base.bottom = layer.bottom;
    }
    if layer.right.is_some() {
        base.right = layer.right;
    }
}

fn read_bold(format: &mut TableFormat, element: &Element) -> bool {
    let mut findings = Findings::default();
    let (run, style) = props::run(element, &mut findings);
    format.bold = run.bold;
    findings.complete
        && style.is_none()
        && run.italic.is_none()
        && run.strike.is_none()
        && run.hidden.is_none()
        && run.underline.is_none()
        && run.size.is_none()
        && run.color.is_none()
        && run.fonts == joyword_model::FontSlots::default()
        && run.languages == joyword_model::Languages::default()
        && run.preserved.is_none()
}
