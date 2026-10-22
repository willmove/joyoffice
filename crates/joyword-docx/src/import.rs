//! Import entry point and the parts around the body: main document
//! discovery, relationships, styles and settings.

use std::borrow::Cow;
use std::sync::Arc;

use joyoffice_opc::{Budget, ExternalKind, Package, Part, PartName, Target};
use joyword_model::{
    Document, DocumentSettings, Measure, PageSetup, Severity as ModelSeverity, SourceRef, Style,
    StyleKind, ThemeColorSlot, ThemeScheme,
};

use crate::backing::{Fragment, PackageBacking, PreservedReason};
use crate::ns::{
    self, A, MAIN_CONTENT_TYPES, MC, R, REL_COMMENTS, REL_COMMENTS_EXTENDED,
    REL_COMMENTS_EXTENSIBLE, REL_COMMENTS_IDS, REL_FOOTER, REL_HEADER, REL_NUMBERING,
    REL_OFFICE_DOCUMENT, REL_OFFICE_DOCUMENT_STRICT, REL_SETTINGS, REL_STYLES, REL_THEME,
    REL_VBA_PROJECT, STORY_RELATIONSHIPS, UNDERSTOOD, W, W_STRICT, WORD_COMPAT_URI,
};
use crate::props::{self, Findings, on_off, val};
use crate::result::{
    Diagnostic, DiagnosticKind, Diagnostics, Editability, ImportError, ImportResult,
    ReadOnlyReason, Severity,
};
use crate::xml::{Declaration, Element, XmlReader};

/// Namespace context while walking a part.
#[derive(Clone, Debug, Default)]
pub(crate) struct Scope {
    pub namespaces: Vec<Declaration>,
    /// Namespace URIs made ignorable by `mc:Ignorable` on an ancestor.
    pub ignorable: Vec<Arc<str>>,
}

impl Scope {
    /// The scope for `element`'s content; borrowed unless the element
    /// declares namespaces or ignorable prefixes.
    pub fn enter(&self, element: &Element) -> Cow<'_, Scope> {
        let ignorable = element.prefixed_list(MC, "Ignorable", &self.namespaces);
        if ignorable.is_empty() && element.declarations.is_empty() {
            return Cow::Borrowed(self);
        }
        let mut inner = self.clone();
        inner.ignorable.extend(ignorable);
        inner
            .namespaces
            .extend(element.declarations.iter().cloned());
        Cow::Owned(inner)
    }
}

pub(crate) struct Importer<'a> {
    pub budget: &'a Budget,
    pub document: Document,
    pub backing: PackageBacking,
    pub diagnostics: Diagnostics,
    pub editability: Editability,
    /// Property elements preserved per part, reported as one summary each.
    properties_preserved: Vec<(PartName, usize)>,
    /// Layout and proofing hints skipped per part and element.
    pub hints: Vec<(PartName, &'static str, usize)>,
    /// Header and footer parts already mapped to a story.
    pub(crate) imported_stories: Vec<(PartName, joyword_model::StoryId)>,
    /// Image relationships whose targets were stored as resources.
    /// Keyed by the source part and the relationship id (`r:embed` / `r:id`).
    imported_images: Vec<(PartName, Arc<str>)>,
    /// Move range markers waiting until their paragraphs sit in a story.
    pub(crate) pending_moves: Vec<crate::revision::PendingMove>,
    /// Comment range markers recorded while story paragraphs are read.
    pub(crate) comment_marks: Vec<crate::comment::CommentMark>,
    /// Comment ids whose markers sit in an opaque table.
    pub(crate) unmapped_comment_ids: std::collections::BTreeSet<u32>,
    /// `false` while reading `comments.xml`, so a comment body is not a
    /// document anchor.
    pub(crate) record_comment_anchors: bool,
    /// Supplemental theme fonts until `w:themeFontLang` is read.
    theme_scripts: Option<crate::settings::ThemeScripts>,
    theme_languages: crate::settings::ThemeLanguages,
}

