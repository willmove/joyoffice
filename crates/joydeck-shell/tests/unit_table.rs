use joydeck_shell::text::{Run, Style};
extern crate joydeck_shell as joydeck;

use joydeck::table::*;
use joydeck::{
    edit::{self, Command, Session},
    scene::SceneCache,
};
use std::sync::atomic::AtomicBool;
fn session() -> Session {
    let mut s = Session::new();
    s.execute(s.id, Command::AddObject(0, object(4, 5).unwrap()))
        .unwrap();
    s
}
fn command(s: &mut Session, a: Action) {
    let id = s.deck.slides[0].objects[0].id.clone();
    s.execute(s.id, Command::Table(0, id, a)).unwrap();
}
#[test]
fn tsv_transaction_budget_and_undo() {
    let mut s = session();
    command(
        &mut s,
        Action::Paste(0, 0, "名称\tQ1\tQ2\n北区\t42\t56".into()),
    );
    let before = s.deck.clone();
    for (r, c, value) in [(0, 0, "a\tb\nc"), (3, 4, "a\tb"), (usize::MAX, 0, "a\nb")] {
        let id = s.deck.slides[0].objects[0].id.clone();
        assert!(
            s.execute(
                s.id,
                Command::Table(0, id, Action::Paste(r, c, value.into()))
            )
            .is_err()
        );
        assert_eq!(*s.deck, *before);
    }
    s.undo();
    assert!(
        s.deck.slides[0].objects[0].table.as_ref().unwrap().cells[0]
            .text
            .is_empty()
    );
    s.redo();
    assert_eq!(*s.deck, *before);
    assert!(Table::new(129, 2).is_err());
    assert!(Table::new(128, 64).is_err());
}
#[test]
fn merge_structure_and_illegal_intersection_are_atomic() {
    let mut t = Table::new(4, 5).unwrap();
    t.apply(Action::Paste(0, 0, "A\tB\nC\tD".into())).unwrap();
    t.apply(Action::Merge(Range {
        top: 0,
        left: 0,
        bottom: 1,
        right: 1,
    }))
    .unwrap();
    assert_eq!(t.owner(1, 1).unwrap(), (0, 0));
    assert_eq!(t.cells[0].text, "A\nB\nC\nD");
    let before = t.clone();
    assert!(
        t.apply(Action::Merge(Range {
            top: 1,
            left: 1,
            bottom: 2,
            right: 2
        }))
        .is_err()
    );
    assert_eq!(before, t);
    t.apply(Action::InsertRow(1)).unwrap();
    t.apply(Action::InsertColumn(1)).unwrap();
    assert_eq!(t.cells[0].span, [3, 3]);
    t.apply(Action::DeleteRow(0)).unwrap();
    t.apply(Action::DeleteColumn(0)).unwrap();
    assert_eq!(t.cells[0].span, [2, 2]);
    assert!(t.cells[0].text.starts_with('A'));
    t.apply(Action::Split(1, 1)).unwrap();
    assert!(t.cells.iter().all(|c| c.span == [1, 1]));
    t.validate().unwrap();
}
#[test]
fn seeded_structure_operations_preserve_topology() {
    let mut t = Table::new(5, 6).unwrap();
    let mut seed = 91u64;
    for _ in 0..500 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let r = (seed >> 12) as usize % t.rows.len();
        let c = (seed >> 28) as usize % t.columns.len();
        let a = match seed % 8 {
            0 => Action::InsertRow(r),
            1 => Action::InsertColumn(c),
            2 => Action::DeleteRow(r),
            3 => Action::DeleteColumn(c),
            4 => Action::Merge(Range {
                top: r,
                left: c,
                bottom: (r + 1).min(t.rows.len() - 1),
                right: (c + 1).min(t.columns.len() - 1),
            }),
            5 => Action::Split(r, c),
            6 => Action::DistributeRows,
            _ => Action::DistributeColumns,
        };
        let before = t.clone();
        if t.apply(a).is_err() {
            assert_eq!(before, t);
        }
        t.validate().unwrap();
    }
}
#[test]
fn inherited_style_rich_merge_and_native_roundtrip() {
    let mut s = session();
    let text = "中文😀 e\u{301}";
    let style = Style {
        family: "sans-serif".into(),
        size: 20.,
        color: "#e05080".into(),
        bold: true,
        italic: true,
    };
    command(
        &mut s,
        Action::SetCell(
            0,
            0,
            text.into(),
            vec![Run {
                start: 0,
                end: text.len(),
                style: style.clone(),
            }],
        ),
    );
    command(&mut s, Action::SetCell(0, 1, "业务".into(), vec![]));
    command(
        &mut s,
        Action::Merge(Range {
            top: 0,
            left: 0,
            bottom: 0,
            right: 1,
        }),
    );
    let t = s.deck.slides[0].objects[0].table.as_ref().unwrap();
    assert_eq!(t.cells[0].runs[0].style, style);
    joydeck::text::validate(&t.cells[0].text, &t.cells[0].runs).unwrap();
    assert!(t.cells[0].runs.len() > 1);
    let p = std::env::temp_dir().join(format!("joydeck-m8-native-{}.joydeck", std::process::id()));
    edit::save(&p, &s.deck).unwrap();
    assert_eq!(edit::open_editable(&p).unwrap(), *s.deck);
    std::fs::remove_file(p).unwrap();
}
#[test]
fn public_pptx_has_real_merged_cells_without_private_snapshot() {
    let mut s = session();
    command(
        &mut s,
        Action::Paste(0, 0, "项目\t收入\t增长\n北区\t42\t20%".into()),
    );
    command(
        &mut s,
        Action::Merge(Range {
            top: 2,
            left: 0,
            bottom: 3,
            right: 1,
        }),
    );
    command(
        &mut s,
        Action::CellStyle(
            Range::cell(0, 0),
            CellStyle {
                border: Some("#d02080".into()),
                border_width: Some(2.),
                fill: Some("#eef0ff".into()),
                align: Some("center".into()),
                ..CellStyle::default()
            },
        ),
    );
    let mut parts = joydeck::pptx_write::generate(&s.deck).unwrap();
    parts.remove("customXml/joydeck.xml");
    let xml = String::from_utf8(parts["ppt/slides/slide1.xml"].clone()).unwrap();
    assert!(xml.contains("a:tbl"));
    assert!(xml.contains("rowSpan=\"2\""));
    assert!(xml.contains("hMerge=\"1\""));
    assert!(xml.contains("vMerge=\"1\""));
    let d = joydeck::pptx::import(&parts, &AtomicBool::new(false)).unwrap();
    let table = d.slides[0].objects[0].table.as_ref().unwrap();
    assert_eq!(table.owner(3, 1).unwrap(), (2, 0));
    assert_eq!(table.cells[0].text, "项目");
    assert_eq!(table.cells[0].style.border.as_deref(), Some("#d02080"));
    assert_eq!(table.cells[0].style.align.as_deref(), Some("center"));
    assert!(table.cells[0].runs[0].style.bold);
}
#[test]
fn multiline_empty_paragraphs_and_rich_public_read() {
    let mut s = session();
    let text = "\n中文\n\n😀\n";
    command(
        &mut s,
        Action::SetCell(
            1,
            0,
            text.into(),
            vec![Run {
                start: 0,
                end: text.len(),
                style: Style::for_object(&edit::object("text")),
            }],
        ),
    );
    let mut parts = joydeck::pptx_write::generate(&s.deck).unwrap();
    parts.remove("customXml/joydeck.xml");
    let d = joydeck::pptx::import(&parts, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        d.slides[0].objects[0].table.as_ref().unwrap().cells[5].text,
        text
    );
}
#[test]
fn hit_rotation_resize_and_theme_override() {
    let mut s = session();
    let mut o = s.deck.slides[0].objects[0].clone();
    assert_eq!(hit(&o, o.x + 20., o.y + 20.), Some((0, 0)));
    o.rotation = 180.;
    assert_eq!(
        hit(&o, o.x + o.width - 20., o.y + o.height - 20.),
        Some((0, 0))
    );
    o.width *= 1.5;
    resize(&mut o);
    validate(&o).unwrap();
    let t = o.table.as_ref().unwrap();
    let before = resolve(&s.deck, 0, t, 0, 0);
    let mut d = joydeck::design::starter();
    d.themes
        .values_mut()
        .next()
        .unwrap()
        .colors
        .insert("accent".into(), "#c02460".into());
    s.execute(
        s.id,
        Command::Design(joydeck::design_commands::Action::SetDesign(d)),
    )
    .unwrap();
    assert_ne!(before.fill, resolve(&s.deck, 0, t, 0, 0).fill);
}
#[test]
fn scene_cpu_pixel_parity_and_cache() {
    let mut s = session();
    command(
        &mut s,
        Action::Paste(0, 0, "产品\t计划\t实际\n服务\t120\t136".into()),
    );
    command(
        &mut s,
        Action::Merge(Range {
            top: 2,
            left: 0,
            bottom: 3,
            right: 1,
        }),
    );
    let cancel = AtomicBool::new(false);
    let mut cache = SceneCache::with_budget(joydeck::scene::CACHE_BUDGET);
    let scene = cache.build(&s.deck, 0, s.id, s.revision, &cancel).unwrap();
    let cpu = joydeck::preview::render(&s.deck, 0, 960, &cancel).unwrap();
    let rendered = scene.raster(960, &cancel).unwrap();
    assert_eq!(cpu.bgra, rendered.bgra);
}
#[test]
fn hostile_spans_and_global_cells_are_rejected() {
    let mut t = Table::new(2, 2).unwrap();
    t.cells[3].span = [usize::MAX, 1];
    assert!(t.validate().is_err());
    let mut s = Session::new();
    for _ in 0..4 {
        s.execute(s.id, Command::AddObject(0, object(64, 64).unwrap()))
            .unwrap();
    }
    let before = s.deck.clone();
    assert!(
        s.execute(s.id, Command::AddObject(0, object(1, 1).unwrap()))
            .is_err()
    );
    assert!(s.deck == before);
}
#[test]
fn cross_document_table_copy_freezes_source_and_page_sizes_scale() {
    let mut source = session();
    let mut d = joydeck::design::starter();
    d.themes
        .values_mut()
        .next()
        .unwrap()
        .colors
        .insert("accent".into(), "#c02460".into());
    source
        .execute(
            source.id,
            Command::Design(joydeck::design_commands::Action::SetDesign(d)),
        )
        .unwrap();
    let id = source.deck.slides[0].objects[0].id.clone();
    let transfer =
        joydeck::media::Transfer::capture(&source.deck, 0, &std::collections::BTreeSet::from([id]))
            .unwrap();
    let mut target = Session::new();
    target
        .execute(target.id, Command::Paste(0, transfer))
        .unwrap();
    let t = target.deck.slides[0].objects[0].table.as_ref().unwrap();
    assert_eq!(resolve(&target.deck, 0, t, 0, 0).fill, "#c02460");
    target
        .execute(target.id, Command::PageSize(480., 270.))
        .unwrap();
    let pages = joydeck::design_commands::PageTransfer::capture(&source.deck, &[0]).unwrap();
    target
        .execute(
            target.id,
            Command::Design(joydeck::design_commands::Action::PastePages(1, pages, None)),
        )
        .unwrap();
    let o = &target.deck.slides[1].objects[0];
    validate(o).unwrap();
    assert_eq!(o.width, 280.);
    assert_eq!(o.table.as_ref().unwrap().style.size, 9.);
}
#[test]
fn table_missing_font_diagnostic_preserves_the_authored_name() {
    let mut s = session();
    let mut style = s.deck.slides[0].objects[0]
        .table
        .as_ref()
        .unwrap()
        .style
        .clone();
    style.family = "JoyDeck-Missing-Table-Font-987654".into();
    command(&mut s, Action::Style(style));
    let mut deck = (*s.deck).clone();
    joydeck::preview::diagnose_fonts(&mut deck);
    assert!(
        deck.warnings
            .iter()
            .any(|w| w.code == "missing-font" && w.message.contains("Missing-Table"))
    );
    assert_eq!(
        deck.slides[0].objects[0]
            .table
            .as_ref()
            .unwrap()
            .style
            .family,
        "JoyDeck-Missing-Table-Font-987654"
    );
}
#[test]
fn merged_selection_expands_before_applying_cell_style() {
    let mut t = Table::new(4, 5).unwrap();
    t.apply(Action::Merge(Range {
        top: 1,
        left: 1,
        bottom: 2,
        right: 2,
    }))
    .unwrap();
    let range = t.selection_range((1, 1), (1, 1)).unwrap();
    assert_eq!(
        range,
        Range {
            top: 1,
            left: 1,
            bottom: 2,
            right: 2
        }
    );
    t.apply(Action::CellStyle(
        range,
        CellStyle {
            fill: Some("#ab34cd".into()),
            ..CellStyle::default()
        },
    ))
    .unwrap();
    assert_eq!(
        t.cells[t.index(1, 1).unwrap()].style.fill.as_deref(),
        Some("#ab34cd")
    );
    assert_eq!(t.next(2, 2, false), Some((1, 3)));
}
