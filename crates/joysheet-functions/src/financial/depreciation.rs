//! First-party asset depreciation functions (`SLN`, `SYD`, `DB`, `DDB`, `VDB`,
//! `AMORLINC`, `AMORDEGRC`).
//!
//! `VDB` runs as an `O(1)`-memory single-pass streaming accumulator across
//! `[start_period, end_period]` instead of materializing an intermediate
//! `Vec<f64>` schedule (avoiding up to 80 MiB of temporary heap allocation on
//! large-life inputs).

use super::day_count::{basis_arg, date, year_frac};
use super::*;

const MAX_DB_PERIOD: f64 = 1e7;
const MAX_VDB_PERIODS: f64 = 1e7;
const MAX_AMOR_PERIOD: f64 = 1e6;

pub(super) fn sln(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    let cost = num(args, 0)?;
    let salvage = num(args, 1)?;
    let life = num(args, 2)?;
    if life == 0.0 {
        return Err(CellError::Div0);
    }
    num_val((cost - salvage) / life)
}

pub(super) fn syd(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    let cost = num(args, 0)?;
    let salvage = num(args, 1)?;
    let life = num(args, 2)?;
    let per = num(args, 3)?;
    if life <= 0.0 || per <= 0.0 || per > life || salvage < 0.0 {
        return Err(CellError::Num);
    }
    num_val((cost - salvage) * (life - per + 1.0) * 2.0 / (life * (life + 1.0)))
}

pub(super) fn db(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    let cost = num(args, 0)?;
    let salvage = num(args, 1)?;
    let life = num(args, 2)?;
    let period = num(args, 3)?.trunc();
    let month = opt_num(args, 4, 12.0)?.trunc();
    if cost < 0.0 || salvage < 0.0 || life <= 0.0 || period <= 0.0 || !(1.0..=12.0).contains(&month) {
        return Err(CellError::Num);
    }
    let life_int = life.trunc();
    let max_period = if month == 12.0 { life_int } else { life_int + 1.0 };
    if period > max_period.max(1.0) || period > MAX_DB_PERIOD {
        return Err(CellError::Num);
    }
    if cost == 0.0 {
        return num_val(0.0);
    }
    let rate = crate::util::round_half_away(1.0 - (salvage / cost).powf(1.0 / life), 3);
    let mut accumulated = 0.0;
    let mut current = 0.0;
    let mut p = 1.0;
    while p <= period {
        current = if p == 1.0 {
            cost * rate * (month / 12.0)
        } else if p == life_int + 1.0 {
            (cost - accumulated) * rate * ((12.0 - month) / 12.0)
        } else {
            (cost - accumulated) * rate
        };
        accumulated += current;
        p += 1.0;
    }
    num_val(current)
}

pub(super) fn ddb(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    let cost = num(args, 0)?;
    let salvage = num(args, 1)?;
    let life = num(args, 2)?;
    let period = num(args, 3)?;
    let factor = opt_num(args, 4, 2.0)?;
    if cost < 0.0 || salvage < 0.0 || life <= 0.0 || period <= 0.0 || factor <= 0.0 || period > life {
        return Err(CellError::Num);
    }
    let rate = (factor / life).min(1.0);
    let prior_book = if rate >= 1.0 { if period == 1.0 { cost } else { 0.0 } } else { cost * (1.0 - rate).powf(period - 1.0) };
    let next_book = cost * (1.0 - rate).powf(period);
    let dep = if next_book < salvage { prior_book - salvage } else { prior_book - next_book };
    num_val(dep.max(0.0))
}

