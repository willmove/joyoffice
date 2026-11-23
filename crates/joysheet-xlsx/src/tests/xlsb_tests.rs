//! Original BIFF12 records and ZIP packages assembled in Rust from the public format schema.
use joysheet_core::{CellError, CellRef, DateSystem, RangeRef, Value};
use joysheet_model::Visibility;

use super::make_zip;
use crate::{Format, read_xlsb, read_xlsx, sniff, write_xlsx};

const ROOT: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="book" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="records/book.bin"/></Relationships>"#;
const RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="one" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="../data/one.bin"/><Relationship Id="two" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="../data/two.bin"/><Relationship Id="strings" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="../text/strings.bin"/></Relationships>"#;
const TYPES: &str = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/records/book.bin" ContentType="application/vnd.ms-excel.sheet.binary.macroEnabled.main"/><Override PartName="/data/one.bin" ContentType="application/vnd.ms-excel.worksheet"/><Override PartName="/data/two.bin" ContentType="application/vnd.ms-excel.worksheet"/><Override PartName="/text/strings.bin" ContentType="application/vnd.ms-excel.sharedStrings"/></Types>"#;

fn record(id: u16, bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![];
    if id < 128 {
        out.push(id as u8);
    } else {
        out.extend([(id as u8 & 0x7f) | 0x80, (id >> 7) as u8]);
    }
    let mut len = bytes.len();
    loop {
        let byte = (len & 0x7f) as u8;
        len >>= 7;
        out.push(if len == 0 { byte } else { byte | 0x80 });
        if len == 0 {
            break;
        }
    }
    out.extend(bytes);
    out
}
fn wide(text: &str) -> Vec<u8> {
    let units: Vec<_> = text.encode_utf16().collect();
    let mut out = (units.len() as u32).to_le_bytes().to_vec();
    for unit in units {
        out.extend(unit.to_le_bytes());
    }
    out
}
fn rich(text: &str, extras: bool) -> Vec<u8> {
    let mut out = vec![if extras { 3 } else { 0 }];
    out.extend(wide(text));
    if extras {
        out.extend(1u32.to_le_bytes());
        out.extend([0, 0, 0, 0]); // One original formatting run.
        out.extend(wide("sound"));
        out.extend(1u32.to_le_bytes());
        out.extend([0, 0, 0, 0, 1, 0, 0, 0, 0, 0]); // One ten-byte phonetic run.
    }
    out
}
fn row(r: u32) -> Vec<u8> {
    let mut out = r.to_le_bytes().to_vec();
    out.extend([0; 13]);
    record(0, &out)
}
fn cell(id: u16, col: u32, value: &[u8]) -> Vec<u8> {
    let mut data = col.to_le_bytes().to_vec();
    data.extend([0; 4]);
    data.extend(value);
    if (8..=11).contains(&id) {
        data.extend([0; 2]); // Formula flags.
        data.extend(3u32.to_le_bytes());
        data.extend([0x1e, 1, 0]); // Original PtgInt(1); ignored because we import the cache.
        data.extend(0u32.to_le_bytes());
    }
    record(id, &data)
}
fn worksheet(data: &[u8], tail: &[u8]) -> Vec<u8> {
    let mut out = record(129, &[]);
    out.extend(record(148, &[0xff; 16])); // A bogus dimension must never allocate a dense grid.
    out.extend(record(145, &[]));
    out.extend(data);
    out.extend(record(146, &[]));
    out.extend(tail);
    out.extend(record(130, &[]));
    out
}
fn book(date1904: bool) -> Vec<u8> {
    let mut out = record(131, &[]);
    let mut props = u32::from(date1904).to_le_bytes().to_vec();
    props.extend(0u32.to_le_bytes());
    props.extend(wide(""));
    out.extend(record(153, &props));
    out.extend(record(143, &[]));
    for (id, rel, name, state) in [(1u32, "one", "Data", 0u32), (2, "two", "Hidden", 1)] {
        let mut data = state.to_le_bytes().to_vec();
        data.extend(id.to_le_bytes());
        data.extend(wide(rel));
        data.extend(wide(name));
        out.extend(record(156, &data));
    }
    out.extend(record(144, &[]));
    out.extend(record(132, &[]));
    out
}
fn sst() -> Vec<u8> {
    let mut out = record(159, &[1, 0, 0, 0, 1, 0, 0, 0]);
    out.extend(record(19, &rich("shared 🦀 café", true)));
    out.extend(record(160, &[]));
    out
}
fn package(book: &[u8], sheet: &[u8], strings: &[u8], roots: &str, rels: &str, types: &str) -> Vec<u8> {
    make_zip(&[
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", roots.as_bytes()),
        ("records/book.bin", book),
        ("records/_rels/book.bin.rels", rels.as_bytes()),
        ("data/one.bin", sheet),
        ("data/two.bin", &worksheet(&[], &[])),
        ("text/strings.bin", strings),
    ])
}
fn file(sheet: &[u8]) -> Vec<u8> {
    package(&book(false), sheet, &sst(), ROOT, RELS, TYPES)
}
fn text(s: &str) -> Value {
    Value::Text(s.into())
}

