//! Compatibility cases retained from the original XML adapter.
use super::*;
use crate::PptxError;

#[test]
fn literal_cr_lf_and_lone_cr_read_as_lf_but_char_ref_stays() -> Result<(), PptxError> {
    let d =
        parse(b"<a:p><a:t>one\r\ntwo\rthree&#13;four</a:t><a:t><![CDATA[x\r\ny]]></a:t></a:p>")?;
    let t: Vec<String> = d.root.children_named("t").map(|t| t.text()).collect();
    assert_eq!(
        t,
        vec!["one\ntwo\nthree\rfour".to_string(), "x\ny".to_string()]
    );
    Ok(())
}

#[test]
fn cr_in_text_survives_write_read() -> Result<(), PptxError> {
    let mut w = W::default();
    w.elt("a:t", "one\rtwo\r\nthree");
    let d = parse(w.s.as_bytes())?;
    assert_eq!(d.root.text(), "one\rtwo\r\nthree");
    Ok(())
}

#[test]
fn parse_basic_and_entities() -> Result<(), PptxError> {
    let d = parse(b"<?xml version=\"1.0\"?><a:p xmlns:a=\"x\"><a:r><a:t>A &amp; B &#x41; &lt;</a:t></a:r><a:x r:id=\"rId1\" id=\"5\"/></a:p>")?;
    assert_eq!(d.root.local(), "p");
    assert_eq!(
        d.root.find("t").map(|t| t.text()),
        Some("A & B A <".to_string())
    );
    let child = d.root.child("x");
    assert_eq!(child.and_then(|x| x.attr("id")), Some("5"));
    assert_eq!(child.and_then(|x| x.attr("r:id")), Some("rId1"));
    assert_eq!(d.ns_decls().len(), 1);
    Ok(())
}

#[test]
fn truncated_and_garbage() {
    assert!(parse(b"<a><b><c>text").is_ok());
    assert!(parse(b"").is_err());
    let deep = "<a>".repeat(MAX_DEPTH + 10);
    assert!(parse(deep.as_bytes()).is_err());
}

#[test]
fn escape_drops_control_chars() {
    let mut writer = W::frag();
    writer.empty("e", A::new().a("v", "\"x\"\n"));
    writer.text("a\u{b}b<\u{0}");
    assert_eq!(writer.s, "<e v=\"&quot;x&quot;&#10;\"/>ab&lt;");
}

#[test]
fn roundtrip_serialize() -> Result<(), PptxError> {
    let d = parse(b"<p:sp a=\"1\"><p:t>x &amp; y</p:t><e/></p:sp>")?;
    assert_eq!(
        d.root.to_xml(),
        "<p:sp a=\"1\"><p:t>x &amp; y</p:t><e/></p:sp>"
    );
    Ok(())
}
