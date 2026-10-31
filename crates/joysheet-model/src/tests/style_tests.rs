use super::*;

#[test]
fn interning() {
    let mut t = StyleTable::default();
    let bold = t.derive(StyleId::DEFAULT, |s| s.font.bold = true);
    assert_ne!(bold, StyleId::DEFAULT);
    assert_eq!(t.derive(StyleId::DEFAULT, |s| s.font.bold = true), bold);
    assert_eq!(t.derive(bold, |s| s.font.bold = false), StyleId::DEFAULT);
    assert_eq!(t.len(), 2);
    assert!(!t.get(StyleId(999)).font.bold);
}

#[test]
fn a_theme_saved_before_per_script_fonts_still_opens() {
    let mut json = serde_json::to_value(Theme::default()).unwrap();
    let fields = json.as_object_mut().unwrap();
    assert!(fields.remove("major_font_scripts").is_some() && fields.remove("minor_font_scripts").is_some());
    assert_eq!(serde_json::from_value::<Theme>(json).unwrap(), Theme::default());
}

#[test]
fn colors() {
    let th = Theme::default();
    assert_eq!(Color::from_hex("#FF0000").unwrap().resolve(&th), Some([255, 0, 0]));
    assert_eq!(Color::from_hex("FF00FF00").unwrap().resolve(&th), Some([0, 255, 0]));
    assert_eq!(Color::Theme(0, -500).resolve(&th), Some([128, 128, 128]));
    assert_eq!(Color::Theme(1, 500).resolve(&th), Some([128, 128, 128]));
    assert_eq!(Color::Auto.resolve(&th), None);
}

#[test]
fn theme_json_keeps_ea_and_cs_and_reads_old_json() {
    // JSON written before the ea/cs fields existed still deserializes (`serde(default)`).
    let mut old = serde_json::to_value(Theme::default()).unwrap();
    let obj = old.as_object_mut().unwrap();
    for key in ["major_font_ea", "minor_font_ea", "major_font_cs", "minor_font_cs"] {
        assert!(obj.remove(key).is_some(), "the default theme serializes {key}");
    }
    assert_eq!(serde_json::from_value::<Theme>(old).unwrap(), Theme::default());

    // Set fields serialize and come back.
    let t = Theme {
        major_font_ea: "游ゴシック".into(),
        minor_font_ea: "ＭＳ Ｐゴシック".into(),
        major_font_cs: "Arial".into(),
        minor_font_cs: "Times New Roman".into(),
        ..Theme::default()
    };
    let json = serde_json::to_string(&t).unwrap();
    assert!(json.contains("major_font_ea") && json.contains("游ゴシック"), "{json}");
    assert_eq!(serde_json::from_str::<Theme>(&json).unwrap(), t);
}
