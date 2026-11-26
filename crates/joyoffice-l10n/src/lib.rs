//! Shared JoyOffice UI localization engine and locale registry.
//!
//! Pure Rust with zero external dependencies, no GUI, and no filesystem access.
//! Product crates ship their own `.lang` resource catalogs and validate them
//! through [`CatalogSet`] so adding new mainstream languages stays uniform and
//! strictly verified across JoyWord, JoyDeck, and JoySheet.
//!
//! Resource format, one entry per line (`#` starts a comment):
//!
//! ```text
//! title = Untitled {number}
//! ribbon.new_slide = New\nSlide
//! documents[one] = {count} document
//! documents[other] = {count} documents
//! ```
//!
//! `{name}` is a placeholder, `{{` / `}}` are literal braces, and `\n`, `\t`,
//! `\\` are unescaped inside message values. Plural variants follow CLDR
//! cardinal categories (`zero`, `one`, `two`, `few`, `many`, `other`).

#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::OnceLock;

/// UI locales supported across JoyOffice desktop applications.
///
/// Adding a new mainstream language (for example `ZhTw`, `Ja`, `Ko`, `Fr`,
/// `De`, `Es`, `PtBr`, `Ru`, `Ar`) requires:
/// 1. Adding a variant here and appending it to [`UiLocale::ALL`].
/// 2. Providing its BCP-47 tag, short label, native autonym, English name,
///    fallback locale, and CLDR cardinal plural rule.
/// 3. Adding matching `<tag>.lang` resource files in `joyoffice-l10n` and each
///    product crate; cross-locale tests automatically verify completeness.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UiLocale {
    #[default]
    ZhCn,
    En,
}

impl UiLocale {
    /// Every UI locale currently shipped across JoyOffice.
    pub const ALL: [UiLocale; 2] = [UiLocale::ZhCn, UiLocale::En];

    /// Canonical BCP-47 language tag used for persistence and CLI/API interchange.
    pub const fn tag(self) -> &'static str {
        match self {
            UiLocale::ZhCn => "zh-CN",
            UiLocale::En => "en",
        }
    }

    /// Compact label for narrow header/toolbar pills.
    pub const fn short_label(self) -> &'static str {
        match self {
            UiLocale::ZhCn => "中文",
            UiLocale::En => "EN",
        }
    }

    /// Autonym shown in language selectors so users can always recognize their own language.
    pub const fn native_name(self) -> &'static str {
        match self {
            UiLocale::ZhCn => "简体中文",
            UiLocale::En => "English",
        }
    }

    /// English display name for accessibility, tooltips, or search.
    pub const fn english_name(self) -> &'static str {
        match self {
            UiLocale::ZhCn => "Simplified Chinese",
            UiLocale::En => "English",
        }
    }

    /// Whether this locale uses right-to-left UI layout.
    pub const fn is_rtl(self) -> bool {
        match self {
            UiLocale::ZhCn | UiLocale::En => false,
        }
    }

    /// Next fallback locale when a catalog key is missing at runtime.
    pub const fn fallback(self) -> Option<UiLocale> {
        match self {
            UiLocale::ZhCn => Some(UiLocale::En),
            UiLocale::En => None,
        }
    }

    /// Legacy short tag used by JoySheet's original `preferences.json`.
    pub const fn legacy_short_tag(self) -> &'static str {
        match self {
            UiLocale::ZhCn => "zh",
            UiLocale::En => "en",
        }
    }

    /// Convenience helper during incremental migration of binary UI branches.
    pub const fn is_zh_cn(self) -> bool {
        matches!(self, UiLocale::ZhCn)
    }

    /// Cycles to the next locale in [`UiLocale::ALL`] (used by legacy toggle shortcuts).
    pub fn next(self) -> UiLocale {
        let index = Self::ALL
            .iter()
            .position(|&candidate| candidate == self)
            .map_or(0, |i| (i + 1) % Self::ALL.len());
        Self::ALL[index]
    }

    /// Parse a BCP-47 or POSIX locale tag (case-insensitive, tolerates `_`,
    /// `.UTF-8`, `@modifier`, and regional subtags).
    pub fn from_tag(tag: &str) -> Option<Self> {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            return None;
        }
        let base = trimmed
            .split(['.', '@'])
            .next()
            .unwrap_or(trimmed)
            .replace('_', "-")
            .to_ascii_lowercase();
        match base.as_str() {
            "zh-cn" | "zh-hans" | "zh" | "zh-hans-cn" | "zh-sg" | "zh-hans-sg" => {
                Some(UiLocale::ZhCn)
            }
            "en" | "en-us" | "en-gb" | "en-au" | "en-ca" | "en-nz" | "en-ie" => Some(UiLocale::En),
            _ => None,
        }
    }

    /// CLDR cardinal plural categories distinguished by this locale.
    pub const fn plural_categories(self) -> &'static [Plural] {
        match self {
            UiLocale::ZhCn => &[Plural::Other],
            UiLocale::En => &[Plural::One, Plural::Other],
        }
    }

    /// CLDR cardinal plural category for a non-negative integer count.
    pub const fn plural(self, count: u64) -> Plural {
        match self {
            UiLocale::ZhCn => Plural::Other,
            UiLocale::En if count == 1 => Plural::One,
            UiLocale::En => Plural::Other,
        }
    }
}

