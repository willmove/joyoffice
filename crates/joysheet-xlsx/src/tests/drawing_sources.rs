use super::make_zip;
use crate::{read_xlsx, write_xlsx};
use std::io::Read;

#[test]
fn embedded_package_has_host_metadata_and_exact_native_bytes() {
    let bytes = crate::drawing_objects::embedded_package("test.bin", b"\x00\xffexact payload").unwrap();
    let file = crate::cfb::Compound::open(&bytes).unwrap();
    let meta = file.root_stream("\u{1}CompObj").unwrap().unwrap();
    assert_eq!(&meta[28..44], b"\x0c\0\0\0OLE Package\0");
    assert_eq!(&meta[44..48], &[0; 4], "Package does not declare an unregistered clipboard format");
    assert_eq!(&meta[60..64], &0x71b239f4u32.to_le_bytes());
    let stream = file.root_stream("\u{1}Ole10Native").unwrap().unwrap();
    // Header + flags + two NUL-terminated test.bin strings + two WORDs.
    assert_eq!(u32::from_le_bytes(stream[28..32].try_into().unwrap()), 9);
    assert_eq!(&stream[32..41], b"test.bin\0");
    let tail = 41 + 4 + b"\x00\xffexact payload".len();
    let mut at = tail;
    for _ in 0..3 {
        let count = u32::from_le_bytes(stream[at..at + 4].try_into().unwrap()) as usize;
        at += 4;
        let chars: Vec<u16> = stream[at..at + count * 2].as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        assert_eq!(String::from_utf16(&chars).unwrap(), "test.bin");
        at += count * 2;
    }
    assert_eq!(at, stream.len(), "all Unicode Package strings have explicit lengths");
    assert_eq!(crate::drawing_objects::unpack_embedded(&bytes).unwrap(), b"\x00\xffexact payload");
}

#[test]
fn package_reads_canonical_command_length_and_legacy_omission() {
    for command in [b"\x02\0\0\0f\0".as_slice(), b"f\0".as_slice()] {
        // MS-OLEDS native stream, with Package flags/label/source/command/data.
        // Build independently of the exporter so its binary offsets are checked.
        let mut body = b"\x02\0f\0f\0\0\0\x03\0".to_vec();
        body.extend_from_slice(command);
        body.extend_from_slice(b"\x04\0\0\0\0\xff\x01\x02");
        let mut native = (body.len() as u32).to_le_bytes().to_vec();
        native.extend_from_slice(&body);
        let mut cfb = joyoffice_cfb::CfbWriter::new();
        cfb.add_stream("\u{1}Ole10Native", native);
        assert_eq!(crate::drawing_objects::unpack_embedded(&cfb.build().unwrap()).unwrap(), b"\0\xff\x01\x02");
    }
}

#[test]
fn timeline_uses_excel_part_types_and_links_its_typed_date_cache() {
    use joysheet_core::{CellRef, Value};
    use joysheet_model::*;
    let mut wb = Workbook::new();
    let sh = wb.sheet_mut(0).unwrap();
    sh.name = "Data".into();
    sh.set_value(CellRef::new(0, 0), Value::text("Date"));
    sh.set_value(CellRef::new(0, 1), Value::text("Sales"));
    for (r, m) in [(1, 1), (2, 2)] {
        sh.set_value(CellRef::new(r, 0), Value::number(joysheet_core::date::serial_from_ymd(joysheet_core::DateSystem::D1900, 2026, m, 1).unwrap()));
        sh.set_value(CellRef::new(r, 1), Value::number(10.));
    }
    sh.pivots.push(PivotTable {
        name: "DatePivot".into(),
        source: "Data!A1:B3".into(),
        anchor: CellRef::new(0, 4),
        values: vec![PivotValue { source_col: "Sales".into(), ..Default::default() }],
        ..Default::default()
    });
    let object = DrawingObject::Timeline {
        pivot: "DatePivot".into(),
        column: "Date".into(),
        level: TimelineLevel::Months,
        selection: Some((46023, 46024)),
        scroll_start: None,
    };
    sh.shapes.push(Shape {
        id: 8,
        kind: ShapeKind::TextBox,
        anchor: Anchor { width: 420., height: 120., ..Default::default() },
        fill: Color::Auto,
        line: Color::Auto,
        text: "Date".into(),
        format: ShapeFormat { object: Some(object.clone()), ..Default::default() },
    });
    wb.names.push(DefinedName { name: "joytimelinecache_0_8".into(), scope: None, formula: "42".into(), comment: String::new(), hidden: false });
    let bytes = write_xlsx(&wb).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let mut part = |name: &str| {
        let mut s = String::new();
        zip.by_name(name).unwrap().read_to_string(&mut s).unwrap();
        s
    };
    assert!(part("[Content_Types].xml").contains("application/vnd.ms-excel.timelineCache+xml"));
    assert!(part("xl/_rels/workbook.xml.rels").contains("http://schemas.microsoft.com/office/2011/relationships/timelineCache"));
    assert!(part("xl/worksheets/_rels/sheet1.xml.rels").contains("http://schemas.microsoft.com/office/2011/relationships/timeline"));
    let cache = crate::xml::parse(part("xl/pivotCache/pivotCacheDefinition1.xml").as_bytes()).unwrap();
    let field = cache.child("cacheFields").unwrap().kids("cacheField").find(|f| f.attr("name") == Some("Date")).unwrap();
    assert_eq!(field.attr("numFmtId"), Some("14"));
    let items = field.child("sharedItems").unwrap();
    assert_eq!(items.attr("containsDate"), Some("1"));
    assert_eq!(items.kids("d").count(), 2);
    let cache_id = cache.path(&["extLst", "ext", "pivotCacheDefinition"]).unwrap().attr("pivotCacheId").unwrap();
    let timeline = crate::xml::parse(part("xl/timelineCaches/timelineCache0_8.xml").as_bytes()).unwrap();
    assert_eq!(timeline.attr("name"), Some("JoyTimelineCache_0_8_2"));
    assert_eq!(timeline.path(&["state", "selection"]).unwrap().attr("endDate"), Some("2026-01-02T00:00:00"));
    let book = crate::xml::parse(part("xl/workbook.xml").as_bytes()).unwrap();
    let registered = book.child("definedNames").unwrap().kids("definedName").find(|n| n.attr("name") == timeline.attr("name")).unwrap();
    assert_eq!(registered.text, "#N/A");
    assert_eq!(timeline.child("state").unwrap().attr("pivotCacheId"), Some(cache_id));
    let drawing = crate::xml::parse(part("xl/drawings/drawing1.xml").as_bytes()).unwrap();
    assert!(drawing.child("twoCellAnchor").unwrap().child("AlternateContent").is_some());
    let (again, report) = read_xlsx(&bytes).unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert_eq!(again.sheet(0).unwrap().shapes[0].format.object, Some(object));
    assert_eq!(again.names, wb.names, "internal cache registration is consumed; user names survive");
}

