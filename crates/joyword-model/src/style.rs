//! Style sheet storage and bounded inheritance (ARCHITECTURE §2.2, STYLE-001).
//!
//! A style's `w:styleId` is a document-local key: Chinese Word writes `a` for
//! Normal and `1` for heading 1, and WPS uses other numeric IDs. Built-in
//! styles are therefore recognised by their canonical `w:name` (ECMA-376
//! Part 1 §17.7.4), compared ASCII-case-insensitively. Localized display names
//! belong to the UI and are never stored as the canonical name.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::sync::Arc;

use crate::id::{PreservedId, SourceRef};
use crate::props::{ParaProps, RunProps};

/// Longest `basedOn` chain followed before reporting [`StyleProblem::TooDeep`].
pub const MAX_STYLE_DEPTH: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StyleId(Arc<str>);

impl StyleId {
    pub fn new(id: &str) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StyleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StyleKind {
    Paragraph,
    Character,
    Table,
    Numbering,
}

impl fmt::Display for StyleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            StyleKind::Paragraph => "paragraph",
            StyleKind::Character => "character",
            StyleKind::Table => "table",
            StyleKind::Numbering => "numbering",
        })
    }
}

/// Built-in styles JoyWord needs to identify; extended as features arrive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltinStyle {
    Normal,
    /// Heading level 1–9.
    Heading(u8),
    Title,
    Subtitle,
    DefaultParagraphFont,
    NormalTable,
    NoList,
    TableGrid,
    ListParagraph,
    Caption,
    Header,
    Footer,
    FootnoteText,
    FootnoteReference,
    CommentText,
    CommentReference,
    Hyperlink,
    /// Table of contents level 1–9.
    Toc(u8),
}

impl BuiltinStyle {
    /// Canonical `w:name`, as Word writes it.
    pub fn canonical_name(self) -> String {
        match self {
            BuiltinStyle::Normal => "Normal".into(),
            BuiltinStyle::Heading(level) => format!("heading {level}"),
            BuiltinStyle::Title => "Title".into(),
            BuiltinStyle::Subtitle => "Subtitle".into(),
            BuiltinStyle::DefaultParagraphFont => "Default Paragraph Font".into(),
            BuiltinStyle::NormalTable => "Normal Table".into(),
            BuiltinStyle::NoList => "No List".into(),
            BuiltinStyle::TableGrid => "Table Grid".into(),
            BuiltinStyle::ListParagraph => "List Paragraph".into(),
            BuiltinStyle::Caption => "caption".into(),
            BuiltinStyle::Header => "header".into(),
            BuiltinStyle::Footer => "footer".into(),
            BuiltinStyle::FootnoteText => "footnote text".into(),
            BuiltinStyle::FootnoteReference => "footnote reference".into(),
            BuiltinStyle::CommentText => "annotation text".into(),
            BuiltinStyle::CommentReference => "annotation reference".into(),
            BuiltinStyle::Hyperlink => "Hyperlink".into(),
            BuiltinStyle::Toc(level) => format!("toc {level}"),
        }
    }

