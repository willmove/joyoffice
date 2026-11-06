use super::*;
use html5ever::tokenizer::{
    BufferQueue, EndTag, StartTag, Tag, Token, TokenSink, TokenSinkResult, Tokenizer,
    states::RawKind,
};
use joydeck_model::text::{Align, Strike};

/// Read plain HTML or a Windows CF_HTML payload. Offsets count UTF-8 bytes.
pub fn import(bytes: &[u8]) -> Option<TextBody> {
    import_with_policy(bytes, false)
}

/// Refuse table TSV fallback when the selected HTML explicitly hides content.
/// `Some(empty)` consumes the paste without changing the target table.
pub fn import_for_table(bytes: &[u8]) -> Option<TextBody> {
    import_with_policy(bytes, true)
}

fn import_with_policy(bytes: &[u8], table: bool) -> Option<TextBody> {
    if bytes.len() > MAX_INPUT {
        return None;
    }
    let html = std::str::from_utf8(bytes)
        .ok()?
        .trim_start_matches('\u{feff}');
    let html = fragment(html);
    let mut input = BufferQueue::default();
    input.push_back(html.into());
    let mut tokenizer = Tokenizer::new(Sink::default(), Default::default());
    let _ = tokenizer.feed(&mut input);
    tokenizer.end();
    if tokenizer.sink.saw_hidden && (table || tokenizer.sink.out.overflow) {
        // An output/depth limit must not undo a recognized hiding decision.
        return Some(TextBody::default());
    }
    if tokenizer.sink.out.overflow {
        return None;
    }
    tokenizer.sink.out.finish()
}

fn fragment(html: &str) -> &str {
    if !html.starts_with("Version:") {
        return html;
    }
    let header = html.split('<').next().unwrap_or("").lines();
    let offset = |key: &str| {
        header
            .clone()
            .find_map(|line| line.strip_prefix(key))?
            .trim()
            .parse::<usize>()
            .ok()
    };
    if let Some((start, end)) = offset("StartFragment:").zip(offset("EndFragment:"))
        && let Some(part) = html.get(start..end)
    {
        return part;
    }
    if let Some((_, after)) = html.split_once("<!--StartFragment-->")
        && let Some((part, _)) = after.split_once("<!--EndFragment-->")
    {
        return part;
    }
    html.find('<')
        .and_then(|start| html.get(start..))
        .unwrap_or("")
}

#[derive(Default)]
struct Sink {
    saw_hidden: bool,
    out: Fragment,
    stack: Vec<Frame>,
    pending_space: bool,
}

#[derive(Clone, Default)]
struct Frame {
    name: String,
    props: RunProps,
    align: Option<Align>,
    hidden: bool,
    explicit_hidden: bool,
    pre: bool,
}

fn block(tag: &str) -> bool {
    matches!(
        tag,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "div"
            | "footer"
            | "header"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "li"
            | "p"
            | "pre"
            | "section"
            | "tr"
    )
}

