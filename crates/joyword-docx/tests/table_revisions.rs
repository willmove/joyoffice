//! T301: typed table property revisions (`w:tblPrChange`, `w:trPrChange`,
//! `w:tcPrChange`, ECMA-376 Part 1 §17.13.5): import, preservation through
//! unchanged and neighbouring exports, the rewrite refusal while a change is
//! pending, resolution through the engine, and the out-of-subset fallback that
//! keeps the whole table preserved without making the document read-only.

use std::sync::Arc;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{ExportError, ExportMode, ImportResult, import, plan_export};
use joyword_engine::{Command, Session, Transaction};
use joyword_model::{
    Block, Color, DumpOptions, PreviousTableProps, ScalarOffset, Table, TableAlign, VerticalAlign,
};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#;
const PACKAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
const DOCUMENT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>"#;

const TBL_CHANGE_61: &str = r#"<w:tblPrChange w:id="61" w:author="A" w:date="2020-01-02T03:04:05Z"><w:tblPr><w:jc w:val="center"/></w:tblPr></w:tblPrChange>"#;
const TR_CHANGE_62: &str =
    r#"<w:trPrChange w:id="62" w:author="B"><w:trPr><w:tblHeader/></w:trPr></w:trPrChange>"#;
const TC_CHANGE_63: &str = r#"<w:tcPrChange w:id="63" w:author="C" w:date="2020-01-04T05:06:07Z"><w:tcPr><w:vAlign w:val="bottom"/></w:tcPr></w:tcPrChange>"#;

fn package(body: &str) -> Vec<u8> {
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>{body}</w:body></w:document>"#
    );
    let settings =
        format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="{W}"></w:settings>"#);
    let items = [
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", PACKAGE_RELS),
        ("word/document.xml", document.as_str()),
        ("word/_rels/document.xml.rels", DOCUMENT_RELS),
        ("word/settings.xml", settings.as_str()),
    ];
    let stored: Vec<(&str, &[u8], Method)> = items
        .iter()
        .map(|(name, data)| (*name, data.as_bytes(), Method::Deflated))
        .collect();
    write_archive(&stored).unwrap()
}

fn open(bytes: &[u8]) -> ImportResult {
    import(bytes, &Budget::default()).unwrap()
}

fn document_xml(package_bytes: &[u8]) -> String {
    String::from_utf8(
        Package::read(package_bytes, &Budget::default())
            .unwrap()
            .part("/word/document.xml")
            .unwrap()
            .data
            .to_vec(),
    )
    .unwrap()
}

/// A paragraph, then a one-row two-cell table whose table, row and first
/// cell each carry a pending property change, then another paragraph.
/// Current values: table right-aligned, row `cantSplit`, first cell yellow.
/// Previous values: table centred, row was a header, first cell bottom-aligned.
fn table_body(tbl_change: &str, tr_change: &str, tc_change: &str) -> String {
    format!(
        concat!(
            r#"<w:p><w:r><w:t>before</w:t></w:r></w:p>"#,
            r#"<w:tbl>"#,
            r#"<w:tblPr><w:jc w:val="right"/>{tbl}</w:tblPr>"#,
            r#"<w:tblGrid><w:gridCol w:w="1000"/><w:gridCol w:w="1000"/></w:tblGrid>"#,
            r#"<w:tr><w:trPr><w:cantSplit/>{tr}</w:trPr>"#,
            r#"<w:tc><w:tcPr><w:shd w:val="clear" w:fill="FFFF00"/>{tc}</w:tcPr><w:p><w:r><w:t>one</w:t></w:r></w:p></w:tc>"#,
            r#"<w:tc><w:p><w:r><w:t>two</w:t></w:r></w:p></w:tc>"#,
            r#"</w:tr></w:tbl>"#,
            r#"<w:p><w:r><w:t>after</w:t></w:r></w:p>"#,
        ),
        tbl = tbl_change,
        tr = tr_change,
        tc = tc_change,
    )
}

fn full_body() -> String {
    table_body(TBL_CHANGE_61, TR_CHANGE_62, TC_CHANGE_63)
}

fn table_of(document: &joyword_model::Document) -> &Table {
    match document.body().blocks.get(1) {
        Some(Block::Table(table)) => table,
        other => panic!("expected the table at body index 1, got {other:?}"),
    }
}

