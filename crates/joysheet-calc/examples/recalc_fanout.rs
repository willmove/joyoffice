//! Repeatable CPU-only fan-out edit, matching the desktop stress fixture.
use joysheet_calc::{Calc, profile};
use joysheet_core::{CellRef, Value};
use joysheet_model::{Cell, Formula, Workbook};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let rows: u32 = args.next().ok_or("usage: recalc_fanout ROWS")?.parse().map_err(|_| "invalid row count")?;
    if !(1..=100_000).contains(&rows) || args.next().is_some() {
        return Err("expected one row count between 1 and 100000".into());
    }
    let mut wb = Workbook::new();
    let input = CellRef::new(0, 0);
    let total = CellRef::new(0, 3);
    let sheet = wb.sheet_mut(0).ok_or("missing sheet")?;
    for i in 1..=rows {
        sheet.set_cell(CellRef::new(i - 1, 1), Cell::formula(Formula::new(&format!("=$A$1+{i}"))));
    }
    sheet.set_cell(total, Cell::formula(Formula::new(&format!("=SUM(B1:B{rows})"))));
    let mut calc = Calc::new();
    calc.recalc_all(&mut wb);
    println!("edit,rows,formulas,recalc_ms,dependency_ms,preparation_ms,evaluation_ms,writeback_ms");
    for edit in 1..=22 {
        let value = f64::from(edit);
        wb.sheet_mut(0).ok_or("missing sheet")?.set_value(input, Value::Number(value));
        profile::start();
        calc.cells_changed(&mut wb, &[(0, input)]);
        let sample = profile::finish();
        let sheet = wb.sheet(0).ok_or("missing sheet")?;
        for i in 1..=rows {
            if sheet.value(CellRef::new(i - 1, 1)) != Value::Number(value + f64::from(i)) {
                return Err(format!("wrong result at B{i} on edit {edit}"));
            }
        }
        let n = f64::from(rows);
        if sheet.value(total) != Value::Number(n * (n + 1.) / 2. + n * value) || sample.formulas != rows as usize + 1 {
            return Err(format!("aggregate or dependency count differs on edit {edit}"));
        }
        if edit > 2 {
            println!(
                "{},{rows},{},{:.4},{:.4},{:.4},{:.4},{:.4}",
                edit - 2,
                sample.formulas,
                sample.recalc_ms,
                sample.dependency_ms,
                sample.preparation_ms,
                sample.evaluation_ms,
                sample.writeback_ms
            );
        }
    }
    Ok(())
}
