use joysheet_core::{CellRef, Value};
use serde_json::json;

use crate::Session;

fn s() -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s
}
fn v(s: &Session, a: &str) -> Value {
    let d = s.doc().unwrap();
    let (sh, body) = crate::cmd::split_sheet(a);
    let si = sh.map(|n| d.wb.sheet_index(&n).unwrap()).unwrap_or(d.wb.active_sheet());
    d.wb.sheet(si).unwrap().value(CellRef::parse(body).unwrap())
}
fn n(s: &Session, a: &str) -> f64 {
    v(s, a).as_f64().unwrap_or(f64::NAN)
}
fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}
/// Evaluates a formula in a scratch cell.
fn eval(s: &mut Session, f: &str) -> f64 {
    s.execute("cell.set", json!({"cell": "Z1000", "input": f})).unwrap();
    let x = n(s, "Z1000");
    s.execute("cell.set", json!({"cell": "Z1000", "input": ""})).unwrap();
    x
}

#[test]
fn forecast_standard_errors_match_the_full_window_rescan() {
    // #1043: the running sum must give what re-adding every window gave, gaps included.
    let x: [f64; 11] = [2.0, 4.0, 5.0, 4.0, 5.0, 7.0, 1.0, 3.0, 8.0, 6.0, 2.0];
    let f = [None, Some(3.0), Some(4.5), None, Some(4.0), Some(6.5), Some(2.0), Some(2.5), Some(7.0), Some(5.0), Some(4.0)];
    let rescan = |i: usize, w: usize| -> Value {
        let v = (i + 1).checked_sub(w).and_then(|s0| {
            let mut acc = 0.0;
            for j in s0..=i {
                acc += (x.get(j)? - (*f.get(j)?)?).powi(2);
            }
            Some((acc / w as f64).sqrt())
        });
        v.map(Value::number).unwrap_or(Value::Error(joysheet_core::CellError::NA))
    };
    for w in 1..=5 {
        let g = super::forecast_grid(&x, &f, w, &json!({"standardErrors": true}));
        for i in 0..f.len() {
            let (got, want) = (g.cells.get(&(i as u32, 1)).cloned().unwrap_or(Value::Empty), rescan(i, w));
            match (got.as_f64(), want.as_f64()) {
                (Some(a), Some(b)) => assert!(close(a, b, 1e-12), "window {w}, row {i}: {a} vs {b}"),
                _ => assert_eq!(got, want, "window {w}, row {i}"),
            }
        }
    }
    // Row 3 has no forecast: windows through it are #N/A, the next full window after it is not.
    let g = super::forecast_grid(&x, &f, 3, &json!({"standardErrors": true}));
    assert_eq!(g.cells.get(&(5, 1)), Some(&Value::Error(joysheet_core::CellError::NA)));
    assert!(g.cells.get(&(6, 1)).and_then(Value::as_f64).is_some());
}

#[test]
fn descriptive_statistics_of_one_to_five() {
    // 1..5: mean 3, s = √2.5, kurtosis −1.2, skewness 0 (textbook values).
    let mut s = s();
    s.execute("range.setValues", json!({"range": "A1", "values": [["Score"], [1], [2], [3], [4], [5]]})).unwrap();
    let r = s
        .execute("data.analysisToolPak", json!({"tool": "Descriptive Statistics", "range": "A1:A6", "labels": true, "output": "C1", "confidence": 95, "kthLargest": 2, "kthSmallest": 1}))
        .unwrap();
    assert_eq!(r["range"], "C1:D18");
    assert_eq!(v(&s, "C1"), Value::text("Score"));
    assert_eq!(v(&s, "C3"), Value::text("Mean"));
    assert_eq!(n(&s, "D3"), 3.0);
    assert!(close(n(&s, "D4"), (2.5f64 / 5.0).sqrt(), 1e-12));
    assert_eq!(n(&s, "D5"), 3.0);
    assert_eq!(v(&s, "D6"), Value::Error(joysheet_core::CellError::NA));
    assert!(close(n(&s, "D7"), 2.5f64.sqrt(), 1e-12));
    assert_eq!(n(&s, "D8"), 2.5);
    assert!(close(n(&s, "D9"), -1.2, 1e-12));
    assert!(close(n(&s, "D10"), 0.0, 1e-12));
    assert_eq!((n(&s, "D11"), n(&s, "D12"), n(&s, "D13"), n(&s, "D14"), n(&s, "D15")), (4.0, 1.0, 5.0, 15.0, 5.0));
    assert_eq!(v(&s, "C16"), Value::text("Largest(2)"));
    assert_eq!(n(&s, "D16"), 4.0);
    assert_eq!(n(&s, "D17"), 1.0);
    assert_eq!(v(&s, "C18"), Value::text("Confidence Level(95.0%)"));
    let want = eval(&mut s, "=CONFIDENCE.T(0.05,STDEV.S(A2:A6),5)");
    assert!(close(n(&s, "D18"), want, 1e-9), "{} vs {want}", n(&s, "D18"));
    // The output range now holds data: a second run is refused unless it may overwrite.
    let again = json!({"tool": "descriptive", "range": "A2:A6", "output": "C1"});
    let e = s.execute("data.analysisToolPak", again.clone()).unwrap_err().to_string();
    assert!(e.contains(super::OVERWRITE_ERROR), "{e}");
    let mut again = again;
    again["overwrite"] = json!(true);
    s.execute("data.analysisToolPak", again).unwrap();
    assert_eq!(v(&s, "C1"), Value::text("Column1"));
}

