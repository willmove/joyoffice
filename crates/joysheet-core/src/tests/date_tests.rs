use super::*;

#[test]
fn serial_rounding_into_next_day_past_max_is_none() {
    for sys in [DateSystem::D1900, DateSystem::D1904] {
        let serial = max_serial(sys) as f64 + 0.9999999995;
        assert_eq!(datetime_from_serial(sys, serial), None);
    }
}

#[test]
fn leap_and_month_length_outside_chrono_range() {
    // In range, the ordinary Gregorian rule holds.
    assert!(is_leap(2000));
    assert!(!is_leap(1900));
    assert!(is_leap(2024));
    assert!(!is_leap(2023));
    // Outside chrono's NaiveDate range the Gregorian rule is still applied.
    assert!(is_leap(400_000)); // divisible by 400
    assert!(!is_leap(300_100)); // century not divisible by 400
    assert!(is_leap(i32::MAX - 3)); // 2147483644 is divisible by 4, not a century
    assert!(!is_leap(i32::MAX)); // 2147483647 is odd
    // days_in_month keeps calendar lengths out of range and 0 for invalid months.
    assert_eq!(days_in_month(400_000, 2), 29);
    assert_eq!(days_in_month(300_100, 2), 28);
    assert_eq!(days_in_month(400_000, 1), 31);
    assert_eq!(days_in_month(400_000, 4), 30);
    assert_eq!(days_in_month(2024, 2), 29);
    assert_eq!(days_in_month(2023, 2), 28);
    assert_eq!(days_in_month(2023, 0), 0);
    assert_eq!(days_in_month(2023, 13), 0);
    // December at i32::MAX does not overflow and still reports 31 days.
    assert_eq!(days_in_month(i32::MAX, 12), 31);
    assert_eq!(days_in_month(i32::MAX, 2), 28); // 2147483647 is not a leap year
}

#[test]
fn civil_conversions_exact_beyond_chrono_range() {
    assert_eq!(days_from_civil(1970, 1, 1), 0);
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    // Past `NaiveDate`'s ±262143 the 400-year cycles carry the result, exactly, both sides of
    // the epoch; values round-trip through the proleptic Gregorian converters.
    for (y, m, d, z) in [
        (262_144i32, 1u32, 1u32, 95_026_602i64),
        (300_000, 1, 1, 108_853_222),
        (400_000, 2, 29, 145_377_531),
        (-262_145, 3, 1, -96_465_964),
        (-300_000, 6, 15, -110_292_112),
    ] {
        assert_eq!(days_from_civil(y, m, d), z, "{y}-{m}-{d}");
        assert_eq!(civil_from_days(z), (y, m, d), "{z}");
    }
    // The extremes must not overflow or panic (i128 intermediates).
    let _ = (days_from_civil(i32::MAX, 12, 31), days_from_civil(i32::MIN, 1, 1), civil_from_days(i64::MAX), civil_from_days(i64::MIN));
}

#[test]
fn serials_unchanged_through_cycle_conversion() {
    // The 1900 leap bug and 1904 semantics must stay exact; the converters only ever see
    // in-range years here, so the serial outputs are byte-for-byte the pre-chrono ones.
    let s = DateSystem::D1900;
    assert_eq!(serial_from_ymd(s, 1900, 2, 29), Some(60.0)); // Excel's fictitious leap day
    assert_eq!(serial_from_ymd(s, 1900, 3, 1), Some(61.0));
    assert_eq!(serial_from_ymd(s, 9999, 12, 31), Some(2958465.0));
    assert_eq!(days_in_month_in(s, 1900, 2), 29);
    let s4 = DateSystem::D1904;
    assert_eq!(serial_from_ymd(s4, 1904, 1, 1), Some(0.0)); // serial 0 is 1904-01-01
    assert_eq!(serial_from_ymd(s4, 1904, 2, 29), Some(59.0)); // 1904 is a real leap year
    assert_eq!(serial_from_ymd(s4, 1900, 2, 29), None); // no fictitious day in 1904 system
    assert_eq!(days_in_month_in(s4, 1900, 2), 28);
}

#[test]
fn serial_from_unix_time() {
    assert_eq!(serial_from_unix_ms(0.0), 25569.0);
    // 2026-10-10T12:00:00Z
    assert_eq!(serial_from_unix_ms(1_791_633_600_000.0), 46305.5);
    assert!(unix_now_ms().is_some_and(|ms| serial_from_unix_ms(ms) > 46305.0));
}

