use joyword_model::props::{Border, BorderStyle, Borders, LineSpacing};

use super::placement;
use super::xml::{Attrs, Ev, Xml, get, length, local, stream};
use super::{Align, Arc, Columns, Fmt, HashMap, Kind, Para, Rgb, VAlign, model};
use model::Spacing;

// ---------------------------------------------------------------------------------------------
// Import

#[derive(Clone, Debug, Default)]
struct FmtDelta {
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    strike: Option<bool>,
    sup: Option<bool>,
    sub: Option<bool>,
    color: Option<Rgb>,
    background: Option<Rgb>,
    size: Option<f32>,
    font: Option<Arc<str>>,
    code: Option<bool>,
}

impl FmtDelta {
    fn apply(&self, f: &mut Fmt) {
        macro_rules! set {
            ($($k:ident),*) => { $( if let Some(v) = self.$k.clone() { f.$k = v; } )* };
        }
        set!(bold, italic, underline, strike, sup, sub, code);
        if let Some(bold) = self.bold {
            f.normal_weight = !bold;
        }
        if self.color.is_some() {
            f.color = self.color;
        }
        if self.background.is_some() {
            f.background = self.background;
        }
        if self.size.is_some() {
            f.size = self.size;
        }
        if let Some(fam) = &self.font {
            f.font = Some(fam.clone());
        }
    }
}

#[derive(Clone, Debug, Default)]
struct StyleInfo {
    parent: Option<String>,
    display: Option<String>,
    auto: bool,
    fmt: FmtDelta,
    align: Option<Align>,
    page_break: bool,
    /// `fo:keep-with-next`: `Some(true)` for "always", `Some(false)` for "auto".
    keep_next: Option<bool>,
    outline: Option<u8>,
    border_bottom: bool,
    /// `fo:background-color` of a table-cell style's `style:table-cell-properties`.
    cell_background: Option<Rgb>,
    /// `style:vertical-align` of a table-cell style's `style:table-cell-properties`.
    cell_valign: Option<VAlign>,
    cell_borders: [Option<Border>; 4],
    cell_padding: [Option<f32>; 4],
    spacing: Spacing,
    graphic: Attrs,
    /// `style:column-width` of a table-column style's `style:table-column-properties`, in points.
    column_width: Option<f32>,
    /// `style:master-page-name` of a paragraph or table style: its paragraphs (tables) start a
    /// page of that master page. Empty: explicitly none, which stops the lookup up the chain.
    master_page: Option<String>,
    /// `style:page-number` of a paragraph or table style (applied only with a master page): `Some(Some(n))` restarts page numbering at n,
    /// `Some(None)` ("auto") continues it.
    page_number: Option<Option<u32>>,
    /// `style:columns` of a section style.
    columns: Option<Columns>,
}

/// Columns of a `style:columns` element (ODF 1.3 §17.12): count and gap, clamped as the RTF and
/// DOCX readers do; equal widths only (`style:column` widths are not read).
pub(super) fn columns_of(a: &Attrs) -> Columns {
    let mut c = Columns::default();
    if let Some(n) = get(a, "fo:column-count").and_then(|v| v.trim().parse::<u64>().ok()) {
        c.count = n.clamp(1, 45) as u32;
    }
    if let Some(g) = get(a, "fo:column-gap")
        .and_then(length)
        .filter(|v| *v >= 0.0)
    {
        c.space = g.min(1584.0);
    }
    c
}

/// A `style:column-sep` (ODF 1.3 §17.13) draws a line between columns unless its style is "none".
pub(super) fn column_sep(a: &Attrs) -> bool {
    get(a, "style:style") != Some("none")
}

fn family<'a>(a: &'a Attrs, aliases: &'a HashMap<String, String>) -> Option<&'a str> {
    get(a, "fo:font-family").or_else(|| {
        let name = get(a, "style:font-name")?;
        Some(aliases.get(name).map_or(name, String::as_str))
    })
}

fn default_font(a: &Attrs, aliases: &HashMap<String, String>) -> Option<String> {
    let family = family(a, aliases)?;
    let family = family
        .split(',')
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches(['\'', '"']);
    (!family.is_empty() && !model::is_mono(family)).then(|| family.to_string())
}

