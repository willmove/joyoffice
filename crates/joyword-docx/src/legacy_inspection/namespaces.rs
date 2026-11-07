//! Namespace safety for a main part already accepted by the bounded XML parser.
use crate::xml;
use quick_xml::{
    events::Event,
    name::{QName, ResolveResult},
    reader::NsReader,
};

const MC: &[u8] = b"http://schemas.openxmlformats.org/markup-compatibility/2006";

#[derive(Clone, Default)]
struct Scope {
    ignored: Vec<Vec<u8>>,
    alternate: bool,
}

pub(super) fn requires_read_only(bytes: &[u8]) -> Result<bool, &'static str> {
    let transcoded = xml::utf16_to_utf8(bytes);
    let bytes = transcoded.as_ref().map_or(bytes, |text| text.as_bytes());
    let mut reader = NsReader::from_reader(bytes);
    let mut scopes: Vec<Scope> = Vec::new();
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        let (namespace, event) = reader
            .read_resolved_event_into(&mut buffer)
            .map_err(|_| "invalid namespace scope")?;
        let namespace = match namespace {
            ResolveResult::Bound(namespace) => namespace.as_ref().to_vec(),
            ResolveResult::Unbound => Vec::new(),
            ResolveResult::Unknown(_) => return Err("unbound namespace prefix"),
        };
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                let mut scope = scopes.last().cloned().unwrap_or_default();
                // The legacy projection preserves AlternateContent as a whole,
                // without interpreting a Choice's unsupported vocabulary.
                scope.alternate |=
                    namespace == MC && element.local_name().as_ref() == b"AlternateContent";
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|_| "invalid namespace attribute")?;
                    let (ns, local) = reader.resolver().resolve_attribute(attribute.key);
                    if local.as_ref() != b"Ignorable"
                        || !matches!(ns, ResolveResult::Bound(ns) if ns.as_ref() == MC)
                    {
                        continue;
                    }
                    let value = attribute
                        .normalized_value(quick_xml::XmlVersion::Explicit1_0)
                        .map_err(|_| "invalid Ignorable value")?;
                    for prefix in value.split_whitespace() {
                        let qualified = format!("{prefix}:scope");
                        let (ns, _) = reader
                            .resolver()
                            .resolve_element(QName(qualified.as_bytes()));
                        if let ResolveResult::Bound(ns) = ns {
                            let uri = ns.as_ref();
                            if uri.len() > 4096 || scope.ignored.len() >= 64 {
                                return Err("Ignorable namespaces exceed budget");
                            }
                            if !scope.ignored.iter().any(|known| known == uri) {
                                scope.ignored.push(uri.to_vec());
                            }
                        }
                    }
                }
                if !scope.alternate
                    && xml::prefix_for(&namespace, &[]) == "?"
                    && !scope.ignored.contains(&namespace)
                {
                    return Ok(true);
                }
                if matches!(event, Event::Start(_)) {
                    scopes.push(scope);
                }
            }
            Event::End(_) => {
                scopes.pop();
            }
            Event::Eof => return Ok(false),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ignorable_is_namespace_based_and_scoped() -> Result<(), &'static str> {
        let wrap = |body: &str| {
            format!(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:x="urn:extension" xmlns:y="urn:extension"><w:body>{body}</w:body></w:document>"#
            )
        };
        for (body, expected) in [
            (r#"<w:p mc:Ignorable="x"><y:future/></w:p>"#, false),
            (
                r#"<w:p mc:Ignorable="x"><x:future xmlns:x="urn:different"/></w:p>"#,
                true,
            ),
            (
                r#"<w:p mc:Ignorable="x"><x:future/></w:p><x:future/>"#,
                true,
            ),
            (
                r#"<mc:AlternateContent><mc:Choice Requires="x"><x:future/></mc:Choice></mc:AlternateContent>"#,
                false,
            ),
        ] {
            let xml = wrap(body);
            assert_eq!(super::requires_read_only(xml.as_bytes())?, expected);
            let mut utf16 = vec![0xff, 0xfe];
            utf16.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
            assert_eq!(super::requires_read_only(&utf16)?, expected);
        }
        Ok(())
    }
}
