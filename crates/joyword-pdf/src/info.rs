//! Document title and heading outlines for one already-paginated scene.
//!
//! The title is the Dublin Core `title` in an OPC core-properties part
//! (ECMA-376 Part 2, Core Properties; Dublin Core `http://purl.org/dc/elements/1.1/`).
//! Outline entries come only from paragraph styles whose canonical name is
//! `heading 1` … `heading 9` (ECMA-376 Part 1 §17.7.4), including a style
//! whose `basedOn` chain reaches one of those names. The level is the
//! resolved `outlineLvl` when it is 0..=8 (§17.3.1.20, zero means level 1);
//! otherwise it is the heading number minus one. A heading that this
//! snapshot did not place is omitted rather than given a guessed page.

use joyword_layout::{LayoutSnapshot, to_points};
use joyword_model::{Block, BuiltinStyle, Document, Paragraph, StyleId};
use krilla::geom::Point;
use krilla::outline::{Outline, OutlineNode};

/// One bookmark. `level` is zero-based. `page` indexes the scene. `top` is
/// points down from the top of that page, matching the scene's y axis.
#[derive(Clone, Debug, PartialEq)]
pub struct OutlineEntry {
    pub title: String,
    pub level: u8,
    pub page: usize,
    pub top: f32,
}

/// Optional information attached to a scene PDF. The producer is always the
/// fixed [`crate::PRODUCER`] and is not stored here.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PdfExport {
    pub title: Option<String>,
    pub outline: Vec<OutlineEntry>,
}

const DC_NS: &str = "http://purl.org/dc/elements/1.1/";
const MAX_CORE_BYTES: usize = 64 * 1024;
const MAX_TITLE_CHARS: usize = 512;
const MAX_OUTLINE: usize = 512;
const MAX_BOOKMARK_CHARS: usize = 200;

/// `dc:title` text from core-properties XML, or `None` when the part is
/// absent, too large, not UTF-8, or the title is not a single text node.
pub fn core_title(xml: &[u8]) -> Option<String> {
    if xml.is_empty() || xml.len() > MAX_CORE_BYTES {
        return None;
    }
    if xml
        .windows(9)
        .any(|window| window.eq_ignore_ascii_case(b"<!DOCTYPE"))
    {
        return None;
    }
    let text = std::str::from_utf8(xml).ok()?;
    let raw = first_dc_title(text)?;
    let decoded = decode_xml_text(&raw)?;
    let cleaned = clean_title(&decoded);
    (!cleaned.is_empty()).then_some(cleaned)
}

/// Heading bookmarks whose first placed line is in `snapshot`.
pub fn outline_from_headings(document: &Document, snapshot: &LayoutSnapshot) -> Vec<OutlineEntry> {
    let mut paragraphs = Vec::new();
    collect_paragraphs(document.body().blocks.iter(), &mut paragraphs, 0);
    let mut entries = Vec::new();
    for paragraph in paragraphs {
        if entries.len() >= MAX_OUTLINE {
            break;
        }
        let Some(level) = heading_level(document, paragraph) else {
            continue;
        };
        let Some(line) = snapshot
            .lines()
            .iter()
            .find(|line| line.paragraph == paragraph.id())
        else {
            continue;
        };
        if line.page >= snapshot.pages().len() {
            continue;
        }
        let title = clean_title(&paragraph.plain_text(|_| String::new()));
        if title.is_empty() {
            continue;
        }
        let title = truncate_chars(&title, MAX_BOOKMARK_CHARS);
        let top = to_points(line.top) as f32;
        if !top.is_finite() || top < 0.0 {
            continue;
        }
        entries.push(OutlineEntry {
            title,
            level,
            page: line.page,
            top,
        });
    }
    entries
}

pub(crate) fn krilla_outline(entries: &[OutlineEntry], pages: usize) -> Option<Outline> {
    let tree = nest(entries, pages);
    if tree.is_empty() {
        return None;
    }
    let mut outline = Outline::new();
    for node in tree {
        outline.push_child(to_krilla(&node));
    }
    Some(outline)
}

struct Built {
    title: String,
    page: usize,
    top: f32,
    level: u8,
    children: Vec<Built>,
}

