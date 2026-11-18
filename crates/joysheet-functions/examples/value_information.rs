//! Time value predicates through the worksheet registry, checking each result before sampling.
use std::{hint::black_box, time::Instant};

use joysheet_core::{CellError, DateSystem, Value};
use joysheet_functions::{Arg, Ctx, call, lookup};

struct Context;
impl Ctx for Context {
    fn date_system(&self) -> DateSystem {
        DateSystem::D1900
    }
    fn now_serial(&self) -> f64 {
        45000.
    }
    fn random(&mut self) -> f64 {
        0.5
    }
}

fn main() -> Result<(), &'static str> {
    // Paired runs verify the selected binary on a case whose result changed.
    // With no argument this example remains a standalone timing command.
    if let Some(label) = std::env::args().nth(1) {
        let expected = match label.as_str() {
            "baseline" => Value::Error(CellError::Num),
            "candidate" => Value::Bool(true),
            _ => return Err("expected baseline or candidate"),
        };
        let parity = lookup("ISEVEN").ok_or("worksheet function is missing")?;
        assert_eq!(call(parity, &[Arg::val(2_f64.powi(54))], &mut Context), expected);
    }
    let cases = [
        ("text-predicate", "ISTEXT", Arg::reference("中文工作表".repeat(128)), Value::Bool(true)),
        ("number-predicate", "ISNUMBER", Arg::reference(12.), Value::Bool(true)),
        ("na-predicate", "ISERR", Arg::reference(Value::Error(CellError::NA)), Value::Bool(false)),
        ("text-type", "TYPE", Arg::reference("instrument"), Value::Number(2.)),
        ("fractional-parity", "ISODD", Arg::val(-391.75), Value::Bool(true)),
        ("reference-boolean", "N", Arg::reference(true), Value::Number(1.)),
    ];
    let repetitions = 100_000;
    for (label, name, arg, expected) in cases {
        let spec = lookup(name).ok_or("worksheet function is missing")?;
        let args = [arg];
        assert_eq!(call(spec, &args, &mut Context), expected);
        let start = Instant::now();
        for _ in 0..repetitions {
            black_box(call(spec, black_box(&args), &mut Context));
        }
        println!("{label},{repetitions},{}", start.elapsed().as_nanos());
    }
    Ok(())
}