impl<'a> Importer<'a> {
    pub fn diagnose(
        &mut self,
        severity: Severity,
        part: Option<&PartName>,
        range: Option<std::ops::Range<usize>>,
        kind: DiagnosticKind,
    ) {
        if let DiagnosticKind::ReadOnly(reason) = &kind {
            self.editability.deny(reason.clone());
        }
        self.diagnostics.push(Diagnostic {
            severity,
            part: part.map(ToString::to_string),
            range,
            kind,
        });
    }

    pub fn deny(&mut self, part: &PartName, element: &Element, reason: ReadOnlyReason) {
        if self
            .editability
            .read_only_reasons()
            .any(|known| *known == reason)
        {
            return;
        }
        self.diagnose(
            Severity::Error,
            Some(part),
            Some(element.span.clone()),
            DiagnosticKind::ReadOnly(reason),
        );
    }

    /// Records that `id` on `source` was stored as an image resource.
    pub(crate) fn note_imported_image(&mut self, source: &PartName, id: &str) {
        if self
            .imported_images
            .iter()
            .any(|(part, known)| part == source && known.as_ref() == id)
        {
            return;
        }
        self.imported_images.push((source.clone(), Arc::from(id)));
    }

    pub fn source(part: &PartName, element: &Element) -> Arc<SourceRef> {
        Arc::new(SourceRef {
            part: part.as_str().into(),
            locator: format!("@{}..{}", element.span.start, element.span.end).into(),
        })
    }

    /// Keeps `element` in the backing and returns its handle. Property
    /// fragments are summarised per part; everything else is reported.
    pub fn preserve(
        &mut self,
        part: &PartName,
        element: &Element,
        scope: &Scope,
        reason: PreservedReason,
    ) -> joyword_model::PreservedId {
        let id = self.document.ids().preserved();
        self.backing.insert(
            id,
            Fragment {
                part: part.clone(),
                element: Arc::from(&*element.qname),
                namespace: element.namespace.clone(),
                range: element.span.clone(),
                namespaces: scope.namespaces.clone(),
                reason,
            },
        );
        if reason == PreservedReason::Properties {
            match self
                .properties_preserved
                .iter_mut()
                .find(|(p, _)| p == part)
            {
                Some((_, count)) => *count += 1,
                None => self.properties_preserved.push((part.clone(), 1)),
            }
        } else {
            let severity = match reason {
                PreservedReason::Revision => Severity::Error,
                _ => Severity::Warning,
            };
            self.diagnose(
                severity,
                Some(part),
                Some(element.span.clone()),
                DiagnosticKind::Preserved {
                    element: element.qname.to_string(),
                    reason,
                },
            );
        }
        id
    }

    /// Reports invalid values and preserves `element` when its mapping was
    /// incomplete.
    pub fn settle(
        &mut self,
        part: &PartName,
        element: &Element,
        scope: &Scope,
        findings: Findings,
    ) -> Option<joyword_model::PreservedId> {
        for invalid in findings.invalid {
            self.diagnose(
                Severity::Info,
                Some(part),
                Some(element.span.clone()),
                DiagnosticKind::InvalidValue {
                    element: invalid.element,
                    attribute: invalid.attribute,
                    value: invalid.value,
                },
            );
        }
        (!findings.complete)
            .then(|| self.preserve(part, element, scope, PreservedReason::Properties))
    }

    /// Walks a subtree for constructs that make the whole document
    /// read-only wherever they occur, including inside preserved content:
    /// tracked changes and unsatisfied `mc:MustUnderstand`.
    /// Recursion is bounded by the XML depth budget.
    pub fn scan(&mut self, part: &PartName, element: &Element, scope: &Scope) {
        if element.namespace.as_deref() == Some(W) && ns::blocks_for_review(&element.local) {
            self.deny(part, element, ReadOnlyReason::Revisions);
        }
        for namespace in element.prefixed_list(MC, "MustUnderstand", &scope.namespaces) {
            if !UNDERSTOOD.contains(&&*namespace) {
                self.deny(
                    part,
                    element,
                    ReadOnlyReason::MustUnderstand {
                        namespace: namespace.to_string(),
                    },
                );
            }
        }
        let inner = scope.enter(element);
        for child in element.elements() {
            self.scan(part, child, &inner);
        }
    }

