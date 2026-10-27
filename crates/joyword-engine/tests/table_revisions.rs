//! T301: table property revisions (`w:tblPrChange` / `w:trPrChange` /
//! `w:tcPrChange`) accept, reject, batch resolution over nested tables, the
//! caret lookup, the locked-container refusal and undo. Each command is one
//! transaction, so one undo restores the whole prior state.

use std::sync::Arc;

use joyword_engine::{
    Command, CommandError, Selection, Session, Transaction, TxError, current_revision,
};
use joyword_model::{
    Bias, Block, BlockSeq, Cell, CellBorders, CellChangeProps, Color, Document, IdAllocator,
    Measure, NodeId, Paragraph, Position, PreviousTableProps, RowChangeProps, ScalarOffset, Sdt,
    SdtLock, SdtProps, Shading, Table, TableAlign, TableChangeProps, TablePropsRevision,
    VerticalAlign, VerticalMerge,
};
use joyword_model::{Row, StoryId};

const DATE: &str = "2020-01-02T03:04:05Z";

fn revision(id: u32, author: &str, previous: PreviousTableProps) -> Arc<TablePropsRevision> {
    Arc::new(TablePropsRevision {
        id,
        author: Arc::from(author),
        date: Some(Arc::from(DATE)),
        previous,
    })
}

fn paragraph(ids: &mut IdAllocator, text: &str) -> Block {
    let mut paragraph = Paragraph::new(ids.node());
    paragraph.push_text(text, None, Arc::default()).unwrap();
    paragraph.into()
}

fn cell(ids: &mut IdAllocator, blocks: Vec<Block>) -> Cell {
    Cell {
        id: ids.node(),
        grid_span: 1,
        vertical_merge: VerticalMerge::None,
        width: Some(Measure::Twips(2_000)),
        borders: CellBorders::default(),
        shading: None,
        margin: None,
        vertical_align: None,
        blocks: BlockSeq::from_vec(blocks),
        preserved: None,
        props_revision: None,
    }
}

fn row(ids: &mut IdAllocator, cells: Vec<Cell>) -> Row {
    Row {
        id: ids.node(),
        grid_before: 0,
        grid_after: 0,
        cells,
        header: false,
        cant_split: false,
        height: None,
        preserved: None,
        props_revision: None,
    }
}

fn table(ids: &mut IdAllocator, rows: Vec<Row>) -> Table {
    Table {
        id: ids.node(),
        grid: vec![Measure::Twips(2_000); rows[0].cells.len()],
        rows,
        style: None,
        width: None,
        layout: None,
        alignment: None,
        indent: None,
        look: None,
        cell_margin: None,
        preserved: None,
        props_revision: None,
        source: None,
    }
}

fn yellow() -> Shading {
    Shading {
        fill: Color::Rgb(0xFF, 0xFF, 0x00),
    }
}

/// A body paragraph, then a table whose table (id 61, previously centred,
/// now right-aligned), row (id 62, previously a header, now `cantSplit`) and
/// first cell (id 63, previously bottom-aligned, now yellow) carry pending
/// property changes. The second cell holds a nested table whose own row
/// carries id 64 (previously `cantSplit`).
struct Fixture {
    session: Session,
    story: StoryId,
    table: NodeId,
    first_cell_paragraph: NodeId,
    second_cell_paragraph: NodeId,
    nested_paragraph: NodeId,
    body_paragraph: NodeId,
}

fn fixture() -> Fixture {
    let mut document = Document::new_blank();
    let story = document.body_id();
    let ids = document.ids();

    let nested_text = paragraph(ids, "nested");
    let nested_paragraph = nested_text.id();
    let nested_cell = cell(ids, vec![nested_text]);
    let nested_row = row(ids, vec![nested_cell]);
    let mut nested = table(ids, vec![nested_row]);
    nested.rows[0].props_revision = Some(revision(
        64,
        "D",
        PreviousTableProps::Row(RowChangeProps {
            cant_split: true,
            ..RowChangeProps::default()
        }),
    ));

    let first_text = paragraph(ids, "one");
    let first_cell_paragraph = first_text.id();
    let mut first = cell(ids, vec![first_text]);
    first.shading = Some(yellow());
    first.props_revision = Some(revision(
        63,
        "C",
        PreviousTableProps::Cell(CellChangeProps {
            grid_span: 1,
            width: Some(Measure::Twips(2_000)),
            vertical_align: Some(VerticalAlign::Bottom),
            ..CellChangeProps::default()
        }),
    ));
    let second_text = paragraph(ids, "two");
    let second_cell_paragraph = second_text.id();
    let second = cell(ids, vec![second_text, nested.into()]);

    let mut outer_row = row(ids, vec![first, second]);
    outer_row.cant_split = true;
    outer_row.props_revision = Some(revision(
        62,
        "B",
        PreviousTableProps::Row(RowChangeProps {
            header: true,
            ..RowChangeProps::default()
        }),
    ));
    let mut outer = table(ids, vec![outer_row]);
    outer.alignment = Some(TableAlign::Right);
    outer.props_revision = Some(revision(
        61,
        "A",
        PreviousTableProps::Table(TableChangeProps {
            alignment: Some(TableAlign::Center),
            ..TableChangeProps::default()
        }),
    ));
    let table_id = outer.id;
    let body_text = paragraph(ids, "body");
    let body_paragraph = body_text.id();
    for id in [61, 62, 63, 64] {
        document.note_revision_id(id);
    }
    document.story_mut(story).unwrap().blocks = BlockSeq::from_vec(vec![body_text, outer.into()]);
    Fixture {
        session: Session::new(document),
        story,
        table: table_id,
        first_cell_paragraph,
        second_cell_paragraph,
        nested_paragraph,
        body_paragraph,
    }
}

