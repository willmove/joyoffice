//! XML 1.0 output for generated parts and imported mixed-content trees.

use crate::xml::{El, Node};

const DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

#[derive(Default)]
pub struct A {
    pub v: Vec<(&'static str, String)>,
}

impl A {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn a(mut self, name: &'static str, value: impl ToString) -> Self {
        self.v.push((name, value.to_string()));
        self
    }

    pub fn o(self, name: &'static str, value: Option<impl ToString>) -> Self {
        match value {
            Some(value) => self.a(name, value),
            None => self,
        }
    }

    pub fn b(self, name: &'static str, value: Option<bool>) -> Self {
        self.o(name, value.map(u8::from))
    }

    pub fn t(self, name: &'static str, enabled: bool) -> Self {
        self.o(name, enabled.then_some(1))
    }
}

#[derive(Default)]
pub struct W {
    pub s: String,
}

impl W {
    pub fn new() -> Self {
        Self {
            s: DECLARATION.into(),
        }
    }

    pub fn frag() -> Self {
        Self::default()
    }

    fn tag<'a>(
        &mut self,
        name: &str,
        attrs: impl IntoIterator<Item = (&'a str, &'a str)>,
        end: &str,
    ) {
        self.s.push('<');
        self.s.push_str(name);
        for (name, value) in attrs {
            self.s.push(' ');
            self.s.push_str(name);
            self.s.push_str("=\"");
            escaped(&mut self.s, value, true);
            self.s.push('"');
        }
        self.s.push_str(end);
    }

    pub fn open(&mut self, name: &str, attrs: A) {
        self.tag(
            name,
            attrs.v.iter().map(|(key, value)| (*key, value.as_str())),
            ">",
        );
    }

    pub fn open0(&mut self, name: &str) {
        self.tag(name, [], ">");
    }

    pub fn empty(&mut self, name: &str, attrs: A) {
        self.tag(
            name,
            attrs.v.iter().map(|(key, value)| (*key, value.as_str())),
            "/>",
        );
    }

    pub fn empty0(&mut self, name: &str) {
        self.tag(name, [], "/>");
    }

    pub fn close(&mut self, name: &str) {
        self.s.push_str("</");
        self.s.push_str(name);
        self.s.push('>');
    }

    pub fn val(&mut self, name: &str, value: impl ToString) {
        self.empty(name, A::new().a("val", value));
    }

    pub fn text(&mut self, text: &str) {
        escaped(&mut self.s, text, false);
    }

    pub fn elt(&mut self, name: &str, text: &str) {
        self.open0(name);
        self.text(text);
        self.close(name);
    }

    pub fn raw(&mut self, markup: &str) {
        self.s.push_str(markup);
    }

    pub fn finish(self) -> Vec<u8> {
        self.s.into_bytes()
    }

    pub fn tree(&mut self, root: &El) {
        let mut pending = Vec::new();
        self.start_tree(root, &mut pending);
        while let Some((name, children)) = pending.last_mut() {
            match children.next() {
                Some(Node::El(element)) => self.start_tree(element, &mut pending),
                Some(Node::Text(text)) => self.text(text),
                Some(Node::Markup(markup)) => self.raw(markup),
                None => {
                    self.close(name);
                    pending.pop();
                }
            }
        }
    }

    fn start_tree<'a>(
        &mut self,
        element: &'a El,
        pending: &mut Vec<(&'a str, std::slice::Iter<'a, Node>)>,
    ) {
        let empty = element.children.is_empty();
        self.tag(
            &element.name,
            element
                .attrs
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
            if empty { "/>" } else { ">" },
        );
        if !empty {
            pending.push((&element.name, element.children.iter()));
        }
    }
}

fn escaped(out: &mut String, input: &str, attribute: bool) {
    let mut start = 0;
    for (offset, ch) in input.char_indices() {
        let replacement = match ch {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '\r' => "&#13;",
            '"' if attribute => "&quot;",
            '\n' if attribute => "&#10;",
            '\t' if attribute => "&#9;",
            '\0'..='\x08' | '\x0b'..='\x0c' | '\x0e'..='\x1f' | '\u{fffe}' | '\u{ffff}' => "",
            _ => continue,
        };
        out.push_str(&input[start..offset]);
        out.push_str(replacement);
        start = offset + ch.len_utf8();
    }
    out.push_str(&input[start..]);
}
