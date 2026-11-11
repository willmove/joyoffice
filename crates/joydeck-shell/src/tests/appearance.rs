use crate::storage::{load_theme, save_theme};
use joyoffice_theme::ThemePreset;

#[test]
fn appearance_store_round_trips_and_rejects_unknown_or_oversized_input() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "joydeck-appearance-{}-{}",
        std::process::id(),
        crate::storage::test_preference_id()
    ));
    assert_eq!(load_theme(&root)?, ThemePreset::ClassicLight);
    for theme in ThemePreset::ALL {
        save_theme(&root, theme)?;
        assert_eq!(load_theme(&root)?, theme);
    }
    let path = root.join("appearance.v1");
    for invalid in ["future-theme".to_string(), "x".repeat(129)] {
        std::fs::write(&path, &invalid).map_err(|e| e.to_string())?;
        assert!(load_theme(&root).is_err());
        assert_eq!(
            std::fs::read_to_string(&path).map_err(|e| e.to_string())?,
            invalid
        );
    }
    std::fs::remove_dir_all(root).map_err(|e| e.to_string())
}