#[test]
fn xlsb_scalar_caches_rk_unicode_visibility_and_epochs_survive_xlsx_save() {
    let mut data = row(2);
    let real_rk = (3.5f64.to_bits() >> 32) as u32;
    let encoded = [
        cell(1, 0, &[]),
        cell(2, 1, &((-123i32 << 2) | 2).to_le_bytes()),
        cell(2, 2, &((123i32 << 2) | 3).to_le_bytes()),
        cell(2, 3, &real_rk.to_le_bytes()),
        cell(2, 4, &(real_rk | 1).to_le_bytes()),
        cell(3, 5, &[0x17]),
        cell(4, 6, &[1]),
        cell(5, 7, &123.5f64.to_le_bytes()),
        cell(6, 8, &wide("inline λ")),
        cell(7, 9, &0u32.to_le_bytes()),
        cell(8, 10, &wide("cached text")),
        cell(9, 11, &2.0f64.to_le_bytes()),
        cell(10, 12, &[0]),
        cell(11, 13, &[7]),
        cell(62, 14, &rich("rich text", true)),
    ];
    for cell in encoded {
        data.extend(cell);
    }
    let mut merge = vec![];
    for n in [5u32, 6, 0, 1] {
        merge.extend(n.to_le_bytes());
    }
    let sheet = worksheet(&data, &record(176, &merge));
    let expected = [
        Value::Empty,
        Value::Number(-123.0),
        Value::Number(1.23),
        Value::Number(3.5),
        Value::Number(0.035),
        Value::Error(CellError::Ref),
        Value::Bool(true),
        Value::Number(123.5),
        text("inline λ"),
        text("shared 🦀 café"),
        text("cached text"),
        Value::Number(2.0),
        Value::Bool(false),
        Value::Error(CellError::Div0),
        text("rich text"),
    ];
    for epoch in [false, true] {
        let bytes = package(&book(epoch), &sheet, &sst(), ROOT, RELS, TYPES);
        assert_eq!(sniff(&bytes), Format::Xlsb);
        let (wb, report) = read_xlsb(&bytes).unwrap();
        assert_eq!(wb.date_system, if epoch { DateSystem::D1904 } else { DateSystem::D1900 });
        assert_eq!(wb.sheets.len(), 2);
        assert_eq!(wb.sheets[1].visibility, Visibility::Hidden);
        assert_eq!(wb.sheets[0].cells.len(), 14);
        assert_eq!(wb.sheets[0].merges, vec![RangeRef::new(CellRef::new(5, 0), CellRef::new(6, 1))]);
        assert!(report.warnings.iter().any(|w| w.contains("will not recalculate") && w.contains("may display as numbers")));
        for (col, expected) in expected.iter().enumerate() {
            assert_eq!(&wb.sheets[0].value(CellRef::new(2, col as u32)), expected);
        }
        assert!(wb.sheets[0].cells.iter().all(|(_, c)| c.formula.is_none()));
        let (roundtrip, _) = read_xlsx(&write_xlsx(&wb).unwrap()).unwrap();
        assert_eq!(roundtrip.date_system, wb.date_system);
        for (col, expected) in expected.iter().enumerate() {
            assert_eq!(&roundtrip.sheets[0].value(CellRef::new(2, col as u32)), expected);
        }
    }
}

