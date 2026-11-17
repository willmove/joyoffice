//! Date serial numbers.
//!
//! The 1900 system counts days from 1899-12-31 (serial 1 = 1900-01-01) and, like Lotus 1-2-3 and
//! Excel, treats 1900 as a leap year: serial 60 is the fictitious 1900-02-29. The 1904 system
//! counts from 1904-01-01 (serial 0). Times are fractions of a day.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DateSystem {
    #[default]
    D1900,
    D1904,
}

/// A calendar date and time of day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct DateTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    /// Milliseconds (rounded).
    pub milli: u32,
    /// 0 = Sunday.
    pub weekday: u32,
}

/// Days since 1970-01-01 for a proleptic Gregorian date, exact for every `i32` year.
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let (Ok(month), Ok(day)) = (i8::try_from(m), i8::try_from(d)) else {
        return 0;
    };
    let (cycles, year) = (i64::from(y.div_euclid(400)), y.rem_euclid(400) as i16);
    let Ok(date) = jiff::civil::Date::new(year, month, day) else {
        return 0;
    };
    let epoch = jiff::civil::date(1970, 1, 1);
    i64::from((date - epoch).get_days()) + cycles * 146_097
}

/// Milliseconds since the Unix epoch (UTC) from the system clock; `None` when the clock is unavailable.
/// On wasm32 `std::time::SystemTime::now` traps, so the browser clock is read through `Date.now()`.
#[allow(clippy::disallowed_methods)] // the std clock is read only off wasm
pub fn unix_now_ms() -> Option<f64> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?;
        Some(d.as_secs_f64() * 1000.0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        Some(js_sys::Date::now())
    }
}

/// Date serial (1900 system, with fraction) for a Unix time in milliseconds.
pub fn serial_from_unix_ms(ms: f64) -> f64 {
    25569.0 + ms / 86_400_000.0
}

/// Converts a 1900-system serial (as the clock supplies it) to the serial of the same instant in `sys`.
pub fn serial_in_system(sys: DateSystem, serial_1900: f64) -> f64 {
    match sys {
        DateSystem::D1900 => serial_1900,
        DateSystem::D1904 => serial_1900 - 1462.0,
    }
}

/// Proleptic Gregorian date for days since 1970-01-01. Whole 400-year cycles
/// keep the calculation exact beyond the civil calendar library’s year range.
pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let (cycles, rest) = (z.div_euclid(146_097), z.rem_euclid(146_097));
    let epoch = jiff::civil::date(1970, 1, 1);
    let Ok(date) = epoch.checked_add(jiff::Span::new().days(rest)) else {
        return (1970, 1, 1);
    };
    let year = i64::from(date.year()) + cycles * 400;
    (year.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32, date.month() as u32, date.day() as u32)
}

const UNIX_1899_12_30: i64 = -25569; // days_from_civil(1899, 12, 30)
const UNIX_1904_01_01: i64 = -24107;

/// Whether `y` is a Gregorian leap year, for every `i32` year.
pub fn is_leap(y: i32) -> bool {
    jiff::civil::date(y.rem_euclid(400) as i16, 1, 1).in_leap_year()
}

/// Number of days in month `m` of year `y`, or `0` for an invalid month.
pub fn days_in_month(y: i32, m: u32) -> u32 {
    i8::try_from(m)
        .ok()
        .and_then(|month| jiff::civil::Date::new(y.rem_euclid(400) as i16, month, 1).ok())
        .map_or(0, |date| date.days_in_month() as u32)
}

/// Days in a month of a date system's calendar: like [`days_in_month`], except that February
/// 1900 has 29 days in the 1900 system (serial 60 is the fictitious 1900-02-29).
pub fn days_in_month_in(sys: DateSystem, y: i32, m: u32) -> u32 {
    if sys == DateSystem::D1900 && y == 1900 && m == 2 { 29 } else { days_in_month(y, m) }
}

