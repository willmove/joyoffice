//! Periodic payments use their order in the range. Dated payments discount by
//! actual whole days over a 365-day year, including for leap years.
use super::{Arg, CellError, Ctx, DateSystem, R, Value, max_serial, num, num_val, opt_num};
use crate::util::{array_numbers, visit_numbers};

fn cells(value: &Value) -> &[Value] {
    match value {
        Value::Array(grid) => &grid.data,
        scalar => std::slice::from_ref(scalar),
    }
}

pub(super) fn npv(args: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    let factor = 1.0 + num(args, 0)?;
    if factor == 0.0 {
        return Err(CellError::Div0);
    }
    let mut divisor = 1.0;
    let mut total = 0.0;
    visit_numbers(args.get(1..).ok_or(CellError::Value)?, |amount| {
        divisor *= factor;
        total += amount / divisor;
    })?;
    num_val(total)
}

pub(super) fn mirr(args: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    let financing = 1.0 + num(args, 1)?;
    let reinvestment = 1.0 + num(args, 2)?;
    let mut divisor = 1.0;
    let mut paid = 0.0;
    let mut received = 0.0;
    let mut periods = 0usize;
    // Grow receipts to the last period and discount payments to the first.
    // A skipped range entry consumes no period; a numeric zero does.
    visit_numbers(args.get(..1).ok_or(CellError::Value)?, |amount| {
        received = received * reinvestment + amount.max(0.0);
        if amount < 0.0 {
            paid -= amount / divisor;
        }
        divisor *= financing;
        periods += 1;
    })?;
    if periods < 2 || paid == 0.0 || !paid.is_finite() || reinvestment == 0.0 {
        return Err(CellError::Div0);
    }
    num_val((received / paid).powf(1.0 / (periods - 1) as f64) - 1.0)
}

/// Illinois interpolation retains a bracket while avoiding repeated full
/// scans of long schedules when one endpoint stays fixed for several steps.
fn refine(mut a: f64, mut fa: f64, mut b: f64, mut fb: f64, evaluate: &impl Fn(f64) -> f64) -> R<f64> {
    let mut previous_side = None;
    for _ in 0..100 {
        let mid = a + (b - a) * 0.5;
        if (b - a).abs() <= 2e-13 * mid.abs().max(1.0) {
            return Ok(mid);
        }
        let secant = a - (b - a) * (fa / (fb - fa));
        let x = if secant > a.min(b) && secant < a.max(b) { secant } else { mid };
        let y = evaluate(x);
        if !y.is_finite() {
            return Err(CellError::Num);
        }
        if y == 0.0 {
            return Ok(x);
        }
        let moved_a = y.is_sign_negative() == fa.is_sign_negative();
        if moved_a {
            (a, fa) = (x, y);
            if previous_side == Some(true) {
                fb *= 0.5;
            }
        } else {
            (b, fb) = (x, y);
            if previous_side == Some(false) {
                fa *= 0.5;
            }
        }
        previous_side = Some(moved_a);
    }
    Err(CellError::Num)
}

/// A bounded search in the direction of the return from the undiscounted
/// balance. Preserve the guess's domain for dated cash flows. Discounted
/// signs are invariant under a common change of units or valuation date.
fn bracketed_return(first: f64, balance: f64, guess: f64, evaluate: impl Fn(f64) -> f64) -> R<f64> {
    if guess <= -1.0 {
        return Err(CellError::Num);
    }
    if balance == 0.0 {
        return Ok(0.0);
    }
    let positive_return = first.is_sign_negative() != balance.is_sign_negative();
    if positive_return && guess < 0.0 {
        return Err(CellError::Num);
    }
    let mut inside = 0.0;
    let mut previous = balance;
    let step = guess.abs().max(0.01);
    let mut outside = if positive_return { step } else { -step.min(0.5) };
    for _ in 0..100 {
        let value = evaluate(outside);
        if value == 0.0 {
            return Ok(outside);
        }
        if !value.is_finite() {
            break;
        }
        if value.is_sign_negative() != previous.is_sign_negative() {
            return refine(inside, previous, outside, value, &evaluate);
        }
        inside = outside;
        previous = value;
        outside = if positive_return { outside * 2.0 } else { (outside * 2.0).max((outside - 1.0) * 0.5) };
        if outside <= -1.0 || outside == inside {
            break;
        }
    }
    Err(CellError::Num)
}

/// Discount from the earliest payment for positive rates and from the last
/// for negative rates. The common scale cancels in a Newton step. Accumulate
/// both the balance and its derivative with respect to log(1 + rate).
fn periodic_balance(values: &[f64], log_rate: f64) -> (f64, f64) {
    let mut total = 0.0;
    let mut slope = 0.0;
    let mut power = 1.0;
    let factor = (-log_rate.abs()).exp();
    let mut add = |(period, amount): (usize, &f64)| {
        let discounted = amount * power;
        total += discounted;
        slope -= period as f64 * discounted;
        power *= factor;
    };
    if log_rate < 0.0 {
        values.iter().enumerate().rev().for_each(&mut add);
    } else {
        values.iter().enumerate().for_each(&mut add);
    }
    (total, slope)
}

