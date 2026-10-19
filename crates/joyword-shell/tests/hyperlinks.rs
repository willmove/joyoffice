//! Hyperlink edits are one undo step. Opening is an allowlist, not a fetch.

use std::sync::Arc;

use joyword_engine::Session;
use joyword_model::{
    AbstractNum, Block, BlockSeq, Document, Hyperlink, NumInstance, NumberFormat, NumberingLevel,
    OpenRefusal, Paragraph, open_target,
};
use joyword_shell::input::{InputLimits, TextInput};

fn input(text: &str) -> TextInput {
    let mut document = Document::new_blank();
    let mut blocks = Vec::new();
    for line in text.split('\n') {
        let mut paragraph = Paragraph::new(document.ids().node());
        if !line.is_empty() {
            paragraph.push_text(line, None, Arc::default()).unwrap();
        }
        blocks.push(paragraph.into());
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    TextInput::new(Session::new(document), InputLimits::default()).unwrap()
}

fn link_at(editor: &TextInput, paragraph: usize, offset: usize) -> Option<Hyperlink> {
    editor
        .backend()
        .document()
        .body()
        .blocks
        .get(paragraph)
        .unwrap()
        .as_paragraph()
        .unwrap()
        .link_at(offset)
        .cloned()
}

#[test]
fn set_edit_and_remove_a_hyperlink_are_separate_undo_steps() {
    let mut editor = input("hello");
    editor.select(0..5, false).unwrap();
    assert!(
        editor
            .set_hyperlink(Some("javascript:alert(1)"), None)
            .is_err()
    );
    assert!(editor.set_hyperlink(Some("file:///tmp/a"), None).is_err());
    assert!(link_at(&editor, 0, 0).is_none());

    editor
        .set_hyperlink(Some("https://example.invalid/a"), None)
        .unwrap();
    let link = link_at(&editor, 0, 0).unwrap();
    assert_eq!(link.external.as_deref(), Some("https://example.invalid/a"));
    assert!(open_target(&link).is_ok());

    editor
        .set_hyperlink(Some("mailto:a@example.invalid"), Some("note"))
        .unwrap();
    let link = link_at(&editor, 0, 0).unwrap();
    assert_eq!(link.external.as_deref(), Some("mailto:a@example.invalid"));
    assert_eq!(link.anchor.as_deref(), Some("note"));

    editor.set_hyperlink(None, None).unwrap();
    assert!(link_at(&editor, 0, 0).is_none());
    editor.undo().unwrap();
    assert_eq!(
        link_at(&editor, 0, 0).unwrap().anchor.as_deref(),
        Some("note")
    );
    editor.undo().unwrap();
    assert_eq!(
        link_at(&editor, 0, 0).unwrap().external.as_deref(),
        Some("https://example.invalid/a")
    );
    editor.undo().unwrap();
    assert!(link_at(&editor, 0, 0).is_none());
    assert_eq!(editor.view().index().text(), "hello");
}

#[test]
fn openable_link_refuses_schemes_outside_the_allowlist() {
    let mut editor = input("go");
    editor.select(0..2, false).unwrap();
    editor
        .set_hyperlink(Some("https://example.invalid/a"), None)
        .unwrap();
    let anchor = editor.view().selection().anchor;
    assert_eq!(
        editor.openable_link(&anchor),
        Some("https://example.invalid/a")
    );

    let stored = Hyperlink::imported(None, Some(Arc::from("javascript:alert(1)")), None).unwrap();
    assert_eq!(open_target(&stored), Err(OpenRefusal::Scheme));
    let file = Hyperlink::imported(None, Some(Arc::from("file:///tmp/a")), None).unwrap();
    assert_eq!(open_target(&file), Err(OpenRefusal::Scheme));
    let data = Hyperlink::imported(None, Some(Arc::from("data:text/plain,a")), None).unwrap();
    assert_eq!(open_target(&data), Err(OpenRefusal::Scheme));
    let userinfo = Hyperlink::imported(
        None,
        Some(Arc::from("https://user@example.invalid/a")),
        None,
    )
    .unwrap();
    assert_eq!(open_target(&userinfo), Err(OpenRefusal::Malformed));
}

#[test]
fn an_empty_selection_does_not_create_a_hyperlink() {
    let mut editor = input("hello");
    assert!(
        !editor
            .set_hyperlink(Some("https://example.invalid/a"), None)
            .unwrap()
    );
    assert!(link_at(&editor, 0, 0).is_none());
}

#[test]
fn a_grounded_list_paste_uses_the_existing_numbering_instance() {
    let mut document = Document::new_blank();
    let mut abstract_num = AbstractNum::new(1);
    abstract_num.levels[0] = Some(NumberingLevel::new(NumberFormat::Decimal, "%1."));
    document.numbering_mut().insert_abstract(abstract_num);
    document.numbering_mut().insert_instance(NumInstance {
        id: 1,
        abstract_id: 1,
        overrides: Vec::new(),
    });
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text("", None, Arc::default()).ok();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let mut editor = TextInput::new(Session::new(document), InputLimits::default()).unwrap();
    let joyword_shell::rich_paste::ParsedRich::Formatted(parsed) =
        joyword_shell::rich_paste::parse_html("<ol><li>one</li><li>two</li></ol>")
    else {
        panic!("list");
    };
    editor.paste_rich(&parsed).unwrap();
    let paragraphs: Vec<_> = editor
        .backend()
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph())
        .collect();
    assert!(paragraphs.len() >= 2, "{}", paragraphs.len());
    assert_eq!(paragraphs[0].plain_text(|_| String::new()), "one");
    assert_eq!(paragraphs[1].plain_text(|_| String::new()), "two");
    assert_eq!(
        paragraphs[0].props.numbering.map(|item| item.num_id),
        Some(1)
    );
    assert_eq!(
        paragraphs[1].props.numbering.map(|item| item.num_id),
        Some(1)
    );
    editor.undo().unwrap();
    assert_eq!(editor.view().index().text(), "");
}

