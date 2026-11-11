use super::*;
use joyoffice_theme::ThemePreset;

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
