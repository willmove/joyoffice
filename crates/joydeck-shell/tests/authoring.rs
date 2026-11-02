#![allow(clippy::unwrap_used, clippy::expect_used, reason = "Test assertions")]
use joydeck_engine::{
    Command, TableAction, TextTarget,
    text::{self, Draft},
};
use joydeck_model::{ShapeId, ShapeKind, TextBody, text::Run};
use joydeck_shell::{Cancellation, Document, editor::Editor};
const PPTX: &[u8] = include_bytes!("../../../tests/fixtures/joydeck/independent.pptx");
fn editor() -> (Editor, std::path::PathBuf) {
    let root = joydeck_shell::platform::private_directory("r3-tests").unwrap();
    let input = root.join("input.pptx");
    std::fs::write(&input, PPTX).unwrap();
    (Editor::open(&input, root.join("recovery")).unwrap(), root)
}
#[test]
fn composition_ranges_graphemes_cancel_and_single_commit() {
    let (mut e, _) = editor();
    let original = e.session.snapshot();
    let mut d = Draft::new(TextBody::from_text("A😀e\u{301}中"));
    assert_eq!(text::utf16_range(&d.body.text(), 2..3), None);
    assert_eq!(text::previous("A😀e\u{301}中", 8), 5);
    d.move_to(5, false);
    d.replace(None, "pin", true, Some(3..3)).unwrap();
    assert!(d.marked.is_some());
    assert_eq!(e.session.snapshot().model, original.model);
    let before = d.body.clone();
    assert!(d.replace(Some(2..3), "bad", true, None).is_err());
    assert_eq!(d.body, before);
    d.cancel();
    assert_eq!(d.body.text(), "A😀e\u{301}中");
    assert_eq!(d.selection, 5..5);
    d.replace(None, "zhong", true, None).unwrap();
    d.replace(None, "中文", false, None).unwrap();
    e.session
        .execute(Command::SetText {
            target: TextTarget::Shape { shape: ShapeId(2) },
            body: Box::new(d.body.clone()),
        })
        .unwrap();
    assert_eq!(e.session.revision(), 1);
    e.session.execute(Command::Undo).unwrap();
    assert_eq!(e.session.model(), original.model.as_ref());
    e.session.execute(Command::Redo).unwrap();
    assert_eq!(
        e.session.model().slides[0].shapes[0].text.as_ref().unwrap(),
        &d.body
    );
}
#[test]
fn range_edit_preserves_other_runs_and_paragraphs() {
    let mut b = TextBody::from_text("one TWO\nlast");
    let p = &mut b.paragraphs[0];
    let mut a = Run::new("one ");
    a.props.bold = Some(true);
    let mut z = Run::new("TWO");
    z.props.italic = Some(true);
    p.runs = vec![a, z];
    let last = b.paragraphs[1].clone();
    text::replace(&mut b, 4..7, "二😀").unwrap();
    assert_eq!(b.text(), "one 二😀\nlast");
    assert_eq!(b.paragraphs[0].runs[0].props.bold, Some(true));
    assert_eq!(b.paragraphs[0].runs[1].props.italic, Some(true));
    assert_eq!(b.paragraphs[1], last);
}
#[test]
fn real_pptx_table_cells_tsv_merge_fill_undo_and_reopen() {
    let (mut e, root) = editor();
    let s = e.session.model().slides[1]
        .shapes
        .iter()
        .find(|s| matches!(s.kind, ShapeKind::Table(_)))
        .unwrap()
        .id;
    e.session.select(1, vec![s]).unwrap();
    e.session
        .execute(Command::Table {
            action: TableAction::PasteTsv {
                row: 0,
                col: 0,
                text: "中文😀\tB\nC\tD".into(),
            },
        })
        .unwrap();
    let snapshot = e.session.snapshot();
    assert!(
        e.session
            .execute(Command::Table {
                action: TableAction::PasteTsv {
                    row: 2,
                    col: 1,
                    text: "one\ttwo".into()
                }
            })
            .is_err()
    );
    assert_eq!(snapshot.model, e.session.snapshot().model);
    e.session
        .execute(Command::Table {
            action: TableAction::Fill {
                row: 0,
                col: 0,
                rgb: 0xffe8a0,
            },
        })
        .unwrap();
    e.session
        .execute(Command::Table {
            action: TableAction::Merge {
                row: 1,
                col: 0,
                end_row: 1,
                end_col: 1,
            },
        })
        .unwrap();
    let output = root.join("cells.pptx");
    e.save_job(output.clone()).unwrap().run().unwrap();
    let read = Document::open(&output, &Cancellation::default()).unwrap();
    let ShapeKind::Table(t) = &read.presentation.slides[1].shape(s).unwrap().kind else {
        unreachable!()
    };
    assert_eq!(t.cell(0, 0).unwrap().text.text(), "中文😀");
    assert_eq!(t.cell(1, 0).unwrap().grid_span, 2);
    assert!(t.cell(1, 1).unwrap().h_merge);
    e.session.execute(Command::Undo).unwrap();
    e.session.execute(Command::Redo).unwrap();
    let native = root.join("cells.joydeck");
    e.save_job(native.clone()).unwrap().run().unwrap();
    assert_eq!(
        Document::open(&native, &Cancellation::default())
            .unwrap()
            .presentation,
        e.session.model().clone()
    );
    assert_eq!(std::fs::read(root.join("input.pptx")).unwrap(), PPTX);
}
#[test]
fn table_structure_insert_delete_and_new_table_public_roundtrip() {
    let (mut e, root) = editor();
    let id = e.session.model().slides[1]
        .shapes
        .iter()
        .find(|s| matches!(s.kind, ShapeKind::Table(_)))
        .unwrap()
        .id;
    e.session.select(1, vec![id]).unwrap();
    for action in [
        TableAction::InsertRow { at: 1 },
        TableAction::InsertCol { at: 1 },
        TableAction::DeleteRow { at: 0 },
        TableAction::DeleteCol { at: 0 },
    ] {
        e.session.execute(Command::Table { action }).unwrap();
    }
    e.save_job(root.join("structure.pptx"))
        .unwrap()
        .run()
        .unwrap();
    e.session
        .execute(Command::InsertTable {
            x: 40.,
            y: 40.,
            rows: 2,
            cols: 3,
            width: 360.,
            row_height: 40.,
        })
        .unwrap();
    let path = root.join("new-table.pptx");
    e.save_job(path.clone()).unwrap().run().unwrap();
    let doc = Document::open(&path, &Cancellation::default()).unwrap();
    assert_eq!(
        doc.presentation.slides[1].shapes.len(),
        e.session.model().slides[1].shapes.len()
    );
}
#[test]
fn notes_public_and_native_preserve_speaker_body() {
    let (mut e, root) = editor();
    e.session
        .execute(Command::SetText {
            target: TextTarget::Notes,
            body: Box::new(TextBody::from_text("R3 notes 中文")),
        })
        .unwrap();
    let path = root.join("notes.pptx");
    e.save_job(path.clone()).unwrap().run().unwrap();
    assert_eq!(
        Document::open(&path, &Cancellation::default())
            .unwrap()
            .presentation
            .slides[0]
            .notes
            .text(),
        "R3 notes 中文"
    );
    e.save_job(root.join("notes.joydeck"))
        .unwrap()
        .run()
        .unwrap();
    e.session
        .execute(Command::SetText {
            target: TextTarget::Notes,
            body: Box::new(TextBody::default()),
        })
        .unwrap();
    let path = root.join("cleared-notes.pptx");
    e.save_job(path.clone()).unwrap().run().unwrap();
    assert!(
        Document::open(&path, &Cancellation::default())
            .unwrap()
            .presentation
            .slides[0]
            .notes
            .is_empty()
    );
}