/// Last serial that is a valid date in a date system: 9999-12-31 (2958465 in the 1900 system, 2957003 in the 1904 system).
pub fn max_serial(sys: DateSystem) -> i64 {
    match sys {
        DateSystem::D1900 => 2_958_465,
        DateSystem::D1904 => 2_958_465 - 1462,
    }
}

/// Serial for a date. Month and day overflow roll over like `DATE()` (month 13 = January next
/// year, day 0 = last day of the previous month), in the date system's own calendar: in the 1900
/// system February 1900 has 29 days, so `(1900, 2, 29)` and `(1900, 3, 0)` are serial 60.
/// `None` before the epoch or after 9999-12-31.
pub fn serial_from_ymd(sys: DateSystem, year: i64, month: i64, day: i64) -> Option<f64> {
    // DATE(): years 0–1899 are added to 1900.
    let y = if (0..1900).contains(&year) { year + 1900 } else { year };
    let m0 = month.checked_sub(1)?;
    let y = y.checked_add(m0.div_euclid(12))?;
    let m = m0.rem_euclid(12) + 1;
    if !(1900..=9999).contains(&y) && sys == DateSystem::D1900 || !(1904..=9999).contains(&y) && sys == DateSystem::D1904 {
        return None;
    }
    let first = days_from_civil(y as i32, m as u32, 1);
    // Serial of the first of the month; the day (and any overflow) counts on from there.
    let first = match sys {
        // Up to February 1900 the serial is one lower than the day count from 1899-12-30: the
        // fictitious 1900-02-29 (serial 60) comes before 1 March.
        DateSystem::D1900 => first - UNIX_1899_12_30 - i64::from(y == 1900 && m <= 2),
        DateSystem::D1904 => first - UNIX_1904_01_01,
    };
    let serial = first.checked_add(day)?.checked_sub(1)?;
    if !(0..=max_serial(sys)).contains(&serial) {
        return None;
    }
    Some(serial as f64)
}

/// Date/time from a serial. `None` for negative or too-large serials.
pub fn datetime_from_serial(sys: DateSystem, serial: f64) -> Option<DateTime> {
    if !serial.is_finite() || !(0.0..max_serial(sys) as f64 + 1.0).contains(&serial) {
        return None;
    }
    let mut day = serial.floor() as i64;
    let mut frac_ms = ((serial - serial.floor()) * 86_400_000.0).round() as i64;
    if frac_ms >= 86_400_000 {
        frac_ms -= 86_400_000;
        day += 1;
        if day > max_serial(sys) {
            return None;
        }
    }
    let (y, m, d, weekday) = match sys {
        DateSystem::D1900 => {
            // Excel's weekdays continue the fictitious calendar: serial 1 is a Sunday.
            let wd = ((day + 6) % 7) as u32;
            if day == 60 {
                (1900, 2, 29, wd)
            } else if day == 0 {
                (1900, 1, 0, wd)
            } else {
                let real = if day < 60 { day + 1 } else { day };
                let (y, m, d) = civil_from_days(real + UNIX_1899_12_30);
                (y, m, d, wd)
            }
        }
        DateSystem::D1904 => {
            let unix = day + UNIX_1904_01_01;
            let (y, m, d) = civil_from_days(unix);
            (y, m, d, ((unix + 4).rem_euclid(7)) as u32)
        }
    };
    let secs = frac_ms / 1000;
    Some(DateTime {
        year: y,
        month: m,
        day: d,
        hour: (secs / 3600) as u32,
        minute: ((secs / 60) % 60) as u32,
        second: (secs % 60) as u32,
        milli: (frac_ms % 1000) as u32,
        weekday,
    })
}

/// Fraction of a day for a time.
pub fn time_fraction(h: f64, m: f64, s: f64) -> f64 {
    (h * 3600.0 + m * 60.0 + s) / 86400.0
}

pub const MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
pub const WEEKDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

#[cfg(test)]
#[path = "tests/date_tests.rs"]
mod tests;
