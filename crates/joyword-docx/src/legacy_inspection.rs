//! Schema 1 read-only body projection for the pre-R8 CLI.
//! The source package is never rewritten. See JOYWORD-R8-HANDOFF.md for projection limits.
use crate::xml::{self, El};
use joyoffice_opc::{Budget, Package, Part, Target};

const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";
const MAIN: &[&str] = &[
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml",
    "application/vnd.ms-word.document.macroEnabled.main+xml",
    "application/vnd.ms-word.template.macroEnabledTemplate.main+xml",
];

pub struct Projection {
    pub text: String,
    pub truncated: bool,
    pub paragraphs: usize,
    pub omitted_objects: usize,
    pub preserved_fragments: usize,
    pub diagnostics: usize,
    pub read_only: bool,
    pub unsigned_docx: bool,
    pub unsupported_body: bool,
    pub style_approximation_required: bool,
}

struct Body {
    report: Projection,
    limit: usize,
    property_fragments: usize,
    layout_hint: bool,
}

impl Body {
    fn push(&mut self, value: &str) {
        if !self.report.truncated {
            let remaining = self.limit - self.report.text.len();
            let end = value.floor_char_boundary(value.len().min(remaining));
            self.report.text.push_str(&value[..end]);
            self.report.truncated = end != value.len();
        }
    }

    fn omit(&mut self, preserved: bool, layout: bool) {
        self.report.omitted_objects += 1;
        if preserved {
            self.preserve();
        }
        self.report.style_approximation_required |= layout;
    }

    fn preserve(&mut self) {
        self.report.preserved_fragments += 1;
        self.report.diagnostics += 1;
    }

    fn properties(&mut self, el: &El) {
        let known = match el.name.as_str() {
            "w:pPr" => {
                "pStyle keepNext keepLines pageBreakBefore widowControl snapToGrid numPr tabs spacing ind jc outlineLvl rPr sectPr pPrChange"
            }
            "w:rPr" => "rStyle rFonts b i strike vanish color sz u lang rPrChange ins del",
            _ => return,
        };
        let mut unmapped = el
            .els()
            .filter(|child| !word_member(known, &child.name))
            .peekable();
        if unmapped.peek().is_some() {
            self.property_fragments += 1;
            self.report.style_approximation_required |= unmapped.any(|child| {
                el.name != "w:rPr" || !word_member("szCs bCs iCs kern noProof", &child.name)
            });
        }
        if let Some(fonts) = el.child("w:rFonts")
            && let (Some(a), Some(b)) = (fonts.attr("w:ascii"), fonts.attr("w:hAnsi"))
        {
            self.report.style_approximation_required |= a != b;
        }
        if let Some(mark) = el.child("w:rPr") {
            self.properties(mark);
        }
        if let Some(section) = el.child("w:sectPr") {
            self.section(section);
        }
    }

    fn section(&mut self, el: &El) {
        for (name, required) in [
            ("w:pgSz", &["w:w", "w:h"][..]),
            (
                "w:pgMar",
                &[
                    "w:top", "w:right", "w:bottom", "w:left", "w:header", "w:footer", "w:gutter",
                ][..],
            ),
        ] {
            if el.child(name).is_none_or(|part| {
                required.iter().any(|key| {
                    part.attr(key)
                        .is_none_or(|value| value.parse::<i64>().is_err())
                })
            }) {
                self.report.diagnostics += 1;
            }
        }
    }

    fn blocks(&mut self, el: &El) {
        for child in el.els() {
            match child.name.as_str() {
                "w:p" => {
                    if self.report.paragraphs > 0 {
                        self.push("\n");
                    }
                    self.report.paragraphs += 1;
                    self.inlines(child);
                }
                "w:tbl" => {
                    if child
                        .els()
                        .any(|e| !word_member("tblPr tblGrid tr", &e.name))
                    {
                        self.omit(true, false);
                    } else {
                        for row in child.children("w:tr") {
                            if row.els().any(|e| !word_member("trPr tblPrEx tc", &e.name)) {
                                self.report.unsupported_body = true;
                            }
                            for cell in row.children("w:tc") {
                                self.blocks(cell);
                            }
                        }
                    }
                }
                "w:sectPr" => self.section(child),
                "w:tcPr" => {}
                _ => self.omit(true, false),
            }
        }
    }

