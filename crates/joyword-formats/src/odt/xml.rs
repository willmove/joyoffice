use super::{BytesStart, Event, XmlVersion};
use quick_xml::{
    NsReader,
    name::{NamespaceResolver, QName, ResolveResult},
};

pub(super) const OFFICE: &[u8] = b"urn:oasis:names:tc:opendocument:xmlns:office:1.0";

/// Length → points.
pub(super) fn length(v: &str) -> Option<f32> {
    let v = v.trim().to_ascii_lowercase();
    let num = |s: &str| s.trim().parse::<f32>().ok().filter(|x| x.is_finite());
    if let Some(n) = v.strip_suffix("pt") {
        num(n)
    } else if let Some(n) = v.strip_suffix("in") {
        num(n).map(|x| x * 72.0)
    } else if let Some(n) = v.strip_suffix("cm") {
        num(n).map(|x| x * 72.0 / 2.54)
    } else if let Some(n) = v.strip_suffix("mm") {
        num(n).map(|x| x * 72.0 / 25.4)
    } else if let Some(n) = v.strip_suffix("pc") {
        num(n).map(|x| x * 12.0)
    } else if let Some(n) = v.strip_suffix("px") {
        num(n).map(|x| x * 0.75)
    } else {
        num(&v)
    }
}

pub(super) type Attrs = Vec<(String, String)>;

fn attrs_of(e: &BytesStart<'_>, namespaces: Option<&NamespaceResolver>) -> Attrs {
    let mut v = Vec::new();
    for a in e.attributes().with_checks(false).flatten().take(128) {
        let k = name(a.key, namespaces, true);
        let val = a
            .normalized_value(XmlVersion::Explicit1_0)
            .map(|c| c.into_owned())
            .unwrap_or_else(|_| String::from_utf8_lossy(&a.value).into_owned());
        v.push((k, val));
    }
    v
}

fn name(raw: QName<'_>, namespaces: Option<&NamespaceResolver>, attribute: bool) -> String {
    let Some(resolver) = namespaces else {
        return String::from_utf8_lossy(raw.as_ref()).into_owned();
    };
    let (ns, local) = resolver.resolve(raw, !attribute);
    let prefix = match ns {
        ResolveResult::Bound(ns) => match ns.as_ref() {
            OFFICE => "office",
            b"urn:oasis:names:tc:opendocument:xmlns:text:1.0" => "text",
            b"urn:oasis:names:tc:opendocument:xmlns:style:1.0" => "style",
            b"urn:oasis:names:tc:opendocument:xmlns:table:1.0" => "table",
            b"urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" => "draw",
            b"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" => "fo",
            b"urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" => "svg",
            b"urn:oasis:names:tc:opendocument:xmlns:meta:1.0" => "meta",
            b"http://purl.org/dc/elements/1.1/" => "dc",
            b"http://www.w3.org/1999/xlink" => "xlink",
            b"http://www.w3.org/XML/1998/namespace" => "xml",
            b"urn:org:documentfoundation:names:experimental:office:xmlns:loext:1.0" => "loext",
            _ => return String::new(),
        },
        _ => return String::new(),
    };
    format!("{prefix}:{}", String::from_utf8_lossy(local.as_ref()))
}

pub(super) fn get<'a>(a: &'a Attrs, k: &str) -> Option<&'a str> {
    a.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
}

pub(super) fn local(q: &str) -> &str {
    q.rsplit(':').next().unwrap_or(q)
}

pub(super) enum Ev {
    Start(String, Attrs),
    End(String),
    Text(String),
}

/// A package member, or a flat document section with the root's in-scope namespace bindings.
#[derive(Clone, Copy)]
pub(super) struct Xml<'a> {
    pub bytes: &'a [u8],
    pub root: Option<&'a BytesStart<'a>>,
}

impl<'a> From<&'a [u8]> for Xml<'a> {
    fn from(bytes: &'a [u8]) -> Self {
        Self { bytes, root: None }
    }
}

pub(super) fn events(xml: &[u8], f: impl FnMut(Ev)) {
    stream(xml.into(), f);
}

/// Stream XML events (empty elements become start + end). Malformed XML ends the stream.
pub(super) fn stream(xml: Xml<'_>, mut f: impl FnMut(Ev)) {
    if let Some(root) = xml.root {
        let mut reader = NsReader::from_reader(xml.bytes);
        if reader.resolver_mut().push(root).is_err() {
            return;
        }
        // The complete flat document has already been checked for well-formedness.
        while let Ok(event) = reader.read_event()
            && emit(event, Some(reader.resolver()), &mut f)
        {}
    } else {
        // Package members keep the established lenient handling of malformed XML.
        let mut reader = quick_xml::Reader::from_reader(xml.bytes);
        let config = reader.config_mut();
        config.check_end_names = false;
        config.allow_unmatched_ends = true;
        while let Ok(event) = reader.read_event()
            && emit(event, None, &mut f)
        {}
    }
}

fn emit(event: Event<'_>, ns: Option<&NamespaceResolver>, f: &mut impl FnMut(Ev)) -> bool {
    match event {
        Event::Start(e) => f(Ev::Start(name(e.name(), ns, false), attrs_of(&e, ns))),
        Event::Empty(e) => {
            let n = name(e.name(), ns, false);
            f(Ev::Start(n.clone(), attrs_of(&e, ns)));
            f(Ev::End(n));
        }
        Event::End(e) => f(Ev::End(name(e.name(), ns, false))),
        Event::Text(t) => f(Ev::Text(
            t.decode().map(|c| c.into_owned()).unwrap_or_default(),
        )),
        Event::CData(t) => f(Ev::Text(String::from_utf8_lossy(&t).into_owned())),
        Event::GeneralRef(r) => {
            let name = String::from_utf8_lossy(&r);
            let s = match name.as_ref() {
                "amp" => "&".to_string(),
                "lt" => "<".to_string(),
                "gt" => ">".to_string(),
                "quot" => "\"".to_string(),
                "apos" => "'".to_string(),
                _ => r
                    .resolve_char_ref()
                    .ok()
                    .flatten()
                    .map(|c| c.to_string())
                    .unwrap_or_default(),
            };
            f(Ev::Text(s));
        }
        Event::Eof => return false,
        _ => {}
    }
    true
}