fn table_of(session: &Session) -> &Table {
    match session.document().body().blocks.get(1) {
        Some(Block::Table(table)) => table,
        other => panic!("expected the table, got {other:?}"),
    }
}

fn nested_of(table: &Table) -> &Table {
    match table.rows[0].cells[1].blocks.get(1) {
        Some(Block::Table(nested)) => nested,
        other => panic!("expected the nested table, got {other:?}"),
    }
}

fn pending_ids(table: &Table) -> Vec<u32> {
    let mut ids = Vec::new();
    if let Some(revision) = &table.props_revision {
        ids.push(revision.id);
    }
    for row in &table.rows {
        if let Some(revision) = &row.props_revision {
            ids.push(revision.id);
        }
        for cell in &row.cells {
            if let Some(revision) = &cell.props_revision {
                ids.push(revision.id);
            }
            for block in cell.blocks.iter() {
                if let Block::Table(nested) = block {
                    ids.extend(pending_ids(nested));
                }
            }
        }
    }
    ids
}

fn exec(session: &mut Session, commands: Vec<Command>) -> Result<(), TxError> {
    session
        .apply(Transaction::new(session.version(), commands))
        .map(|_| ())
}

fn command_error(error: TxError) -> CommandError {
    match error {
        TxError::Command { error, .. } => error,
        other => panic!("expected a command error, got {other:?}"),
    }
}

fn caret(story: StoryId, paragraph: NodeId, offset: usize) -> Selection {
    let position = Position {
        story,
        paragraph,
        offset: ScalarOffset(offset),
        bias: Bias::Before,
    };
    Selection {
        anchor: position,
        head: position,
    }
}

#[test]
fn accepting_one_table_property_revision_is_one_undo_step() {
    let mut fx = fixture();
    exec(&mut fx.session, vec![Command::AcceptRevision { id: 61 }]).unwrap();
    let table = table_of(&fx.session);
    // Accept keeps the current (post-change) value and drops the record; the
    // row, cell and nested records are untouched.
    assert_eq!(table.alignment, Some(TableAlign::Right));
    assert_eq!(pending_ids(table), vec![62, 63, 64]);

    fx.session.undo().unwrap();
    let table = table_of(&fx.session);
    assert_eq!(pending_ids(table), vec![61, 62, 63, 64]);
    let change = table.props_revision.as_ref().unwrap();
    assert_eq!(
        (change.id, &*change.author, change.date.as_deref()),
        (61, "A", Some(DATE))
    );
    assert!(matches!(
        &change.previous,
        PreviousTableProps::Table(previous) if previous.alignment == Some(TableAlign::Center)
    ));

    // An unknown id is refused and changes nothing.
    let error = exec(&mut fx.session, vec![Command::AcceptRevision { id: 999 }]).unwrap_err();
    assert!(matches!(
        command_error(error),
        CommandError::UnknownRevision(999)
    ));
    assert_eq!(pending_ids(table_of(&fx.session)), vec![61, 62, 63, 64]);
}

#[test]
fn rejecting_row_and_cell_property_revisions_restores_the_previous_values() {
    let mut fx = fixture();
    exec(&mut fx.session, vec![Command::RejectRevision { id: 62 }]).unwrap();
    let row = &table_of(&fx.session).rows[0];
    assert!(
        row.header && !row.cant_split,
        "the previous row properties return"
    );
    assert_eq!(pending_ids(table_of(&fx.session)), vec![61, 63, 64]);

    exec(&mut fx.session, vec![Command::RejectRevision { id: 63 }]).unwrap();
    let cell = &table_of(&fx.session).rows[0].cells[0];
    assert_eq!(cell.vertical_align, Some(VerticalAlign::Bottom));
    assert_eq!(cell.shading, None);
    assert_eq!(cell.width, Some(Measure::Twips(2_000)));
    assert_eq!(pending_ids(table_of(&fx.session)), vec![61, 64]);

    // Two transactions, two undo steps, each restoring exactly one record.
    fx.session.undo().unwrap();
    let cell = &table_of(&fx.session).rows[0].cells[0];
    assert_eq!(cell.shading, Some(yellow()));
    assert_eq!(cell.vertical_align, None);
    assert_eq!(pending_ids(table_of(&fx.session)), vec![61, 63, 64]);
    fx.session.undo().unwrap();
    let row = &table_of(&fx.session).rows[0];
    assert!(!row.header && row.cant_split);
    assert_eq!(pending_ids(table_of(&fx.session)), vec![61, 62, 63, 64]);
}

