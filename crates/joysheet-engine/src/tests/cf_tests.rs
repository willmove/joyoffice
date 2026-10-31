use serde_json::json;

use super::*;

#[test]
fn every_matching_rule_applies() {
    let mut s = crate::Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [[200], [400], [510]]})).unwrap();
    let rule =
        |op: &str, value: &str, preset: &str| json!({"range": "A1:A3", "rule": {"type": "cellIs", "operator": op, "value": value, "preset": preset}});
    s.execute("home.conditionalFormat", rule("greaterThan", "300", "lightRedFill")).unwrap();
    s.execute("home.conditionalFormat", rule("greaterThan", "500", "redBorder")).unwrap();
    let d = s.doc().unwrap();
    let mut cache = CfCache::new();
    let mut look = |a: &str| cache.look(&d.wb, 0, CellRef::parse(a).unwrap()).and_then(|l| l.style);
    assert!(look("A1").is_none());
    let only_fill = look("A2").unwrap();
    assert_eq!(only_fill.fill.pattern, PatternType::Solid);
    assert!(only_fill.borders.top.is_none());
    let both = look("A3").unwrap();
    assert_eq!(both.fill, only_fill.fill);
    assert!(!both.borders.top.is_none());
}

#[test]
fn cell_value_rules_on_blanks_and_errors() {
    let mut s = crate::Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [[3, "=NA()"], [null, 7], ["=NA()", null], [" ", null], [7, null]]})).unwrap();
    let rule = |range: &str, r: serde_json::Value| json!({"range": range, "rule": r});
    s.execute("home.conditionalFormat", rule("A1:A5", json!({"type": "cellIs", "operator": "less", "value": "5", "preset": "lightRedFill"})))
        .unwrap();
    s.execute("home.conditionalFormat", rule("A1:A5", json!({"type": "noBlanks", "preset": "redBorder"}))).unwrap();
    s.execute("home.conditionalFormat", rule("B1:B2", json!({"type": "cellIs", "operator": "greater", "value": "5", "preset": "lightRedFill"})))
        .unwrap();
    let d = s.doc().unwrap();
    let mut cache = CfCache::new();
    let mut look = |a: &str| cache.look(&d.wb, 0, CellRef::parse(a).unwrap()).and_then(|l| l.style);
    let fill = |st: Option<Style>| st.is_some_and(|st| st.fill.pattern == PatternType::Solid);
    let border = |st: Option<Style>| st.is_some_and(|st| !st.borders.top.is_none());
    // Less than 5: 3 and the blank cell (as 0), not #N/A.
    assert_eq!([look("A1"), look("A2"), look("A3"), look("A5")].map(fill), [true, true, false, false]);
    // Greater than 5: 7, not #N/A.
    assert_eq!([look("B1"), look("B2")].map(fill), [false, true]);
    // No blanks: neither the empty cell nor the one holding a space.
    assert_eq!([look("A1"), look("A2"), look("A4")].map(border), [true, false, false]);
}

/// The statistics as every rule used to get them: one pass collecting both parts.
fn all_stats_before(sh: &Sheet, ranges: &[RangeRef]) -> RuleStats {
    let mut nums = Vec::new();
    let mut counts = HashMap::new();
    for r in ranges {
        let Some(r) = sh.used_range().and_then(|u| r.intersection(&u)) else { continue };
        for c in r.iter() {
            let v = sh.value(c);
            if let Value::Number(n) = v {
                nums.push(n);
            }
            if !v.is_empty() {
                *counts.entry(v.display().to_lowercase()).or_insert(0) += 1;
            }
        }
    }
    nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = nums.len().max(1) as f64;
    let avg = nums.iter().sum::<f64>() / n;
    let std = (nums.iter().map(|x| (x - avg).powi(2)).sum::<f64>() / n).sqrt();
    RuleStats { sorted: nums, avg, std, counts }
}

#[test]
fn needed_statistics_equal_the_full_ones() {
    let mut s = crate::Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [[3, "x"], [7, "X"], [3, null], ["=NA()", 2.5], [" ", 9], [-1, "y"]]})).unwrap();
    let d = s.doc().unwrap();
    let sh = d.wb.sheet(0).unwrap();
    for ranges in ["A1:B6", "A1:A3", "B2:B9", "D1:E4", "A1:A2"].map(|r| vec![RangeRef::parse(r).unwrap()]) {
        let full = all_stats_before(sh, &ranges);
        let numbers = RuleStats::build(sh, &ranges, StatsNeed::Numbers);
        assert_eq!((numbers.sorted, numbers.avg.to_bits(), numbers.std.to_bits()), (full.sorted, full.avg.to_bits(), full.std.to_bits()));
        assert_eq!(RuleStats::build(sh, &ranges, StatsNeed::Counts).counts, full.counts);
    }
}