pub(super) fn irr(args: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    let mut flows = array_numbers(&args.first().ok_or(CellError::Value)?.value).map_err(|_| CellError::Value)?;
    let guess = opt_num(args, 1, 0.1)?;
    if !flows.iter().any(|&v| v > 0.0) || !flows.iter().any(|&v| v < 0.0) || guess <= -1.0 {
        return Err(CellError::Num);
    }
    let scale = flows.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
    let first = flows.iter().position(|&n| n != 0.0).ok_or(CellError::Num)?;
    let last = flows.iter().rposition(|&n| n != 0.0).ok_or(CellError::Num)?;
    let flows = flows.get_mut(first..=last).ok_or(CellError::Num)?;
    for flow in &mut *flows {
        *flow /= scale;
    }
    let mut log_rate = guess.ln_1p();
    for _ in 0..40 {
        let (balance, slope) = periodic_balance(flows, log_rate);
        if balance == 0.0 {
            return num_val(log_rate.exp_m1());
        }
        let step = balance / slope;
        if !step.is_finite() || step.abs() > 5.0 {
            break;
        }
        log_rate -= step;
        if step.abs() <= 1e-13 {
            return num_val(log_rate.exp_m1());
        }
    }
    let first = flows.first().copied().ok_or(CellError::Num)?;
    num_val(bracketed_return(first, flows.iter().sum(), guess.abs(), |r| periodic_balance(flows, r.ln_1p()).0)?)
}

fn paired(args: &[Arg], index: usize) -> R<(&[Value], &[Value])> {
    let amounts = cells(&args.get(index).ok_or(CellError::Value)?.value);
    let dates = cells(&args.get(index + 1).ok_or(CellError::Value)?.value);
    if amounts.len() != dates.len() || amounts.is_empty() {
        return Err(CellError::Num);
    }
    Ok((amounts, dates))
}

fn dated_number(cell: &Value) -> R<f64> {
    match cell {
        Value::Number(n) => Ok(*n),
        Value::Error(e) => Err(*e),
        _ => Err(CellError::Num),
    }
}

pub(super) fn xnpv(args: &[Arg], ctx: &mut dyn Ctx) -> R<Value> {
    let rate = num(args, 0)?;
    if rate <= 0.0 {
        return Err(CellError::Num);
    }
    let (amounts, dates) = paired(args, 1)?;
    let first = dated_number(dates.first().ok_or(CellError::Num)?)?.trunc();
    let max = max_serial(ctx.date_system()) as f64;
    if first < 1.0 || first > max {
        return Err(CellError::Num);
    }
    let log_rate = rate.ln_1p();
    let mut total = 0.0;
    for (cell, day) in amounts.iter().zip(dates) {
        let amount = dated_number(cell)?;
        let date = dated_number(day)?.trunc();
        if date < first || date > max {
            return Err(CellError::Num);
        }
        total += amount * (-log_rate * ((date - first) / 365.0)).exp();
    }
    num_val(total)
}

fn rate_number(cell: &Value, system: DateSystem) -> R<f64> {
    if matches!(cell, Value::Bool(_)) {
        return Err(CellError::Value);
    }
    cell.to_number_in(system)
}

pub(super) fn xirr(args: &[Arg], ctx: &mut dyn Ctx) -> R<Value> {
    let (amounts, dates) = paired(args, 0)?;
    if amounts.len() < 2 {
        return Err(CellError::NA);
    }
    let guess = opt_num(args, 2, 0.1)?;
    let system = ctx.date_system();
    let limit = max_serial(system) as f64;
    let mut earliest = limit;
    let mut latest = 0.0_f64;
    let mut flows = Vec::with_capacity(amounts.len());
    let mut scale = 0.0_f64;
    for (amount, day) in amounts.iter().zip(dates) {
        let amount = rate_number(amount, system)?;
        let day = rate_number(day, system)?.trunc();
        if !(0.0..=limit).contains(&day) {
            return Err(CellError::Num);
        }
        if amount != 0.0 {
            earliest = earliest.min(day);
            latest = latest.max(day);
            scale = scale.max(amount.abs());
        }
        flows.push((amount, day));
    }
    if earliest == latest || !flows.iter().any(|(n, _)| *n > 0.0) || !flows.iter().any(|(n, _)| *n < 0.0) {
        return Err(CellError::Num);
    }
    let first = flows.first().ok_or(CellError::Num)?.0;
    // Excel treats a schedule with a zero initial outlay as a zero return
    // for a nonnegative initial guess. Preserve that ambiguity on exchange.
    if first == 0.0 {
        return if guess < 0.0 { Err(CellError::Num) } else { num_val(0.0) };
    }
    flows.retain(|(amount, _)| *amount != 0.0);
    for (amount, _) in &mut flows {
        *amount /= scale;
    }
    let balance = flows.iter().map(|(amount, _)| amount).sum();
    num_val(bracketed_return(first, balance, guess, |r| {
        let origin = if r < 0.0 { latest } else { earliest };
        let log_rate = r.ln_1p();
        flows.iter().map(|(n, t)| n * (-log_rate * ((t - origin) / 365.0)).exp()).sum()
    })?)
}
