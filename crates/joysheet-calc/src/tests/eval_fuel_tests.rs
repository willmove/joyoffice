use super::*;
use joysheet_model::Workbook;

struct Empty {
    wb: Workbook,
}
impl Host for Empty {
    fn workbook(&self) -> &Workbook {
        &self.wb
    }
    fn cell_value(&mut self, _sheet: usize, _c: CellRef) -> Value {
        Value::Empty
    }
    fn now_serial(&self) -> f64 {
        0.0
    }
    fn random(&mut self) -> f64 {
        0.0
    }
}

fn eval_with(fuel: u64, src: &str) -> Value {
    let mut host = Empty { wb: Workbook::default() };
    let mut ev = Evaluator::new(&mut host, 0, CellRef::new(0, 0));
    ev.fuel = fuel;
    ev.value(&joysheet_formula::parser::parse(src).unwrap_or_else(|e| panic!("{e}")))
}

#[test]
fn lambda_calls_spend_fuel_and_run_out_quietly() {
    // Three applications, one charge each: 3 inside the budget. (A nested MAP wouldn't do
    // here: an inner MAP result is an array, and MAP of array elements is #CALC! on shape,
    // which happens well before the budget does.)
    let r = eval_with(3, "=LAMBDA(x,LAMBDA(y,LAMBDA(z,z+1)(3))(2))(1)");
    assert_eq!(r, Value::number(4.0), "{r:?}");
    // One charge short: the nest bottoms out in #CALC! instead of running unbounded.
    let r = eval_with(2, "=LAMBDA(x,LAMBDA(y,LAMBDA(z,z+1)(3))(2))(1)");
    assert!(matches!(r, Value::Error(CellError::Calc)), "{r:?}");
    // One charge short of a single application: #CALC! with a zero budget.
    let r = eval_with(0, "=LAMBDA(x,x+1)(2)");
    assert!(matches!(r, Value::Error(CellError::Calc)), "{r:?}");
    let r = eval_with(1, "=LAMBDA(x,x+1)(2)");
    assert_eq!(r, Value::number(3.0));
    // A flat helper map keeps working (3 of 3 invokes inside the budget).
    let r = eval_with(3, "=MAP({1,2,3},LAMBDA(x,x*2))");
    assert!(matches!(r, Value::Array(_)) && !matches!(&r, Value::Array(a) if a.data.iter().any(|v| matches!(v, Value::Error(_)))), "{r:?}");
}
