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
use crate::props::{FontSlots, Indent, Languages, ParaProps, RunProps, Spacing, merge_tab_stops};
use crate::theme::{ThemeColorSlot, ThemeScheme};

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
    KindMismatch {
        style: StyleId,
        base: StyleId,
    },
    /// Table and numbering styles are kept on the sheet and are not resolved
    /// into paragraph or character formatting.
    UnresolvedKind {
        style: StyleId,
        kind: StyleKind,
    },
    /// A theme font or theme colour has no scheme entry.
    UnresolvedThemeFont,
    UnresolvedThemeColor(ThemeColorSlot),
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
    Missing(StyleId),
}

#[derive(Clone, Debug, Default)]
pub struct StyleSheet {
    pub defaults: DocDefaults,
    theme: ThemeScheme,
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

    /// Replaces the definition of an existing style. Callers that edit a
    /// definition use this so a missing id is not inserted by accident.
    pub fn replace(&mut self, style: Style) -> Result<(), StyleError> {
        if !self.styles.contains_key(&style.id) {
            return Err(StyleError::Missing(style.id));
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

    pub fn theme(&self) -> &ThemeScheme {
        &self.theme
    }

    pub fn set_theme(&mut self, theme: ThemeScheme) {
        self.theme = theme;
    }

    /// Paragraph properties and the paragraph style's character formatting.
    ///
    /// Order follows ECMA-376 Part 1 §17.7.2: document defaults, then the
    /// `basedOn` chain from the root, then direct paragraph properties.
    /// A missing `w:pStyle` uses the default paragraph style. `w:link` pairs
    /// a paragraph style with a character style for application (§17.7.4.6);
    /// it is not a second property layer. Table and numbering styles are
    /// reported and not applied.
    pub fn resolve_paragraph(
        &self,
        style: Option<&StyleId>,
        direct: &ParaProps,
    ) -> ResolvedParagraph {
        let mut problems = Vec::new();
        let mut para = self.defaults.para.clone();
        let mut run = self.defaults.run.clone();
        if let Some(id) = self.paragraph_style(style, &mut problems) {
            self.stack_style(&id, &mut para, &mut run, &mut problems);
        }
        layer_para(&mut para, direct);
        layer_run(&mut run, &RunProps::default());
        concretize(self, &mut run, &mut problems);
        ResolvedParagraph {
            props: para,
            run,
            problems,
        }
    }

    /// Character formatting for one run: document defaults, the paragraph
    /// style (or the default paragraph style), the character style (or the
    /// default character style), then direct formatting. Toggle properties
    /// follow ECMA-376 Part 1 §17.7.3: an explicit off forces off, and an
    /// on flips the value inherited so far. The first on therefore turns
    /// the property on. Other properties use the nearest specified value.
    pub fn resolve_run(
        &self,
        paragraph_style: Option<&StyleId>,
        character_style: Option<&StyleId>,
        direct: &RunProps,
    ) -> ResolvedRun {
        let mut problems = Vec::new();
        let mut run = self.defaults.run.clone();
        let mut ignored = self.defaults.para.clone();
        if let Some(id) = self.paragraph_style(paragraph_style, &mut problems) {
            self.stack_style(&id, &mut ignored, &mut run, &mut problems);
        }
        if let Some(id) = self.character_style(character_style, &mut problems) {
            self.stack_style(&id, &mut ignored, &mut run, &mut problems);
        }
        layer_run(&mut run, direct);
        concretize(self, &mut run, &mut problems);
        ResolvedRun {
            props: run,
            problems,
        }
    }

    fn paragraph_style(
        &self,
        requested: Option<&StyleId>,
        problems: &mut Vec<StyleProblem>,
    ) -> Option<StyleId> {
        self.usable_style(requested, StyleKind::Paragraph, problems)
    }

    fn character_style(
        &self,
        requested: Option<&StyleId>,
        problems: &mut Vec<StyleProblem>,
    ) -> Option<StyleId> {
        self.usable_style(requested, StyleKind::Character, problems)
    }

    /// `None` selects the default style of `kind`. An explicit table or
    /// numbering style is not replaced with that default.
    fn usable_style(
        &self,
        requested: Option<&StyleId>,
        kind: StyleKind,
        problems: &mut Vec<StyleProblem>,
    ) -> Option<StyleId> {
        let Some(id) = requested else {
            return self.default_of(kind).map(|style| style.id.clone());
        };
        match self.styles.get(id) {
            None => {
                problems.push(StyleProblem::Missing(id.clone()));
                None
            }
            Some(style) if style.kind == kind => Some(id.clone()),
            Some(style) if matches!(style.kind, StyleKind::Table | StyleKind::Numbering) => {
                problems.push(StyleProblem::UnresolvedKind {
                    style: id.clone(),
                    kind: style.kind,
                });
                None
            }
            Some(_) => {
                problems.push(StyleProblem::KindMismatch {
                    style: id.clone(),
                    base: id.clone(),
                });
                None
            }
        }
    }

    fn stack_style(
        &self,
        id: &StyleId,
        para: &mut ParaProps,
        run: &mut RunProps,
        problems: &mut Vec<StyleProblem>,
    ) {
        let chain = self.chain(id);
        if let Some(problem) = chain.problem.clone() {
            problems.push(problem);
        }
        for style in chain.styles.iter().rev() {
            layer_para(para, &style.para);
            layer_run(run, &style.run);
        }
    }
}

/// Resolved paragraph properties plus the paragraph style's character
/// formatting, before a character style or direct run formatting.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedParagraph {
    pub props: ParaProps,
    pub run: RunProps,
    pub problems: Vec<StyleProblem>,
}

/// Resolved character formatting, with theme references substituted when the
/// scheme has an entry.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedRun {
    pub props: RunProps,
    pub problems: Vec<StyleProblem>,
}