#[test]
fn xlsb_extension_blocks_are_skipped_with_strict_boundaries_and_record_framing() {
    let mut data = row(0);
    data.extend(cell(5, 0, &1.0f64.to_le_bytes()));
    data.extend(record(35, &[0; 4]));
    data.extend(record(37, &[1, 0, 0, 0, 0, 0]));
    data.extend(cell(5, 1, &99.0f64.to_le_bytes()));
    data.extend(record(130, &[])); // Looks like EndSheet, but is inside an ignored extension.
    data.extend(record(38, &[]));
    data.extend(record(36, &[]));
    data.extend([0xe8, 0x87, 0x80, 0x80, 0x80, 0x80]); // ID1000: ignored high second-ID/fourth-length bits.
    data.extend(cell(5, 1, &2.0f64.to_le_bytes()));
    let (wb, report) = read_xlsb(&file(&worksheet(&data, &[]))).unwrap();
    assert_eq!(wb.sheets[0].cells.len(), 2);
    assert_eq!(wb.sheets[0].value(CellRef::new(0, 1)), Value::Number(2.0));
    assert!(report.warnings.iter().any(|w| w.contains("blocks were skipped")));
    for bad in [
        vec![0x81],
        vec![0x81, 0],
        vec![1, 0x80],
        vec![1, 10, 0],
        record(36, &[]),
        [record(35, &[0; 4]), record(38, &[])].concat(),
        record(35, &[0; 4]),
        record(37, &[0, 0]),
    ] {
        assert!(read_xlsb(&file(&worksheet(&bad, &[]))).is_err(), "{bad:?}");
    }
    let nested = record(35, &[0; 4]).repeat(65);
    assert!(read_xlsb(&file(&worksheet(&nested, &[]))).unwrap_err().to_string().contains("nested too deeply"));
    let mut missing_end = worksheet(&[], &[]);
    missing_end.truncate(missing_end.len() - record(130, &[]).len());
    assert!(read_xlsb(&file(&missing_end)).is_err());
}

#[test]
fn xlsb_bad_values_strings_and_coordinates_fail_without_silent_data_loss() {
    let malformed = [
        cell(4, 0, &[2]),
        cell(3, 0, &[0xff]),
        cell(5, 0, &f64::INFINITY.to_le_bytes()),
        cell(2, 0, &0x7ff00000u32.to_le_bytes()),
        cell(7, 0, &1u32.to_le_bytes()),
        cell(6, 0, &[1, 0, 0, 0, 0, 0xd8]),
        cell(6, 0, &u32::MAX.to_le_bytes()),
        cell(5, 16384, &1.0f64.to_le_bytes()),
        cell(5, 0, &[0; 3]),
        cell(62, 0, &[1, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]),
    ];
    for bad in malformed {
        assert!(read_xlsb(&file(&worksheet(&[row(0), bad].concat(), &[]))).is_err());
    }
    assert!(read_xlsb(&file(&worksheet(&cell(5, 0, &1.0f64.to_le_bytes()), &[]))).is_err());
    assert!(read_xlsb(&file(&worksheet(&row(1048576), &[]))).is_err());
    let repeated = [row(0), cell(5, 0, &1.0f64.to_le_bytes()), cell(5, 0, &2.0f64.to_le_bytes())].concat();
    assert!(read_xlsb(&file(&worksheet(&repeated, &[]))).is_err());
    let mut formula_payload = [0u8; 18].to_vec(); // Cell header + f64 cache + flags, but no parsed-formula structure.
    assert!(read_xlsb(&file(&worksheet(&[row(0), record(9, &formula_payload)].concat(), &[]))).is_err());
    formula_payload.extend(0u32.to_le_bytes()); // Invalid zero token count.
    assert!(read_xlsb(&file(&worksheet(&[row(0), record(9, &formula_payload)].concat(), &[]))).is_err());
    let bad_sst = [record(159, &[1, 0, 0, 0, 2, 0, 0, 0]), record(160, &[])].concat();
    assert!(read_xlsb(&package(&book(false), &worksheet(&[], &[]), &bad_sst, ROOT, RELS, TYPES)).is_err());
}