#[test]
fn regression_matches_textbook_and_linest() {
    // x = 1..5, y = 2,4,5,4,5: b = 0.6, a = 2.2, R² = 0.6, F = 4.5 (textbook example).
    let mut s = s();
    s.execute("range.setValues", json!({"range": "A1", "values": [["y", "x"], [2, 1], [4, 2], [5, 3], [4, 4], [5, 5]]})).unwrap();
    s.execute(
        "data.analysisToolPak",
        json!({"tool": "regression", "inputY": "A1:A6", "inputX": "B1:B6", "labels": true, "newSheet": "Fit", "residuals": true, "standardizedResiduals": true}),
    )
    .unwrap();
    assert!(close(n(&s, "Fit!B5"), 0.6, 1e-12), "R²");
    assert!(close(n(&s, "Fit!B6"), 1.0 - 0.4 * 4.0 / 3.0, 1e-12), "adjusted R²");
    assert!(close(n(&s, "Fit!B7"), 0.8f64.sqrt(), 1e-12), "standard error");
    assert!(close(n(&s, "Fit!E12"), 4.5, 1e-12), "F");
    assert!(close(n(&s, "Fit!B17"), 2.2, 1e-12), "intercept");
    assert!(close(n(&s, "Fit!B18"), 0.6, 1e-12), "slope");
    assert_eq!(v(&s, "Fit!A18"), Value::text("x"));
    assert!(close(n(&s, "Fit!C18"), (0.8f64 / 10.0).sqrt(), 1e-12), "SE of slope");
    // Simple regression: Significance F equals the slope's p-value.
    assert!(close(n(&s, "Fit!F12"), n(&s, "Fit!E18"), 1e-9));
    assert!(n(&s, "Fit!E18") > 0.12 && n(&s, "Fit!E18") < 0.13);
    assert_eq!(v(&s, "Fit!A22"), Value::text("RESIDUAL OUTPUT"));
    assert!(close(n(&s, "Fit!B25"), 2.8, 1e-12), "first prediction");
    assert!(close(n(&s, "Fit!C25"), -0.8, 1e-12), "first residual");
    // Multiple regression agrees with LINEST.
    s.execute(
        "range.setValues",
        json!({"range": "Sheet1!D1", "values": [[1, 2, 3], [3, 1, 5], [4, 5, 2], [7, 3, 4], [6, 6, 1], [9, 4, 6], [8, 7, 3]]}),
    )
    .unwrap();
    s.execute("data.analysisToolPak", json!({"tool": "regression", "inputY": "Sheet1!D1:D7", "inputX": "Sheet1!E1:F7", "newSheet": "Multi"}))
        .unwrap();
    let slope2 = eval(&mut s, "=INDEX(LINEST(Sheet1!D1:D7,Sheet1!E1:F7,TRUE,TRUE),1,1)");
    let slope1 = eval(&mut s, "=INDEX(LINEST(Sheet1!D1:D7,Sheet1!E1:F7,TRUE,TRUE),1,2)");
    let se1 = eval(&mut s, "=INDEX(LINEST(Sheet1!D1:D7,Sheet1!E1:F7,TRUE,TRUE),2,2)");
    let icpt = eval(&mut s, "=INDEX(LINEST(Sheet1!D1:D7,Sheet1!E1:F7,TRUE,TRUE),1,3)");
    assert!(close(n(&s, "Multi!B17"), icpt, 1e-9));
    assert!(close(n(&s, "Multi!B18"), slope1, 1e-9));
    assert!(close(n(&s, "Multi!B19"), slope2, 1e-9));
    assert!(close(n(&s, "Multi!C18"), se1, 1e-9));
    assert_eq!(v(&s, "Multi!A19"), Value::text("X Variable 2"));
}

