use super::*;
use encoding_rs::{Encoding, WINDOWS_1252};
use joydeck_model::text::{Align, Strike};
use std::collections::BTreeMap;

/// Import an RTF clipboard fragment without loading embedded objects or local resources.
pub fn import(bytes: &[u8]) -> Option<TextBody> {
    import_for_paste(bytes).filter(|body| !body.is_empty())
}

/// Like `import`, but an explicitly hidden, otherwise empty fragment returns
/// `Some(empty)`. Paste callers must consume it without editing or falling back
/// to the provider's plain text, which may contain the hidden characters.
pub fn import_for_paste(bytes: &[u8]) -> Option<TextBody> {
    import_with_policy(bytes, false)
}

/// Consume hidden RTF without permitting a table TSV fallback. Empty means
/// refuse the paste: the visible fragment cannot preserve arbitrary table cells.
pub fn import_for_table(bytes: &[u8]) -> Option<TextBody> {
    import_with_policy(bytes, true)
}

fn import_with_policy(bytes: &[u8], table: bool) -> Option<TextBody> {
    let input = bytes.trim_ascii_start();
    if bytes.len() > MAX_INPUT || !crate::rtf::sniff_rtf(input) {
        return None;
    }
    let mut reader = Reader::default();
    let result = reader.read(input);
    if table && reader.saw_hidden {
        // Once hiding is recognized, a later syntax/budget error must not
        // re-enable the provider's potentially unfiltered TSV.
        Some(TextBody::default())
    } else {
        result
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Destination {
    #[default]
    Text,
    FontTable,
    ColorTable,
    Instruction,
    Skip,
}

#[derive(Clone, Default)]
struct State {
    dest: Destination,
    props: RunProps,
    font: Option<i32>,
    align: Option<Align>,
    uc: usize,
    hidden: bool,
    starred: bool,
    first: bool,
}

struct Reader {
    saw_hidden: bool,
    stack: Vec<State>,
    state: State,
    out: Fragment,
    encoding: &'static Encoding,
    bytes: Vec<u8>,
    high_surrogate: Option<u16>,
    fallback: usize,
    fonts: BTreeMap<i32, String>,
    font_name: String,
    default_font: Option<i32>,
    colors: Vec<Option<Rgba>>,
    color: Option<[u8; 3]>,
    instruction: String,
}

impl Default for Reader {
    fn default() -> Self {
        Self {
            saw_hidden: false,
            stack: Vec::new(),
            state: State {
                uc: 1,
                ..State::default()
            },
            out: Fragment::default(),
            encoding: WINDOWS_1252,
            bytes: Vec::new(),
            high_surrogate: None,
            fallback: 0,
            fonts: BTreeMap::new(),
            font_name: String::new(),
            default_font: None,
            colors: Vec::new(),
            color: None,
            instruction: String::new(),
        }
    }
}

impl Reader {
    fn read(&mut self, mut input: &[u8]) -> Option<TextBody> {
        while let Some((&byte, tail)) = input.split_first() {
            input = tail;
            match byte {
                b'{' => {
                    self.flush_bytes();
                    self.flush_high();
                    if self.stack.len() == MAX_DEPTH {
                        return None;
                    }
                    self.stack.push(self.state.clone());
                    self.state.starred = false;
                    self.state.first = true;
                    self.fallback = 0;
                }
                b'}' => {
                    self.flush_bytes();
                    self.flush_high();
                    let previous = self.stack.pop()?;
                    let instruction = self.state.dest == Destination::Instruction
                        && previous.dest != Destination::Instruction;
                    self.state = previous;
                    if instruction {
                        self.state.props.link = instruction_link(&self.instruction);
                        self.instruction.clear();
                    }
                    self.fallback = 0;
                    if self.stack.is_empty() {
                        if self.out.overflow {
                            return None;
                        }
                        return std::mem::take(&mut self.out)
                            .finish()
                            .or_else(|| self.saw_hidden.then(TextBody::default));
                    }
                }
                b'\\' => {
                    let (&control, remaining) = input.split_first()?;
                    if control.is_ascii_alphabetic() {
                        let (name, arg, rest) = word(input)?;
                        input = rest;
                        if name == "bin" {
                            self.flush_bytes();
                            self.flush_high();
                            let count = usize::try_from(arg?).ok()?;
                            input = input.get(count..)?;
                            self.state.first = false;
                        } else {
                            self.control(name, arg);
                        }
                    } else if control == b'\'' {
                        let hex = std::str::from_utf8(remaining.get(..2)?).ok()?;
                        self.byte(u8::from_str_radix(hex, 16).ok()?);
                        input = remaining.get(2..)?;
                    } else {
                        self.symbol(control);
                        input = remaining;
                    }
                }
                b'\r' | b'\n' => {}
                _ => self.byte(byte),
            }
            if self.out.overflow {
                return None;
            }
        }
        None
    }

    fn byte(&mut self, byte: u8) {
        if self.fallback > 0 {
            self.fallback -= 1;
        } else if self.state.dest != Destination::Skip && byte != 0 {
            self.flush_high();
            self.bytes.push(byte);
        }
        self.state.first = false;
    }

    fn flush_bytes(&mut self) {
        if !self.bytes.is_empty() {
            let bytes = std::mem::take(&mut self.bytes);
            let (value, _) = self.encoding.decode_without_bom_handling(&bytes);
            self.text(&value);
        }
    }

    fn flush_high(&mut self) {
        if self.high_surrogate.take().is_some() {
            self.text("�");
        }
    }

    fn text(&mut self, value: &str) {
        match self.state.dest {
            Destination::Text => {
                if self.state.hidden {
                    return;
                }
                let mut props = self.state.props.clone();
                if let Some(font) = self
                    .state
                    .font
                    .or(self.default_font)
                    .and_then(|i| self.fonts.get(&i))
                {
                    props.font = Some(font.clone());
                }
                self.out.current.props.align = self.state.align;
                self.out.text(value, &props);
            }
            Destination::Instruction
                if self.instruction.len().saturating_add(value.len()) <= 2048 =>
            {
                self.instruction.push_str(value);
            }
            Destination::Instruction => {
                // A sentinel keeps a truncated field instruction from becoming a valid URL.
                self.instruction.clear();
                self.state.dest = Destination::Skip;
            }
            Destination::FontTable => {
                for (index, name) in value.split(';').enumerate() {
                    if index > 0 {
                        let name = self.font_name.trim();
                        if let Some(id) = self.state.font
                            && !name.is_empty()
                            && name.len() <= 128
                            && !name.chars().any(char::is_control)
                            && self.fonts.len() < 256
                        {
                            self.fonts.insert(id, name.into());
                        }
                        self.font_name.clear();
                    }
                    if self.font_name.len().saturating_add(name.len()) <= 128 {
                        self.font_name.push_str(name);
                    }
                }
            }
            Destination::ColorTable => {
                for _ in value.bytes().filter(|&b| b == b';') {
                    if self.colors.len() < 256 {
                        self.colors
                            .push(self.color.take().map(|[r, g, b]| Rgba::rgb(r, g, b)));
                    }
                }
            }
            Destination::Skip => {}
        }
    }

    fn symbol(&mut self, c: u8) {
        if self.fallback > 0 {
            self.fallback -= 1;
            return;
        }
        if c == b'*' && self.state.first {
            self.state.starred = true;
            return;
        }
        match c {
            b'\\' | b'{' | b'}' => self.byte(c),
            b'~' | b'_' | b'-' => {
                self.flush_bytes();
                self.flush_high();
                self.text(match c {
                    b'~' => "\u{a0}",
                    b'_' => "\u{2011}",
                    _ => "\u{ad}",
                });
            }
            _ => {}
        }
        self.state.first = false;
    }

    fn control(&mut self, name: &str, arg: Option<i32>) {
        if self.fallback > 0 {
            self.fallback -= 1;
            return;
        }
        self.flush_bytes();
        if name != "u" {
            self.flush_high();
        }
        if self.state.dest == Destination::Skip {
            return;
        }
        if self.state.first {
            self.state.first = false;
            self.state.dest = match name {
                "fonttbl" => Destination::FontTable,
                "colortbl" => Destination::ColorTable,
                "fldinst" if self.state.dest == Destination::Text => {
                    self.instruction.clear();
                    Destination::Instruction
                }
                "pict" | "object" | "stylesheet" | "info" | "header" | "footer" | "footnote"
                | "listtable" | "listoverridetable" | "generator" | "listtext" | "pntext"
                | "themedata" | "datastore" | "xmlnstbl" | "shp" => Destination::Skip,
                _ if self.state.starred => Destination::Skip,
                _ => self.state.dest,
            };
        }
        let number = arg.unwrap_or(0);
        match self.state.dest {
            Destination::FontTable if name == "f" => {
                self.state.font = arg;
                self.font_name.clear();
            }
            Destination::ColorTable => {
                let index = match name {
                    "red" => 0,
                    "green" => 1,
                    "blue" => 2,
                    _ => return,
                };
                self.color.get_or_insert([0; 3])[index] = number.clamp(0, 255) as u8;
            }
            Destination::Text => self.control_text(name, arg),
            Destination::Instruction if name == "u" => self.unicode(number),
            _ => {}
        }
    }

    fn control_text(&mut self, name: &str, arg: Option<i32>) {
        let number = arg.unwrap_or(0);
        let active = arg != Some(0);
        match name {
            "ansicpg" => self.encoding = codepage(number).unwrap_or(WINDOWS_1252),
            "deff" => self.default_font = arg,
            "f" => self.state.font = arg,
            "fs" if (2..=800).contains(&number) => {
                self.state.props.size = Some(f64::from(number) / 2.0)
            }
            "v" => {
                self.state.hidden = active;
                self.saw_hidden |= active;
            }
            "b" => self.state.props.bold = Some(active),
            "i" => self.state.props.italic = Some(active),
            "ul" | "ulnone" | "uldb" => {
                self.state.props.underline = Some(
                    if !active || name == "ulnone" {
                        "none"
                    } else if name == "uldb" {
                        "dbl"
                    } else {
                        "sng"
                    }
                    .into(),
                );
            }
            "strike" | "striked" => {
                self.state.props.strike = Some(if !active {
                    Strike::None
                } else if name == "striked" {
                    Strike::Double
                } else {
                    Strike::Single
                })
            }
            "super" => self.state.props.baseline = Some(0.30),
            "sub" => self.state.props.baseline = Some(-0.25),
            "nosupersub" => self.state.props.baseline = Some(0.0),
            "cf" | "highlight" => {
                let color = usize::try_from(number)
                    .ok()
                    .and_then(|n| self.colors.get(n))
                    .copied()
                    .flatten();
                if name == "cf" {
                    self.state.props.fill = color.map(color_fill);
                } else {
                    self.state.props.highlight = color.map(ColorRef::rgb);
                }
            }
            "plain" => {
                self.state.props = RunProps::default();
                self.state.font = None;
                self.state.hidden = false;
            }
            "pard" => self.state.align = None,
            "ql" => self.state.align = Some(Align::Left),
            "qc" => self.state.align = Some(Align::Center),
            "qr" => self.state.align = Some(Align::Right),
            "qj" => self.state.align = Some(Align::Justify),
            "par" if !self.state.hidden => self.out.paragraph(),
            "line" if !self.state.hidden => self.out.soft_break(&self.state.props),
            "tab" => self.text("\t"),
            "uc" => self.state.uc = number.clamp(0, 16) as usize,
            "u" => self.unicode(number),
            "emdash" => self.text("—"),
            "endash" => self.text("–"),
            "bullet" => self.text("•"),
            "lquote" => self.text("‘"),
            "rquote" => self.text("’"),
            "ldblquote" => self.text("“"),
            "rdblquote" => self.text("”"),
            _ => {}
        }
    }

    fn unicode(&mut self, value: i32) {
        self.fallback = self.state.uc;
        let unit = (value & 0xffff) as u16;
        if (0xd800..=0xdbff).contains(&unit) {
            self.flush_high();
            self.high_surrogate = Some(unit);
            return;
        }
        let units = [self.high_surrogate.take(), Some(unit)];
        let value: String = char::decode_utf16(units.into_iter().flatten())
            .map(|c| c.unwrap_or('�'))
            .collect();
        self.text(&value);
    }
}

fn word(input: &[u8]) -> Option<(&str, Option<i32>, &[u8])> {
    let letters = input.iter().take_while(|b| b.is_ascii_alphabetic()).count();
    let (name, input) = input.split_at(letters);
    let negative = input.first() == Some(&b'-');
    let digits = input
        .iter()
        .skip(usize::from(negative))
        .take_while(|b| b.is_ascii_digit())
        .count();
    let size = if digits > 0 {
        digits + usize::from(negative)
    } else {
        0
    };
    let (value, tail) = input.split_at(size);
    let param = if value.is_empty() {
        None
    } else {
        Some(std::str::from_utf8(value).ok()?.parse().ok()?)
    };
    Some((
        std::str::from_utf8(name).ok()?,
        param,
        tail.strip_prefix(b" ").unwrap_or(tail),
    ))
}

fn codepage(page: i32) -> Option<&'static Encoding> {
    let label = match page {
        932 => "shift_jis",
        936 => "gbk",
        949 => "euc-kr",
        950 => "big5",
        65001 => "utf-8",
        1250..=1258 => return Encoding::for_label(format!("windows-{page}").as_bytes()),
        _ => return None,
    };
    Encoding::for_label(label.as_bytes())
}

fn instruction_link(instruction: &str) -> Option<Hyperlink> {
    let (command, tail) = instruction.trim().split_once(char::is_whitespace)?;
    if !command.eq_ignore_ascii_case("HYPERLINK") {
        return None;
    }
    let tail = tail.trim_start();
    let address = if let Some(quoted) = tail.strip_prefix('"') {
        quoted.split_once('"')?.0
    } else {
        tail.split_whitespace().next()?
    };
    url_link(address)
}
