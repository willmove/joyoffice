//! Preserve the imported theme; patch only explicitly edited model-owned attributes.
use std::collections::BTreeMap;

use quick_xml::{Writer, events::Event, name::ResolveResult, reader::NsReader};

use crate::{DocxError, xml};

pub(crate) const SOURCE: &str = "joyword:theme.source";
pub(crate) const BASELINE: &str = "joyword:theme.baseline";

fn failure() -> DocxError {
    DocxError::Xml(
        "Cannot safely update the preserved theme: missing, ambiguous or unsupported edited field"
            .into(),
    )
}

fn owned(bytes: &[u8]) -> Result<BTreeMap<String, String>, DocxError> {
    let root = xml::parse(bytes)?;
    let mut out = BTreeMap::new();
    out.insert("theme".into(), root.attr("name").unwrap_or_default().into());
    let elements = root.child("a:themeElements").ok_or_else(failure)?;
    let fonts = elements.child("a:fontScheme").ok_or_else(failure)?;
    for name in ["majorFont", "minorFont"] {
        let value = fonts
            .child(&format!("a:{name}"))
            .and_then(|f| f.child("a:latin"))
            .and_then(|f| f.attr("typeface"))
            .ok_or_else(failure)?;
        out.insert(
            format!("theme/themeElements/fontScheme/{name}/latin"),
            value.into(),
        );
    }
    let colors = elements.child("a:clrScheme").ok_or_else(failure)?;
    for child in colors.els() {
        let value = child
            .child("a:srgbClr")
            .and_then(|c| c.attr("val"))
            .ok_or_else(failure)?;
        out.insert(
            format!("theme/themeElements/clrScheme/{}", child.local()),
            value.into(),
        );
    }
    Ok(out)
}

pub(super) fn merge(
    original: &[u8],
    baseline: &[u8],
    generated: &[u8],
) -> Result<Vec<u8>, DocxError> {
    // The bounded parser validates even opaque data restored from a model snapshot.
    xml::parse(original)?;
    if baseline == generated {
        return Ok(original.to_vec());
    }
    let old = owned(baseline)?;
    let mut changed = owned(generated)?;
    changed.retain(|key, value| old.get(key) != Some(value));
    if changed.is_empty() {
        return Ok(original.to_vec());
    }
    let decoded = xml::utf16_to_utf8(original);
    let input = decoded.as_ref().map_or(original, |s| s.as_bytes());
    let mut reader = NsReader::from_reader(input);
    let mut path: Vec<String> = Vec::new();
    let mut patches = Vec::new();
    let mut hits = BTreeMap::<String, usize>::new();
    loop {
        let start = reader.buffer_position() as usize;
        let event = reader.read_event().map_err(|_| failure())?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(tag) | Event::Empty(tag) => {
                let (ns, local) = reader.resolver_mut().resolve_element(tag.name());
                let drawing = matches!(ns, ResolveResult::Bound(n) if n.as_ref() == b"http://schemas.openxmlformats.org/drawingml/2006/main" || n.as_ref() == b"http://purl.oclc.org/ooxml/drawingml/main");
                let local = std::str::from_utf8(local.as_ref()).map_err(|_| failure())?;
                path.push(if drawing {
                    local.to_string()
                } else {
                    "?".into()
                });
                let joined = path.join("/");
                let parent = path[..path.len().saturating_sub(1)].join("/");
                let color = drawing
                    && ["srgbClr", "sysClr"].contains(&local)
                    && parent.starts_with("theme/themeElements/clrScheme/");
                let key = if color { &parent } else { &joined };
                if let Some(value) = changed
                    .get(key)
                    .filter(|_| color || joined == "theme" || joined.ends_with("/latin"))
                {
                    // Color transforms would make a changed base color ambiguous. Reject
                    // non-empty color nodes rather than removing their children.
                    if color && !empty {
                        return Err(failure());
                    }
                    *hits.entry(key.clone()).or_default() += 1;
                    let attr = if color {
                        "val"
                    } else if joined == "theme" {
                        "name"
                    } else {
                        "typeface"
                    };
                    let mut replacement = tag.to_owned();
                    replacement.clear_attributes();
                    for a in tag.attributes() {
                        let a = a.map_err(|_| failure())?;
                        if a.key.as_ref() != attr.as_bytes()
                            && !(color && local == "sysClr" && a.key.as_ref() == b"lastClr")
                        {
                            replacement.push_attribute(a);
                        }
                    }
                    replacement.push_attribute((attr, value.as_str()));
                    if color && local == "sysClr" {
                        let name = std::str::from_utf8(tag.name().as_ref())
                            .map_err(|_| failure())?
                            .to_string();
                        let name = name
                            .rsplit_once(':')
                            .map_or("srgbClr".to_string(), |(p, _)| format!("{p}:srgbClr"));
                        replacement.set_name(name.as_bytes());
                    }
                    let mut writer = Writer::new(Vec::new());
                    writer
                        .write_event(if empty {
                            Event::Empty(replacement)
                        } else {
                            Event::Start(replacement)
                        })
                        .map_err(|_| failure())?;
                    patches.push((
                        start,
                        reader.buffer_position() as usize,
                        writer.into_inner(),
                    ));
                }
                if empty {
                    path.pop();
                }
            }
            Event::End(_) => {
                path.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if changed.keys().any(|k| hits.get(k) != Some(&1)) {
        return Err(failure());
    }
    let mut result = Vec::new();
    let mut offset = 0;
    for (start, end, replacement) in patches {
        result.extend_from_slice(input.get(offset..start).ok_or_else(failure)?);
        result.extend(replacement);
        offset = end;
    }
    result.extend_from_slice(input.get(offset..).ok_or_else(failure)?);
    xml::parse(&result)?;
    if decoded.is_some() {
        let text = std::str::from_utf8(&result).map_err(|_| failure())?;
        let little = original.starts_with(&[0xff, 0xfe]);
        let mut encoded = original[..2].to_vec();
        for unit in text.encode_utf16() {
            encoded.extend(if little {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        return Ok(encoded);
    }
    Ok(result)
}
