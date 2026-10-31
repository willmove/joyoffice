use joysheet_core::{CellError, Value};

use crate::util::testutil::*;

fn d(y: f64, m: f64, dd: f64) -> f64 {
    match ev("DATE", vec![n(y), n(m), n(dd)]) {
        Value::Number(x) => x,
        v => panic!("{v:?}"),
    }
}

#[test]
fn construct() {
    assert_eq!(d(2026.0, 10.0, 7.0), 46302.0);
    assert_eq!(d(2020.0, 13.0, 1.0), 44197.0);
    assert_eq!(d(2020.0, 3.0, 0.0), 43890.0);
    assert_eq!(d(120.0, 1.0, 1.0), 43831.0);
    assert_eq!(d(2020.0, 1.0, -1.0), 43829.0);
    is_err(ev("DATE", vec![n(10000.0), n(1.0), n(1.0)]), CellError::Num);
    is_err(ev("DATE", vec![n(-1.0), n(1.0), n(1.0)]), CellError::Num);
    is_err(ev("DATE", vec![n(1900.0), n(1.0), n(-5.0)]), CellError::Num);
    close(ev("TIME", vec![n(12.0), n(0.0), n(0.0)]), 0.5);
    close(ev("TIME", vec![n(25.0), n(0.0), n(0.0)]), 1.0 / 24.0);
    is_err(ev("TIME", vec![n(0.0), n(-1.0), n(0.0)]), CellError::Num);
}

#[test]
fn time_negative() {
    is_err(ev("TIME", vec![n(0.0), n(0.0), n(-1.0)]), CellError::Num);
    close(ev("TIME", vec![n(1.0), n(-30.0), n(0.0)]), 1.0 / 48.0);
}

#[test]
fn parts() {
    close(ev("YEAR", vec![n(46302.75)]), 2026.0);
    close(ev("MONTH", vec![n(46302.75)]), 10.0);
    close(ev("DAY", vec![n(46302.75)]), 7.0);
    close(ev("HOUR", vec![n(46302.75)]), 18.0);
    close(ev("MINUTE", vec![n(0.5 + 61.0 / 1440.0)]), 1.0);
    close(ev("SECOND", vec![n(12.5 / 86400.0)]), 13.0);
    close(ev("HOUR", vec![n(0.999_999_9)]), 0.0);
    close(ev("YEAR", vec![t("2020-05-06")]), 2020.0);
    close(ev("DAY", vec![n(0.0)]), 0.0);
    close(ev("MONTH", vec![n(60.0)]), 2.0);
    close(ev("DAY", vec![n(60.0)]), 29.0);
    is_err(ev("YEAR", vec![n(-1.0)]), CellError::Num);
    is_err(ev("YEAR", vec![t("abc")]), CellError::Value);
    close(ev("NOW", vec![]), 46302.5);
    close(ev("TODAY", vec![]), 46302.0);
}

#[test]
fn weekdays() {
    // 2026-10-07 is a Wednesday.
    close(ev("WEEKDAY", vec![n(46302.0)]), 4.0);
    close(ev("WEEKDAY", vec![n(46302.0), n(2.0)]), 3.0);
    close(ev("WEEKDAY", vec![n(46302.0), n(3.0)]), 2.0);
    close(ev("WEEKDAY", vec![n(46302.0), n(13.0)]), 1.0);
    close(ev("WEEKDAY", vec![n(46302.0), n(16.0)]), 5.0);
    close(ev("WEEKDAY", vec![n(46302.0), n(17.0)]), 4.0);
    is_err(ev("WEEKDAY", vec![n(46302.0), n(4.0)]), CellError::Num);
    close(ev("WEEKDAY", vec![n(1.0)]), 1.0);
    // 2024-01-01 Monday.
    let jan1 = d(2024.0, 1.0, 1.0);
    close(ev("WEEKNUM", vec![n(jan1)]), 1.0);
    close(ev("WEEKNUM", vec![n(d(2024.0, 1.0, 7.0))]), 2.0); // Sunday starts week 2
    close(ev("WEEKNUM", vec![n(d(2024.0, 1.0, 7.0)), n(2.0)]), 1.0);
    close(ev("WEEKNUM", vec![n(d(2026.0, 10.0, 7.0))]), 41.0);
    close(ev("WEEKNUM", vec![n(d(2021.0, 1.0, 1.0)), n(21.0)]), 53.0);
    close(ev("ISOWEEKNUM", vec![n(d(2021.0, 1.0, 1.0))]), 53.0);
    close(ev("ISOWEEKNUM", vec![n(d(2026.0, 10.0, 7.0))]), 41.0);
    close(ev("ISOWEEKNUM", vec![n(d(2024.0, 12.0, 30.0))]), 1.0);
    is_err(ev("WEEKNUM", vec![n(1.0), n(3.0)]), CellError::Num);
}

