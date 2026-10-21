//! T400: the shell links the AI runtime only with `--features ai`.

#[cfg(not(feature = "ai"))]
#[test]
fn ai_feature_is_absent_from_the_default_shell() {
    assert!(option_env!("CARGO_FEATURE_AI").is_none());
}

#[cfg(feature = "ai")]
#[test]
fn startup_with_ai_feature_makes_no_provider_request() {
    let mut toggle = joyword_shell::ai::AiToggle::new();
    assert_eq!(toggle.startup_provider_requests(), 0);
    let settings = toggle.settings().expect("settings");
    assert!(!settings.user_enabled);
    assert!(!settings.effective_enabled);
    assert!(settings.entry_available);
    assert!(!settings.force_disabled);
    toggle
        .apply_admin_policy(joyword_agent::AdminPolicy::force_disabled())
        .expect("policy");
    let managed = toggle.settings().expect("settings");
    assert!(managed.managed);
    assert!(!managed.entry_available);
    assert_eq!(toggle.startup_provider_requests(), 0);
}