    pub fn hint(&mut self, part: &PartName, element: &'static str) {
        match self
            .hints
            .iter_mut()
            .find(|(p, name, _)| p == part && *name == element)
        {
            Some((_, _, count)) => *count += 1,
            None => self.hints.push((part.clone(), element, 1)),
        }
    }

    fn finish_summaries(&mut self) {
        for (part, count) in std::mem::take(&mut self.properties_preserved) {
            self.diagnose(
                Severity::Info,
                Some(&part),
                None,
                DiagnosticKind::PropertiesPreserved { count },
            );
        }
        for (part, element, count) in std::mem::take(&mut self.hints) {
            self.diagnose(
                Severity::Info,
                Some(&part),
                None,
                DiagnosticKind::HintsSkipped {
                    element: format!("w:{element}"),
                    count,
                },
            );
        }
    }
}

pub(crate) fn xml_error(part: &PartName) -> impl Fn(joyoffice_opc::XmlError) -> ImportError + '_ {
    move |error| ImportError::Xml {
        part: part.to_string(),
        error,
    }
}

/// Finds the main document part through the package relationships
/// (ECMA-376 Part 1 §11.3.10; Part 2 package relationships).
fn main_part(package: &Package) -> Result<&Part, ImportError> {
    let package_relationships = package.relationships(None);
    if package_relationships
        .iter()
        .any(|relationship| &*relationship.kind == REL_OFFICE_DOCUMENT_STRICT)
    {
        return Err(ImportError::StrictUnsupported);
    }
    let mut main = package_relationships
        .iter()
        .filter(|relationship| &*relationship.kind == REL_OFFICE_DOCUMENT);
    let relationship = main.next().ok_or(ImportError::MissingMainDocument)?;
    if main.next().is_some() {
        return Err(ImportError::AmbiguousMainDocument);
    }
    let name = match &relationship.target {
        Target::Internal(name) => name,
        Target::External { raw, .. } | Target::Unresolved(raw) => {
            return Err(ImportError::InvalidMainDocumentTarget {
                target: raw.to_string(),
            });
        }
    };
    let part = package
        .part(name.as_str())
        .ok_or_else(|| ImportError::MissingPart {
            part: name.to_string(),
        })?;
    let content_type = part.content_type.as_deref();
    if !content_type.is_some_and(|content_type| MAIN_CONTENT_TYPES.contains(&content_type)) {
        return Err(ImportError::UnexpectedContentType {
            part: name.to_string(),
            content_type: content_type.map(ToOwned::to_owned),
        });
    }
    Ok(part)
}

