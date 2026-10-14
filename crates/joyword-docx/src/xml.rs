//! Bounded, namespace-aware XML reading for WordprocessingML parts.
//!
//! The same rules as the OPC layer's package XML apply (QUALITY-005): a
//! document type declaration is rejected outright, so no DTD, internal or
//! external entity is ever defined or fetched; only the five predefined
//! entities and character references are expanded; UTF-16 and declared
//! non-UTF-8 encodings are refused; nesting depth and the number of events are
//! checked against the [`Budget`] while reading, before the work they bound.
//!
//! Parts are read as a stream of top-level subtrees, so the main document is
//! never held as one tree: the body is mapped one block at a time. Every
//! element records its exact byte range in the part, which is how content the
//! model does not interpret is preserved (see [`crate::PackageBacking`]).

use std::ops::Range;
use std::sync::Arc;

use joyoffice_opc::{Budget, BudgetError, XmlError};
use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};

use crate::ns::MC;

/// Namespace bound to the `xml` prefix (Namespaces in XML 1.0, §3).
pub(crate) const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";

/// A namespace declaration: prefix (`""` for the default namespace) and URI.
pub(crate) type Declaration = (Arc<str>, Arc<str>);

#[derive(Clone, Debug)]
pub(crate) struct Attribute {
    pub namespace: Option<Arc<str>>,
    pub local: Box<str>,
    pub value: String,
}

#[derive(Clone, Debug)]
pub(crate) enum Node {
    Element(Element),
    Text(String),
}

#[derive(Clone, Debug)]
pub(crate) struct Element {
    pub namespace: Option<Arc<str>>,
    pub local: Box<str>,
    /// Name as written, e.g. `w:p`.
    pub qname: Box<str>,
    pub attributes: Vec<Attribute>,
    pub children: Vec<Node>,
    /// Namespace declarations made on this element.
    pub declarations: Vec<Declaration>,
    /// Byte range of the whole element in the part, start tag to end tag.
    pub span: Range<usize>,
}

impl Element {
    pub fn is(&self, namespace: &str, local: &str) -> bool {
        self.namespace.as_deref() == Some(namespace) && &*self.local == local
    }

    pub fn attribute(&self, namespace: Option<&str>, local: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|attribute| {
                attribute.namespace.as_deref() == namespace && &*attribute.local == local
            })
            .map(|attribute| attribute.value.as_str())
    }

    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|node| match node {
            Node::Element(element) => Some(element),
            Node::Text(_) => None,
        })
    }

    pub fn child(&self, namespace: &str, local: &str) -> Option<&Element> {
        self.elements().find(|child| child.is(namespace, local))
    }

    /// Concatenated character data of the direct children.
    pub fn text(&self) -> String {
        self.children
            .iter()
            .filter_map(|node| match node {
                Node::Text(text) => Some(text.as_str()),
                Node::Element(_) => None,
            })
            .collect()
    }

    /// Namespace URIs this element lists in `mc:Ignorable` or another
    /// markup-compatibility attribute, resolved against `scope`.
    pub fn prefixed_list(
        &self,
        namespace: &str,
        local: &str,
        scope: &[Declaration],
    ) -> Vec<Arc<str>> {
        let Some(list) = self.attribute(Some(namespace), local) else {
            return Vec::new();
        };
        list.split_ascii_whitespace()
            .filter_map(|prefix| {
                lookup(&self.declarations, prefix).or_else(|| lookup(scope, prefix))
            })
            .collect()
    }
}

fn lookup(scope: &[Declaration], prefix: &str) -> Option<Arc<str>> {
    scope
        .iter()
        .rev()
        .find(|(declared, _)| &**declared == prefix)
        .map(|(_, uri)| uri.clone())
}

/// A start tag read from the stream, before its content.
pub(crate) struct Start {
    pub element: Element,
    pub empty: bool,
}

pub(crate) enum Step {
    Start(Start),
    End,
    Text(String),
    Eof,
}

pub(crate) struct XmlReader<'a> {
    part: &'a str,
    reader: Reader<&'a [u8]>,
    /// Length of a skipped UTF-8 byte order mark, added to every span.
    offset: usize,
    budget: &'a Budget,
    depth: usize,
    events: u64,
    /// Declarations of each open element, outermost first.
    scopes: Vec<Vec<Declaration>>,
}

impl<'a> XmlReader<'a> {
    pub fn new(part: &'a str, bytes: &'a [u8], budget: &'a Budget) -> Result<Self, XmlError> {
        if bytes.starts_with(&[0xFE, 0xFF]) || bytes.starts_with(&[0xFF, 0xFE]) {
            return Err(XmlError::Utf16);
        }
        let stripped = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
        Ok(Self {
            part,
            reader: Reader::from_reader(stripped),
            offset: bytes.len() - stripped.len(),
            budget,
            depth: 0,
            events: 0,
            scopes: Vec::new(),
        })
    }

