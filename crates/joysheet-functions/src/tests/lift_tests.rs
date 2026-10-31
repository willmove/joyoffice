use joysheet_core::{CellError, Value};

use crate::util::testutil::*;
use crate::util::{R, num};
use crate::{Arg, Ctx, FnSpec};

fn add(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    Ok(Value::Number(num(a, 0)? + num(a, 1)?))
}

fn spec() -> FnSpec {
    f!("ADD2", 2, 2, MathTrig, &[true, true], "ADD2(a, b)", "Adds two numbers.", add)
}

#[test]
fn broadcasts_row_and_column() {
    let s = spec();
    let out = crate::call(&s, &[av(row(&[1.0, 2.0, 3.0])), av(col(&[10.0, 20.0]))], &mut TestCtx::default());
    assert_eq!(rows_of(&out), vec![vec![nv(11.0), nv(12.0), nv(13.0)], vec![nv(21.0), nv(22.0), nv(23.0)]]);
}

#[test]
fn mismatched_sizes_pad_with_na() {
    let s = spec();
    let out = crate::call(&s, &[av(row(&[1.0, 2.0, 3.0])), av(row(&[1.0, 1.0]))], &mut TestCtx::default());
    assert_eq!(rows_of(&out), vec![vec![nv(2.0), nv(3.0), Value::Error(CellError::NA)]]);
}

#[test]
fn single_cell_unwraps_and_arity() {
    let s = spec();
    let out = crate::call(&s, &[rf(col(&[4.0])), n(1.0)], &mut TestCtx::default());
    assert_eq!(out, nv(5.0));
    is_err(crate::call(&s, &[n(1.0)], &mut TestCtx::default()), CellError::Value);
    let huge = crate::call(
        &s,
        &[av(joysheet_core::Array::filled(1, 100_000, nv(1.0)).into()), av(joysheet_core::Array::filled(100_000, 1, nv(1.0)).into())],
        &mut TestCtx::default(),
    );
    is_err(huge, CellError::Num);
}

#[test]
fn value_params_reject_multi_cell_references() {
    let s = FnSpec { value_args: &[true, false], ..spec() };
    let mut ctx = TestCtx::default();
    // A multi-cell reference is #VALUE!; a one-cell reference and a computed array are fine.
    is_err(crate::call(&s, &[rf(col(&[1.0, 2.0, 3.0])), n(1.0)], &mut ctx), CellError::Value);
    assert_eq!(crate::call(&s, &[rf(col(&[4.0])), n(1.0)], &mut ctx), nv(5.0));
    assert_eq!(rows_of(&crate::call(&s, &[av(col(&[1.0, 2.0])), n(1.0)], &mut ctx)), vec![vec![nv(2.0)], vec![nv(3.0)]]);
    // The rejected reference adds no shape: the other argument still lifts.
    let v = Value::Error(CellError::Value);
    assert_eq!(rows_of(&crate::call(&s, &[rf(col(&[1.0, 2.0, 3.0])), av(col(&[1.0, 2.0]))], &mut ctx)), vec![vec![v.clone()], vec![v]]);
    // A position that isn't a value parameter still lifts the reference.
    assert_eq!(rows_of(&crate::call(&s, &[n(1.0), rf(row(&[1.0, 2.0]))], &mut ctx)), vec![vec![nv(2.0), nv(3.0)]]);
}

#[test]
fn value_arg_table_names_registered_scalar_positions() {
    for (names, mask) in super::VALUE_ARGS {
        assert!(!mask.is_empty());
        for name in names.split_ascii_whitespace() {
            let Some(s) = crate::lookup(name) else { panic!("{name} isn't registered") };
            assert_eq!(s.value_args, *mask, "{name}");
            for i in 0..s.max_args.min(12) {
                assert!(!s.is_value_arg(i) || s.is_scalar_arg(i), "{name} argument {i} is a value parameter but doesn't lift");
            }
        }
    }
    assert!(crate::lookup("WORKDAY").is_some_and(|s| s.is_value_arg(1) && !s.is_value_arg(2)));
    assert!(crate::lookup("WORKDAY.INTL").is_some_and(|s| !s.is_value_arg(2) && s.is_scalar_arg(2)));
    assert!(crate::lookup("SUM").is_some_and(|s| s.value_args.is_empty()));
    assert!(crate::lookup("ISOWEEKNUM").is_some_and(|s| s.value_args.is_empty()));
}