fn layer_para(base: &mut ParaProps, layer: &ParaProps) {
    if layer.justification.is_some() {
        base.justification = layer.justification;
    }
    layer_indent(&mut base.indent, &layer.indent);
    layer_spacing(&mut base.spacing, &layer.spacing);
    set_flag(&mut base.keep_next, layer.keep_next);
    set_flag(&mut base.keep_lines, layer.keep_lines);
    set_flag(&mut base.widow_control, layer.widow_control);
    set_flag(&mut base.page_break_before, layer.page_break_before);
    set_flag(&mut base.snap_to_grid, layer.snap_to_grid);
    if layer.outline_level.is_some() {
        base.outline_level = layer.outline_level;
    }
    if layer.numbering.is_some() {
        base.numbering = layer.numbering;
    }
    if let Some(tabs) = &layer.tabs {
        base.tabs = Some(merge_tab_stops(base.tabs.as_deref().unwrap_or(&[]), tabs));
    }
    if layer.preserved.is_some() {
        base.preserved = layer.preserved;
    }
}

fn layer_indent(base: &mut Indent, layer: &Indent) {
    if layer.start.is_some() {
        base.start = layer.start;
    }
    if layer.end.is_some() {
        base.end = layer.end;
    }
    if layer.first_line.is_some() {
        base.first_line = layer.first_line;
    }
    if layer.hanging.is_some() {
        base.hanging = layer.hanging;
    }
}

fn layer_spacing(base: &mut Spacing, layer: &Spacing) {
    if layer.before.is_some() {
        base.before = layer.before;
    }
    if layer.after.is_some() {
        base.after = layer.after;
    }
    set_flag(&mut base.before_auto, layer.before_auto);
    set_flag(&mut base.after_auto, layer.after_auto);
    if layer.line.is_some() {
        base.line = layer.line;
    }
}

fn layer_run(base: &mut RunProps, layer: &RunProps) {
    toggle(&mut base.bold, layer.bold);
    toggle(&mut base.italic, layer.italic);
    toggle(&mut base.strike, layer.strike);
    toggle(&mut base.hidden, layer.hidden);
    if layer.underline.is_some() {
        base.underline = layer.underline;
    }
    if layer.size.is_some() {
        base.size = layer.size;
    }
    layer_fonts(&mut base.fonts, &layer.fonts);
    if layer.color.is_some() {
        base.color = layer.color;
    }
    layer_languages(&mut base.languages, &layer.languages);
    if layer.preserved.is_some() {
        base.preserved = layer.preserved;
    }
}

