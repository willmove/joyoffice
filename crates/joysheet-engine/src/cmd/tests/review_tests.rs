use super::*;

fn book() -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("home.insertSheet", json!({"name": "Two"})).unwrap();
    s.execute("sheet.activate", json!({"sheet": 0})).unwrap();
    for (sheet, cell) in [(0, "C1"), (0, "A3"), (1, "B2")] {
        s.execute("review.newComment", json!({"sheet": sheet, "cell": cell, "text": "c", "author": "Ann"})).unwrap();
    }
    s.execute("review.newNote", json!({"sheet": 0, "cell": "A2", "text": "note", "author": "Bob"})).unwrap();
    s
}

fn at(s: &Session) -> (usize, String) {
    let d = s.doc().unwrap();
    (d.wb.active_sheet(), d.selection.active.a1())
}

#[test]
fn next_and_previous_comment_go_in_reading_order_across_sheets_and_wrap() {
    let mut s = book();
    s.execute("selection.set", json!({"cell": "B1"})).unwrap();
    // Notes are skipped: A2 holds a note, not a comment.
    let order = [(0, "C1"), (0, "A3"), (1, "B2"), (0, "C1")];
    for (sheet, cell) in order {
        s.execute("review.nextComment", json!({})).unwrap();
        assert_eq!(at(&s), (sheet, cell.to_string()));
    }
    for (sheet, cell) in [(1, "B2"), (0, "A3"), (0, "C1")] {
        s.execute("review.previousComment", json!({})).unwrap();
        assert_eq!(at(&s), (sheet, cell.to_string()));
    }
}

#[test]
fn comment_navigation_is_disabled_without_comments() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("review.newNote", json!({"cell": "A1", "text": "note"})).unwrap();
    assert!(s.execute("review.nextComment", json!({})).is_err());
    assert!(s.execute("review.previousComment", json!({})).is_err());
    assert_eq!(at(&s), (0, "A1".to_string()));
}

#[test]
fn protection_that_forbids_selecting_locked_cells_is_enforced_and_kept() {
    let mut s = book();
    s.execute("home.lockCell", json!({"range": "B2", "on": false})).unwrap();
    s.execute("review.protectSheet", json!({"selectLockedCells": false})).unwrap();
    // Locked cells can't be selected; unlocked ones can.
    assert!(s.execute("selection.set", json!({"cell": "A1"})).is_err());
    s.execute("selection.set", json!({"cell": "B2"})).unwrap();
    // Arrow keys skip locked cells, and stay put when nothing selectable is left.
    s.execute("selection.move", json!({"dr": 0, "dc": 1})).unwrap();
    assert_eq!(s.doc().unwrap().selection.active.a1(), "B2");
    // Re-protecting without the flag keeps the sheet's selection rights.
    s.execute("review.unprotectSheet", json!({})).unwrap();
    s.execute("review.protectSheet", json!({"selectLockedCells": false})).unwrap();
    s.execute("review.protectSheet", json!({"sort": true})).unwrap();
    assert!(!s.doc().unwrap().wb.active().unwrap().protection.clone().unwrap().select_locked);
    s.execute("review.unprotectSheet", json!({})).unwrap();
    s.execute("selection.set", json!({"cell": "A1"})).unwrap();
}

#[test]
fn sheet_passwords_use_excel_hashes_and_old_json_hashes_still_verify() {
    let mut s = book();
    s.execute("review.protectSheet", json!({"password": "pa55"})).unwrap();
    let prot = s.doc().unwrap().wb.active().unwrap().protection.clone().unwrap();
    assert!(matches!(&prot.password, Some(PasswordHash::Iterated { algorithm, .. }) if algorithm == "SHA-512"));
    assert!(s.execute("review.unprotectSheet", json!({})).is_err());
    assert!(s.execute("review.unprotectSheet", json!({"password": "pass"})).is_err());
    // Protecting again can't swap the password out from under it.
    assert!(s.execute("review.protectSheet", json!({})).is_err());
    assert_eq!(s.doc().unwrap().wb.active().unwrap().protection.as_ref(), Some(&prot));
    s.execute("review.unprotectSheet", json!({"password": "pa55"})).unwrap();
    assert!(!s.doc().unwrap().wb.active().unwrap().is_protected());
    // A GridCraft JSON workbook saved before carried the internal hash under `password_hash`.
    let old = format!(
        r#"{{"password_hash": {}, "select_locked": true, "select_unlocked": true, "format_cells": false, "format_columns": false, "format_rows": false, "insert_columns": false, "insert_rows": false, "delete_columns": false, "delete_rows": false, "sort": false, "autofilter": false}}"#,
        password_hash("old")
    );
    let prot: SheetProtection = serde_json::from_str(&old).unwrap();
    assert_eq!((prot.legacy_password_hash, prot.password.is_none()), (Some(password_hash("old")), true));
    let i = s.doc().unwrap().wb.active_sheet();
    let wb = std::sync::Arc::make_mut(&mut s.doc_mut().unwrap().wb);
    std::sync::Arc::make_mut(&mut wb.sheets[i]).protection = Some(prot);
    assert!(s.execute("review.unprotectSheet", json!({"password": "new"})).is_err());
    s.execute("review.unprotectSheet", json!({"password": "old"})).unwrap();
}

