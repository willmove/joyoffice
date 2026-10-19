//! T201: typed `w:tbl` import and export. Unknown table markup stays an
//! opaque block; a represented table round-trips through the semantic dump.

use std::sync::Arc;

use joyoffice_opc::{Budget, Method, write_archive};
use joyword_docx::{import, plan_export};
use joyword_model::{Block, Measure, VerticalMerge};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
const PACKAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

fn package(body: &str) -> Vec<u8> {
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>{body}</w:body></w:document>"#
    );
    let items = [
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", PACKAGE_RELS),
        ("word/document.xml", document.as_str()),
    ];
    let owned: Vec<(&str, &[u8], Method)> = items
        .iter()
        .map(|(name, data)| (*name, data.as_bytes(), Method::Deflated))
        .collect();
    write_archive(&owned).unwrap()
}

fn xml_of(bytes: &[u8]) -> String {
    let package = joyoffice_opc::Package::read(bytes, &Budget::default()).unwrap();
    String::from_utf8(package.part("/word/document.xml").unwrap().data.to_vec()).unwrap()
}

fn simple_table() -> String {
    r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/><w:shd w:val="clear" w:color="auto" w:fill="FF0000"/><w:tcBorders><w:top w:val="single" w:sz="4" w:color="auto"/><w:left w:val="nil"/><w:bottom w:val="single" w:sz="8" w:color="00FF00"/><w:right w:val="single" w:color="auto"/></w:tcBorders></w:tcPr><w:p><w:r><w:t>左</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="3000" w:type="dxa"/></w:tcPr><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>右</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#.to_owned()
}

#[test]
fn a_represented_table_is_typed_and_an_unknown_sibling_stays_opaque() {
    let looked = r#"<w:tbl><w:tblPr><w:tblLook w:val="04A0"/></w:tblPr><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>隐</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#;
    let result = import(
        &package(&format!(
            "{}<w:p><w:r><w:t>后</w:t></w:r></w:p>",
            simple_table()
        )),
        &Budget::default(),
    )
    .unwrap();
    assert!(!result.editability.is_read_only());
    let Block::Table(table) = &result.document.body().blocks.get(0).unwrap() else {
        panic!("typed table");
    };
    assert!(result.editability.can_edit(table.id));
    assert_eq!(table.grid, vec![Measure::Twips(2000), Measure::Twips(3000)]);
    assert!(matches!(table.width, Some(joyword_model::TableWidth::Auto)));
    let cell = &table.rows[0].cells[0];
    assert_eq!(
        cell.shading.unwrap().fill,
        joyword_model::Color::Rgb(255, 0, 0)
    );
    assert_eq!(cell.borders.top.unwrap().size_eighths, 4);
    assert_eq!(
        cell.borders.left.unwrap().kind,
        joyword_model::BorderKind::Nil
    );
    assert_eq!(cell.borders.bottom.unwrap().size_eighths, 8);
    assert_eq!(
        cell.borders.right.unwrap().color,
        joyword_model::Color::Auto
    );
    assert_eq!(cell.borders.right.unwrap().size_eighths, 4);

    let unknown = import(
        &package(&format!("{looked}<w:p><w:r><w:t>后</w:t></w:r></w:p>")),
        &Budget::default(),
    )
    .unwrap();
    let blocks: Vec<&Block> = unknown.document.body().blocks.iter().collect();
    assert!(matches!(blocks[0], Block::Opaque(opaque) if &*opaque.preserved.element == "w:tbl"));
    assert!(!unknown.editability.can_edit(blocks[0].id()));
    assert!(unknown.editability.can_edit(blocks[1].id()));
    assert!(!unknown.editability.is_read_only());

    let revised = import(
        &package(
            r#"<w:tbl><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:rPr><w:rPrChange w:id="3" w:author="A"><w:rPr/></w:rPrChange></w:rPr><w:t>改</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
        ),
        &Budget::default(),
    )
    .unwrap();
    assert!(revised.editability.is_read_only());
    assert!(matches!(
        revised.document.body().blocks.get(0),
        Some(Block::Opaque(_))
    ));
}

