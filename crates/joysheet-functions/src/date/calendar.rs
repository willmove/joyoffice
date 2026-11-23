//! First-party calendar date/time arithmetic and day-count functions:
//! `DATE`, `TIME`, `YEAR`, `MONTH`, `DAY`, `HOUR`, `MINUTE`, `SECOND`, `NOW`,
//! `TODAY`, `WEEKDAY`, `WEEKNUM`, `ISOWEEKNUM`, `EDATE`, `EOMONTH`, `DATEDIF`,
//! `DATEVALUE`, `TIMEVALUE`, `DAYS`, `DAYS360`, and `YEARFRAC`.

use joysheet_core::date::{DateTime, datetime_from_serial, days_in_month, is_leap, max_serial, serial_from_ymd, serial_in_system};
use joysheet_core::parse::parse_input;
use joysheet_core::{CellError, DateSystem, Value};

use crate::util::{R, arg, has, num, num_val, opt_bool, opt_num, scalar, text, to_int};
use crate::{Arg, Ctx};

fn serial_arg(a: &[Arg], i: usize, sys: DateSystem) -> R<f64> {
    let n = scalar(arg(a, i)?).to_number_in(sys)?;
    if !(0.0..(max_serial(sys) + 1) as f64).contains(&n) {
        return Err(CellError::Num);
    }
    Ok(n)
}

fn day_arg(a: &[Arg], i: usize, sys: DateSystem) -> R<i64> {
    Ok(serial_arg(a, i, sys)?.floor() as i64)
}

fn dt(sys: DateSystem, serial: f64) -> R<DateTime> {
    datetime_from_serial(sys, serial).ok_or(CellError::Num)
}

fn ymd_serial(sys: DateSystem, y: i64, m: i64, d: i64) -> R<i64> {
    serial_from_ymd(sys, y, m, d).map(|s| s as i64).ok_or(CellError::Num)
}

/// Day of week for `serial` with 0 = Sunday .. 6 = Saturday.
pub(super) fn weekday0(sys: DateSystem, serial: i64) -> usize {
    match sys {
        DateSystem::D1900 => (serial + 6).rem_euclid(7) as usize,
        DateSystem::D1904 => (serial + 5).rem_euclid(7) as usize,
    }
}

pub(super) fn date(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let y = to_int(num(a, 0)?)?;
    let m = to_int(num(a, 1)?)?;
    let d = to_int(num(a, 2)?)?;
    if !(0..10000).contains(&y) || m.abs() > 1_000_000 || d.abs() > 100_000_000 {
        return Err(CellError::Num);
    }
    num_val(ymd_serial(c.date_system(), y, m, d)? as f64)
}

pub(super) fn time(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let h = num(a, 0)?.trunc();
    let m = num(a, 1)?.trunc();
    let s = num(a, 2)?.trunc();
    if h > 32767.0 || m > 32767.0 || s > 32767.0 {
        return Err(CellError::Num);
    }
    let total = h * 3600.0 + m * 60.0 + s;
    if total < 0.0 {
        return Err(CellError::Num);
    }
    num_val(total.rem_euclid(86400.0) / 86400.0)
}

fn extract_part(a: &[Arg], c: &mut dyn Ctx, f: fn(&DateTime) -> f64) -> R<Value> {
    let sys = c.date_system();
    let s = serial_arg(a, 0, sys)?;
    num_val(f(&dt(sys, s.floor())?))
}

pub(super) fn year(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    extract_part(a, c, |d| d.year as f64)
}

pub(super) fn month(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    extract_part(a, c, |d| d.month as f64)
}

pub(super) fn day(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    extract_part(a, c, |d| d.day as f64)
}

fn day_seconds(a: &[Arg], sys: DateSystem) -> R<i64> {
    let s = serial_arg(a, 0, sys)?;
    let secs = ((s - s.floor()) * 86400.0).round() as i64;
    Ok(if secs >= 86400 { 0 } else { secs })
}

pub(super) fn hour(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    num_val((day_seconds(a, c.date_system())? / 3600) as f64)
}

pub(super) fn minute(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    num_val((day_seconds(a, c.date_system())? / 60 % 60) as f64)
}

pub(super) fn second(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    num_val((day_seconds(a, c.date_system())? % 60) as f64)
}

pub(super) fn now(_a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    num_val(serial_in_system(c.date_system(), c.now_serial()))
}

pub(super) fn today(_a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    num_val(serial_in_system(c.date_system(), c.now_serial()).floor())
}