#[test]
fn known_serials() {
    let s = DateSystem::D1900;
    assert_eq!(serial_from_ymd(s, 1900, 1, 1), Some(1.0));
    assert_eq!(serial_from_ymd(s, 1900, 2, 28), Some(59.0));
    assert_eq!(serial_from_ymd(s, 1900, 3, 1), Some(61.0));
    assert_eq!(serial_from_ymd(s, 2000, 1, 1), Some(36526.0));
    assert_eq!(serial_from_ymd(s, 2026, 10, 7), Some(46302.0));
    assert_eq!(serial_from_ymd(s, 9999, 12, 31), Some(2958465.0));
    assert_eq!(serial_from_ymd(DateSystem::D1904, 9999, 12, 31), Some(2957003.0));
    assert_eq!(serial_from_ymd(DateSystem::D1904, 9999, 12, 32), None);
    assert!(datetime_from_serial(DateSystem::D1904, 2957003.5).is_some());
    assert!(datetime_from_serial(DateSystem::D1904, 2957004.0).is_none());
    assert_eq!(serial_from_ymd(s, 2020, 13, 1), serial_from_ymd(s, 2021, 1, 1));
    assert_eq!(serial_from_ymd(s, 2020, 3, 0), serial_from_ymd(s, 2020, 2, 29));
    assert_eq!(serial_from_ymd(s, 120, 1, 1), serial_from_ymd(s, 2020, 1, 1));
    assert_eq!(serial_from_ymd(DateSystem::D1904, 1904, 1, 1), Some(0.0));
    assert_eq!(serial_from_ymd(DateSystem::D1904, 2000, 1, 1), Some(35064.0));
}

#[test]
fn fictitious_1900_02_29() {
    // Excel's 1900 calendar has a 29 February 1900 (serial 60); day and month overflow count
    // through it.
    let s = DateSystem::D1900;
    assert_eq!(serial_from_ymd(s, 1900, 2, 29), Some(60.0));
    assert_eq!(serial_from_ymd(s, 1900, 3, 0), Some(60.0));
    assert_eq!(serial_from_ymd(s, 1900, 2, 30), Some(61.0));
    assert_eq!(serial_from_ymd(s, 1900, 1, 60), Some(60.0));
    assert_eq!(serial_from_ymd(s, 1900, 1, 61), Some(61.0));
    assert_eq!(serial_from_ymd(s, 1900, 1, 400), Some(400.0));
    assert_eq!(serial_from_ymd(s, 1900, 4, -30), Some(61.0));
    assert_eq!(serial_from_ymd(s, 1900, 4, -31), Some(60.0));
    assert_eq!(serial_from_ymd(s, 1900, 3, -1), Some(59.0));
    assert_eq!(serial_from_ymd(s, 1901, -10, 29), Some(60.0));
    assert_eq!(serial_from_ymd(s, 1900, 1, 0), Some(0.0));
    assert_eq!(serial_from_ymd(s, 1900, 1, -1), None);
    assert_eq!(serial_from_ymd(s, 1900, 0, 1), None);
    // Later years keep the real calendar.
    assert_eq!(serial_from_ymd(s, 1901, 2, 29), serial_from_ymd(s, 1901, 3, 1));
    assert_eq!(serial_from_ymd(s, 1904, 2, 29), Some(1521.0));
    // Every serial up to 1900-12-31 comes back from its own year, month and day.
    for serial in 0..=366 {
        let d = datetime_from_serial(s, serial as f64).unwrap();
        assert_eq!(serial_from_ymd(s, d.year as i64, d.month as i64, d.day as i64), Some(serial as f64), "{d:?}");
    }
    assert_eq!(days_in_month_in(s, 1900, 2), 29);
    assert_eq!(days_in_month_in(s, 1901, 2), 28);
    assert_eq!(days_in_month_in(DateSystem::D1904, 1900, 2), 28);
    // The 1904 system has no fictitious day: 1904 is a real leap year.
    let s = DateSystem::D1904;
    assert_eq!(serial_from_ymd(s, 1904, 2, 29), Some(59.0));
    assert_eq!(serial_from_ymd(s, 1904, 3, 0), Some(59.0));
    assert_eq!(serial_from_ymd(s, 1904, 3, 1), Some(60.0));
    assert_eq!(serial_from_ymd(s, 1904, 1, 0), None);
    assert_eq!(serial_from_ymd(s, 1900, 2, 29), None);
}

#[test]
fn hostile_month_and_day() {
    let s = DateSystem::D1900;
    assert_eq!(serial_from_ymd(s, 2000, i64::MIN, 1), None);
    assert_eq!(serial_from_ymd(s, 2000, i64::MAX, 1), None);
    assert_eq!(serial_from_ymd(s, 2000, 1, i64::MAX), None);
    assert_eq!(serial_from_ymd(s, 2000, 1, i64::MIN), None);
    assert_eq!(serial_from_ymd(s, i64::MAX, 1, 1), None);
}

#[test]
fn roundtrip() {
    let s = DateSystem::D1900;
    for serial in [1.0, 59.0, 60.0, 61.0, 36526.0, 46302.0, 2958465.0] {
        let d = datetime_from_serial(s, serial).unwrap();
        assert_eq!(serial_from_ymd(s, d.year as i64, d.month as i64, d.day as i64), Some(serial));
    }
    let d = datetime_from_serial(s, 60.0).unwrap();
    assert_eq!((d.year, d.month, d.day), (1900, 2, 29));
    let d = datetime_from_serial(s, 46302.75).unwrap();
    assert_eq!((d.hour, d.minute, d.weekday), (18, 0, 3)); // 2026-10-07 is a Wednesday
    let d = datetime_from_serial(s, 1.0).unwrap();
    assert_eq!(d.weekday, 0); // 1900-01-01 was a Sunday in Excel's calendar
}