#[test]
fn month_math() {
    close(ev("EDATE", vec![n(d(2020.0, 1.0, 31.0)), n(1.0)]), d(2020.0, 2.0, 29.0));
    close(ev("EDATE", vec![n(d(2020.0, 3.0, 15.0)), n(-14.0)]), d(2019.0, 1.0, 15.0));
    close(ev("EDATE", vec![n(d(2020.0, 3.0, 15.5)), n(1.9)]), d(2020.0, 4.0, 15.0));
    close(ev("EOMONTH", vec![n(d(2020.0, 1.0, 15.0)), n(1.0)]), d(2020.0, 2.0, 29.0));
    close(ev("EOMONTH", vec![n(d(2020.0, 1.0, 15.0)), n(-1.0)]), d(2019.0, 12.0, 31.0));
    close(ev("EOMONTH", vec![n(d(2020.0, 1.0, 15.0)), n(0.0)]), d(2020.0, 1.0, 31.0));
    is_err(ev("EOMONTH", vec![n(1.0), n(-12.0)]), CellError::Num);
    is_err(ev("EDATE", vec![n(1.0), n(1e12)]), CellError::Num);
}

#[test]
fn fictitious_1900_02_29() {
    // Excel's 1900 calendar has 29 February 1900 = serial 60, and DATE counts day and month
    // overflow through it.
    assert_eq!(d(1900.0, 2.0, 29.0), 60.0);
    assert_eq!(d(1900.0, 3.0, 0.0), 60.0);
    assert_eq!(d(1900.0, 2.0, 28.0), 59.0);
    assert_eq!(d(1900.0, 3.0, 1.0), 61.0);
    assert_eq!(d(1900.0, 2.0, 30.0), 61.0);
    assert_eq!(d(1900.0, 1.0, 60.0), 60.0);
    assert_eq!(d(1900.0, 4.0, -31.0), 60.0);
    assert_eq!(d(1901.0, -10.0, 29.0), 60.0);
    assert_eq!(d(0.0, 2.0, 29.0), 60.0);
    assert_eq!(d(1900.0, 1.0, 0.0), 0.0);
    close(ev("DAY", vec![n(d(1900.0, 3.0, 0.0))]), 29.0);
    close(ev("DATEVALUE", vec![t("1900-02-29")]), 60.0);
    close(ev("DATEVALUE", vec![t("2/29/1900")]), 60.0);
    close(ev("DATEVALUE", vec![t("3/1/1900")]), 61.0);
    is_err(ev("DATEVALUE", vec![t("2/29/1901")]), CellError::Value);
    // EDATE and EOMONTH read it as 29 February, but end February 1900 on the 28th.
    close(ev("EOMONTH", vec![n(d(1900.0, 1.0, 15.0)), n(1.0)]), 59.0);
    close(ev("EOMONTH", vec![n(60.0), n(0.0)]), 59.0);
    close(ev("EOMONTH", vec![n(d(1900.0, 3.0, 15.0)), n(-1.0)]), 59.0);
    close(ev("EOMONTH", vec![n(60.0), n(12.0)]), d(1901.0, 2.0, 28.0));
    close(ev("EDATE", vec![n(d(1900.0, 1.0, 31.0)), n(1.0)]), 59.0);
    close(ev("EDATE", vec![n(d(1900.0, 3.0, 31.0)), n(-1.0)]), 59.0);
    close(ev("EDATE", vec![n(60.0), n(1.0)]), d(1900.0, 3.0, 29.0));
    close(ev("EDATE", vec![n(60.0), n(12.0)]), d(1901.0, 2.0, 28.0));
}