fn text_properties(a: &Attrs, aliases: &HashMap<String, String>, style: &mut StyleInfo) {
    let fmt = &mut style.fmt;
    let is_bold = |weight: &str| {
        let weight = weight.trim();
        weight.eq_ignore_ascii_case("bold") || weight.parse::<u32>().is_ok_and(|n| n >= 600)
    };
    // Latin attributes take precedence; Asian and complex-script attributes are fallbacks.
    if let Some(weight) = get(a, "fo:font-weight")
        .or_else(|| get(a, "style:font-weight-asian"))
        .or_else(|| get(a, "style:font-weight-complex"))
    {
        fmt.bold = Some(is_bold(weight));
    }
    let is_italic = |value: &str| {
        value.trim().eq_ignore_ascii_case("italic") || value.trim().eq_ignore_ascii_case("oblique")
    };
    if let Some(value) = get(a, "fo:font-style")
        .or_else(|| get(a, "style:font-style-asian"))
        .or_else(|| get(a, "style:font-style-complex"))
    {
        fmt.italic = Some(is_italic(value));
    }
    let is_not_none = |value: &str| !value.trim().eq_ignore_ascii_case("none");
    if let Some(value) =
        get(a, "style:text-underline-style").or_else(|| get(a, "style:text-underline-type"))
    {
        fmt.underline = Some(is_not_none(value));
    }
    if let Some(value) =
        get(a, "style:text-line-through-style").or_else(|| get(a, "style:text-line-through-type"))
    {
        fmt.strike = Some(is_not_none(value));
    }
    if let Some(value) = get(a, "style:text-position") {
        let first = value.split_whitespace().next().unwrap_or("");
        let pct = first.trim_end_matches('%').parse::<f32>().unwrap_or(0.0);
        fmt.sup = Some(first.eq_ignore_ascii_case("super") || pct > 0.0);
        fmt.sub = Some(first.eq_ignore_ascii_case("sub") || pct < 0.0);
    }
    if let Some(color) = get(a, "fo:color").and_then(model::parse_color) {
        fmt.color = Some(color);
    }
    if let Some(color) = get(a, "fo:background-color").and_then(model::parse_color) {
        fmt.background = Some(color);
    }
    if let Some(size) = get(a, "fo:font-size")
        .and_then(length)
        .filter(|size| (1.0..=1638.0).contains(size))
    {
        fmt.size = Some(size);
    }
    if let Some(family) = family(a, aliases) {
        let family = family
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches(['\'', '"'])
            .to_string();
        if model::is_mono(&family)
            || get(a, "style:font-pitch")
                .is_some_and(|pitch| pitch.trim().eq_ignore_ascii_case("fixed"))
        {
            fmt.code = Some(true);
        } else if !family.is_empty() {
            fmt.font = model::kept_font(&family);
        }
    }
}

#[derive(Default)]
pub(super) struct Styles {
    map: HashMap<String, StyleInfo>,
    pub(super) lists: HashMap<String, Vec<bool>>,
    /// Font of `style:default-style` for paragraphs (the document default, ODF 1.3 §16.4).
    default_font: Option<String>,
    default_para: StyleInfo,
    font_aliases: HashMap<String, String>,
}

