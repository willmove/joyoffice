//! T201: undoable table structure edits, including merge and split.

use std::sync::Arc;

use joyword_engine::{
    CellAddress, Command, CommandError, Outcome, Selection, Session, TableMerge,
    TablePropertyChange, TableStructure, Transaction, TxError, cell_address, cell_caret,
    cell_selection, current_revision,
};
use joyword_model::{
    Bias, Block, BlockSeq, BorderEdge, BorderKind, Cell, CellBorders, CellMargins, Color, Document,
    Issue, Measure, NodeId, Paragraph, Position, RevisionKind, ScalarOffset, Table, TableAlign,
    TableWidth, VerticalMerge, table_structure,
};

fn blank(lines: &[&str]) -> Document {
    let mut document = Document::new_blank();
    let mut blocks = Vec::new();
    for line in lines {
        blocks.push(Block::from(paragraph(&mut document, line)));
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    document
}

fn paragraph(document: &mut Document, text: &str) -> Paragraph {
    let mut paragraph = Paragraph::new(document.ids().node());
    if !text.is_empty() {
        paragraph.push_text(text, None, Arc::default()).unwrap();
    }
    paragraph
}

fn cell(document: &mut Document, text: &str, span: u32, merge: VerticalMerge) -> Cell {
    let paragraph = paragraph(document, text);
    Cell {
        id: document.ids().node(),
        grid_span: span,
        vertical_merge: merge,
        width: Some(Measure::Twips(2000)),
        borders: CellBorders::default(),
        shading: None,
        margin: None,
        vertical_align: None,
        blocks: BlockSeq::from_vec(vec![paragraph.into()]),
        preserved: None,
    }
}

fn table(document: &mut Document, rows: Vec<Vec<Cell>>, columns: usize) -> Table {
    let rows = rows
        .into_iter()
        .map(|cells| joyword_model::Row {
            id: document.ids().node(),
            grid_before: 0,
            grid_after: 0,
            cells,
            header: false,
            cant_split: false,
            height: None,
            preserved: None,
        })
        .collect();
    Table {
        id: document.ids().node(),
        grid: vec![Measure::Twips(2000); columns],
        rows,
        style: None,
        width: None,
        layout: None,
        alignment: None,
        indent: None,
        look: None,
        cell_margin: None,
        preserved: None,
        source: None,
    }
}

fn install(document: &mut Document, built: Table) {
    let body = document.body_id();
    document
        .story_mut(body)
        .unwrap()
        .blocks
        .insert(0, built.into());
}

fn pos(session: &Session, paragraph: NodeId, offset: usize) -> Position {
    Position {
        story: session.document().body_id(),
        paragraph,
        offset: ScalarOffset(offset),
        bias: Bias::After,
    }
}

fn exec(session: &mut Session, commands: Vec<Command>) -> Result<Outcome, TxError> {
    session.apply(Transaction::new(session.version(), commands))
}

fn table_of(session: &Session) -> &Table {
    match session.document().body().blocks.get(0) {
        Some(Block::Table(table)) => table,
        other => panic!("expected a table, got {other:?}"),
    }
}

fn first_cell_paragraph(session: &Session) -> NodeId {
    table_of(session).rows[0].cells[0]
        .blocks
        .get(0)
        .unwrap()
        .id()
}

#[test]
fn grid_invariants_reject_a_short_row_and_an_orphan_merge() {
    let mut document = blank(&[""]);
    let one = cell(&mut document, "甲", 1, VerticalMerge::None);
    let short = table(&mut document, vec![vec![one]], 2);
    assert!(table_structure(&short).iter().any(|issue| {
        matches!(
            issue,
            Issue::TableGridMismatch {
                grid: 2,
                used: 1,
                ..
            }
        )
    }));
    let continued = cell(&mut document, "", 1, VerticalMerge::Continue);
    let orphan = table(&mut document, vec![vec![continued]], 1);
    assert!(
        table_structure(&orphan)
            .iter()
            .any(|issue| matches!(issue, Issue::OrphanVerticalMerge { .. }))
    );
}

#[test]
fn inserting_a_table_is_one_undoable_transaction() {
    let mut session = Session::new(blank(&["正文"]));
    let paragraph = session.document().body().blocks.get(0).unwrap().id();
    let at = pos(&session, paragraph, 0);
    let applied = exec(
        &mut session,
        vec![Command::InsertTable {
            at,
            rows: 2,
            columns: 2,
        }],
    )
    .unwrap();
    assert!(matches!(applied, Outcome::Applied(_)));
    assert!(matches!(
        session.document().body().blocks.get(0),
        Some(Block::Table(_))
    ));
    assert_eq!(
        table_of(&session).grid,
        vec![Measure::Twips(4153), Measure::Twips(4153)]
    );
    assert!(!session.document().validate().has_errors());
    let before = session.document().body().blocks.len();
    session.undo().unwrap();
    assert!(
        session
            .document()
            .body()
            .blocks
            .get(0)
            .unwrap()
            .as_paragraph()
            .is_some()
    );
    assert_eq!(
        session
            .document()
            .body()
            .blocks
            .get(0)
            .unwrap()
            .as_paragraph()
            .unwrap()
            .plain_text(|_| String::new()),
        "正文"
    );
    session.redo().unwrap();
    assert_eq!(session.document().body().blocks.len(), before);

    let version = session.version();
    let at = pos(&session, paragraph, 0);
    let error = exec(
        &mut session,
        vec![Command::InsertTable {
            at,
            rows: 0,
            columns: 2,
        }],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        TxError::Command {
            error: CommandError::TableLimit,
            ..
        }
    ));
    assert_eq!(session.version(), version);
}