#[test]
fn xlsb_requires_internal_typed_relationships_and_sniffs_only_root_workbook() {
    let sheet = worksheet(&[], &[]);
    for (roots, rels, types) in [
        (ROOT.replace("officeDocument\"", "wrong\""), RELS.to_owned(), TYPES.to_owned()),
        (ROOT.replace("Target=", "TargetMode=\"External\" Target="), RELS.to_owned(), TYPES.to_owned()),
        (ROOT.to_owned(), RELS.replace("Target=\"../data/one.bin\"", "Target=\"missing.bin\""), TYPES.to_owned()),
        (
            ROOT.to_owned(),
            RELS.replace("Target=\"../data/one.bin\"", "TargetMode=\"External\" Target=\"https://example.com/data.bin\""),
            TYPES.to_owned(),
        ),
        (
            ROOT.to_owned(),
            RELS.replace("Target=\"../text/strings.bin\"", "TargetMode=\"External\" Target=\"https://example.com/strings.bin\""),
            TYPES.to_owned(),
        ),
        (ROOT.to_owned(), RELS.to_owned(), TYPES.replace("application/vnd.ms-excel.worksheet", "application/xml")),
        (ROOT.to_owned(), RELS.trim_end_matches("</Relationships>").to_owned(), TYPES.to_owned()),
        (ROOT.to_owned(), RELS.to_owned(), TYPES.replace("http://schemas.openxmlformats.org/package/2006/content-types", "wrong")),
    ] {
        assert!(read_xlsb(&package(&book(false), &sheet, &sst(), &roots, &rels, &types)).is_err());
    }
    let chartsheet = RELS.replace(
        "Id=\"two\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet",
        "Id=\"two\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/chartsheet",
    );
    let (wb, report) = read_xlsb(&package(&book(false), &sheet, &sst(), ROOT, &chartsheet, TYPES)).unwrap();
    assert_eq!(wb.sheets.len(), 1);
    assert!(report.warnings.iter().any(|w| w.contains("non-worksheet")));
    // An unrelated binary part must not change the format of an XML workbook package.
    let xml_book = TYPES.replace(
        "/records/book.bin\" ContentType=\"application/vnd.ms-excel.sheet.binary.macroEnabled.main",
        "/elsewhere.bin\" ContentType=\"application/vnd.ms-excel.sheet.binary.macroEnabled.main",
    );
    assert_eq!(sniff(&package(&book(false), &sheet, &sst(), ROOT, RELS, &xml_book)), Format::Xlsx);
    let spec_type = TYPES.replace("application/vnd.ms-excel.sheet.binary.macroEnabled.main", "application/vnd.ms-excel.main");
    assert_eq!(sniff(&package(&book(false), &sheet, &sst(), ROOT, RELS, &spec_type)), Format::Xlsb);
    assert!(read_xlsb(&package(&book(false), &sheet, &sst(), ROOT, RELS, &spec_type)).is_ok());
}

#[test]
fn xlsb_declared_counts_and_decoded_text_are_bounded() {
    let bad_sst = record(159, &[0xff; 8]);
    assert!(read_xlsb(&package(&book(false), &worksheet(&[], &[]), &bad_sst, ROOT, RELS, TYPES)).is_err());
    let mut count_mismatch = record(159, &[1, 0, 0, 0, 1, 0, 0, 0]);
    count_mismatch.extend(record(160, &[]));
    assert!(read_xlsb(&package(&book(false), &worksheet(&[], &[]), &count_mismatch, ROOT, RELS, TYPES)).is_err());
    let mut data = row(0);
    let text = wide(&"x".repeat(32767));
    for col in 0..2050 {
        data.extend(cell(6, col, &text));
    }
    assert!(read_xlsb(&file(&worksheet(&data, &[]))).unwrap_err().to_string().contains("64 MiB"));
}

