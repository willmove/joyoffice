//! DrawingML table grid, cells and merge coordinates in a graphic frame.

use joydeck_model::{
    ColorRef, Fill, Table,
    table::{Cell, Row, TableBorder, TablePart, TablePartStyle, TableStyleDef},
    text::{Anchor, TextDir},
};

use crate::read::{Imp, Part, dml as input};
use crate::write::{Exp, Out, dml as output};
use crate::xml::{A, El, W};

const MAX_AXIS: usize = 1000;
const MAX_SIZE: f64 = 20_000.0;
const MARGINS: [&str; 4] = ["marL", "marR", "marT", "marB"];
const DEFAULT_MARGINS: [f64; 4] = [7.2, 7.2, 3.6, 3.6];
const SIDES: [&str; 4] = ["lnL", "lnR", "lnT", "lnB"];
const FLAGS: [&str; 6] = [
    "firstRow", "firstCol", "lastRow", "lastCol", "bandRow", "bandCol",
];

pub(super) fn decode_styles(reader: &mut Imp, part: &Part, root: &El) -> Vec<TableStyleDef> {
    root.children_named("tblStyle")
        .take(256)
        .filter_map(|node| {
            let id = node.attr("styleId")?.to_ascii_uppercase();
            let background = node.child("tblBg").map(|bg| TablePartStyle {
                fill: bg
                    .elements()
                    .find(|node| node.local().ends_with("Fill"))
                    .and_then(|fill| input::fill_el(reader, part, fill)),
                fill_ref: bg.child("fillRef").map(style_ref),
                ..Default::default()
            });
            let parts = node
                .elements()
                .filter_map(|section| {
                    let area = TablePart::from_xml(section.local())?;
                    Some((area, decode_area(reader, part, section)))
                })
                .collect();
            Some(TableStyleDef {
                id,
                name: node.attr("styleName").unwrap_or_default().into(),
                raw: part.keep(node),
                background,
                parts,
            })
        })
        .collect()
}

fn decode_area(reader: &mut Imp, part: &Part, area: &El) -> TablePartStyle {
    let text = area.child("tcTxStyle");
    let cell = area.child("tcStyle");
    let borders = cell.and_then(|style| style.child("tcBdr"));
    TablePartStyle {
        text_color: text.and_then(input::color),
        bold: text
            .and_then(|style| style.attr("b"))
            .map(|flag| flag == "on"),
        italic: text
            .and_then(|style| style.attr("i"))
            .map(|flag| flag == "on"),
        fill: cell
            .and_then(|style| style.child("fill"))
            .and_then(|wrapper| wrapper.elements().next())
            .and_then(|fill| input::fill_el(reader, part, fill)),
        fill_ref: cell.and_then(|style| style.child("fillRef")).map(style_ref),
        borders: ["left", "right", "top", "bottom", "insideH", "insideV"].map(|side| {
            let border = borders?.child(side)?;
            if let Some(line) = border.child("ln") {
                Some(TableBorder::Line {
                    line: input::line(reader, part, line),
                })
            } else {
                let (idx, color) = style_ref(border.child("lnRef")?);
                Some(TableBorder::Ref { idx, color })
            }
        }),
    }
}

fn style_ref(node: &El) -> (u32, ColorRef) {
    let index = node.u32("idx").unwrap_or_default();
    let color =
        input::color(node).unwrap_or_else(|| ColorRef::scheme(joydeck_color::SchemeSlot::Accent1));
    (index, color)
}

pub(super) fn decode(imp: &mut Imp, part: &Part, element: &El) -> Table {
    let mut table = Table {
        style: "grid".into(),
        first_row: false,
        band_row: false,
        ..Default::default()
    };
    if let Some(props) = element.child("tblPr") {
        table.background = input::fill_of(imp, part, props).filter(|fill| *fill != Fill::None);
        [
            table.first_row,
            table.first_col,
            table.last_row,
            table.last_col,
            table.band_row,
            table.band_col,
        ] = FLAGS.map(|flag| props.bool(flag).unwrap_or_default());
        if let Some(style) = props.child("tableStyleId") {
            let value = style.text();
            let guid = value.trim().to_ascii_uppercase();
            table.style =
                if imp.table_style_ids.contains(&guid) && !crate::tables::is_own_guid(&guid) {
                    guid
                } else {
                    crate::tables::style_from_guid(value.trim())
                };
        }
    }
    table.cols = element
        .child("tblGrid")
        .into_iter()
        .flat_map(|grid| grid.children_named("gridCol"))
        .take(MAX_AXIS)
        .map(|col| measure(col, "w", 72.0))
        .collect();
    table.rows = element
        .children_named("tr")
        .take(MAX_AXIS)
        .map(|row| Row {
            height: measure(row, "h", 0.0),
            cells: row
                .children_named("tc")
                .take(MAX_AXIS)
                .map(|cell| decode_cell(imp, part, cell))
                .collect(),
        })
        .collect();
    if table.cols.is_empty() {
        let width = table
            .rows
            .iter()
            .map(|row| row.cells.len())
            .max()
            .unwrap_or_default();
        table.cols.resize(width, 72.0);
    }
    table.normalize();
    table
}