#[test]
fn inserting_and_deleting_a_row_is_one_undoable_transaction() {
    let mut document = blank(&["正文"]);
    let first = cell(&mut document, "甲", 1, VerticalMerge::None);
    let second = cell(&mut document, "乙", 1, VerticalMerge::None);
    let third = cell(&mut document, "丙", 1, VerticalMerge::None);
    let fourth = cell(&mut document, "丁", 1, VerticalMerge::None);
    let built = table(
        &mut document,
        vec![vec![first, second], vec![third, fourth]],
        2,
    );
    install(&mut document, built);
    let mut session = Session::new(document);
    let paragraph = first_cell_paragraph(&session);
    let at = pos(&session, paragraph, 0);
    exec(&mut session, vec![Command::InsertTableRow { at }]).unwrap();
    assert_eq!(table_of(&session).rows.len(), 3);
    assert_eq!(
        table_of(&session).rows[1].cells[0]
            .blocks
            .get(0)
            .unwrap()
            .as_paragraph()
            .unwrap()
            .plain_text(|_| String::new()),
        ""
    );
    session.undo().unwrap();
    assert_eq!(table_of(&session).rows.len(), 2);
    session.redo().unwrap();
    assert_eq!(table_of(&session).rows.len(), 3);

    let only = {
        let mut document = blank(&[""]);
        let only_cell = cell(&mut document, "甲", 1, VerticalMerge::None);
        let built = table(&mut document, vec![vec![only_cell]], 1);
        install(&mut document, built);
        Session::new(document)
    };
    let mut session = only;
    let paragraph = first_cell_paragraph(&session);
    let at = pos(&session, paragraph, 0);
    let error = exec(&mut session, vec![Command::DeleteTableRow { at }]).unwrap_err();
    assert!(matches!(
        error,
        TxError::Command {
            error: CommandError::LastRow(_),
            ..
        }
    ));
    assert_eq!(table_of(&session).rows.len(), 1);
}

