use super::*;

fn p(s: &str) -> Parsed {
    parse_input(s, DateSystem::D1900)
}

/// #2163: digits too many for a finite number don't make one (∞/∞ was NaN).
#[test]
fn overlong_digits_stay_text() {
    let big = "9".repeat(400);
    for s in [format!("1 {big}/{big}"), format!("1 {big}/2"), format!("1 1/{big}0"), format!("{big}:00"), format!("1:00:{big}.5")] {
        assert_eq!(p(&s).value, Value::text(s.as_str()), "{s}");
        assert_eq!(parse_number_text(&s), None, "{s}");
    }
    assert_eq!(parse_number_text("1 1/2"), Some(1.5));
}

#[test]
fn overlong_time_fields_stay_text() {
    // #2168: all-digit field lengths are gated before allocation/derivation,
    // so hostile strings never spin the time-derivation machinery.
    let big = "9".repeat(500);
    for s in [format!("{big}:00"), format!("1:00:{big}"), format!("{big}:{big}:{big}"), format!("1:0{big}"), format!("00:00:{big}.1")] {
        assert_eq!(p(&s).value, Value::text(s.as_str()), "{s}");
        assert_eq!(parse_number_text(&s), None, "{s}");
    }
    // Within the gate, times still work.
    assert_eq!(p("01:02:03").value, Value::number(3.0 / 86_400.0 + 1.0 / 24.0 + 2.0 / 1440.0));
    assert_eq!(parse_number_text("13:14:15"), Some(0.5515625));
}

#[test]
fn non_digit_time_and_fraction_parts_stay_text() {
    for s in ["1:nan", "nan:30", "1:30:NaN", "1 nan/2", "1 1/nan", "inf:00", "1e1:00", "+5:00"] {
        assert_eq!(p(s).value, Value::text(s), "{s}");
    }
}

#[test]
fn signed_date_parts_stay_text() {
    for s in ["+3/4/2026", "3/+4/2026", "3/4/+2026", "1/1/2020 +5:00"] {
        assert_eq!(p(s).value, Value::text(s), "{s}");
    }
    assert_eq!(p("3/4/2026").value, Value::Number(46085.0));
}

#[test]
fn month_name_short_year() {
    for s in ["Mar 4, 26", "Mar 4 26", "March 4 26", "4 March 26", "Mar 4, 2026"] {
        assert_eq!(p(s).value, Value::Number(46085.0), "{s}");
    }
    assert_eq!(p("Mar 4, 99").value, p("Mar 4, 1999").value);
    assert_eq!(p("Mar 4, 1776").value, Value::text("Mar 4, 1776"));
    assert_eq!(p("Feb 30, 26").value, Value::text("Feb 30, 26"));
}

#[test]
fn numbers() {
    assert_eq!(p("42").value, Value::Number(42.0));
    assert_eq!(p("-3.5").value, Value::Number(-3.5));
    assert_eq!(p("1,234").format, Some("#,##0"));
    assert_eq!(p("1,234").value, Value::Number(1234.0));
    assert_eq!(p("1,23").value, Value::text("1,23"));
    assert_eq!(p("12%").value, Value::Number(0.12));
    assert_eq!(p("$1,200.50").value, Value::Number(1200.5));
    assert_eq!(p("$1,200.50").format, Some("\"$\"#,##0.00"));
    assert_eq!(p("(100)").value, Value::Number(-100.0));
    assert_eq!(p("1e3").value, Value::Number(1000.0));
    assert_eq!(p("1 1/2").value, Value::Number(1.5));
    assert_eq!(p(".5").value, Value::Number(0.5));
}

/// Observed in Excel 365 (en-US): typed entry, VALUE and `--"text"` agree.
#[test]
fn accounting_parentheses() {
    let accepted: [(&str, f64, Option<&str>); 22] = [
        ("$(5)", -5.0, Some("\"$\"#,##0")),
        ("$ (5)", -5.0, Some("\"$\"#,##0")),
        ("$( 5 )", -5.0, Some("\"$\"#,##0")),
        ("$ ( 1,234 )", -1234.0, Some("\"$\"#,##0")),
        ("$(1,234)", -1234.0, Some("\"$\"#,##0")),
        ("$(1,000.50)", -1000.5, Some("\"$\"#,##0.00")),
        ("$(1,234.)", -1234.0, Some("\"$\"#,##0.00")),
        ("$(.5)", -0.5, Some("\"$\"#,##0.00")),
        ("$(0)", 0.0, Some("\"$\"#,##0")),
        (" $(5) ", -5.0, Some("\"$\"#,##0")),
        ("($5)", -5.0, Some("\"$\"#,##0")),
        ("( $ 1,234 )", -1234.0, Some("\"$\"#,##0")),
        ("(5)", -5.0, None),
        ("( 5 )", -5.0, None),
        ("(1,234.5)", -1234.5, Some("#,##0.00")),
        ("(1e-3)", -0.001, Some("0.00E+00")),
        ("(5)%", -0.05, Some("0%")),
        ("( 5 ) %", -0.05, Some("0%")),
        ("(5%)", -0.05, Some("0%")),
        ("(5.)%", -0.05, Some("0.00%")),
        ("(1,234)%", -12.34, Some("0%")),
        ("(1 1/2)", -1.5, Some("# ?/?")),
    ];
    for (s, n, f) in accepted {
        assert_eq!(p(s), Parsed { value: Value::Number(n), format: f }, "{s}");
        assert_eq!(parse_number_text(s), Some(n), "{s}");
    }
    for s in [
        "$5%",
        "$(5%)",
        "$(5)%",
        "($5%)",
        "($5)%",
        "$ ( 5 ) %",
        "(5)%%",
        "(5%)%",
        "((5))",
        "(-5)",
        "(+5)",
        "($-5)",
        "($ -5)",
        "($+5)",
        "(-$5)",
        "$(-5)",
        "$ (-5)",
        "$(+5)",
        "-(5)",
        "-$(5)",
        "+$(5)",
        "$+(5)",
        "$$(5)",
        "$ $(5)",
        "$((5))",
        "($(5))",
        "(-1 1/2)",
        "()",
        "$()",
        "(5)$",
        "$(5)$",
        "(5)(5)",
        "$(5)0",
        "(5)e3",
        "(5)-",
        "(5-)",
        "$(5-)",
        "$(5",
        "$5)",
        "(5",
        "$(1,23)",
        "$(,5)",
        "(5e)",
        "(1/2)",
        "$(1/2)",
        "(3/4/2026)",
        "$(3/4/2026)",
        "(10:30)",
        "$(10:30)",
    ] {
        assert_eq!(p(s).value, Value::text(s), "{s}");
        assert_eq!(parse_number_text(s), None, "{s}");
    }
}