impl fmt::Display for UiLocale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.tag())
    }
}

/// CLDR cardinal plural categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Plural {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl Plural {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "zero" => Some(Plural::Zero),
            "one" => Some(Plural::One),
            "two" => Some(Plural::Two),
            "few" => Some(Plural::Few),
            "many" => Some(Plural::Many),
            "other" => Some(Plural::Other),
            _ => None,
        }
    }
}

/// Validation issue found while parsing or cross-checking `.lang` catalogs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum L10nIssue {
    Syntax {
        locale: UiLocale,
        line: usize,
    },
    Duplicate {
        locale: UiLocale,
        key: String,
    },
    UnknownPlural {
        locale: UiLocale,
        key: String,
    },
    MissingPlural {
        locale: UiLocale,
        key: String,
        category: Plural,
    },
    MixedPlural {
        locale: UiLocale,
        key: String,
    },
    UnbalancedBrace {
        locale: UiLocale,
        key: String,
    },
    MissingKey {
        locale: UiLocale,
        key: String,
    },
    Placeholders {
        key: String,
    },
}

impl fmt::Display for L10nIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Message {
    Plain(String),
    Plural(BTreeMap<Plural, String>),
}

fn unescape_value(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Placeholder names in a template, or `None` for unbalanced braces.
fn placeholders(template: &str) -> Option<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
            }
            '{' => {
                let mut name = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '}' {
                        closed = true;
                        break;
                    }
                    name.push(c);
                }
                if !closed || name.is_empty() || name.contains('{') {
                    return None;
                }
                names.insert(name);
            }
            '}' => return None,
            _ => {}
        }
    }
    Some(names)
}