#[test]
fn inserting_and_deleting_a_column_is_one_undoable_transaction() {
    let mut document = blank(&["正文"]);
    let left = cell(&mut document, "甲", 1, VerticalMerge::None);
    let right = cell(&mut document, "乙", 1, VerticalMerge::None);
    let built = table(&mut document, vec![vec![left, right]], 2);
    install(&mut document, built);
    let mut session = Session::new(document);
    let paragraph = first_cell_paragraph(&session);
    let at = pos(&session, paragraph, 0);
    exec(&mut session, vec![Command::InsertTableColumn { at }]).unwrap();
    assert_eq!(table_of(&session).grid.len(), 3);
    assert_eq!(table_of(&session).rows[0].cells.len(), 3);
    session.undo().unwrap();
    assert_eq!(table_of(&session).grid.len(), 2);
    session.redo().unwrap();
    let at = pos(&session, paragraph, 0);
    exec(&mut session, vec![Command::DeleteTableColumn { at }]).unwrap();
    assert_eq!(table_of(&session).grid.len(), 2);
    session.undo().unwrap();
    assert_eq!(table_of(&session).grid.len(), 3);

    let mut lone = blank(&[""]);
    let only_cell = cell(&mut lone, "甲", 1, VerticalMerge::None);
    let built = table(&mut lone, vec![vec![only_cell]], 1);
    install(&mut lone, built);
    let mut session = Session::new(lone);
    let paragraph = first_cell_paragraph(&session);
    let at = pos(&session, paragraph, 0);
    let error = exec(&mut session, vec![Command::DeleteTableColumn { at }]).unwrap_err();
    assert!(matches!(
        error,
        TxError::Command {
            error: CommandError::LastColumn(_),
            ..
        }
    ));
}

#[test]
fn column_and_row_edits_refuse_merged_structure() {
    let mut document = blank(&[""]);
    let top = cell(&mut document, "合", 2, VerticalMerge::Restart);
    let bottom = cell(&mut document, "", 2, VerticalMerge::Continue);
    let built = table(&mut document, vec![vec![top], vec![bottom]], 2);
    install(&mut document, built);
    assert!(table_structure(table_of_document(&document)).is_empty());
    let mut session = Session::new(document);
    let paragraph = first_cell_paragraph(&session);
    let at = pos(&session, paragraph, 0);
    for command in [
        Command::InsertTableColumn { at },
        Command::DeleteTableColumn { at },
        Command::InsertTableRow { at },
        Command::DeleteTableRow { at },
    ] {
        let version = session.version();
        let error = exec(&mut session, vec![command]).unwrap_err();
        assert!(
            matches!(
                error,
                TxError::Command {
                    error: CommandError::MergedStructure(_),
                    ..
                }
            ),
            "{error:?}"
        );
        assert_eq!(session.version(), version);
    }
}

fn structure_error(error: &TxError) -> Option<TableStructure> {
    match error {
        TxError::Command {
            error: CommandError::TableStructure(_, reason),
            ..
        } => Some(*reason),
        _ => None,
    }
}

fn cell_text(session: &Session, row: usize, cell: usize, block: usize) -> String {
    table_of(session).rows[row].cells[cell]
        .blocks
        .get(block)
        .unwrap()
        .as_paragraph()
        .unwrap()
        .plain_text(|_| String::new())
}

