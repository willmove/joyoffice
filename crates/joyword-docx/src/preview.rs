//! Bounded, read-only MAIN BODY text projection. This is neither an editable
//! import nor a Word page layout. Tables are expanded in row/cell order;
//! pictures and unknown objects have visible placeholders. Deleted/move-from
//! revisions, resolved hidden runs, field instructions and other stories are
//! excluded. Stored field results are never evaluated and links never fetched.
//!
//! Sources: ECMA-376 Part 1 §17.2, §17.3, §17.4, §17.5 and §17.13.5;
//! https://learn.microsoft.com/en-us/office/open-xml/word/how-to-change-text-in-a-table-in-a-word-processing-document
use std::fmt;
use std::sync::Arc;

use joyoffice_opc::{Budget, XmlError};
use joyword_model::{BlockSeq, Document, OBJECT_MARKER, Paragraph, StyleId};

use crate::props::{self, Findings};
use crate::xml::{Element, Step, XmlReader};
use crate::{PackageBacking, W};

#[derive(Clone, Copy, Debug)]
pub struct TextPreviewLimits {
    pub max_paragraphs: usize,
    pub max_paragraph_bytes: usize,
    pub max_text_bytes: usize,
    pub max_runs: usize,
}

impl Default for TextPreviewLimits {
    fn default() -> Self {
        Self {
            max_paragraphs: 10_000,
            max_paragraph_bytes: 65_536,
            max_text_bytes: 16 << 20,
            max_runs: 100_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextPreviewError {
    Xml(XmlError),
    Budget(&'static str),
    InvalidDocument,
}

impl fmt::Display for TextPreviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "text preview: {self:?}")
    }
}
impl std::error::Error for TextPreviewError {}

impl From<XmlError> for TextPreviewError {
    fn from(error: XmlError) -> Self {
        Self::Xml(error)
    }
}

/// An independent display document. Never pass this document to an export
/// planner for `backing`: saves must use the original imported document.
/// Failure returns no partial projection. XML is read one body block at a
/// time with the SAME parser and budget as import, before text is accumulated.
pub fn text_preview(
    backing: &PackageBacking,
    source: &Document,
    budget: &Budget,
    limits: TextPreviewLimits,
) -> Result<Document, TextPreviewError> {
    let part = backing
        .package()
        .part(backing.main_part().as_str())
        .ok_or(TextPreviewError::InvalidDocument)?;
    let mut reader = XmlReader::new(part.name.as_str(), &part.data, budget)?;
    let root = reader.root()?;
    if !root.element.is(W, "document") || root.empty {
        return Err(TextPreviewError::InvalidDocument);
    }
    let mut builder = Builder::new(source, limits);
    let mut body_seen = false;
    loop {
        match reader.step()? {
            Step::Start(start) if start.element.is(W, "body") && !body_seen => {
                body_seen = true;
                if !start.empty {
                    loop {
                        match reader.step()? {
                            Step::Start(start) => builder.block(&reader.finish(start)?)?,
                            Step::End => break,
                            Step::Text(_) => {}
                            Step::Eof => return Err(TextPreviewError::InvalidDocument),
                        }
                    }
                }
            }
            Step::Start(start) => {
                let _ = reader.finish(start)?;
            }
            Step::End => break,
            Step::Text(_) => {}
            Step::Eof => return Err(TextPreviewError::InvalidDocument),
        }
    }
    if !body_seen || !matches!(reader.step()?, Step::Eof) {
        return Err(TextPreviewError::InvalidDocument);
    }
    if builder.blocks.is_empty() {
        builder.line()?;
    }
    let story = builder.document.body_id();
    builder
        .document
        .story_mut(story)
        .ok_or(TextPreviewError::InvalidDocument)?
        .blocks = BlockSeq::from_vec(builder.blocks);
    Ok(builder.document)
}

struct Builder<'a> {
    source: &'a Document,
    document: Document,
    limits: TextPreviewLimits,
    blocks: Vec<joyword_model::Block>,
    text: String,
    bytes: usize,
    runs: usize,
}

impl<'a> Builder<'a> {
    fn new(source: &'a Document, limits: TextPreviewLimits) -> Self {
        let cap = TextPreviewLimits::default();
        Self {
            source,
            document: Document::new_blank(),
            limits: TextPreviewLimits {
                max_paragraphs: limits.max_paragraphs.min(cap.max_paragraphs),
                max_paragraph_bytes: limits.max_paragraph_bytes.min(cap.max_paragraph_bytes),
                max_text_bytes: limits.max_text_bytes.min(cap.max_text_bytes),
                max_runs: limits.max_runs.min(cap.max_runs),
            },
            blocks: Vec::new(),
            text: String::new(),
            bytes: 0,
            runs: 0,
        }
    }

    fn append(&mut self, value: &str) -> Result<(), TextPreviewError> {
        for ch in value.chars() {
            if ch == '\n' || ch == '\r' {
                self.line()?;
                continue;
            }
            // The model reserves its inline marker. It must not create an
            // inline object in a plain-text display document.
            let ch = if ch == OBJECT_MARKER { '\u{fffd}' } else { ch };
            if self.text.len().saturating_add(ch.len_utf8()) > self.limits.max_paragraph_bytes {
                return Err(TextPreviewError::Budget("paragraph bytes"));
            }
            if self.bytes.saturating_add(ch.len_utf8()) > self.limits.max_text_bytes {
                return Err(TextPreviewError::Budget("text bytes"));
            }
            self.text.push(ch);
            self.bytes += ch.len_utf8();
        }
        Ok(())
    }

