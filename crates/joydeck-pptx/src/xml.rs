//! A small, bounded XML DOM over quick-xml for reading parts, plus an escaping string builder for
//! writing them.
//!
//! Reading is lenient about namespaces: elements and attributes are matched by local name, which is
//! how real-world files are best understood (prefixes are conventional and stable in practice).

use std::fmt::Write as _;

pub use crate::xml_read::parse;

/// Maximum element nesting accepted when parsing.
pub const MAX_DEPTH: usize = 256;
/// Maximum number of elements in one part.
pub const MAX_NODES: usize = 4_000_000;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct El {
    /// Qualified name as written (`p:sp`).
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    El(El),
    Text(String),
    /// An XML comment or processing instruction in a part being preserved.
    Markup(String),
}

fn local_of(name: &str) -> &str {
    name.rsplit_once(':').map(|(_, l)| l).unwrap_or(name)
}

impl El {
    pub fn local(&self) -> &str {
        local_of(&self.name)
    }
    pub fn prefix(&self) -> &str {
        self.name.split_once(':').map(|(p, _)| p).unwrap_or("")
    }
    pub fn is(&self, local: &str) -> bool {
        self.local() == local
    }
    /// Attribute by local name (`id` matches `id`; `r:id` matches `id` only when no plain `id`
    /// exists). Pass a qualified name (`r:id`) to require that prefix's local name with any prefix.
    pub fn attr(&self, name: &str) -> Option<&str> {
        if let Some((_, value)) = self.attrs.iter().find(|(key, _)| key == name) {
            return Some(value);
        }
        if let Some((_, l)) = name.split_once(':') {
            return self
                .attrs
                .iter()
                .find(|(k, _)| k.contains(':') && !k.starts_with("xmlns") && local_of(k) == l)
                .map(|(_, v)| v.as_str());
        }
        self.attrs
            .iter()
            .find(|(k, _)| !k.starts_with("xmlns") && local_of(k) == name)
            .map(|(_, v)| v.as_str())
    }
    pub fn has_attr(&self, name: &str) -> bool {
        self.attr(name).is_some()
    }
    pub fn set_attr(&mut self, name: &str, value: &str) {
        if let Some(a) = self.attrs.iter_mut().find(|(k, _)| k == name) {
            a.1 = value.to_string();
        } else {
            self.attrs.push((name.to_string(), value.to_string()));
        }
    }
    pub fn i64(&self, name: &str) -> Option<i64> {
        let v = self.attr(name)?.trim();
        v.parse::<i64>().ok().or_else(|| {
            v.parse::<f64>()
                .ok()
                .filter(|f| f.is_finite())
                .map(|f| f.clamp(-9.0e18, 9.0e18) as i64)
        })
    }
    pub fn i32(&self, name: &str) -> Option<i32> {
        self.i64(name)
            .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
    }
    pub fn u32(&self, name: &str) -> Option<u32> {
        self.i64(name).map(|v| v.clamp(0, u32::MAX as i64) as u32)
    }
    pub fn f64(&self, name: &str) -> Option<f64> {
        self.attr(name)?
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|f| f.is_finite())
    }
    pub fn bool(&self, name: &str) -> Option<bool> {
        match self.attr(name)?.trim() {
            "1" | "true" | "on" => Some(true),
            "0" | "false" | "off" => Some(false),
            _ => None,
        }
    }
    pub fn elements(&self) -> impl Iterator<Item = &El> {
        self.children.iter().filter_map(|n| match n {
            Node::El(e) => Some(e),
            Node::Text(_) | Node::Markup(_) => None,
        })
    }
    pub fn elements_mut(&mut self) -> impl Iterator<Item = &mut El> {
        self.children.iter_mut().filter_map(|n| match n {
            Node::El(e) => Some(e),
            Node::Text(_) | Node::Markup(_) => None,
        })
    }
    pub fn child(&self, local: &str) -> Option<&El> {
        self.elements().find(|e| e.is(local))
    }
    pub fn children_named<'a>(&'a self, local: &'a str) -> impl Iterator<Item = &'a El> + 'a {
        self.elements().filter(move |e| e.is(local))
    }
    /// Follow a chain of child local names.
    pub fn path(&self, names: &[&str]) -> Option<&El> {
        let mut cur = self;
        for n in names {
            cur = cur.child(n)?;
        }
        Some(cur)
    }
    /// First descendant (depth-first, bounded) with this local name.
    pub fn find(&self, local: &str) -> Option<&El> {
        fn rec<'a>(e: &'a El, local: &str, depth: usize) -> Option<&'a El> {
            if depth > MAX_DEPTH {
                return None;
            }
            for c in e.elements() {
                if c.is(local) {
                    return Some(c);
                }
                if let Some(f) = rec(c, local, depth + 1) {
                    return Some(f);
                }
            }
            None
        }
        rec(self, local, 0)
    }
    /// All descendants with this local name (bounded depth).
    pub fn find_all<'a>(&'a self, local: &str, out: &mut Vec<&'a El>) {
        fn rec<'a>(e: &'a El, local: &str, depth: usize, out: &mut Vec<&'a El>) {
            if depth > MAX_DEPTH {
                return;
            }
            for c in e.elements() {
                if c.is(local) {
                    out.push(c);
                }
                rec(c, local, depth + 1, out);
            }
        }
        rec(self, local, 0, out)
    }
    /// Concatenated text content of this element and its descendants.
    pub fn text(&self) -> String {
        let mut s = String::new();
        fn rec(e: &El, s: &mut String, depth: usize) {
            if depth > MAX_DEPTH {
                return;
            }
            for c in &e.children {
                match c {
                    Node::Text(t) => s.push_str(t),
                    Node::El(e) => rec(e, s, depth + 1),
                    Node::Markup(_) => {}
                }
            }
        }
        rec(self, &mut s, 0);
        s
    }
    /// Value of `<x val="…"/>` child.
    pub fn child_val(&self, local: &str) -> Option<&str> {
        self.child(local).and_then(|c| c.attr("val"))
    }
    /// Serialize to a string (no XML declaration).
    pub fn to_xml(&self) -> String {
        let mut s = String::new();
        write_el(self, &mut s, 0);
        s
    }
}