#[test]
fn merging_cells_is_one_undoable_transaction() {
    let mut document = blank(&[""]);
    let top_left = cell(&mut document, "甲", 1, VerticalMerge::None);
    let top_right = cell(&mut document, "乙", 1, VerticalMerge::None);
    let bottom_left = cell(&mut document, "丙", 1, VerticalMerge::None);
    let bottom_right = cell(&mut document, "丁", 1, VerticalMerge::None);
    let built = table(
        &mut document,
        vec![vec![top_left, top_right], vec![bottom_left, bottom_right]],
        2,
    );
    install(&mut document, built);
    let mut session = Session::new(document);
    let paragraph = first_cell_paragraph(&session);
    let at = pos(&session, paragraph, 0);
    exec(
        &mut session,
        vec![Command::MergeTableCells {
            at,
            direction: TableMerge::Right,
        }],
    )
    .unwrap();
    assert_eq!(table_of(&session).rows[0].cells.len(), 1);
    assert_eq!(table_of(&session).rows[0].cells[0].grid_span, 2);
    assert_eq!(
        table_of(&session).rows[0].cells[0].width,
        Some(Measure::Twips(4000))
    );
    assert_eq!(cell_text(&session, 0, 0, 0), "甲");
    assert_eq!(cell_text(&session, 0, 0, 1), "乙");
    assert!(table_structure(table_of(&session)).is_empty());
    session.undo().unwrap();
    assert_eq!(table_of(&session).rows[0].cells.len(), 2);
    assert_eq!(cell_text(&session, 0, 1, 0), "乙");
    session.redo().unwrap();
    assert_eq!(table_of(&session).rows[0].cells[0].grid_span, 2);

    let paragraph = first_cell_paragraph(&session);
    let at = pos(&session, paragraph, 0);
    let version = session.version();
    let error = exec(
        &mut session,
        vec![Command::MergeTableCells {
            at,
            direction: TableMerge::Right,
        }],
    )
    .unwrap_err();
    assert_eq!(structure_error(&error), Some(TableStructure::NoNeighbour));
    assert_eq!(session.version(), version);

    session.undo().unwrap();
    let paragraph = first_cell_paragraph(&session);
    let at = pos(&session, paragraph, 0);
    exec(
        &mut session,
        vec![Command::MergeTableCells {
            at,
            direction: TableMerge::Down,
        }],
    )
    .unwrap();
    assert_eq!(
        table_of(&session).rows[0].cells[0].vertical_merge,
        VerticalMerge::Restart
    );
    assert_eq!(
        table_of(&session).rows[1].cells[0].vertical_merge,
        VerticalMerge::Continue
    );
    assert_eq!(cell_text(&session, 0, 0, 0), "甲");
    assert_eq!(cell_text(&session, 0, 0, 1), "丙");
    assert_eq!(cell_text(&session, 1, 0, 0), "");
    assert!(table_structure(table_of(&session)).is_empty());
    session.undo().unwrap();
    assert_eq!(
        table_of(&session).rows[0].cells[0].vertical_merge,
        VerticalMerge::None
    );
    assert_eq!(cell_text(&session, 1, 0, 0), "丙");
    session.redo().unwrap();
    let paragraph = first_cell_paragraph(&session);
    let at = pos(&session, paragraph, 0);
    let version = session.version();
    let error = exec(&mut session, vec![Command::InsertTableRow { at }]).unwrap_err();
    assert!(matches!(
        error,
        TxError::Command {
            error: CommandError::MergedStructure(_),
            ..
        }
    ));
    assert_eq!(session.version(), version);

    let continue_paragraph = table_of(&session).rows[1].cells[0]
        .blocks
        .get(0)
        .unwrap()
        .id();
    let at = pos(&session, continue_paragraph, 0);
    let error = exec(
        &mut session,
        vec![Command::MergeTableCells {
            at,
            direction: TableMerge::Down,
        }],
    )
    .unwrap_err();
    assert_eq!(structure_error(&error), Some(TableStructure::ContinueCell));
}