    fn inlines(&mut self, el: &El) {
        for child in el.els() {
            match child.name.as_str() {
                "w:r" => self.inlines(child),
                "w:hyperlink" => {
                    if child
                        .attrs
                        .iter()
                        .any(|(key, _)| !matches!(key.as_str(), "r:id" | "w:anchor"))
                        || child
                            .children("w:r")
                            .flat_map(El::els)
                            .any(|item| !word_member("rPr t tab", &item.name))
                    {
                        self.preserve();
                    }
                    self.inlines(child);
                }
                "w:ins" | "w:moveTo" => {
                    if !revision_metadata(child) {
                        self.preserve();
                    }
                    self.inlines(child);
                }
                "w:del" | "w:moveFrom" => {
                    if !revision_metadata(child) {
                        self.omit(true, true);
                    }
                }
                "w:delText" | "w:delInstrText" => {}
                "w:pPr" | "w:rPr" => self.properties(child),
                "w:bookmarkStart"
                | "w:bookmarkEnd"
                | "w:commentRangeStart"
                | "w:commentRangeEnd"
                | "w:commentReference"
                | "w:moveFromRangeStart"
                | "w:moveFromRangeEnd"
                | "w:moveToRangeStart"
                | "w:moveToRangeEnd" => {}
                "w:t" if child.els().next().is_none() => {
                    let text = child.text();
                    let xml_space = [' ', '\t', '\r', '\n'];
                    if child.attr("xml:space") != Some("preserve")
                        && (text.starts_with(xml_space) || text.ends_with(xml_space))
                    {
                        self.report.diagnostics += 1;
                    }
                    if text.contains('\u{fdd0}') {
                        self.omit(true, true);
                    } else {
                        self.push(&text);
                    }
                }
                "w:tab" if child.attrs.is_empty() => self.push("\t"),
                "w:cr" => self.push("\n"),
                "w:br" if child.attr("w:clear").is_none_or(|value| value == "none") => {
                    match child.attr("w:type") {
                        Some("page" | "column") => self.push("\x0c"),
                        None | Some("textWrapping") => self.push("\n"),
                        _ => self.omit(true, true),
                    }
                }
                "w:softHyphen" => self.push("\u{ad}"),
                "w:noBreakHyphen" => self.push("\u{2011}"),
                "w:lastRenderedPageBreak" => self.layout_hint = true,
                "w:drawing" | "w:pict" => {
                    let represented =
                        child.find("a:blip").is_some() || child.find("v:imagedata").is_some();
                    self.omit(!represented, !represented);
                }
                "w:fldSimple" => {
                    let supported = child.attr("w:instr").is_some_and(page_field);
                    self.omit(!supported, !supported);
                }
                "w:sdt" => {
                    self.preserve();
                    if let Some(content) = child.child("w:sdtContent") {
                        self.inlines(content);
                    }
                }
                _ => self.omit(true, true),
            }
        }
    }

    fn story_properties(&mut self, el: &El) {
        match el.name.as_str() {
            "w:pPr" | "w:rPr" => self.properties(el),
            "w:style" => {
                if el.els().any(|child| {
                    !word_member(
                        "name basedOn next link pPr rPr tblPr trPr tcPr tblStylePr",
                        &child.name,
                    )
                }) {
                    self.property_fragments += 1;
                }
                el.els().for_each(|child| self.story_properties(child));
            }
            _ => el.els().for_each(|child| self.story_properties(child)),
        }
    }

    fn finish_part(&mut self) {
        self.report.preserved_fragments += self.property_fragments;
        self.report.diagnostics += usize::from(self.property_fragments > 0);
        self.report.diagnostics += usize::from(self.layout_hint);
        self.property_fragments = 0;
        self.layout_hint = false;
    }
}

fn word_member(names: &str, name: &str) -> bool {
    name.strip_prefix("w:")
        .is_some_and(|local| names.split_ascii_whitespace().any(|n| n == local))
}

fn revision_metadata(el: &El) -> bool {
    el.attr("w:id").is_some_and(|id| id.parse::<i64>().is_ok()) && el.attr("w:author").is_some()
}

fn page_field(instruction: &str) -> bool {
    let mut words = instruction.split_ascii_whitespace();
    words.next().is_some_and(|word| {
        ["PAGE", "NUMPAGES", "SECTIONPAGES"]
            .iter()
            .any(|kind| kind.eq_ignore_ascii_case(word))
    }) && match words.next() {
        None => true,
        Some("\\*") => {
            words
                .next()
                .is_some_and(|word| word.eq_ignore_ascii_case("MERGEFORMAT"))
                && words.next().is_none()
        }
        _ => false,
    }
}

