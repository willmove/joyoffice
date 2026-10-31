use super::*;

#[test]
fn only_http_https_and_mailto_leave_the_workbook() {
    for ok in ["http://a.example/x", "HTTPS://A.example", "  https://a.example  ", "MailTo:a@b.example"] {
        assert!(matches!(classify(ok), LinkKind::External(_)), "{ok}");
    }
    assert_eq!(classify(" https://a.example "), LinkKind::External("https://a.example".into()));
    for bad in [
        "javascript:alert(1)",
        "JaVaScRiPt:alert(1)",
        "file:///etc/passwd",
        "smb://host/share",
        "ms-excel:ofe|u|http://x",
        "\\\\host\\share\\x.exe",
        "//host/share",
        "C:\\Windows\\calc.exe",
        "\tjavascript:1",
        "java\nscript:1",
        "\u{1}http://a.example",
        "http:",
        "mailto:",
        "x-custom://a",
    ] {
        assert!(matches!(classify(bad), LinkKind::Refused(_)), "{bad:?} -> {:?}", classify(bad));
    }
}

#[test]
fn references_stay_internal() {
    for r in ["Sheet2!A1", "'My Sheet'!B2:C3", "A1:B2", "#Sheet1!A1", "Total", "", "   "] {
        assert!(matches!(classify(r), LinkKind::Internal(_)), "{r:?} -> {:?}", classify(r));
    }
}