#[test]
fn splitting_a_cell_is_one_undoable_transaction() {
    let mut document = blank(&[""]);
    let left = cell(&mut document, "甲", 1, VerticalMerge::None);
    let right = cell(&mut document, "乙", 1, VerticalMerge::None);
    let built = table(&mut document, vec![vec![left, right]], 2);
    install(&mut document, built);
    let mut session = Session::new(document);
    let paragraph = first_cell_paragraph(&session);
    let at = pos(&session, paragraph, 0);
    exec(
        &mut session,
        vec![Command::MergeTableCells {
            at,
            direction: TableMerge::Right,
        }],
    )
    .unwrap();
    let paragraph = first_cell_paragraph(&session);
    let at = pos(&session, paragraph, 0);
    exec(&mut session, vec![Command::SplitTableCell { at }]).unwrap();
    assert_eq!(table_of(&session).rows[0].cells.len(), 2);
    assert_eq!(table_of(&session).rows[0].cells[0].grid_span, 1);
    assert_eq!(cell_text(&session, 0, 0, 0), "甲");
    assert_eq!(cell_text(&session, 0, 0, 1), "乙");
    assert_eq!(cell_text(&session, 0, 1, 0), "");
    assert!(table_structure(table_of(&session)).is_empty());
    session.undo().unwrap();
    assert_eq!(table_of(&session).rows[0].cells.len(), 1);
    assert_eq!(table_of(&session).rows[0].cells[0].grid_span, 2);
    session.redo().unwrap();
    assert_eq!(table_of(&session).rows[0].cells.len(), 2);

    let paragraph = first_cell_paragraph(&session);
    let at = pos(&session, paragraph, 0);
    let version = session.version();
    let error = exec(&mut session, vec![Command::SplitTableCell { at }]).unwrap_err();
    assert_eq!(structure_error(&error), Some(TableStructure::NotMerged));
    assert_eq!(session.version(), version);

    let mut document = blank(&[""]);
    let top = cell(&mut document, "合", 2, VerticalMerge::Restart);
    let bottom = cell(&mut document, "续", 2, VerticalMerge::Continue);
    let built = table(&mut document, vec![vec![top], vec![bottom]], 2);
    install(&mut document, built);
    let mut session = Session::new(document);
    let paragraph = first_cell_paragraph(&session);
    let at = pos(&session, paragraph, 0);
    exec(&mut session, vec![Command::SplitTableCell { at }]).unwrap();
    assert_eq!(
        table_of(&session).rows[0].cells[0].vertical_merge,
        VerticalMerge::None
    );
    assert_eq!(
        table_of(&session).rows[1].cells[0].vertical_merge,
        VerticalMerge::None
    );
    assert_eq!(table_of(&session).rows[0].cells[0].grid_span, 2);
    assert_eq!(cell_text(&session, 0, 0, 0), "合");
    assert_eq!(cell_text(&session, 1, 0, 0), "续");
    session.undo().unwrap();
    assert_eq!(
        table_of(&session).rows[0].cells[0].vertical_merge,
        VerticalMerge::Restart
    );
    session.redo().unwrap();
    let paragraph = first_cell_paragraph(&session);
    let at = pos(&session, paragraph, 0);
    exec(&mut session, vec![Command::SplitTableCell { at }]).unwrap();
    assert_eq!(table_of(&session).rows[0].cells.len(), 2);
    assert!(table_structure(table_of(&session)).is_empty());
}

fn table_of_document(document: &Document) -> &Table {
    match document.body().blocks.get(0) {
        Some(Block::Table(table)) => table,
        other => panic!("expected a table, got {other:?}"),
    }
}

#[test]
fn cell_selection_covers_one_cell_and_cross_cell_delete_is_refused() {
    let mut document = blank(&["正文"]);
    let left_cell = cell(&mut document, "甲乙", 1, VerticalMerge::None);
    let right_cell = cell(&mut document, "丙", 1, VerticalMerge::None);
    let built = table(&mut document, vec![vec![left_cell, right_cell]], 2);
    install(&mut document, built);
    let session = Session::new(document);
    let left = table_of(&session).rows[0].cells[0]
        .blocks
        .get(0)
        .unwrap()
        .id();
    let right = table_of(&session).rows[0].cells[1]
        .blocks
        .get(0)
        .unwrap()
        .id();
    let address = cell_address(session.document(), &pos(&session, left, 1)).unwrap();
    assert_eq!(
        address,
        CellAddress {
            story: session.document().body_id(),
            table: table_of(&session).id,
            row: 0,
            cell: 0,
        }
    );
    let selection = cell_selection(session.document(), address).unwrap();
    assert_eq!(selection.anchor.paragraph, left);
    assert_eq!(selection.anchor.offset, ScalarOffset(0));
    assert_eq!(selection.head.paragraph, left);
    assert_eq!(selection.head.offset, ScalarOffset(2));
    let caret = cell_caret(session.document(), address).unwrap();
    assert_eq!(caret, Selection::caret(pos(&session, left, 0)));

    let start = pos(&session, left, 1);
    let end = pos(&session, right, 1);
    let mut session = session;
    let error = exec(&mut session, vec![Command::DeleteRange { start, end }]).unwrap_err();
    assert!(matches!(
        error,
        TxError::Command {
            error: CommandError::CrossContainer,
            ..
        }
    ));
    assert_eq!(
        table_of(&session).rows[0].cells[0]
            .blocks
            .get(0)
            .unwrap()
            .as_paragraph()
            .unwrap()
            .plain_text(|_| String::new()),
        "甲乙"
    );
}