fn nest(entries: &[OutlineEntry], pages: usize) -> Vec<Built> {
    let mut roots = Vec::new();
    let mut stack: Vec<Built> = Vec::new();
    for entry in entries {
        if entry.page >= pages || entry.level > 8 || entry.title.is_empty() {
            continue;
        }
        let node = Built {
            title: entry.title.clone(),
            page: entry.page,
            top: entry.top,
            level: entry.level,
            children: Vec::new(),
        };
        while stack
            .last()
            .is_some_and(|parent| parent.level >= node.level)
        {
            attach(&mut roots, &mut stack);
        }
        stack.push(node);
    }
    while !stack.is_empty() {
        attach(&mut roots, &mut stack);
    }
    roots
}

fn attach(roots: &mut Vec<Built>, stack: &mut Vec<Built>) {
    let Some(finished) = stack.pop() else {
        return;
    };
    if let Some(parent) = stack.last_mut() {
        parent.children.push(finished);
    } else {
        roots.push(finished);
    }
}

fn to_krilla(node: &Built) -> OutlineNode {
    let destination =
        krilla::destination::XyzDestination::new(node.page, Point::from_xy(0.0, node.top));
    let mut outline = OutlineNode::new(node.title.clone(), destination).with_open(true);
    for child in &node.children {
        outline.push_child(to_krilla(child));
    }
    outline
}

fn collect_paragraphs<'a>(
    blocks: impl Iterator<Item = &'a Block>,
    out: &mut Vec<&'a Paragraph>,
    depth: usize,
) {
    if depth > 8 {
        return;
    }
    for block in blocks {
        match block {
            Block::Paragraph(paragraph) => out.push(paragraph),
            Block::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        collect_paragraphs(cell.blocks.iter(), out, depth + 1);
                    }
                }
            }
            Block::Sdt(sdt) => collect_paragraphs(sdt.blocks.iter(), out, depth + 1),
            Block::Opaque(_) => {}
        }
    }
}

fn heading_level(document: &Document, paragraph: &Paragraph) -> Option<u8> {
    let number = heading_number(document, paragraph.style.as_ref())?;
    let resolved = document
        .styles()
        .resolve_paragraph(paragraph.style.as_ref(), &paragraph.props);
    Some(
        resolved
            .props
            .outline_level
            .filter(|level| *level <= 8)
            .unwrap_or(number.saturating_sub(1)),
    )
}

fn heading_number(document: &Document, style: Option<&StyleId>) -> Option<u8> {
    let mut current = style.cloned();
    let mut seen = 0usize;
    while let Some(id) = current {
        if seen > 64 {
            return None;
        }
        seen += 1;
        let style = document.styles().get(&id)?;
        if let Some(BuiltinStyle::Heading(level)) = style.builtin()
            && (1..=9).contains(&level)
        {
            return Some(level);
        }
        current = style.based_on.clone();
    }
    None
}

fn clean_title(text: &str) -> String {
    let mut out = String::new();
    let mut space = false;
    for ch in text.chars() {
        if out.chars().count() >= MAX_TITLE_CHARS {
            break;
        }
        if ch.is_control() {
            continue;
        }
        if ch.is_whitespace() {
            if !out.is_empty() {
                space = true;
            }
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(ch);
    }
    out
}

fn truncate_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

struct XmlCursor<'a> {
    text: &'a str,
    index: usize,
}

impl<'a> XmlCursor<'a> {
    fn rest(&self) -> &'a str {
        &self.text[self.index..]
    }

    fn bump(&mut self, bytes: usize) {
        self.index += bytes;
    }
}

fn first_dc_title(text: &str) -> Option<String> {
    let mut cursor = XmlCursor { text, index: 0 };
    let mut stack: Vec<Vec<(String, String)>> = Vec::new();
    while cursor.index < text.len() {
        let start = cursor.rest().find('<')?;
        cursor.bump(start);
        if cursor.rest().starts_with("<?") {
            let end = cursor.rest().find("?>")?;
            cursor.bump(end + 2);
            continue;
        }
        if cursor.rest().starts_with("<!--") {
            let end = cursor.rest().find("-->")?;
            cursor.bump(end + 3);
            continue;
        }
        if cursor.rest().starts_with("</") {
            stack.pop();
            let end = cursor.rest().find('>')?;
            cursor.bump(end + 1);
            continue;
        }
        let started = parse_start(cursor.rest())?;
        let name = started.name;
        let attrs = started.attrs;
        let empty = started.empty;
        let consumed = started.consumed;
        cursor.bump(consumed);
        let bindings = bindings_from(&attrs);
        let ns = element_namespace(&stack, &bindings, &name);
        let local = local_name(&name);
        if ns.as_deref() == Some(DC_NS) && local == "title" {
            if empty {
                return None;
            }
            return simple_text(cursor.rest());
        }
        if !empty {
            let mut next = stack.last().cloned().unwrap_or_default();
            next.extend(bindings);
            stack.push(next);
        }
    }
    None
}