/// Imports a DOCX package read-only. See the crate documentation for what
/// is mapped, preserved, restricted and refused.
pub fn import(bytes: &[u8], budget: &Budget) -> Result<ImportResult, ImportError> {
    let package = Package::read(bytes, budget)?;
    let main = main_part(&package)?.clone();
    let mut importer = Importer {
        budget,
        document: Document::new(PageSetup::a4_zh_cn()),
        backing: PackageBacking::new(Arc::from(bytes), package.clone(), main.name.clone()),
        diagnostics: Diagnostics::default(),
        editability: Editability::default(),
        properties_preserved: Vec::new(),
        hints: Vec::new(),
        imported_stories: Vec::new(),
        imported_images: Vec::new(),
        pending_moves: Vec::new(),
        comment_marks: Vec::new(),
        unmapped_comment_ids: std::collections::BTreeSet::new(),
        record_comment_anchors: true,
        theme_scripts: None,
        theme_languages: crate::settings::ThemeLanguages::default(),
    };
    for diagnostic in package.diagnostics() {
        importer.diagnose(
            Severity::Warning,
            None,
            None,
            DiagnosticKind::Package(diagnostic.clone()),
        );
    }
    importer.external_targets(&package);

    let mut styles = None;
    let mut settings = None;
    let mut numbering = None;
    for relationship in package.relationships(Some(&main.name)) {
        let Target::Internal(target) = &relationship.target else {
            continue;
        };
        let kind = &*relationship.kind;
        let Some(part) = package.part(target.as_str()) else {
            importer.diagnose(
                Severity::Warning,
                Some(&main.name),
                None,
                DiagnosticKind::MissingPart {
                    relationship: kind.to_owned(),
                    part: target.to_string(),
                },
            );
            continue;
        };
        if kind == REL_STYLES && styles.is_none() {
            styles = Some(part.clone());
        } else if kind == REL_THEME {
            importer.theme(part);
        } else if kind == REL_SETTINGS && settings.is_none() {
            settings = Some(part.clone());
        } else if kind == REL_NUMBERING && numbering.is_none() {
            numbering = Some(part.clone());
        } else if kind == REL_HEADER || kind == REL_FOOTER {
            // Resolved from `w:headerReference` / `w:footerReference`.
        } else if crate::images::is_image_relationship(kind) {
            // Resolved from `a:blip` / `v:imagedata` (ECMA-376 Part 1
            // §20.4.2.7) while the body is read. A relationship whose target
            // was stored as a resource is consumed there; an unconsumed
            // internal image part is reported after that walk.
        } else if kind == REL_COMMENTS
            || kind == REL_COMMENTS_EXTENDED
            || kind == REL_COMMENTS_IDS
            || kind == REL_COMMENTS_EXTENSIBLE
        {
            // Imported after the body so anchors bind to paragraphs already
            // allocated. People.xml stays unimported.
        } else if kind == REL_VBA_PROJECT {
            importer.diagnose(
                Severity::Warning,
                Some(&part.name),
                None,
                DiagnosticKind::MacroProject,
            );
        } else {
            let severity = if STORY_RELATIONSHIPS.contains(&kind) {
                Severity::Warning
            } else {
                Severity::Info
            };
            importer.diagnose(
                severity,
                Some(&main.name),
                None,
                DiagnosticKind::NotImported {
                    relationship: kind.to_owned(),
                    part: target.to_string(),
                },
            );
        }
    }

    if let Some(part) = &styles {
        importer.styles(part)?;
    }
    if let Some(part) = &settings {
        importer.settings(part)?;
    }
    importer.name_theme_fonts();
    if let Some(part) = &numbering {
        importer.numbering(part)?;
    }
    importer.main_document(&main)?;
    importer.import_comments(&package)?;
    importer.warn_unreferenced_stories(&package, &main.name);
    importer.warn_unreferenced_images(&package, &main.name);
    importer.finish_summaries();

    let validation = importer.document.validate();
    let mut invalid = false;
    for item in validation.iter() {
        let severity = match item.severity {
            ModelSeverity::Error => {
                invalid = true;
                Severity::Error
            }
            ModelSeverity::Warning => Severity::Warning,
        };
        importer.diagnose(
            severity,
            None,
            None,
            DiagnosticKind::Model(item.issue.clone()),
        );
    }
    if invalid || validation.truncated() {
        importer.diagnose(
            Severity::Error,
            None,
            None,
            DiagnosticKind::ReadOnly(ReadOnlyReason::ModelInvalid),
        );
    }

    Ok(ImportResult {
        document: importer.document,
        backing: importer.backing,
        diagnostics: importer.diagnostics,
        editability: importer.editability,
    })
}

