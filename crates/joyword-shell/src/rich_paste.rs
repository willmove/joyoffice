//! HTML and RTF paste into plain formatted runs.
//!
//! The subset is hand-written from the public HTML living standard and RTF
//! 1.9.1 control words this slice uses (`b`, `i`, `ul`, `par`, `line`, `tab`).
//! Scripts, styles, objects, pictures and remote resources are dropped and
//! never fetched. Anything outside the subset or the size budget becomes
//! plain text, or is refused when even that cannot be bounded.

use joyword_model::Underline;

/// One mebibyte of clipboard markup. Larger offers are refused.
pub const MAX_RICH_BYTES: usize = 1024 * 1024;
const MAX_DEPTH: usize = 32;
const MAX_CHARS: usize = 100_000;
const MAX_PARAGRAPHS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKind {
    Bullet,
    Ordered,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichParagraph {
    pub runs: Vec<RichRun>,
    pub list: Option<ListKind>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichDocument {
    pub paragraphs: Vec<RichParagraph>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParsedRich {
    Formatted(RichDocument),
    Plain(String),
    Refuse(&'static str),
}

impl RichRun {
    pub fn underline(&self) -> Option<Underline> {
        if self.underline {
            Some(Underline::Single)
        } else {
            Some(Underline::None)
        }
    }
}

/// Every non-empty paragraph is the same list kind.
pub fn uniform_list(document: &RichDocument) -> Option<ListKind> {
    let mut kind = None;
    for paragraph in &document.paragraphs {
        if paragraph.runs.iter().all(|run| run.text.is_empty()) {
            continue;
        }
        match (kind, paragraph.list) {
            (None, Some(next)) => kind = Some(next),
            (Some(current), Some(next)) if current == next => {}
            _ => return None,
        }
    }
    kind
}

pub fn parse_html(input: &str) -> ParsedRich {
    if input.len() > MAX_RICH_BYTES {
        return ParsedRich::Refuse("rich paste exceeds the size budget");
    }
    match walk_html(input) {
        Ok(document) => finish(document),
        Err(Stop::Plain) => ParsedRich::Plain(strip_html(input)),
    }
}

pub fn parse_rtf(input: &str) -> ParsedRich {
    if input.len() > MAX_RICH_BYTES {
        return ParsedRich::Refuse("rich paste exceeds the size budget");
    }
    let trimmed = input.trim_start();
    if !trimmed.starts_with("{\\rtf") {
        return ParsedRich::Refuse("rtf paste is not an rtf document");
    }
    match walk_rtf(trimmed) {
        Ok(document) => finish(document),
        Err(Stop::Plain) => ParsedRich::Plain(strip_rtf(trimmed)),
    }
}

/// Clipboard text that is itself an RTF document or an HTML fragment.
pub fn rich_kind(text: &str) -> Option<RichSource> {
    let trimmed = text.trim_start();
    if trimmed.starts_with("{\\rtf") {
        return Some(RichSource::Rtf);
    }
    if looks_like_html(trimmed) {
        return Some(RichSource::Html);
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RichSource {
    Html,
    Rtf,
}

fn looks_like_html(text: &str) -> bool {
    let head = text.get(..text.len().min(64)).unwrap_or(text);
    let head = head.trim_start().to_ascii_lowercase();
    head.starts_with("<!doctype")
        || head.starts_with("<html")
        || head.starts_with("<body")
        || head.starts_with("<p")
        || head.starts_with("<div")
        || head.starts_with("<ul")
        || head.starts_with("<ol")
        || head.starts_with("<span")
        || head.starts_with("<b")
        || head.starts_with("<i")
        || head.starts_with("<em")
        || head.starts_with("<strong")
        || head.starts_with("<u")
}

fn finish(document: RichDocument) -> ParsedRich {
    let empty = document.paragraphs.iter().all(|paragraph| {
        paragraph.runs.is_empty() || paragraph.runs.iter().all(|run| run.text.is_empty())
    });
    if empty {
        ParsedRich::Plain(String::new())
    } else {
        ParsedRich::Formatted(document)
    }
}

enum Stop {
    Plain,
}

struct Builder {
    paragraphs: Vec<RichParagraph>,
    runs: Vec<RichRun>,
    chars: usize,
    list: Option<ListKind>,
}

impl Builder {
    fn new() -> Self {
        Self {
            paragraphs: Vec::new(),
            runs: Vec::new(),
            chars: 0,
            list: None,
        }
    }

    fn push_text(
        &mut self,
        text: &str,
        bold: bool,
        italic: bool,
        underline: bool,
    ) -> Result<(), Stop> {
        if text.is_empty() {
            return Ok(());
        }
        let added = text.chars().count();
        if self.chars.saturating_add(added) > MAX_CHARS {
            return Err(Stop::Plain);
        }
        self.chars += added;
        if let Some(last) = self.runs.last_mut()
            && last.bold == bold
            && last.italic == italic
            && last.underline == underline
        {
            last.text.push_str(text);
            return Ok(());
        }
        self.runs.push(RichRun {
            text: text.to_owned(),
            bold,
            italic,
            underline,
        });
        Ok(())
    }

    fn break_paragraph(&mut self) -> Result<(), Stop> {
        if self.runs.is_empty() {
            return Ok(());
        }
        if self.paragraphs.len() >= MAX_PARAGRAPHS {
            return Err(Stop::Plain);
        }
        self.paragraphs.push(RichParagraph {
            runs: std::mem::take(&mut self.runs),
            list: self.list,
        });
        Ok(())
    }

    fn finish(mut self) -> Result<RichDocument, Stop> {
        self.break_paragraph()?;
        Ok(RichDocument {
            paragraphs: self.paragraphs,
        })
    }
}

#[derive(Clone, Copy)]
struct Marks {
    bold: bool,
    italic: bool,
    underline: bool,
}

fn walk_html(input: &str) -> Result<RichDocument, Stop> {
    let mut builder = Builder::new();
    let mut marks = Marks {
        bold: false,
        italic: false,
        underline: false,
    };
    let mut stack: Vec<HtmlFrame> = Vec::new();
    let mut index = 0usize;
    while index < input.len() {
        if input[index..].starts_with('<') {
            let Some((tag, next)) = read_tag(input, index) else {
                builder.push_text("<", marks.bold, marks.italic, marks.underline)?;
                index += 1;
                continue;
            };
            index = next;
            apply_tag(&mut builder, &mut marks, &mut stack, tag)?;
            continue;
        }
        let end = input[index..]
            .find('<')
            .map(|offset| index + offset)
            .unwrap_or(input.len());
        if stack.last().is_none_or(|frame| !frame.skip) {
            let decoded = decode_entities(&input[index..end]);
            builder.push_text(&decoded, marks.bold, marks.italic, marks.underline)?;
        }
        index = end;
    }
    builder.finish()
}

struct HtmlFrame {
    name: &'static str,
    skip: bool,
    marks: Marks,
    list: Option<ListKind>,
}

fn apply_tag(
    builder: &mut Builder,
    marks: &mut Marks,
    stack: &mut Vec<HtmlFrame>,
    tag: Tag,
) -> Result<(), Stop> {
    match tag {
        Tag::Comment | Tag::Ignored => Ok(()),
        Tag::Close(name) => {
            close_html(builder, marks, stack, name);
            Ok(())
        }
        Tag::Open { name } => {
            if name.is_empty() || name == "img" {
                return Ok(());
            }
            if stack.len() >= MAX_DEPTH {
                return Err(Stop::Plain);
            }
            if matches!(name, "p" | "div" | "li" | "h1" | "h2" | "h3" | "tr" | "br") {
                builder.break_paragraph()?;
            }
            if name == "br" {
                return Ok(());
            }
            let saved = *marks;
            let saved_list = builder.list;
            if matches!(
                name,
                "script" | "style" | "iframe" | "object" | "embed" | "svg"
            ) {
                stack.push(HtmlFrame {
                    name,
                    skip: true,
                    marks: saved,
                    list: saved_list,
                });
                return Ok(());
            }
            match name {
                "b" | "strong" => marks.bold = true,
                "i" | "em" => marks.italic = true,
                "u" => marks.underline = true,
                "ul" => builder.list = Some(ListKind::Bullet),
                "ol" => builder.list = Some(ListKind::Ordered),
                "li" if builder.list.is_none() => builder.list = Some(ListKind::Bullet),
                _ => {}
            }
            stack.push(HtmlFrame {
                name,
                skip: false,
                marks: saved,
                list: saved_list,
            });
            Ok(())
        }
    }
}

fn close_html(builder: &mut Builder, marks: &mut Marks, stack: &mut Vec<HtmlFrame>, name: &str) {
    if matches!(
        name,
        "p" | "div" | "li" | "h1" | "h2" | "h3" | "tr" | "ul" | "ol"
    ) {
        let _ = builder.break_paragraph();
    }
    let Some(position) = stack.iter().rposition(|frame| same_tag(frame.name, name)) else {
        return;
    };
    let saved = stack[position].marks;
    let list = stack[position].list;
    stack.truncate(position);
    *marks = saved;
    if matches!(name, "ul" | "ol" | "li") {
        builder.list = list;
    }
}

fn same_tag(open: &str, close: &str) -> bool {
    open == close
        || matches!(
            (open, close),
            ("b", "strong") | ("strong", "b") | ("i", "em") | ("em", "i")
        )
}

enum Tag {
    Open { name: &'static str },
    Close(&'static str),
    Comment,
    Ignored,
}

fn read_tag(input: &str, start: usize) -> Option<(Tag, usize)> {
    let bytes = input.as_bytes();
    if bytes.get(start) != Some(&b'<') {
        return None;
    }
    if input[start..].starts_with("<!--") {
        let end = input[start + 4..]
            .find("-->")
            .map(|offset| start + 4 + offset + 3)?;
        return Some((Tag::Comment, end));
    }
    let mut index = start + 1;
    let close = if bytes.get(index) == Some(&b'/') {
        index += 1;
        true
    } else {
        false
    };
    let name_at = index;
    while index < bytes.len() && bytes[index].is_ascii_alphanumeric() {
        index += 1;
    }
    if index == name_at {
        return None;
    }
    let name = &input[name_at..index];
    let mut quote = 0u8;
    while index < bytes.len() {
        let byte = bytes[index];
        if quote != 0 {
            if byte == quote {
                quote = 0;
            }
            index += 1;
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            quote = byte;
            index += 1;
            continue;
        }
        if byte == b'>' {
            index += 1;
            break;
        }
        index += 1;
    }
    if quote != 0 {
        return None;
    }
    let name_end = name.len();
    let mut lower = String::new();
    for ch in name.chars() {
        if lower.len() >= 16 {
            break;
        }
        lower.push(ch.to_ascii_lowercase());
    }
    let _ = name_end;
    // The name slice is the original case. Compare through the lowered copy
    // by returning a leaked... no. Store lowered names only for known tags
    // by matching ascii on the fly. The caller needs a &str that lives as
    // long as input. Known tags are ascii, so use the original slice when it
    // is already lower, otherwise map via a function that doesn't need the
    // lowered string to outlive this function.
    let _ = lower;
    let normalized = normalize_name(name);
    if close {
        Some((Tag::Close(normalized), index))
    } else if normalized.is_empty() {
        Some((Tag::Ignored, index))
    } else {
        Some((Tag::Open { name: normalized }, index))
    }
}

fn normalize_name(name: &str) -> &'static str {
    let mut buf = [0u8; 16];
    let mut len = 0usize;
    for byte in name.bytes() {
        if len == buf.len() {
            return "";
        }
        if !byte.is_ascii_alphanumeric() {
            return "";
        }
        buf[len] = byte.to_ascii_lowercase();
        len += 1;
    }
    match std::str::from_utf8(&buf[..len]).unwrap_or("") {
        "b" => "b",
        "strong" => "strong",
        "i" => "i",
        "em" => "em",
        "u" => "u",
        "p" => "p",
        "div" => "div",
        "br" => "br",
        "ul" => "ul",
        "ol" => "ol",
        "li" => "li",
        "script" => "script",
        "style" => "style",
        "iframe" => "iframe",
        "object" => "object",
        "embed" => "embed",
        "svg" => "svg",
        "img" => "img",
        "h1" => "h1",
        "h2" => "h2",
        "h3" => "h3",
        "tr" => "tr",
        "span" => "span",
        "a" => "a",
        "html" => "html",
        "body" => "body",
        "head" => "head",
        _ => "",
    }
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        if let Some(end) = rest.find(';').filter(|end| *end <= 16) {
            let token = &rest[1..end];
            if let Some(ch) = entity(token) {
                out.push(ch);
                rest = &rest[end + 1..];
                continue;
            }
        }
        out.push('&');
        rest = &rest[1..];
    }
    out.push_str(rest);
    out
}

fn entity(token: &str) -> Option<char> {
    match token {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{00a0}'),
        _ => {
            let digits = token.strip_prefix('#')?;
            let code = if let Some(hex) = digits.strip_prefix(['x', 'X']) {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                digits.parse().ok()?
            };
            char::from_u32(code).filter(|ch| !ch.is_control() || *ch == '\t')
        }
    }
}

fn strip_html(input: &str) -> String {
    let mut out = String::new();
    let mut index = 0usize;
    let mut skip = 0u32;
    while index < input.len() && out.chars().count() < MAX_CHARS {
        if input[index..].starts_with('<')
            && let Some((tag, next)) = read_tag(input, index)
        {
            match tag {
                Tag::Open {
                    name: "script" | "style" | "iframe" | "object" | "embed" | "svg",
                } => {
                    skip = skip.saturating_add(1);
                }
                Tag::Close(name)
                    if skip > 0
                        && matches!(
                            name,
                            "script" | "style" | "iframe" | "object" | "embed" | "svg"
                        ) =>
                {
                    skip = skip.saturating_sub(1);
                }
                Tag::Open {
                    name: "p" | "div" | "br" | "li",
                } if skip == 0 && !out.is_empty() && !out.ends_with('\n') => {
                    out.push('\n');
                }
                _ => {}
            }
            index = next;
            continue;
        }
        let end = input[index..]
            .find('<')
            .map(|offset| index + offset)
            .unwrap_or(input.len());
        if skip == 0 {
            let decoded = decode_entities(&input[index..end]);
            for ch in decoded.chars() {
                if out.chars().count() >= MAX_CHARS {
                    break;
                }
                out.push(ch);
            }
        }
        index = end;
    }
    out
}

#[derive(Clone)]
struct RtfGroup {
    bold: bool,
    italic: bool,
    underline: bool,
    skip: bool,
    field: bool,
    unicode_skip: u8,
}

fn walk_rtf(input: &str) -> Result<RichDocument, Stop> {
    let bytes = input.as_bytes();
    let mut builder = Builder::new();
    let mut stack = vec![RtfGroup {
        bold: false,
        italic: false,
        underline: false,
        skip: false,
        field: false,
        unicode_skip: 1,
    }];
    let mut index = 0usize;
    let mut pending_skip = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'{' {
            if stack.len() >= MAX_DEPTH {
                return Err(Stop::Plain);
            }
            let parent = stack.last().cloned().ok_or(Stop::Plain)?;
            stack.push(parent);
            index += 1;
            continue;
        }
        if byte == b'}' {
            if stack.len() <= 1 {
                break;
            }
            stack.pop();
            index += 1;
            continue;
        }
        if byte == b'\\' {
            let (control, next) = read_control(bytes, index)?;
            index = next;
            let group = stack.last_mut().ok_or(Stop::Plain)?;
            match control {
                Control::Word("b", number) => group.bold = number.unwrap_or(1) != 0,
                Control::Word("i", number) => group.italic = number.unwrap_or(1) != 0,
                Control::Word("ul", number) => group.underline = number.unwrap_or(1) != 0,
                Control::Word("ulnone", _) => group.underline = false,
                Control::Word("par" | "line", _) => builder.break_paragraph()?,
                Control::Word("tab", _) => {
                    if !group.skip {
                        let (bold, italic, underline) = (group.bold, group.italic, group.underline);
                        builder.push_text("\t", bold, italic, underline)?;
                    }
                }
                Control::Word("uc", Some(value)) if (0..=16).contains(&value) => {
                    group.unicode_skip = value as u8;
                }
                Control::Word("u", Some(value)) => {
                    let skip_count = group.unicode_skip;
                    let emit = !group.skip;
                    let (bold, italic, underline) = (group.bold, group.italic, group.underline);
                    let code = if value < 0 { value + 65_536 } else { value };
                    if emit && let Some(ch) = u32::try_from(code).ok().and_then(char::from_u32) {
                        let mut buf = [0u8; 4];
                        builder.push_text(ch.encode_utf8(&mut buf), bold, italic, underline)?;
                    }
                    index = skip_rtf_fallback(bytes, index, skip_count);
                }
                Control::Word(
                    "object" | "pict" | "stylesheet" | "fonttbl" | "info" | "field" | "fldinst",
                    _,
                ) => {
                    group.skip = true;
                    if matches!(control, Control::Word("field", _)) {
                        group.field = true;
                    }
                }
                Control::Word("fldrslt", _) => {
                    if group.field {
                        group.skip = false;
                    }
                    pending_skip = false;
                }
                Control::Word("*", _) => pending_skip = true,
                Control::Word(_, _) if pending_skip => {
                    group.skip = true;
                    pending_skip = false;
                }
                Control::Hex(value) => {
                    if !group.skip
                        && let Some(ch) = char::from_u32(u32::from(value))
                    {
                        let (bold, italic, underline) = (group.bold, group.italic, group.underline);
                        let mut buf = [0u8; 4];
                        builder.push_text(ch.encode_utf8(&mut buf), bold, italic, underline)?;
                    }
                }
                Control::Symbol(ch) => {
                    if !group.skip {
                        let (bold, italic, underline) = (group.bold, group.italic, group.underline);
                        let mut buf = [0u8; 4];
                        builder.push_text(ch.encode_utf8(&mut buf), bold, italic, underline)?;
                    }
                }
                Control::Word(_, _) => {}
            }
            continue;
        }
        if byte == b'\r' || byte == b'\n' {
            index += 1;
            continue;
        }
        let group = stack.last().ok_or(Stop::Plain)?;
        if !group.skip {
            let (bold, italic, underline) = (group.bold, group.italic, group.underline);
            let ch = char::from_u32(u32::from(byte)).ok_or(Stop::Plain)?;
            let mut buf = [0u8; 4];
            builder.push_text(ch.encode_utf8(&mut buf), bold, italic, underline)?;
        }
        index += 1;
    }
    builder.finish()
}

#[derive(Clone, Copy)]
enum Control<'a> {
    Word(&'a str, Option<i32>),
    Hex(u8),
    Symbol(char),
}

fn read_control(bytes: &[u8], start: usize) -> Result<(Control<'_>, usize), Stop> {
    let Some(b'\\') = bytes.get(start) else {
        return Err(Stop::Plain);
    };
    let next = bytes.get(start + 1).copied().ok_or(Stop::Plain)?;
    if next == b'\'' {
        let hex = bytes.get(start + 2..start + 4).ok_or(Stop::Plain)?;
        let text = std::str::from_utf8(hex).map_err(|_| Stop::Plain)?;
        let value = u8::from_str_radix(text, 16).map_err(|_| Stop::Plain)?;
        return Ok((Control::Hex(value), start + 4));
    }
    if !next.is_ascii_alphabetic() && next != b'*' {
        let symbol = match next {
            b'\\' => '\\',
            b'{' => '{',
            b'}' => '}',
            _ => return Ok((Control::Word("", None), start + 2)),
        };
        return Ok((Control::Symbol(symbol), start + 2));
    }
    let mut end = start + 1;
    if bytes.get(end) == Some(&b'*') {
        return Ok((Control::Word("*", None), end + 1));
    }
    while end < bytes.len() && bytes[end].is_ascii_alphabetic() {
        end += 1;
        if end - start > 32 {
            return Err(Stop::Plain);
        }
    }
    let word = std::str::from_utf8(&bytes[start + 1..end]).map_err(|_| Stop::Plain)?;
    let mut number = None;
    if bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'-')
    {
        let num_at = end;
        if bytes.get(end) == Some(&b'-') {
            end += 1;
        }
        let digits = end;
        while end < bytes.len() && bytes[end].is_ascii_digit() && end - digits < 10 {
            end += 1;
        }
        number = std::str::from_utf8(&bytes[num_at..end])
            .ok()
            .and_then(|text| text.parse().ok());
    }
    if bytes.get(end) == Some(&b' ') {
        end += 1;
    }
    Ok((Control::Word(word, number), end))
}

fn skip_rtf_fallback(bytes: &[u8], mut index: usize, count: u8) -> usize {
    let mut left = count;
    while left > 0 && index < bytes.len() {
        let byte = bytes[index];
        if byte == b'\\' || byte == b'{' || byte == b'}' {
            break;
        }
        if byte == b'\r' || byte == b'\n' {
            index += 1;
            continue;
        }
        index += 1;
        left = left.saturating_sub(1);
    }
    index
}

fn strip_rtf(input: &str) -> String {
    match walk_rtf(input) {
        Ok(document) => flatten(&document),
        Err(_) => String::new(),
    }
}

fn flatten(document: &RichDocument) -> String {
    let mut out = String::new();
    for (index, paragraph) in document.paragraphs.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        for run in &paragraph.runs {
            if out.chars().count() >= MAX_CHARS {
                return out;
            }
            out.push_str(&run.text);
        }
    }
    out
}