fn interpolate(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let name: String = chars.by_ref().take_while(|&c| c != '}').collect();
                match args.iter().find(|(key, _)| *key == name) {
                    Some((_, value)) => out.push_str(value),
                    None => {
                        out.push('{');
                        out.push_str(&name);
                        out.push('}');
                    }
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Localized strings for a single [`UiLocale`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    locale: UiLocale,
    messages: BTreeMap<String, Message>,
}

impl Catalog {
    fn parse_inner(locale: UiLocale, text: &str) -> (Catalog, Vec<L10nIssue>) {
        let mut issues = Vec::new();
        let mut messages: BTreeMap<String, Message> = BTreeMap::new();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                issues.push(L10nIssue::Syntax {
                    locale,
                    line: number + 1,
                });
                continue;
            };
            let key = key.trim();
            let value = unescape_value(value.trim());
            if key.is_empty() || value.is_empty() {
                issues.push(L10nIssue::Syntax {
                    locale,
                    line: number + 1,
                });
                continue;
            }
            let (base, plural) = match key.split_once('[') {
                Some((base, rest)) => match rest.strip_suffix(']').and_then(Plural::parse) {
                    Some(plural) => (base.trim(), Some(plural)),
                    None => {
                        issues.push(L10nIssue::UnknownPlural {
                            locale,
                            key: key.to_owned(),
                        });
                        continue;
                    }
                },
                None => (key, None),
            };
            if placeholders(&value).is_none() {
                issues.push(L10nIssue::UnbalancedBrace {
                    locale,
                    key: base.to_owned(),
                });
                continue;
            }
            match plural {
                None => {
                    if messages.contains_key(base) {
                        issues.push(L10nIssue::Duplicate {
                            locale,
                            key: base.to_owned(),
                        });
                    } else {
                        messages.insert(base.to_owned(), Message::Plain(value));
                    }
                }
                Some(category) => {
                    if !locale.plural_categories().contains(&category) {
                        issues.push(L10nIssue::UnknownPlural {
                            locale,
                            key: key.to_owned(),
                        });
                        continue;
                    }
                    match messages.get_mut(base) {
                        Some(Message::Plain(_)) => issues.push(L10nIssue::MixedPlural {
                            locale,
                            key: base.to_owned(),
                        }),
                        Some(Message::Plural(map)) => {
                            if map.insert(category, value).is_some() {
                                issues.push(L10nIssue::Duplicate {
                                    locale,
                                    key: key.to_owned(),
                                });
                            }
                        }
                        None => {
                            let mut map = BTreeMap::new();
                            map.insert(category, value);
                            messages.insert(base.to_owned(), Message::Plural(map));
                        }
                    }
                }
            }
        }
        for (key, message) in &messages {
            if let Message::Plural(map) = message {
                for &required in locale.plural_categories() {
                    if !map.contains_key(&required) {
                        issues.push(L10nIssue::MissingPlural {
                            locale,
                            key: key.clone(),
                            category: required,
                        });
                    }
                }
            }
        }
        (Catalog { locale, messages }, issues)
    }

    pub fn parse(locale: UiLocale, text: &str) -> Result<Catalog, Vec<L10nIssue>> {
        let (catalog, issues) = Self::parse_inner(locale, text);
        if issues.is_empty() {
            Ok(catalog)
        } else {
            Err(issues)
        }
    }

    /// Parse tolerantly for runtime `OnceLock` initialization; strict tests use [`Catalog::parse`].
    pub fn parse_lossy(locale: UiLocale, text: &str) -> Catalog {
        Self::parse_inner(locale, text).0
    }

    pub fn locale(&self) -> UiLocale {
        self.locale
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.messages.contains_key(key)
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        match self.messages.get(key) {
            Some(Message::Plain(text)) => Some(text.as_str()),
            _ => None,
        }
    }

    /// Plain text for `key`, or `key` itself when missing so gaps stay visible.
    pub fn text(&self, key: &str) -> String {
        self.format_or_key(key, &[])
    }

    pub fn format(&self, key: &str, args: &[(&str, &str)]) -> Option<String> {
        self.get(key).map(|template| interpolate(template, args))
    }

    /// Format a plain or plural (`other`) entry, falling back to `key` when missing.
    pub fn format_or_key(&self, key: &str, args: &[(&str, &str)]) -> String {
        match self.messages.get(key) {
            Some(Message::Plain(template)) => interpolate(template, args),
            Some(Message::Plural(map)) => {
                interpolate(map.get(&Plural::Other).map_or(key, String::as_str), args)
            }
            None => key.to_owned(),
        }
    }

    /// Format a pluralized message; `{count}` is bound automatically from `count`.
    pub fn plural(&self, key: &str, count: u64, args: &[(&str, &str)]) -> Option<String> {
        let count_text = count.to_string();
        let mut all = Vec::with_capacity(args.len() + 1);
        all.push(("count", count_text.as_str()));
        all.extend_from_slice(args);
        match self.messages.get(key) {
            Some(Message::Plural(map)) => {
                let category = self.locale.plural(count);
                let template = map.get(&category).or_else(|| map.get(&Plural::Other))?;
                Some(interpolate(template, &all))
            }
            Some(Message::Plain(template)) => Some(interpolate(template, &all)),
            None => None,
        }
    }

    /// Plural message selected by `count`, falling back to `key` when missing.
    pub fn format_count(&self, key: &str, count: u64, args: &[(&str, &str)]) -> String {
        self.plural(key, count, args)
            .unwrap_or_else(|| key.to_owned())
    }

    fn key_placeholders(&self, key: &str) -> Option<BTreeSet<String>> {
        match self.messages.get(key)? {
            Message::Plain(text) => placeholders(text),
            Message::Plural(map) => {
                let mut union = BTreeSet::new();
                for text in map.values() {
                    union.extend(placeholders(text)?);
                }
                union.remove("count");
                Some(union)
            }
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.messages.keys().map(String::as_str)
    }
}

/// Cross-locale checks: same keys, same placeholders across all catalogs.
pub fn validate_catalogs(catalogs: &[&Catalog]) -> Vec<L10nIssue> {
    let mut issues = Vec::new();
    let all_keys: BTreeSet<&str> = catalogs.iter().flat_map(|catalog| catalog.keys()).collect();
    for key in all_keys {
        let mut reference: Option<BTreeSet<String>> = None;
        let mut placeholder_mismatch = false;
        for catalog in catalogs {
            if !catalog.messages.contains_key(key) {
                issues.push(L10nIssue::MissingKey {
                    locale: catalog.locale,
                    key: key.to_owned(),
                });
                continue;
            }
            if let Some(names) = catalog.key_placeholders(key) {
                match &reference {
                    None => reference = Some(names),
                    Some(expected) if expected != &names => placeholder_mismatch = true,
                    Some(_) => {}
                }
            }
        }
        if placeholder_mismatch {
            issues.push(L10nIssue::Placeholders {
                key: key.to_owned(),
            });
        }
    }
    issues
}

/// Multi-locale catalog container with automatic fallback chain lookup.
#[derive(Clone, Debug)]
pub struct CatalogSet {
    catalogs: BTreeMap<UiLocale, Catalog>,
}

impl CatalogSet {
    /// Strict constructor for tests and validation gates.
    pub fn parse_strict(sources: &[(UiLocale, &str)]) -> Result<Self, Vec<L10nIssue>> {
        let mut issues = Vec::new();
        let mut catalogs = BTreeMap::new();
        for &(locale, text) in sources {
            let (catalog, mut parse_issues) = Catalog::parse_inner(locale, text);
            issues.append(&mut parse_issues);
            catalogs.insert(locale, catalog);
        }
        let refs: Vec<&Catalog> = catalogs.values().collect();
        issues.extend(validate_catalogs(&refs));
        if issues.is_empty() {
            Ok(Self { catalogs })
        } else {
            Err(issues)
        }
    }

    /// Tolerant constructor for static runtime catalogs.
    pub fn from_sources_lossy(sources: &[(UiLocale, &str)]) -> Self {
        let mut catalogs = BTreeMap::new();
        for &(locale, text) in sources {
            catalogs.insert(locale, Catalog::parse_lossy(locale, text));
        }
        Self { catalogs }
    }

    pub fn catalog(&self, locale: UiLocale) -> Option<&Catalog> {
        self.catalogs.get(&locale)
    }

    /// Look up a plain key following the locale's fallback chain.
    pub fn try_get(&self, locale: UiLocale, key: &str) -> Option<&str> {
        let mut cursor = Some(locale);
        let mut depth = 0usize;
        while let Some(current) = cursor {
            if let Some(value) = self.catalogs.get(&current).and_then(|c| c.get(key)) {
                return Some(value);
            }
            depth += 1;
            if depth > UiLocale::ALL.len() {
                break;
            }
            cursor = current.fallback();
        }
        None
    }

    /// Look up a plain key, returning `key` itself if absent in every fallback locale.
    pub fn text<'a>(&'a self, locale: UiLocale, key: &'a str) -> &'a str {
        self.try_get(locale, key).unwrap_or(key)
    }

    /// Format an interpolated message following the fallback chain.
    pub fn format(&self, locale: UiLocale, key: &str, args: &[(&str, &str)]) -> String {
        let mut cursor = Some(locale);
        let mut depth = 0usize;
        while let Some(current) = cursor {
            if let Some(value) = self
                .catalogs
                .get(&current)
                .and_then(|c| c.format(key, args))
            {
                return value;
            }
            depth += 1;
            if depth > UiLocale::ALL.len() {
                break;
            }
            cursor = current.fallback();
        }
        interpolate(key, args)
    }

    /// Format a pluralized message following the fallback chain.
    pub fn plural(&self, locale: UiLocale, key: &str, count: u64, args: &[(&str, &str)]) -> String {
        let mut cursor = Some(locale);
        let mut depth = 0usize;
        while let Some(current) = cursor {
            if let Some(value) = self
                .catalogs
                .get(&current)
                .and_then(|c| c.plural(key, count, args))
            {
                return value;
            }
            depth += 1;
            if depth > UiLocale::ALL.len() {
                break;
            }
            cursor = current.fallback();
        }
        key.to_owned()
    }
}

const SHARED_SOURCES: &[(UiLocale, &str)] = &[
    (UiLocale::ZhCn, include_str!("../locales/zh-CN.lang")),
    (UiLocale::En, include_str!("../locales/en.lang")),
];

/// Shared JoyOffice UI strings (themes, language selector, common chrome).
pub fn shared() -> &'static CatalogSet {
    static SHARED: OnceLock<CatalogSet> = OnceLock::new();
    SHARED.get_or_init(|| CatalogSet::from_sources_lossy(SHARED_SOURCES))
}