#[test]
fn rules_that_read_statistics_still_match() {
    let mut s = crate::Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [[1, "a"], [2, "A"], [3, "b"], [10, "c"], [2, null]]})).unwrap();
    let rule = |range: &str, r: serde_json::Value| json!({"range": range, "rule": r});
    s.execute("home.conditionalFormat", rule("B1:B5", json!({"type": "duplicate", "preset": "lightRedFill"}))).unwrap();
    s.execute("home.conditionalFormat", rule("A1:A5", json!({"type": "top10", "rank": 1, "preset": "redBorder"}))).unwrap();
    s.execute("home.conditionalFormat", rule("A1:A5", json!({"type": "colorScale"}))).unwrap();
    let d = s.doc().unwrap();
    let mut cache = CfCache::new();
    let mut look = |a: &str| cache.look(&d.wb, 0, CellRef::parse(a).unwrap());
    let fill = |l: Option<CfLook>| l.and_then(|l| l.style).is_some_and(|st| st.fill.pattern == PatternType::Solid);
    // Duplicates are case-insensitive texts: "a" and "A".
    assert_eq!(["B1", "B2", "B3", "B4", "B5"].map(|a| fill(look(a))), [true, true, false, false, false]);
    // The top value gets the border, the scale runs from the minimum to the maximum.
    assert!(look("A4").and_then(|l| l.style).is_some_and(|st| !st.borders.top.is_none()));
    assert!(look("A1").and_then(|l| l.style).is_none_or(|st| st.borders.top.is_none()));
    assert_eq!(look("A1").and_then(|l| l.scale_fill), Some([0xF8, 0x69, 0x6B]));
    assert_eq!(look("A4").and_then(|l| l.scale_fill), Some([0x63, 0xBE, 0x7B]));
}

/// Looking cells up does work in proportion to the cells asked for, not the rule's range: the
/// statistics are only gathered for rules that read them, and once per rule.
#[test]
fn lookups_do_not_scan_the_range_for_rules_without_statistics() {
    let mut s = crate::Session::new();
    s.new_workbook();
    let rows: Vec<serde_json::Value> = (1..=10_000).map(|i| json!([i, i % 7])).collect();
    s.execute("range.setValues", json!({"range": "A1", "values": rows})).unwrap();
    s.execute(
        "home.conditionalFormat",
        json!({"range": "A1:B10000", "rule": {"type": "expression", "formula": "=MOD(ROW(),2)=0", "preset": "lightRedFill"}}),
    )
    .unwrap();
    s.execute(
        "home.conditionalFormat",
        json!({"range": "A1:B10000", "rule": {"type": "cellIs", "operator": "greater", "value": "5", "preset": "redBorder"}}),
    )
    .unwrap();
    s.execute("home.conditionalFormat", json!({"range": "A1:B10000", "rule": {"type": "noBlanks", "preset": "redBorder"}})).unwrap();
    let d = s.doc().unwrap();
    let mut cache = CfCache::new();
    SCANNED.with(|n| n.set(0));
    for r in 1..=50 {
        for col in ["A", "B"] {
            cache.look(&d.wb, 0, CellRef::parse(&format!("{col}{r}")).unwrap());
        }
    }
    assert_eq!(SCANNED.with(|n| n.get()), 0);
    // A rule that does read them scans its range once, however many cells are looked up.
    s.execute("home.conditionalFormat", json!({"range": "A1:A10000", "rule": {"type": "top10", "preset": "redBorder"}})).unwrap();
    let d = s.doc().unwrap();
    let mut cache = CfCache::new();
    SCANNED.with(|n| n.set(0));
    for r in 1..=50 {
        cache.look(&d.wb, 0, CellRef::parse(&format!("A{r}")).unwrap());
    }
    assert_eq!(SCANNED.with(|n| n.get()), 10_000);
}

#[test]
fn cell_value_operator_names() {
    let mut s = crate::Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [[3], [7]]})).unwrap();
    let rule = |op: &str| json!({"range": "A1:A2", "rule": {"type": "cellIs", "operator": op, "value": "5", "preset": "lightRedFill"}});
    s.execute("home.conditionalFormat", rule("lessThan")).unwrap();
    assert!(s.execute("home.conditionalFormat", rule("smaller")).is_err());
    let d = s.doc().unwrap();
    let mut cache = CfCache::new();
    let mut look = |a: &str| cache.look(&d.wb, 0, CellRef::parse(a).unwrap()).and_then(|l| l.style).is_some();
    assert_eq!([look("A1"), look("A2")], [true, false]);
}

#[test]
fn text_rules_skip_error_values() {
    let mut s = crate::Session::new();
    s.new_workbook();
    let values = json!([["=NA()", "=NA()", "=1/0", "=NA()"], ["abc", "#x", "x!", "abc"], ["xyz", "y", "y", "xyz"]]);
    s.execute("range.setValues", json!({"range": "A1", "values": values})).unwrap();
    let rule = |range: &str, kind: &str, text: &str| json!({"range": range, "rule": {"type": kind, "text": text, "preset": "lightRedFill"}});
    s.execute("home.conditionalFormat", rule("A1:A3", "containsText", "a")).unwrap();
    s.execute("home.conditionalFormat", rule("B1:B3", "beginsWith", "#")).unwrap();
    s.execute("home.conditionalFormat", rule("C1:C3", "endsWith", "!")).unwrap();
    s.execute("home.conditionalFormat", rule("D1:D3", "notContainsText", "a")).unwrap();
    let d = s.doc().unwrap();
    let mut cache = CfCache::new();
    let mut look = |a: &str| cache.look(&d.wb, 0, CellRef::parse(a).unwrap()).and_then(|l| l.style).is_some();
    // Contains "a": abc, never #N/A although its text holds an "a".
    assert_eq!([look("A1"), look("A2"), look("A3")], [false, true, false]);
    // Begins with "#": #x, never #N/A.
    assert_eq!([look("B1"), look("B2"), look("B3")], [false, true, false]);
    // Ends with "!": x!, never #DIV/0!.
    assert_eq!([look("C1"), look("C2"), look("C3")], [false, true, false]);
    // Does not contain "a": #N/A and xyz, not abc.
    assert_eq!([look("D1"), look("D2"), look("D3")], [true, false, true]);
}
