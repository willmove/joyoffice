//! Localized UI strings with interpolation and plural categories
//! (PLATFORM-003, T106).
//!
//! Resource format, one entry per line (`#` starts a comment):
//!
//! ```text
//! title = Untitled {number}
//! documents[one] = {count} document
//! documents[other] = {count} documents
//! ```
//!
//! `{name}` is a placeholder and `{{` / `}}` are literal braces. Plural
//! variants follow the CLDR cardinal categories each locale uses: Chinese has
//! only `other`; English has `one` and `other`. Validation requires the same
//! keys and the same placeholders in every locale, so a missing translation
//! or argument is caught by tests, not by users.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::OnceLock;

/// UI locales shipped with JoyWord.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Locale {
    ZhCn,
    En,
}

impl Locale {
    pub const ALL: [Locale; 2] = [Locale::ZhCn, Locale::En];

    pub fn tag(self) -> &'static str {
        match self {
            Locale::ZhCn => "zh-CN",
            Locale::En => "en",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag.to_ascii_lowercase().as_str() {
            "zh-cn" | "zh-hans" | "zh" | "zh-hans-cn" => Some(Locale::ZhCn),
            "en" | "en-us" | "en-gb" => Some(Locale::En),
            _ => None,
        }
    }

    /// CLDR cardinal categories this locale distinguishes.
    pub fn plural_categories(self) -> &'static [Plural] {
        match self {
            Locale::ZhCn => &[Plural::Other],
            Locale::En => &[Plural::One, Plural::Other],
        }
    }

    /// CLDR cardinal rule for a non-negative integer count.
    pub fn plural(self, count: u64) -> Plural {
        match self {
            Locale::ZhCn => Plural::Other,
            Locale::En if count == 1 => Plural::One,
            Locale::En => Plural::Other,
        }
    }

    fn resource(self) -> &'static str {
        match self {
            Locale::ZhCn => include_str!("../locales/zh-CN.lang"),
            Locale::En => include_str!("../locales/en.lang"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Plural {
    One,
    Other,
}

impl Plural {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "one" => Some(Plural::One),
            "other" => Some(Plural::Other),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum L10nIssue {
    Syntax {
        locale: Locale,
        line: usize,
    },
    Duplicate {
        locale: Locale,
        key: String,
    },
    UnknownPlural {
        locale: Locale,
        key: String,
    },
    MissingPlural {
        locale: Locale,
        key: String,
        category: Plural,
    },
    MixedPlural {
        locale: Locale,
        key: String,
    },
    UnbalancedBrace {
        locale: Locale,
        key: String,
    },
    MissingKey {
        locale: Locale,
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
                    // Validation makes this unreachable for shipped strings;
                    // a visible placeholder beats a silently dropped value.
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

/// Strings of one locale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    locale: Locale,
    messages: BTreeMap<String, Message>,
}

impl Catalog {
    pub fn parse(locale: Locale, text: &str) -> Result<Catalog, Vec<L10nIssue>> {
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
            let (key, value) = (key.trim(), value.trim().to_owned());
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
            }
            match (messages.get_mut(base), plural) {
                (None, None) => {
                    messages.insert(base.to_owned(), Message::Plain(value));
                }
                (None, Some(plural)) => {
                    messages.insert(
                        base.to_owned(),
                        Message::Plural(BTreeMap::from([(plural, value)])),
                    );
                }
                (Some(Message::Plural(forms)), Some(plural)) => {
                    if forms.insert(plural, value).is_some() {
                        issues.push(L10nIssue::Duplicate {
                            locale,
                            key: key.to_owned(),
                        });
                    }
                }
                (Some(Message::Plain(_)), None) => issues.push(L10nIssue::Duplicate {
                    locale,
                    key: key.to_owned(),
                }),
                (Some(_), _) => issues.push(L10nIssue::MixedPlural {
                    locale,
                    key: base.to_owned(),
                }),
            }
        }
        for (key, message) in &messages {
            if let Message::Plural(forms) = message {
                for &category in locale.plural_categories() {
                    if !forms.contains_key(&category) {
                        issues.push(L10nIssue::MissingPlural {
                            locale,
                            key: key.clone(),
                            category,
                        });
                    }
                }
                if forms
                    .keys()
                    .any(|form| !locale.plural_categories().contains(form))
                {
                    issues.push(L10nIssue::UnknownPlural {
                        locale,
                        key: key.clone(),
                    });
                }
            }
        }
        if issues.is_empty() {
            Ok(Catalog { locale, messages })
        } else {
            Err(issues)
        }
    }

    /// Shipped catalog for `locale`; resources are validated by tests.
    pub fn shipped(locale: Locale) -> &'static Catalog {
        static CATALOGS: OnceLock<Vec<Catalog>> = OnceLock::new();
        let catalogs = CATALOGS.get_or_init(|| {
            Locale::ALL
                .iter()
                .map(|&locale| {
                    Catalog::parse(locale, locale.resource()).unwrap_or_else(|issues| {
                        panic!("invalid {} resource: {issues:?}", locale.tag())
                    })
                })
                .collect()
        });
        &catalogs[Locale::ALL
            .iter()
            .position(|&l| l == locale)
            .expect("shipped locale")]
    }

    pub fn locale(&self) -> Locale {
        self.locale
    }

    pub fn contains(&self, key: &str) -> bool {
        self.messages.contains_key(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.messages.keys().map(String::as_str)
    }

    /// Text for `key`; the key itself when missing, so gaps stay visible.
    pub fn text(&self, key: &str) -> String {
        self.format(key, &[])
    }

    pub fn format(&self, key: &str, args: &[(&str, &str)]) -> String {
        match self.messages.get(key) {
            Some(Message::Plain(template)) => interpolate(template, args),
            Some(Message::Plural(forms)) => {
                interpolate(forms.get(&Plural::Other).map_or(key, String::as_str), args)
            }
            None => key.to_owned(),
        }
    }

    /// Plural message selected by `count`, which is also available as the
    /// `{count}` placeholder.
    pub fn format_count(&self, key: &str, count: u64, args: &[(&str, &str)]) -> String {
        let count_text = count.to_string();
        let mut all = vec![("count", count_text.as_str())];
        all.extend_from_slice(args);
        match self.messages.get(key) {
            Some(Message::Plural(forms)) => {
                let category = self.locale.plural(count);
                let template = forms
                    .get(&category)
                    .or_else(|| forms.get(&Plural::Other))
                    .map_or(key, String::as_str);
                interpolate(template, &all)
            }
            _ => self.format(key, &all),
        }
    }

    fn placeholder_set(&self, key: &str) -> BTreeSet<String> {
        match self.messages.get(key) {
            Some(Message::Plain(template)) => placeholders(template).unwrap_or_default(),
            Some(Message::Plural(forms)) => forms
                .values()
                .flat_map(|template| placeholders(template).unwrap_or_default())
                .collect(),
            None => BTreeSet::new(),
        }
    }
}

/// Cross-locale checks: same keys, same placeholders (for plural messages,
/// the union over forms, with `{count}` implied).
pub fn check_parity(catalogs: &[&Catalog]) -> Vec<L10nIssue> {
    let mut issues = Vec::new();
    let all_keys: BTreeSet<&str> = catalogs.iter().flat_map(|catalog| catalog.keys()).collect();
    for key in all_keys {
        for catalog in catalogs {
            if !catalog.contains(key) {
                issues.push(L10nIssue::MissingKey {
                    locale: catalog.locale,
                    key: key.to_owned(),
                });
            }
        }
        let sets: Vec<BTreeSet<String>> = catalogs
            .iter()
            .filter(|catalog| catalog.contains(key))
            .map(|catalog| {
                let mut set = catalog.placeholder_set(key);
                set.remove("count");
                set
            })
            .collect();
        if sets.windows(2).any(|pair| pair[0] != pair[1]) {
            issues.push(L10nIssue::Placeholders {
                key: key.to_owned(),
            });
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_resources_are_valid_and_in_parity() {
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
            key: "n".into()
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
