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
    if std::env::args().nth(1).as_deref() == Some("multivariate") {
        return multivariate(&mut context);
    }
    if std::env::args().nth(1).as_deref() == Some("paired") {
        return paired(&mut context);
    }
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
            measure(name, &args, size, repetitions, &mut context)?;
        }
    }
    Ok(())
}

fn measure(name: &str, args: &[Arg], size: usize, repetitions: usize, context: &mut Context) -> Result<(), &'static str> {
    let function = lookup(name).ok_or("missing statistic")?;
    let Value::Number(value) = call(function, args, context) else {
        return Err("expected a numeric result");
    };
    assert!(value.is_finite());
    let start = Instant::now();
    for _ in 0..repetitions {
        black_box(call(function, black_box(args), context));
    }
    println!("{name}-{size},{repetitions},{}", start.elapsed().as_nanos());
    Ok(())
}

fn paired(context: &mut Context) -> Result<(), &'static str> {
    if let Some(version) = std::env::args().nth(2) {
        let sample =
            Arg::reference(Array::new(1, 3, vec![Value::Number(1e100), Value::Number(2e100), Value::Number(3e100)]).ok_or("invalid sample")?);
        let test = call(lookup("CORREL").ok_or("missing CORREL")?, &[sample.clone(), sample], context);
        match (version.as_str(), test) {
            ("baseline", Value::Number(0.)) => {}
            ("candidate", Value::Number(r)) if (r - 1.).abs() < 1e-14 => {}
            _ => return Err("wrong regression implementation"),
        }
    }
    for (size, repetitions) in [(128, 2500), (32768, 15)] {
        let input = |step: usize| {
            let data = (0..size).map(|v| Value::Number((v * step % 127) as f64 - 53.)).collect();
            Array::new(size, 1, data).map(Arg::reference).ok_or("invalid test input")
        };
        let args = [Arg::val(14.), input(17)?, input(5)?];
        for name in ["CORREL", "SLOPE", "STEYX", "FORECAST", "COVARIANCE.P"] {
            measure(name, if name == "FORECAST" { &args } else { &args[1..] }, size, repetitions, context)?;
        }
    }
    Ok(())
}

fn multivariate(context: &mut Context) -> Result<(), &'static str> {
    for (size, repetitions) in [(128, 400), (16_384, 8)] {
        let mut predictors = Vec::with_capacity(size * 2);
        let mut ys = Vec::with_capacity(size);
        for i in 0..size {
            let (x, z) = ((i * 13 % 89) as f64, (i * 7 % 53) as f64);
            predictors.extend([Value::Number(x), Value::Number(z)]);
            ys.push(Value::Number(12. + x / 10. + z / 18. + (i % 11) as f64 / 20.));
        }
        let y = Arg::reference(Array::new(size, 1, ys).ok_or("invalid responses")?);
        let x = Arg::reference(Array::new(size, 2, predictors).ok_or("invalid predictors")?);
        for name in ["LINEST", "LOGEST", "TREND", "GROWTH"] {
            let mut args = vec![y.clone(), x.clone()];
            if matches!(name, "LINEST" | "LOGEST") {
                args.extend([Arg::val(true), Arg::val(true)]);
            } else {
                args.push(Arg::reference(Array::new(1, 2, vec![Value::Number(20.), Value::Number(3.)]).ok_or("invalid sample")?));
            }
            let function = lookup(name).ok_or("missing regression function")?;
            match call(function, &args, context) {
                Value::Array(a) if matches!(a.data.first(), Some(Value::Number(v)) if v.is_finite()) => (),
                Value::Number(v) if v.is_finite() => (),
                _ => return Err("expected a finite result"),
            }
            let start = Instant::now();
            for _ in 0..repetitions {
                black_box(call(function, black_box(&args), context));
            }
            println!("{name}-{size},{repetitions},{}", start.elapsed().as_nanos());
        }
    }
    Ok(())
}