#[test]
fn xlsb_phonetic_runs_use_documented_size_and_tolerate_padding() {
    // [MS-XLSB] 2.5.103: a PhRun is 10 bytes. Phonetic data is not imported, so
    // padding after the documented runs (e.g. 12-byte runs) is ignored, while
    // a run shorter than documented is still rejected.
    let phonetic = |runs: u32, bytes: usize| {
        let mut out = vec![2];
        out.extend(wide("ruby"));
        out.extend(wide("ルビ"));
        out.extend(runs.to_le_bytes());
        out.extend(vec![0; bytes]);
        out
    };
    for padded in [phonetic(1, 10), phonetic(1, 12), phonetic(2, 24)] {
        let (wb, _) = read_xlsb(&file(&worksheet(&[row(0), cell(62, 0, &padded)].concat(), &[]))).unwrap();
        assert_eq!(wb.sheets[0].value(CellRef::new(0, 0)), text("ruby"));
    }
    assert!(read_xlsb(&file(&worksheet(&[row(0), cell(62, 0, &phonetic(2, 12))].concat(), &[]))).is_err());
}

#[test]
fn xlsb_book_view_reads_the_first_complete_view_in_its_collection() {
    let view = |tab: u32| {
        let mut bytes = [0; 29];
        bytes[24..28].copy_from_slice(&tab.to_le_bytes());
        record(158, &bytes)
    };
    for (tab, expected) in [(1, "Second"), (7, "Data")] {
        let mut book = record(131, &[]);
        for part in [view(0), record(135, &[]), record(158, &[0; 28]), view(tab), view(0), record(136, &[]), record(143, &[])] {
            book.extend(part);
        }
        for (sheet_id, rel, name) in [(1u32, "one", "Data"), (2, "two", "Second")] {
            let mut data = vec![0; 4];
            data.extend(sheet_id.to_le_bytes());
            data.extend(wide(rel));
            data.extend(wide(name));
            book.extend(record(156, &data));
        }
        book.extend(record(144, &[]));
        book.extend(record(132, &[]));
        let data = package(&book, &worksheet(&[], &[]), &sst(), ROOT, RELS, TYPES);
        let (wb, report) = read_xlsb(&data).unwrap();
        assert_eq!(wb.active().unwrap().name, expected);
        assert!(report.warnings.iter().any(|s| s.contains("truncated workbook view")));
    }
}

#[test]
fn xlsb_display_options_view_mode_zoom_and_outline_summary_survive_xlsx_save() {
    let ws_view = |flags: u16, xl_view: u32, w_scale: u16, w_normal: u16, w_slv: u16, w_plv: u16| {
        let mut p = Vec::with_capacity(30);
        p.extend(flags.to_le_bytes());
        p.extend(xl_view.to_le_bytes());
        p.extend(2u32.to_le_bytes()); // rwTop = row 3
        p.extend(3u32.to_le_bytes()); // colLeft = col D
        p.extend([0x40, 0, 0, 0]);
        p.extend(w_scale.to_le_bytes());
        p.extend(w_normal.to_le_bytes());
        p.extend(w_slv.to_le_bytes());
        p.extend(w_plv.to_le_bytes());
        p.extend(0u32.to_le_bytes());
        record(137, &p)
    };
    // BrtWsProp: summaryBelow=0, summaryRight=1 (0x0080 set, 0x0040 clear).
    let mut ws_prop = vec![0u8; 23];
    ws_prop[0..2].copy_from_slice(&0x0081u16.to_le_bytes());
    let mut head = record(129, &[]);
    head.extend(record(147, &ws_prop));
    // Primary view: PageLayout (2), formulas=1 (0x0002), gridlines=0, headings=1 (0x0008), zeros=0, rtl=1 (0x0020), outlineSymbols=0, wScale=0 -> fallback wScalePLV=135.
    head.extend(ws_view(0x0002 | 0x0008 | 0x0020, 2, 0, 100, 75, 135));
    head.extend(record(138, &[]));
    // Secondary view must be ignored.
    head.extend(ws_view(0x039c, 0, 90, 90, 0, 0));
    head.extend(record(138, &[]));
    head.extend(record(145, &[]));
    head.extend(record(146, &[]));
    head.extend(record(130, &[]));

    let (wb, _) = read_xlsb(&file(&head)).unwrap();
    let sh = &wb.sheets[0];
    assert!(sh.show_formulas);
    assert!(!sh.show_gridlines);
    assert!(sh.show_headings);
    assert!(!sh.show_zeros);
    assert!(!sh.show_outline_symbols);
    assert!(sh.right_to_left);
    assert_eq!(sh.view, joysheet_model::SheetViewMode::PageLayout);
    assert_eq!(sh.zoom, 135);
    assert_eq!(sh.view_top_left, CellRef::new(2, 3));
    assert!(!sh.summary_below);
    assert!(sh.summary_right);

    let (rt, _) = read_xlsx(&write_xlsx(&wb).unwrap()).unwrap();
    let rsh = &rt.sheets[0];
    assert_eq!(
        (
            rsh.show_formulas,
            rsh.show_gridlines,
            rsh.show_headings,
            rsh.show_zeros,
            rsh.show_outline_symbols,
            rsh.right_to_left,
            rsh.view,
            rsh.zoom,
            rsh.view_top_left,
            rsh.summary_below,
            rsh.summary_right
        ),
        (true, false, true, false, false, true, joysheet_model::SheetViewMode::PageLayout, 135, CellRef::new(2, 3), false, true)
    );
}

