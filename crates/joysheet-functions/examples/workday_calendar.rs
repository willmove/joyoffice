//! Time WORKDAY in both directions across an uninterrupted run of excluded working days.
//! Run the same example with a release build of each revision.
use std::hint::black_box;
use std::time::Instant;

use joysheet_core::{Array, DateSystem, Value};
use joysheet_functions::{Arg, Ctx, call, lookup};

struct CalendarContext;
impl Ctx for CalendarContext {
    fn date_system(&self) -> DateSystem {
        DateSystem::D1900
    }
    fn now_serial(&self) -> f64 {
        45292.0
    }
    fn random(&mut self) -> f64 {
        0.5
    }
}

fn main() -> Result<(), &'static str> {
    let start = 45292;
    let mut context = CalendarContext;
    let workday = lookup("WORKDAY").ok_or("WORKDAY is missing")?;
    for (count, repeats) in [(12usize, 6000), (1200, 12)] {
        let mut working_days = (start + 1..).filter(|d| (1..=5).contains(&((d + 6) % 7)));
        let dates = working_days.by_ref().take(count).map(|d| Value::Number(f64::from(d))).collect();
        let holidays = Arg::reference(Array::new(count, 1, dates).ok_or("invalid holiday range")?);
        let next = working_days.next().ok_or("missing following workday")?;
        let forward = [Arg::val(f64::from(start)), Arg::val(1.0), holidays.clone()];
        let backward = [Arg::val(f64::from(next)), Arg::val(-1.0), holidays];
        let timed = Instant::now();
        for _ in 0..repeats {
            if call(workday, black_box(&forward), &mut context) != Value::Number(f64::from(next))
                || call(workday, black_box(&backward), &mut context) != Value::Number(f64::from(start))
            {
                return Err("holiday selection differs");
            }
        }
        println!("{count},{repeats},{}", timed.elapsed().as_nanos());
    }
    Ok(())
}
