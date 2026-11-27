//! Bounded lexical reader for OPC XML members and carried DrawingML fragments.

use std::borrow::Cow;

use quick_xml::{
    Reader, XmlVersion,
    events::{BytesStart, Event},
};

use crate::PptxError;
use crate::xml::{Doc, El, MAX_DEPTH, MAX_NODES, Node};

fn decode(bytes: &[u8]) -> Cow<'_, [u8]> {
    let endian = match bytes.get(..2) {
        Some([0xff, 0xfe]) => u16::from_le_bytes,
        Some([0xfe, 0xff]) => u16::from_be_bytes,
        _ => return Cow::Borrowed(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes)),
    };
    let units = bytes[2..].as_chunks::<2>().0.iter().copied().map(endian);
    let mut text: String = char::decode_utf16(units)
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect();
    if text.starts_with("<?xml")
        && let Some(declaration) = text.find("?>")
    {
        text.drain(..declaration + 2);
    }
    Cow::Owned(text.into_bytes())
}

/// Check the envelope of an XML part with the host's limits before importing any model data.
pub fn validate(bytes: &[u8], max_depth: usize, max_events: u64) -> Result<(), String> {
    let xml = decode(bytes);
    let mut reader = Reader::from_reader(xml.as_ref());
    let mut depth = 0;
    for _ in 0..max_events {
        match reader
            .read_event()
            .map_err(|error| format!("invalid XML: {error}"))?
        {
            Event::Start(_) => {
                depth += 1;
                if depth > max_depth {
                    return Err("XML depth budget exceeded".into());
                }
            }
            Event::End(_) => depth = depth.saturating_sub(1),
            Event::DocType(_) => return Err("DTDs are unsupported".into()),
            Event::Eof if depth == 0 => return Ok(()),
            Event::Eof => return Err("unclosed XML element".into()),
            _ => {}
        }
    }
    Err("XML node budget exceeded".into())
}

/// Decode one view of a part. The standalone fragment importer also handles unfinished elements.
pub fn parse(bytes: &[u8]) -> Result<Doc, PptxError> {
    let xml = decode(bytes);
    let mut reader = Reader::from_reader(xml.as_ref());
    reader.config_mut().check_end_names = false;
    reader.config_mut().allow_unmatched_ends = true;
    let mut parents = Vec::new();
    let mut root = None;
    let mut before_root = Vec::new();
    let mut after_root = Vec::new();
    let mut element_count = 0;
    let mut event_count = 0;
    loop {
        let event = reader
            .read_event()
            .map_err(|error| PptxError::Xml(error.to_string()))?;
        if matches!(event, Event::Eof) {
            break;
        }
        event_count += 1;
        if event_count > MAX_NODES * 8 {
            return Err(PptxError::Xml("document has too many XML events".into()));
        }
        if root.is_some() && !matches!(event, Event::Comment(_) | Event::PI(_)) {
            continue;
        }
        match event {
            Event::Start(start) => {
                element_count += 1;
                if element_count > MAX_NODES || parents.len() >= MAX_DEPTH {
                    return Err(PptxError::Xml(
                        "document too large or too deeply nested".into(),
                    ));
                }
                parents.push(element(&start, &reader));
            }
            Event::Empty(start) => {
                element_count += 1;
                if element_count > MAX_NODES || parents.len() >= MAX_DEPTH {
                    return Err(PptxError::Xml(
                        "document too large or too deeply nested".into(),
                    ));
                }
                append_element(&mut parents, &mut root, element(&start, &reader));
            }
            Event::End(_) => {
                if let Some(closed) = parents.pop() {
                    append_element(&mut parents, &mut root, closed);
                }
            }
            Event::Text(text) => {
                if let (Some(parent), Ok(content)) = (parents.last_mut(), text.decode()) {
                    append_text(parent, &normalize_lines(content));
                }
            }
            Event::CData(text) => {
                if let Some(parent) = parents.last_mut() {
                    append_text(parent, &normalize_lines(String::from_utf8_lossy(&text)));
                }
            }
            Event::GeneralRef(reference) => {
                if let Some(parent) = parents.last_mut() {
                    if reference.is_char_ref() {
                        if let Ok(Some(ch)) = reference.resolve_char_ref() {
                            append_text(parent, ch.encode_utf8(&mut [0; 4]));
                        }
                    } else if let Ok(name) = reference.decode()
                        && let Some(value) = quick_xml::escape::resolve_xml_entity(&name)
                    {
                        append_text(parent, value);
                    }
                }
            }
            Event::Comment(comment) => {
                if let Ok(text) = comment.decode() {
                    append_markup(
                        &mut parents,
                        if root.is_some() {
                            &mut after_root
                        } else {
                            &mut before_root
                        },
                        format!("<!--{text}-->"),
                    );
                }
            }
            Event::PI(instruction) => {
                append_markup(
                    &mut parents,
                    if root.is_some() {
                        &mut after_root
                    } else {
                        &mut before_root
                    },
                    format!("<?{}?>", String::from_utf8_lossy(&instruction)),
                );
            }
            Event::DocType(_) => return Err(PptxError::Xml("DTDs are unsupported".into())),
            _ => {}
        }
    }
    while let Some(open) = parents.pop() {
        append_element(&mut parents, &mut root, open);
    }
    Ok(Doc {
        root: root.ok_or_else(|| PptxError::Xml("no root element".into()))?,
        before_root,
        after_root,
    })
}

fn append_markup(parents: &mut [El], outside: &mut Vec<String>, markup: String) {
    if let Some(parent) = parents.last_mut() {
        parent.children.push(Node::Markup(markup));
    } else {
        outside.push(markup);
    }
}

fn element(start: &BytesStart, reader: &Reader<&[u8]>) -> El {
    let attrs = start
        .attributes()
        .with_checks(false)
        .filter_map(Result::ok)
        .map(|attr| {
            let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
            let value = attr
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                .unwrap_or_else(|_| String::from_utf8_lossy(&attr.value))
                .into_owned();
            (key, value)
        })
        .collect();
    El {
        name: String::from_utf8_lossy(start.name().as_ref()).into_owned(),
        attrs,
        children: Vec::new(),
    }
}

fn append_element(parents: &mut [El], root: &mut Option<El>, element: El) {
    if let Some(parent) = parents.last_mut() {
        parent.children.push(Node::El(element));
    } else {
        *root = Some(element);
    }
}

/// XML 1.0 line normalization applies to literal text before references become characters.
fn normalize_lines(text: Cow<'_, str>) -> Cow<'_, str> {
    if text.contains('\r') {
        Cow::Owned(text.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        text
    }
}

fn append_text(parent: &mut El, value: &str) {
    if !value.is_empty() {
        if let Some(Node::Text(text)) = parent.children.last_mut() {
            text.push_str(value);
        } else {
            parent.children.push(Node::Text(value.into()));
        }
    }
}