#[test]
fn edate_eomonth_early_1900() {
    // Excel 365: serial 0 is 0 January 1900 and February 1900 has 28 days here.
    close(ev("EDATE", vec![n(0.0), n(0.0)]), 0.0);
    close(ev("EDATE", vec![n(0.0), n(1.0)]), 31.0);
    close(ev("EDATE", vec![n(0.0), n(12.0)]), 366.0);
    close(ev("EOMONTH", vec![n(0.0), n(1.0)]), 59.0);
    close(ev("EDATE", vec![n(31.0), n(1.0)]), 59.0);
    close(ev("EOMONTH", vec![n(59.0), n(0.0)]), 59.0);
    // Unchanged, and the same as Excel.
    close(ev("EOMONTH", vec![n(0.0), n(0.0)]), 31.0);
    is_err(ev("EOMONTH", vec![n(0.0), n(-1.0)]), CellError::Num);
    close(ev("EDATE", vec![n(59.0), n(1.0)]), 88.0);
    close(ev("EDATE", vec![n(60.0), n(1.0)]), 89.0);
    close(ev("EDATE", vec![n(61.0), n(-1.0)]), 32.0);
    close(ev("EOMONTH", vec![n(31.0), n(0.0)]), 31.0);
}

#[test]
fn datedif_units() {
    let s = n(d(2001.0, 6.0, 1.0));
    let e = n(d(2002.0, 8.0, 15.0));
    close(ev("DATEDIF", vec![s.clone(), e.clone(), t("Y")]), 1.0);
    close(ev("DATEDIF", vec![s.clone(), e.clone(), t("M")]), 14.0);
    close(ev("DATEDIF", vec![s.clone(), e.clone(), t("D")]), 440.0);
    close(ev("DATEDIF", vec![s.clone(), e.clone(), t("YM")]), 2.0);
    close(ev("DATEDIF", vec![s.clone(), e.clone(), t("MD")]), 14.0);
    close(ev("DATEDIF", vec![s.clone(), e.clone(), t("yd")]), 75.0);
    close(ev("DATEDIF", vec![n(d(2020.0, 1.0, 31.0)), n(d(2020.0, 3.0, 1.0)), t("M")]), 1.0);
    close(ev("DATEDIF", vec![n(d(2020.0, 1.0, 31.0)), n(d(2020.0, 3.0, 1.0)), t("MD")]), -1.0);
    is_err(ev("DATEDIF", vec![e.clone(), s.clone(), t("D")]), CellError::Num);
    is_err(ev("DATEDIF", vec![s, e, t("X")]), CellError::Num);
}

#[test]
fn text_conversion() {
    close(ev("DATEVALUE", vec![t("2020-01-01")]), 43831.0);
    close(ev("DATEVALUE", vec![t("1/2/2020 10:00")]), 43832.0);
    close(ev("DATEVALUE", vec![t("4-Mar-2026")]), d(2026.0, 3.0, 4.0));
    is_err(ev("DATEVALUE", vec![t("hello")]), CellError::Value);
    is_err(ev("DATEVALUE", vec![t("123")]), CellError::Value);
    is_err(ev("DATEVALUE", vec![n(43831.0)]), CellError::Value);
    close(ev("TIMEVALUE", vec![t("6:00 PM")]), 0.75);
    close(ev("TIMEVALUE", vec![t("1/2/2020 12:00")]), 0.5);
    is_err(ev("TIMEVALUE", vec![t("x")]), CellError::Value);
}