    fn malformed(&self, message: impl Into<String>) -> XmlError {
        XmlError::Malformed(format!(
            "{} at byte {}: {}",
            self.part,
            self.reader.buffer_position() as usize + self.offset,
            message.into()
        ))
    }

    fn resolve(
        &self,
        declarations: &[Declaration],
        prefix: Option<&str>,
    ) -> Result<Option<Arc<str>>, XmlError> {
        let prefix = prefix.unwrap_or("");
        if prefix == "xml" {
            return Ok(Some(XML_NAMESPACE.into()));
        }
        let found = lookup(declarations, prefix).or_else(|| {
            self.scopes
                .iter()
                .rev()
                .find_map(|scope| lookup(scope, prefix))
        });
        match found {
            // `xmlns=""` undeclares the default namespace.
            Some(uri) if uri.is_empty() => Ok(None),
            Some(uri) => Ok(Some(uri)),
            None if prefix.is_empty() => Ok(None),
            None => Err(self.malformed(format!("undeclared prefix {prefix:?}"))),
        }
    }

    fn start(&self, start: &BytesStart<'_>, begin: usize) -> Result<Element, XmlError> {
        let mut declarations = Vec::new();
        let mut raw = Vec::new();
        for attribute in start.attributes() {
            let attribute = attribute.map_err(|error| self.malformed(error.to_string()))?;
            let key = std::str::from_utf8(attribute.key.as_ref())
                .map_err(|_| self.malformed("attribute name is not UTF-8"))?
                .to_owned();
            let value = attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|error| self.malformed(error.to_string()))?
                .into_owned();
            if !value.chars().all(is_xml_char) {
                return Err(self.malformed("invalid XML character in attribute"));
            }
            if key == "xmlns" {
                declarations.push((Arc::from(""), Arc::from(value)));
            } else if let Some(prefix) = key.strip_prefix("xmlns:") {
                if value.is_empty() || prefix == "xmlns" || value == XMLNS_NAMESPACE {
                    return Err(self.malformed(format!("invalid declaration of {prefix:?}")));
                }
                declarations.push((Arc::from(prefix), Arc::from(value)));
            } else {
                raw.push((key, value));
            }
        }
        let qname = std::str::from_utf8(start.name().as_ref())
            .map_err(|_| self.malformed("element name is not UTF-8"))?
            .to_owned();
        let (prefix, local) = split(&qname);
        let namespace = self.resolve(&declarations, prefix)?;
        let mut attributes = Vec::with_capacity(raw.len());
        for (key, value) in raw {
            let (prefix, local) = split(&key);
            // Unprefixed attributes are in no namespace.
            let namespace = match prefix {
                Some(_) => self.resolve(&declarations, prefix)?,
                None => None,
            };
            // ECMA-376 Part 3 sections 7.2/7.4 require these prefix-list
            // values to name namespaces bound in this element's scope.
            // Do not let prefixed_list's filtering erase invalid mandatory
            // processing instructions before the importer can enforce them.
            if namespace.as_deref() == Some(MC) && matches!(local, "MustUnderstand" | "Ignorable") {
                for prefix in value.split_ascii_whitespace() {
                    let bound = self.resolve(&declarations, Some(prefix))?;
                    if bound.as_deref().is_none_or(|uri| uri == MC) {
                        return Err(self.malformed(format!("invalid mc:{local} prefix {prefix:?}")));
                    }
                }
            }
            if attributes.iter().any(|existing: &Attribute| {
                existing.namespace == namespace && &*existing.local == local
            }) {
                return Err(self.malformed(format!("duplicate attribute {key:?}")));
            }
            attributes.push(Attribute {
                namespace,
                local: local.into(),
                value,
            });
        }
        Ok(Element {
            namespace,
            local: local.into(),
            qname: qname.as_str().into(),
            attributes,
            children: Vec::new(),
            declarations,
            span: begin..begin,
        })
    }

    fn count(&mut self) -> Result<(), XmlError> {
        self.events += 1;
        if self.events > self.budget.max_xml_nodes {
            return Err(XmlError::Budget(BudgetError::XmlNodes {
                part: self.part.to_owned(),
                limit: self.budget.max_xml_nodes,
            }));
        }
        Ok(())
    }

    fn position(&self) -> usize {
        self.reader.buffer_position() as usize + self.offset
    }

    /// Next structural step; declarations, comments and processing
    /// instructions are consumed (and counted) here.
    pub fn step(&mut self) -> Result<Step, XmlError> {
        loop {
            let begin = self.position();
            let event = self
                .reader
                .read_event()
                .map_err(|error| self.malformed(error.to_string()))?;
            self.count()?;
            match event {
                Event::DocType(_) => return Err(XmlError::DocumentType),
                Event::Decl(declaration) => {
                    if let Some(encoding) = declaration.encoding() {
                        let encoding =
                            encoding.map_err(|error| self.malformed(error.to_string()))?;
                        let name = String::from_utf8_lossy(&encoding).to_ascii_lowercase();
                        if name != "utf-8" && name != "utf8" {
                            return Err(self.malformed(format!("unsupported encoding {name:?}")));
                        }
                    }
                }
                Event::Start(ref start) | Event::Empty(ref start) => {
                    let empty = matches!(event, Event::Empty(_));
                    let mut element = self.start(start, begin)?;
                    if empty {
                        element.span.end = self.position();
                    } else {
                        self.depth += 1;
                        if self.depth > self.budget.max_xml_depth {
                            return Err(XmlError::Budget(BudgetError::XmlDepth {
                                part: self.part.to_owned(),
                                limit: self.budget.max_xml_depth,
                            }));
                        }
                        self.scopes.push(element.declarations.clone());
                    }
                    return Ok(Step::Start(Start { element, empty }));
                }
                Event::End(_) => {
                    if self.depth == 0 {
                        return Err(self.malformed("unmatched end tag"));
                    }
                    self.depth -= 1;
                    self.scopes.pop();
                    return Ok(Step::End);
                }
                Event::Text(text) => {
                    let text = text
                        .xml10_content()
                        .map_err(|error| self.malformed(error.to_string()))?
                        .into_owned();
                    if !text.chars().all(is_xml_char) {
                        return Err(self.malformed("invalid XML character in text"));
                    }
                    if self.depth == 0 {
                        if text.trim_matches(is_xml_space).is_empty() {
                            continue;
                        }
                        return Err(self.malformed("text outside the root element"));
                    }
                    return Ok(Step::Text(text));
                }
                Event::CData(data) => {
                    let text = data
                        .xml10_content()
                        .map_err(|error| self.malformed(error.to_string()))?
                        .into_owned();
                    if !text.chars().all(is_xml_char) {
                        return Err(self.malformed("invalid XML character in CDATA"));
                    }
                    return Ok(Step::Text(text));
                }
                Event::GeneralRef(reference) => {
                    let ch = if reference.is_char_ref() {
                        reference
                            .resolve_char_ref()
                            .map_err(|error| self.malformed(error.to_string()))?
                            .filter(|&ch| is_xml_char(ch))
                            .ok_or_else(|| self.malformed("invalid character reference"))?
                    } else {
                        let name = reference
                            .decode()
                            .map_err(|error| self.malformed(error.to_string()))?;
                        match &*name {
                            "lt" => '<',
                            "gt" => '>',
                            "amp" => '&',
                            "apos" => '\'',
                            "quot" => '"',
                            other => {
                                return Err(self.malformed(format!("undefined entity {other:?}")));
                            }
                        }
                    };
                    return Ok(Step::Text(ch.to_string()));
                }
                Event::Comment(_) | Event::PI(_) => {}
                Event::Eof => {
                    if self.depth != 0 {
                        return Err(self.malformed("unclosed element"));
                    }
                    return Ok(Step::Eof);
                }
            }
        }
    }

    /// Reads the rest of an element whose start tag was just returned,
    /// building its subtree. Adjacent character data is joined.
    pub fn finish(&mut self, start: Start) -> Result<Element, XmlError> {
        let Start { element, empty } = start;
        if empty {
            return Ok(element);
        }
        let mut stack = vec![element];
        loop {
            match self.step()? {
                Step::Start(Start { element, empty }) => {
                    if empty {
                        push_node(&mut stack, Node::Element(element));
                    } else {
                        stack.push(element);
                    }
                }
                Step::Text(text) => {
                    let parent = stack.last_mut().expect("open element");
                    if let Some(Node::Text(previous)) = parent.children.last_mut() {
                        previous.push_str(&text);
                    } else {
                        parent.children.push(Node::Text(text));
                    }
                }
                Step::End => {
                    let mut done = stack.pop().expect("open element");
                    done.span.end = self.position();
                    if stack.is_empty() {
                        return Ok(done);
                    }
                    push_node(&mut stack, Node::Element(done));
                }
                Step::Eof => return Err(self.malformed("unexpected end of part")),
            }
        }
    }

    /// Skips leading markup to the root start tag.
    pub fn root(&mut self) -> Result<Start, XmlError> {
        match self.step()? {
            Step::Start(start) => Ok(start),
            _ => Err(XmlError::UnexpectedRoot),
        }
    }

    /// Reads a whole part as one tree; for small parts such as styles.
    pub fn whole(mut self) -> Result<Element, XmlError> {
        let root = self.root()?;
        let element = self.finish(root)?;
        match self.step()? {
            Step::Eof => Ok(element),
            _ => Err(self.malformed("content after the root element")),
        }
    }
}