impl Styles {
    pub(super) fn read(&mut self, xml: Xml<'_>, auto_section_is_auto: bool) {
        let mut in_auto = false;
        let mut cur: Option<(String, StyleInfo)> = None;
        let mut in_default = false;
        let mut list: Option<(String, Vec<bool>)> = None;
        stream(xml, |ev| match ev {
            Ev::Start(q, a) => match local(&q) {
                "automatic-styles" => in_auto = true,
                "style" if q.starts_with("style:") => {
                    if let Some(name) = get(&a, "style:name") {
                        let info = StyleInfo {
                            parent: get(&a, "style:parent-style-name").map(String::from),
                            display: get(&a, "style:display-name").map(String::from),
                            auto: in_auto && auto_section_is_auto,
                            outline: get(&a, "style:default-outline-level")
                                .and_then(|v| v.trim().parse::<u8>().ok()),
                            master_page: get(&a, "style:master-page-name").map(String::from),
                            ..Default::default()
                        };
                        cur = Some((name.to_string(), info));
                    }
                }
                "default-style" if q.starts_with("style:") => {
                    in_default = get(&a, "style:family") == Some("paragraph")
                }
                "font-face" if q == "style:font-face" => {
                    if let (Some(name), Some(family)) =
                        (get(&a, "style:name"), get(&a, "svg:font-family"))
                    {
                        self.font_aliases
                            .insert(name.to_string(), family.to_string());
                    }
                }
                "text-properties" if in_default && cur.is_none() => {
                    self.default_font = default_font(&a, &self.font_aliases);
                    text_properties(&a, &self.font_aliases, &mut self.default_para);
                }
                "text-properties" => {
                    if let Some((_, s)) = &mut cur {
                        text_properties(&a, &self.font_aliases, s);
                    }
                }
                "table-properties" => {
                    if let Some((_, s)) = &mut cur
                        && let Some(v) = get(&a, "style:page-number")
                    {
                        s.page_number = Some(v.trim().parse::<u32>().ok().map(|n| n.min(32_767)));
                    }
                }
                "table-column-properties" => {
                    if let Some((_, s)) = &mut cur
                        && let Some(w) = get(&a, "style:column-width").and_then(length)
                    {
                        s.column_width = Some(w);
                    }
                }
                "graphic-properties" => {
                    if let Some((_, s)) = &mut cur {
                        s.graphic = a
                            .into_iter()
                            .filter(|(key, _)| placement::supported_property(key))
                            .collect();
                    }
                }
                "table-cell-properties" => {
                    if let Some((_, s)) = &mut cur {
                        let all = get(&a, "fo:border");
                        s.cell_borders = ["top", "left", "bottom", "right"].map(|side| {
                            get(&a, &format!("fo:border-{side}"))
                                .or(all)
                                .and_then(border)
                        });
                        s.cell_padding = placement::margins(&a, "padding");
                        if let Some(c) = get(&a, "fo:background-color").and_then(model::parse_color)
                        {
                            s.cell_background = Some(c);
                        }
                        if let Some(va) = get(&a, "style:vertical-align") {
                            s.cell_valign = match va.trim() {
                                "top" => Some(VAlign::Top),
                                "middle" | "center" => Some(VAlign::Center),
                                "bottom" => Some(VAlign::Bottom),
                                _ => None,
                            };
                        }
                    }
                }
                "paragraph-properties" => {
                    if in_default && cur.is_none() {
                        self.default_para.spacing = spacing(&a);
                    }
                    if let Some((_, s)) = &mut cur {
                        s.spacing = spacing(&a);
                        s.align = match get(&a, "fo:text-align")
                            .map(|v| v.trim().to_ascii_lowercase())
                            .as_deref()
                        {
                            Some("center") => Some(Align::Center),
                            Some("end") | Some("right") => Some(Align::Right),
                            Some("justify") => Some(Align::Justify),
                            Some("start") | Some("left") => Some(Align::Left),
                            _ => s.align,
                        };
                        if get(&a, "fo:break-before")
                            .is_some_and(|b| b.trim().eq_ignore_ascii_case("page"))
                        {
                            s.page_break = true;
                        }
                        match get(&a, "fo:keep-with-next") {
                            Some("always") => s.keep_next = Some(true),
                            Some("auto") => s.keep_next = Some(false),
                            _ => {}
                        }
                        if let Some(v) = get(&a, "style:page-number") {
                            s.page_number =
                                Some(v.trim().parse::<u32>().ok().map(|n| n.min(32_767)));
                        }
                        if get(&a, "fo:border-bottom")
                            .is_some_and(|b| !b.trim().eq_ignore_ascii_case("none"))
                        {
                            s.border_bottom = true;
                        }
                    }
                }
                "columns" if q.starts_with("style:") => {
                    if let Some((_, s)) = &mut cur {
                        s.columns = Some(columns_of(&a));
                    }
                }
                "column-sep" => {
                    if let Some((_, s)) = &mut cur
                        && let Some(c) = &mut s.columns
                    {
                        c.separator = column_sep(&a);
                    }
                }
                "list-style" => {
                    list = get(&a, "style:name").map(|n| (n.to_string(), vec![false; 10]))
                }
                "list-level-style-number"
                | "list-level-style-bullet"
                | "list-level-style-image" => {
                    if let Some((_, v)) = &mut list {
                        let lv = get(&a, "text:level")
                            .and_then(|l| l.trim().parse::<usize>().ok())
                            .unwrap_or(1)
                            .clamp(1, 10);
                        let ordered = local(&q) == "list-level-style-number"
                            && get(&a, "style:num-format").is_some_and(|f| !f.is_empty());
                        if let Some(slot) = v.get_mut(lv - 1) {
                            *slot = ordered;
                        }
                    }
                }
                _ => {}
            },
            Ev::End(q) => match local(&q) {
                "automatic-styles" => in_auto = false,
                "default-style" if q.starts_with("style:") => in_default = false,
                "style" if q.starts_with("style:") => {
                    if let Some((n, s)) = cur.take()
                        && self.map.len() < 100_000
                    {
                        self.map.insert(n, s);
                    }
                }
                "list-style" => {
                    if let Some((n, v)) = list.take() {
                        self.lists.insert(n, v);
                    }
                }
                _ => {}
            },
            Ev::Text(_) => {}
        });
    }

