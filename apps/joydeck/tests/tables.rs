//! Original JoyDeck table, modified and saved by PowerPoint 16.114.1 on macOS.
use joydeck::{edit, export, interop, pptx_write, table};
use std::sync::atomic::AtomicBool;
#[test]
fn actual_powerpoint_edit_uses_public_cells_and_preserves_merges() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m8-powerpoint-edited.pptx");
    let deck = interop::import(&path).unwrap();
    let object = deck.slides[0]
        .objects
        .iter()
        .find(|o| o.table.is_some())
        .unwrap();
    let t = object.table.as_ref().unwrap();
    assert_eq!((t.rows.len(), t.columns.len()), (4, 5));
    assert_eq!(t.cells[t.index(1, 1).unwrap()].text, "131");
    assert_eq!(t.cells[0].text, "业务板块");
    assert_eq!(t.owner(3, 4).unwrap(), (2, 3));
    assert_eq!(t.cells[t.index(2, 3).unwrap()].span, [2, 2]);
    assert!(!t.cells[0].runs.is_empty());
    table::validate(object).unwrap();
    let native = std::env::temp_dir().join(format!(
        "joydeck-office-table-{}.joydeck",
        std::process::id()
    ));
    edit::save(&native, &deck).unwrap();
    let reopened = edit::open_editable(&native).unwrap();
    assert!(
        serde_json::to_value(&reopened).unwrap() == serde_json::to_value(&deck).unwrap(),
        "persisted model/source changed"
    );
    assert!(reopened.warnings.iter().any(|w| w.code == "pptx-source"));
    std::fs::remove_file(native).unwrap();
    let target =
        std::env::temp_dir().join(format!("joydeck-office-table-{}.pptx", std::process::id()));
    let _ = std::fs::remove_file(&target);
    export::save_parts(
        &target,
        &pptx_write::generate(&deck).unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let again = interop::import(&target).unwrap();
    assert_eq!(
        again.slides[0]
            .objects
            .iter()
            .find_map(|o| o.table.as_ref())
            .unwrap(),
        t
    );
    std::fs::remove_file(target).unwrap();
}