#[test]
fn protected_structure_blocks_rename_move_copy_hide_and_unhide_sheet() {
    let mut s = book();
    let names = |s: &Session| s.doc().unwrap().wb.sheets.iter().map(|x| x.name.clone()).collect::<Vec<_>>();
    let before = names(&s);
    s.execute("review.protectWorkbook", json!({})).unwrap();
    assert!(s.execute("sheet.rename", json!({"sheet": 0, "name": "Renamed"})).is_err());
    assert!(s.execute("sheet.move", json!({"sheet": 0, "to": 1})).is_err());
    assert!(s.execute("sheet.move", json!({"sheet": 0, "to": 1, "copy": true})).is_err());
    assert!(s.execute("sheet.hide", json!({"sheet": 1})).is_err());
    assert_eq!(names(&s), before);
    assert!(s.doc().unwrap().wb.sheets.iter().all(|x| x.visibility == Visibility::Visible));
    s.execute("review.protectWorkbook", json!({})).unwrap();
    s.execute("sheet.rename", json!({"sheet": 0, "name": "Renamed"})).unwrap();
    s.execute("sheet.move", json!({"sheet": 0, "to": 1})).unwrap();
    s.execute("sheet.move", json!({"sheet": 0, "to": 1, "copy": true})).unwrap();
    assert_eq!(s.doc().unwrap().wb.sheets.len(), 3);
    s.execute("sheet.hide", json!({"sheet": 1})).unwrap();
    s.execute("review.protectWorkbook", json!({})).unwrap();
    assert!(s.execute("sheet.unhide", json!({"sheet": 1})).is_err());
    assert!(s.execute("sheet.unhide", json!({})).is_err());
    assert_eq!(s.doc().unwrap().wb.sheets[1].visibility, Visibility::Hidden);
    s.execute("review.protectWorkbook", json!({})).unwrap();
    s.execute("sheet.unhide", json!({"sheet": 1})).unwrap();
}

#[test]
fn protect_workbook_with_a_password_really_verifies() {
    let mut s = book();
    s.execute("review.protectWorkbook", json!({"password": "pa55"})).unwrap();
    assert!(s.doc().unwrap().wb.protected_structure);
    assert!(s.execute("home.insertSheet", json!({"name": "Three"})).is_err(), "the structure is locked");
    // A second Protect keeps it protected; unprotecting needs the right password.
    assert_eq!(s.execute("review.protectWorkbook", json!({"on": true})).unwrap()["on"], true);
    assert!(s.execute("review.protectWorkbook", json!({})).is_err());
    assert!(s.execute("review.protectWorkbook", json!({"on": false, "password": "pass"})).is_err());
    assert!(s.doc().unwrap().wb.protected_structure);
    s.execute("review.protectWorkbook", json!({"on": false, "password": "pa55"})).unwrap();
    let wb = &s.doc().unwrap().wb;
    assert!(!wb.protected_structure && wb.structure_password.is_none());
    // Without a password it toggles freely.
    s.execute("review.protectWorkbook", json!({})).unwrap();
    s.execute("review.protectWorkbook", json!({})).unwrap();
    assert!(!s.doc().unwrap().wb.protected_structure);
}

#[test]
fn convert_to_comments_turns_every_note_into_a_comment_and_undoes() {
    let mut s = book();
    s.execute("review.newNote", json!({"sheet": 1, "cell": "D4", "text": "second", "author": "Cy"})).unwrap();
    s.execute("review.showNote", json!({"sheet": 1, "cell": "D4"})).unwrap();
    let r = s.execute("review.convertToComments", json!({})).unwrap();
    assert_eq!(r["converted"], 2);
    let wb = &s.doc().unwrap().wb;
    let a2 = &wb.sheets[0].comments[&CellRef::parse("A2").unwrap()];
    let d4 = &wb.sheets[1].comments[&CellRef::parse("D4").unwrap()];
    assert!(a2.threaded && d4.threaded && !d4.visible);
    assert_eq!((a2.author.as_str(), a2.text.as_str(), d4.author.as_str(), d4.text.as_str()), ("Bob", "note", "Cy", "second"));
    // Nothing left to convert; one undo restores the notes.
    assert_eq!(s.execute("review.convertToComments", json!({})).unwrap()["converted"], 0);
    s.execute("edit.undo", json!({})).unwrap();
    assert!(!s.doc().unwrap().wb.sheets[0].comments[&CellRef::parse("A2").unwrap()].threaded);
}

#[test]
fn show_comments_and_show_all_notes_toggle() {
    let mut s = book();
    assert_eq!(s.execute("review.showComments", json!({})).unwrap()["on"], true);
    assert_eq!(s.execute("review.showComments", json!({})).unwrap()["on"], false);
    assert_eq!(s.execute("review.showComments", json!({"on": true})).unwrap()["on"], true);
    assert!(s.comments_pane);
    assert_eq!(s.execute("review.showAllNotes", json!({})).unwrap()["on"], true);
    assert_eq!(s.execute("review.showAllNotes", json!({"on": false})).unwrap()["on"], false);
    assert!(!s.show_all_notes);
}
