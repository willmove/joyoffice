#![allow(clippy::unwrap_used, clippy::expect_used, reason = "Test assertions")]
use joydeck_engine::{CellBorders, CellFormat, CellRange, Command, TableAction};
use joydeck_model::{ShapeKind, text::Anchor};
use joydeck_shell::{Cancellation, Document, editor::Editor, platform, storage};
#[test]
fn table_controls_round_trip_both_formats_and_keep_untouched_text() {
    let root = platform::private_directory("r9-table-roundtrip").unwrap();
    let mut e = Editor::new(root.join("recovery")).unwrap();
    e.session
        .execute(Command::InsertTable {
            x: 20.,
            y: 200.,
            rows: 2,
            cols: 2,
            width: 300.,
            row_height: 40.,
        })
        .unwrap();
    let id = e.session.selection().shapes[0];
    e.session
        .execute(Command::Table {
            action: TableAction::Format {
                range: CellRange {
                    row: 0,
                    col: 0,
                    end_row: 1,
                    end_col: 1,
                },
                format: CellFormat {
                    fill_rgb: Some(0xdce6f1),
                    borders: Some(CellBorders {
                        sides: [true; 4],
                        rgb: 0x123456,
                        width: 1.5,
                        remove: false,
                    }),
                    margins: Some([5., 6., 7., 8.]),
                    anchor: Some(Anchor::Middle),
                    ..Default::default()
                },
            },
        })
        .unwrap();
    for name in ["formatted.joydeck", "formatted.pptx"] {
        let path = root.join(name);
        let saved = e.save_job(path.clone()).unwrap().run().unwrap();
        e.apply_saved(saved);
        let loaded = Document::open(&path, &Cancellation::default()).unwrap();
        let ShapeKind::Table(t) = &loaded.presentation.slides[0]
            .shapes
            .iter()
            .find(|s| matches!(s.kind, ShapeKind::Table(_)))
            .unwrap()
            .kind
        else {
            unreachable!()
        };
        assert_eq!(t.n_rows(), 2);
        assert_eq!(t.n_cols(), 2);
        assert_eq!(t.rows[1].cells[1].margins, Some([5., 6., 7., 8.]));
        assert_eq!(t.rows[1].cells[1].anchor, Some(Anchor::Middle));
        assert_eq!(
            t.rows[1].cells[1].borders[0].as_ref().unwrap().width,
            Some(1.5)
        );
        assert!(e.session.model().slides[0].shape(id).is_some());
    }
}
#[test]
fn recent_list_is_deduplicated_bounded_and_corruption_is_preserved() {
    let root = platform::private_directory("r9-recent").unwrap();
    let state = root.join("state");
    for n in 0..22 {
        let path = root.join(format!("文稿 {n}.pptx"));
        std::fs::write(&path, b"fixture").unwrap();
        storage::remember_file(&state, &path).unwrap();
    }
    let paths = storage::recent_files(&state).unwrap();
    assert_eq!(paths.len(), 20);
    storage::remember_file(&state, &paths[3]).unwrap();
    let after = storage::recent_files(&state).unwrap();
    assert_eq!(after[0], paths[3]);
    assert_eq!(after.len(), 20);
    let index = state.join("recent-files.v1");
    std::fs::write(&index, b"future format").unwrap();
    assert!(storage::remember_file(&state, &paths[0]).is_err());
    assert_eq!(std::fs::read(&index).unwrap(), b"future format");
}
#[test]
fn formatting_existing_pptx_cells_keeps_margins_borders_style_flags_and_dimensions() {
    let root = platform::private_directory("r9-existing-table").unwrap();
    let mut e = Editor::new(root.join("recovery")).unwrap();
    e.session
        .execute(Command::InsertTable {
            x: 30.,
            y: 200.,
            rows: 2,
            cols: 2,
            width: 300.,
            row_height: 40.,
        })
        .unwrap();
    let input = root.join("input.pptx");
    e.save_job(input.clone()).unwrap().run().unwrap();
    let mut e = Editor::open(&input, root.join("opened")).unwrap();
    let id = e.session.model().slides[0]
        .shapes
        .iter()
        .find(|s| matches!(s.kind, ShapeKind::Table(_)))
        .unwrap()
        .id;
    e.session.select(0, vec![id]).unwrap();
    e.session
        .execute(Command::Batch {
            commands: vec![
                Command::Table {
                    action: TableAction::Format {
                        range: CellRange {
                            row: 0,
                            col: 0,
                            end_row: 1,
                            end_col: 1,
                        },
                        format: CellFormat {
                            margins: Some([4., 5., 6., 7.]),
                            borders: Some(CellBorders {
                                sides: [true; 4],
                                rgb: 0x123456,
                                width: 1.5,
                                remove: false,
                            }),
                            anchor: Some(Anchor::Middle),
                            ..Default::default()
                        },
                    },
                },
                Command::Table {
                    action: TableAction::RowHeight {
                        row: 0,
                        height: 60.,
                    },
                },
                Command::Table {
                    action: TableAction::ColWidth {
                        col: 1,
                        width: 180.,
                    },
                },
                Command::Table {
                    action: TableAction::Style {
                        name: "medium2-accent2".into(),
                        first_row: false,
                        band_row: false,
                    },
                },
            ],
        })
        .unwrap();
    let output = root.join("formatted.pptx");
    e.save_job(output.clone()).unwrap().run().unwrap();
    let doc = Document::open(&output, &Cancellation::default()).unwrap();
    let ShapeKind::Table(t) = &doc.presentation.slides[0].shape(id).unwrap().kind else {
        unreachable!()
    };
    assert_eq!(t.rows[0].height, 60.);
    assert_eq!(t.cols[1], 180.);
    assert!(!t.first_row);
    assert!(!t.band_row);
    assert_eq!(t.rows[1].cells[1].margins, Some([4., 5., 6., 7.]));
    assert_eq!(t.rows[1].cells[1].anchor, Some(Anchor::Middle));
    assert_eq!(
        t.rows[1].cells[1].borders[2].as_ref().unwrap().width,
        Some(1.5)
    );
}