pub(super) fn weekday(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let sys = c.date_system();
    let s = day_arg(a, 0, sys)?;
    let t = to_int(opt_num(a, 1, 1.0)?)?;
    let wd = weekday0(sys, s) as i64;
    let r = match t {
        1 | 17 => wd + 1,
        2 | 11 => (wd + 6) % 7 + 1,
        3 => (wd + 6) % 7,
        12..=16 => (wd + 7 - (t - 10)) % 7 + 1,
        _ => return Err(CellError::Num),
    };
    num_val(r as f64)
}

fn isoweek(sys: DateSystem, s: i64) -> R<i64> {
    let iso_wd = (weekday0(sys, s) + 6) % 7 + 1;
    let thursday = s - iso_wd as i64 + 4;
    let first = match sys {
        DateSystem::D1900 => 1,
        DateSystem::D1904 => 0,
    };
    let jan1 = if thursday < first { first - 365 } else { ymd_serial(sys, dt(sys, thursday as f64)?.year as i64, 1, 1)? };
    Ok((thursday - jan1).div_euclid(7) + 1)
}

pub(super) fn weeknum(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let sys = c.date_system();
    let s = day_arg(a, 0, sys)?;
    let t = to_int(opt_num(a, 1, 1.0)?)?;
    let start = match t {
        1 | 17 => 0,
        2 | 11 => 1,
        12..=16 => t - 10,
        21 => return num_val(isoweek(sys, s)? as f64),
        _ => return Err(CellError::Num),
    } as usize;
    let y = dt(sys, s as f64)?.year as i64;
    let jan1 = ymd_serial(sys, y, 1, 1).unwrap_or(s.min(1));
    let offset = (weekday0(sys, jan1) + 7 - start) % 7;
    num_val(((s - jan1 + offset as i64).div_euclid(7) + 1) as f64)
}

pub(super) fn isoweeknum(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let sys = c.date_system();
    num_val(isoweek(sys, day_arg(a, 0, sys)?)? as f64)
}

fn shift_months(a: &[Arg], c: &mut dyn Ctx) -> R<(DateSystem, DateTime, i64, i64)> {
    let sys = c.date_system();
    let d = dt(sys, day_arg(a, 0, sys)? as f64)?;
    let m = to_int(num(a, 1)?)?;
    if m.abs() > 200_000 {
        return Err(CellError::Num);
    }
    let total = d.year as i64 * 12 + d.month as i64 - 1 + m;
    let (y, mo) = (total.div_euclid(12), total.rem_euclid(12) + 1);
    if !(1900..=9999).contains(&y) {
        return Err(CellError::Num);
    }
    Ok((sys, d, y, mo))
}

pub(super) fn edate(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let (sys, d, y, mo) = shift_months(a, c)?;
    let dd = (d.day as i64).min(days_in_month(y as i32, mo as u32) as i64);
    num_val(ymd_serial(sys, y, mo, dd)? as f64)
}

pub(super) fn eomonth(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let (sys, _, y, mo) = shift_months(a, c)?;
    num_val(ymd_serial(sys, y, mo, days_in_month(y as i32, mo as u32) as i64)? as f64)
}

pub(super) fn datedif(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let sys = c.date_system();
    let s = day_arg(a, 0, sys)?;
    let e = day_arg(a, 1, sys)?;
    let unit = text(a, 2)?.to_ascii_uppercase();
    if s > e {
        return Err(CellError::Num);
    }
    let d1 = dt(sys, s as f64)?;
    let d2 = dt(sys, e as f64)?;
    let months = (d2.year as i64 - d1.year as i64) * 12 + d2.month as i64 - d1.month as i64 - i64::from(d2.day < d1.day);
    let r = match unit.as_str() {
        "D" => e - s,
        "M" => months,
        "Y" => months / 12,
        "YM" => months % 12,
        "MD" => {
            if d2.day >= d1.day {
                (d2.day - d1.day) as i64
            } else {
                let (py, pm) = if d2.month == 1 { (d2.year - 1, 12) } else { (d2.year, d2.month - 1) };
                days_in_month(py, pm) as i64 - d1.day as i64 + d2.day as i64
            }
        }
        "YD" => {
            let later = (d1.month, d1.day) > (d2.month, d2.day);
            let y = if later { d2.year - 1 } else { d2.year };
            let dd = d1.day.min(days_in_month(y, d1.month).max(1));
            let shifted = ymd_serial(sys, y as i64, d1.month as i64, dd as i64)?;
            e - shifted
        }
        _ => return Err(CellError::Num),
    };
    num_val(r as f64)
}

