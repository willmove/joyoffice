//! ODF 1.3 Part 3 §3.1.2. Index the standalone XML without copying its body or images.
use std::ops::Range;

use quick_xml::{
    NsReader,
    name::{Namespace, NamespaceResolver, QName, ResolveResult},
};

use super::{
    BytesStart, Event, XmlVersion,
    limits::{MAX_FLAT_BYTES, MAX_XML_DEPTH},
    xml::{OFFICE, Xml},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Section {
    Metadata,
    Fonts,
    Styles,
    Automatic,
    Masters,
    Body,
}

impl Section {
    fn of(local: &[u8]) -> Option<Self> {
        Some(match local {
            b"meta" => Self::Metadata,
            b"font-face-decls" => Self::Fonts,
            b"styles" => Self::Styles,
            b"automatic-styles" => Self::Automatic,
            b"master-styles" => Self::Masters,
            b"body" => Self::Body,
            _ => return None,
        })
    }
}

pub(super) struct FlatDocument<'a> {
    bytes: &'a [u8],
    root: BytesStart<'a>,
    sections: [Option<Range<usize>>; 6],
}

fn office_name<'a>(resolver: &NamespaceResolver, name: QName<'a>) -> Option<&'a [u8]> {
    let (ns, local) = resolver.resolve_element(name);
    (ns == ResolveResult::Bound(Namespace(OFFICE))).then(|| local.into_inner())
}

impl<'a> FlatDocument<'a> {
    pub(super) fn open(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.len() > MAX_FLAT_BYTES {
            return Err("FODT exceeds 64 MiB".into());
        }
        std::str::from_utf8(bytes).map_err(|_| "FODT requires UTF-8 XML")?;
        let mut reader = NsReader::from_reader(bytes);
        let mut root = None;
        let mut sections = [const { None }; 6];
        let mut section = None;
        let mut depth = 0;
        let mut text = false;
        loop {
            let start = reader.buffer_position() as usize;
            let event = reader.read_event().map_err(|e| format!("FODT XML: {e}"))?;
            let end = reader.buffer_position() as usize;
            let empty = matches!(&event, Event::Empty(_));
            match event {
                Event::Start(tag) | Event::Empty(tag) => {
                    let resolver = reader.resolver();
                    let name = office_name(resolver, tag.name());
                    match depth {
                        0 => {
                            if root.is_some() || name != Some(b"document") || empty {
                                return Err("FODT requires one office:document root".into());
                            }
                            let mime = tag.attributes().filter_map(Result::ok).find_map(|a| {
                                let (ns, name) = resolver.resolve_attribute(a.key);
                                (ns == ResolveResult::Bound(Namespace(OFFICE))
                                    && name.as_ref() == b"mimetype")
                                    .then(|| a.normalized_value(XmlVersion::Explicit1_0).ok())
                                    .flatten()
                            });
                            if !matches!(
                                mime.as_deref(),
                                Some(
                                    "application/vnd.oasis.opendocument.text"
                                        | "application/vnd.oasis.opendocument.text-template"
                                )
                            ) {
                                return Err("FODT root is not OpenDocument Text".into());
                            }
                            root = Some(tag);
                        }
                        1 => {
                            if let Some(kind) = name.and_then(Section::of) {
                                if sections[kind as usize].is_some() {
                                    return Err("Duplicate FODT document section".into());
                                }
                                if empty {
                                    sections[kind as usize] = Some(start..end);
                                } else {
                                    section = Some((kind, start));
                                }
                            }
                        }
                        2 if section.is_some_and(|(kind, _)| kind == Section::Body) => {
                            if text || name != Some(b"text") {
                                return Err("FODT body must contain one office:text".into());
                            }
                            text = true;
                        }
                        _ => {}
                    }
                    depth += usize::from(!empty);
                    if depth > MAX_XML_DEPTH {
                        return Err("FODT XML nesting exceeds limit".into());
                    }
                }
                Event::End(_) => {
                    depth -= 1;
                    if depth == 1
                        && let Some((kind, start)) = section.take()
                    {
                        sections[kind as usize] = Some(start..end);
                    }
                }
                Event::DocType(_) => {
                    return Err("FODT does not allow a document type declaration".into());
                }
                Event::Text(t) if depth == 0 && t.iter().any(|c| !c.is_ascii_whitespace()) => {
                    return Err("Unexpected text outside the FODT root".into());
                }
                Event::CData(_) | Event::GeneralRef(_) if depth == 0 => {
                    return Err("Unexpected content outside the FODT root".into());
                }
                Event::Eof => break,
                _ => {}
            }
        }
        let root = root.ok_or("FODT root is missing")?;
        if depth != 0 || !text || sections[Section::Body as usize].is_none() {
            return Err("Incomplete FODT text document".into());
        }
        Ok(Self {
            bytes,
            root,
            sections,
        })
    }

    pub(super) fn section(&self, kind: Section) -> Option<Xml<'_>> {
        let range = self.sections[kind as usize].clone()?;
        Some(Xml {
            bytes: &self.bytes[range],
            root: Some(&self.root),
        })
    }
}
