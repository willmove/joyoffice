use super::*;
use joysheet_core::Value;

#[test]
fn jumps() {
    let mut s = Sheet::new("S");
    for r in 0..5 {
        s.set_value(CellRef::new(r, 0), Value::Number(1.0));
    }
    s.set_value(CellRef::new(10, 0), Value::Number(1.0));
    assert_eq!(jump(&s, CellRef::new(0, 0), 1, 0), CellRef::new(4, 0));
    // Without a direction it stays put (it used to loop forever from a blank cell).
    assert_eq!(jump(&s, CellRef::new(7, 0), 0, 0), CellRef::new(7, 0));
    assert_eq!(jump(&s, CellRef::new(4, 0), 1, 0), CellRef::new(10, 0));
    assert_eq!(jump(&s, CellRef::new(10, 0), 1, 0), CellRef::new(MAX_ROWS - 1, 0));
    assert_eq!(jump(&s, CellRef::new(0, 0), 0, 1), CellRef::new(0, MAX_COLS - 1));
    assert_eq!(jump(&s, CellRef::new(0, 0), -1, 0), CellRef::new(0, 0));
}

#[test]
fn regions() {
    let mut s = Sheet::new("S");
    for r in 2..5 {
        for c in 1..4 {
            s.set_value(CellRef::new(r, c), Value::Number(1.0));
        }
    }
    assert_eq!(current_region(&s, CellRef::new(3, 2)).a1(), "B3:D5");
    assert_eq!(current_region(&s, CellRef::new(5, 4)).a1(), "B3:E6");
    assert_eq!(current_region(&s, CellRef::new(20, 20)).a1(), "U21");
    // Taller than 10,000 rows: it used to stop growing after 10,000 steps.
    let mut s = Sheet::new("S");
    for r in 0..20_000 {
        s.set_value(CellRef::new(r, 0), Value::Number(r as f64));
        s.set_value(CellRef::new(r, 1), Value::Number(1.0));
    }
    assert_eq!(current_region(&s, CellRef::new(5, 0)).a1(), "A1:B20000");
}

#[test]
fn cycling() {
    let mut sel = Selection::range(RangeRef::parse("A1:B2").unwrap());
    sel.cycle(true, false);
    assert_eq!(sel.active.a1(), "A2");
    sel.cycle(true, false);
    assert_eq!(sel.active.a1(), "B1");
    sel.cycle(true, true);
    assert_eq!(sel.active.a1(), "A2");
}