fn typed_cell(session: &Session) -> &Paragraph {
    table_of(session).rows[0].cells[0]
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap()
}

#[test]
fn a_hyperlink_inside_a_cell_is_set_edited_and_cleared_with_undo() {
    let mut document = blank(&["正文"]);
    let only_cell = cell(&mut document, "甲乙", 1, VerticalMerge::None);
    let built = table(&mut document, vec![vec![only_cell]], 1);
    install(&mut document, built);
    let mut session = Session::new(document);
    let paragraph = table_of(&session).rows[0].cells[0]
        .blocks
        .get(0)
        .unwrap()
        .id();

    let link = joyword_model::Hyperlink::for_edit(Some("https://example.invalid/cell"), None)
        .map_err(|reason| panic!("{reason}"))
        .unwrap();
    let start = pos(&session, paragraph, 0);
    let end = pos(&session, paragraph, 2);
    exec(
        &mut session,
        vec![Command::SetHyperlink {
            start,
            end,
            link: Some(link),
        }],
    )
    .unwrap();
    let stored = table_of(&session).rows[0].cells[0]
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap()
        .link_at(0)
        .expect("link inside the cell");
    assert_eq!(
        stored.external.as_deref(),
        Some("https://example.invalid/cell")
    );

    let edited = joyword_model::Hyperlink::for_edit(Some("mailto:c@example.invalid"), None)
        .map_err(|reason| panic!("{reason}"))
        .unwrap();
    let start = pos(&session, paragraph, 0);
    let end = pos(&session, paragraph, 2);
    exec(
        &mut session,
        vec![Command::SetHyperlink {
            start,
            end,
            link: Some(edited),
        }],
    )
    .unwrap();
    let start = pos(&session, paragraph, 0);
    let end = pos(&session, paragraph, 2);
    exec(
        &mut session,
        vec![Command::SetHyperlink {
            start,
            end,
            link: None,
        }],
    )
    .unwrap();
    assert!(
        table_of(&session).rows[0].cells[0]
            .blocks
            .get(0)
            .unwrap()
            .as_paragraph()
            .unwrap()
            .link_at(0)
            .is_none()
    );
    session.undo().unwrap();
    assert_eq!(
        table_of(&session).rows[0].cells[0]
            .blocks
            .get(0)
            .unwrap()
            .as_paragraph()
            .unwrap()
            .link_at(0)
            .unwrap()
            .external
            .as_deref(),
        Some("mailto:c@example.invalid")
    );
    session.undo().unwrap();
    session.undo().unwrap();
    assert!(
        table_of(&session).rows[0].cells[0]
            .blocks
            .get(0)
            .unwrap()
            .as_paragraph()
            .unwrap()
            .link_at(0)
            .is_none()
    );
    assert_eq!(
        table_of(&session).rows[0].cells[0]
            .blocks
            .get(0)
            .unwrap()
            .as_paragraph()
            .unwrap()
            .plain_text(|_| String::new()),
        "甲乙"
    );
}

