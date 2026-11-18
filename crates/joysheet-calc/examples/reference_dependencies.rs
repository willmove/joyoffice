//! Measure row-local defined names, a shared constant and an unrelated edit.
//! Run twice with JOYSHEET_REFERENCE_DEPENDENCIES=0 and =1.
use joysheet_calc::Calc;
use joysheet_core::{CellRef, Value};
use joysheet_model::{Cell, DefinedName, Formula, Workbook};
use std::time::Instant;

fn main() -> Result<(), &'static str> {
    let rows = 4096;
    let mut wb = Workbook::new();
    for (name, formula, scope) in [("RowPrice", "$B1", Some(0)), ("TaxRate", "$F$1", None)] {
        wb.names.push(DefinedName { name: name.into(), formula: formula.into(), scope, comment: String::new(), hidden: false });
    }
    let rate = CellRef::new(0, 5);
    let local = CellRef::new(rows / 2, 1);
    let unrelated = CellRef::new(rows + 5, 7);
    let sheet = wb.sheet_mut(0).ok_or("missing sheet")?;
    let formula = Formula::new("RowPrice*TaxRate+1");
    let mut prices: Vec<_> = (0..rows).map(f64::from).collect();
    let mut tax = 3.;
    sheet.set_value(rate, Value::Number(tax));
    for row in 0..rows {
        sheet.set_value(CellRef::new(row, 1), Value::Number(prices[row as usize]));
        sheet.set_cell(CellRef::new(row, 3), Cell::formula(formula.clone()));
    }
    let mut calc = Calc::new();
    calc.recalc_all(&mut wb);
    println!("round,edit,formulas,microseconds");
    for round in 0..22 {
        for (mode, at) in [("unrelated", unrelated), ("local", local), ("shared", rate)] {
            let value = f64::from(round + 5);
            if at == rate {
                tax = value;
            } else if at == local {
                prices[at.row as usize] = value;
            }
            wb.sheet_mut(0).ok_or("missing sheet")?.set_value(at, Value::Number(value));
            let start = Instant::now();
            calc.cells_changed(&mut wb, &[(0, at)]);
            let elapsed = start.elapsed().as_secs_f64() * 1_000_000.;
            let sheet = wb.sheet(0).ok_or("missing sheet")?;
            for row in 0..rows {
                if sheet.value(CellRef::new(row, 3)) != Value::Number(prices[row as usize] * tax + 1.) {
                    return Err("stale result");
                }
            }
            if round >= 2 {
                println!("{round},{mode},{},{elapsed:.3}", calc.last_recalc_cells);
            }
        }
    }
    Ok(())
}
