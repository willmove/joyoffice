//! SpreadsheetML data cells and references for regenerated PresentationML charts.

use std::borrow::Cow;

use joydeck_model::chart::{Chart, ChartType, Series};
use joyoffice_opc::{Method, write_archive};

use crate::PptxError;
use crate::chart_cache::format_number;
use crate::opc::{self, NS_R, PackageOut, RelsOut};
use crate::xml::{A, W};

const MAX_SERIES: usize = 64;
const MAX_ROWS: usize = 4096;
const MAX_CELLS: usize = 65_536;
const MAX_SHEET_BYTES: usize = 8 << 20;
const SHEET: &str = "Sheet1";
const NS_SHEET: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const CONTENT_TYPE: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
const PACKAGE_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package";
const TYPES: &str = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#;
const ROOT_RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
const WORKBOOK_RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#;

#[derive(Clone, Copy)]
enum Values<'a> {
    Text(&'a [String]),
    Numbers(&'a [Option<f64>]),
    Ordinal(usize),
    Ones(usize),
}

impl Values<'_> {
    fn len(self) -> usize {
        match self {
            Self::Text(items) => items.len(),
            Self::Numbers(items) => items.len(),
            Self::Ordinal(n) | Self::Ones(n) => n,
        }
    }

    fn number(self, index: usize) -> Option<f64> {
        match self {
            Self::Numbers(items) => items.get(index).copied().flatten(),
            Self::Ordinal(n) if index < n => Some((index + 1) as f64),
            Self::Ones(n) if index < n => Some(1.0),
            _ => None,
        }
        .filter(|value| value.is_finite())
    }
}

struct Column<'a> {
    title: &'a str,
    values: Values<'a>,
}

#[derive(Clone, Copy)]
pub(super) struct SeriesColumns {
    pub(super) values: usize,
    pub(super) x: Option<usize>,
    pub(super) size: Option<usize>,
}

/// A shared mapping for both the formulas in the chart and the cells in its workbook.
pub(super) struct ChartWorkbook<'a> {
    columns: Vec<Column<'a>>,
    series: Vec<SeriesColumns>,
    category_columns: usize,
    rows: usize,
}

impl<'a> ChartWorkbook<'a> {
    pub(super) fn new(chart: &'a Chart, extended: bool, series_count: usize) -> Option<Self> {
        if series_count == 0 || series_count > MAX_SERIES {
            return None;
        }
        let mut sheet = Self {
            columns: Vec::new(),
            series: Vec::new(),
            category_columns: 0,
            rows: 0,
        };
        if extended {
            for level in chart
                .category_levels
                .iter()
                .take(joydeck_model::chart::MAX_LEVELS)
                .rev()
            {
                sheet.push("Group", Values::Text(level));
            }
        }
        sheet.push("Category", Values::Text(&chart.categories));
        sheet.category_columns = sheet.columns.len();
        for item in chart.series.iter().take(series_count) {
            let values = sheet.push(&item.name, Values::Numbers(&item.values));
            let x = matches!(chart.kind, ChartType::Scatter | ChartType::Bubble)
                .then(|| sheet.push("X", x_values(item)));
            let size =
                (chart.kind == ChartType::Bubble).then(|| sheet.push("Size", bubble_sizes(item)));
            sheet.series.push(SeriesColumns { values, x, size });
        }
        // Count each potential cell so blanks cannot bypass the worksheet budget.
        (sheet.rows <= MAX_ROWS
            && (sheet.rows + 1).saturating_mul(sheet.columns.len()) <= MAX_CELLS)
            .then_some(sheet)
    }

    fn push(&mut self, title: &'a str, values: Values<'a>) -> usize {
        let index = self.columns.len();
        self.rows = self.rows.max(values.len());
        self.columns.push(Column { title, values });
        index
    }

    pub(super) fn columns(&self, series: usize) -> Option<SeriesColumns> {
        self.series.get(series).copied()
    }

    pub(super) fn categories(&self, count: usize) -> String {
        range(0, self.category_columns - 1, count)
    }