#[test]
fn formatted_text_public_roundtrip_and_shared_caret_geometry() {
    let (mut e, root) = editor();
    let id = ShapeId(2);
    let mut body = e.session.model().slides[0]
        .shape(id)
        .unwrap()
        .text
        .clone()
        .unwrap();
    joydeck_model::edit::format_all(&mut body, &|p| {
        p.bold = Some(true);
        p.italic = Some(true);
    });
    e.session
        .execute(Command::SetText {
            target: TextTarget::Shape { shape: id },
            body: Box::new(body),
        })
        .unwrap();
    let path = root.join("formatted.pptx");
    e.save_job(path.clone()).unwrap().run().unwrap();
    let d = Document::open(&path, &Cancellation::default()).unwrap();
    let b = d.presentation.slides[0]
        .shape(id)
        .unwrap()
        .text
        .as_ref()
        .unwrap();
    assert_eq!(b.paragraphs[0].runs[0].props.bold, Some(true));
    assert_eq!(b.paragraphs[0].runs[0].props.italic, Some(true));
    let p = joydeck_render::editable_text(&d.presentation, 0, id, None).unwrap();
    let at = joydeck_text::Pos { para: 0, ch: 3 };
    let (x, top, bottom) = p.layout.caret(at).unwrap();
    let hit = p
        .layout
        .hit(joydeck_geom::Point::new(x, (top + bottom) * 0.5));
    assert_eq!(hit, at);
}

#[test]
fn composition_invalid_and_oversized_updates_leave_draft_unchanged() {
    let mut d = Draft::new(TextBody::from_text("😀e\u{301}"));
    let before = d.body.clone();
    let selection = d.selection.clone();
    assert!(d.replace(Some(1..2), "x", true, None).is_err());
    assert!(d.replace(None, "中", true, Some(0..2)).is_err());
    assert!(d.replace(None, &"a".repeat(65537), true, None).is_err());
    assert_eq!(d.body, before);
    assert_eq!(d.selection, selection);
    assert!(d.marked.is_none());
    d.replace(None, "ni", true, Some(2..2)).unwrap();
    d.replace(None, "", true, None).unwrap();
    assert_eq!(d.body, before);
    d.replace(None, "ni", true, None).unwrap();
    d.settle();
    assert!(d.marked.is_none());
    assert_eq!(d.body.text(), "ni");
}

#[test]
fn tsv_paste_that_hits_covered_cell_is_atomic() {
    let (mut e, _) = editor();
    let id = e.session.model().slides[1]
        .shapes
        .iter()
        .find(|s| matches!(s.kind, ShapeKind::Table(_)))
        .unwrap()
        .id;
    e.session.select(1, vec![id]).unwrap();
    e.session
        .execute(Command::Table {
            action: TableAction::Merge {
                row: 1,
                col: 0,
                end_row: 1,
                end_col: 1,
            },
        })
        .unwrap();
    let before = e.session.snapshot();
    let revision = e.session.revision();
    assert!(
        e.session
            .execute(Command::Table {
                action: TableAction::PasteTsv {
                    row: 0,
                    col: 0,
                    text: "A\tB\nC\tD".into()
                }
            })
            .is_err()
    );
    assert_eq!(e.session.snapshot().model, before.model);
    assert_eq!(e.session.revision(), revision);
}