impl Importer<'_> {
    /// Records every external relationship target in the package. None is
    /// resolved or accessed (QUALITY-005).
    fn external_targets(&mut self, package: &Package) {
        let sources: Vec<Option<PartName>> = std::iter::once(None)
            .chain(package.parts().map(|part| Some(part.name.clone())))
            .collect();
        for source in sources {
            for relationship in package.relationships(source.as_ref()) {
                let Target::External { raw, kind } = &relationship.target else {
                    continue;
                };
                let severity = match kind {
                    ExternalKind::Web | ExternalKind::Mail => Severity::Info,
                    _ => Severity::Warning,
                };
                self.diagnose(
                    severity,
                    source.as_ref(),
                    None,
                    DiagnosticKind::ExternalTarget {
                        source: source
                            .as_ref()
                            .map_or_else(|| "/".to_owned(), ToString::to_string),
                        id: relationship.id.to_string(),
                        relationship: relationship.kind.to_string(),
                        target: raw.to_string(),
                        kind: *kind,
                    },
                );
            }
        }
    }

    fn root(&mut self, part: &Part, local: &'static str) -> Result<(Element, Scope), ImportError> {
        let element = XmlReader::new(part.name.as_str(), &part.data, self.budget)
            .and_then(XmlReader::whole)
            .map_err(xml_error(&part.name))?;
        if element.namespace.as_deref() == Some(W_STRICT) {
            return Err(ImportError::StrictUnsupported);
        }
        if !element.is(W, local) {
            return Err(ImportError::UnexpectedElement {
                part: part.name.to_string(),
                expected: local,
            });
        }
        self.backing.interpret(part.name.clone());
        let scope = Scope::default();
        self.scan(&part.name, &element, &scope);
        let inner = scope.enter(&element);
        Ok((element, inner.into_owned()))
    }

    /// Theme font and colour scheme (ECMA-376 Part 1 §14.2.7, §20.1.4).
    /// The part stays in the package. A scheme this slice cannot read leaves
    /// the model theme empty and does not drop the part.
    fn theme(&mut self, part: &Part) {
        let parsed =
            XmlReader::new(part.name.as_str(), &part.data, self.budget).and_then(XmlReader::whole);
        let element = match parsed {
            Ok(element) => element,
            Err(_) => {
                self.diagnose(
                    Severity::Warning,
                    Some(&part.name),
                    None,
                    DiagnosticKind::InvalidValue {
                        element: "theme".into(),
                        attribute: "part".into(),
                        value: part.name.to_string(),
                    },
                );
                return;
            }
        };
        if !element.is(A, "theme") {
            self.diagnose(
                Severity::Warning,
                Some(&part.name),
                Some(element.span.clone()),
                DiagnosticKind::InvalidValue {
                    element: element.qname.to_string(),
                    attribute: "root".into(),
                    value: element.local.to_string(),
                },
            );
            return;
        }
        self.backing.interpret(part.name.clone());
        let Some(elements) = element.child(A, "themeElements") else {
            return;
        };
        let mut theme = ThemeScheme::default();
        if let Some(scheme) = elements.child(A, "fontScheme") {
            let mut scripts = crate::settings::ThemeScripts::default();
            if let Some(major) = scheme.child(A, "majorFont") {
                theme.fonts.major_latin = typeface(major, "latin");
                theme.fonts.major_east_asia = typeface(major, "ea");
                theme.fonts.major_complex = typeface(major, "cs");
                scripts.read(major, true);
            }
            if let Some(minor) = scheme.child(A, "minorFont") {
                theme.fonts.minor_latin = typeface(minor, "latin");
                theme.fonts.minor_east_asia = typeface(minor, "ea");
                theme.fonts.minor_complex = typeface(minor, "cs");
                scripts.read(minor, false);
            }
            self.theme_scripts = Some(scripts);
        }
        if let Some(colors) = elements.child(A, "clrScheme") {
            for child in colors.elements() {
                let Some(slot) = ThemeColorSlot::from_scheme_name(&child.local) else {
                    continue;
                };
                if let Some(rgb) = scheme_rgb(child) {
                    theme.colors.insert(slot, rgb);
                }
            }
        }
        self.document.styles_mut().set_theme(theme);
    }

    /// An empty theme typeface takes the supplemental `a:font` for the
    /// `w:themeFontLang` script (§17.15.1.88). Without one the slot names no
    /// face. Runs once, after the settings part.
    fn name_theme_fonts(&mut self) {
        let Some(scripts) = self.theme_scripts.take() else {
            return;
        };
        let mut theme = self.document.styles().theme().clone();
        scripts.apply(&self.theme_languages, &mut theme.fonts);
        self.document.styles_mut().set_theme(theme);
    }

    /// Style definitions (ECMA-376 Part 1 §17.7). Built-in styles are
    /// recognised later by `w:name`, never by `w:styleId`.
    fn styles(&mut self, part: &Part) -> Result<(), ImportError> {
        let name = &part.name;
        let (root, scope) = self.root(part, "styles")?;
        for child in root.elements() {
            if child.is(W, "docDefaults") {
                self.doc_defaults(name, child, &scope.enter(child));
            } else if child.is(W, "style") {
                self.style(name, child, &scope);
            } else {
                // latentStyles and anything else stays in the styles part.
                self.preserve(name, child, &scope, PreservedReason::Properties);
            }
        }
        Ok(())
    }

    fn doc_defaults(&mut self, part: &PartName, element: &Element, scope: &Scope) {
        let mut complete = true;
        for child in element.elements() {
            let inner = scope.enter(child);
            let props = child
                .elements()
                .next()
                .filter(|_| child.elements().count() == 1);
            match (child.namespace.as_deref() == Some(W), &*child.local, props) {
                (true, "rPrDefault", Some(rpr)) if rpr.is(W, "rPr") => {
                    let mut findings = Findings::default();
                    let (mut run, _) = props::run(rpr, &mut findings);
                    run.preserved_layout_neutral = findings.layout_neutral();
                    run.preserved = self.settle(part, rpr, &inner, findings);
                    self.document.styles_mut().defaults.run = run;
                }
                (true, "pPrDefault", Some(ppr)) if ppr.is(W, "pPr") => {
                    let mut findings = Findings::default();
                    let para = props::para(ppr, &mut findings);
                    if para.style.is_some() || para.mark.is_some() || para.section.is_some() {
                        findings.gap();
                    }
                    let mut props = para.props;
                    props.preserved = self.settle(part, ppr, &inner, findings);
                    self.document.styles_mut().defaults.para = props;
                }
                _ => complete = false,
            }
        }
        if !complete {
            self.preserve(part, element, scope, PreservedReason::Properties);
        }
    }

    fn style(&mut self, part: &PartName, element: &Element, scope: &Scope) {
        let inner = scope.enter(element);
        let id = element.attribute(Some(W), "styleId");
        let duplicate = id.is_some_and(|id| {
            self.document
                .styles()
                .get(&joyword_model::StyleId::new(id))
                .is_some()
        });
        let kind = match element.attribute(Some(W), "type") {
            // A style without w:type is a paragraph style.
            None | Some("paragraph") => Some(StyleKind::Paragraph),
            Some("character") => Some(StyleKind::Character),
            Some("table") => Some(StyleKind::Table),
            Some("numbering") => Some(StyleKind::Numbering),
            Some(_) => None,
        };
        let (Some(id), Some(kind), false) = (id, kind, duplicate) else {
            self.preserve(part, element, scope, PreservedReason::Properties);
            self.diagnose(
                Severity::Warning,
                Some(part),
                Some(element.span.clone()),
                DiagnosticKind::StyleSkipped {
                    id: id.map(ToOwned::to_owned),
                },
            );
            return;
        };
        let name = element.child(W, "name").and_then(val);
        let mut style = Style::new(id, kind, name);
        let mut complete = true;
        let flag = |attribute: &str, complete: &mut bool| {
            let value = element.attribute(Some(W), attribute);
            value
                .map_or(Some(false), |value| on_off(Some(value)))
                .unwrap_or_else(|| {
                    *complete = false;
                    false
                })
        };
        style.is_default = flag("default", &mut complete);
        style.custom = flag("customStyle", &mut complete);
        for child in element.elements() {
            let child_scope = inner.enter(child);
            if child.namespace.as_deref() != Some(W) {
                complete = false;
                continue;
            }
            let reference = || val(child).map(joyword_model::StyleId::new);
            match &*child.local {
                "name" => {}
                "basedOn" => style.based_on = reference(),
                "next" => style.next = reference(),
                "link" => style.link = reference(),
                "pPr" => {
                    let mut findings = Findings::default();
                    let para = props::para(child, &mut findings);
                    if para.style.is_some() || para.mark.is_some() || para.section.is_some() {
                        findings.gap();
                    }
                    style.para = para.props;
                    style.para.preserved = self.settle(part, child, &child_scope, findings);
                }
                "rPr" => {
                    let mut findings = Findings::default();
                    let (run, run_style) = props::run(child, &mut findings);
                    if run_style.is_some() {
                        findings.gap();
                    }
                    style.run = run;
                    style.run.preserved_layout_neutral = findings.layout_neutral();
                    style.run.preserved = self.settle(part, child, &child_scope, findings);
                }
                "tblPr" | "trPr" | "tcPr" | "tblStylePr" => {
                    if !crate::table::read_style_table(&mut style, child) {
                        complete = false;
                    }
                }
                // qFormat, uiPriority and the like.
                _ => complete = false,
            }
        }
        style.source = Some(Self::source(part, element));
        if style.kind == joyword_model::StyleKind::Table && style.table.bold.is_none() {
            style.table.bold = style.run.bold;
        }
        if !complete {
            style.preserved =
                Some(self.preserve(part, element, scope, PreservedReason::Properties));
        }
        self.document
            .styles_mut()
            .insert(style)
            .expect("duplicate ids were skipped");
    }

    /// Document settings (ECMA-376 Part 1 §17.15). The part bytes stay in the
    /// package. A child is modeled when it is one of the fields below;
    /// `w:attachedTemplate` is recognized and its relationship id is stored,
    /// never resolved (QUALITY-005). The settings element is preserved only
    /// when a child has no model field: an unknown `w:` child, a
    /// foreign-namespace child, a `w:compatSetting` that is not
    /// compatibilityMode, or a compatibilityMode value that does not parse.
    fn settings(&mut self, part: &Part) -> Result<(), ImportError> {
        let name = &part.name;
        let (root, scope) = self.root(part, "settings")?;
        let mut settings = DocumentSettings::default();
        let mut unmodeled = false;
        let mut layout_neutral = true;
        let mut keep = |child: &Element| {
            unmodeled = true;
            layout_neutral &= crate::settings::layout_neutral(child);
        };
        for child in root.elements() {
            if child.namespace.as_deref() != Some(W) {
                keep(child);
                continue;
            }
            match &*child.local {
                "compat" => {
                    if !self.read_compat(name, child, &mut settings) {
                        keep(child);
                    }
                }
                "themeFontLang" => {
                    self.theme_languages = crate::settings::ThemeLanguages::read(child);
                    keep(child);
                }
                "trackRevisions" => {
                    // §17.15.1.89. Tracking is a setting. It does not make the
                    // document read-only; document protection still does.
                    settings.track_revisions = on_off(val(child));
                }
                "evenAndOddHeaders" => settings.even_and_odd_headers = on_off(val(child)),
                "defaultTabStop" => {
                    let value = val(child).unwrap_or_default();
                    match value.parse::<i64>() {
                        Ok(twips) => settings.default_tab_stop = Some(Measure::Twips(twips)),
                        Err(_) => self.diagnose(
                            Severity::Warning,
                            Some(name),
                            Some(child.span.clone()),
                            DiagnosticKind::InvalidValue {
                                element: child.qname.to_string(),
                                attribute: "w:val".into(),
                                value: value.to_owned(),
                            },
                        ),
                    }
                }
                "documentProtection" => {
                    // ECMA-376 treats an omitted w:enforcement as off, but
                    // Word enforces it ([MS-OI29500] note on §17.15.1.29); the stricter
                    // reading is used. Only an explicit "off" lifts it.
                    let enforced = child
                        .attribute(Some(W), "enforcement")
                        .is_none_or(|value| on_off(Some(value)) != Some(false));
                    let edit = child.attribute(Some(W), "edit");
                    let formatting = child
                        .attribute(Some(W), "formatting")
                        .is_some_and(|value| on_off(Some(value)) != Some(false));
                    if enforced && (edit.is_some_and(|edit| edit != "none") || formatting) {
                        self.deny(
                            name,
                            child,
                            ReadOnlyReason::DocumentProtection {
                                edit: edit.map(ToOwned::to_owned),
                            },
                        );
                    }
                }
                "writeProtection" => self.deny(name, child, ReadOnlyReason::WriteProtection),
                "attachedTemplate" => {
                    // QUALITY-005: keep the relationship id. Do not open the
                    // target, whether it is a file, UNC path or URL.
                    let id = child.attribute(Some(R), "id").unwrap_or("");
                    settings.attached_template = Some(Arc::from(id));
                }
                _ => keep(child),
            }
        }
        if unmodeled {
            settings.preserved =
                Some(self.preserve(name, &root, &scope, PreservedReason::Properties));
            settings.preserved_layout_neutral = layout_neutral;
        }
        *self.document.settings_mut() = settings;
        Ok(())
    }

    /// `true` when every `w:compat` child is a parsed compatibilityMode.
    fn read_compat(
        &mut self,
        name: &PartName,
        compat: &Element,
        settings: &mut DocumentSettings,
    ) -> bool {
        let mut modeled = true;
        for setting in compat.elements() {
            let recognized = setting.is(W, "compatSetting")
                && setting.attribute(Some(W), "name") == Some("compatibilityMode")
                && setting.attribute(Some(W), "uri") == Some(WORD_COMPAT_URI);
            if !recognized {
                modeled = false;
                continue;
            }
            let value = val(setting).unwrap_or_default();
            match value.parse::<u32>() {
                Ok(mode) => settings.compatibility_mode = Some(mode),
                Err(_) => {
                    modeled = false;
                    self.diagnose(
                        Severity::Warning,
                        Some(name),
                        Some(setting.span.clone()),
                        DiagnosticKind::InvalidValue {
                            element: setting.qname.to_string(),
                            attribute: "w:val".into(),
                            value: value.to_owned(),
                        },
                    );
                }
            }
        }
        modeled
    }

    /// Numbering part (ECMA-376 Part 1 §17.9). The part is interpreted into
    /// the numbering store and its bytes are kept. Export does not rewrite it.
    fn numbering(&mut self, part: &Part) -> Result<(), ImportError> {
        let (root, _) = self.root(part, "numbering")?;
        let store = crate::numbering::read(&root);
        if store.unmodeled
            || store.abstracts().iter().any(|item| {
                item.unmodeled || item.levels.iter().flatten().any(|level| level.unmodeled)
            })
        {
            self.hint(&part.name, "numbering");
        }
        *self.document.numbering_mut() = store;
        Ok(())
    }

    fn warn_unreferenced_stories(&mut self, package: &Package, main: &PartName) {
        for relationship in package.relationships(Some(main)) {
            let kind = relationship.kind.as_ref();
            if kind != REL_HEADER && kind != REL_FOOTER {
                continue;
            }
            let Target::Internal(target) = &relationship.target else {
                continue;
            };
            if self.imported_stories.iter().any(|(name, _)| name == target) {
                continue;
            }
            self.diagnose(
                Severity::Warning,
                Some(main),
                None,
                DiagnosticKind::NotImported {
                    relationship: kind.to_owned(),
                    part: target.to_string(),
                },
            );
        }
    }

    /// Image relationships the body did not store. A missing part was already
    /// reported as [`DiagnosticKind::MissingPart`] and is not repeated.
    /// External targets stay on the external-target diagnostic only.
    fn warn_unreferenced_images(&mut self, package: &Package, main: &PartName) {
        for relationship in package.relationships(Some(main)) {
            if !crate::images::is_image_relationship(relationship.kind.as_ref()) {
                continue;
            }
            let Target::Internal(target) = &relationship.target else {
                continue;
            };
            if package.part(target.as_str()).is_none() {
                continue;
            }
            if self
                .imported_images
                .iter()
                .any(|(source, id)| source == main && id.as_ref() == relationship.id.as_ref())
            {
                continue;
            }
            self.diagnose(
                Severity::Info,
                Some(main),
                None,
                DiagnosticKind::NotImported {
                    relationship: relationship.kind.to_string(),
                    part: target.to_string(),
                },
            );
        }
    }

    fn main_document(&mut self, part: &Part) -> Result<(), ImportError> {
        let name = part.name.clone();
        let mut reader =
            XmlReader::new(name.as_str(), &part.data, self.budget).map_err(xml_error(&name))?;
        crate::body::read(self, &name, &mut reader)
    }
}

fn typeface(parent: &Element, local: &str) -> Option<std::sync::Arc<str>> {
    let face = parent.child(A, local)?.attribute(None, "typeface")?;
    if face.is_empty() {
        None
    } else {
        Some(std::sync::Arc::from(face))
    }
}

fn scheme_rgb(slot: &Element) -> Option<(u8, u8, u8)> {
    let value = slot
        .child(A, "srgbClr")
        .and_then(|color| color.attribute(None, "val"))
        .or_else(|| {
            slot.child(A, "sysClr")
                .and_then(|color| color.attribute(None, "lastClr"))
        })?;
    if value.len() != 6 {
        return None;
    }
    let channel = |range: std::ops::Range<usize>| u8::from_str_radix(value.get(range)?, 16).ok();
    Some((channel(0..2)?, channel(2..4)?, channel(4..6)?))
}