fn write_el(e: &El, s: &mut String, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    s.push('<');
    s.push_str(&e.name);
    for (k, v) in &e.attrs {
        let _ = write!(s, " {}=\"{}\"", k, esc_attr(v));
    }
    if e.children.is_empty() {
        s.push_str("/>");
        return;
    }
    s.push('>');
    for c in &e.children {
        match c {
            Node::Text(t) => s.push_str(&esc(t)),
            Node::El(c) => write_el(c, s, depth + 1),
            Node::Markup(raw) => s.push_str(raw),
        }
    }
    s.push_str("</");
    s.push_str(&e.name);
    s.push('>');
}

/// A parsed document: its root element and the namespace declarations on the root.
#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub root: El,
}

impl Doc {
    /// Namespace declarations (`xmlns:x`, `xmlns`) on the root element.
    pub fn ns_decls(&self) -> Vec<(String, String)> {
        self.root
            .attrs
            .iter()
            .filter(|(k, _)| k == "xmlns" || k.starts_with("xmlns:"))
            .cloned()
            .collect()
    }
}

/// Is `c` allowed in XML 1.0 text?
fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r') || (c >= ' ' && c != '\u{FFFE}' && c != '\u{FFFF}')
}

/// Escape text content (and drop characters XML 1.0 forbids).
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '\r' => o.push_str("&#13;"),
            c if xml_char(c) => o.push(c),
            _ => {}
        }
    }
    o
}

/// Escape an attribute value.
pub fn esc_attr(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\n' => o.push_str("&#10;"),
            '\r' => o.push_str("&#13;"),
            '\t' => o.push_str("&#9;"),
            c if xml_char(c) => o.push(c),
            _ => {}
        }
    }
    o
}

/// A string builder for XML parts.
#[derive(Default)]
pub struct W {
    pub s: String,
}