fn simple_text(rest: &str) -> Option<String> {
    if rest.contains('<') && rest.find('<')? < rest.find("</").unwrap_or(usize::MAX) {
        let child = rest.find('<')?;
        let end = rest.find("</")?;
        if child < end {
            return None;
        }
    }
    let end = rest.find("</")?;
    Some(rest[..end].to_string())
}

struct StartTag {
    name: String,
    attrs: Vec<(String, String)>,
    empty: bool,
    consumed: usize,
}

fn parse_start(text: &str) -> Option<StartTag> {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'<') {
        return None;
    }
    let mut index = 1;
    while index < bytes.len() && is_name_char(bytes[index]) {
        index += 1;
    }
    if index == 1 {
        return None;
    }
    let name = text[1..index].to_string();
    let mut attrs = Vec::new();
    loop {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            return None;
        }
        if bytes[index] == b'/' {
            index += 1;
            while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            if bytes.get(index) != Some(&b'>') {
                return None;
            }
            return Some(StartTag {
                name,
                attrs,
                empty: true,
                consumed: index + 1,
            });
        }
        if bytes[index] == b'>' {
            return Some(StartTag {
                name,
                attrs,
                empty: false,
                consumed: index + 1,
            });
        }
        let name_at = index;
        while index < bytes.len() && is_name_char(bytes[index]) {
            index += 1;
        }
        if index == name_at {
            return None;
        }
        let attr = text[name_at..index].to_string();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) != Some(&b'=') {
            return None;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let quote = *bytes.get(index)?;
        if quote != b'"' && quote != b'\'' {
            return None;
        }
        index += 1;
        let value_at = index;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if index >= bytes.len() {
            return None;
        }
        attrs.push((attr, text[value_at..index].to_string()));
        index += 1;
    }
}

fn is_name_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-' | b'.')
}

fn bindings_from(attrs: &[(String, String)]) -> Vec<(String, String)> {
    let mut bindings = Vec::new();
    for (name, value) in attrs {
        if name == "xmlns" {
            bindings.push((String::new(), value.clone()));
        } else if let Some(prefix) = name.strip_prefix("xmlns:") {
            bindings.push((prefix.to_string(), value.clone()));
        }
    }
    bindings
}

fn element_namespace(
    stack: &[Vec<(String, String)>],
    own: &[(String, String)],
    name: &str,
) -> Option<String> {
    let (prefix, _) = split_name(name);
    own.iter()
        .rev()
        .chain(stack.iter().rev().flatten().rev())
        .find(|(bound, _)| bound == prefix)
        .map(|(_, uri)| uri.clone())
}

fn local_name(name: &str) -> &str {
    split_name(name).1
}

fn split_name(name: &str) -> (&str, &str) {
    match name.split_once(':') {
        Some((prefix, local)) => (prefix, local),
        None => ("", name),
    }
}

fn decode_xml_text(text: &str) -> Option<String> {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at + 1..];
        let end = rest.find(';')?;
        let token = &rest[..end];
        rest = &rest[end + 1..];
        let ch = if token == "lt" {
            '<'
        } else if token == "gt" {
            '>'
        } else if token == "amp" {
            '&'
        } else if token == "quot" {
            '"'
        } else if token == "apos" {
            '\''
        } else {
            let digits = token.strip_prefix('#')?;
            let code = if let Some(hex) = digits.strip_prefix(['x', 'X']) {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                digits.parse().ok()?
            };
            char::from_u32(code).filter(|ch| *ch != '\0')?
        };
        out.push(ch);
    }
    out.push_str(rest);
    Some(out)
}