#[test]
fn tracked_text_in_a_typed_cell_uses_the_author_and_accepts() {
    let mut document = blank(&[""]);
    let one = cell(&mut document, "甲", 1, VerticalMerge::None);
    let built = table(&mut document, vec![vec![one]], 1);
    install(&mut document, built);
    let mut session = Session::new(document);
    session.set_revision_author("审阅人");
    session.set_revision_date(Some(Arc::from("2026-01-01T00:00:00Z")));
    exec(
        &mut session,
        vec![Command::SetTrackRevisions { enabled: true }],
    )
    .unwrap();
    let paragraph = first_cell_paragraph(&session);
    let at = pos(&session, paragraph, 1);
    exec(
        &mut session,
        vec![Command::InsertText {
            at,
            text: "乙".into(),
        }],
    )
    .unwrap();
    assert_eq!(typed_cell(&session).plain_text(|_| String::new()), "甲乙");
    let mark = typed_cell(&session)
        .runs()
        .last()
        .unwrap()
        .revision
        .as_ref()
        .unwrap();
    assert_eq!(mark.author.as_ref(), "审阅人");
    assert_eq!(mark.kind, RevisionKind::Insert);
    let id = mark.id;
    let caret = Selection::caret(pos(&session, paragraph, 2));
    assert_eq!(current_revision(session.document(), &caret), Some(id));
    exec(&mut session, vec![Command::AcceptRevision { id }]).unwrap();
    assert!(
        typed_cell(&session)
            .runs()
            .iter()
            .all(|run| run.revision.is_none())
    );
    assert_eq!(typed_cell(&session).plain_text(|_| String::new()), "甲乙");
    session.undo().unwrap();
    exec(&mut session, vec![Command::RejectRevision { id }]).unwrap();
    assert_eq!(typed_cell(&session).plain_text(|_| String::new()), "甲");

    let paragraph = first_cell_paragraph(&session);
    let start = pos(&session, paragraph, 0);
    let end = pos(&session, paragraph, 1);
    exec(&mut session, vec![Command::DeleteRange { start, end }]).unwrap();
    let deleted = typed_cell(&session).runs()[0]
        .revision
        .as_ref()
        .unwrap()
        .clone();
    assert_eq!(deleted.kind, RevisionKind::Delete);
    assert_eq!(deleted.author.as_ref(), "审阅人");
    assert_eq!(typed_cell(&session).plain_text(|_| String::new()), "甲");
    exec(
        &mut session,
        vec![Command::AcceptRevision { id: deleted.id }],
    )
    .unwrap();
    assert_eq!(typed_cell(&session).plain_text(|_| String::new()), "");
}

#[test]
fn setting_table_properties_is_one_undoable_transaction() {
    let mut document = blank(&[""]);
    let left = cell(&mut document, "甲", 1, VerticalMerge::None);
    let right = cell(&mut document, "乙", 1, VerticalMerge::None);
    let built = table(&mut document, vec![vec![left, right]], 2);
    install(&mut document, built);
    let mut session = Session::new(document);
    let at = pos(&session, first_cell_paragraph(&session), 0);
    let border = BorderEdge {
        kind: BorderKind::Dashed,
        size_eighths: 8,
        color: Color::Rgb(0, 0, 255),
    };
    let change = TablePropertyChange {
        width: Some(TableWidth::Dxa(3_000)),
        alignment: Some(TableAlign::Center),
        cell_margin: Some(CellMargins {
            top: 40,
            left: 80,
            bottom: 40,
            right: 80,
        }),
        borders: Some(CellBorders {
            top: Some(border),
            left: Some(border),
            bottom: Some(border),
            right: Some(border),
        }),
    };
    exec(
        &mut session,
        vec![Command::SetTableProperties {
            at,
            change: change.clone(),
        }],
    )
    .unwrap();
    let written = table_of(&session);
    assert_eq!(written.width, Some(TableWidth::Dxa(3_000)));
    assert_eq!(written.alignment, Some(TableAlign::Center));
    assert_eq!(written.cell_margin.unwrap().left, 80);
    assert_eq!(written.rows[0].cells[1].borders.top, Some(border));
    assert_eq!(
        written.grid,
        vec![Measure::Twips(2000), Measure::Twips(2000)]
    );
    session.undo().unwrap();
    let restored = table_of(&session);
    assert!(restored.width.is_none());
    assert!(restored.alignment.is_none());
    assert!(restored.cell_margin.is_none());
    assert!(restored.rows[0].cells[0].borders.top.is_none());
    assert!(!session.can_undo());

    let error = exec(
        &mut session,
        vec![Command::SetTableProperties {
            at,
            change: TablePropertyChange {
                width: Some(TableWidth::Dxa(0)),
                alignment: None,
                cell_margin: None,
                borders: None,
            },
        }],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        TxError::Command {
            error: CommandError::TableProperties(_),
            ..
        }
    ));
    assert!(table_of(&session).width.is_none());
}

