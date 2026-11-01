use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_engine::{CommandError, Selection, TxError};
use joyword_model::{Bias, Block, Position, ScalarOffset};
use joyword_shell::{
    docx::DocxSession,
    input::{InputError, InputLimits, TextInput},
};

fn package(body: &str) -> Vec<u8> {
    let source = Package::read(
        include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-001.docx"),
        &Budget::default(),
    )
    .unwrap();
    let xml = format!(
        r#"<w:document xmlns:w="{}" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml"><w:body>{body}<w:sectPr/></w:body></w:document>"#,
        joyword_docx::W
    );
    let mut items = vec![(
        "[Content_Types].xml",
        source.content_types_xml(),
        Method::Deflated,
    )];
    for p in source.parts() {
        items.push((
            p.name.item_name(),
            if p.name.as_str() == "/word/document.xml" {
                xml.as_bytes()
            } else {
                &p.data
            },
            p.method,
        ));
    }
    write_archive(&items).unwrap()
}

fn table(inserted: bool) -> String {
    let revision = if inserted {
        r#"<w:ins w:id="91" w:author="Test"/>"#
    } else {
        ""
    };
    format!(
        r#"<w:tbl><w:tblPr><w:jc w:val="center"/><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="23465A"/><w:insideH w:val="single" w:sz="4" w:space="0"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr w:rsidR="12345678" w14:paraId="12345678"><w:trPr><w:cantSplit/><w:tblHeader/><w:jc w:val="center"/>{revision}</w:trPr><w:tc><w:tcPr><w:tcW w:type="dxa" w:w="4000"/><w:shd w:fill="23465A"/><w:vAlign w:val="center"/></w:tcPr><w:p w14:paraId="11111111"><w:pPr><w:spacing w:after="20" w:line="264" w:lineRule="auto"/></w:pPr><w:proofErr w:type="spellStart"/><w:r><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri"/><w:sz w:val="18"/><w:lang w:val="en-US"/></w:rPr><w:t>AV</w:t><w:br/><w:t>fi</w:t></w:r><w:proofErr w:type="spellEnd"/></w:p></w:tc></w:tr></w:tbl>"#
    )
}

fn first_cell<B: joyword_shell::input::InputBackend>(input: &TextInput<B>) -> Position {
    let d = joyword_shell::input::InputBackend::session(input.backend()).document();
    let t = d
        .body()
        .blocks
        .iter()
        .find_map(|b| {
            if let Block::Table(t) = b {
                Some(t)
            } else {
                None
            }
        })
        .unwrap();
    Position {
        story: d.body_id(),
        paragraph: t.rows[0].cells[0].blocks.get(0).unwrap().id(),
        offset: ScalarOffset(0),
        bias: Bias::After,
    }
}

#[test]
fn word_table_metadata_soft_breaks_and_cell_edit_round_trip_without_rewriting_neighbors() {
    let bytes = package(&table(false));
    let mut input = TextInput::new(
        DocxSession::open_default(&bytes).unwrap(),
        InputLimits::default(),
    )
    .unwrap();
    assert!(!input.read_only());
    assert!(!input.view().index().is_read_only_preview());
    let Block::Table(t) = input
        .backend()
        .session()
        .document()
        .body()
        .blocks
        .get(0)
        .unwrap()
    else {
        panic!("typed table")
    };
    assert_eq!(t.borders[0].unwrap().size_eighths, 4);
    assert!(t.rows[0].header && t.rows[0].cant_split);
    input
        .select_positions(Selection::caret(first_cell(&input)))
        .unwrap();
    input.replace(None, "Z").unwrap();
    let plan = input.backend().export_plan().unwrap();
    let old = Package::read(&bytes, &Budget::default()).unwrap();
    let new = Package::read(plan.bytes(), &Budget::default()).unwrap();
    for part in old
        .parts()
        .filter(|p| p.name.as_str() != "/word/document.xml")
    {
        assert_eq!(new.part(part.name.as_str()).unwrap().data, part.data);
    }
    let old_xml = std::str::from_utf8(&old.part("/word/document.xml").unwrap().data).unwrap();
    let new_xml = std::str::from_utf8(&new.part("/word/document.xml").unwrap().data).unwrap();
    let before = old_xml.find("<w:p ").unwrap();
    let after = old_xml.find("</w:p>").unwrap() + 6;
    assert_eq!(&old_xml[..before], &new_xml[..before]);
    assert_eq!(
        &old_xml[after..],
        &new_xml[new_xml.find("</w:p>").unwrap() + 6..]
    );
    let saved = DocxSession::open_default(plan.bytes()).unwrap();
    assert!(
        input
            .backend()
            .session()
            .document()
            .semantic_difference(saved.session().document())
            .is_none()
    );
    input.undo().unwrap();
    assert_eq!(input.backend().export_plan().unwrap().bytes(), bytes);
}