fn parse(part: &Part, budget: &Budget) -> Result<El, &'static str> {
    xml::parse_bounded(
        &part.data,
        budget.max_xml_depth,
        budget.max_xml_nodes as usize,
    )
    .map_err(|_| "invalid or over-budget Word XML")
}

pub fn project(bytes: &[u8], limit: usize) -> Result<Projection, &'static str> {
    let budget = Budget {
        max_entries: 1024,
        max_part_bytes: 16 << 20,
        max_total_bytes: 64 << 20,
        max_xml_depth: 128,
        max_xml_nodes: 1_000_000,
        max_relationships: 4096,
        ..Budget::default()
    };
    let package = Package::read(bytes, &budget).map_err(|_| "invalid Word package")?;
    let mut main = package
        .relationships(None)
        .iter()
        .filter(|rel| rel.kind.strip_prefix(REL) == Some("officeDocument"));
    let Some(Target::Internal(name)) = main.next().map(|rel| &rel.target) else {
        return Err("missing internal main document relationship");
    };
    if main.next().is_some() {
        return Err("ambiguous main document relationship");
    }
    let part = package
        .part(name.as_str())
        .ok_or("missing main document part")?;
    if !part
        .content_type
        .as_deref()
        .is_some_and(|ty| MAIN.contains(&ty))
    {
        return Err("unsupported main document type");
    }
    let root = parse(part, &budget)?;
    let body = root
        .child("w:body")
        .filter(|_| root.name == "w:document")
        .ok_or("missing document body")?;
    let external = std::iter::once(None)
        .chain(package.parts().map(|part| Some(&part.name)))
        .flat_map(|source| package.relationships(source))
        .filter(|rel| matches!(rel.target, Target::External { .. }))
        .count();
    let mut projection = Body {
        report: Projection {
            text: String::new(),
            paragraphs: 0,
            truncated: false,
            omitted_objects: 0,
            preserved_fragments: 0,
            diagnostics: package.diagnostics().len() + external,
            read_only: root.attr("mc:MustUnderstand").is_some(),
            unsigned_docx: part.content_type.as_deref() == Some(MAIN[0])
                && !package
                    .relationships(None)
                    .iter()
                    .any(|rel| rel.kind.contains("/digital-signature/"))
                && !package.parts().any(|part| {
                    part.name
                        .as_str()
                        .to_ascii_lowercase()
                        .starts_with("/_xmlsignatures/")
                        || part.content_type.as_deref().is_some_and(|kind| {
                            kind.starts_with(
                                "application/vnd.openxmlformats-package.digital-signature",
                            )
                        })
                }),
            unsupported_body: false,
            style_approximation_required: false,
        },
        limit,
        property_fragments: 0,
        layout_hint: false,
    };
    if projection.report.read_only {
        projection.report.diagnostics += 1;
    }
    projection.blocks(body);
    projection.report.unsupported_body |= projection.report.preserved_fragments != 0;
    projection.finish_part();
    projection.report.paragraphs = projection.report.paragraphs.max(1);
    for rel in package.relationships(Some(name)) {
        let Target::Internal(target) = &rel.target else {
            continue;
        };
        let Some(part) = package.part(target.as_str()) else {
            continue;
        };
        match rel.kind.strip_prefix(REL) {
            Some("settings") => {
                let settings = parse(part, &budget)?;
                if settings.child("w:documentProtection").is_some_and(|p| {
                    !matches!(p.attr("w:enforcement"), Some("false" | "0" | "off"))
                }) {
                    projection.report.read_only = true;
                    projection.report.diagnostics += 1;
                }
                if settings.els().any(|child| !word_member("compat trackRevisions evenAndOddHeaders defaultTabStop documentProtection attachedTemplate", &child.name)) {
                    projection.report.preserved_fragments += 1;
                    projection.report.diagnostics += 1;
                }
            }
            Some("styles") => {
                let styles = parse(part, &budget)?;
                projection.story_properties(&styles);
                projection.finish_part();
            }
            Some("numbering") => {
                let numbering = parse(part, &budget)?;
                if numbering
                    .children("w:abstractNum")
                    .flat_map(El::els)
                    .any(|item| !word_member("lvl numStyleLink styleLink", &item.name))
                {
                    projection.report.diagnostics += 1;
                }
            }
            Some("header" | "footer" | "comments" | "theme") => {
                projection.story_properties(&parse(part, &budget)?);
                projection.finish_part();
            }
            Some("image" | "commentsExtended" | "commentsIds" | "commentsExtensible") => {}
            _ => projection.report.diagnostics += 1,
        }
    }
    Ok(projection.report)
}
