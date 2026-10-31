use super::*;

#[test]
fn si_without_phonetics_is_written_exactly_as_before() {
    for s in ["plain", " leading ", "a & <b>", "line\nbreak", "日本語", "x_x0041_"] {
        assert_eq!(si_body(s, None), t_el(s));
    }
}

#[test]
fn a_part_of_endless_empty_elements_is_refused() {
    assert!(parse(b"<r><x/><y t=\"1\"/><x/></r>").is_ok());
    let xml = format!("<r>{}<y/></r>", "<x/>".repeat(MAX_XML_NODES + 1));
    let err = parse(xml.as_bytes()).unwrap_err();
    assert!(err.to_string().contains("elements"), "{err}");
}

#[test]
fn phonetic_data_validates_offsets() {
    let x =
        r#"<si><t>課きく</t><rPh sb="0" eb="1"><t>カ</t></rPh><rPh sb="2" eb="9"><t>ク</t></rPh><rPh sb="2" eb="2"><t>x</t></rPh><rPh sb="1"/></si>"#;
    let si = parse(x.as_bytes()).unwrap();
    let (runs, props) = phonetic_data(&si, &rich_text(&si));
    assert_eq!(runs, vec![PhoneticRun { start: 0, end: 1, text: "カ".into() }, PhoneticRun { start: 2, end: 3, text: "ク".into() }]);
    assert!(props.is_none());
}

/// The write path validates too: a `PhoneticString` that came from JSON or an API caller (not
/// from this reader) is written only where the schema allows it — an `sb` before `eb`, both
/// inside the base text, a reading that isn't empty, and known `type`/`alignment` values.
#[test]
fn si_body_drops_damaged_phonetic_data() {
    // 課きく is three UTF-16 units.
    let text = "課きく";
    let damaged = PhoneticString {
        text: text.into(),
        runs: vec![
            PhoneticRun { start: 8, end: 9, text: "x".into() },    // past the text
            PhoneticRun { start: 2, end: 2, text: "x".into() },    // empty range
            PhoneticRun { start: 0, end: 1, text: String::new() }, // no reading
            PhoneticRun { start: 1, end: 3, text: "く".into() },   // valid, clamped end
        ],
        props: Some(PhoneticProps { font_id: 7, kind: Some("bogus".into()), alignment: Some("center".into()) }),
    };
    let s = si_body(text, Some(&damaged));
    assert_eq!(s, "<t>課きく</t><rPh sb=\"1\" eb=\"3\"><t>く</t></rPh><phoneticPr fontId=\"7\" alignment=\"center\"/>");
    // With no run left, valid properties are still written: `<phoneticPr>` is a legal child of
    // `<si>` on its own, and Excel keeps the settings when the last reading is removed.
    let empty = PhoneticString { text: text.into(), runs: vec![], props: damaged.props.clone() };
    assert_eq!(si_body(text, Some(&empty)), "<t>課きく</t><phoneticPr fontId=\"7\" alignment=\"center\"/>");
    // With neither runs nor properties there is nothing to write.
    let none = PhoneticString { text: text.into(), runs: vec![], props: None };
    assert_eq!(si_body(text, Some(&none)), t_el(text));
    // A known kind is written; the schema default ("none") is not invented for a missing one.
    let known = PhoneticString {
        text: text.into(),
        runs: vec![PhoneticRun { start: 0, end: 1, text: "カ".into() }],
        props: Some(PhoneticProps { font_id: 1, kind: Some("Hiragana".into()), alignment: None }),
    };
    assert_eq!(si_body(text, Some(&known)), "<t>課きく</t><rPh sb=\"0\" eb=\"1\"><t>カ</t></rPh><phoneticPr fontId=\"1\" type=\"Hiragana\"/>");
}

#[test]
fn parse_basic() {
    let x = br#"<?xml version="1.0"?><a:root xmlns:a="x" k="v &amp; w"><b>t &lt; u&#65;</b><c/><![CDATA[<raw>]]></a:root>"#;
    let e = parse(x).unwrap();
    assert_eq!(e.name, "root");
    assert_eq!(e.attr("k"), Some("v & w"));
    assert_eq!(e.child("b").unwrap().text, "t < uA");
    assert!(e.child("c").is_some());
    assert_eq!(e.text, "<raw>");
}

#[test]
fn streaming_split() {
    let x = b"<r><row n='1'/><row n='2'><c/></row><other/></r>";
    let mut n = 0;
    let root = parse_streaming(x, &["row"], &mut |e| {
        assert_eq!(e.name, "row");
        n += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(n, 2);
    assert_eq!(root.children.len(), 1);
}

#[test]
fn deep_nesting_is_an_error() {
    let mut s = String::new();
    for _ in 0..10_000 {
        s.push_str("<a>");
    }
    assert!(parse(s.as_bytes()).is_err());
}

#[test]
fn garbage() {
    assert!(parse(b"").is_err());
    assert!(parse(b"\x00\x01\x02 not xml").is_err() || parse(b"\x00\x01\x02 not xml").is_ok());
    let _ = parse(b"<a><b></a>");
    let _ = parse(b"<a attr='&bogus;'>&unknown;</a>");
}

#[test]
fn xstring() {
    assert_eq!(decode_xstring("a_x000D_b"), "a\rb");
    assert_eq!(encode_xstring("a\r\nb\rc"), "a_x000D_\nb_x000D_c");
    assert_eq!(decode_xstring(&encode_xstring("a\r\nb\rc")), "a\r\nb\rc");
    assert_eq!(encode_xstring("a\u{1}b"), "a_x0001_b");
    assert_eq!(decode_xstring(&encode_xstring("_x0041_")), "_x0041_");
    assert_eq!(decode_xstring("_x00"), "_x00");
}

#[test]
fn xstring_surrogate_pairs() {
    assert_eq!(decode_xstring("a_xD83D__xDE00_b"), "a\u{1F600}b");
    assert_eq!(decode_xstring("_xd83d__xde00_"), "\u{1F600}");
    // Lone surrogates stay as literal escape text.
    assert_eq!(decode_xstring("_xD83D_"), "_xD83D_");
    assert_eq!(decode_xstring("_xDE00_"), "_xDE00_");
    assert_eq!(decode_xstring("_xD83D_x"), "_xD83D_x");
    assert_eq!(decode_xstring("_xD83D__x0041_"), "_xD83D_A");
    // A protected literal escape is not a low surrogate.
    assert_eq!(decode_xstring("_xD83D__x005F_xDE00_"), "_xD83D__xDE00_");
}

#[test]
fn text_escaping_preserves_carriage_returns() {
    assert_eq!(esc("a\rb"), "a&#13;b");
    assert_eq!(esc_attr("a\rb"), "a&#13;b");
}

#[test]
fn numbers() {
    assert_eq!(num(1.0), "1");
    assert_eq!(num(0.1), "0.1");
    assert_eq!(num(1e-7), "1E-7");
    assert_eq!(num(-3.0), "-3");
    assert_eq!(num(1.5e20), "1.5E20");
    assert_eq!("1.5E20".parse::<f64>().unwrap(), 1.5e20);
}
