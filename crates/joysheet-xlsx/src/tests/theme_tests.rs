use super::*;

/// A Japanese Office theme: the heading font sets latin, ea and cs; the body font only ea.
const EA_THEME: &str = r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office Theme"><a:themeElements><a:clrScheme name="Office"/><a:fontScheme name="Office"><a:majorFont><a:latin typeface="Calibri"/><a:ea typeface="游ゴシック"/><a:cs typeface="Arial"/></a:majorFont><a:minorFont><a:ea typeface="ＭＳ Ｐゴシック"/></a:minorFont></a:fontScheme></a:themeElements></a:theme>"#;

#[test]
fn roundtrip() {
    let mut t = Theme::default();
    t.colors[0] = 0xFEFEFE;
    t.colors[1] = 0x101010;
    t.colors[9] = 0x123456;
    t.major_font = "Cambria".into();
    t.name = "Mine".into();
    let x = write_theme(&t);
    let back = read_theme(&crate::xml::parse(x.as_bytes()).unwrap());
    assert_eq!(back, t);
}

#[test]
fn ea_and_cs_typefaces_roundtrip() {
    let t = read_theme(&crate::xml::parse(EA_THEME.as_bytes()).unwrap());
    assert_eq!(t.major_font, "Calibri");
    assert_eq!(t.major_font_ea, "游ゴシック");
    assert_eq!(t.major_font_cs, "Arial");
    // No `<a:latin>` under the minor font: the default stays.
    assert_eq!(t.minor_font, Theme::default().minor_font);
    assert_eq!(t.minor_font_ea, "ＭＳ Ｐゴシック");
    assert!(t.minor_font_cs.is_empty());

    let x = write_theme(&t);
    assert!(x.contains(r#"<a:majorFont><a:latin typeface="Calibri"/><a:ea typeface="游ゴシック"/><a:cs typeface="Arial"/></a:majorFont>"#), "{x}");
    assert!(x.contains(r#"<a:minorFont><a:latin typeface="Calibri"/><a:ea typeface="ＭＳ Ｐゴシック"/><a:cs typeface=""/></a:minorFont>"#), "{x}");
    assert_eq!(read_theme(&crate::xml::parse(x.as_bytes()).unwrap()), t);
}

#[test]
fn theme_without_ea_or_cs_writes_empty_attributes() {
    let t = Theme::default();
    let x = write_theme(&t);
    assert!(x.contains(r#"<a:majorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont>"#), "{x}");
    assert!(x.contains(r#"<a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont>"#), "{x}");
}

#[test]
fn explicit_empty_ea_and_cs_read_as_unset() {
    let xml = EA_THEME
        .replace(r#"<a:ea typeface="游ゴシック"/>"#, r#"<a:ea typeface=""/>"#)
        .replace(r#"<a:ea typeface="ＭＳ Ｐゴシック"/>"#, r#"<a:ea typeface=""/>"#);
    let t = read_theme(&crate::xml::parse(xml.as_bytes()).unwrap());
    assert!(t.major_font_ea.is_empty() && t.minor_font_ea.is_empty());
    assert_eq!(t.major_font_cs, "Arial");
    let x = write_theme(&t);
    assert!(x.contains(r#"<a:majorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface="Arial"/></a:majorFont>"#), "{x}");
    assert_eq!(read_theme(&crate::xml::parse(x.as_bytes()).unwrap()), t);
}

#[test]
fn ea_and_cs_typefaces_are_escaped() {
    let t = Theme { major_font_ea: "游&ゴシック".into(), minor_font_cs: "A<B>\"C\"".into(), ..Theme::default() };
    let x = write_theme(&t);
    assert!(x.contains(r#"<a:ea typeface="游&amp;ゴシック"/>"#), "{x}");
    assert!(x.contains(r#"<a:cs typeface="A&lt;B&gt;&quot;C&quot;"/>"#), "{x}");
    assert_eq!(read_theme(&crate::xml::parse(x.as_bytes()).unwrap()), t);
}

/// An Arabic Office theme: Arabic, Hebrew and Japanese faces in the per-script lists.
const SCRIPT_THEME: &str = r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office Theme"><a:themeElements><a:fontScheme name="Office"><a:majorFont><a:latin typeface="Calibri Light"/><a:ea typeface=""/><a:cs typeface="Times New Roman"/><a:font script="Arab" typeface="Times New Roman"/><a:font script="Hebr" typeface="Times New Roman"/><a:font script="Jpan" typeface="游ゴシック Light"/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface="Arial"/><a:font script="Arab" typeface="Arial"/><a:font script="Hebr" typeface="Arial"/><a:font script="Jpan" typeface="游ゴシック"/></a:minorFont></a:fontScheme></a:themeElements></a:theme>"#;

#[test]
fn per_script_typefaces_roundtrip_in_file_order() {
    let t = read_theme(&crate::xml::parse(SCRIPT_THEME.as_bytes()).unwrap());
    let pairs = |v: &[(String, String)]| v.iter().map(|(s, f)| (s.clone(), f.clone())).collect::<Vec<_>>();
    let expect =
        |a: &str, j: &str| vec![("Arab".to_string(), a.to_string()), ("Hebr".to_string(), a.to_string()), ("Jpan".to_string(), j.to_string())];
    assert_eq!(pairs(&t.major_font_scripts), expect("Times New Roman", "游ゴシック Light"));
    assert_eq!(pairs(&t.minor_font_scripts), expect("Arial", "游ゴシック"));
    let x = write_theme(&t);
    assert!(
        x.contains(r#"<a:cs typeface="Times New Roman"/><a:font script="Arab" typeface="Times New Roman"/><a:font script="Hebr" typeface="Times New Roman"/><a:font script="Jpan" typeface="游ゴシック Light"/></a:majorFont>"#),
        "{x}"
    );
    assert_eq!(read_theme(&crate::xml::parse(x.as_bytes()).unwrap()), t);
}

#[test]
fn hostile_per_script_lists_are_capped() {
    let entries: String = (0..100).map(|i| format!(r#"<a:font script="S{i}" typeface="{}"/>"#, "x".repeat(300))).collect();
    let xml = format!(
        r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:themeElements><a:fontScheme><a:majorFont><a:latin typeface="Calibri"/>{entries}</a:majorFont></a:fontScheme></a:themeElements></a:theme>"#
    );
    let t = read_theme(&crate::xml::parse(xml.as_bytes()).unwrap());
    assert_eq!(t.major_font_scripts.len(), 64);
    assert!(t.major_font_scripts.iter().all(|(script, face)| script.chars().count() <= 256 && face.chars().count() <= 256));
}