fn layer_fonts(base: &mut FontSlots, layer: &FontSlots) {
    if layer.ascii.is_some() {
        base.ascii.clone_from(&layer.ascii);
    }
    if layer.h_ansi.is_some() {
        base.h_ansi.clone_from(&layer.h_ansi);
    }
    if layer.east_asia.is_some() {
        base.east_asia.clone_from(&layer.east_asia);
    }
    if layer.complex_script.is_some() {
        base.complex_script.clone_from(&layer.complex_script);
    }
    if layer.hint.is_some() {
        base.hint = layer.hint;
    }
}

fn layer_languages(base: &mut Languages, layer: &Languages) {
    if layer.latin.is_some() {
        base.latin.clone_from(&layer.latin);
    }
    if layer.east_asia.is_some() {
        base.east_asia.clone_from(&layer.east_asia);
    }
    if layer.bidi.is_some() {
        base.bidi.clone_from(&layer.bidi);
    }
}

/// ECMA-376 §17.7.3. `None` inherits. `Some(false)` forces off. `Some(true)`
/// flips the inherited value, so the first on turns the property on.
fn toggle(current: &mut Option<bool>, specified: Option<bool>) {
    match specified {
        None => {}
        Some(false) => *current = Some(false),
        Some(true) => {
            let on = matches!(*current, Some(true));
            *current = Some(!on);
        }
    }
}

/// Apply one paragraph-property layer. Used by style resolution and by the
/// numbering level, which sits between the style and direct properties.
pub fn overlay_paragraph(base: &mut ParaProps, layer: &ParaProps) {
    layer_para(base, layer);
}

fn set_flag(current: &mut Option<bool>, specified: Option<bool>) {
    if specified.is_some() {
        *current = specified;
    }
}

