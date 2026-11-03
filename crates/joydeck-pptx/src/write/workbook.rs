//! Bounded category-chart workbook, matching chart cache formulas (Sheet1!A2 / B2 ...).
//! Kept workbooks remain byte-identical for unchanged charts. Edited category charts receive a
//! fresh data-only workbook; macros, formulas and external links are never executed or copied.
use crate::{
    PptxError, opc,
    xml::{self, El, Node},
};
use std::io::{Cursor, Write};

fn column(mut index: usize) -> String {
    let mut out = String::new();
    loop {
        out.insert(0, (b'A' + (index % 26) as u8) as char);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    out
}
fn cell(row: usize, col: usize, text: &str) -> String {
    format!(
        "<c r=\"{}{row}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
        column(col),
        xml::esc(text)
    )
}
fn bytes(c: &joydeck_model::Chart) -> Result<Vec<u8>, PptxError> {
    let mut sheet = String::from(
        "<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData><row r=\"1\">",
    );
    sheet.push_str(&cell(1, 0, "Category"));
    for (i, s) in c.series.iter().enumerate() {
        sheet.push_str(&cell(1, i + 1, &s.name));
    }
    sheet.push_str("</row>");
    for (i, category) in c.categories.iter().enumerate() {
        let row = i + 2;
        sheet.push_str(&format!("<row r=\"{row}\">"));
        sheet.push_str(&cell(row, 0, category));
        for (j, series) in c.series.iter().enumerate() {
            if let Some(Some(value)) = series.values.get(i)
                && value.is_finite()
            {
                sheet.push_str(&format!(
                    "<c r=\"{}{row}\"><v>{value}</v></c>",
                    column(j + 1)
                ));
            }
        }
        sheet.push_str("</row>");
        if sheet.len() > 8 << 20 {
            return Err(PptxError::Write(
                "chart workbook sheet exceeds 8 MiB XML budget".into(),
            ));
        }
    }
    sheet.push_str("</sheetData></worksheet>");
    let entries = [
        (
            "[Content_Types].xml",
            "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/></Types>",
        ),
        (
            "_rels/.rels",
            "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>",
        ),
        (
            "xl/workbook.xml",
            "<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets><sheet name=\"Sheet1\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>",
        ),
        (
            "xl/_rels/workbook.xml.rels",
            "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/></Relationships>",
        ),
        ("xl/worksheets/sheet1.xml", sheet.as_str()),
    ];
    let mut zip = zip::ZipWriter::new(Cursor::new(vec![]));
    for (name, xml) in entries {
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .map_err(|e| PptxError::Write(e.to_string()))?;
        zip.write_all(xml.as_bytes())
            .map_err(|e| PptxError::Write(e.to_string()))?;
    }
    zip.finish()
        .map(|c| c.into_inner())
        .map_err(|e| PptxError::Write(e.to_string()))
}
pub(super) fn attach(
    pkg: &mut opc::PackageOut,
    chart_part: &str,
    chart: &joydeck_model::Chart,
    chart_xml: &[u8],
    index: u32,
) -> Result<Vec<u8>, PptxError> {
    let workbook = format!("ppt/embeddings/joydeck-chart-{index}.xlsx");
    if chart.series.len() > 64
        || chart.categories.len() > 4096
        || chart.series.len().saturating_mul(chart.categories.len()) > 65536
    {
        return Err(PptxError::Write(
            "chart workbook exceeds series/category/cell budget".into(),
        ));
    }
    pkg.binary(
        &workbook,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        bytes(chart)?,
    );
    let mut rels = opc::RelsOut::default();
    let id = rels.add("package", &opc::relative(chart_part, &workbook), false);
    pkg.rels(chart_part, &rels);
    let mut root = xml::parse(chart_xml)?.root;
    root.children
        .retain(|n| !matches!(n,Node::El(e) if e.is("externalData")));
    root.children.push(Node::El(El {
        name: "c:externalData".into(),
        attrs: vec![("r:id".into(), id)],
        children: vec![Node::El(El {
            name: "c:autoUpdate".into(),
            attrs: vec![("val".into(), "0".into())],
            children: vec![],
        })],
    }));
    Ok(root.to_xml().into_bytes())
}
