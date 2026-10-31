use super::*;

#[test]
fn registry_is_consistent() {
    let mut seen = std::collections::HashSet::new();
    for s in all() {
        assert!(seen.insert(s.name), "duplicate function {}", s.name);
        assert_eq!(s.name, s.name.to_ascii_uppercase(), "{} not upper-case", s.name);
        assert!(s.min_args <= s.max_args, "{} arity", s.name);
        assert!(!s.description.is_empty() && !s.signature.is_empty(), "{} docs", s.name);
    }
    println!("registered functions: {}", all().len());
    assert!(all().len() >= 400);
    assert!(lookup("_xlfn._xlws.sort").is_some_and(|s| s.name == "SORT"));
    assert!(lookup("nosuchfn").is_none());
    for s in all() {
        assert!(lookup(&format!("_xlfn.{}", s.name.to_ascii_lowercase())).is_some_and(|x| x.name == s.name));
    }
}

/// #2163: a NaN column index is refused (it used to become 0 and underflow to `usize::MAX`).
#[test]
fn nan_index_is_a_value_error() {
    use crate::util::testutil::*;
    use joysheet_core::CellError;
    let table = rf(arr(vec![vec![tv("Field"), tv("Other")], vec![nv(1.0), nv(2.0)]]));
    let nan = || av(Value::Number(f64::NAN));
    assert_eq!(ev("VLOOKUP", vec![n(1.0), table.clone(), nan()]), Value::Error(CellError::Value));
    assert_eq!(ev("HLOOKUP", vec![t("Field"), table.clone(), nan()]), Value::Error(CellError::Value));
    assert_eq!(ev("DSUM", vec![table.clone(), nan(), table.clone()]), Value::Error(CellError::Value));
    assert_eq!(ev("VLOOKUP", vec![n(1.0), table, n(2.0)]), Value::Number(2.0));
}

/// #2162: REGEXREPLACE gives up on a too-long result while building it, and otherwise replaces
/// as before (every match, `$n` groups).
#[test]
fn regexreplace_bounds_its_result() {
    use crate::util::testutil::*;
    use joysheet_core::CellError;
    let many = "a".repeat(32_767);
    assert_eq!(ev("REGEXREPLACE", vec![t(&many), t("."), t(&"b".repeat(32_767))]), Value::Error(CellError::Value));
    assert_eq!(ev("REGEXREPLACE", vec![t(&many), t("a"), t("b")]), Value::text("b".repeat(32_767)));
    assert_eq!(ev("REGEXREPLACE", vec![t("a1b22c"), t("[0-9]+"), t("#")]), Value::text("a#b#c"));
    assert_eq!(ev("REGEXREPLACE", vec![t("2026-10-22"), t("(\\d+)-(\\d+)-(\\d+)"), t("$3/$2/$1")]), Value::text("22/10/2026"));
    assert_eq!(ev("REGEXREPLACE", vec![t("abc"), t("x*"), t("-")]), Value::text("-a-b-c-"));
    assert_eq!(ev("REGEXREPLACE", vec![t("a1b2"), t("[0-9]"), t("#"), n(2.0)]), Value::text("a1b#"));
    // One match whose replacement repeats it 10,000 times: refused before it is built.
    assert_eq!(ev("REGEXREPLACE", vec![t(&many), t("^.*$"), t(&"$0".repeat(10_000))]), Value::Error(CellError::Value));
    assert_eq!(ev("REGEXREPLACE", vec![t(&many), t("^.*$"), t(&"$0".repeat(10_000)), n(1.0)]), Value::Error(CellError::Value));
}

#[test]
fn hostile_inputs_never_panic() {
    use crate::util::testutil::*;
    use joysheet_core::{Array, CellError};
    let mixed = Value::from(
        Array::new(2, 3, vec![Value::Number(1.0), Value::from("x"), Value::Empty, Value::Bool(true), Value::Number(-1e300), Value::Number(0.5)])
            .unwrap(),
    );
    let patterns: Vec<Arg> = vec![
        n(0.0),
        n(-1.0),
        n(1e308),
        n(-1e308),
        n(1e10),
        n(2.5),
        // Numbers no formula should make, but code can (#2163).
        av(Value::Number(f64::NAN)),
        av(Value::Number(f64::INFINITY)),
        t(""),
        t("abc"),
        t("*?~"),
        b(true),
        e(CellError::NA),
        empty(),
        av(mixed.clone()),
        rf(mixed),
        rf(col(&[3.0, 1.0, 2.0])),
    ];
    for spec in all() {
        let max = spec.max_args.min(spec.min_args + 3);
        for count in spec.min_args..=max {
            for p in &patterns {
                let args = vec![p.clone(); count];
                let _ = call(spec, &args, &mut TestCtx::default());
            }
            // Mixed: cycle through patterns.
            let args: Vec<Arg> = (0..count).map(|i| patterns[(i * 7 + count) % patterns.len()].clone()).collect();
            let _ = call(spec, &args, &mut TestCtx::default());
        }
    }
}