fn measure(node: &El, attr: &str, fallback: f64) -> f64 {
    input::pt(node, attr)
        .unwrap_or(fallback)
        .clamp(0.0, MAX_SIZE)
}

fn decode_cell(imp: &mut Imp, part: &Part, node: &El) -> Cell {
    let span = |name| node.u32(name).unwrap_or(1).clamp(1, MAX_AXIS as u32);
    let mut cell = Cell {
        grid_span: span("gridSpan"),
        row_span: span("rowSpan"),
        h_merge: node.bool("hMerge").unwrap_or_default(),
        v_merge: node.bool("vMerge").unwrap_or_default(),
        text: node
            .child("txBody")
            .map(|text| input::text_body(imp, part, text))
            .unwrap_or_default(),
        ..Default::default()
    };
    let Some(props) = node.child("tcPr") else {
        return cell;
    };
    if MARGINS.iter().any(|attr| props.has_attr(attr)) {
        cell.margins = Some(std::array::from_fn(|i| {
            input::pt(props, MARGINS[i]).unwrap_or(DEFAULT_MARGINS[i])
        }));
    }
    cell.anchor = props.attr("anchor").and_then(Anchor::from_xml);
    cell.vertical = props.attr("vert").and_then(TextDir::from_xml);
    let mut line = |name| {
        props
            .child(name)
            .map(|element| input::line(imp, part, element))
    };
    cell.borders = SIDES.map(&mut line);
    cell.diag_down = line("lnTlToBr");
    cell.diag_up = line("lnBlToTr");
    cell.fill = input::fill_of(imp, part, props);
    cell
}

pub(super) fn encode(xml: &mut W, export: &mut Exp, out: &mut Out, table: &Table) {
    let mut table = table.clone();
    table.normalize();
    if !export.table_styles.contains(&table.style) {
        export.table_styles.push(table.style.clone());
    }
    let (guid, _) = crate::tables::guid_for_style(&table.style);
    let attrs = FLAGS
        .into_iter()
        .zip([
            table.first_row,
            table.first_col,
            table.last_row,
            table.last_col,
            table.band_row,
            table.band_col,
        ])
        .fold(A::new(), |attrs, (name, value)| attrs.t(name, value));
    xml.open0("a:tbl");
    xml.open("a:tblPr", attrs);
    if let Some(background) = &table.background {
        output::fill(xml, export, out, background);
    }
    xml.elt("a:tableStyleId", &guid);
    xml.close("a:tblPr");
    xml.open0("a:tblGrid");
    for &width in &table.cols {
        xml.empty("a:gridCol", A::new().a("w", output::emu_pos(width)));
    }
    xml.close("a:tblGrid");
    for row in &table.rows {
        xml.open("a:tr", A::new().a("h", output::emu_pos(row.height)));
        for cell in &row.cells {
            encode_cell(xml, export, out, cell);
        }
        xml.close("a:tr");
    }
    xml.close("a:tbl");
}

fn encode_cell(xml: &mut W, export: &mut Exp, out: &mut Out, cell: &Cell) {
    xml.open(
        "a:tc",
        A::new()
            .o("gridSpan", (cell.grid_span > 1).then_some(cell.grid_span))
            .o("rowSpan", (cell.row_span > 1).then_some(cell.row_span))
            .t("hMerge", cell.h_merge)
            .t("vMerge", cell.v_merge),
    );
    output::text_body(xml, export, out, "a:txBody", &cell.text, false);
    let anchor = match cell.anchor {
        Some(Anchor::Justified | Anchor::Distributed) => Some("t"),
        other => other.map(Anchor::xml),
    };
    let margins = cell
        .margins
        .into_iter()
        .flat_map(|values| MARGINS.into_iter().zip(values));
    let attrs = margins.fold(A::new(), |attrs, (name, value)| {
        attrs.a(name, output::emu(value))
    });
    xml.open(
        "a:tcPr",
        attrs
            .o("vert", cell.vertical.map(TextDir::xml))
            .o("anchor", anchor),
    );
    let lines = [
        "a:lnL",
        "a:lnR",
        "a:lnT",
        "a:lnB",
        "a:lnTlToBr",
        "a:lnBlToTr",
    ];
    for (tag, line) in lines
        .into_iter()
        .zip(cell.borders.iter().chain([&cell.diag_down, &cell.diag_up]))
    {
        if let Some(line) = line {
            output::line(xml, export, out, tag, line);
        }
    }
    if let Some(fill) = cell.fill.as_ref().filter(|fill| **fill != Fill::Background) {
        output::fill(xml, export, out, fill);
    }
    xml.close("a:tcPr");
    xml.close("a:tc");
}