#[test]
fn tests_of_means_and_variances_match_worksheet_functions() {
    let mut s = s();
    s.execute("range.setValues", json!({"range": "A1", "values": [[12, 10], [15, 14], [11, 9], [18, 15], [14, 13], [16, 12]]})).unwrap();
    s.execute("data.analysisToolPak", json!({"tool": "tTestEqual", "range1": "A1:A6", "range2": "B1:B6", "output": "D1"})).unwrap();
    assert_eq!(v(&s, "D1"), Value::text("t-Test: Two-Sample Assuming Equal Variances"));
    assert_eq!(n(&s, "E9"), 10.0, "df");
    let p2 = eval(&mut s, "=T.TEST(A1:A6,B1:B6,2,2)");
    assert!(close(n(&s, "E13"), p2, 1e-9), "two-tail p");
    s.execute("data.analysisToolPak", json!({"tool": "tTestPaired", "range1": "A1:A6", "range2": "B1:B6", "output": "H1"})).unwrap();
    let pp = eval(&mut s, "=T.TEST(A1:A6,B1:B6,2,1)");
    assert_eq!(v(&s, "H13"), Value::text("P(T<=t) two-tail"));
    assert!(close(n(&s, "I13"), pp, 1e-9), "paired two-tail p");
    s.execute("data.analysisToolPak", json!({"tool": "fTest", "range1": "A1:A6", "range2": "B1:B6", "output": "L1"})).unwrap();
    let fp = eval(&mut s, "=F.TEST(A1:A6,B1:B6)");
    assert!(close(n(&s, "M9") * 2.0, fp, 1e-9), "one-tail p is half of F.TEST");
    // Anova on {1,2,3}, {4,5,6}, {7,8,9}: SSB 54, SSW 6, F 27.
    s.execute("range.setValues", json!({"range": "A20", "values": [[1, 4, 7], [2, 5, 8], [3, 6, 9]]})).unwrap();
    s.execute("data.analysisToolPak", json!({"tool": "anovaSingle", "range": "A20:C22", "output": "E20"})).unwrap();
    assert_eq!(v(&s, "E31"), Value::text("Between Groups"));
    assert_eq!((n(&s, "F31"), n(&s, "F32"), n(&s, "I31")), (54.0, 6.0, 27.0));
    let p = eval(&mut s, "=F.DIST.RT(27,2,6)");
    assert!(close(n(&s, "J31"), p, 1e-12));
}

#[test]
fn histogram_matrices_smoothing_and_ranks() {
    let mut s = s();
    s.execute("range.setValues", json!({"range": "A1", "values": [[1], [2], [2], [3], [5], [7]]})).unwrap();
    s.execute("range.setValues", json!({"range": "B1", "values": [[2], [4], [6]]})).unwrap();
    s.execute(
        "data.analysisToolPak",
        json!({"tool": "histogram", "range": "A1:A6", "bins": "B1:B3", "output": "D1", "cumulative": true, "pareto": true}),
    )
    .unwrap();
    assert_eq!((n(&s, "E2"), n(&s, "E3"), n(&s, "E4"), n(&s, "E5")), (3.0, 1.0, 1.0, 1.0));
    assert_eq!(v(&s, "D5"), Value::text("More"));
    assert_eq!(n(&s, "F5"), 1.0);
    assert_eq!((n(&s, "G2"), n(&s, "H2")), (2.0, 3.0), "Pareto puts the largest bin first");
    // Correlation and covariance against CORREL and COVARIANCE.P.
    s.execute("range.setValues", json!({"range": "J1", "values": [[1, 2], [2, 4], [3, 5], [4, 4], [5, 5]]})).unwrap();
    s.execute("data.analysisToolPak", json!({"tool": "correlation", "range": "J1:K5", "output": "M1"})).unwrap();
    assert_eq!(n(&s, "N2"), 1.0);
    assert!(close(n(&s, "N3"), eval(&mut s, "=CORREL(J1:J5,K1:K5)"), 1e-12));
    assert!(v(&s, "O2").is_empty(), "upper triangle is blank");
    s.execute("data.analysisToolPak", json!({"tool": "covariance", "range": "J1:K5", "output": "M10"})).unwrap();
    assert!(close(n(&s, "N12"), eval(&mut s, "=COVARIANCE.P(J1:J5,K1:K5)"), 1e-12));
    assert!(close(n(&s, "N11"), eval(&mut s, "=VAR.P(J1:J5)"), 1e-12));
    // Moving average of 3 and exponential smoothing with damping 0.3.
    s.execute("data.analysisToolPak", json!({"tool": "movingAverage", "range": "K1:K5", "output": "Q1", "standardErrors": true})).unwrap();
    assert_eq!(v(&s, "Q2"), Value::Error(joysheet_core::CellError::NA));
    assert!(close(n(&s, "Q3"), 11.0 / 3.0, 1e-12));
    assert!(close(n(&s, "Q5"), 14.0 / 3.0, 1e-12));
    assert_eq!(v(&s, "R4"), Value::Error(joysheet_core::CellError::NA));
    assert!(n(&s, "R5").is_finite());
    s.execute("data.analysisToolPak", json!({"tool": "exponentialSmoothing", "range": "K1:K5", "output": "S1"})).unwrap();
    assert_eq!(n(&s, "S2"), 2.0);
    assert!(close(n(&s, "S3"), 0.7 * 4.0 + 0.3 * 2.0, 1e-12));
    // Rank and Percentile of 2,4,5,4,5: the two 5s share rank 1 (3 of 4 others below them).
    s.execute("data.analysisToolPak", json!({"tool": "rankPercentile", "range": "K1:K5", "output": "U1"})).unwrap();
    assert_eq!((n(&s, "U2"), n(&s, "V2"), n(&s, "W2"), n(&s, "X2")), (3.0, 5.0, 1.0, 0.75));
    assert_eq!((n(&s, "U3"), n(&s, "W3")), (5.0, 1.0));
    assert_eq!((n(&s, "V6"), n(&s, "X6")), (2.0, 0.0));
    assert_eq!(n(&s, "X4"), 0.25, "4 has one value below it among five");
}

