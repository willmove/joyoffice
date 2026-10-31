use super::*;

#[test]
fn range_parse_rejects_signed_rows() {
    assert!(RangeRef::parse("+1:+3").is_none());
    assert!(RangeRef::parse("-1:3").is_none());
    assert!(RangeRef::parse("$$1:3").is_none());
    assert!(RangeRef::parse(":3").is_none());
    assert!(RangeRef::parse("$$A:$$C").is_none());
    assert!(RangeRef::parse("$A:$C").is_some());
    let r = RangeRef::parse("$1:$3").unwrap();
    assert_eq!((r.start.row, r.end.row), (0, 2));
}

#[test]
fn sheet_quoting_follows_excel() {
    for quoted in [
        "A1",
        "XFD1",
        "AB1048576",
        "R1",
        "C1",
        "RC1",
        "R",
        "C",
        "RC",
        "R1C",
        "R1C1",
        "r2c3",
        "CR1",
        ".x",
        "TRUE",
        "false",
        "1abc",
        "My Sheet",
        "Bob's",
        "a-b",
        "",
    ] {
        assert!(sheet_needs_quotes(quoted), "{quoted:?} should be quoted");
    }
    for bare in ["Sheet1", "XFE1", "AB1048577", "Ärger", "日本", "Σ", "Sheet.1", "_x", "A1A", "Data", "RR", "CRX"] {
        assert!(!sheet_needs_quotes(bare), "{bare:?} should stay bare");
    }
}

#[test]
fn quote_unquote_and_split_sheet() {
    assert_eq!(quote_sheet("Bob's"), "'Bob''s'");
    assert_eq!(quote_sheet("Data"), "Data");
    assert_eq!(unquote_sheet("'Bob''s'"), "Bob's");
    assert_eq!(unquote_sheet("Data"), "Data");
    assert_eq!(unquote_sheet("'"), "'");
    assert_eq!(split_sheet("'a!b'!A1"), (Some("a!b".to_string()), "A1"));
    assert_eq!(split_sheet("'Bob''s'!$A$1:$B$2"), (Some("Bob's".to_string()), "$A$1:$B$2"));
    assert_eq!(split_sheet("A1"), (None, "A1"));
}

/// #2312: device/backslash garbage in front of `!` is cosmetic at worst — `split_sheet` only
/// splits at `!`, and every consumer matches the sheet part against workbook sheet names (it
/// never reaches a filesystem path or a shell); the body half always stays a pure cell/range
/// reference.
#[test]
fn split_sheet_leaves_device_names_inert_in_the_body() {
    for s in ["C:\\a\\file!A1", "CON!A1", "NUL!A1", "a[b]!A1"] {
        let (sheet, rest) = split_sheet(s);
        assert_eq!(CellRef::parse(rest), Some(CellRef::new(0, 0)), "{s:?}: body must stay a bare reference");
        let _ = sheet;
    }
    // A plain single-letter sheet is still split, not mistaken for a body.
    assert_eq!(split_sheet("C!A1").0, Some("C".to_string()));
}

#[test]
fn letters_roundtrip() {
    for (c, s) in [(0, "A"), (25, "Z"), (26, "AA"), (51, "AZ"), (52, "BA"), (701, "ZZ"), (702, "AAA"), (16383, "XFD")] {
        assert_eq!(col_to_letters(c), s);
        assert_eq!(letters_to_col(s), Some(c));
    }
    assert_eq!(letters_to_col("XFE"), None);
    assert_eq!(letters_to_col(""), None);
    assert_eq!(letters_to_col("A1"), None);
}

#[test]
fn parse_cells() {
    assert_eq!(CellRef::parse("B7"), Some(CellRef::new(6, 1)));
    assert_eq!(CellRef::parse("$b$7"), Some(CellRef::new(6, 1)));
    assert_eq!(CellRef::parse("XFD1048576"), Some(CellRef::new(MAX_ROWS - 1, MAX_COLS - 1)));
    assert_eq!(CellRef::parse("A0"), None);
    assert_eq!(CellRef::parse("A1048577"), None);
    assert_eq!(CellRef::parse("A1x"), None);
    assert_eq!(CellRef::parse(""), None);
    assert_eq!(CellRef::parse("$"), None);
}

#[test]
fn ranges() {
    let r = RangeRef::parse("C3:A1").unwrap();
    assert_eq!(r.a1(), "A1:C3");
    assert_eq!(r.count(), 9);
    assert_eq!(RangeRef::parse("A:C").unwrap().a1(), "A:C");
    assert_eq!(RangeRef::parse("3:5").unwrap().a1(), "3:5");
    assert!(RangeRef::parse("0:5").is_none());
    let a = RangeRef::parse("A1:C3").unwrap();
    let b = RangeRef::parse("B2:D4").unwrap();
    assert_eq!(a.intersection(&b).unwrap().a1(), "B2:C3");
    assert_eq!(a.union(&b).a1(), "A1:D4");
    assert_eq!(a.iter().count(), 9);
}

#[test]
fn extreme_offsets() {
    let cell = CellRef::new(1, 1);
    for delta in [i64::MIN, i64::MAX] {
        assert_eq!(cell.offset(delta, 0), None);
        assert_eq!(cell.offset(0, delta), None);
        assert_eq!(cell.offset(delta, delta), None);
    }
    assert_eq!(cell.offset_clamped(i64::MAX, i64::MAX), CellRef::new(MAX_ROWS - 1, MAX_COLS - 1));
    assert_eq!(cell.offset_clamped(i64::MIN, i64::MIN), CellRef::new(0, 0));
    assert_eq!(cell.offset_clamped(i64::MAX, i64::MIN), CellRef::new(MAX_ROWS - 1, 0));
    assert_eq!(cell.offset_clamped(i64::MIN, i64::MAX), CellRef::new(0, MAX_COLS - 1));
    assert_eq!(cell.offset(2, -1), Some(CellRef::new(3, 0)));
}

#[test]
fn offsets() {
    assert_eq!(CellRef::new(0, 0).offset(-1, 0), None);
    assert_eq!(CellRef::new(0, 0).offset_clamped(-5, 3), CellRef::new(0, 3));
}