    fn pack(&self) -> Result<Vec<u8>, PptxError> {
        let mut book = W::new();
        book.open("workbook", A::new().a("xmlns", NS_SHEET).a("xmlns:r", NS_R));
        book.open0("sheets");
        book.empty(
            "sheet",
            A::new().a("name", SHEET).a("sheetId", 1).a("r:id", "rId1"),
        );
        book.close("sheets");
        book.close("workbook");
        let book = book.finish();

        let names: Vec<_> = (0..self.columns.len()).map(column).collect();
        let mut sheet = W::new();
        sheet.open("worksheet", A::new().a("xmlns", NS_SHEET));
        sheet.open0("sheetData");
        sheet.open("row", A::new().a("r", 1));
        for (label, column) in names.iter().zip(&self.columns) {
            text_cell(&mut sheet, &format!("{label}1"), column.title);
            check_size(&sheet)?;
        }
        sheet.close("row");
        for row in 0..self.rows {
            let number = row + 2;
            sheet.open("row", A::new().a("r", number));
            for (label, column) in names.iter().zip(&self.columns) {
                if let Values::Text(items) = column.values {
                    if let Some(value) = items.get(row) {
                        text_cell(&mut sheet, &format!("{label}{number}"), value);
                    }
                } else if let Some(value) = column.values.number(row) {
                    sheet.open("c", A::new().a("r", format!("{label}{number}")));
                    sheet.elt("v", &format_number(value));
                    sheet.close("c");
                }
                check_size(&sheet)?;
            }
            sheet.close("row");
        }
        sheet.close("sheetData");
        sheet.close("worksheet");
        let sheet = sheet.finish();
        let entries = [
            ("[Content_Types].xml", TYPES.as_bytes(), Method::Deflated),
            ("_rels/.rels", ROOT_RELS.as_bytes(), Method::Deflated),
            ("xl/workbook.xml", book.as_slice(), Method::Deflated),
            (
                "xl/_rels/workbook.xml.rels",
                WORKBOOK_RELS.as_bytes(),
                Method::Deflated,
            ),
            (
                "xl/worksheets/sheet1.xml",
                sheet.as_slice(),
                Method::Deflated,
            ),
        ];
        write_archive(&entries).map_err(|error| PptxError::Write(error.to_string()))
    }

    pub(super) fn attach(
        &self,
        package: &mut PackageOut,
        chart_path: &str,
        filename: &str,
        relationships: &mut RelsOut,
    ) -> Result<String, PptxError> {
        let path = format!("ppt/embeddings/{filename}");
        package.binary(&path, CONTENT_TYPE, self.pack()?);
        Ok(crate::chart_sidecars::add_internal(
            relationships,
            PACKAGE_REL,
            &opc::relative(chart_path, &path),
        ))
    }
}

fn check_size(sheet: &W) -> Result<(), PptxError> {
    if sheet.s.len() > MAX_SHEET_BYTES {
        return Err(PptxError::Write(
            "chart workbook exceeds the 8 MiB worksheet budget".into(),
        ));
    }
    Ok(())
}

pub(super) fn x_cache(series: &Series) -> Cow<'_, [Option<f64>]> {
    if series.x_values.is_empty() {
        Cow::Owned(
            (1..=series.values.len())
                .map(|index| Some(index as f64))
                .collect(),
        )
    } else {
        Cow::Borrowed(&series.x_values)
    }
}

pub(super) fn size_cache(series: &Series) -> Cow<'_, [Option<f64>]> {
    if series.sizes.is_empty() {
        Cow::Owned(vec![Some(1.0); series.values.len()])
    } else {
        Cow::Borrowed(&series.sizes)
    }
}

fn x_values(series: &Series) -> Values<'_> {
    if series.x_values.is_empty() {
        Values::Ordinal(series.values.len())
    } else {
        Values::Numbers(&series.x_values)
    }
}

fn bubble_sizes(series: &Series) -> Values<'_> {
    if series.sizes.is_empty() {
        Values::Ones(series.values.len())
    } else {
        Values::Numbers(&series.sizes)
    }
}

pub(super) fn name_cell(column_index: usize) -> String {
    format!("{SHEET}!${}$1", column(column_index))
}

pub(super) fn range(first_column: usize, last_column: usize, count: usize) -> String {
    let first = column(first_column);
    let last = column(last_column);
    let end = count.max(1) + 1;
    format!("{SHEET}!${first}$2:${last}${end}")
}

fn column(index: usize) -> String {
    let mut result = String::new();
    let mut index = index + 1;
    while index > 0 {
        index -= 1;
        result.insert(0, char::from(b'A' + (index % 26) as u8));
        index /= 26;
    }
    result
}

fn text_cell(sheet: &mut W, address: &str, text: &str) {
    sheet.open("c", A::new().a("r", address).a("t", "inlineStr"));
    sheet.open0("is");
    sheet.open("t", A::new().a("xml:space", "preserve"));
    sheet.text(text);
    sheet.close("t");
    sheet.close("is");
    sheet.close("c");
}
