//! Measure worksheet registry calls on referenced ranges of two sizes.
use std::{hint::black_box, time::Instant};

use joysheet_core::{Array, CellError, DateSystem, Value};
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
    let mut context = Context;
    if let Some(version) = std::env::args().nth(1) {
        let skew = lookup("SKEW.P").ok_or("missing SKEW.P")?;
        let pair = [Arg::val(1.), Arg::val(2.)];
        let expected = match version.as_str() {
            "baseline" => Value::Number(0.),
            "candidate" => Value::Error(CellError::Div0),
            _ => return Err("expected baseline or candidate"),
        };
        assert_eq!(call(skew, &pair, &mut context), expected);
    }
    for (size, repetitions) in [(128, 2500), (32768, 15)] {
        let cells = (0..size).map(|n| Value::Number((n % 127) as f64 - 48.)).collect();
        let args = [Arg::reference(Array::new(size, 1, cells).ok_or("invalid range")?)];
        for name in ["AVERAGEA", "VAR.S", "STDEVPA", "AVEDEV", "SKEW", "KURT"] {
            let function = lookup(name).ok_or("missing statistic")?;
            let Value::Number(value) = call(function, &args, &mut context) else {
                return Err("expected a numeric result");
            };
            assert!(value.is_finite());
            let start = Instant::now();
            for _ in 0..repetitions {
                black_box(call(function, black_box(&args), &mut context));
            }
            println!("{name}-{size},{repetitions},{}", start.elapsed().as_nanos());
        }
    }
    Ok(())
}