    pub fn from_canonical_name(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        let leveled = |prefix: &str| -> Option<u8> {
            let level: u8 = name.strip_prefix(prefix)?.parse().ok()?;
            (1..=9).contains(&level).then_some(level)
        };
        if let Some(level) = leveled("heading ") {
            return Some(BuiltinStyle::Heading(level));
        }
        if let Some(level) = leveled("toc ") {
            return Some(BuiltinStyle::Toc(level));
        }
        Some(match name.as_str() {
            "normal" => BuiltinStyle::Normal,
            "title" => BuiltinStyle::Title,
            "subtitle" => BuiltinStyle::Subtitle,
            "default paragraph font" => BuiltinStyle::DefaultParagraphFont,
            "normal table" => BuiltinStyle::NormalTable,
            "no list" => BuiltinStyle::NoList,
            "table grid" => BuiltinStyle::TableGrid,
            "list paragraph" => BuiltinStyle::ListParagraph,
            "caption" => BuiltinStyle::Caption,
            "header" => BuiltinStyle::Header,
            "footer" => BuiltinStyle::Footer,
            "footnote text" => BuiltinStyle::FootnoteText,
            "footnote reference" => BuiltinStyle::FootnoteReference,
            "annotation text" => BuiltinStyle::CommentText,
            "annotation reference" => BuiltinStyle::CommentReference,
            "hyperlink" => BuiltinStyle::Hyperlink,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub id: StyleId,
    pub kind: StyleKind,
    /// Canonical `w:name`; `None` when the source omitted it.
    pub name: Option<Arc<str>>,
    pub based_on: Option<StyleId>,
    pub next: Option<StyleId>,
    pub link: Option<StyleId>,
    /// `w:default`: the default style of its kind.
    pub is_default: bool,
    pub custom: bool,
    pub para: ParaProps,
    pub run: RunProps,
    pub source: Option<Arc<SourceRef>>,
    pub preserved: Option<PreservedId>,
}

impl Style {
    pub fn new(id: &str, kind: StyleKind, name: Option<&str>) -> Self {
        Self {
            id: StyleId::new(id),
            kind,
            name: name.map(Into::into),
            based_on: None,
            next: None,
            link: None,
            is_default: false,
            custom: false,
            para: ParaProps::default(),
            run: RunProps::default(),
            source: None,
            preserved: None,
        }
    }

    pub fn builtin(&self) -> Option<BuiltinStyle> {
        self.name
            .as_deref()
            .and_then(BuiltinStyle::from_canonical_name)
    }
}

/// `w:docDefaults`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocDefaults {
    pub run: RunProps,
    pub para: ParaProps,
}

/// Why a `basedOn` chain stopped early. The chain up to that point is still
/// returned so resolution can fall back instead of failing.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum StyleProblem {
    Missing(StyleId),
    Cycle(StyleId),
    TooDeep,
    KindMismatch { style: StyleId, base: StyleId },
}

/// A style followed by its ancestors, nearest first.
#[derive(Clone, Debug)]
pub struct StyleChain {
    pub styles: Vec<Arc<Style>>,
    pub problem: Option<StyleProblem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StyleError {
    DuplicateId(StyleId),
}

#[derive(Clone, Debug, Default)]
pub struct StyleSheet {
    pub defaults: DocDefaults,
    styles: BTreeMap<StyleId, Arc<Style>>,
}

impl StyleSheet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, style: Style) -> Result<(), StyleError> {
        if self.styles.contains_key(&style.id) {
            return Err(StyleError::DuplicateId(style.id));
        }
        self.styles.insert(style.id.clone(), Arc::new(style));
        Ok(())
    }

