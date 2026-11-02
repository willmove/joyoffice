//! Bounded XML reading for package-level parts. Document type declarations
//! are rejected outright (no DTDs, no entity definitions, no external
//! entities); only the predefined entities and character references are
//! expanded. Depth and event counts are limited per part.

use quick_xml::NsReader;
use quick_xml::XmlVersion;
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;

use crate::budget::{Budget, BudgetError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XmlError {
    /// `<!DOCTYPE …>` is never accepted in package XML.
    DocumentType,
    Utf16,
    Malformed(String),
    UnexpectedRoot,
    Budget(BudgetError),
}

/// One child element of the root: local name and unqualified attributes.
pub(crate) struct Child {
    pub local: String,
    pub attributes: Vec<(String, String)>,
}

/// Parses `<root xmlns=namespace>` with a flat list of children, returning
/// the children in the root's namespace. Children in other namespaces and
/// nested content are skipped but still bounded.
pub(crate) fn flat_children(
    part: &str,
    bytes: &[u8],
    budget: &Budget,
    namespace: &str,
    root: &str,
) -> Result<Vec<Child>, XmlError> {
    if bytes.starts_with(&[0xFE, 0xFF]) || bytes.starts_with(&[0xFF, 0xFE]) {
        return Err(XmlError::Utf16);
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let mut reader = NsReader::from_reader(bytes);
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut events = 0u64;
    let mut seen_root = false;
    let mut children = Vec::new();
    loop {
        buf.clear();
        let (resolved, event) = reader
            .read_resolved_event_into(&mut buf)
            .map_err(|error| XmlError::Malformed(error.to_string()))?;
        events += 1;
        if events > budget.max_xml_nodes {
            return Err(XmlError::Budget(BudgetError::XmlNodes {
                part: part.to_owned(),
                limit: budget.max_xml_nodes,
            }));
        }
        let in_namespace =
            matches!(resolved, ResolveResult::Bound(ns) if ns.0 == namespace.as_bytes());
        match event {
            Event::DocType(_) => return Err(XmlError::DocumentType),
            Event::Start(ref element) | Event::Empty(ref element) => {
                let empty = matches!(event, Event::Empty(_));
                if depth == 0 {
                    if seen_root
                        || !in_namespace
                        || element.local_name().as_ref() != root.as_bytes()
                    {
                        return Err(XmlError::UnexpectedRoot);
                    }
                    seen_root = true;
                } else if depth == 1 && in_namespace {
                    let mut attributes = Vec::new();
                    for attribute in element.attributes() {
                        let attribute =
                            attribute.map_err(|error| XmlError::Malformed(error.to_string()))?;
                        // Package XML attributes are unqualified.
                        if attribute.key.prefix().is_some() {
                            continue;
                        }
                        let value = attribute
                            .normalized_value(XmlVersion::Implicit1_0)
                            .map_err(|error| XmlError::Malformed(error.to_string()))?;
                        let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
                        attributes.push((key, value.into_owned()));
                    }
                    children.push(Child {
                        local: String::from_utf8_lossy(element.local_name().as_ref()).into_owned(),
                        attributes,
                    });
                }
                if !empty {
                    depth += 1;
                    if depth > budget.max_xml_depth {
                        return Err(XmlError::Budget(BudgetError::XmlDepth {
                            part: part.to_owned(),
                            limit: budget.max_xml_depth,
                        }));
                    }
                }
            }
            Event::End(_) => depth = depth.saturating_sub(1),
            Event::Eof => break,
            _ => {}
        }
    }
    if !seen_root || depth != 0 {
        return Err(XmlError::Malformed(
            "unclosed or missing root element".into(),
        ));
    }
    Ok(children)
}

pub(crate) fn attribute<'a>(child: &'a Child, name: &str) -> Option<&'a str> {
    child
        .attributes
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}
