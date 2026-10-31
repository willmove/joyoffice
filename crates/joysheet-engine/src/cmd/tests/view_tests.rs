use super::*;

#[test]
fn zoom_100_resets_the_active_sheet_zoom() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("view.zoom", json!({"percent": 150})).unwrap();
    assert_eq!(s.execute("view.zoom100", json!({})).unwrap()["zoom"], 100);
    assert_eq!(s.doc().unwrap().wb.active().unwrap().zoom, 100);
}

#[test]
fn formula_bar_toggles_without_a_workbook_and_defaults_to_shown() {
    let mut s = Session::new();
    assert!(s.prefs.formula_bar);
    assert_eq!(s.execute("view.formulaBar", json!({})).unwrap()["on"], false);
    assert_eq!(s.execute("view.formulaBar", json!({"on": true})).unwrap()["on"], true);
    // A prefs.json saved before the option existed keeps the formula bar shown.
    let old: crate::Prefs = serde_json::from_str(r#"{"userName": "Ann"}"#).unwrap();
    assert!(old.formula_bar);
}

#[test]
fn status_bar_statistics_default_to_average_count_and_sum_and_turn_on_one_at_a_time() {
    let mut s = Session::new();
    let all = json!({"average": true, "count": true, "numericalCount": false, "minimum": false, "maximum": false, "sum": true});
    assert_eq!(s.execute("view.statusBar", json!({})).unwrap(), all);
    let r = s.execute("view.statusBar", json!({"minimum": true, "sum": false, "count": "yes"})).unwrap();
    assert_eq!(r, json!({"average": true, "count": true, "numericalCount": false, "minimum": true, "maximum": false, "sum": false}));
    assert!(s.prefs.status_bar.minimum && !s.prefs.status_bar.sum);
    // A prefs.json saved before the option existed, or with only some of the six, keeps the defaults for the rest.
    let old: crate::Prefs = serde_json::from_str(r#"{"userName": "Ann"}"#).unwrap();
    assert_eq!(old.status_bar, crate::StatusBarStats::default());
    let part: crate::Prefs = serde_json::from_str(r#"{"statusBar": {"maximum": true}}"#).unwrap();
    assert!(part.status_bar.maximum && part.status_bar.average && part.status_bar.sum && !part.status_bar.minimum);
    let saved = serde_json::to_value(&s.prefs).unwrap();
    assert_eq!(saved["statusBar"]["minimum"], true);
    assert_eq!(saved["statusBar"]["numericalCount"], false);
}