fn table_input(text: &str) -> TextInput {
    let mut document = Document::new_blank();
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text(text, None, Arc::default()).unwrap();
    let table = joyword_model::Table {
        id: document.ids().node(),
        grid: vec![joyword_model::Measure::Twips(2400)],
        rows: vec![joyword_model::Row {
            id: document.ids().node(),
            grid_before: 0,
            grid_after: 0,
            cells: vec![joyword_model::Cell {
                id: document.ids().node(),
                grid_span: 1,
                vertical_merge: joyword_model::VerticalMerge::None,
                width: Some(joyword_model::Measure::Twips(2400)),
                borders: joyword_model::CellBorders::default(),
                shading: None,
                margin: None,
                vertical_align: None,
                blocks: BlockSeq::from_vec(vec![paragraph.into()]),
                preserved: None,
            }],
            header: false,
            cant_split: false,
            height: None,
            preserved: None,
        }],
        style: None,
        width: None,
        layout: None,
        alignment: None,
        indent: None,
        look: None,
        cell_margin: None,
        preserved: None,
        source: None,
    };
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![table.into()]);
    TextInput::new(Session::new(document), InputLimits::default()).unwrap()
}

fn cell_link(editor: &TextInput) -> Option<Hyperlink> {
    let Block::Table(table) = editor.backend().document().body().blocks.get(0).unwrap() else {
        panic!("table");
    };
    table.rows[0].cells[0]
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap()
        .link_at(0)
        .cloned()
}

#[test]
fn a_hyperlink_inside_a_table_cell_is_set_edited_removed_and_openable() {
    let mut editor = table_input("cell");
    assert_eq!(editor.view().index().text(), "cell");
    editor.select(0..4, false).unwrap();
    editor
        .set_hyperlink(Some("https://example.invalid/cell"), None)
        .unwrap();
    let link = cell_link(&editor).expect("link in the cell");
    assert_eq!(
        link.external.as_deref(),
        Some("https://example.invalid/cell")
    );
    let anchor = editor.view().selection().anchor;
    assert_eq!(
        editor.openable_link(&anchor),
        Some("https://example.invalid/cell")
    );

    editor
        .set_hyperlink(Some("mailto:c@example.invalid"), None)
        .unwrap();
    assert_eq!(
        cell_link(&editor).unwrap().external.as_deref(),
        Some("mailto:c@example.invalid")
    );
    editor.set_hyperlink(None, None).unwrap();
    assert!(cell_link(&editor).is_none());
    editor.undo().unwrap();
    assert_eq!(
        cell_link(&editor).unwrap().external.as_deref(),
        Some("mailto:c@example.invalid")
    );
    editor.undo().unwrap();
    assert_eq!(
        cell_link(&editor).unwrap().external.as_deref(),
        Some("https://example.invalid/cell")
    );
    editor.undo().unwrap();
    assert!(cell_link(&editor).is_none());
    assert_eq!(editor.view().index().text(), "cell");
}