    /// The document's default font: the Standard paragraph style's, else the default style's.
    pub(super) fn doc_font(&self) -> Option<String> {
        self.map
            .get("Standard")
            .filter(|s| !s.auto)
            .and_then(|s| s.fmt.font.as_deref().map(str::to_string))
            .or_else(|| self.default_font.clone())
    }

    fn chain<'a>(&'a self, name: &'a str) -> Vec<(&'a str, &'a StyleInfo)> {
        let mut out = Vec::new();
        let mut cur = Some(name);
        while let Some(n) = cur {
            if out.len() >= 16 {
                break;
            }
            let Some(s) = self.map.get(n) else { break };
            if out.iter().any(|(m, _): &(&str, &StyleInfo)| *m == n) {
                break;
            }
            out.push((n, s));
            cur = s.parent.as_deref();
        }
        out
    }

    /// Text formatting of a text style (whole chain).
    pub(super) fn text_fmt(&self, name: &str, base: &Fmt) -> Fmt {
        let mut f = base.clone();
        for (_, s) in self.chain(name).iter().rev() {
            s.fmt.apply(&mut f);
        }
        f
    }

    /// The page a paragraph style starts (`style:master-page-name` and `style:page-number`,
    /// ODF 1.3 §16.2 and §17.6): the nearest master page and page-number setting in its chain.
    pub(super) fn page_start<'a>(
        &'a self,
        name: &'a str,
    ) -> (Option<&'a str>, Option<Option<u32>>) {
        let chain = self.chain(name);
        let master = chain
            .iter()
            .find_map(|(_, s)| s.master_page.as_deref())
            .filter(|m| !m.is_empty());
        (master, chain.iter().find_map(|(_, s)| s.page_number))
    }

    /// Columns of a section style (whole chain).
    pub(super) fn columns(&self, name: &str) -> Option<Columns> {
        self.chain(name).iter().find_map(|(_, s)| s.columns.clone())
    }

    /// Width in points of a table-column style (whole chain).
    pub(super) fn column_width(&self, name: &str) -> Option<f32> {
        self.chain(name).iter().find_map(|(_, s)| s.column_width)
    }

    /// Background colour of a table-cell style (whole chain).
    pub(super) fn cell_background(&self, name: &str) -> Option<Rgb> {
        self.chain(name).iter().find_map(|(_, s)| s.cell_background)
    }

    /// Vertical alignment of a table-cell style (whole chain).
    pub(super) fn cell_valign(&self, name: &str) -> Option<VAlign> {
        self.chain(name).iter().find_map(|(_, s)| s.cell_valign)
    }

    pub(super) fn cell_layout(&self, name: &str) -> (Borders, [f32; 4]) {
        let chain = self.chain(name);
        let side = |i: usize| {
            Some(
                chain
                    .iter()
                    .find_map(|(_, s)| s.cell_borders[i])
                    .unwrap_or_default(),
            )
        };
        let margins = std::array::from_fn(|i| {
            chain
                .iter()
                .find_map(|(_, s)| s.cell_padding[i])
                .unwrap_or(0.0)
        });
        (
            Borders {
                top: side(0),
                left: side(1),
                bottom: side(2),
                right: side(3),
                ..Borders::default()
            },
            margins,
        )
    }

    pub(super) fn graphic(&self, name: &str) -> Attrs {
        let mut attrs = Attrs::new();
        for (_, style) in self.chain(name) {
            for (key, val) in &style.graphic {
                if get(&attrs, key).is_none() {
                    attrs.push((key.clone(), val.clone()));
                }
            }
        }
        attrs
    }

    /// Resolve the source style chain before mapping the heading role to a JoyWord style.
    pub(super) fn para(&self, name: &str) -> (Para, Fmt) {
        let chain = self.chain(name);
        let mut kind = Kind::Normal;
        for (n, s) in &chain {
            let dn = s
                .display
                .as_deref()
                .unwrap_or(n)
                .to_ascii_lowercase()
                .replace("_20_", " ");
            let nn = n.to_ascii_lowercase().replace("_20_", " ");
            for cand in [dn.as_str(), nn.as_str()] {
                if let Some(d) = cand
                    .strip_prefix("heading ")
                    .and_then(|r| r.trim().parse::<u8>().ok())
                {
                    kind = Kind::Heading(d.clamp(1, 6));
                } else if cand == "title" {
                    kind = Kind::Title;
                } else if cand == "quotations" || cand == "quote" {
                    kind = Kind::Quote;
                } else if cand == "preformatted text" || cand == "source text" || cand == "code" {
                    kind = Kind::Code;
                }
            }
            if kind != Kind::Normal {
                break;
            }
            if let Some(o) = s.outline.filter(|o| (1..=10).contains(o)) {
                kind = Kind::Heading(o.min(6));
                break;
            }
        }
        let align = chain.iter().find_map(|(_, s)| s.align);
        let pb = chain
            .iter()
            .take_while(|(_, s)| s.auto)
            .any(|(_, s)| s.page_break);
        if kind == Kind::Normal && chain.iter().any(|(_, s)| s.border_bottom) {
            kind = Kind::Rule;
        }
        let mut f = Fmt {
            normal_weight: matches!(kind, Kind::Heading(_) | Kind::Title),
            ..Fmt::default()
        };
        self.default_para.fmt.apply(&mut f);
        for (_, s) in chain.iter().rev() {
            s.fmt.apply(&mut f);
        }
        let pick = |property: fn(&Spacing) -> Option<f32>| {
            chain
                .iter()
                .find_map(|(_, s)| property(&s.spacing))
                .or_else(|| property(&self.default_para.spacing))
                .or(Some(0.0))
        };
        let spacing = Spacing {
            before: pick(|s| s.before),
            after: pick(|s| s.after),
            left: pick(|s| s.left),
            right: pick(|s| s.right),
            first: pick(|s| s.first),
            line: Some(
                chain
                    .iter()
                    .find_map(|(_, s)| s.spacing.line)
                    .or(self.default_para.spacing.line)
                    .unwrap_or_default(),
            ),
            ..Spacing::default()
        };
        let keep_next = chain.iter().find_map(|(_, s)| s.keep_next).unwrap_or(false);
        (
            Para {
                kind,
                align,
                page_break: pb,
                keep_next,
                spacing,
                ..Para::default()
            },
            f,
        )
    }
}