/// Attribute list builder.
#[derive(Default)]
pub struct A {
    pub v: Vec<(&'static str, String)>,
}

impl A {
    pub fn new() -> Self {
        A { v: vec![] }
    }
    pub fn a(mut self, k: &'static str, v: impl ToString) -> Self {
        self.v.push((k, v.to_string()));
        self
    }
    pub fn o<T: ToString>(mut self, k: &'static str, v: Option<T>) -> Self {
        if let Some(v) = v {
            self.v.push((k, v.to_string()));
        }
        self
    }
    /// Boolean attribute written as `1`/`0` when `Some`.
    pub fn b(mut self, k: &'static str, v: Option<bool>) -> Self {
        if let Some(v) = v {
            self.v.push((k, if v { "1" } else { "0" }.to_string()));
        }
        self
    }
    /// Boolean attribute written only when true.
    pub fn t(mut self, k: &'static str, v: bool) -> Self {
        if v {
            self.v.push((k, "1".to_string()));
        }
        self
    }
}

impl W {
    pub fn new() -> Self {
        W {
            s: String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n"),
        }
    }
    pub fn frag() -> Self {
        W { s: String::new() }
    }
    fn attrs(&mut self, a: &A) {
        for (k, v) in &a.v {
            let _ = write!(self.s, " {}=\"{}\"", k, esc_attr(v));
        }
    }
    pub fn open(&mut self, name: &str, a: A) {
        self.s.push('<');
        self.s.push_str(name);
        self.attrs(&a);
        self.s.push('>');
    }
    pub fn open0(&mut self, name: &str) {
        self.s.push('<');
        self.s.push_str(name);
        self.s.push('>');
    }
    pub fn close(&mut self, name: &str) {
        self.s.push_str("</");
        self.s.push_str(name);
        self.s.push('>');
    }
    pub fn empty(&mut self, name: &str, a: A) {
        self.s.push('<');
        self.s.push_str(name);
        self.attrs(&a);
        self.s.push_str("/>");
    }
    pub fn empty0(&mut self, name: &str) {
        self.s.push('<');
        self.s.push_str(name);
        self.s.push_str("/>");
    }
    /// `<name val="v"/>`
    pub fn val(&mut self, name: &str, v: impl ToString) {
        self.empty(name, A::new().a("val", v));
    }
    pub fn text(&mut self, t: &str) {
        self.s.push_str(&esc(t));
    }
    /// `<name>text</name>`
    pub fn elt(&mut self, name: &str, t: &str) {
        self.open0(name);
        self.text(t);
        self.close(name);
    }
    pub fn raw(&mut self, xml: &str) {
        self.s.push_str(xml);
    }
    pub fn finish(self) -> Vec<u8> {
        self.s.into_bytes()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn literal_cr_lf_and_lone_cr_read_as_lf_but_char_ref_stays() {
        let d =
            parse(b"<a:p><a:t>one\r\ntwo\rthree&#13;four</a:t><a:t><![CDATA[x\r\ny]]></a:t></a:p>")
                .expect("parse");
        let t: Vec<String> = d.root.children_named("t").map(|t| t.text()).collect();
        assert_eq!(
            t,
            vec!["one\ntwo\nthree\rfour".to_string(), "x\ny".to_string()]
        );
    }

    #[test]
    fn cr_in_text_survives_write_read() {
        assert_eq!(esc("a\rb"), "a&#13;b");
        let mut w = W::default();
        w.elt("a:t", "one\rtwo\r\nthree");
        let d = parse(w.s.as_bytes()).expect("parse");
        assert_eq!(d.root.text(), "one\rtwo\r\nthree");
    }

    #[test]
    fn parse_basic_and_entities() {
        let d = parse(b"<?xml version=\"1.0\"?><a:p xmlns:a=\"x\"><a:r><a:t>A &amp; B &#x41; &lt;</a:t></a:r><a:x r:id=\"rId1\" id=\"5\"/></a:p>")
            .unwrap();
        assert_eq!(d.root.local(), "p");
        assert_eq!(
            d.root.find("t").map(|t| t.text()),
            Some("A & B A <".to_string())
        );
        let x = d.root.child("x").unwrap();
        assert_eq!(x.attr("id"), Some("5"));
        assert_eq!(x.attr("r:id"), Some("rId1"));
        assert_eq!(d.ns_decls().len(), 1);
    }

    #[test]
    fn truncated_and_garbage() {
        assert!(parse(b"<a><b><c>text").is_ok());
        assert!(parse(b"").is_err());
        let deep = "<a>".repeat(MAX_DEPTH + 10);
        assert!(parse(deep.as_bytes()).is_err());
    }

    #[test]
    fn escape_drops_control_chars() {
        assert_eq!(esc("a\u{b}b<\u{0}"), "ab&lt;");
        assert_eq!(esc_attr("\"x\"\n"), "&quot;x&quot;&#10;");
    }

    #[test]
    fn roundtrip_serialize() {
        let d = parse(b"<p:sp a=\"1\"><p:t>x &amp; y</p:t><e/></p:sp>").unwrap();
        assert_eq!(
            d.root.to_xml(),
            "<p:sp a=\"1\"><p:t>x &amp; y</p:t><e/></p:sp>"
        );
    }
}