#[test]
fn deeply_nested_parentheses_do_not_overflow() {
    // #2180: coercion used to recurse once per parenthesis and overflow the stack. Excel reads
    // one pair only, so nesting is text now, and it is rejected without recursing.
    let depth = 100_000;
    let s = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
    assert_eq!(parse_number_text(&s), None);
    assert_eq!(p(&s).value, Value::text(s.as_str()));
    let odd = format!("{}1{}", "(".repeat(depth + 1), ")".repeat(depth + 1));
    assert_eq!(parse_number_text(&odd), None);
    let unbalanced = format!("{}1{}", "(".repeat(depth), ")".repeat(depth - 1));
    assert_eq!(parse_number_text(&unbalanced), None);
    // A single pair still reads.
    assert_eq!(parse_number_text("(100)"), Some(-100.0));
    assert_eq!(parse_number_text("( $1,200.50 )"), Some(-1200.5));
    assert_eq!(parse_number_text("()"), None);
}

#[test]
fn texts_and_specials() {
    assert_eq!(p("hello").value, Value::text("hello"));
    assert_eq!(p("'123").value, Value::text("123"));
    assert_eq!(p("true").value, Value::Bool(true));
    assert_eq!(p("#N/A").value, Value::Error(CellError::NA));
    assert_eq!(p("e").value, Value::text("e"));
    assert_eq!(p("-").value, Value::text("-"));
    assert_eq!(p("1.2.3.4").value, Value::text("1.2.3.4"));
}

#[test]
fn dates_and_times() {
    assert_eq!(p("10/7/2026").value, Value::Number(46302.0));
    assert_eq!(p("2026-10-07").value, Value::Number(46302.0));
    assert_eq!(p("7-Oct-2026").value, Value::Number(46302.0));
    assert_eq!(p("Oct 7, 2026").value, Value::Number(46302.0));
    assert_eq!(p("2/30/2026").value, Value::text("2/30/2026"));
    assert_eq!(p("7/4/1776").value, Value::text("7/4/1776"));
    assert_eq!(p("1776-07-04").value, Value::text("1776-07-04"));
    assert_eq!(p("4-Jul-1776").value, Value::text("4-Jul-1776"));
    assert_eq!(p("12/31/1899").value, Value::text("12/31/1899"));
    assert_eq!(p("1/1/1900").value, Value::Number(1.0));
    assert_eq!(p("12:00").value, Value::Number(0.5));
    assert_eq!(p("6:00 PM").value, Value::Number(0.75));
    assert_eq!(p("12:00 AM").value, Value::Number(0.0));
    assert_eq!(p("10/7/2026 18:00").value, Value::Number(46302.75));
    assert_eq!(p("25:00").value, Value::Number(25.0 / 24.0));
    assert_eq!(p("25:00").format, Some("[h]:mm:ss"));
    assert_eq!(p("23:59").format, Some("h:mm"));
}

#[test]
fn fictitious_1900_02_29() {
    // Excel accepts 29 February 1900 (serial 60) and keeps 1 March 1900 at 61.
    assert_eq!(p("2/29/1900").value, Value::Number(60.0));
    assert_eq!(p("1900-02-29").value, Value::Number(60.0));
    assert_eq!(p("29-Feb-1900").value, Value::Number(60.0));
    assert_eq!(p("2/28/1900").value, Value::Number(59.0));
    assert_eq!(p("3/1/1900").value, Value::Number(61.0));
    assert_eq!(p("2/29/1901").value, Value::text("2/29/1901"));
    assert_eq!(p("2/30/1900").value, Value::text("2/30/1900"));
    // The 1904 system starts after it.
    assert_eq!(parse_input("2/29/1900", DateSystem::D1904).value, Value::text("2/29/1900"));
    assert_eq!(parse_input("2/29/1904", DateSystem::D1904).value, Value::Number(59.0));
}

#[test]
fn numbers_stored_as_text() {
    for (text, n) in [
        ("123", 123.0),
        (" 42 ", 42.0),
        ("-1,000.5", -1000.5),
        ("$3", 3.0),
        ("50%", 0.5),
        ("1E3", 1000.0),
        ("(100)", -100.0),
        ("$(5)", -5.0),
        ("(5)%", -0.05),
    ] {
        assert_eq!(number_stored_as_text(text), Some(n), "{text}");
    }
    // Dates, times, fractions, words and blanks are not flagged.
    for text in ["3/4/2026", "10:30", "1 1/2", "abc", "12abc", "", "  ", "TRUE", "#N/A", "((1))", "(-5)", "$(5)%", "(1/2)"] {
        assert_eq!(number_stored_as_text(text), None, "{text}");
    }
}