#[test]
fn xlsb_sheet_format_cols_and_row_headers_survive_xlsx_save() {
    // BrtWsFmtInfo (485): dxGCol=12*256, cchDefColWidth=10, miyDefRwHeight=480 (24pt), grbit=0x0001 (fUnsynced).
    let mut fmt = Vec::with_capacity(12);
    fmt.extend((12u32 * 256).to_le_bytes());
    fmt.extend(10u16.to_le_bytes());
    fmt.extend(480u16.to_le_bytes());
    fmt.extend(1u16.to_le_bytes());
    fmt.extend([3u8, 2u8]);

    // BrtColInfo (60): cols 1..=2, coldx=20*256, ixfe=0, flags=hidden|userSet|outline=2|collapsed.
    let mut col_info = Vec::with_capacity(18);
    col_info.extend(1u32.to_le_bytes());
    col_info.extend(2u32.to_le_bytes());
    col_info.extend((20u32 * 256).to_le_bytes());
    col_info.extend(0u32.to_le_bytes());
    col_info.extend((0x0001u16 | 0x0002 | (2 << 8) | 0x1000).to_le_bytes());

    // BrtRowHdr (0): row 1, ixfe=0, miyRw=600 (30pt), flags1 = outline=3 | collapsed(0x08) | hidden(0x10) | unsynced(0x20), 0 spans.
    let mut row1 = Vec::with_capacity(17);
    row1.extend(1u32.to_le_bytes());
    row1.extend(0u32.to_le_bytes());
    row1.extend(600u16.to_le_bytes());
    row1.extend([0u8, 3u8 | 0x08 | 0x10 | 0x20, 0u8]);
    row1.extend(0u32.to_le_bytes());

    // BrtRowHdr (0): row 2, miyRw=480 (matches 24pt default, not custom) -> only outline=1 should be stored, size=None.
    let mut row2 = Vec::with_capacity(17);
    row2.extend(2u32.to_le_bytes());
    row2.extend(0u32.to_le_bytes());
    row2.extend(480u16.to_le_bytes());
    row2.extend([0u8, 1u8, 0u8]);
    row2.extend(0u32.to_le_bytes());

    let mut ws = record(129, &[]);
    ws.extend(record(485, &fmt));
    ws.extend(record(60, &col_info));
    ws.extend(record(145, &[]));
    ws.extend(record(0, &row1));
    ws.extend(record(0, &row2));
    ws.extend(record(146, &[]));
    ws.extend(record(130, &[]));

    let (wb, _) = read_xlsb(&file(&ws)).unwrap();
    let sh = &wb.sheets[0];
    assert!((sh.default_row_height - 32.0).abs() < 0.1);
    assert!(sh.default_col_width > 80.0);
    let c1 = sh.cols.get(&1).copied().unwrap();
    assert!(c1.hidden && c1.collapsed && c1.outline == 2 && c1.size.is_some());
    let r1 = sh.rows.get(&1).copied().unwrap();
    assert!(r1.hidden && r1.collapsed && r1.custom && r1.outline == 3);
    assert!((r1.size.unwrap() - 40.0).abs() < 0.1);
    let r2 = sh.rows.get(&2).copied().unwrap();
    assert_eq!(r2.outline, 1);
    assert_eq!(r2.size, None);

    let (rt, _) = read_xlsx(&write_xlsx(&wb).unwrap()).unwrap();
    let rsh = &rt.sheets[0];
    assert!((rsh.default_row_height - sh.default_row_height).abs() < 0.1);
    assert_eq!(rsh.cols.get(&1).map(|c| (c.hidden, c.outline, c.collapsed)), Some((true, 2, true)));
    assert_eq!(rsh.rows.get(&1).map(|r| (r.hidden, r.outline, r.collapsed, r.custom)), Some((true, 3, true, true)));
    assert_eq!(rsh.rows.get(&2).map(|r| (r.outline, r.size)), Some((1, None)));
}