fn spacing(a: &Attrs) -> Spacing {
    let read = |name| {
        get(a, name)
            .and_then(length)
            .map(|v| v.clamp(-1584.0, 1584.0))
    };
    let margin = read("fo:margin");
    let line = get(a, "fo:line-height").and_then(|v| {
        if v == "normal" {
            return Some(LineSpacing::default());
        }
        if let Some(pct) = v.strip_suffix('%') {
            let n = pct.parse::<f32>().ok().filter(|n| n.is_finite())?;
            return Some(LineSpacing::Multiple((n / 100.0).clamp(0.01, 20.0)));
        }
        length(v).map(|v| LineSpacing::Exactly(v.clamp(0.1, 1584.0)))
    });
    Spacing {
        before: read("fo:margin-top").or(margin).map(|v| v.max(0.0)),
        after: read("fo:margin-bottom").or(margin).map(|v| v.max(0.0)),
        left: read("fo:margin-left").or(margin),
        right: read("fo:margin-right").or(margin),
        first: read("fo:text-indent"),
        line: line.or_else(|| {
            read("style:line-height-at-least").map(|v| LineSpacing::AtLeast(v.max(0.0)))
        }),
        ..Spacing::default()
    }
}

fn border(value: &str) -> Option<Border> {
    let mut parts = value.split_ascii_whitespace();
    let first = parts.next()?;
    if matches!(first, "none" | "hidden") {
        return Some(Border::default());
    }
    let width = length(first)?.clamp(0.0, 1584.0);
    let style = match parts.next()? {
        "none" | "hidden" => BorderStyle::None,
        "dotted" => BorderStyle::Dotted,
        "dashed" => BorderStyle::Dashed,
        "dash-dot" | "dash-dot-dot" => BorderStyle::DotDash,
        "double" | "double-thin" => BorderStyle::Double,
        _ => BorderStyle::Single,
    };
    Some(Border {
        style,
        width,
        color: parts.next().and_then(model::parse_color),
        space: 0.0,
    })
}
