//! Working-day count and selection in Excel serial coordinates. Holidays are
//! stored both by date and by the workday index they remove from the calendar.
use super::{R, weekday0};
use crate::util::{arg, as_array, scalar};
use crate::{Arg, Ctx};
use joysheet_core::{CellError, DateSystem, Value};

#[derive(Clone, Copy)]
pub(super) struct CalendarDates {
    pub(super) sys: DateSystem,
    year_first: bool,
}

impl CalendarDates {
    pub(super) fn new(ctx: &dyn Ctx) -> Self {
        Self { sys: ctx.date_system(), year_first: ctx.year_first_calendar_dates() }
    }

    pub(super) fn origin(self) -> i64 {
        if self.sys == DateSystem::D1904 { 1462 } else { 0 }
    }

    pub(super) fn number(self, value: &Value) -> R<f64> {
        match value {
            Value::Bool(_) => Err(CellError::Value),
            Value::Text(text) if self.year_first => {
                let parsed = joysheet_core::parse::parse_input(text, self.sys);
                match parsed.value {
                    Value::Number(n) if parsed.format != Some("m/d/yyyy") => Ok(n),
                    _ => Err(CellError::Value),
                }
            }
            other => other.to_number_in(self.sys),
        }
    }

    fn value(self, value: &Value, text_origin: i64) -> R<i64> {
        let n = self.number(value)?;
        if !(0.0..(joysheet_core::date::max_serial(self.sys) + 1) as f64).contains(&n) {
            return Err(CellError::Num);
        }
        Ok(n.floor() as i64 - if matches!(value, Value::Text(_)) { text_origin } else { 0 })
    }

    pub(super) fn day(self, args: &[Arg], index: usize) -> R<i64> {
        let a = arg(args, index)?;
        // Excel translates literal text serials from the 1900 epoch. Values
        // read from a cell are already in the workbook's coordinates.
        self.value(&scalar(a), if a.from_ref { 0 } else { self.origin() })
    }
}

pub(super) struct Workweek {
    /// Number of nominal workdays before each offset in the serial's seven-day cycle.
    prefix: [i64; 8],
    /// Serial offsets corresponding to the workday indices within one week.
    offsets: Vec<i64>,
    /// Both coordinates are nondecreasing. Consecutive holidays remove the same workday index.
    holidays: Vec<(i64, i64)>,
}

impl Workweek {
    pub(super) fn new(dates: CalendarDates, weekend: [bool; 7], values: Option<&Arg>, literal_text_origin: i64) -> R<Self> {
        let mut prefix = [0; 8];
        let mut offsets = Vec::with_capacity(7);
        for day in 0..7 {
            if !weekend[weekday0(dates.sys, day)] {
                offsets.push(day);
            }
            prefix[day as usize + 1] = offsets.len() as i64;
        }
        let mut calendar = Self { prefix, offsets, holidays: Vec::new() };
        if let Some(arg) = values {
            let text_origin = if arg.from_ref { 0 } else { literal_text_origin };
            for value in as_array(&arg.value).iter() {
                if matches!(value, Value::Empty) {
                    continue;
                }
                let day = dates.value(value, text_origin)?;
                if !weekend[weekday0(dates.sys, day)] {
                    calendar.holidays.push((day, calendar.nominal_before(day)));
                }
            }
            calendar.holidays.sort_unstable();
            calendar.holidays.dedup();
            for (removed, (_, rank)) in calendar.holidays.iter_mut().enumerate() {
                *rank -= removed as i64;
            }
        }
        Ok(calendar)
    }

    fn nominal_before(&self, day: i64) -> i64 {
        day.div_euclid(7) * self.prefix[7] + self.prefix[day.rem_euclid(7) as usize]
    }
    fn before(&self, day: i64) -> i64 {
        self.nominal_before(day) - self.holidays.partition_point(|&(date, _)| date < day) as i64
    }
    pub(super) fn inclusive(&self, start: i64, end: i64) -> i64 {
        if start <= end { self.before(end + 1) - self.before(start) } else { self.before(end) - self.before(start + 1) }
    }

    pub(super) fn offset(&self, start: i64, days: i64, last: i64) -> R<i64> {
        if self.offsets.is_empty() {
            return Err(CellError::Value);
        }
        let target = if days > 0 { self.before(start + 1) + days - 1 } else { self.before(start) + days };
        if target < 0 {
            return Err(CellError::Num);
        }
        let restored = self.holidays.partition_point(|&(_, rank)| rank <= target) as i64;
        let nominal = target + restored;
        let week = self.prefix[7];
        let date = 7 * (nominal / week) + self.offsets[(nominal % week) as usize];
        (date <= last).then_some(date).ok_or(CellError::Num)
    }
}