fn yellow() -> Color {
    Color::Rgb(0xFF, 0xFF, 0x00)
}

fn exec(session: &mut Session, commands: Vec<Command>) {
    session
        .apply(Transaction::new(session.version(), commands))
        .unwrap();
}

#[test]
fn table_property_revisions_import_typed_and_keep_the_document_editable() {
    let bytes = package(&full_body());
    let result = open(&bytes);
    assert!(
        !result.editability.is_read_only(),
        "typed table property revisions must not make the document read-only: {:?}",
        result.editability.read_only_reasons().collect::<Vec<_>>()
    );
    let table = table_of(&result.document);

    let change = table.props_revision.as_ref().expect("tblPrChange typed");
    assert_eq!((change.id, &*change.author), (61, "A"));
    assert_eq!(change.date.as_deref(), Some("2020-01-02T03:04:05Z"));
    assert_eq!(
        table.alignment,
        Some(TableAlign::Right),
        "current value stays"
    );
    match &change.previous {
        PreviousTableProps::Table(previous) => {
            assert_eq!(previous.alignment, Some(TableAlign::Center));
            assert_eq!(previous.width, None);
        }
        other => panic!("expected table previous props, got {other:?}"),
    }

    let row = &table.rows[0];
    let change = row.props_revision.as_ref().expect("trPrChange typed");
    assert_eq!(
        (change.id, &*change.author, change.date.as_deref()),
        (62, "B", None)
    );
    assert!(row.cant_split && !row.header, "current row values stay");
    match &change.previous {
        PreviousTableProps::Row(previous) => {
            assert!(previous.header && !previous.cant_split);
        }
        other => panic!("expected row previous props, got {other:?}"),
    }

    let cell = &row.cells[0];
    let change = cell.props_revision.as_ref().expect("tcPrChange typed");
    assert_eq!((change.id, &*change.author), (63, "C"));
    assert_eq!(cell.shading.map(|s| s.fill), Some(yellow()));
    assert_eq!(cell.vertical_align, None);
    match &change.previous {
        PreviousTableProps::Cell(previous) => {
            assert_eq!(previous.vertical_align, Some(VerticalAlign::Bottom));
            assert_eq!(previous.shading, None);
        }
        other => panic!("expected cell previous props, got {other:?}"),
    }
    assert!(row.cells[1].props_revision.is_none());
    assert!(table.has_props_revision(None));
    assert!(table.has_props_revision(Some(62)));
    assert!(!table.has_props_revision(Some(99)));

    let dump = result
        .document
        .canonical_dump(DumpOptions { node_ids: false });
    for line in [
        "tblPrChange#61 author=\"A\" date=2020-01-02T03:04:05Z",
        "trPrChange#62 author=\"B\" date=-",
        "tcPrChange#63 author=\"C\" date=2020-01-04T05:06:07Z",
    ] {
        assert!(dump.contains(line), "dump lacks {line:?}:\n{dump}");
    }
}

#[test]
fn unchanged_and_neighbour_exports_keep_table_change_bytes_verbatim() {
    let bytes = package(&full_body());
    let source = open(&bytes);

    // No edit: the original package is handed back byte for byte.
    let copy = plan_export(
        &source.backing,
        &source.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(copy.mode(), ExportMode::OriginalPackage);
    assert_eq!(copy.bytes(), &bytes[..]);

    // Editing the paragraph after the table rewrites only that paragraph; the
    // table keeps its three change elements byte for byte and they re-import
    // typed.
    let mut model = source.document.clone();
    let body_id = model.body_id();
    model.story_mut(body_id).unwrap().blocks.update(2, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        Arc::make_mut(paragraph)
            .insert_text(ScalarOffset(5), "X")
            .unwrap();
    });
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    assert_eq!(plan.mode(), ExportMode::TextPatch);
    let xml = document_xml(plan.bytes());
    for markup in [TBL_CHANGE_61, TR_CHANGE_62, TC_CHANGE_63] {
        assert!(xml.contains(markup), "change bytes lost:\n{xml}");
    }
    let saved = open(plan.bytes());
    let table = table_of(&saved.document);
    assert_eq!(table.props_revision.as_ref().map(|c| c.id), Some(61));
    assert_eq!(
        table.rows[0].props_revision.as_ref().map(|c| c.id),
        Some(62)
    );
    assert_eq!(
        table.rows[0].cells[0].props_revision.as_ref().map(|c| c.id),
        Some(63)
    );
    assert!(model.semantic_difference(&saved.document).is_none());
}