    pub fn get(&self, id: &StyleId) -> Option<&Arc<Style>> {
        self.styles.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<Style>> {
        self.styles.values()
    }

    pub fn len(&self) -> usize {
        self.styles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }

    /// Finds a built-in style by canonical name, never by `w:styleId`.
    pub fn builtin(&self, builtin: BuiltinStyle) -> Option<&Arc<Style>> {
        self.styles
            .values()
            .find(|style| style.builtin() == Some(builtin))
    }

    /// The `w:default` style of a kind; the first one wins as in Word.
    pub fn default_of(&self, kind: StyleKind) -> Option<&Arc<Style>> {
        self.styles
            .values()
            .find(|style| style.kind == kind && style.is_default)
    }

    /// Follows `basedOn` from `id` with cycle and depth limits.
    pub fn chain(&self, id: &StyleId) -> StyleChain {
        let mut styles: Vec<Arc<Style>> = Vec::new();
        let mut seen = HashSet::new();
        let mut current = id.clone();
        let problem = loop {
            let Some(style) = self.styles.get(&current) else {
                break Some(StyleProblem::Missing(current));
            };
            if !seen.insert(current.clone()) {
                break Some(StyleProblem::Cycle(current));
            }
            if styles.len() == MAX_STYLE_DEPTH {
                break Some(StyleProblem::TooDeep);
            }
            if let Some(child) = styles.last().cloned()
                && child.kind != style.kind
            {
                break Some(StyleProblem::KindMismatch {
                    style: child.id.clone(),
                    base: style.id.clone(),
                });
            }
            styles.push(style.clone());
            match &style.based_on {
                Some(base) => current = base.clone(),
                None => break None,
            }
        };
        StyleChain { styles, problem }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn based(id: &str, name: Option<&str>, base: Option<&str>) -> Style {
        let mut style = Style::new(id, StyleKind::Paragraph, name);
        style.based_on = base.map(StyleId::new);
        style
    }

    #[test]
    fn builtin_styles_are_found_by_canonical_name_not_style_id() {
        // Chinese Word: Normal is `a`, heading 1 is `1`. WPS uses other IDs.
        let mut sheet = StyleSheet::new();
        sheet.insert(based("a", Some("Normal"), None)).unwrap();
        sheet
            .insert(based("1", Some("heading 1"), Some("a")))
            .unwrap();
        sheet
            .insert(based("Heading1", Some("Custom heading"), Some("a")))
            .unwrap();
        sheet
            .insert(based("12", Some("Heading 2"), Some("a")))
            .unwrap();
        assert_eq!(
            sheet.builtin(BuiltinStyle::Normal).unwrap().id.as_str(),
            "a"
        );
        assert_eq!(
            sheet.builtin(BuiltinStyle::Heading(1)).unwrap().id.as_str(),
            "1"
        );
        assert_eq!(
            sheet.builtin(BuiltinStyle::Heading(2)).unwrap().id.as_str(),
            "12"
        );
        assert!(sheet.builtin(BuiltinStyle::Title).is_none());
        assert_eq!(BuiltinStyle::from_canonical_name("heading 10"), None);
        assert_eq!(BuiltinStyle::from_canonical_name("标题 1"), None);
        for builtin in [
            BuiltinStyle::Heading(9),
            BuiltinStyle::Toc(3),
            BuiltinStyle::NoList,
        ] {
            assert_eq!(
                BuiltinStyle::from_canonical_name(&builtin.canonical_name()),
                Some(builtin)
            );
        }
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let mut sheet = StyleSheet::new();
        sheet.insert(based("a", None, None)).unwrap();
        assert_eq!(
            sheet.insert(based("a", None, None)),
            Err(StyleError::DuplicateId(StyleId::new("a")))
        );
    }

    #[test]
    fn chains_stop_at_cycles_missing_bases_and_depth_limit() {
        let mut sheet = StyleSheet::new();
        sheet.insert(based("x", None, Some("y"))).unwrap();
        sheet.insert(based("y", None, Some("x"))).unwrap();
        sheet.insert(based("orphan", None, Some("gone"))).unwrap();
        let cycle = sheet.chain(&StyleId::new("x"));
        assert_eq!(cycle.styles.len(), 2);
        assert_eq!(cycle.problem, Some(StyleProblem::Cycle(StyleId::new("x"))));
        let orphan = sheet.chain(&StyleId::new("orphan"));
        assert_eq!(orphan.styles.len(), 1);
        assert_eq!(
            orphan.problem,
            Some(StyleProblem::Missing(StyleId::new("gone")))
        );

        let mut deep = StyleSheet::new();
        for level in 0..100 {
            let base = (level > 0).then(|| format!("s{}", level - 1));
            deep.insert(based(&format!("s{level}"), None, base.as_deref()))
                .unwrap();
        }
        let chain = deep.chain(&StyleId::new("s99"));
        assert_eq!(chain.styles.len(), MAX_STYLE_DEPTH);
        assert_eq!(chain.problem, Some(StyleProblem::TooDeep));
    }

    #[test]
    fn character_style_cannot_inherit_from_paragraph_style() {
        let mut sheet = StyleSheet::new();
        sheet.insert(based("p", None, None)).unwrap();
        let mut character = Style::new("c", StyleKind::Character, None);
        character.based_on = Some(StyleId::new("p"));
        sheet.insert(character).unwrap();
        let chain = sheet.chain(&StyleId::new("c"));
        assert_eq!(chain.styles.len(), 1);
        assert!(matches!(
            chain.problem,
            Some(StyleProblem::KindMismatch { .. })
        ));
    }
}