fn push_node(stack: &mut [Element], node: Node) {
    stack.last_mut().expect("open element").children.push(node);
}

fn split(qname: &str) -> (Option<&str>, &str) {
    match qname.split_once(':') {
        Some((prefix, local)) => (Some(prefix), local),
        None => (None, qname),
    }
}

/// XML 1.0 white space (`S`, §2.3).
pub(crate) fn is_xml_space(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\r' | '\n')
}

/// XML 1.0 `Char` (§2.2).
fn is_xml_char(ch: char) -> bool {
    matches!(ch, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(xml: &str) -> Result<Element, XmlError> {
        XmlReader::new("/t.xml", xml.as_bytes(), &Budget::default())?.whole()
    }

    #[test]
    fn resolves_namespaces_spans_and_references() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><a:r xmlns:a="urn:a" xmlns="urn:d"><b x="1" a:y="&amp;2">t&lt;&#x4E2D;<![CDATA[<c>]]></b><c xmlns=""/></a:r>"#;
        let root = read(xml).unwrap();
        assert!(root.is("urn:a", "r"));
        assert_eq!(root.span, 38..xml.len());
        let b = root.child("urn:d", "b").unwrap();
        assert_eq!(b.attribute(None, "x"), Some("1"));
        assert_eq!(b.attribute(Some("urn:a"), "y"), Some("&2"));
        assert_eq!(b.text(), "t<中<c>");
        assert_eq!(
            &xml[b.span.clone()],
            r#"<b x="1" a:y="&amp;2">t&lt;&#x4E2D;<![CDATA[<c>]]></b>"#
        );
        let c = root.elements().nth(1).unwrap();
        assert_eq!(c.namespace, None);
    }

    #[test]
    fn rejects_doctype_entities_encodings_and_bad_names() {
        assert_eq!(
            read(r#"<!DOCTYPE a [<!ENTITY e "x">]><a>&e;</a>"#).unwrap_err(),
            XmlError::DocumentType
        );
        assert!(matches!(read("<a>&e;</a>"), Err(XmlError::Malformed(_))));
        assert!(matches!(read("<a>&#1;</a>"), Err(XmlError::Malformed(_))));
        assert!(matches!(
            read(r#"<?xml version="1.0" encoding="GB2312"?><a/>"#),
            Err(XmlError::Malformed(_))
        ));
        assert!(matches!(read("<p:a/>"), Err(XmlError::Malformed(_))));
        assert!(matches!(read("<a><b></a>"), Err(XmlError::Malformed(_))));
        assert!(matches!(read("<a>"), Err(XmlError::Malformed(_))));
        assert!(matches!(
            read(r#"<a x="1" x="2"/>"#),
            Err(XmlError::Malformed(_))
        ));
        assert_eq!(
            XmlReader::new("/t.xml", &[0xFF, 0xFE, b'<', 0], &Budget::default()).err(),
            Some(XmlError::Utf16)
        );
    }

    #[test]
    fn depth_and_event_budgets_apply_while_reading() {
        let budget = Budget {
            max_xml_depth: 3,
            max_xml_nodes: 10,
            ..Budget::default()
        };
        let deep = XmlReader::new("/t.xml", b"<a><b><c><d/></c></b></a>", &budget)
            .unwrap()
            .whole();
        assert!(deep.is_ok());
        let deeper = XmlReader::new("/t.xml", b"<a><b><c><d></d></c></b></a>", &budget)
            .unwrap()
            .whole();
        assert!(matches!(
            deeper,
            Err(XmlError::Budget(BudgetError::XmlDepth { .. }))
        ));
        let many = XmlReader::new(
            "/t.xml",
            b"<a><b/><b/><b/><b/><b/><b/><b/><b/><b/><b/></a>",
            &budget,
        )
        .unwrap()
        .whole();
        assert!(matches!(
            many,
            Err(XmlError::Budget(BudgetError::XmlNodes { .. }))
        ));
    }

    #[test]
    fn byte_order_mark_shifts_spans() {
        let bytes = b"\xEF\xBB\xBF<a><b/></a>";
        let root = XmlReader::new("/t.xml", bytes, &Budget::default())
            .unwrap()
            .whole()
            .unwrap();
        let b = root.elements().next().unwrap();
        assert_eq!(&bytes[b.span.clone()], b"<b/>");
    }
}