fn parsed_text(a: &[Arg], c: &mut dyn Ctx) -> R<(f64, &'static str)> {
    let v = scalar(arg(a, 0)?);
    let Value::Text(t) = v else {
        return Err(match v {
            Value::Error(e) => e,
            _ => CellError::Value,
        });
    };
    let p = parse_input(&t, c.date_system());
    match (p.value, p.format) {
        (Value::Number(n), Some(f)) if f.contains(['d', 'y', 'h']) => Ok((n, f)),
        _ => Err(CellError::Value),
    }
}

pub(super) fn datevalue(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let (n, f) = parsed_text(a, c)?;
    if !f.contains(['d', 'y']) {
        return Err(CellError::Value);
    }
    num_val(n.floor())
}

pub(super) fn timevalue(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let (n, _) = parsed_text(a, c)?;
    num_val(n - n.floor())
}

pub(super) fn days(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let sys = c.date_system();
    num_val((day_arg(a, 0, sys)? - day_arg(a, 1, sys)?) as f64)
}

fn last_of_feb(d: &DateTime) -> bool {
    d.month == 2 && d.day == days_in_month(d.year, 2)
}

fn days360_core(d1: &DateTime, d2: &DateTime, european: bool) -> i64 {
    let (mut a, mut b) = (d1.day as i64, d2.day as i64);
    if european {
        a = a.min(30);
        b = b.min(30);
    } else {
        if a == 31 || last_of_feb(d1) {
            a = 30;
        }
        if b == 31 && a >= 30 {
            b = 30;
        }
    }
    (d2.year as i64 - d1.year as i64) * 360 + (d2.month as i64 - d1.month as i64) * 30 + b - a
}

pub(super) fn days360(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let sys = c.date_system();
    let d1 = dt(sys, day_arg(a, 0, sys)? as f64)?;
    let d2 = dt(sys, day_arg(a, 1, sys)? as f64)?;
    let european = opt_bool(a, 2, false)?;
    num_val(days360_core(&d1, &d2, european) as f64)
}

fn feb29_between(sys: DateSystem, s: i64, e: i64, d1: &DateTime, d2: &DateTime) -> bool {
    (d1.year..=d2.year).any(|y| is_leap(y) && serial_from_ymd(sys, y as i64, 2, 29).is_some_and(|f| (f as i64) >= s && (f as i64) <= e))
}

/// Day-count year fraction for bases `0` (US 30/360), `1` (Actual/Actual),
/// `2` (Actual/360), `3` (Actual/365), and `4` (European 30/360).
fn yearfrac_core(sys: DateSystem, s: i64, e: i64, basis: i64) -> R<f64> {
    let (s, e) = if s <= e { (s, e) } else { (e, s) };
    let d1 = dt(sys, s as f64)?;
    let d2 = dt(sys, e as f64)?;
    let days = (e - s) as f64;
    Ok(match basis {
        0 => {
            let (mut a, mut b) = (d1.day as i64, d2.day as i64);
            if last_of_feb(&d1) && last_of_feb(&d2) {
                b = 30;
            }
            if last_of_feb(&d1) || a == 31 {
                a = 30;
            }
            if a == 30 && b == 31 {
                b = 30;
            }
            ((d2.year as i64 - d1.year as i64) * 360 + (d2.month as i64 - d1.month as i64) * 30 + b - a) as f64 / 360.0
        }
        1 => {
            let within_year = d1.year == d2.year || (d2.year == d1.year + 1 && (d1.month > d2.month || (d1.month == d2.month && d1.day >= d2.day)));
            if within_year {
                let year_len = if (d1.year == d2.year && is_leap(d1.year)) || feb29_between(sys, s, e, &d1, &d2) || (d2.month == 2 && d2.day == 29) {
                    366.0
                } else {
                    365.0
                };
                days / year_len
            } else {
                let years = (d2.year - d1.year + 1) as f64;
                let span = ymd_serial(sys, d2.year as i64 + 1, 1, 1).unwrap_or(e + 1) - ymd_serial(sys, d1.year as i64, 1, 1).unwrap_or(s);
                days / (span as f64 / years)
            }
        }
        2 => days / 360.0,
        3 => days / 365.0,
        4 => days360_core(&d1, &d2, true) as f64 / 360.0,
        _ => return Err(CellError::Num),
    })
}

pub(super) fn yearfrac(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let sys = c.date_system();
    let s = day_arg(a, 0, sys)?;
    let e = day_arg(a, 1, sys)?;
    let basis = if has(a, 2) { to_int(num(a, 2)?)? } else { 0 };
    num_val(yearfrac_core(sys, s, e, basis)?)
}