#[test]
fn editing_a_cell_round_trips_and_keeps_merge_state() {
    let merged = r#"<w:tbl><w:tblGrid><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/><w:vMerge w:val="restart"/></w:tcPr><w:p><w:r><w:t>合</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/><w:vMerge/></w:tcPr><w:p/></w:tc></w:tr></w:tbl>"#;
    let source = import(&package(merged), &Budget::default()).unwrap();
    let Block::Table(table) = &source.document.body().blocks.get(0).unwrap() else {
        panic!("merged table");
    };
    assert_eq!(table.rows[0].cells[0].grid_span, 2);
    assert_eq!(
        table.rows[0].cells[0].vertical_merge,
        VerticalMerge::Restart
    );
    assert_eq!(
        table.rows[1].cells[0].vertical_merge,
        VerticalMerge::Continue
    );

    let mut model = source.document.clone();
    let body = model.body_id();
    model.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Table(table) = block else {
            panic!("table");
        };
        let table = Arc::make_mut(table);
        table.rows[0].cells[0].blocks.update(0, |block| {
            let Block::Paragraph(paragraph) = block else {
                panic!("paragraph");
            };
            Arc::make_mut(paragraph)
                .push_text("并", None, Arc::default())
                .unwrap();
        });
    });
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(
        model.semantic_difference(&saved.document),
        None,
        "{}",
        model.canonical_dump(joyword_model::DumpOptions { node_ids: false })
    );
    let xml = xml_of(plan.bytes());
    assert!(xml.contains(r#"w:val="restart""#), "{xml}");
    assert!(xml.contains(r#"w:val="continue""#), "{xml}");
    assert!(xml.contains(r#"w:val="2""#), "{xml}");
}

#[test]
fn an_edited_paragraph_beside_an_opaque_table_is_refused() {
    let body = r#"<w:p><w:r><w:t>前</w:t></w:r></w:p><w:tbl><w:tblPr><w:tblLook w:val="04A0"/></w:tblPr><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>隐</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p><w:r><w:t>后</w:t></w:r></w:p>"#;
    let source = import(&package(body), &Budget::default()).unwrap();
    let mut model = source.document.clone();
    let body_id = model.body_id();
    model.story_mut(body_id).unwrap().blocks.update(2, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        Arc::make_mut(paragraph)
            .push_text("增", None, Arc::default())
            .unwrap();
    });
    let error = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap_err();
    assert!(
        matches!(error, joyword_docx::ExportError::Restricted),
        "{error:?}"
    );
    let xml = xml_of(&package(body));
    assert!(xml.contains(r#"w:val="04A0""#), "{xml}");
    assert!(xml.contains("隐"), "{xml}");
}

#[test]
fn a_simple_grid_table_round_trips_after_a_cell_edit() {
    let source = import(&package(&simple_table()), &Budget::default()).unwrap();
    let mut model = source.document.clone();
    let body = model.body_id();
    model.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Table(table) = block else {
            panic!("table");
        };
        Arc::make_mut(table).rows[0].cells[1]
            .blocks
            .update(0, |block| {
                let Block::Paragraph(paragraph) = block else {
                    panic!("paragraph");
                };
                Arc::make_mut(paragraph)
                    .push_text("侧", None, Arc::default())
                    .unwrap();
            });
    });
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(model.semantic_difference(&saved.document), None);
    let xml = xml_of(plan.bytes());
    assert!(xml.contains("侧"), "{xml}");
    assert!(xml.contains(r#"w:fill="FF0000""#), "{xml}");
    assert!(xml.contains(r#"w:val="nil""#), "{xml}");
}

#[test]
fn cell_margins_cant_split_and_header_round_trip_after_an_edit() {
    let body = r#"<w:tbl><w:tblPr><w:tblCellMar><w:left w:w="0" w:type="dxa"/><w:bottom w:w="120" w:type="dxa"/><w:right w:w="200" w:type="dxa"/></w:tblCellMar></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:trPr><w:cantSplit/><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>头</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#;
    let source = import(&package(body), &Budget::default()).unwrap();
    let Block::Table(table) = &source.document.body().blocks.get(0).unwrap() else {
        panic!("typed table");
    };
    let margin = table.cell_margin.unwrap();
    assert_eq!(margin.top, 0);
    assert_eq!(margin.left, 0);
    assert_eq!(margin.bottom, 120);
    assert_eq!(margin.right, 200);
    assert!(table.rows[0].cant_split);
    assert!(table.rows[0].header);

    let mut model = source.document.clone();
    let body_id = model.body_id();
    model.story_mut(body_id).unwrap().blocks.update(0, |block| {
        let Block::Table(table) = block else {
            panic!("table");
        };
        Arc::make_mut(table).rows[0].cells[0]
            .blocks
            .update(0, |block| {
                let Block::Paragraph(paragraph) = block else {
                    panic!("paragraph");
                };
                Arc::make_mut(paragraph)
                    .push_text("行", None, Arc::default())
                    .unwrap();
            });
    });
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let xml = xml_of(plan.bytes());
    assert!(xml.contains("头"), "{xml}");
    assert!(xml.contains("行"), "{xml}");
    assert!(xml.contains("tblCellMar"), "{xml}");
    assert!(xml.contains("cantSplit"), "{xml}");
    assert!(xml.contains("tblHeader"), "{xml}");
    assert!(xml.contains(r#"w:w="0""#), "{xml}");
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(model.semantic_difference(&saved.document), None);

    let percent = r#"<w:tbl><w:tblPr><w:tblCellMar><w:left w:w="100" w:type="pct"/></w:tblCellMar></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>隐</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#;
    let opaque = import(&package(percent), &Budget::default()).unwrap();
    assert!(matches!(
        opaque.document.body().blocks.get(0),
        Some(Block::Opaque(_))
    ));
}

#[test]
fn cell_margins_borders_and_percentage_widths_round_trip_after_an_edit() {
    let both = r#"<w:tbl><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcMar><w:left w:w="36" w:type="dxa"/><w:start w:w="1" w:type="dxa"/></w:tcMar></w:tcPr><w:p><w:r><w:t>隐</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#;
    assert!(matches!(
        import(&package(both), &Budget::default())
            .unwrap()
            .document
            .body()
            .blocks
            .get(0),
        Some(Block::Opaque(_))
    ));

    let clean = r#"<w:tbl><w:tblPr><w:tblW w:w="2500" w:type="pct"/><w:tblLayout w:type="fixed"/><w:jc w:val="center"/><w:tblInd w:w="120" w:type="dxa"/></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="2500" w:type="pct"/><w:tcMar><w:top w:w="10" w:type="dxa"/><w:left w:w="36" w:type="dxa"/></w:tcMar></w:tcPr><w:p><w:r><w:t>甲</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcBorders><w:top w:val="double" w:sz="12" w:color="auto" w:themeColor="accent1"/><w:left w:val="dotted" w:sz="4" w:color="00FF00"/><w:bottom w:val="dashed" w:sz="8" w:color="0000FF"/><w:right w:val="thick" w:sz="16" w:color="FF0000"/></w:tcBorders></w:tcPr><w:p><w:r><w:t>乙</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#;
    let source = import(&package(clean), &Budget::default()).unwrap();
    let Block::Table(table) = &source.document.body().blocks.get(0).unwrap() else {
        panic!("typed table");
    };
    assert_eq!(
        table.rows[0].cells[0].margin.unwrap(),
        joyword_model::CellMargins {
            top: 10,
            left: 36,
            bottom: 0,
            right: 0,
        }
    );
    let borders = table.rows[0].cells[1].borders;
    assert_eq!(borders.top.unwrap().kind, joyword_model::BorderKind::Double);
    assert_eq!(
        borders.top.unwrap().color,
        joyword_model::Color::Theme(joyword_model::ThemeColorSlot::Accent1)
    );
    assert_eq!(
        borders.left.unwrap().kind,
        joyword_model::BorderKind::Dotted
    );
    assert_eq!(
        borders.bottom.unwrap().kind,
        joyword_model::BorderKind::Dashed
    );
    assert_eq!(
        borders.right.unwrap().kind,
        joyword_model::BorderKind::Thick
    );

    let mut model = source.document.clone();
    let body_id = model.body_id();
    model.story_mut(body_id).unwrap().blocks.update(0, |block| {
        let Block::Table(table) = block else {
            panic!("table");
        };
        Arc::make_mut(table).rows[0].cells[0]
            .blocks
            .update(0, |block| {
                let Block::Paragraph(paragraph) = block else {
                    panic!("paragraph");
                };
                Arc::make_mut(paragraph)
                    .push_text("增", None, Arc::default())
                    .unwrap();
            });
    });
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let xml = xml_of(plan.bytes());
    assert!(xml.contains(r#"w:type="pct""#), "{xml}");
    assert!(xml.contains(r#"w:type="fixed""#), "{xml}");
    assert!(xml.contains(r#"w:val="center""#), "{xml}");
    assert!(xml.contains("tcMar"), "{xml}");
    assert!(xml.contains(r#"w:val="double""#), "{xml}");
    assert!(xml.contains(r#"w:themeColor="accent1""#), "{xml}");
    assert!(xml.contains(r#"w:val="dotted""#), "{xml}");
    assert!(xml.contains(r#"w:val="dashed""#), "{xml}");
    assert!(xml.contains(r#"w:val="thick""#), "{xml}");
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(model.semantic_difference(&saved.document), None);

    for extra in [
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>外</w:t></w:r></w:p><w:tbl><w:tblGrid><w:gridCol w:w="500"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>内</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl>"#,
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid><w:tr><w:trPr><w:trHeight w:val="400" w:hRule="exact"/></w:trPr><w:tc><w:p><w:r><w:t>高</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:vAlign w:val="center"/></w:tcPr><w:p><w:r><w:t>中</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcBorders><w:top w:val="double" w:sz="4" w:themeColor="accent1" w:themeTint="99"/></w:tcBorders></w:tcPr><w:p><w:r><w:t>色</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
    ] {
        let imported = import(&package(extra), &Budget::default()).unwrap();
        assert!(
            matches!(
                imported.document.body().blocks.get(0),
                Some(Block::Opaque(_))
            ),
            "{extra}"
        );
    }
}