#[test]
fn day_counts() {
    close(ev("DAYS", vec![n(d(2021.0, 3.0, 15.0)), n(d(2021.0, 2.0, 1.0))]), 42.0);
    close(ev("DAYS", vec![t("2021-02-01"), t("2021-03-15")]), -42.0);
    close(ev("DAYS360", vec![n(d(2011.0, 1.0, 30.0)), n(d(2011.0, 12.0, 31.0))]), 330.0);
    close(ev("DAYS360", vec![n(d(2011.0, 1.0, 29.0)), n(d(2011.0, 12.0, 31.0))]), 332.0);
    close(ev("DAYS360", vec![n(d(2011.0, 1.0, 29.0)), n(d(2011.0, 12.0, 31.0)), b(true)]), 331.0);
    close(ev("DAYS360", vec![n(d(2011.0, 1.0, 30.0)), n(d(2011.0, 12.0, 31.0)), b(true)]), 330.0);
    close(ev("DAYS360", vec![n(d(2011.0, 2.0, 28.0)), n(d(2011.0, 3.0, 31.0))]), 30.0);
    close(ev("DAYS360", vec![n(d(2011.0, 1.0, 1.0)), n(d(2011.0, 1.0, 31.0))]), 30.0);
}

#[test]
fn working_days() {
    let s = d(2012.0, 10.0, 1.0);
    let e = d(2013.0, 3.0, 1.0);
    close(ev("NETWORKDAYS", vec![n(s), n(e)]), 110.0);
    close(ev("NETWORKDAYS", vec![n(s), n(e), av(col(&[d(2012.0, 11.0, 22.0)]))]), 109.0);
    close(ev("NETWORKDAYS", vec![n(s), n(e), av(col(&[d(2012.0, 11.0, 22.0), d(2012.0, 12.0, 4.0), d(2013.0, 1.0, 21.0)]))]), 107.0);
    close(ev("NETWORKDAYS", vec![n(e), n(s)]), -110.0);
    close(ev("NETWORKDAYS.INTL", vec![n(d(2006.0, 1.0, 1.0)), n(d(2006.0, 1.0, 31.0))]), 22.0);
    is_err(ev("NETWORKDAYS.INTL", vec![n(d(2006.0, 1.0, 1.0)), n(d(2006.0, 2.0, 1.0)), n(-1.0)]), CellError::Num);
    close(ev("NETWORKDAYS.INTL", vec![n(d(2006.0, 1.0, 1.0)), n(d(2006.0, 2.0, 1.0)), n(7.0)]), 24.0);
    close(ev("NETWORKDAYS.INTL", vec![n(d(2006.0, 1.0, 1.0)), n(d(2006.0, 2.0, 1.0)), t("0000110")]), 24.0);
    close(ev("NETWORKDAYS.INTL", vec![n(d(2006.0, 1.0, 1.0)), n(d(2006.0, 1.0, 31.0)), n(11.0)]), 26.0);
    // An all-weekend string is valid: no day is a workday, so the count is 0.
    close(ev("NETWORKDAYS.INTL", vec![n(1.0), n(10.0), t("1111111")]), 0.0);
    // WORKDAY.INTL rejects it (there is no working day to move to).
    is_err(ev("WORKDAY.INTL", vec![n(1.0), n(10.0), t("1111111")]), CellError::Value);
    is_err(ev("NETWORKDAYS.INTL", vec![n(1.0), n(10.0), n(8.0)]), CellError::Num);
    // WORKDAY
    close(ev("WORKDAY", vec![n(d(2008.0, 10.0, 1.0)), n(151.0)]), d(2009.0, 4.0, 30.0));
    let hol = av(col(&[d(2008.0, 11.0, 26.0), d(2008.0, 12.0, 4.0), d(2009.0, 1.0, 21.0)]));
    close(ev("WORKDAY", vec![n(d(2008.0, 10.0, 1.0)), n(151.0), hol]), d(2009.0, 5.0, 5.0));
    close(ev("WORKDAY", vec![n(d(2026.0, 10.0, 9.0)), n(1.0)]), d(2026.0, 10.0, 12.0));
    close(ev("WORKDAY", vec![n(d(2026.0, 10.0, 12.0)), n(-1.0)]), d(2026.0, 10.0, 9.0));
    close(ev("WORKDAY", vec![n(d(2026.0, 10.0, 10.0)), n(0.0)]), d(2026.0, 10.0, 10.0));
    is_err(ev("WORKDAY.INTL", vec![n(d(2012.0, 1.0, 1.0)), n(30.0), n(0.0)]), CellError::Num);
    close(ev("WORKDAY.INTL", vec![n(d(2012.0, 1.0, 1.0)), n(90.0), n(11.0)]), d(2012.0, 4.0, 14.0));
    close(ev("WORKDAY.INTL", vec![n(d(2012.0, 1.0, 1.0)), n(30.0), n(17.0)]), d(2012.0, 2.0, 5.0));
    is_err(ev("WORKDAY", vec![n(d(2026.0, 1.0, 1.0)), n(1e9)]), CellError::Num);
    is_err(ev("WORKDAY", vec![n(d(2026.0, 1.0, 1.0)), n(3e6)]), CellError::Num);
}

