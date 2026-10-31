use super::*;
use crate::util::testutil::*;

thread_local! {
    pub(super) static INTERPOLATION_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[test]
fn interpolation_preserves_gaps_and_endpoint_values() {
    for (slots, expected) in [
        (
            vec![None, Some(4.0), None, None, Some(13.0), Some(-2.0), None, Some(8.0), None, None],
            vec![4.0, 4.0, 7.0, 10.0, 13.0, -2.0, 3.0, 8.0, 8.0, 8.0],
        ),
        (vec![Some(-7.0), None, Some(4.0)], vec![-7.0, -1.5, 4.0]),
        (vec![Some(3.0), Some(-7.0), Some(2.0)], vec![3.0, -7.0, 2.0]),
        (vec![None, None, None], vec![0.0, 0.0, 0.0]),
        (vec![Some(6.0)], vec![6.0]),
        (vec![], vec![]),
    ] {
        assert_eq!(interpolate(&slots), expected, "{slots:?}");
    }
}

#[test]
fn interpolation_lookahead_work_is_linear() {
    let mut measured = Vec::new();
    for len in [2_000, 4_000] {
        let missing = len * 3 / 10;
        let gap = (len - missing) / 2..(len + missing) / 2;
        let expected: Vec<f64> = (0..len).map(|i| (3 * i + 5) as f64).collect();
        let slots: Vec<Option<f64>> = expected.iter().enumerate().map(|(i, &v)| (!gap.contains(&i)).then_some(v)).collect();
        INTERPOLATION_VISITS.with(|visits| visits.set(0));
        let actual = interpolate(&slots);
        let visits = INTERPOLATION_VISITS.with(|visits| visits.get());
        assert_eq!(actual, expected);
        measured.push((len, visits));
    }
    // Record both sizes before asserting the work bound, including on the old algorithm.
    eprintln!("interpolation lookahead visits (slots, visits): {measured:?}");
    for (len, visits) in measured {
        assert!(visits > 0 && visits <= 2 * len, "{len} slots required {visits} lookahead visits");
    }
}

#[test]
fn series_completes_a_long_gap_at_the_missing_point_limit() {
    let len = 2_000;
    let gap = 700..1_300; // Exactly 30% of the completed timeline.
    let mut times: Vec<f64> = (0..len).filter(|i| !gap.contains(i)).map(|i| (i + 1) as f64).collect();
    let mut values: Vec<f64> = times.iter().map(|t| 3.0 * t + 2.0).collect();
    let options = Options::default();
    let completed = series(&col(&values), &col(&times), &options, DateSystem::D1900).unwrap();
    let expected: Vec<f64> = (0..len).map(|i| (3 * i + 5) as f64).collect();
    assert_eq!(completed.y, expected);
    assert_eq!(completed.step, 1.0);
    assert_eq!(completed.horizon((len + 1) as f64), Ok(1.0));

    let zeroes = series(&col(&values), &col(&times), &Options { interpolate: false, ..options }, DateSystem::D1900).unwrap();
    for (i, &value) in zeroes.y.iter().enumerate() {
        assert_eq!(value, if gap.contains(&i) { 0.0 } else { expected[i] });
    }

    // Remove the first known point after the gap without changing either timeline endpoint.
    times.remove(gap.start);
    values.remove(gap.start);
    assert!(matches!(series(&col(&values), &col(&times), &options, DateSystem::D1900), Err(CellError::Num)));
}

fn range(xs: &[f64]) -> Arg {
    rf(col(xs))
}

fn timeline(n: usize) -> Vec<f64> {
    (1..=n).map(|t| t as f64).collect()
}

/// y = 3 + 2t plus a four-step season, over `n` points.
fn seasonal(n: usize) -> Vec<f64> {
    let pattern = [4.0, -2.0, 7.0, -9.0];
    (1..=n).map(|t| 3.0 + 2.0 * t as f64 + pattern[(t - 1) % 4]).collect()
}

#[test]
fn straight_line_forecasts_exactly() {
    let ys: Vec<f64> = (1..=20).map(|t| 3.0 + 2.0 * t as f64).collect();
    let ts = timeline(20);
    close_tol(ev("FORECAST.ETS", vec![n(25.0), range(&ys), range(&ts)]), 53.0, 1e-9);
    close_tol(ev("FORECAST.ETS", vec![n(20.5), range(&ys), range(&ts), n(0.0)]), 44.0, 1e-9);
    close(ev("FORECAST.ETS.SEASONALITY", vec![range(&ys), range(&ts)]), 0.0);
    close_tol(ev("FORECAST.ETS.CONFINT", vec![n(25.0), range(&ys), range(&ts)]), 0.0, 1e-9);
    close(ev("FORECAST.ETS.STAT", vec![range(&ys), range(&ts), n(8.0)]), 1.0);
    close_tol(ev("FORECAST.ETS.STAT", vec![range(&ys), range(&ts), n(6.0)]), 0.0, 1e-9);
    // Target dates are lifted over arrays.
    let v = ev("FORECAST.ETS", vec![av(row(&[21.0, 22.0])), range(&ys), range(&ts)]);
    let r = rows_of(&v);
    close_tol(r[0][0].clone(), 45.0, 1e-9);
    close_tol(r[0][1].clone(), 47.0, 1e-9);
}

#[test]
fn seasonal_series_is_detected_and_continued() {
    let ys = seasonal(24);
    let ts = timeline(24);
    close(ev("FORECAST.ETS.SEASONALITY", vec![range(&ys), range(&ts)]), 4.0);
    // t = 27 is the third position of the pattern: 3 + 54 + 7.
    close_tol(ev("FORECAST.ETS", vec![n(27.0), range(&ys), range(&ts)]), 64.0, 1e-7);
    close_tol(ev("FORECAST.ETS", vec![n(28.0), range(&ys), range(&ts), n(4.0)]), 50.0, 1e-7);
    // Timeline order does not matter.
    let mut rev_y = ys.clone();
    let mut rev_t = ts.clone();
    rev_y.reverse();
    rev_t.reverse();
    close_tol(ev("FORECAST.ETS", vec![n(27.0), range(&rev_y), range(&rev_t)]), 64.0, 1e-7);
}

#[test]
fn noisy_series_statistics_are_consistent() {
    // A seasonal series with deterministic pseudo-noise.
    let ys: Vec<f64> = seasonal(48).iter().enumerate().map(|(i, v)| v + ((i * 37 % 11) as f64 - 5.0) * 0.4).collect();
    let ts = timeline(48);
    let stat = |k: f64| match ev("FORECAST.ETS.STAT", vec![range(&ys), range(&ts), n(k)]) {
        Value::Number(x) => x,
        other => panic!("statistic {k}: {other:?}"),
    };
    for k in [1.0, 2.0, 3.0] {
        assert!((0.0..=1.0).contains(&stat(k)), "parameter {k} out of range");
    }
    let (mase, smape, mae, rmse) = (stat(4.0), stat(5.0), stat(6.0), stat(7.0));
    assert!(mae > 0.0 && rmse >= mae && mase > 0.0 && smape > 0.0 && smape < 2.0);
    let conf = |t: f64, c: f64| match ev("FORECAST.ETS.CONFINT", vec![n(t), range(&ys), range(&ts), n(c)]) {
        Value::Number(x) => x,
        other => panic!("confint: {other:?}"),
    };
    assert!(conf(49.0, 0.95) > 0.0);
    assert!(conf(60.0, 0.95) > conf(49.0, 0.95), "the interval widens with the horizon");
    assert!(conf(49.0, 0.99) > conf(49.0, 0.95));
    // The forecast stays close to the noise-free continuation.
    match ev("FORECAST.ETS", vec![n(51.0), range(&ys), range(&ts)]) {
        Value::Number(x) => assert!((x - (3.0 + 102.0 + 7.0)).abs() < 3.0, "forecast {x}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn monthly_dates_missing_points_and_duplicates() {
    // First of each month, 2024-01 … 2025-12 (serials of 1 Jan 2024 = 45292).
    let mut ts = Vec::new();
    let mut serial = 45292.0;
    for i in 0..24 {
        ts.push(serial);
        let (y, m) = (2024 + i / 12, (i % 12) as u32 + 1);
        serial += f64::from(joysheet_core::date::days_in_month(y, m));
    }
    let ys: Vec<f64> = (0..24).map(|i| 100.0 + 5.0 * i as f64).collect();
    // 2026-01-01 = 46023, 2026-03-01 = 46082.
    close_tol(ev("FORECAST.ETS", vec![n(46023.0), range(&ys), range(&ts), n(0.0)]), 220.0, 1e-9);
    close_tol(ev("FORECAST.ETS", vec![n(46082.0), range(&ys), range(&ts), n(0.0)]), 230.0, 1e-9);
    // Missing points are interpolated (or zero with data_completion 0).
    let gaps_t: Vec<f64> = [1.0, 2.0, 4.0, 5.0, 6.0, 8.0, 9.0, 10.0].to_vec();
    let gaps_y: Vec<f64> = gaps_t.iter().map(|t| 10.0 * t).collect();
    close_tol(ev("FORECAST.ETS", vec![n(12.0), range(&gaps_y), range(&gaps_t), n(0.0)]), 120.0, 1e-9);
    assert_ne!(ev("FORECAST.ETS", vec![n(12.0), range(&gaps_y), range(&gaps_t), n(0.0), n(0.0)]), Value::Number(120.0));
    // Duplicate time stamps are averaged by default, summed with aggregation 7.
    let dup_t = [1.0, 2.0, 2.0, 3.0, 4.0, 5.0];
    let dup_y = [2.0, 3.0, 5.0, 6.0, 8.0, 10.0];
    close_tol(ev("FORECAST.ETS", vec![n(6.0), range(&dup_y), range(&dup_t), n(0.0)]), 12.0, 1e-9);
    close_tol(ev("FORECAST.ETS.STAT", vec![range(&dup_y), range(&dup_t), n(8.0)]), 1.0, 1e-12);
    assert_ne!(ev("FORECAST.ETS", vec![n(6.0), range(&dup_y), range(&dup_t), n(0.0), n(1.0), n(7.0)]), Value::Number(12.0));
}

#[test]
fn documented_errors() {
    let ys = seasonal(12);
    let ts = timeline(12);
    let ets = |args: Vec<Arg>| ev("FORECAST.ETS", args);
    // Target before the end of the timeline.
    is_err(ets(vec![n(5.0), range(&ys), range(&ts)]), CellError::Num);
    // Timeline and values of different sizes.
    is_err(ets(vec![n(13.0), range(&ys), range(&ts[..11])]), CellError::NA);
    // No constant step.
    is_err(ets(vec![n(5.0), range(&[1.0, 2.0, 3.0]), range(&[1.0, 2.0, 4.5])]), CellError::Num);
    // Seasonality, completion, aggregation out of range.
    for (i, bad) in [(3, 8761.0), (3, -1.0), (4, 2.0), (5, 8.0)] {
        let mut args = vec![n(13.0), range(&ys), range(&ts), n(1.0), n(1.0), n(0.0)];
        args[i] = n(bad);
        is_err(ets(args), CellError::Num);
    }
    // More than 30% of the points missing.
    is_err(ets(vec![n(30.0), range(&[1.0, 2.0, 3.0]), range(&[1.0, 2.0, 20.0])]), CellError::Num);
    // Text time stamps; errors propagate.
    is_err(ets(vec![n(13.0), range(&[1.0, 2.0]), rf(arr(vec![vec![nv(1.0)], vec![tv("x")]]))]), CellError::Value);
    is_err(ets(vec![n(13.0), rf(arr(vec![vec![nv(1.0)], vec![Value::Error(CellError::Div0)]])), range(&[1.0, 2.0])]), CellError::Div0);
    // Confidence level outside (0, 1); statistic type outside 1–8.
    is_err(ev("FORECAST.ETS.CONFINT", vec![n(13.0), range(&ys), range(&ts), n(1.0)]), CellError::Num);
    is_err(ev("FORECAST.ETS.CONFINT", vec![n(13.0), range(&ys), range(&ts), n(0.0)]), CellError::Num);
    is_err(ev("FORECAST.ETS.STAT", vec![range(&ys), range(&ts), n(9.0)]), CellError::Num);
    is_err(ev("FORECAST.ETS.STAT", vec![range(&ys), range(&ts), n(0.0)]), CellError::Num);
    // Hostile spans: a huge gap is rejected before anything is allocated.
    is_err(ets(vec![n(1e15), range(&[1.0, 2.0, 3.0]), range(&[0.0, 1.0, 1e12])]), CellError::Num);
    is_err(ets(vec![n(1e308), range(&ys), range(&ts)]), CellError::Num);
}

#[test]
fn timelines_continue_past_their_last_point() {
    let tl = Timeline::detect(&[3.0, 1.0, 2.0, 2.0, 5.0], DateSystem::D1900).expect("numeric step");
    assert_eq!(tl.after_last(1), Some(6.0));
    assert_eq!(tl.steps_after_last(8.5), Some(3.5));
    // Month ends: 2024-01-31 (45322), 2024-02-29 (45351), 2024-03-31 (45382).
    let tl = Timeline::detect(&[45322.0, 45351.0, 45382.0], DateSystem::D1900).expect("months");
    assert_eq!(tl.after_last(1), Some(45412.0)); // 2024-04-30
    assert_eq!(tl.steps_after_last(45412.0), Some(1.0));
    assert!(Timeline::detect(&[1.0, 2.0, 4.5], DateSystem::D1900).is_none());
    assert!(Timeline::detect(&[1.0, 2.0, 9.0], DateSystem::D1900).is_none(), "too many missing points");
    assert!(Timeline::detect(&[1.0], DateSystem::D1900).is_none());
    assert!(Timeline::detect(&[1.0, f64::NAN], DateSystem::D1900).is_none());
}