fn plain(session: &Session, paragraph: NodeId) -> String {
    session
        .document()
        .find_paragraph(session.document().body_id(), paragraph)
        .unwrap()
        .plain_text(|_| String::new())
}

#[test]
fn editing_outer_or_inner_cell_text_is_one_undoable_transaction() {
    let mut document = blank(&[""]);
    let outer_paragraph = paragraph(&mut document, "外");
    let inner_paragraph = paragraph(&mut document, "内");
    let outer_id = outer_paragraph.id();
    let inner_id = inner_paragraph.id();
    let inner_cell = document.ids().node();
    let left_id = document.ids().node();
    let inner = table(
        &mut document,
        vec![vec![Cell {
            id: inner_cell,
            grid_span: 1,
            vertical_merge: VerticalMerge::None,
            width: Some(Measure::Twips(2000)),
            borders: CellBorders::default(),
            shading: None,
            margin: None,
            vertical_align: None,
            blocks: BlockSeq::from_vec(vec![inner_paragraph.into()]),
            preserved: None,
        }]],
        1,
    );
    let left = Cell {
        id: left_id,
        grid_span: 1,
        vertical_merge: VerticalMerge::None,
        width: Some(Measure::Twips(2000)),
        borders: CellBorders::default(),
        shading: None,
        margin: None,
        vertical_align: None,
        blocks: BlockSeq::from_vec(vec![outer_paragraph.into(), inner.into()]),
        preserved: None,
    };
    let right = cell(&mut document, "右", 1, VerticalMerge::None);
    let outer = table(&mut document, vec![vec![left, right]], 2);
    install(&mut document, outer);
    let continued = cell(&mut document, "", 1, VerticalMerge::Continue);
    let orphan = table(&mut document, vec![vec![continued]], 1);
    let mut session = Session::new(document);

    let at = pos(&session, outer_id, 1);
    exec(
        &mut session,
        vec![Command::InsertText {
            at,
            text: "增".into(),
        }],
    )
    .unwrap();
    assert_eq!(plain(&session, outer_id), "外增");
    assert_eq!(plain(&session, inner_id), "内");
    assert_eq!(session.undo_depth(), 1);
    session.undo().unwrap();
    assert_eq!(plain(&session, outer_id), "外");
    assert!(!session.can_undo());

    let at = pos(&session, inner_id, 1);
    exec(
        &mut session,
        vec![Command::InsertText {
            at,
            text: "改".into(),
        }],
    )
    .unwrap();
    assert_eq!(plain(&session, inner_id), "内改");
    assert_eq!(plain(&session, outer_id), "外");
    assert_eq!(session.undo_depth(), 1);
    session.undo().unwrap();
    assert_eq!(plain(&session, inner_id), "内");
    assert!(!session.can_undo());

    assert!(
        table_structure(&orphan)
            .iter()
            .any(|issue| matches!(issue, Issue::OrphanVerticalMerge { .. }))
    );

    let at = pos(&session, outer_id, 0);
    let error = exec(
        &mut session,
        vec![Command::MergeTableCells {
            at,
            direction: TableMerge::Right,
        }],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        TxError::Command {
            error: CommandError::TableStructure(_, TableStructure::UnsupportedCell),
            ..
        }
    ));
    assert_eq!(plain(&session, outer_id), "外");
    assert_eq!(plain(&session, inner_id), "内");
    assert_eq!(table_of(&session).rows[0].cells.len(), 2);
    assert!(table_structure(table_of(&session)).is_empty());
}
