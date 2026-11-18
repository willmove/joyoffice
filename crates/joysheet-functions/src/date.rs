//! Date and time functions.

use joysheet_core::date::{DateTime, datetime_from_serial, days_in_month, is_leap, serial_from_ymd, serial_in_system};
use joysheet_core::parse::parse_input;
use joysheet_core::{CellError, DateSystem, Value};

use crate::util::{R, S, arg, has, num, num_val, opt_bool, opt_num, scalar, text, to_int};
use crate::{Arg, Ctx, FnSpec};

mod workweek;
use workweek::{CalendarDates, Workweek};

/// NETWORKDAYS / WORKDAY: start, end/days scalar; holidays take arrays.
const ND: &[bool] = &[true, true, false];
const NDI: &[bool] = &[true, true, true, false];

/// A date serial, including its fractional day, in the workbook's date system.
fn serial_arg(a: &[Arg], i: usize, sys: DateSystem) -> R<f64> {
    let n = scalar(arg(a, i)?).to_number_in(sys)?;
    if !(0.0..(joysheet_core::date::max_serial(sys) + 1) as f64).contains(&n) {
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

/// Weekday of a serial, 0 = Sunday.
fn weekday0(sys: DateSystem, serial: i64) -> usize {
    match sys {
        DateSystem::D1900 => (serial + 6).rem_euclid(7) as usize,
        DateSystem::D1904 => (serial + 5).rem_euclid(7) as usize,
    }
}

fn date(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let y = to_int(num(a, 0)?)?;
    let m = to_int(num(a, 1)?)?;
    let d = to_int(num(a, 2)?)?;
    if !(0..10000).contains(&y) || m.abs() > 1_000_000 || d.abs() > 100_000_000 {
        return Err(CellError::Num);
    }
    num_val(ymd_serial(c.date_system(), y, m, d)? as f64)
}

fn time(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
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

fn part(a: &[Arg], c: &mut dyn Ctx, f: fn(&DateTime) -> f64) -> R<Value> {
    let sys = c.date_system();
    let s = serial_arg(a, 0, sys)?;
    num_val(f(&dt(sys, s.floor())?))
}

fn year(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    part(a, c, |d| d.year as f64)
}
fn month(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    part(a, c, |d| d.month as f64)
}
fn day(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    part(a, c, |d| d.day as f64)
}

/// Seconds into the day, rounded to the nearest second.
fn day_seconds(a: &[Arg], sys: DateSystem) -> R<i64> {
    let s = serial_arg(a, 0, sys)?;
    let secs = ((s - s.floor()) * 86400.0).round() as i64;
    Ok(if secs >= 86400 { 0 } else { secs })
}

fn hour(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    num_val((day_seconds(a, c.date_system())? / 3600) as f64)
}
fn minute(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    num_val((day_seconds(a, c.date_system())? / 60 % 60) as f64)
}
fn second(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    num_val((day_seconds(a, c.date_system())? % 60) as f64)
}

fn now(_a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    num_val(serial_in_system(c.date_system(), c.now_serial()))
}
fn today(_a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    num_val(serial_in_system(c.date_system(), c.now_serial()).floor())
}

fn weekday(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
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
    let iso_wd = (weekday0(sys, s) + 6) % 7 + 1; // Monday = 1
    let thursday = s - iso_wd as i64 + 4;
    let first = match sys {
        DateSystem::D1900 => 1,
        DateSystem::D1904 => 0,
    };
    let jan1 = if thursday < first {
        // The week belongs to the year before the calendar starts.
        first - 365
    } else {
        ymd_serial(sys, dt(sys, thursday as f64)?.year as i64, 1, 1)?
    };
    Ok((thursday - jan1).div_euclid(7) + 1)
}

fn weeknum(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
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

fn isoweeknum(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let sys = c.date_system();
    num_val(isoweek(sys, day_arg(a, 0, sys)?)? as f64)
}

fn months_arg(a: &[Arg], i: usize) -> R<i64> {
    let m = to_int(num(a, i)?)?;
    if m.abs() > 200_000 {
        return Err(CellError::Num);
    }
    Ok(m)
}

fn edate(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let sys = c.date_system();
    let d = dt(sys, day_arg(a, 0, sys)? as f64)?;
    let m = months_arg(a, 1)?;
    let total = d.year as i64 * 12 + d.month as i64 - 1 + m;
    let (y, mo) = (total.div_euclid(12), total.rem_euclid(12) + 1);
    if !(1900..=9999).contains(&y) {
        return Err(CellError::Num);
    }
    // Excel keeps serial 0's day 0 and gives February 1900 its real 28 days here, though the
    // sheet's calendar has a 29 February 1900 (serial 60).
    let dd = (d.day as i64).min(days_in_month(y as i32, mo as u32) as i64);
    num_val(ymd_serial(sys, y, mo, dd)? as f64)
}

fn eomonth(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let sys = c.date_system();
    let d = dt(sys, day_arg(a, 0, sys)? as f64)?;
    let m = months_arg(a, 1)?;
    let total = d.year as i64 * 12 + d.month as i64 - 1 + m;
    let (y, mo) = (total.div_euclid(12), total.rem_euclid(12) + 1);
    if !(1900..=9999).contains(&y) {
        return Err(CellError::Num);
    }
    // Like EDATE, February 1900 ends on the 28th (serial 59).
    num_val(ymd_serial(sys, y, mo, days_in_month(y as i32, mo as u32) as i64)? as f64)
}

fn datedif(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
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

fn datevalue(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let (n, f) = parsed_text(a, c)?;
    if !f.contains(['d', 'y']) {
        return Err(CellError::Value);
    }
    num_val(n.floor())
}

fn timevalue(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let (n, _) = parsed_text(a, c)?;
    num_val(n - n.floor())
}

fn days(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let sys = c.date_system();
    num_val((day_arg(a, 0, sys)? - day_arg(a, 1, sys)?) as f64)
}

fn last_of_feb(d: &DateTime) -> bool {
    d.month == 2 && d.day == days_in_month(d.year, 2)
}

fn days360(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let sys = c.date_system();
    let d1 = dt(sys, day_arg(a, 0, sys)? as f64)?;
    let d2 = dt(sys, day_arg(a, 1, sys)? as f64)?;
    let european = opt_bool(a, 2, false)?;
    num_val(days360_core(&d1, &d2, european) as f64)
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

/// Weekend mask indexed by weekday (0 = Sunday).
fn weekend_mask(v: Option<&Arg>, error_argument: CellError) -> R<[bool; 7]> {
    let Some(arg) = v else { return Ok([true, false, false, false, false, false, true]) };
    match scalar(arg) {
        Value::Empty if !arg.from_ref => Ok([true, false, false, false, false, false, true]),
        Value::Text(t) => {
            let b = t.as_bytes();
            // All-weekend calendars can count days but cannot select a working day.
            if b.len() != 7 || !b.iter().all(|c| *c == b'0' || *c == b'1') {
                return Err(CellError::Value);
            }
            // String order is Monday..Sunday.
            let mut m = [false; 7];
            for (i, c) in b.iter().enumerate() {
                m[(i + 1) % 7] = *c == b'1';
            }
            Ok(m)
        }
        Value::Error(_) => Err(error_argument),
        other => {
            let k = to_int(other.to_number()?)?;
            let mut m = [false; 7];
            match k {
                1..=7 => {
                    m[((k + 5) % 7) as usize] = true;
                    m[((k + 6) % 7) as usize] = true;
                }
                11..=17 => m[(k - 11) as usize] = true,
                _ => return Err(CellError::Num),
            }
            Ok(m)
        }
    }
}

fn networkdays_core(a: &[Arg], c: &mut dyn Ctx, international: bool) -> R<Value> {
    let dates = CalendarDates::new(c);
    let start = dates.day(a, 0)?;
    let end = dates.day(a, 1)?;
    let weekend = weekend_mask(if international { a.get(2) } else { None }, CellError::Value)?;
    let calendar = Workweek::new(dates, weekend, a.get(if international { 3 } else { 2 }), dates.origin())?;
    num_val(calendar.inclusive(start, end) as f64)
}

fn networkdays(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    networkdays_core(a, c, false)
}
fn networkdays_intl(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    networkdays_core(a, c, true)
}

fn workday_core(a: &[Arg], c: &mut dyn Ctx, international: bool) -> R<Value> {
    let dates = CalendarDates::new(c);
    let start = dates.day(a, 0)?;
    let days = dates.number(&scalar(arg(a, 1)?))?.trunc();
    // WORKDAY checks its serial bounds in the 1900 epoch before it reads
    // weekend or holiday arguments. This also applies to a zero displacement.
    let last = joysheet_core::date::max_serial(dates.sys);
    let estimate = start as f64 + days + dates.origin() as f64;
    if !(0.0..=last as f64).contains(&estimate) {
        return Err(CellError::Num);
    }
    if days == 0.0 {
        return num_val(start as f64);
    }
    let weekend = weekend_mask(if international { a.get(2) } else { None }, CellError::Num)?;
    // WORKDAY interprets holidays in workbook coordinates even when supplied as literal text.
    let calendar = Workweek::new(dates, weekend, a.get(if international { 3 } else { 2 }), 0)?;
    num_val(calendar.offset(start, days as i64, last)? as f64)
}
fn workday(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    workday_core(a, c, false)
}
fn workday_intl(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    workday_core(a, c, true)
}

pub(crate) fn yearfrac_core(sys: DateSystem, s: i64, e: i64, basis: i64) -> R<f64> {
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

fn feb29_between(sys: DateSystem, s: i64, e: i64, d1: &DateTime, d2: &DateTime) -> bool {
    (d1.year..=d2.year).any(|y| is_leap(y) && serial_from_ymd(sys, y as i64, 2, 29).is_some_and(|f| (f as i64) >= s && (f as i64) <= e))
}

fn yearfrac(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let sys = c.date_system();
    let s = day_arg(a, 0, sys)?;
    let e = day_arg(a, 1, sys)?;
    let basis = if has(a, 2) { to_int(num(a, 2)?)? } else { 0 };
    num_val(yearfrac_core(sys, s, e, basis)?)
}

pub(crate) fn specs() -> Vec<FnSpec> {
    vec![
        f!("DATE", 3, 3, DateTime, S, "DATE(year, month, day)", "Builds a date serial number from a year, month and day.", date),
        f!("TIME", 3, 3, DateTime, S, "TIME(hour, minute, second)", "Builds a time (a fraction of a day) from hours, minutes and seconds.", time),
        f!("YEAR", 1, 1, DateTime, S, "YEAR(serial_number)", "Returns the year of a date.", year),
        f!("MONTH", 1, 1, DateTime, S, "MONTH(serial_number)", "Returns the month (1-12) of a date.", month),
        f!("DAY", 1, 1, DateTime, S, "DAY(serial_number)", "Returns the day of the month of a date.", day),
        f!("HOUR", 1, 1, DateTime, S, "HOUR(serial_number)", "Returns the hour (0-23) of a time.", hour),
        f!("MINUTE", 1, 1, DateTime, S, "MINUTE(serial_number)", "Returns the minute (0-59) of a time.", minute),
        f!("SECOND", 1, 1, DateTime, S, "SECOND(serial_number)", "Returns the second (0-59) of a time.", second),
        f!(volatile; "NOW", 0, 0, DateTime, S, "NOW()", "Returns the current date and time.", now),
        f!(volatile; "TODAY", 0, 0, DateTime, S, "TODAY()", "Returns the current date.", today),
        f!("WEEKDAY", 1, 2, DateTime, S, "WEEKDAY(serial_number, [return_type])", "Returns the day of the week of a date as a number.", weekday),
        f!("WEEKNUM", 1, 2, DateTime, S, "WEEKNUM(serial_number, [return_type])", "Returns the week number of a date within its year.", weeknum),
        f!("ISOWEEKNUM", 1, 1, DateTime, S, "ISOWEEKNUM(date)", "Returns the ISO 8601 week number of a date.", isoweeknum),
        f!("EDATE", 2, 2, DateTime, S, "EDATE(start_date, months)", "Returns the date a number of months before or after a date.", edate),
        f!("EOMONTH", 2, 2, DateTime, S, "EOMONTH(start_date, months)", "Returns the last day of the month a number of months away.", eomonth),
        f!(
            "DATEDIF",
            3,
            3,
            DateTime,
            S,
            "DATEDIF(start_date, end_date, unit)",
            "Counts whole years, months or days between two dates. unit: Y, M, D, MD, YM or YD.",
            datedif
        ),
        f!("DATEVALUE", 1, 1, DateTime, S, "DATEVALUE(date_text)", "Converts a date written as text into a date serial number.", datevalue),
        f!("TIMEVALUE", 1, 1, DateTime, S, "TIMEVALUE(time_text)", "Converts a time written as text into a fraction of a day.", timevalue),
        f!("DAYS", 2, 2, DateTime, S, "DAYS(end_date, start_date)", "Returns the number of days between two dates.", days),
        f!(
            "DAYS360",
            2,
            3,
            DateTime,
            S,
            "DAYS360(start_date, end_date, [method])",
            "Counts days between two dates on a 360-day year (US or European method).",
            days360
        ),
        f!(
            "NETWORKDAYS",
            2,
            3,
            DateTime,
            ND,
            "NETWORKDAYS(start_date, end_date, [holidays])",
            "Counts working days (Monday to Friday) between two dates, excluding holidays.",
            networkdays
        ),
        f!(
            "NETWORKDAYS.INTL",
            2,
            4,
            DateTime,
            NDI,
            "NETWORKDAYS.INTL(start_date, end_date, [weekend], [holidays])",
            "Counts working days between two dates with a custom weekend, excluding holidays.",
            networkdays_intl
        ),
        f!(
            "WORKDAY",
            2,
            3,
            DateTime,
            ND,
            "WORKDAY(start_date, days, [holidays])",
            "Returns the date a number of working days before or after a date.",
            workday
        ),
        f!(
            "WORKDAY.INTL",
            2,
            4,
            DateTime,
            NDI,
            "WORKDAY.INTL(start_date, days, [weekend], [holidays])",
            "Returns the date a number of working days away with a custom weekend.",
            workday_intl
        ),
        f!(
            "YEARFRAC",
            2,
            3,
            DateTime,
            S,
            "YEARFRAC(start_date, end_date, [basis])",
            "Returns the fraction of a year between two dates under a day-count basis.",
            yearfrac
        ),
    ]
}

#[cfg(test)]
#[path = "tests/date_tests.rs"]
mod tests;
