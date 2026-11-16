use super::*;
use joyoffice_theme::ThemePreset;

#[test]
fn recent_fonts_are_bounded_case_insensitive_and_survive_preferences_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let mut prefs: Preferences = serde_json::from_str("{\"recent\":[\"old.xlsx\"]}")?;
    assert!(prefs.recent_fonts.is_empty());
    for i in 0..12 {
        prefs.remember_font(&format!("Font {i}"));
    }
    prefs.remember_font("  font 9  ");
    assert_eq!(prefs.recent_fonts.len(), 8);
    assert_eq!(prefs.recent_fonts[0], "font 9");
    assert_eq!(prefs.recent_fonts.iter().filter(|f| f.eq_ignore_ascii_case("font 9")).count(), 1);
    let before = prefs.recent_fonts.clone();
    prefs.remember_font("");
    prefs.remember_font("bad\nfont");
    assert_eq!(prefs.recent_fonts, before);
    let decoded: Preferences = serde_json::from_slice(&serde_json::to_vec(&prefs)?)?;
    assert_eq!(decoded.recent_fonts, prefs.recent_fonts);
    assert_eq!(decoded.recent, ["old.xlsx"]);
    Ok(())
}

#[test]
fn old_and_future_preferences_keep_their_other_settings() -> Result<(), Box<dyn std::error::Error>> {
    for extra in ["", ",\"theme\":\"future-theme\""] {
        let prefs: Preferences = serde_json::from_str(&format!("{{\"dark\":true,\"language\":\"en\",\"recent\":[\"a.xlsx\"]{extra}}}"))?;
        assert_eq!(prefs.theme_preset(), ThemePreset::VerdantDark);
        assert_eq!(prefs.language, "en");
        assert_eq!(prefs.recent, ["a.xlsx"]);
    }
    Ok(())
}

#[test]
fn every_preset_round_trips_and_mode_switch_keeps_the_family() -> Result<(), Box<dyn std::error::Error>> {
    for theme in ThemePreset::ALL {
        let mut prefs = Preferences::default();
        prefs.set_theme(theme);
        let stored = serde_json::to_vec(&prefs)?;
        let restored: Preferences = serde_json::from_slice(&stored)?;
        assert_eq!(restored.theme_preset(), theme);
        assert_eq!(restored.dark, theme.is_dark());
        prefs.set_theme(prefs.theme_preset().with_dark(!prefs.dark));
        assert_eq!(prefs.theme_preset(), theme.with_dark(!theme.is_dark()));
    }
    Ok(())
}