fn void(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

impl Sink {
    fn block_boundary(&mut self) {
        if !self.out.current.runs.is_empty() {
            self.out.paragraph();
        }
        self.pending_space = false;
    }

    fn start(&mut self, tag: Tag) -> TokenSinkResult<()> {
        let name = tag.name.as_ref();
        let mut frame = self.stack.last().cloned().unwrap_or_default();
        let attr = |key: &str| {
            tag.attrs
                .iter()
                .find(|a| a.name.local.as_ref() == key)
                .map(|a| a.value.as_ref())
        };
        frame.hidden |= matches!(
            name,
            "head"
                | "script"
                | "style"
                | "title"
                | "template"
                | "object"
                | "iframe"
                | "svg"
                | "math"
        ) || attr("hidden").is_some();
        frame.explicit_hidden |= attr("hidden").is_some();
        frame.pre |= name == "pre";
        match name {
            "b" | "strong" => frame.props.bold = Some(true),
            "i" | "em" => frame.props.italic = Some(true),
            "u" => frame.props.underline = Some("sng".into()),
            "s" | "strike" | "del" => frame.props.strike = Some(Strike::Single),
            "sup" => frame.props.baseline = Some(0.30),
            "sub" => frame.props.baseline = Some(-0.25),
            "a" => frame.props.link = attr("href").and_then(url_link),
            "font" => {
                frame.props.font = attr("face").and_then(font).or(frame.props.font);
                if let Some(c) = attr("color").and_then(color) {
                    frame.props.fill = Some(color_fill(c));
                }
            }
            _ => {}
        }
        if let Some(value) = attr("align").and_then(align) {
            frame.align = Some(value);
        }
        for (key, val) in attr("style")
            .unwrap_or("")
            .split(';')
            .filter_map(|d| d.split_once(':'))
        {
            style(&mut frame, key.trim(), val.trim());
        }
        self.saw_hidden |= frame.explicit_hidden;
        if !frame.hidden {
            if block(name) {
                self.block_boundary();
            } else if name == "br" {
                self.out.soft_break(&frame.props);
                self.pending_space = false;
            } else if name == "img"
                && let Some(alt) = attr("alt")
            {
                self.text(alt, &frame);
            }
        }
        if !void(name) {
            if self.stack.len() >= MAX_DEPTH {
                self.out.overflow = true;
                return TokenSinkResult::Continue;
            }
            frame.name = name.into();
            self.stack.push(frame);
        }
        match name {
            "script" => TokenSinkResult::RawData(RawKind::ScriptData),
            "style" | "iframe" | "xmp" => TokenSinkResult::RawData(RawKind::Rawtext),
            "title" | "textarea" => TokenSinkResult::RawData(RawKind::Rcdata),
            _ => TokenSinkResult::Continue,
        }
    }

    fn end_tag(&mut self, name: &str) {
        if let Some(index) = self.stack.iter().rposition(|frame| frame.name == name) {
            if block(name) && !self.stack[index].hidden {
                self.block_boundary();
            }
            self.stack.truncate(index);
        }
    }

    fn text(&mut self, text: &str, frame: &Frame) {
        if frame.hidden {
            return;
        }
        if self.out.current.runs.is_empty() {
            self.out.current.props.align = frame.align;
        }
        if frame.pre {
            for (i, part) in text.split('\n').enumerate() {
                if i > 0 {
                    self.out.soft_break(&frame.props);
                }
                self.out.text(part, &frame.props);
            }
            return;
        }
        let mut content = String::new();
        for c in text.chars() {
            if matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}') {
                self.pending_space = true;
            } else {
                if self.pending_space && (!content.is_empty() || !self.out.current.runs.is_empty())
                {
                    content.push(' ');
                }
                self.pending_space = false;
                content.push(c);
            }
        }
        self.out.text(&content, &frame.props);
    }
}

impl TokenSink for Sink {
    type Handle = ();

    fn process_token(&mut self, token: Token, _: u64) -> TokenSinkResult<()> {
        if self.out.overflow {
            return TokenSinkResult::Continue;
        }
        match token {
            Token::TagToken(tag) if tag.kind == StartTag => return self.start(tag),
            Token::TagToken(tag) if tag.kind == EndTag => self.end_tag(tag.name.as_ref()),
            Token::CharacterTokens(text) => {
                let frame = self.stack.last().cloned().unwrap_or_default();
                self.text(&text, &frame);
            }
            _ => {}
        }
        TokenSinkResult::Continue
    }
}

fn align(value: &str) -> Option<Align> {
    match value {
        "left" | "start" => Some(Align::Left),
        "right" | "end" => Some(Align::Right),
        "center" => Some(Align::Center),
        "justify" => Some(Align::Justify),
        _ => None,
    }
}

fn font(value: &str) -> Option<String> {
    let name = value.split(',').next()?.trim().trim_matches(['\'', '"']);
    (!name.is_empty() && name.len() <= 128 && !name.chars().any(char::is_control))
        .then(|| name.to_owned())
}

