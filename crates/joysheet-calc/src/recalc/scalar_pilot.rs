//! JoyOffice pilot for a binary operation on a stored numeric constant and a literal.
//! This subset cannot recurse, consult a pending formula, or read a spill from the current pass.
//! Reference resolution, coercion, and all other expressions stay with the general evaluator.
use joysheet_core::{CellError, Value};
use joysheet_formula::{BinOp, Expr, RefKind, Reference, SheetSel};
use joysheet_model::Workbook;

pub(super) fn evaluate(book: &Workbook, sheet: usize, expr: &Expr) -> Option<Value> {
    let Expr::Binary(op @ (BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div), left, right) = expr else {
        return None;
    };
    let (reference, literal, reference_first) = match (left.as_ref(), right.as_ref()) {
        (Expr::Ref(r), Expr::Number(n)) => (r, *n, true),
        (Expr::Number(n), Expr::Ref(r)) => (r, *n, false),
        _ => return None,
    };
    let Reference { sheet: SheetSel::Current, kind: RefKind::Cell(anchor) } = reference else { return None };
    let cell = book.sheet(sheet)?.cell(anchor.cell())?;
    let Value::Number(stored) = cell.value else { return None };
    if cell.formula.is_some() || !stored.is_finite() || !literal.is_finite() {
        return None;
    }
    let (a, b) = if reference_first { (stored, literal) } else { (literal, stored) };
    Some(match op {
        BinOp::Add => Value::number(a + b),
        BinOp::Sub => Value::number(a - b),
        BinOp::Mul => Value::number(a * b),
        BinOp::Div if b == 0.0 => Value::Error(CellError::Div0),
        BinOp::Div => Value::number(a / b),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use joysheet_core::CellRef;
    use joysheet_model::{Cell, Formula};

    #[test]
    fn eligible_arithmetic_agrees_with_the_independent_general_path() -> Result<(), Box<dyn std::error::Error>> {
        let mut book = Workbook::new();
        let at = CellRef::new(0, 1);
        for input in [0.0, -0.0, -7.5, 2.0, f64::MAX] {
            book.sheet_mut(0).ok_or("missing sheet")?.set_value(CellRef::new(0, 0), Value::Number(input));
            for formula in ["=$A$1+2", "=2+A1", "=A1-2", "=2-A1", "=A1*2", "=2*A1", "=A1/0", "=2/A1"] {
                let expr = joysheet_formula::parse(formula)?;
                assert_eq!(evaluate(&book, 0, &expr), Some(crate::recalc::evaluate_expr(&book, 0, at, &expr)));
            }
        }
        Ok(())
    }

    #[test]
    fn declines_formulas_even_with_a_numeric_cached_value() -> Result<(), Box<dyn std::error::Error>> {
        let mut book = Workbook::new();
        let expr = joysheet_formula::parse("=A1+1")?;
        let a1 = CellRef::new(0, 0);
        assert_eq!(evaluate(&book, 0, &expr), None);
        book.sheet_mut(0).ok_or("missing sheet")?.set_value(a1, Value::text("12"));
        assert_eq!(evaluate(&book, 0, &expr), None);
        let mut cell = Cell::formula(Formula::new("=1+1"));
        cell.value = Value::Number(2.);
        book.sheet_mut(0).ok_or("missing sheet")?.set_cell(a1, cell);
        assert_eq!(evaluate(&book, 0, &expr), None);
        Ok(())
    }

    #[test]
    fn reference_errors_and_other_expression_shapes_use_the_general_evaluator() -> Result<(), Box<dyn std::error::Error>> {
        let mut book = Workbook::new();
        let a1 = CellRef::new(0, 0);
        book.sheet_mut(0).ok_or("missing sheet")?.set_value(a1, Value::Error(CellError::Ref));
        assert_eq!(evaluate(&book, 0, &joysheet_formula::parse("=A1+2")?), None);
        book.sheet_mut(0).ok_or("missing sheet")?.set_value(a1, Value::Number(3.0));
        for formula in ["=Sheet2!A1+2", "=A1:A3+2", "=A1+A1", "=A1^2", "=SUM(A1,2)"] {
            assert_eq!(evaluate(&book, 0, &joysheet_formula::parse(formula)?), None, "{formula}");
        }
        Ok(())
    }
}