#[test]
fn batch_accept_and_reject_sweep_nested_tables_in_one_undo_step() {
    let mut fx = fixture();
    exec(&mut fx.session, vec![Command::RejectAllRevisions]).unwrap();
    let table = table_of(&fx.session);
    assert!(pending_ids(table).is_empty());
    assert_eq!(table.alignment, Some(TableAlign::Center));
    assert!(table.rows[0].header);
    assert_eq!(
        table.rows[0].cells[0].vertical_align,
        Some(VerticalAlign::Bottom)
    );
    assert!(
        nested_of(table).rows[0].cant_split,
        "nested row rejected too"
    );

    fx.session.undo().unwrap();
    let table = table_of(&fx.session);
    assert_eq!(pending_ids(table), vec![61, 62, 63, 64]);
    assert_eq!(table.alignment, Some(TableAlign::Right));
    assert!(!nested_of(table).rows[0].cant_split);

    exec(&mut fx.session, vec![Command::AcceptAllRevisions]).unwrap();
    let table = table_of(&fx.session);
    assert!(pending_ids(table).is_empty());
    assert_eq!(table.alignment, Some(TableAlign::Right));
    assert!(table.rows[0].cant_split && !table.rows[0].header);
    assert_eq!(table.rows[0].cells[0].shading, Some(yellow()));
    assert!(!nested_of(table).rows[0].cant_split);
}

#[test]
fn current_revision_finds_the_innermost_table_revision_at_the_caret() {
    let fx = fixture();
    let document = fx.session.document();
    // Cell change before the row change before the table change.
    assert_eq!(
        current_revision(document, &caret(fx.story, fx.first_cell_paragraph, 1)),
        Some(63)
    );
    // The second cell has no change of its own: its row's change applies.
    assert_eq!(
        current_revision(document, &caret(fx.story, fx.second_cell_paragraph, 0)),
        Some(62)
    );
    // Inside the nested table its own row change wins over the outer levels.
    assert_eq!(
        current_revision(document, &caret(fx.story, fx.nested_paragraph, 2)),
        Some(64)
    );
    // Outside the table nothing is current.
    assert_eq!(
        current_revision(document, &caret(fx.story, fx.body_paragraph, 0)),
        None
    );

    // Once the cell and row changes are gone the table change is what remains
    // under the first cell's caret.
    let mut fx = fx;
    exec(
        &mut fx.session,
        vec![
            Command::AcceptRevision { id: 63 },
            Command::AcceptRevision { id: 62 },
        ],
    )
    .unwrap();
    assert_eq!(
        current_revision(
            fx.session.document(),
            &caret(fx.story, fx.first_cell_paragraph, 1)
        ),
        Some(61)
    );
    assert_eq!(fx.table, table_of(&fx.session).id);
}

#[test]
fn a_table_inside_a_locked_content_control_refuses_resolution() {
    let mut document = Document::new_blank();
    let story = document.body_id();
    let ids = document.ids();
    let text = paragraph(ids, "locked");
    let inner_cell = cell(ids, vec![text]);
    let inner_row = row(ids, vec![inner_cell]);
    let mut inner = table(ids, vec![inner_row]);
    inner.alignment = Some(TableAlign::Right);
    inner.props_revision = Some(revision(
        91,
        "A",
        PreviousTableProps::Table(TableChangeProps::default()),
    ));
    let sdt = Sdt {
        id: ids.node(),
        props: SdtProps {
            lock: SdtLock::ContentLocked,
            ..SdtProps::default()
        },
        blocks: BlockSeq::from_vec(vec![inner.into()]),
        source: None,
    };
    document.note_revision_id(91);
    document.story_mut(story).unwrap().blocks = BlockSeq::from_vec(vec![sdt.into()]);
    let mut session = Session::new(document);
    let before = session.version();

    for command in [
        Command::AcceptRevision { id: 91 },
        Command::RejectRevision { id: 91 },
        Command::AcceptAllRevisions,
    ] {
        let error = exec(&mut session, vec![command]).unwrap_err();
        assert!(matches!(command_error(error), CommandError::Locked(_)));
    }
    assert_eq!(
        session.version(),
        before,
        "a refusal leaves the document unchanged"
    );
    let Some(Block::Sdt(sdt)) = session.document().body().blocks.get(0) else {
        panic!("sdt");
    };
    let Some(Block::Table(table)) = sdt.blocks.get(0) else {
        panic!("table");
    };
    assert_eq!(pending_ids(table), vec![91]);
    assert_eq!(table.alignment, Some(TableAlign::Right));
}