fn color(value: &str) -> Option<Rgba> {
    if let Some(hex) = value.strip_prefix('#') {
        let rgb = u32::from_str_radix(hex, 16).ok()?;
        return match hex.len() {
            3 => Some(Rgba::rgb(
                ((rgb >> 8) as u8) * 17,
                ((rgb >> 4) as u8 & 15) * 17,
                (rgb as u8 & 15) * 17,
            )),
            6 => Some(Rgba::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)),
            _ => None,
        };
    }
    if let Some(rgb) = value.strip_prefix("rgb(").and_then(|v| v.strip_suffix(')')) {
        let mut channels = rgb.split(',').map(|c| c.trim().parse::<u8>().ok());
        let (r, g, b) = (channels.next()??, channels.next()??, channels.next()??);
        return channels.next().is_none().then(|| Rgba::rgb(r, g, b));
    }
    match value {
        "black" => Some(Rgba::rgb(0, 0, 0)),
        "white" => Some(Rgba::rgb(255, 255, 255)),
        "red" => Some(Rgba::rgb(255, 0, 0)),
        "green" => Some(Rgba::rgb(0, 128, 0)),
        "blue" => Some(Rgba::rgb(0, 0, 255)),
        "yellow" => Some(Rgba::rgb(255, 255, 0)),
        _ => None,
    }
}

fn style(frame: &mut Frame, key: &str, value: &str) {
    let key = key.to_ascii_lowercase();
    let val = value.to_ascii_lowercase();
    match key.as_str() {
        "display" if val == "none" => {
            frame.hidden = true;
            frame.explicit_hidden = true;
        }
        "visibility" if val == "hidden" => {
            frame.hidden = true;
            frame.explicit_hidden = true;
        }
        "white-space" if matches!(val.as_str(), "pre" | "pre-wrap") => frame.pre = true,
        "font-family" => frame.props.font = font(value).or(frame.props.font.take()),
        "font-size" => {
            let points = val
                .strip_suffix("pt")
                .and_then(|v| v.trim().parse::<f64>().ok())
                .or_else(|| {
                    val.strip_suffix("px")
                        .and_then(|v| v.trim().parse::<f64>().ok())
                        .map(|v| v * 0.75)
                });
            if let Some(size) = points.filter(|p| (1.0..=400.0).contains(p)) {
                frame.props.size = Some(size);
            }
        }
        "font-weight" => {
            frame.props.bold = match val.as_str() {
                "normal" | "lighter" => Some(false),
                "bold" | "bolder" => Some(true),
                v => v.parse::<u16>().ok().map(|weight| weight >= 600),
            }
            .or(frame.props.bold);
        }
        "font-style" if val == "normal" => frame.props.italic = Some(false),
        "font-style" if matches!(val.as_str(), "italic" | "oblique") => {
            frame.props.italic = Some(true)
        }
        "text-decoration" | "text-decoration-line" => {
            if val == "none" || val.split_whitespace().any(|d| d == "underline") {
                frame.props.underline = Some(if val == "none" { "none" } else { "sng" }.into());
            }
            if val == "none" || val.split_whitespace().any(|d| d == "line-through") {
                frame.props.strike = Some(if val == "none" {
                    Strike::None
                } else {
                    Strike::Single
                });
            }
        }
        "color" => {
            if let Some(c) = color(&val) {
                frame.props.fill = Some(color_fill(c));
            }
        }
        "background-color" => {
            if let Some(c) = color(&val) {
                frame.props.highlight = Some(ColorRef::rgb(c));
            }
        }
        "text-align" => frame.align = align(&val).or(frame.align),
        "vertical-align" if val == "super" => frame.props.baseline = Some(0.30),
        "vertical-align" if val == "sub" => frame.props.baseline = Some(-0.25),
        _ => {}
    }
}
