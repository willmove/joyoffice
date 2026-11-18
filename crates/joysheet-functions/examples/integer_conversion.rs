//! Time worksheet integer conversions on numeric and text cells through normal function dispatch.
use std::{hint::black_box, time::Instant};

use joysheet_core::{DateSystem, Value};
use joysheet_functions::{Arg, Ctx, call, lookup};

struct Context;
impl Ctx for Context {
    fn date_system(&self) -> DateSystem {
        DateSystem::D1900
    }
    fn now_serial(&self) -> f64 {
        45000.0
    }
    fn random(&mut self) -> f64 {
        0.5
    }
}

fn main() {
    let cases = [
        ("binary-number", "BIN2DEC", vec![Arg::val(1_010_011_011.)], Value::Number(-357.)),
        ("binary-text", "BIN2DEC", vec![Arg::val("1010011011")], Value::Number(-357.)),
        ("pad-hexadecimal", "DEC2HEX", vec![Arg::val(65535.), Arg::val(8.)], Value::from("0000FFFF")),
        ("base-36", "BASE", vec![Arg::val(60466175.), Arg::val(36.)], Value::from("ZZZZZ")),
        ("read-hexadecimal", "DECIMAL", vec![Arg::val("1E01AB"), Arg::val(16.)], Value::Number(1966507.)),
        ("shift", "BITLSHIFT", vec![Arg::val(21845.), Arg::val(11.)], Value::Number(44738560.)),
    ];
    for (label, function, args, expected) in cases {
        let f = lookup(function).expect("registered worksheet function");
        assert_eq!(call(f, &args, &mut Context), expected, "{label}");
        let repeat = 100_000;
        let start = Instant::now();
        for _ in 0..repeat {
            black_box(call(f, black_box(&args), &mut Context));
        }
        println!("{label},{repeat},{}", start.elapsed().as_nanos());
    }
}
