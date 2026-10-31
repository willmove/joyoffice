use super::Trendline;

#[test]
fn clamp_forecast_bounds_hostile_values() {
    assert_eq!(Trendline::clamp_forecast(2.5), 2.5);
    assert_eq!(Trendline::clamp_forecast(0.0), 0.0);
    assert_eq!(Trendline::clamp_forecast(-4.0), 0.0);
    assert_eq!(Trendline::clamp_forecast(f64::NAN), 0.0);
    assert_eq!(Trendline::clamp_forecast(f64::INFINITY), 0.0);
    assert_eq!(Trendline::clamp_forecast(f64::NEG_INFINITY), 0.0);
    assert_eq!(Trendline::clamp_forecast(1e300), Trendline::MAX_FORECAST);
}
