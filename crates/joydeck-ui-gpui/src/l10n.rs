//! JoyDeck UI localization backed by `joyoffice-l10n` `.lang` catalogs.
use joyoffice_ui::{CatalogSet, UiLocale};
use std::sync::OnceLock;

const SOURCES: &[(UiLocale, &str)] = &[
    (UiLocale::ZhCn, include_str!("../locales/zh-CN.lang")),
    (UiLocale::En, include_str!("../locales/en.lang")),
];

pub(crate) fn catalog() -> &'static CatalogSet {
    static CATALOG: OnceLock<CatalogSet> = OnceLock::new();
    CATALOG.get_or_init(|| CatalogSet::from_sources_lossy(SOURCES))
}

/// Look up a static UI string for `locale` with automatic fallback chain.
pub(crate) fn t(locale: UiLocale, key: &'static str) -> &'static str {
    catalog().text(locale, key)
}

/// Format an interpolated UI string for `locale`.
pub(crate) fn tf(locale: UiLocale, key: &str, args: &[(&str, &str)]) -> String {
    catalog().format(locale, key, args)
}

/// Display name for a built-in presentation theme (matching internal model name).
pub(crate) fn presentation_theme_name(locale: UiLocale, internal_name: &str) -> String {
    let key = match internal_name {
        "现代商务" => "pres_theme.modern_business.name",
        "深空科技" => "pres_theme.deep_space.name",
        "极简留白" => "pres_theme.minimal_white.name",
        "党政赤金" => "pres_theme.crimson_gold.name",
        "咨询藏青" => "pres_theme.consulting_navy.name",
        "学术青蓝" => "pres_theme.academic_teal.name",
        "新中式韵" => "pres_theme.oriental_ink.name",
        "生态青绿" => "pres_theme.eco_green.name",
        "暖阳陶土" => "pres_theme.warm_terracotta.name",
        "黑金奢雅" => "pres_theme.black_gold.name",
        _ => return internal_name.to_owned(),
    };
    t(locale, key).to_owned()
}

/// Localized scene usage hint for a built-in presentation theme.
pub(crate) fn presentation_theme_hint(locale: UiLocale, internal_name: &str) -> &'static str {
    let key = match internal_name {
        "现代商务" => "pres_theme.modern_business.hint",
        "深空科技" => "pres_theme.deep_space.hint",
        "极简留白" => "pres_theme.minimal_white.hint",
        "党政赤金" => "pres_theme.crimson_gold.hint",
        "咨询藏青" => "pres_theme.consulting_navy.hint",
        "学术青蓝" => "pres_theme.academic_teal.hint",
        "新中式韵" => "pres_theme.oriental_ink.hint",
        "生态青绿" => "pres_theme.eco_green.hint",
        "暖阳陶土" => "pres_theme.warm_terracotta.hint",
        "黑金奢雅" => "pres_theme.black_gold.hint",
        _ => "pres_theme.default_hint",
    };
    t(locale, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_joydeck_catalogs_are_valid_and_in_parity() {
        let parsed = CatalogSet::parse_strict(SOURCES);
        assert_eq!(parsed.err(), None);
        for locale in UiLocale::ALL {
            assert!(!t(locale, "tab.home").is_empty());
            assert!(!t(locale, "file.nav.options").is_empty());
            assert_ne!(
                presentation_theme_name(locale, "现代商务"),
                "pres_theme.modern_business.name"
            );
        }
    }
}
