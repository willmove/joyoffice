use joyoffice_opc::Budget;
use joyword_docx::{create_text_document, import};
use joyword_model::{Block, Document, Paragraph, RunProps, SeqTree};
use std::sync::Arc;

fn projected() -> Document {
    let mut doc = Document::new_blank();
    let mut paragraph = Paragraph::new(doc.ids().node());
    paragraph
        .push_text(
            "中文😀 <&> ",
            None,
            Arc::new(RunProps {
                bold: Some(true),
                ..RunProps::default()
            }),
        )
        .unwrap();
    paragraph
        .push_text(
            "tail",
            None,
            Arc::new(RunProps {
                bold: Some(false),
                italic: Some(true),
                ..RunProps::default()
            }),
        )
        .unwrap();
    let body = doc.body_id();
    doc.story_mut(body).unwrap().blocks =
        SeqTree::from_vec(vec![Block::Paragraph(Arc::new(paragraph))]);
    doc
}

#[test]
fn new_projection_docx_reopens_with_unicode_formatting_and_mode_15() {
    let document = projected();
    let bytes = create_text_document(&document, &Budget::default()).unwrap();
    assert_eq!(
        bytes,
        create_text_document(&document, &Budget::default()).unwrap()
    );
    let reopened = import(&bytes, &Budget::default()).unwrap();
    assert!(reopened.editability.is_unrestricted());
    assert!(reopened.diagnostics.is_empty());
    assert_eq!(
        reopened.document.settings().effective_compatibility_mode(),
        15
    );
    let Block::Paragraph(paragraph) = reopened.document.body().blocks.get(0).unwrap() else {
        panic!()
    };
    assert_eq!(paragraph.raw_text(), "中文😀 <&> tail");
    assert_eq!(paragraph.runs()[0].props.bold, Some(true));
    assert_eq!(paragraph.runs()[1].props.bold, Some(false));
    assert_eq!(paragraph.runs()[1].props.italic, Some(true));
}

#[test]
fn creation_honors_the_supplied_package_and_xml_budgets() {
    let document = projected();
    for budget in [
        Budget {
            max_entries: 4,
            ..Budget::default()
        },
        Budget {
            max_part_bytes: 8,
            ..Budget::default()
        },
        Budget {
            max_total_bytes: 8,
            ..Budget::default()
        },
        Budget {
            max_xml_nodes: 1,
            ..Budget::default()
        },
    ] {
        assert!(create_text_document(&document, &budget).is_err());
    }
    assert!(create_text_document(&document, &Budget::default()).is_ok());
}

#[test]
fn unsupported_run_properties_cannot_disappear_in_a_new_document() {
    let mut document = Document::new_blank();
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph
        .push_text(
            "private",
            None,
            Arc::new(RunProps {
                hidden: Some(true),
                ..RunProps::default()
            }),
        )
        .unwrap();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks =
        SeqTree::from_vec(vec![Block::Paragraph(Arc::new(paragraph))]);
    assert!(create_text_document(&document, &Budget::default()).is_err());
}