#[test]
fn yearfrac_bases() {
    let s = n(d(2012.0, 1.0, 1.0));
    let e = n(d(2012.0, 7.0, 30.0));
    close(ev("YEARFRAC", vec![s.clone(), e.clone()]), 0.580_555_555_555_555_6);
    close(ev("YEARFRAC", vec![s.clone(), e.clone(), n(1.0)]), 0.576_502_732_240_437_2);
    close(ev("YEARFRAC", vec![s.clone(), e.clone(), n(2.0)]), 0.586_111_111_111_111_1);
    close(ev("YEARFRAC", vec![s.clone(), e.clone(), n(3.0)]), 0.578_082_191_780_821_9);
    close(ev("YEARFRAC", vec![s.clone(), e.clone(), n(4.0)]), 0.580_555_555_555_555_6);
    close(ev("YEARFRAC", vec![e.clone(), s.clone(), n(1.0)]), 0.576_502_732_240_437_2);
    is_err(ev("YEARFRAC", vec![s.clone(), e, n(5.0)]), CellError::Num);
    // Multi-year actual/actual uses the average year length.
    close(ev("YEARFRAC", vec![n(d(2000.0, 1.0, 1.0)), n(d(2003.0, 1.0, 1.0)), n(1.0)]), 1096.0 / (1461.0 / 4.0));
    close(ev("YEARFRAC", vec![n(d(2011.0, 2.0, 28.0)), n(d(2012.0, 2.0, 29.0)), n(0.0)]), 1.0);
    close(ev("YEARFRAC", vec![n(d(2011.0, 3.0, 1.0)), n(d(2012.0, 2.0, 29.0)), n(1.0)]), 365.0 / 366.0);
    close(ev("YEARFRAC", vec![s, n(d(2012.0, 1.0, 1.0))]), 0.0);
}

#[test]
fn yearfrac_basis0_feb_end_to_31() {
    close(ev("YEARFRAC", vec![n(d(2021.0, 2.0, 28.0)), n(d(2021.0, 3.0, 31.0)), n(0.0)]), 30.0 / 360.0);
    close(ev("YEARFRAC", vec![n(d(2020.0, 2.0, 29.0)), n(d(2020.0, 3.0, 31.0)), n(0.0)]), 30.0 / 360.0);
    close(ev("YEARFRAC", vec![n(d(2021.0, 2.0, 28.0)), n(d(2021.0, 2.0, 28.0)), n(0.0)]), 0.0);
}

