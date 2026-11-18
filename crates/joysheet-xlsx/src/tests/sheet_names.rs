//! Sheet names XLSX cannot hold are refused on export instead of breaking formulas on reopen.

use std::sync::Arc;

use joysheet_model::{Sheet, Workbook};

use crate::{IoError, write_xlsx};

fn book(names: &[&str]) -> Workbook {
    let mut wb = Workbook::new();
    wb.sheets = names.iter().map(|n| Arc::new(Sheet::new(*n))).collect();
    wb
}

#[test]
fn unrepresentable_sheet_names_are_refused() {
    let long_a = format!("{}X", "A".repeat(31));
    let long_b = format!("{}Y", "A".repeat(31));
    for names in [vec!["Data", "a/b"], vec![long_a.as_str(), long_b.as_str()], vec!["Data", "data"], vec!["Data", ""]] {
        assert!(matches!(write_xlsx(&book(&names)), Err(IoError::Format(_))), "{names:?}");
    }
    assert!(write_xlsx(&book(&["Data", "Other"])).is_ok());
}

/// A saved sheet keeps its `sheetId`; a copied or new sheet gets one no other sheet asks for.
#[test]
fn sheet_ids_survive_a_save() {
    let mut wb = book(&["A", "B", "C", "D"]);
    for (sheet, id) in wb.sheets.iter_mut().zip([3, 7, 0, 3]) {
        Arc::make_mut(sheet).file_id = id;
    }
    let (read, _) = crate::read_xlsx(&write_xlsx(&wb).unwrap()).unwrap();
    let ids: Vec<u32> = read.sheets.iter().map(|s| s.file_id).collect();
    assert_eq!(ids, [3, 7, 1, 2]);
    let (again, _) = crate::read_xlsx(&write_xlsx(&read).unwrap()).unwrap();
    assert_eq!(again.sheets.iter().map(|s| s.file_id).collect::<Vec<_>>(), ids);
}

#[test]
fn restored_window_uses_an_existing_visible_sheet_without_unhiding_sheets() {
    use joysheet_model::Visibility::{Hidden, VeryHidden, Visible};
    let mut wb = book(&["Hidden", "VeryHidden", "First", "Chosen"]);
    for (sheet, state) in wb.sheets.iter_mut().zip([Hidden, VeryHidden, Visible, Visible]) {
        Arc::make_mut(sheet).visibility = state;
    }
    for (stored, expected) in [(Some(3), 3), (None, 2), (Some(0), 2), (Some(1), 2), (Some(999), 2)] {
        crate::restore_active_sheet(&wb, stored);
        assert_eq!(wb.active_sheet(), expected);
    }
    for sheet in &mut wb.sheets {
        Arc::make_mut(sheet).visibility = VeryHidden;
    }
    crate::restore_active_sheet(&wb, Some(3));
    assert_eq!(wb.active_sheet(), 0);
    assert!(wb.sheets.iter().all(|sheet| sheet.visibility == VeryHidden));
}