#[test]
fn rewriting_a_table_with_a_pending_property_revision_is_refused() {
    // Editing cell text forces the table to be synthesised, which would drop
    // the pending change or invent its previous-property bytes; each level
    // refuses with its own reason and the plan fails before any write.
    let cases = [
        (
            table_body(TBL_CHANGE_61, "", ""),
            "unresolved table property revision is kept verbatim and the table cannot be rewritten",
        ),
        (
            table_body("", TR_CHANGE_62, ""),
            "unresolved row property revision is kept verbatim and the table cannot be rewritten",
        ),
        (
            table_body("", "", TC_CHANGE_63),
            "unresolved cell property revision is kept verbatim and the table cannot be rewritten",
        ),
    ];
    for (body, expected) in cases {
        let source = open(&package(&body));
        let mut model = source.document.clone();
        let body_id = model.body_id();
        model.story_mut(body_id).unwrap().blocks.update(1, |block| {
            let Block::Table(table) = block else {
                panic!("table");
            };
            let table = Arc::make_mut(table);
            table.rows[0].cells[1].blocks.update(0, |inner| {
                let Block::Paragraph(paragraph) = inner else {
                    panic!("cell paragraph");
                };
                Arc::make_mut(paragraph)
                    .insert_text(ScalarOffset(3), "!")
                    .unwrap();
            });
        });
        match plan_export(&source.backing, &model.snapshot(), &Budget::default()) {
            Err(ExportError::Unsupported { reason, .. }) => assert_eq!(reason, expected),
            other => panic!("expected an unsupported-edit refusal, got {other:?}"),
        }
    }
}