#[test]
fn xlsb_print_setup_margins_headers_and_breaks_survive_xlsx_save() {
    let mut ws_prop = vec![0u8; 23];
    ws_prop[0..2].copy_from_slice(&0x01C1u16.to_le_bytes()); // fFitToPage | summaryBelow | summaryRight
    let print_opts = record(477, &0x000Fu16.to_le_bytes()); // hcenter|vcenter|headings|gridlines
    let mut margins = Vec::with_capacity(48);
    for m in [0.5f64, 0.6, 0.8, 0.9, 0.35, 0.4] {
        margins.extend(m.to_le_bytes());
    }
    let mut setup = Vec::with_capacity(34);
    for v in [9u32, 85, 300, 300, 1, 1, 2, 3] {
        setup.extend(v.to_le_bytes());
    }
    setup.extend(0u16.to_le_bytes()); // grbit=0 -> Landscape, valid printer settings
    let mut hf = Vec::new();
    hf.extend((0x0001u16 | 0x0002 | 0x0004).to_le_bytes()); // diffOddEven | diffFirst | scaleWithDoc (alignMargins=false)
    for s in [Some("&CTitle"), Some("Page &P"), Some("&CEven"), None, Some("&CFirst"), None] {
        match s {
            Some(t) => hf.extend(wide(t)),
            None => hf.extend(u32::MAX.to_le_bytes()),
        }
    }
    let brk = |id: u32, manual: u32| {
        let mut b = Vec::with_capacity(20);
        for v in [id, 0, 255, manual, 0] {
            b.extend(v.to_le_bytes());
        }
        record(396, &b)
    };
    let mut ws = record(129, &[]);
    ws.extend(record(147, &ws_prop));
    ws.extend(record(145, &[]));
    ws.extend(record(146, &[]));
    ws.extend(print_opts);
    ws.extend(record(476, &margins));
    ws.extend(record(478, &setup));
    ws.extend(record(479, &hf));
    ws.extend(record(392, &[]));
    ws.extend(brk(10, 1));
    ws.extend(brk(20, 0)); // automatic break ignored
    ws.extend(brk(25, 1));
    ws.extend(record(393, &[]));
    ws.extend(record(394, &[]));
    ws.extend(brk(4, 1));
    ws.extend(record(395, &[]));
    ws.extend(record(130, &[]));

    let (wb, _) = read_xlsb(&file(&ws)).unwrap();
    let p = &wb.sheets[0].print;
    assert!(p.gridlines && p.headings && p.center_h && p.center_v);
    assert_eq!(p.paper, "A4");
    assert_eq!(p.scale, 85);
    assert_eq!(p.orientation, joysheet_model::Orientation::Landscape);
    assert_eq!(p.fit_to, Some((2, 3)));
    assert_eq!(p.margins, [0.5, 0.6, 0.8, 0.9, 0.35, 0.4]);
    assert_eq!((p.header.as_str(), p.footer.as_str(), p.even_header.as_str(), p.first_header.as_str()), ("&CTitle", "Page &P", "&CEven", "&CFirst"));
    assert!(p.different_odd_even && p.different_first && p.hf_scale_with_doc && !p.hf_align_with_margins);
    assert_eq!(p.row_breaks, vec![10, 25]);
    assert_eq!(p.col_breaks, vec![4]);

    let (rt, _) = read_xlsx(&write_xlsx(&wb).unwrap()).unwrap();
    assert_eq!(rt.sheets[0].print, wb.sheets[0].print);
}