    fn line(&mut self) -> Result<(), TextPreviewError> {
        if self.blocks.len() >= self.limits.max_paragraphs {
            return Err(TextPreviewError::Budget("paragraphs"));
        }
        if !self.blocks.is_empty() {
            if self.bytes >= self.limits.max_text_bytes {
                return Err(TextPreviewError::Budget("text bytes"));
            }
            self.bytes += 1;
        }
        let mut paragraph = Paragraph::new(self.document.ids().node());
        if !self.text.is_empty() {
            if self.runs >= self.limits.max_runs {
                return Err(TextPreviewError::Budget("runs"));
            }
            self.runs += 1;
            paragraph
                .push_text(&self.text, None, Arc::default())
                .map_err(|_| TextPreviewError::InvalidDocument)?;
        }
        self.blocks.push(paragraph.into());
        self.text.clear();
        Ok(())
    }

    fn label(&mut self, text: &str) -> Result<(), TextPreviewError> {
        self.append(text)?;
        self.line()
    }

    fn block(&mut self, element: &Element) -> Result<(), TextPreviewError> {
        if element.namespace.as_deref() != Some(W) {
            return self.label("[Object]");
        }
        match element.local.as_ref() {
            "p" => {
                let style = element
                    .child(W, "pPr")
                    .and_then(|p| p.child(W, "pStyle"))
                    .and_then(props::val)
                    .map(StyleId::new);
                for child in element.elements() {
                    self.inline(child, style.as_ref())?;
                }
                self.line()
            }
            "tbl" => {
                self.label("[Table]")?;
                self.blocks_in(element)?;
                self.label("[/Table]")
            }
            "tc" => {
                self.label("[Cell]")?;
                self.blocks_in(element)
            }
            "tr" | "ins" | "moveTo" | "customXml" | "sdtContent" => self.blocks_in(element),
            "sdt" => match element.child(W, "sdtContent") {
                Some(content) => self.blocks_in(content),
                None => self.label("[Object]"),
            },
            "del" | "moveFrom" | "sectPr" | "tblPr" | "tblGrid" | "trPr" | "tcPr"
            | "customXmlPr" | "bookmarkStart" | "bookmarkEnd" | "proofErr"
            | "commentRangeStart" | "commentRangeEnd" => Ok(()),
            _ => self.label("[Object]"),
        }
    }

    fn blocks_in(&mut self, element: &Element) -> Result<(), TextPreviewError> {
        for child in element.elements() {
            self.block(child)?;
        }
        Ok(())
    }

    fn inline(
        &mut self,
        element: &Element,
        style: Option<&StyleId>,
    ) -> Result<(), TextPreviewError> {
        if element.namespace.as_deref() != Some(W) {
            return self.append("[Object]");
        }
        match element.local.as_ref() {
            "r" => {
                if let Some(properties) = element.child(W, "rPr") {
                    let (direct, character) = props::run(properties, &mut Findings::default());
                    if self
                        .source
                        .styles()
                        .resolve_run(style, character.as_ref(), &direct)
                        .props
                        .hidden
                        == Some(true)
                    {
                        return Ok(());
                    }
                    if properties
                        .child(W, "specVanish")
                        .is_some_and(|e| props::on_off(props::val(e)) != Some(false))
                    {
                        return Ok(());
                    }
                } else if self
                    .source
                    .styles()
                    .resolve_run(style, None, &Default::default())
                    .props
                    .hidden
                    == Some(true)
                {
                    return Ok(());
                }
                self.inlines_in(element, style)
            }
            "t" => self.append(&element.text()),
            "br" | "cr" => self.line(),
            "tab" => self.append("    "),
            "noBreakHyphen" => self.append("\u{2011}"),
            "softHyphen" => self.append("\u{00ad}"),
            "hyperlink" | "fldSimple" | "ins" | "moveTo" | "smartTag" | "customXml"
            | "sdtContent" => self.inlines_in(element, style),
            "sdt" => match element.child(W, "sdtContent") {
                Some(content) => self.inlines_in(content, style),
                None => self.append("[Object]"),
            },
            "del"
            | "moveFrom"
            | "delText"
            | "instrText"
            | "delInstrText"
            | "pPr"
            | "rPr"
            | "fldChar"
            | "bookmarkStart"
            | "bookmarkEnd"
            | "proofErr"
            | "commentRangeStart"
            | "commentRangeEnd"
            | "commentReference"
            | "lastRenderedPageBreak"
            | "customXmlPr"
            | "smartTagPr" => Ok(()),
            _ => self.append("[Object]"),
        }
    }

    fn inlines_in(
        &mut self,
        element: &Element,
        style: Option<&StyleId>,
    ) -> Result<(), TextPreviewError> {
        for child in element.elements() {
            self.inline(child, style)?;
        }
        Ok(())
    }
}
