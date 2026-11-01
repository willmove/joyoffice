//! Opening a preserved DOCX body must remain useful without weakening edit
//! restrictions or replacing the original package with a text-only export.
use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{TextPreviewError, TextPreviewLimits, W, text_preview};
use joyword_engine::Command;
use joyword_shell::docx::{BoundDocx, DocxSession};
use joyword_shell::input::{EditRequest, InputError, InputLimits, TextInput};

fn package(body: &str) -> Vec<u8> {
    let source = include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-001.docx");
    let package = Package::read(source, &Budget::default()).unwrap();
    let xml =
        format!(r#"<w:document xmlns:w="{W}"><w:body>{body}<w:sectPr/></w:body></w:document>"#);
    let mut items = vec![(
        "[Content_Types].xml",
        package.content_types_xml(),
        Method::Deflated,
    )];
    for part in package.parts() {
        items.push((
            part.name.item_name(),
            if part.name.as_str() == "/word/document.xml" {
                xml.as_bytes()
            } else {
                &part.data
            },
            part.method,
        ));
    }
    write_archive(&items).unwrap()
}

fn table(content: &str) -> String {
    // tblOverlap is deliberately outside the editable table subset.
    format!(
        r#"<w:tbl><w:tblPr><w:tblOverlap w:val="never"/></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:tc>{content}</w:tc></w:tr></w:tbl>"#
    )
}

fn find(query: &str) -> EditRequest {
    EditRequest::Find {
        query: query.into(),
        case_sensitive: true,
        whole_word: false,
        fold_width: false,
        regex: false,
        forward: true,
    }
}

#[test]
fn opaque_table_opens_as_searchable_read_only_text_and_saves_the_original_package() {
    let bytes = package(&format!(
        r#"<w:p><w:r><w:t>before</w:t></w:r></w:p>{}<w:p><w:r><w:t>after</w:t></w:r></w:p>"#,
        table(r#"<w:p><w:r><w:t>cell😀</w:t><w:br/><w:t>continued</w:t></w:r></w:p>"#)
    ));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.docx");
    std::fs::write(&path, &bytes).unwrap();
    let bound = BoundDocx::open_default(&path).unwrap();
    let source = bound.session().session().snapshot();
    let mut input = TextInput::new(bound, InputLimits::default()).unwrap();
    assert!(input.view().index().is_read_only_preview());
    assert!(input.read_only());
    assert_eq!(
        input.view().index().text(),
        "before\n[Table]\n[Cell]\ncell😀\ncontinued\n[/Table]\nafter"
    );
    let report = input.editing(find("cell😀")).unwrap();
    assert_eq!(report.matches, 1);
    assert_eq!(input.selected_text().unwrap(), "cell😀");
    input.editing(find("after")).unwrap();
    assert_eq!(input.selected_text().unwrap(), "after");
    assert_eq!(input.replace(None, "changed"), Err(InputError::ReadOnly));
    assert_eq!(input.mark(None, "IME", None), Err(InputError::ReadOnly));
    assert_eq!(input.undo(), Err(InputError::ReadOnly));
    assert_eq!(input.redo(), Err(InputError::ReadOnly));
    assert_eq!(
        input.apply_commands(vec![Command::InsertText {
            at: input.view().selection().head,
            text: "changed".into(),
        }]),
        Err(InputError::ReadOnly)
    );
    assert!(!input.can_undo());
    assert!(!input.has_unsaved_changes());
    assert_eq!(
        input
            .backend()
            .session()
            .session()
            .document()
            .canonical_dump(joyword_model::DumpOptions { node_ids: true }),
        source.canonical_dump(joyword_model::DumpOptions { node_ids: true })
    );
    let destination = directory.path().join("copy.docx");
    input.backend().save_copy_job().run(&destination).unwrap();
    assert_eq!(std::fs::read(destination).unwrap(), bytes);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    let reopened = DocxSession::open_default(&bytes).unwrap();
    assert_eq!(
        reopened
            .session()
            .document()
            .canonical_dump(joyword_model::DumpOptions { node_ids: true }),
        source.canonical_dump(joyword_model::DumpOptions { node_ids: true })
    );
    assert!(
        joyword_docx::plan_export(
            input.backend().session().backing(),
            input.view().snapshot(),
            &Budget::default()
        )
        .is_err(),
        "the display document must never be accepted as a source export"
    );
}

#[test]
fn soft_breaks_stay_editable_and_round_trip_without_a_text_preview() {
    let bytes = package(r#"<w:p><w:r><w:t>first</w:t><w:br/><w:t>second</w:t></w:r></w:p>"#);
    let mut input = TextInput::new(
        DocxSession::open_default(&bytes).unwrap(),
        InputLimits::default(),
    )
    .unwrap();
    assert!(!input.view().index().is_read_only_preview());
    assert!(!input.read_only());
    input.editing(find("second")).unwrap();
    input.replace(None, "next").unwrap();
    let saved = input.backend().export_plan().unwrap();
    let reopened = DocxSession::open_default(saved.bytes()).unwrap();
    let mut input = TextInput::new(reopened, InputLimits::default()).unwrap();
    assert_eq!(input.editing(find("next")).unwrap().matches, 1);
}

#[test]
fn preview_uses_stored_results_and_content_controls_without_leaking_hidden_or_deleted_text() {
    let bytes = package(&table(
        r#"<w:p><w:r><w:t>visible</w:t></w:r><w:del w:id="7" w:author="Test"><w:r><w:delText>deleted</w:delText></w:r></w:del><w:r><w:rPr><w:vanish/></w:rPr><w:t>hidden</w:t></w:r><w:r><w:instrText>INCLUDETEXT remote-secret</w:instrText></w:r><w:fldSimple w:instr="INCLUDETEXT remote-secret"><w:r><w:t>cached</w:t></w:r></w:fldSimple><w:sdt><w:sdtPr><w:alias w:val="metadata-secret"/></w:sdtPr><w:sdtContent><w:r><w:t>control</w:t></w:r></w:sdtContent></w:sdt><w:r><w:drawing/></w:r></w:p>"#,
    ));
    let session = DocxSession::open_default(&bytes).unwrap();
    let input = TextInput::new(session, InputLimits::default()).unwrap();
    assert_eq!(
        input.view().index().text(),
        "[Table]\n[Cell]\nvisiblecachedcontrol[Object]\n[/Table]"
    );
    assert_eq!(input.backend().backing().original_bytes(), bytes);
}

#[test]
fn compatibility_preview_preserves_text_and_xml_budget_failures() {
    let bytes = package(&table(
        r#"<w:p><w:r><w:t>long table text</w:t></w:r></w:p>"#,
    ));
    for (limits, expected) in [
        (
            InputLimits {
                max_paragraphs: 2,
                ..InputLimits::default()
            },
            "paragraphs",
        ),
        (
            InputLimits {
                max_paragraph_bytes: 4,
                ..InputLimits::default()
            },
            "paragraph bytes",
        ),
        (
            InputLimits {
                max_text_bytes: 8,
                ..InputLimits::default()
            },
            "text bytes",
        ),
        (
            InputLimits {
                max_runs: 1,
                ..InputLimits::default()
            },
            "runs",
        ),
    ] {
        assert!(
            matches!(TextInput::new(DocxSession::open_default(&bytes).unwrap(), limits),
            Err(InputError::Budget(label)) if label == expected)
        );
    }
    let session = DocxSession::open_default(&bytes).unwrap();
    for budget in [
        Budget {
            max_xml_depth: 2,
            ..Budget::default()
        },
        Budget {
            max_xml_nodes: 4,
            ..Budget::default()
        },
    ] {
        assert!(matches!(
            text_preview(
                session.backing(),
                session.session().document(),
                &budget,
                TextPreviewLimits::default()
            ),
            Err(TextPreviewError::Xml(_))
        ));
    }
}

#[test]
fn supported_docx_body_keeps_the_editable_input_path() {
    let bytes = package(r#"<w:p><w:r><w:t>original</w:t></w:r></w:p>"#);
    let session = DocxSession::open_default(&bytes).unwrap();
    let mut input = TextInput::new(session, InputLimits::default()).unwrap();
    assert!(!input.view().index().is_read_only_preview());
    assert!(!input.read_only());
    input.select_all().unwrap();
    input.replace(None, "editable").unwrap();
    assert_eq!(input.view().index().text(), "editable");
    input.undo().unwrap();
    assert_eq!(input.view().index().text(), "original");
    assert_eq!(input.backend().export_plan().unwrap().bytes(), bytes);
}