#[test]
fn row_insertion_revision_survives_text_edits_and_structural_edits_are_refused() {
    let bytes = package(&table(true));
    let mut input = TextInput::new(
        DocxSession::open_default(&bytes).unwrap(),
        InputLimits::default(),
    )
    .unwrap();
    input
        .select_positions(Selection::caret(first_cell(&input)))
        .unwrap();
    input.replace(None, "Z").unwrap();
    let plan = input.backend().export_plan().unwrap();
    let saved = DocxSession::open_default(plan.bytes()).unwrap();
    let Block::Table(t) = saved.session().document().body().blocks.get(0).unwrap() else {
        panic!("table")
    };
    assert_eq!(t.rows[0].insertion.as_ref().unwrap().id, 91);
    assert!(
        input
            .backend()
            .session()
            .document()
            .has_table_row_insertions()
    );
    assert!(input.accept_all_revisions().is_err());
    let version = input.backend().session().version();
    assert!(input.delete_table_row().is_err());
    assert_eq!(version, input.backend().session().version());
}

#[test]
fn typed_table_document_keeps_unknown_paragraphs_locally_locked() {
    let bytes = package(&format!(
        r#"{}<w:p><w:r><w:object/></w:r></w:p>"#,
        table(false)
    ));
    let mut input = TextInput::new(
        DocxSession::open_default(&bytes).unwrap(),
        InputLimits::default(),
    )
    .unwrap();
    assert!(!input.read_only());
    let locked = *input
        .backend()
        .session()
        .locked_nodes()
        .iter()
        .next()
        .unwrap();
    let pos = Position {
        story: input.view().snapshot().body_id(),
        paragraph: locked,
        offset: ScalarOffset(0),
        bias: Bias::After,
    };
    input.select_positions(Selection::caret(pos)).unwrap();
    let version = input.backend().session().version();
    assert!(matches!(
        input.replace(None, "X"),
        Err(InputError::Transaction(TxError::Command {
            error: CommandError::Locked(_),
            ..
        }))
    ));
    assert_eq!(version, input.backend().session().version());
    assert!(!input.can_undo());
    input
        .select_positions(Selection::caret(first_cell(&input)))
        .unwrap();
    input.replace(None, "Z").unwrap();
    assert!(input.backend().export_plan().is_ok());
}

#[test]
fn replacing_an_entire_header_retains_its_character_format_and_undo() {
    let body = table(false).replace("<w:br/>", "").replace(
        "<w:sz w:val=\"18\"/>",
        "<w:sz w:val=\"18\"/><w:b/><w:color w:val=\"FFFFFF\"/>",
    );
    let bytes = package(&body);
    let mut input = TextInput::new(
        DocxSession::open_default(&bytes).unwrap(),
        InputLimits::default(),
    )
    .unwrap();
    let start = first_cell(&input);
    let mut end = start;
    end.offset = ScalarOffset(4);
    input
        .select_positions(Selection {
            anchor: start,
            head: end,
        })
        .unwrap();
    input.replace(None, "New header").unwrap();
    let plan = input.backend().export_plan().unwrap();
    let saved = DocxSession::open_default(plan.bytes()).unwrap();
    let Block::Table(t) = saved.session().document().body().blocks.get(0).unwrap() else {
        panic!("table")
    };
    let p = t.rows[0].cells[0]
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap();
    assert_eq!(p.plain_text(|_| String::new()), "New header");
    assert!(p.runs().iter().all(|r| r.props.bold == Some(true)
        && r.props.color == Some(joyword_model::Color::Rgb(255, 255, 255))));
    input.undo().unwrap();
    assert_eq!(input.backend().export_plan().unwrap().bytes(), bytes);
    input.redo().unwrap();
    assert!(input.backend().export_plan().is_ok());
}

#[test]
fn repeated_original_saves_refresh_cell_sources_and_keep_live_local_locks() {
    use joyword_shell::{docx::BoundDocx, storage::OriginalChoice};
    let bytes = package(&format!(
        r#"<w:p><w:r><w:t>Start</w:t></w:r></w:p>{}<w:p><w:r><w:object/></w:r></w:p>"#,
        table(false)
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rich.docx");
    std::fs::write(&path, &bytes).unwrap();
    let mut input = TextInput::new(
        BoundDocx::open_default(&path).unwrap(),
        InputLimits::default(),
    )
    .unwrap();
    let locked = input.backend().session().session().locked_nodes().clone();
    input.replace(None, "Lead\n").unwrap();
    input.save_original(OriginalChoice::Confirm).unwrap();
    assert_eq!(input.backend().session().session().locked_nodes(), &locked);
    input
        .select_positions(Selection::caret(first_cell(&input)))
        .unwrap();
    input.replace(None, "Z").unwrap();
    input.save_original(OriginalChoice::Confirm).unwrap();
    assert!(!input.has_unsaved_changes());
    assert_eq!(input.backend().session().session().locked_nodes(), &locked);
    input.undo().unwrap();
    input.save_original(OriginalChoice::Confirm).unwrap();
    assert!(!input.has_unsaved_changes());
}
