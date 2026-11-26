//! Localized UI strings for JoyWord, backed by the shared `joyoffice-l10n` engine
//! (PLATFORM-003, T106).

use std::sync::OnceLock;

pub use joyoffice_l10n::{L10nIssue, Plural, UiLocale as Locale};

const SHIPPED_SOURCES: &[(Locale, &str)] = &[
    (Locale::ZhCn, include_str!("../locales/zh-CN.lang")),
    (Locale::En, include_str!("../locales/en.lang")),
];

/// Strings of one locale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    inner: joyoffice_l10n::Catalog,
}

impl Catalog {
    pub fn parse(locale: Locale, text: &str) -> Result<Catalog, Vec<L10nIssue>> {
        Ok(Catalog {
            inner: joyoffice_l10n::Catalog::parse(locale, text)?,
        })
    }

    /// Shipped catalog for `locale`; resources are validated by tests.
    pub fn shipped(locale: Locale) -> &'static Catalog {
        static CATALOGS: OnceLock<Vec<(Locale, Catalog)>> = OnceLock::new();
        let list = CATALOGS.get_or_init(|| {
            SHIPPED_SOURCES
                .iter()
                .map(|&(loc, text)| {
                    (
                        loc,
                        Catalog {
                            inner: joyoffice_l10n::Catalog::parse_lossy(loc, text),
                        },
                    )
                })
                .collect()
        });
        list.iter()
            .find(|(loc, _)| *loc == locale)
            .or_else(|| list.first())
            .map_or_else(
                || {
                    static EMPTY: OnceLock<Catalog> = OnceLock::new();
                    EMPTY.get_or_init(|| Catalog {
                        inner: joyoffice_l10n::Catalog::parse_lossy(Locale::En, ""),
                    })
                },
                |(_, catalog)| catalog,
            )
    }

    pub fn locale(&self) -> Locale {
        self.inner.locale()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.inner.contains(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.inner.keys()
    }

    /// Text for `key`; the key itself when missing, so gaps stay visible.
    pub fn text(&self, key: &str) -> String {
        self.inner.text(key)
    }

    pub fn format(&self, key: &str, args: &[(&str, &str)]) -> String {
        self.inner.format_or_key(key, args)
    }

    /// Plural message selected by `count`, which is also available as the
    /// `{count}` placeholder.
    pub fn format_count(&self, key: &str, count: u64, args: &[(&str, &str)]) -> String {
        self.inner.format_count(key, count, args)
    }
}

/// Cross-locale checks: same keys, same placeholders (for plural messages,
/// the union over forms, with `{count}` implied).
pub fn check_parity(catalogs: &[&Catalog]) -> Vec<L10nIssue> {
    let inner: Vec<&joyoffice_l10n::Catalog> = catalogs.iter().map(|c| &c.inner).collect();
    joyoffice_l10n::validate_catalogs(&inner)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Test setup and assertions")]
mod tests {
    use super::*;

    #[test]
    fn shipped_resources_are_valid_and_in_parity() {
        let strict = joyoffice_l10n::CatalogSet::parse_strict(SHIPPED_SOURCES);
        assert_eq!(strict.err(), None);
        let catalogs: Vec<&Catalog> = Locale::ALL.iter().map(|&l| Catalog::shipped(l)).collect();
        assert_eq!(check_parity(&catalogs), vec![]);
    }

    #[test]
    fn interpolation_and_plurals_follow_cldr_categories() {
        let en = Catalog::parse(
            Locale::En,
            "untitled = Untitled {number}\nopen[one] = {count} document open\nopen[other] = {count} documents open\nbraces = {{literal}}",
        )
        .unwrap();
        let zh = Catalog::parse(
            Locale::ZhCn,
            "untitled = 未命名 {number}\nopen[other] = 已打开 {count} 个文档\nbraces = {{literal}}",
        )
        .unwrap();
        assert_eq!(en.format("untitled", &[("number", "2")]), "Untitled 2");
        assert_eq!(zh.format("untitled", &[("number", "2")]), "未命名 2");
        assert_eq!(en.format_count("open", 1, &[]), "1 document open");
        assert_eq!(en.format_count("open", 0, &[]), "0 documents open");
        assert_eq!(en.format_count("open", 21, &[]), "21 documents open");
        assert_eq!(zh.format_count("open", 1, &[]), "已打开 1 个文档");
        assert_eq!(en.text("braces"), "{literal}");
        assert_eq!(en.text("missing.key"), "missing.key");
        assert_eq!(en.format("untitled", &[]), "Untitled {number}");
        assert!(check_parity(&[&en, &zh]).is_empty());
    }

    #[test]
    fn invalid_resources_are_reported() {
        let issues = Catalog::parse(
            Locale::En,
            "a = x\na = y\nb[few] = z\nc[one] = only one\nd = {open\nnoequals\ne = plain\ne[other] = plural",
        )
        .unwrap_err();
        assert!(issues.contains(&L10nIssue::Duplicate {
            locale: Locale::En,
            key: "a".into()
        }));
        assert!(issues.contains(&L10nIssue::UnknownPlural {
            locale: Locale::En,
            key: "b[few]".into()
        }));
        assert!(issues.contains(&L10nIssue::MissingPlural {
            locale: Locale::En,
            key: "c".into(),
            category: Plural::Other
        }));
        assert!(issues.contains(&L10nIssue::UnbalancedBrace {
            locale: Locale::En,
            key: "d".into()
        }));
        assert!(issues.contains(&L10nIssue::Syntax {
            locale: Locale::En,
            line: 6
        }));
        assert!(issues.contains(&L10nIssue::MixedPlural {
            locale: Locale::En,
            key: "e".into()
        }));
        // Chinese has no `one` category.
        let zh = Catalog::parse(Locale::ZhCn, "n[one] = 一\nn[other] = 多").unwrap_err();
        assert!(zh.contains(&L10nIssue::UnknownPlural {
            locale: Locale::ZhCn,
            key: "n[one]".into()
        }));

        let en = Catalog::parse(Locale::En, "k = {name} saved\nonly = x").unwrap();
        let zh = Catalog::parse(Locale::ZhCn, "k = 已保存").unwrap();
        let parity = check_parity(&[&en, &zh]);
        assert!(parity.contains(&L10nIssue::Placeholders { key: "k".into() }));
        assert!(parity.contains(&L10nIssue::MissingKey {
            locale: Locale::ZhCn,
            key: "only".into()
        }));
    }
}
