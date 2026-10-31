use super::*;

#[test]
fn reads_excel_and_libreoffice_cell_values() {
    let excel = r#"<html><head><style>td{mso-number-format:General}</style></head><body><table>
        <!--StartFragment--><tr height=20><td class=xl65 x:num="10000">Rp10,000.00</td><td x:str>Name</td></tr>
        <tr><td colspan=2 x:num="2.5">2.50 &amp; up</td><td>x</td></tr><!--EndFragment--></table></body></html>"#;
    let rows = table_cells(excel, 2, 3);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0], HtmlCell { text: "Rp10,000.00".into(), num: Some(10000.0) });
    assert_eq!(rows[0][1], HtmlCell { text: "Name".into(), num: None });
    assert_eq!(rows[1].len(), 3, "colspan keeps the next cell in its column");
    assert_eq!(rows[1][0].text, "2.50 & up");
    assert_eq!(rows[1][2].text, "x");

    let calc = r#"<table cellspacing="0" border="0"><colgroup width="85"></colgroup><tr>
        <td height="17" align="right" sdval="10000" sdnum="1033;0;[$Rp-421]#,##0.00"><font face="Liberation Sans">Rp10,000.00</font></td></tr></table>"#;
    assert_eq!(table_cells(calc, 1, 1), vec![vec![HtmlCell { text: "Rp10,000.00".into(), num: Some(10000.0) }]]);

    let quoted_gt = r#"<table><tr><td style='mso-number-format:"\[>=1000\]0"' x:num="1500">1500</td></tr></table>"#;
    assert_eq!(table_cells(quoted_gt, 1, 1), vec![vec![HtmlCell { text: "1500".into(), num: Some(1500.0) }]]);
}

#[test]
fn entities_recognized_only_within_the_twelve_byte_window() {
    // The window never scans the rest of the text for a semicolon: a bare '&' followed by
    // long plain text stays a bare '&', and one '&' after another is linear.
    assert_eq!(decode("a&amplifier; b"), "a&amplifier; b");
    assert_eq!(decode("&ampampampampAMP;"), "&ampampampampAMP;");
    // Inside the window the usual forms still decode, and a trailing bare '&' too.
    assert_eq!(decode("&#38;"), "&");
    assert_eq!(decode("&#x266A;"), "\u{266A}");
    assert_eq!(decode("a&amp;b&nbsp;c"), "a&b\u{a0}c");
    assert_eq!(decode("amp&"), "amp&");
}

#[test]
fn hostile_markup_does_not_panic() {
    for html in [
        "",
        "<",
        "<td",
        "<table><td x:num=>",
        "<table><tr><td colspan=999999999>a",
        "&#xFFFFFFFF;&",
        "<table><td x:num='1e999'>1</td>",
        "<!--",
        "<style>",
        "<table><table><td>",
    ] {
        let _ = table_cells(html, MAX_ROW_SPAN, MAX_SPAN);
    }
    assert_eq!(table_cells("<table><td x:num='1e999'>1</td></table>", 1, 1)[0][0].num, None);
    assert!(table_cells("<table><tr><td colspan=999999999>a</td></tr></table>", 1, usize::MAX)[0].len() <= MAX_SPAN);
}

#[test]
fn huge_spans_stay_within_the_cell_budget() {
    // #2171: 100,000 rows of `colspan=16384` once expanded to ~1.6 billion cells (OOM). Even
    // with the sheet's full extent as the limit, at most MAX_HTML_CELLS are materialized.
    let row = "<tr><td colspan=16384 rowspan=99999999999 x:num='1'>1</td><td colspan=4294967296>2</td></tr>";
    let html = format!("<table>{}</table>", row.repeat(100_000));
    let rows = table_cells(&html, usize::MAX, usize::MAX);
    let cells: usize = rows.iter().map(Vec::len).sum();
    assert!(cells <= MAX_HTML_CELLS, "{cells} cells");
    assert!(rows.iter().all(|r| r.len() <= MAX_SPAN));
    assert_eq!(rows[0].len(), MAX_SPAN, "the first span fills the sheet's width");
    assert_eq!(rows[0][0], HtmlCell { text: "1".into(), num: Some(1.0) });

    // Bounded by the pasted text's shape, the same markup yields just that shape.
    let rows = table_cells(&html, 3, 2);
    assert_eq!(rows.iter().map(Vec::len).collect::<Vec<_>>(), [2, 2, 2]);
    assert_eq!(rows[0][0].num, Some(1.0));
    assert!(table_cells(&html, 0, 5).is_empty());
    assert!(table_cells(&html, 4, 0).iter().all(Vec::is_empty));
}

#[test]
fn rowspan_keeps_later_rows_in_their_columns() {
    // A merged A1:A2 is one `rowspan=2` cell; row 2 omits it, and its text has an empty field there.
    let html = r#"<table><tr><td rowspan=2 colspan=2>Merged</td><td x:num="1">1</td></tr>
        <tr><td x:num="2">2</td></tr><tr><td x:num="3">3</td><td>c</td></tr></table>"#;
    let rows = table_cells(html, 3, 3);
    assert_eq!(rows.iter().map(Vec::len).collect::<Vec<_>>(), [3, 3, 2]);
    assert_eq!(rows[0][2].num, Some(1.0));
    assert_eq!((rows[1][0].clone(), rows[1][1].clone()), (HtmlCell::default(), HtmlCell::default()));
    assert_eq!(rows[1][2].num, Some(2.0));
    assert_eq!(rows[2][0].num, Some(3.0), "the span ends after its second row");
}

#[test]
fn infers_a_format_that_shows_the_same_text() {
    assert_eq!(format_for("Rp10,000.00", 10000.0).as_deref(), Some("\"Rp\"#,##0.00"));
    assert_eq!(format_for("1 234 kr", 1234.0), None, "space grouping is not reproduced");
    assert_eq!(format_for("10.000,00 €", 10000.0), None, "comma decimals don't render back in en-US");
    assert_eq!(format_for("-Rp5", -5.0).as_deref(), Some("\"Rp\"0"));
    assert_eq!(format_for("12 EUR", 12.0).as_deref(), Some("0\" EUR\""));
}