/// Look up a shared JoyOffice UI string by key.
pub fn shared_text(locale: UiLocale, key: &'static str) -> &'static str {
    shared().text(locale, key)
}

/// Look up a shared JoyOffice theme preset label for any [`UiLocale`].
pub fn theme_label(preset_id: &str, locale: UiLocale) -> &'static str {
    let key = match preset_id {
        "verdant-light" => "theme.verdant-light",
        "verdant-dark" => "theme.verdant-dark",
        "classic-light" => "theme.classic-light",
        "classic-dark" => "theme.classic-dark",
        "azure-light" => "theme.azure-light",
        "azure-dark" => "theme.azure-dark",
        "sandstone-light" => "theme.sandstone-light",
        "sandstone-dark" => "theme.sandstone-dark",
        _ => return "",
    };
    shared_text(locale, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_tags_parse_bcp47_and_posix_variants() {
        for tag in [
            "zh",
            "zh-CN",
            "zh_CN",
            "zh-Hans",
            "zh_CN.UTF-8",
            "ZH-HANS-CN",
        ] {
            assert_eq!(UiLocale::from_tag(tag), Some(UiLocale::ZhCn), "{tag}");
        }
        for tag in ["en", "en-US", "en_GB.UTF-8", "EN-AU", "en_US@euro"] {
            assert_eq!(UiLocale::from_tag(tag), Some(UiLocale::En), "{tag}");
        }
        assert_eq!(UiLocale::from_tag(""), None);
        assert_eq!(UiLocale::from_tag("xx-ZZ"), None);
    }

    #[test]
    fn shared_catalogs_are_complete_and_valid() {
        let parsed = CatalogSet::parse_strict(SHARED_SOURCES);
        assert!(parsed.is_ok(), "shared catalog issues: {:?}", parsed.err());
        for locale in UiLocale::ALL {
            assert!(!shared_text(locale, "language.section").is_empty());
            assert!(!theme_label("verdant-light", locale).is_empty());
            assert!(!theme_label("classic-dark", locale).is_empty());
        }
    }

    #[test]
    fn unescapes_newlines_and_interpolates_placeholders() -> Result<(), Vec<L10nIssue>> {
        let cat = Catalog::parse(UiLocale::En, "btn = New\\nSlide {n} {{ok}}\n")?;
        assert_eq!(cat.get("btn"), Some("New\nSlide {n} {{ok}}"));
        assert_eq!(
            cat.format("btn", &[("n", "2")]).as_deref(),
            Some("New\nSlide 2 {ok}")
        );
        Ok(())
    }

    #[test]
    fn fallback_chain_resolves_missing_key_from_fallback_locale() {
        let set = CatalogSet::from_sources_lossy(&[
            (UiLocale::ZhCn, "present = 你好\n"),
            (UiLocale::En, "present = Hello\nonly_en = Fallback\n"),
        ]);
        assert_eq!(set.text(UiLocale::ZhCn, "present"), "你好");
        assert_eq!(set.text(UiLocale::ZhCn, "only_en"), "Fallback");
        assert_eq!(set.text(UiLocale::ZhCn, "absent"), "absent");
    }

    #[test]
    fn plural_rules_and_validation_catch_mismatches() -> Result<(), Vec<L10nIssue>> {
        let en = Catalog::parse(
            UiLocale::En,
            "slides[one] = {count} slide\nslides[other] = {count} slides\n",
        )?;
        assert_eq!(en.plural("slides", 1, &[]).as_deref(), Some("1 slide"));
        assert_eq!(en.plural("slides", 3, &[]).as_deref(), Some("3 slides"));

        let bad = CatalogSet::parse_strict(&[
            (UiLocale::ZhCn, "a = 仅中文 {x}\n"),
            (UiLocale::En, "a = English {y}\n"),
        ]);
        assert_eq!(
            bad.err(),
            Some(vec![L10nIssue::Placeholders { key: "a".into() }])
        );
        Ok(())
    }
}
