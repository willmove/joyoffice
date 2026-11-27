//! Qualified XML trees used by the importers and the preserving writer.

use std::slice::Iter;

pub use crate::xml_read::parse;
pub use crate::xml_write::{A, W};

pub const MAX_DEPTH: usize = 256;
pub const MAX_NODES: usize = 4_000_000;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct El {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    El(El),
    Text(String),
    Markup(String),
}

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub root: El,
    pub(super) before_root: Vec<String>,
    pub(super) after_root: Vec<String>,
}

fn qualified(name: &str) -> (&str, &str) {
    name.split_once(':').unwrap_or(("", name))
}

fn namespace_declaration(name: &str) -> bool {
    name == "xmlns" || qualified(name).0 == "xmlns"
}

impl Doc {
    pub fn ns_decls(&self) -> Vec<(String, String)> {
        self.root
            .attrs
            .iter()
            .filter(|(name, _)| namespace_declaration(name))
            .cloned()
            .collect()
    }

    pub fn into_xml(self) -> Vec<u8> {
        let mut writer = W::frag();
        for markup in self.before_root {
            writer.raw(&markup);
        }
        writer.tree(&self.root);
        for markup in self.after_root {
            writer.raw(&markup);
        }
        writer.finish()
    }
}

impl Node {
    fn element(&self) -> Option<&El> {
        if let Self::El(element) = self {
            Some(element)
        } else {
            None
        }
    }
}

/// Preorder traversal of mixed content using one iterator per ancestor.
fn descendants(root: &El) -> impl Iterator<Item = &Node> {
    let mut pending = vec![root.children.iter()];
    std::iter::from_fn(move || {
        while let Some(level) = pending.last_mut() {
            if let Some(node) = Iter::next(level) {
                if let Node::El(element) = node {
                    pending.push(element.children.iter());
                }
                return Some(node);
            }
            pending.pop();
        }
        None
    })
}

impl El {
    pub fn local(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }

    pub fn prefix(&self) -> &str {
        qualified(&self.name).0
    }

    pub fn is(&self, local: &str) -> bool {
        self.local() == local
    }

    /// Prefer the exact qualified attribute, then accept aliases used by imported parts.
    pub fn attr(&self, requested: &str) -> Option<&str> {
        let (prefix, local) = qualified(requested);
        let mut alias = None;
        for (name, value) in &self.attrs {
            if name == requested {
                return Some(value);
            }
            let (candidate_prefix, _) = qualified(name);
            if alias.is_none()
                && !namespace_declaration(name)
                && (prefix.is_empty() || !candidate_prefix.is_empty())
                && name.rsplit(':').next() == Some(local)
            {
                alias = Some(value.as_str());
            }
        }
        alias
    }

    pub fn has_attr(&self, name: &str) -> bool {
        self.attr(name).is_some()
    }

    pub fn set_attr(&mut self, name: &str, value: &str) {
        for (key, old) in &mut self.attrs {
            if key == name {
                value.clone_into(old);
                return;
            }
        }
        self.attrs.push((name.into(), value.into()));
    }

    pub fn i64(&self, name: &str) -> Option<i64> {
        let value = self.attr(name)?.trim();
        match value.parse() {
            Ok(integer) => Some(integer),
            Err(_) => finite(value).map(|number| number.clamp(-9e18, 9e18) as i64),
        }
    }

    pub fn i32(&self, name: &str) -> Option<i32> {
        Some(
            self.i64(name)?
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        )
    }

    pub fn u32(&self, name: &str) -> Option<u32> {
        Some(self.i64(name)?.clamp(0, i64::from(u32::MAX)) as u32)
    }

    pub fn f64(&self, name: &str) -> Option<f64> {
        finite(self.attr(name)?.trim())
    }

    pub fn bool(&self, name: &str) -> Option<bool> {
        let value = self.attr(name)?.trim();
        if ["1", "true", "on"].contains(&value) {
            Some(true)
        } else {
            ["0", "false", "off"].contains(&value).then_some(false)
        }
    }

    pub fn elements(&self) -> impl Iterator<Item = &El> {
        self.children.iter().filter_map(Node::element)
    }

    pub fn elements_mut(&mut self) -> impl Iterator<Item = &mut El> {
        self.children.iter_mut().filter_map(|node| {
            if let Node::El(element) = node {
                Some(element)
            } else {
                None
            }
        })
    }

    pub fn child(&self, name: &str) -> Option<&El> {
        self.elements().find(|element| element.is(name))
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a El> + 'a {
        self.elements().filter(move |element| element.is(name))
    }

    pub fn path(&self, names: &[&str]) -> Option<&El> {
        names
            .iter()
            .try_fold(self, |parent, name| parent.child(name))
    }

    pub fn find(&self, name: &str) -> Option<&El> {
        descendants(self)
            .filter_map(Node::element)
            .find(|element| element.is(name))
    }

    pub fn find_all<'a>(&'a self, name: &str, out: &mut Vec<&'a El>) {
        out.extend(
            descendants(self)
                .filter_map(Node::element)
                .filter(|element| element.is(name)),
        );
    }

    pub fn text(&self) -> String {
        descendants(self)
            .filter_map(|node| {
                if let Node::Text(text) = node {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn child_val(&self, name: &str) -> Option<&str> {
        self.child(name)?.attr("val")
    }

    pub fn to_xml(&self) -> String {
        let mut writer = W::frag();
        writer.tree(self);
        writer.s
    }
}

fn finite(value: &str) -> Option<f64> {
    value.parse().ok().filter(|number: &f64| number.is_finite())
}

#[cfg(test)]
#[path = "xml_tests.rs"]
mod tests;