#[test]
fn date_lifting() {
    let v = ev("YEAR", vec![av(row(&[43831.0, 46302.0]))]);
    assert_eq!(rows_of(&v), vec![vec![nv(2020.0), nv(2026.0)]]);
}

#[test]
fn system_1904() {
    let mut ctx = TestCtx { sys: joysheet_core::DateSystem::D1904, ..TestCtx::default() };
    let spec = crate::lookup("DATE").unwrap();
    close(crate::call(spec, &[n(2000.0), n(1.0), n(1.0)], &mut ctx), 35064.0);
    let spec = crate::lookup("WEEKDAY").unwrap();
    close(crate::call(spec, &[n(0.0)], &mut ctx), 6.0);
    let spec = crate::lookup("YEAR").unwrap();
    close(crate::call(spec, &[n(0.0)], &mut ctx), 1904.0);
    // The clock is 2026-10-07 12:00; in the 1904 system that is serial 44840.5.
    close(crate::call(crate::lookup("NOW").unwrap(), &[], &mut ctx), 44840.5);
    let today = crate::call(crate::lookup("TODAY").unwrap(), &[], &mut ctx);
    close(today.clone(), 44840.0);
    close(crate::call(spec, &[crate::Arg::val(today)], &mut ctx), 2026.0);
    // No fictitious day here: 1904 is a real leap year and 1900 is before the epoch.
    let spec = crate::lookup("DATE").unwrap();
    close(crate::call(spec, &[n(1904.0), n(2.0), n(29.0)], &mut ctx), 59.0);
    close(crate::call(spec, &[n(1904.0), n(3.0), n(0.0)], &mut ctx), 59.0);
    close(crate::call(spec, &[n(1904.0), n(3.0), n(1.0)], &mut ctx), 60.0);
    is_err(crate::call(spec, &[n(1900.0), n(2.0), n(29.0)], &mut ctx), CellError::Num);
    let spec = crate::lookup("EOMONTH").unwrap();
    close(crate::call(spec, &[n(31.0), n(0.0)], &mut ctx), 59.0);
    let spec = crate::lookup("DATEVALUE").unwrap();
    close(crate::call(spec, &[t("2/29/1904")], &mut ctx), 59.0);
    is_err(crate::call(spec, &[t("2/29/1900")], &mut ctx), CellError::Value);
}

#[test]
fn isoweek_at_calendar_start() {
    let iso = crate::lookup("ISOWEEKNUM").unwrap();
    let wk = crate::lookup("WEEKNUM").unwrap();
    let mut ctx = TestCtx::default();
    close(crate::call(iso, &[n(1.0)], &mut ctx), 52.0);
    close(crate::call(wk, &[n(1.0), n(21.0)], &mut ctx), 52.0);
    let mut ctx = TestCtx { sys: joysheet_core::DateSystem::D1904, ..TestCtx::default() };
    for s in [0.0, 1.0, 2.0] {
        close(crate::call(iso, &[n(s)], &mut ctx), 53.0);
    }
}

#[test]
fn date_text_arguments_use_the_workbook_date_system() {
    let mut ctx = TestCtx { sys: joysheet_core::DateSystem::D1904, ..TestCtx::default() };
    close(crate::call(crate::lookup("YEAR").unwrap(), &[t("1/1/2020")], &mut ctx), 2020.0);
    close(crate::call(crate::lookup("MONTH").unwrap(), &[t("2/29/1904")], &mut ctx), 2.0);
    close(crate::call(crate::lookup("DAY").unwrap(), &[t("2/29/1904")], &mut ctx), 29.0);
    close(crate::call(crate::lookup("DAYS").unwrap(), &[t("1/2/2020"), t("1/1/2020")], &mut ctx), 1.0);
    let networkdays = crate::lookup("NETWORKDAYS").unwrap();
    close(crate::call(networkdays, &[t("1/6/2020"), t("1/10/2020"), t("1/8/2020")], &mut ctx), 4.0);
}