/// Streams declining-balance depreciation with optional straight-line switch,
/// accumulating only the overlap with `[start, end]` in `O(1)` space.
fn vdb_accumulate(cost: f64, salvage: f64, life: f64, start: f64, end: f64, factor: f64, no_switch: bool, periods: usize) -> f64 {
    let mut book = cost;
    let mut straight_line: Option<f64> = None;
    let mut total = 0.0;
    for idx in 0..periods {
        let room = (book - salvage).max(0.0);
        if room == 0.0 {
            break;
        }
        let p0 = idx as f64;
        let dep = if let Some(sl) = straight_line {
            sl.min(room)
        } else {
            let db_step = (book * factor / life).min(room);
            let remaining = life - p0;
            let sl_step = if remaining > 0.0 { (book - salvage) / remaining.max(1.0) } else { 0.0 };
            if !no_switch && sl_step > db_step {
                straight_line = Some(sl_step);
                sl_step.min(room)
            } else {
                db_step
            }
        };
        let p1 = p0 + 1.0;
        if p1 > start {
            let overlap = (p1.min(end) - p0.max(start)).max(0.0);
            total += dep * overlap;
        }
        book -= dep;
    }
    total
}

pub(super) fn vdb(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    let cost = num(args, 0)?;
    let salvage = num(args, 1)?;
    let life = num(args, 2)?;
    let start = num(args, 3)?;
    let end = num(args, 4)?;
    let factor = opt_num(args, 5, 2.0)?;
    let no_switch = opt_bool(args, 6, false)?;
    if cost < 0.0 || salvage < 0.0 || life <= 0.0 || start < 0.0 || end < start || end > life || factor <= 0.0 {
        return Err(CellError::Num);
    }
    let periods = end.ceil();
    if periods > MAX_VDB_PERIODS {
        return Err(CellError::Num);
    }
    num_val(vdb_accumulate(cost, salvage, life, start, end, factor, no_switch, periods as usize))
}

pub(super) fn amorlinc(args: &[Arg], ctx: &mut dyn Ctx) -> R<Value> {
    let cost = num(args, 0)?;
    let purchased = date(args, 1)?;
    let first_period = date(args, 2)?;
    let salvage = num(args, 3)?;
    let period = num(args, 4)?.trunc();
    let rate = num(args, 5)?;
    let basis = basis_arg(args, 6)?;
    if cost < 0.0 || salvage < 0.0 || salvage > cost || period < 0.0 || rate <= 0.0 || first_period < purchased {
        return Err(CellError::Num);
    }
    let full = cost * rate;
    let depreciable = cost - salvage;
    let first_dep = (full * year_frac(ctx.date_system(), purchased, first_period, basis)?).min(depreciable);
    if period == 0.0 {
        return num_val(first_dep);
    }
    let remaining = depreciable - first_dep - (period - 1.0) * full;
    num_val(full.min(remaining.max(0.0)))
}

fn amordegrc_coefficient(rate: f64) -> f64 {
    let life = 1.0 / rate;
    if life < 3.0 {
        1.0
    } else if life < 5.0 {
        1.5
    } else if life <= 6.0 {
        2.0
    } else {
        2.5
    }
}

pub(super) fn amordegrc(args: &[Arg], ctx: &mut dyn Ctx) -> R<Value> {
    let cost = num(args, 0)?;
    let purchased = date(args, 1)?;
    let first_period = date(args, 2)?;
    let salvage = num(args, 3)?;
    let period = num(args, 4)?.trunc();
    let rate = num(args, 5)?;
    let basis = basis_arg(args, 6)?;
    if cost < 0.0 || salvage < 0.0 || salvage > cost || period < 0.0 || rate <= 0.0 || first_period < purchased || period > MAX_AMOR_PERIOD {
        return Err(CellError::Num);
    }
    let effective_rate = rate * amordegrc_coefficient(rate);
    let mut book = cost;
    let mut dep = (year_frac(ctx.date_system(), purchased, first_period, basis)? * effective_rate * cost).round();
    if period == 0.0 {
        return num_val(dep);
    }
    book -= dep;
    let mut remaining = book - salvage;
    let mut step = 1.0;
    while step <= period {
        dep = (effective_rate * book).round();
        remaining -= dep;
        if remaining < 0.0 {
            let tail = period - step;
            return num_val(if tail <= 1.0 { (book * 0.5).round() } else { 0.0 });
        }
        book -= dep;
        step += 1.0;
    }
    num_val(dep)
}
