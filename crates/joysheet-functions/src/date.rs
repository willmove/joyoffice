//! First-party date and time function registry (`calendar`, `workweek`).

mod calendar;
mod workweek;

use joysheet_core::{CellError, Value};

use crate::util::{R, S, arg, num_val, scalar, to_int};
use crate::{Arg, Ctx, FnSpec};
use calendar::{
    date, datedif, datevalue, day, days, days360, edate, eomonth, hour, isoweeknum, minute, month, now, second, time, timevalue, today, weekday,
    weekday0, weeknum, year, yearfrac,
};
use workweek::{CalendarDates, Workweek};

/// NETWORKDAYS / WORKDAY: start, end/days scalar; holidays take arrays.
const ND: &[bool] = &[true, true, false];
const NDI: &[bool] = &[true, true, true, false];

/// Weekend mask indexed by weekday (0 = Sunday).
fn weekend_mask(v: Option<&Arg>, error_argument: CellError) -> R<[bool; 7]> {
    let Some(arg) = v else { return Ok([true, false, false, false, false, false, true]) };
    match scalar(arg) {
        Value::Empty if !arg.from_ref => Ok([true, false, false, false, false, false, true]),
        Value::Text(t) => {
            let b = t.as_bytes();
            if b.len() != 7 || !b.iter().all(|c| *c == b'0' || *c == b'1') {
                return Err(CellError::Value);
            }
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
    let last = joysheet_core::date::max_serial(dates.sys);
    let estimate = start as f64 + days + dates.origin() as f64;
    if !(0.0..=last as f64).contains(&estimate) {
        return Err(CellError::Num);
    }
    if days == 0.0 {
        return num_val(start as f64);
    }
    let weekend = weekend_mask(if international { a.get(2) } else { None }, CellError::Num)?;
    let calendar = Workweek::new(dates, weekend, a.get(if international { 3 } else { 2 }), 0)?;
    num_val(calendar.offset(start, days as i64, last)? as f64)
}

fn workday(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    workday_core(a, c, false)
}

fn workday_intl(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    workday_core(a, c, true)
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
