use joysheet_core::{CellRef, Value};
use serde_json::json;

use crate::Session;

fn session_with_history() -> Session {
    let mut s = Session::new();
    s.new_workbook();
    let mut rows = vec![json!(["Month", "Sales"])];
    // y = 10 + 2t with a four-step season.
    let season = [3.0, -1.0, 4.0, -6.0];
    for t in 1..=16 {
        rows.push(json!([t, 10.0 + 2.0 * t as f64 + season[(t - 1) % 4]]));
    }
    s.execute("range.setValues", json!({"range": "A1", "values": rows})).unwrap();
    s
}

fn value(s: &Session, sheet: usize, a1: &str) -> Value {
    s.doc().unwrap().wb.sheet(sheet).unwrap().value(CellRef::parse(a1).unwrap())
}

fn formula(s: &Session, sheet: usize, a1: &str) -> String {
    let wb = &s.doc().unwrap().wb;
    wb.sheet(sheet).unwrap().cell(CellRef::parse(a1).unwrap()).and_then(|c| c.formula.as_ref()).map(|f| f.text.clone()).unwrap_or_default()
}

#[test]
fn forecast_sheet_builds_table_formulas_and_chart() {
    let mut s = session_with_history();
    s.execute("selection.set", json!({"range": "A1:B17"})).unwrap();
    let r = s.execute("data.forecastSheet", json!({"periods": 4, "statistics": true})).unwrap();
    assert_eq!(r["history"], 16);
    assert_eq!(r["periods"], 4);
    let wb = &s.doc().unwrap().wb;
    // Inserted before the data sheet and activated.
    assert_eq!(wb.active_sheet(), 0);
    let sh = wb.sheet(0).unwrap();
    assert_eq!(sh.name, r["sheet"].as_str().unwrap());
    assert_eq!(value(&s, 0, "C1"), Value::text("Forecast(Sales)"));
    assert_eq!(value(&s, 0, "D1"), Value::text("Lower Confidence Bound(Sales)"));
    assert_eq!(value(&s, 0, "E1"), Value::text("Upper Confidence Bound(Sales)"));
    assert_eq!(formula(&s, 0, "C17"), "B17");
    assert_eq!(formula(&s, 0, "C18"), "FORECAST.ETS(A18,$B$2:$B$17,$A$2:$A$17,1,1)");
    assert_eq!(formula(&s, 0, "D18"), "C18-FORECAST.ETS.CONFINT(A18,$B$2:$B$17,$A$2:$A$17,0.95,1,1)");
    assert_eq!(formula(&s, 0, "H2"), "FORECAST.ETS.STAT($B$2:$B$17,$A$2:$A$17,1,1,1)");
    // Future timeline and the exact continuation of the seasonal series (t = 17 → 10 + 34 + 3).
    assert_eq!(value(&s, 0, "A18"), Value::Number(17.0));
    assert_eq!(value(&s, 0, "A21"), Value::Number(20.0));
    let c18 = value(&s, 0, "C18").as_f64().unwrap();
    assert!((c18 - 47.0).abs() < 1e-6, "{c18}");
    let (lo, hi) = (value(&s, 0, "D19").as_f64().unwrap(), value(&s, 0, "E19").as_f64().unwrap());
    assert!(lo <= hi);
    assert_eq!(sh.tables.first().map(|t| t.range.a1()), Some("A1:E21".into()));
    let chart = sh.charts.first().unwrap();
    assert_eq!(chart.series.len(), 4);
    assert_eq!(chart.series[1].values, format!("{}!$C$2:$C$21", r["sheet"].as_str().unwrap()));
    // One undo step removes the sheet again.
    let sheets = s.doc().unwrap().wb.sheets.len();
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.doc().unwrap().wb.sheets.len(), sheets - 1);
}

#[test]
fn history_and_periods_are_capped() {
    use super::{MAX_HISTORY, MAX_PERIODS, MAX_WORK, default_periods, max_periods};
    // The default (a third of the history) never exceeds the caps.
    assert_eq!(default_periods(16), 6);
    assert_eq!(default_periods(1_200), 400);
    assert_eq!(default_periods(3_000), 166);
    assert_eq!(default_periods(100_000), 5);
    assert_eq!(max_periods(10), MAX_PERIODS);
    assert_eq!(max_periods(0), MAX_PERIODS);
    for h in [1usize, 2, 3, 50, 51, 1_000, 1_300, 3_000, 10_000, 100_000, usize::MAX] {
        let (d, m) = (default_periods(h), max_periods(h));
        assert!(d >= 1 && d <= m && m <= MAX_PERIODS, "{h}: {d} {m}");
        assert!(u64::try_from(h).unwrap_or(u64::MAX).saturating_mul(u64::from(m)) <= MAX_WORK.max(u64::try_from(h).unwrap_or(u64::MAX)), "{h}");
    }
    // Explicit periods or an end past the cap are refused before anything is built.
    let mut s = Session::new();
    s.new_workbook();
    let mut rows = vec![json!(["t", "y"])];
    rows.extend((1..=1_300).map(|t| json!([t, (t % 7) as f64 + t as f64 * 0.5])));
    s.execute("range.setValues", json!({"range": "A1", "values": rows})).unwrap();
    let sheets = s.doc().unwrap().wb.sheets.len();
    let cap = max_periods(1_300); // 384
    let err = |r: crate::Result<serde_json::Value>| r.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(err(s.execute("data.forecastSheet", json!({"range": "A1:B1301", "periods": cap + 1}))).contains("from 1 to 384"));
    assert!(err(s.execute("data.forecastSheet", json!({"range": "A1:B1301", "end": 1_300 + cap + 1}))).contains("at most 384"));
    assert!(s.execute("data.forecastSheet", json!({"range": "A1:B1301", "periods": 10_000})).is_err());
    assert_eq!(s.doc().unwrap().wb.sheets.len(), sheets);
    // More history rows than MAX_HISTORY are refused.
    let big = MAX_HISTORY + 2;
    let r = s.execute("data.forecastSheet", json!({"timeline": format!("A1:A{big}"), "values": format!("B1:B{big}")}));
    assert!(r.is_err());
}

