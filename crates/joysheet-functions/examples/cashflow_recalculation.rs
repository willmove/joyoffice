//! Measure the five public cash-flow functions on short and long payment schedules.
use joysheet_core::{Array, DateSystem, Value};
use joysheet_functions::{Arg, Ctx, call, lookup};
use std::{hint::black_box, time::Instant};

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

fn column(values: impl Iterator<Item = f64>) -> Result<Arg, &'static str> {
    let data = values.map(Value::Number).collect::<Vec<_>>();
    Ok(Arg::reference(Array::new(data.len(), 1, data).ok_or("invalid test range")?))
}

fn main() -> Result<(), &'static str> {
    for (count, repeats) in [(24usize, 240), (1200, 12)] {
        let payments = column((0..count).map(|i| if i == 0 { -10.0 * count as f64 } else { 11.0 + (i % 3) as f64 }))?;
        let dates = column((0..count).map(|i| 45000.0 + (3 * i + i / 5) as f64))?;
        for (name, args) in [
            ("NPV", vec![Arg::val(0.035), payments.clone()]),
            ("IRR", vec![payments.clone(), Arg::val(0.001)]),
            ("MIRR", vec![payments.clone(), Arg::val(0.002), Arg::val(0.001)]),
            ("XNPV", vec![Arg::val(0.035), payments.clone(), dates.clone()]),
            ("XIRR", vec![payments.clone(), dates.clone(), Arg::val(0.1)]),
        ] {
            let function = lookup(name).ok_or("missing cash-flow function")?;
            let result = call(function, &args, &mut Context).as_f64().ok_or("invalid cash-flow result")?;
            let timer = Instant::now();
            for _ in 0..repeats {
                if call(function, black_box(&args), &mut Context).as_f64() != Some(result) {
                    return Err("inconsistent cash-flow result");
                }
            }
            println!("{name},{count},{repeats},{},{result}", timer.elapsed().as_nanos());
        }
    }
    Ok(())
}
