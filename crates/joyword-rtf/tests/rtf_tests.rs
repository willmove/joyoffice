use joyword_rtf::{RtfBudget, RtfError, export_rtf, import_rtf};

#[test]
fn test_rtf_roundtrip_basic() {
    let rtf_input = br#"{\rtf1\ansi\deff0
{\fonttbl{\f0\fnil\fcharset134 Songti;}}
\b Bold Text\b0  and \i Italic Text\i0
\par
Unicode text: \u20013?\u25991?
\par
}"#;

    let (doc, report) = import_rtf(rtf_input, RtfBudget::default()).expect("import rtf");
    assert!(!report.has_loss());

    let (exported, exp_report) = export_rtf(&doc);
    assert!(!exp_report.has_loss());
    assert!(exported.starts_with("{\\rtf1"));
    assert!(exported.contains("\\b Bold Text\\b0"));
    assert!(exported.contains("\\i Italic Text\\i0"));
    assert!(exported.contains("\\u20013?\\u25991?"));
}

#[test]
fn test_rtf_table() {
    let rtf_table = br#"{\rtf1\ansi\deff0
\trowd\cellx2000\cellx4000
\intbl Cell 1\cell
\intbl Cell 2\cell
\row
}"#;

    let (doc, _) = import_rtf(rtf_table, RtfBudget::default()).expect("import table");
    let (exported, _) = export_rtf(&doc);
    assert!(exported.contains("\\trowd"));
    assert!(exported.contains("Cell 1"));
    assert!(exported.contains("Cell 2"));
    assert!(exported.contains("\\cell"));
    assert!(exported.contains("\\row"));
}

#[test]
fn test_rtf_ignorable_destination() {
    let rtf_dest = br#"{\rtf1\ansi\deff0
{\*\generator JoyWord 2026;}
{\*\themedata 01020304}
Hello Visible World!
\par
}"#;

    let (doc, _) = import_rtf(rtf_dest, RtfBudget::default()).expect("import rtf with dest");
    let (exported, _) = export_rtf(&doc);
    assert!(exported.contains("Hello Visible World!"));
    assert!(!exported.contains("JoyWord 2026"));
    assert!(!exported.contains("01020304"));
}

#[test]
fn test_rtf_malformed_and_budget() {
    // 1. Missing header
    assert_eq!(
        import_rtf(b"Not an RTF document", RtfBudget::default()).unwrap_err(),
        RtfError::InvalidHeader
    );

    // 2. Budget nesting depth
    let budget = RtfBudget {
        max_group_depth: 3,
        ..RtfBudget::default()
    };
    let deep_nested = b"{\\rtf1 { { { { Deep } } } } }";
    assert_eq!(
        import_rtf(deep_nested, budget).unwrap_err(),
        RtfError::GroupDepthExceeded
    );
}