fn concretize(sheet: &StyleSheet, run: &mut RunProps, problems: &mut Vec<StyleProblem>) {
    if !sheet.theme.concretize_fonts(&mut run.fonts).is_empty() {
        problems.push(StyleProblem::UnresolvedThemeFont);
    }
    if let Some(slot) = sheet.theme.concretize_color(&mut run.color) {
        problems.push(StyleProblem::UnresolvedThemeColor(slot));
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

    #[test]
    fn toggle_on_flips_and_explicit_off_forces_off() -> Result<(), StyleError> {
        // ECMA-376 §17.7.3: docDefaults bold, then a style that restates bold,
        // resolves to not bold. A further explicit off stays off, and another
        // on turns it back on.
        let mut sheet = StyleSheet::new();
        sheet.defaults.run.bold = Some(true);
        sheet.defaults.run.size = Some(crate::props::Measure::HalfPoints(21));
        let mut normal = Style::new("a", StyleKind::Paragraph, Some("Normal"));
        normal.is_default = true;
        sheet.insert(normal)?;
        let mut heading = Style::new("1", StyleKind::Paragraph, Some("heading 1"));
        heading.based_on = Some(StyleId::new("a"));
        heading.run.bold = Some(true);
        heading.run.size = Some(crate::props::Measure::HalfPoints(32));
        sheet.insert(heading)?;
        let resolved = sheet.resolve_paragraph(Some(&StyleId::new("1")), &ParaProps::default());
        assert!(resolved.problems.is_empty());
        assert_eq!(resolved.run.bold, Some(false));
        assert_eq!(
            resolved.run.size,
            Some(crate::props::Measure::HalfPoints(32))
        );
        let off = sheet.resolve_run(
            Some(&StyleId::new("a")),
            None,
            &RunProps {
                bold: Some(false),
                ..RunProps::default()
            },
        );
        assert_eq!(off.props.bold, Some(false));
        let direct = RunProps {
            bold: Some(true),
            ..RunProps::default()
        };
        let flipped = sheet.resolve_run(Some(&StyleId::new("1")), None, &direct);
        // Heading resolved to off; direct on flips it back on.
        assert_eq!(flipped.props.bold, Some(true));
        let body = sheet.resolve_run(None, None, &RunProps::default());
        assert_eq!(body.props.bold, Some(true));
        assert_eq!(body.props.size, Some(crate::props::Measure::HalfPoints(21)));
        Ok(())
    }

    #[test]
    fn linked_character_style_stacks_after_the_paragraph_style() -> Result<(), StyleError> {
        let mut sheet = StyleSheet::new();
        let mut paragraph = Style::new("p", StyleKind::Paragraph, Some("P"));
        paragraph.run.bold = Some(true);
        paragraph.link = Some(StyleId::new("c"));
        let mut character = Style::new("c", StyleKind::Character, Some("C"));
        character.run.italic = Some(true);
        character.link = Some(StyleId::new("p"));
        sheet.insert(paragraph)?;
        sheet.insert(character)?;
        let resolved = sheet.resolve_run(
            Some(&StyleId::new("p")),
            Some(&StyleId::new("c")),
            &RunProps::default(),
        );
        assert_eq!(resolved.props.bold, Some(true));
        assert_eq!(resolved.props.italic, Some(true));
        // The link is not applied a second time on top of the paragraph style.
        let paragraph_only =
            sheet.resolve_paragraph(Some(&StyleId::new("p")), &ParaProps::default());
        assert_eq!(paragraph_only.run.italic, None);
        Ok(())
    }

    #[test]
    fn table_styles_are_preserved_and_not_resolved() -> Result<(), StyleError> {
        let mut sheet = StyleSheet::new();
        let mut table = Style::new("Table", StyleKind::Table, Some("Normal Table"));
        table.run.bold = Some(true);
        sheet.insert(table)?;
        let resolved = sheet.resolve_paragraph(Some(&StyleId::new("Table")), &ParaProps::default());
        assert!(matches!(
            resolved.problems.first(),
            Some(StyleProblem::UnresolvedKind {
                kind: StyleKind::Table,
                ..
            })
        ));
        assert_eq!(resolved.run.bold, None);
        assert!(sheet.get(&StyleId::new("Table")).is_some());
        Ok(())
    }

    #[test]
    fn theme_font_and_colour_resolve_when_the_scheme_has_them() -> Result<(), StyleError> {
        use crate::props::{Color, FontRef, ThemeFont};
        use crate::theme::ThemeFonts;
        let mut sheet = StyleSheet::new();
        let mut theme = ThemeScheme {
            fonts: ThemeFonts {
                minor_east_asia: Some(Arc::from("宋体")),
                minor_latin: Some(Arc::from("Times New Roman")),
                ..ThemeFonts::default()
            },
            ..ThemeScheme::default()
        };
        theme
            .colors
            .insert(ThemeColorSlot::Accent1, (0x2E, 0x75, 0xB6));
        sheet.set_theme(theme);
        let mut style = Style::new("a", StyleKind::Paragraph, Some("Normal"));
        style.is_default = true;
        style.run.fonts.east_asia = Some(FontRef::Theme(ThemeFont::MinorEastAsia));
        style.run.fonts.ascii = Some(FontRef::Theme(ThemeFont::MinorAscii));
        style.run.color = Some(Color::Theme(ThemeColorSlot::Accent1));
        sheet.insert(style)?;
        let resolved = sheet.resolve_run(None, None, &RunProps::default());
        assert!(resolved.problems.is_empty());
        assert_eq!(
            resolved.props.fonts.east_asia,
            Some(FontRef::Named(Arc::from("宋体")))
        );
        assert_eq!(
            resolved.props.fonts.ascii,
            Some(FontRef::Named(Arc::from("Times New Roman")))
        );
        assert_eq!(resolved.props.color, Some(Color::Rgb(0x2E, 0x75, 0xB6)));

        let missing = ThemeScheme {
            fonts: ThemeFonts {
                major_latin: Some(Arc::from("Calibri Light")),
                ..ThemeFonts::default()
            },
            ..ThemeScheme::default()
        };
        sheet.set_theme(missing);
        let unresolved = sheet.resolve_run(None, None, &RunProps::default());
        assert!(
            unresolved
                .problems
                .iter()
                .any(|problem| matches!(problem, StyleProblem::UnresolvedThemeFont))
        );
        assert!(unresolved.problems.iter().any(|problem| matches!(
            problem,
            StyleProblem::UnresolvedThemeColor(ThemeColorSlot::Accent1)
        )));
        Ok(())
    }
}
