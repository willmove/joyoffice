use joysheet_core::{Array, CellError, DateSystem, Value};

use crate::{Arg, Ctx};

/// Fixed clock (2026-10-07 12:00) and a deterministic random generator.
pub struct TestCtx {
    pub state: u64,
    pub sys: DateSystem,
}
impl Default for TestCtx {
    fn default() -> Self {
        TestCtx { state: 0x2545_F491_4F6C_DD1D, sys: DateSystem::D1900 }
    }
}
impl Ctx for TestCtx {
    fn date_system(&self) -> DateSystem {
        self.sys
    }
    fn now_serial(&self) -> f64 {
        46302.5
    }
    fn random(&mut self) -> f64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Calls a registered function by name with literal arguments.
pub fn ev(name: &str, args: Vec<Arg>) -> Value {
    let spec = crate::lookup(name).unwrap_or_else(|| panic!("function {name} not registered"));
    crate::call(spec, &args, &mut TestCtx::default())
}
/// Literal number argument.
pub fn n(x: f64) -> Arg {
    Arg::val(x)
}
/// Literal text argument.
pub fn t(s: &str) -> Arg {
    Arg::val(s)
}
/// Literal boolean argument.
pub fn b(x: bool) -> Arg {
    Arg::val(x)
}
/// Literal error argument.
pub fn e(x: CellError) -> Arg {
    Arg::val(x)
}
/// Empty literal argument (as in `F(1,)`).
pub fn empty() -> Arg {
    Arg::val(Value::Empty)
}
/// Array value from rows of values.
pub fn arr(rows: Vec<Vec<Value>>) -> Value {
    let r = rows.len();
    let c = rows.first().map_or(0, |x| x.len());
    Value::from(Array::new(r, c, rows.into_iter().flatten().collect()).expect("rectangular"))
}
/// Column of numbers as an array value.
pub fn col(xs: &[f64]) -> Value {
    arr(xs.iter().map(|x| vec![Value::Number(*x)]).collect())
}
/// Row of numbers as an array value.
pub fn row(xs: &[f64]) -> Value {
    arr(vec![xs.iter().map(|x| Value::Number(*x)).collect()])
}
/// Range argument (from a reference).
pub fn rf(v: Value) -> Arg {
    Arg::reference(v)
}
/// Array-constant argument.
pub fn av(v: Value) -> Arg {
    Arg::val(v)
}
pub fn tv(s: &str) -> Value {
    Value::from(s)
}
pub fn nv(x: f64) -> Value {
    Value::Number(x)
}
/// Asserts a numeric result within 1e-9 relative (or absolute near zero) tolerance.
#[track_caller]
pub fn close(v: Value, expected: f64) {
    match v {
        Value::Number(x) => {
            let tol = 1e-9 * expected.abs().max(1.0);
            assert!((x - expected).abs() <= tol, "got {x}, expected {expected}");
        }
        other => panic!("expected number {expected}, got {other:?}"),
    }
}
/// Like [`close`] with a custom relative tolerance.
#[track_caller]
pub fn close_tol(v: Value, expected: f64, tol: f64) {
    match v {
        Value::Number(x) => {
            let t = tol * expected.abs().max(1.0);
            assert!((x - expected).abs() <= t, "got {x}, expected {expected}");
        }
        other => panic!("expected number {expected}, got {other:?}"),
    }
}
#[track_caller]
pub fn is_err(v: Value, err: CellError) {
    assert_eq!(v, Value::Error(err));
}
#[track_caller]
pub fn is_text(v: Value, s: &str) {
    assert_eq!(v, Value::from(s));
}
/// Array result as rows of values.
#[track_caller]
pub fn rows_of(v: &Value) -> Vec<Vec<Value>> {
    match v {
        Value::Array(a) => (0..a.rows).map(|r| (0..a.cols).map(|c| a.get(r, c).cloned().unwrap_or(Value::Empty)).collect()).collect(),
        other => vec![vec![other.clone()]],
    }
}