#[test]
fn accepting_table_property_revisions_exports_the_current_properties() {
    let bytes = package(&full_body());
    let source = open(&bytes);
    let mut session = Session::new(source.document.clone());

    // One id resolves one record; the other two stay pending and the table
    // still cannot be rewritten.
    exec(&mut session, vec![Command::AcceptRevision { id: 61 }]);
    let table = table_of(session.document());
    assert!(table.props_revision.is_none());
    assert_eq!(table.alignment, Some(TableAlign::Right));
    assert!(table.rows[0].props_revision.is_some());
    assert!(matches!(
        plan_export(
            &source.backing,
            &session.document().snapshot(),
            &Budget::default()
        ),
        Err(ExportError::Unsupported { .. })
    ));

    exec(&mut session, vec![Command::AcceptAllRevisions]);
    let table = table_of(session.document());
    assert!(!table.has_props_revision(None));
    let plan = plan_export(
        &source.backing,
        &session.document().snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(plan.mode(), ExportMode::TextPatch);
    let xml = document_xml(plan.bytes());
    assert!(
        !xml.contains("PrChange"),
        "accepted changes must not be written back:\n{xml}"
    );
    let saved = open(plan.bytes());
    assert!(!saved.editability.is_read_only());
    let table = table_of(&saved.document);
    assert!(!table.has_props_revision(None));
    assert_eq!(table.alignment, Some(TableAlign::Right));
    assert!(table.rows[0].cant_split && !table.rows[0].header);
    assert_eq!(
        table.rows[0].cells[0].shading.map(|s| s.fill),
        Some(yellow())
    );
    assert_eq!(table.rows[0].cells[0].vertical_align, None);
    assert!(
        session
            .document()
            .semantic_difference(&saved.document)
            .is_none()
    );
}

#[test]
fn rejecting_table_property_revisions_restores_the_previous_properties() {
    let bytes = package(&full_body());
    let source = open(&bytes);
    let mut session = Session::new(source.document.clone());
    exec(&mut session, vec![Command::RejectAllRevisions]);
    let table = table_of(session.document());
    assert!(!table.has_props_revision(None));
    assert_eq!(table.alignment, Some(TableAlign::Center));
    assert!(table.rows[0].header && !table.rows[0].cant_split);
    assert_eq!(
        table.rows[0].cells[0].vertical_align,
        Some(VerticalAlign::Bottom)
    );
    assert_eq!(table.rows[0].cells[0].shading, None);

    let plan = plan_export(
        &source.backing,
        &session.document().snapshot(),
        &Budget::default(),
    )
    .unwrap();
    let saved = open(plan.bytes());
    let table = table_of(&saved.document);
    assert!(!table.has_props_revision(None));
    assert_eq!(table.alignment, Some(TableAlign::Center));
    assert!(table.rows[0].header && !table.rows[0].cant_split);
    assert_eq!(
        table.rows[0].cells[0].vertical_align,
        Some(VerticalAlign::Bottom)
    );
    assert_eq!(table.rows[0].cells[0].shading, None);
    assert!(
        session
            .document()
            .semantic_difference(&saved.document)
            .is_none()
    );
}

#[test]
fn out_of_subset_table_property_changes_keep_the_table_preserved_and_the_document_editable() {
    // Each case is outside the typed subset: the whole table stays an opaque
    // preserved block (so the markup cannot be dropped) and the document is
    // not read-only. Nothing is typed and the original bytes round-trip.
    let cases = [
        (
            "two previous-property children",
            table_body(
                r#"<w:tblPrChange w:id="71" w:author="A"><w:tblPr/><w:tblPr/></w:tblPrChange>"#,
                "",
                "",
            ),
        ),
        (
            "wrong previous-property child",
            table_body(
                r#"<w:tblPrChange w:id="72" w:author="A"><w:trPr/></w:tblPrChange>"#,
                "",
                "",
            ),
        ),
        (
            "unrepresentable previous table property",
            table_body(
                r#"<w:tblPrChange w:id="73" w:author="A"><w:tblPr><w:tblBorders><w:top w:val="single"/></w:tblBorders></w:tblPr></w:tblPrChange>"#,
                "",
                "",
            ),
        ),
        (
            "missing author",
            table_body(
                r#"<w:tblPrChange w:id="74"><w:tblPr/></w:tblPrChange>"#,
                "",
                "",
            ),
        ),
        (
            "duplicate change element",
            table_body(
                r#"<w:tblPrChange w:id="75" w:author="A"/><w:tblPrChange w:id="76" w:author="A"/>"#,
                "",
                "",
            ),
        ),
        (
            "nested change inside the previous row properties",
            table_body(
                "",
                r#"<w:trPrChange w:id="77" w:author="B"><w:trPr><w:trPrChange w:id="78" w:author="B"/></w:trPr></w:trPrChange>"#,
                "",
            ),
        ),
        (
            "nested change inside the previous cell properties",
            table_body(
                "",
                "",
                r#"<w:tcPrChange w:id="79" w:author="C"><w:tcPr><w:tcPrChange w:id="80" w:author="C"/></w:tcPr></w:tcPrChange>"#,
            ),
        ),
    ];
    for (name, body) in cases {
        let bytes = package(&body);
        let result = open(&bytes);
        assert!(
            !result.editability.is_read_only(),
            "{name}: an out-of-subset table change must not lock the document, got {:?}",
            result.editability.read_only_reasons().collect::<Vec<_>>()
        );
        assert!(
            matches!(result.document.body().blocks.get(1), Some(Block::Opaque(_))),
            "{name}: the table must stay preserved, got {:?}",
            result.document.body().blocks.get(1)
        );
        let copy = plan_export(
            &result.backing,
            &result.document.snapshot(),
            &Budget::default(),
        )
        .unwrap();
        assert_eq!(copy.mode(), ExportMode::OriginalPackage);
        assert_eq!(copy.bytes(), &bytes[..], "{name}: bytes must round-trip");
    }
}

#[test]
fn an_empty_change_element_rejects_back_to_absent_properties() {
    // `<w:tblPrChange/>` without a previous `w:tblPr` records that the table
    // had no properties before the change; rejecting it clears them.
    let body = table_body(r#"<w:tblPrChange w:id="81" w:author="A"/>"#, "", "");
    let source = open(&package(&body));
    assert!(!source.editability.is_read_only());
    let mut session = Session::new(source.document.clone());
    exec(&mut session, vec![Command::RejectRevision { id: 81 }]);
    let table = table_of(session.document());
    assert!(table.props_revision.is_none());
    assert_eq!(table.alignment, None);
    let plan = plan_export(
        &source.backing,
        &session.document().snapshot(),
        &Budget::default(),
    )
    .unwrap();
    let saved = open(plan.bytes());
    assert_eq!(table_of(&saved.document).alignment, None);
}