#[test]
fn hostile_inputs_are_refused_without_panicking() {
    let mut s = s();
    let r = s.execute("data.analysisToolPak", json!({})).unwrap();
    assert!(r["tools"].as_array().is_some_and(|t| t.len() == super::TOOLS.len()));
    assert!(s.take_ui_requests().iter().any(|q| matches!(q, crate::UiRequest::Dialog(n, _) if n == "analysisToolPak")));
    s.execute("range.setValues", json!({"range": "A1", "values": [[1, "x"], [2, 3]]})).unwrap();
    for p in [
        json!({"tool": "nope"}),
        json!({"tool": "descriptive"}),
        json!({"tool": "descriptive", "range": "A1:XFD1048576"}),
        json!({"tool": "descriptive", "range": "A1:B2"}),
        json!({"tool": "descriptive", "range": "A1:A2", "output": "XFD1048576"}),
        json!({"tool": "descriptive", "range": "A1:A2", "newSheet": "bad[name]"}),
        json!({"tool": "regression", "inputY": "A1:A2", "inputX": "A1:A2"}),
        json!({"tool": "regression", "inputY": "A1:A2", "inputX": "A1:Q2"}),
        json!({"tool": "movingAverage", "range": "A1:A2", "interval": 1e300}),
        json!({"tool": "movingAverage", "range": "A1:A2", "interval": -3}),
        json!({"tool": "exponentialSmoothing", "range": "A1:A2", "damping": 7}),
        json!({"tool": "histogram", "range": "C1:C3"}),
        json!({"tool": "tTestPaired", "range1": "A1", "range2": "A2"}),
        json!({"tool": "anovaSingle", "range": "A1:A2"}),
        json!({"tool": "correlation", "range": "A1:B2", "alpha": "x", "groupedBy": 5}),
        json!({"tool": "descriptive", "range": "A1:A2", "confidence": 250}),
    ] {
        assert!(s.execute("data.analysisToolPak", p.clone()).is_err(), "{p}");
    }
    // Collinear X columns are an error, not a NaN report.
    s.execute("range.setValues", json!({"range": "D1", "values": [[1, 1, 2], [2, 2, 4], [3, 3, 6], [5, 4, 8]]})).unwrap();
    assert!(s.execute("data.analysisToolPak", json!({"tool": "regression", "inputY": "D1:D4", "inputX": "E1:F4"})).is_err());
    // Constant data gives #DIV/0! cells, not a crash.
    s.execute("range.setValues", json!({"range": "H1", "values": [[4], [4], [4]]})).unwrap();
    s.execute("data.analysisToolPak", json!({"tool": "descriptive", "range": "H1:H3", "output": "J1"})).unwrap();
    assert_eq!(v(&s, "K9"), Value::Error(joysheet_core::CellError::Div0));
}