#[test]
fn forecast_sheet_parameters_and_errors() {
    let mut s = session_with_history();
    let r = s
        .execute(
            "data.forecastSheet",
            json!({"timeline": "A2:A17", "values": "B2:B17", "end": "22", "confidenceInterval": false, "seasonality": 4, "fillMissing": "zeros", "aggregate": "sum", "chart": "column"}),
        )
        .unwrap();
    assert_eq!(r["periods"], 6);
    assert_eq!(value(&s, 0, "B1"), Value::text("Values"));
    assert_eq!(formula(&s, 0, "C18"), "FORECAST.ETS(A18,$B$2:$B$17,$A$2:$A$17,4,0,7)");
    assert_eq!(value(&s, 0, "D1"), Value::Empty);
    let sh = s.doc().unwrap().wb.sheet(0).unwrap();
    assert_eq!(sh.charts.first().map(|c| c.kind), Some(joysheet_model::ChartKind::ColumnClustered));
    // Errors: end before the data, bad seasonality, uneven timeline, one column only.
    s.execute("sheet.activate", json!({"sheet": "Sheet1"})).unwrap();
    assert!(s.execute("data.forecastSheet", json!({"range": "Sheet1!A1:B17", "end": 3})).is_err());
    assert!(s.execute("data.forecastSheet", json!({"range": "Sheet1!A1:B17", "seasonality": 9000})).is_err());
    assert!(s.execute("data.forecastSheet", json!({"range": "Sheet1!B1:B17"})).is_err());
    s.execute("cell.set", json!({"cell": "A5", "input": "4.5"})).unwrap();
    assert!(s.execute("data.forecastSheet", json!({"range": "Sheet1!A1:B17"})).is_err());
}

#[test]
fn forecast_start_hindcasts_over_the_actuals() {
    let mut s = session_with_history(); // t = 1..16 in A2:A17
    let r = s.execute("data.forecastSheet", json!({"range": "A1:B17", "forecastStart": 13, "periods": 2, "confidenceInterval": false})).unwrap();
    assert_eq!(r["fitted"], 12);
    // Fit to t = 1..12 (rows 2..13); the line joins at row 13 and forecasts rows 14..19.
    assert_eq!(formula(&s, 0, "C13"), "B13");
    assert_eq!(value(&s, 0, "C13"), value(&s, 0, "B13"));
    assert_eq!(formula(&s, 0, "C14"), "FORECAST.ETS(A14,$B$2:$B$13,$A$2:$A$13,1,1)");
    assert_eq!(formula(&s, 0, "C19"), "FORECAST.ETS(A19,$B$2:$B$13,$A$2:$A$13,1,1)");
    // The exact seasonal series is recovered, and the actual is still shown next to it.
    let c14 = value(&s, 0, "C14").as_f64().unwrap();
    assert!((c14 - 39.0).abs() < 1e-6, "{c14}"); // t = 13: 10 + 26 + 3
    assert!(value(&s, 0, "B14").as_f64().is_some());
    // Start at the last point or one step later: as without forecastStart.
    for start in [json!(16), json!(17)] {
        let r = s.execute("data.forecastSheet", json!({"range": "Sheet1!A1:B17", "forecastStart": start, "periods": 2})).unwrap();
        assert_eq!(r["fitted"], 16);
    }
    assert_eq!(formula(&s, 0, "C18"), "FORECAST.ETS(A18,$B$2:$B$17,$A$2:$A$17,1,1)");
}

#[test]
fn forecast_start_invalid_values_are_errors() {
    let mut s = session_with_history();
    let sheets = s.doc().unwrap().wb.sheets.len();
    for bad in [json!(0), json!(2), json!(3.5), json!(18), json!(1000), json!("abc"), json!([1]), json!(1e308)] {
        assert!(s.execute("data.forecastSheet", json!({"range": "A1:B17", "forecastStart": bad})).is_err(), "{bad}");
    }
    assert_eq!(s.doc().unwrap().wb.sheets.len(), sheets);
    // Blank or null behaves like no start; empty params unchanged.
    let r = s.execute("data.forecastSheet", json!({"range": "A1:B17", "forecastStart": ""})).unwrap();
    assert_eq!(r["fitted"], 16);
}