#[test]
fn native_smartart_keeps_unknown_xml_parts_and_frame_properties() {
    let ct=br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/drawings/drawing1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/><Override PartName="/xl/diagrams/data1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.diagramData+xml"/><Override PartName="/xl/diagrams/layout1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.diagramLayout+xml"/></Types>"#;
    let data=br#"<dgm:dataModel xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><dgm:ptLst><dgm:pt modelId="0" type="doc"/><dgm:pt modelId="1" type="node"><dgm:t><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Native</a:t></a:r></a:p></dgm:t></dgm:pt></dgm:ptLst><dgm:cxnLst/><dgm:bg/><dgm:whole/><dgm:extLst><a:ext uri="urn:test:unknown"><q:value xmlns:q="urn:unmodelled" value="retain-all-bytes"/></a:ext></dgm:extLst></dgm:dataModel>"#;
    let layout=br#"<dgm:layoutDef xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" uniqueId="urn:native:complex"><dgm:title val="Native"/><dgm:desc val="Native"/><dgm:catLst/><dgm:layoutNode name="root"><dgm:alg type="lin"/><dgm:shape type="rect"/><dgm:presOf axis="self" ptType="node"/><dgm:constrLst/><dgm:ruleLst/></dgm:layoutNode></dgm:layoutDef>"#;
    let input=make_zip(&[
        ("[Content_Types].xml",ct),
        ("_rels/.rels",br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#),
        ("xl/workbook.xml",br#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#),
        ("xl/_rels/workbook.xml.rels",br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#),
        ("xl/worksheets/sheet1.xml",br#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheetData/><drawing r:id="rId1"/></worksheet>"#),
        ("xl/worksheets/_rels/sheet1.xml.rels",br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing1.xml"/></Relationships>"#),
        ("xl/drawings/drawing1.xml",br#"<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram"><xdr:oneCellAnchor><xdr:from><xdr:col>0</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>0</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:ext cx="4000000" cy="2000000"/><xdr:graphicFrame macro="retain-this"><xdr:nvGraphicFramePr><xdr:cNvPr id="41" name="Original" descr="retain-this-too"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x="0" y="0"/><a:ext cx="4000000" cy="2000000"/></xdr:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/diagram"><dgm:relIds r:dm="rId7" r:lo="rId9"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:oneCellAnchor></xdr:wsDr>"#),
        ("xl/drawings/_rels/drawing1.xml.rels",br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId7" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramData" Target="../diagrams/data1.xml"/><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramLayout" Target="../diagrams/layout1.xml"/></Relationships>"#),
        ("xl/diagrams/data1.xml",data),("xl/diagrams/layout1.xml",layout),
    ]);
    let (wb, report) = read_xlsx(&input).unwrap();
    assert!(report.warnings.iter().any(|w| w.contains("read-only")));
    assert!(wb.active().unwrap().shapes[0].format.source.is_some());
    let output = write_xlsx(&wb).unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(&output)).unwrap();
    let mut found = false;
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        if f.name().unwrap().starts_with("xl/diagrams/") {
            let mut bytes = Vec::new();
            f.read_to_end(&mut bytes).unwrap();
            if bytes == data {
                found = true;
            }
        }
    }
    assert!(found, "original diagram data is retained byte-for-byte");
    let mut drawing = String::new();
    z.by_name("xl/drawings/drawing1.xml").unwrap().read_to_string(&mut drawing).unwrap();
    assert!(drawing.contains("retain-this-too"));
    assert!(drawing.contains("macro=\"retain-this\""));
    let (again, _) = read_xlsx(&output).unwrap();
    assert_eq!(again.active().unwrap().shapes.len(), 1);
    assert!(again.active().unwrap().shapes[0].format.source.is_some());
}
