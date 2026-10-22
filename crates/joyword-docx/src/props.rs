//! Paragraph and run property subsets (ECMA-376 Part 1 §17.3.1 paragraph
//! properties, §17.3.2 run properties, §17.18 and §22.9.2 simple types;
//! Transitional spellings from Part 4).
//!
//! Values keep their source unit (twips, half points, hundredths of a
//! character or line); nothing is converted to EMU here. Unset stays `None`,
//! and an explicit "off" stays `Some(false)`. Anything the model has no
//! field for — an unmapped child, an attribute that overrides a mapped one
//! (a theme font, or a theme colour with tint or shade), a universal measure such as `2cm`, an
//! unknown value — clears [`Findings::complete`], and the caller preserves the
//! whole property element.

use std::sync::Arc;

use joyword_model::{
    Color, FontHint, FontRef, Indent, Justification, LineSpacing, Measure, NumberingRef, ParaProps,
    RunProps, Spacing, StyleId, TabAlign, TabLeader, TabStop, ThemeColorSlot, ThemeFont, Toggle,
    Underline,
};

use crate::ns::{W, W14};
use crate::xml::Element;

/// An attribute value outside the supported set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Invalid {
    pub element: String,
    pub attribute: String,
    pub value: String,
}

#[derive(Debug)]
pub(crate) struct Findings {
    /// Every child and value was represented in the model.
    pub complete: bool,
    /// A gap that can change a line or a page. Cleared only by
    /// [`Findings::neutral_gap`] gaps.
    pub layout_relevant: bool,
    pub invalid: Vec<Invalid>,
}

impl Default for Findings {
    fn default() -> Self {
        Self {
            complete: true,
            layout_relevant: false,
            invalid: Vec::new(),
        }
    }
}

impl Findings {
    /// Something the model cannot represent.
    pub(crate) fn gap(&mut self) {
        self.complete = false;
        self.layout_relevant = true;
    }

    /// Something the model cannot represent that never moves or paints a
    /// glyph differently in this layout.
    pub(crate) fn neutral_gap(&mut self) {
        self.complete = false;
    }

    /// `true` when the element is preserved only for gaps layout can ignore.
    pub(crate) fn layout_neutral(&self) -> bool {
        !self.complete && !self.layout_relevant
    }

    pub(crate) fn invalid(&mut self, element: &Element, attribute: &str, value: &str) {
        self.gap();
        self.invalid.push(Invalid {
            element: element.qname.to_string(),
            attribute: attribute.to_owned(),
            value: value.to_owned(),
        });
    }
}

pub(crate) fn val(element: &Element) -> Option<&str> {
    element.attribute(Some(W), "val")
}

/// ST_OnOff (§22.9.2.7; Transitional adds `on` and `off`, Part 4). A
/// missing value means "on".
pub(crate) fn on_off(value: Option<&str>) -> Option<bool> {
    match value {
        None | Some("true" | "1" | "on") => Some(true),
        Some("false" | "0" | "off") => Some(false),
        Some(_) => None,
    }
}

fn toggle(element: &Element, findings: &mut Findings) -> Toggle {
    let value = val(element);
    let parsed = on_off(value);
    if parsed.is_none() {
        findings.invalid(element, "w:val", value.unwrap_or_default());
    }
    parsed
}

/// Decimal integer as in ST_DecimalNumber and the numeric form of the
/// measure types. Universal measures (`1in`, `2.5cm`) are not mapped.
fn integer(element: &Element, attribute: &str, findings: &mut Findings) -> Option<i64> {
    let value = element.attribute(Some(W), attribute)?;
    match value.parse::<i64>() {
        Ok(number) => Some(number),
        Err(_) => {
            findings.invalid(element, &format!("w:{attribute}"), value);
            None
        }
    }
}

/// An ST_TwipsMeasure or ST_SignedTwipsMeasure attribute in twips.
pub(crate) fn twips(
    element: &Element,
    attribute: &str,
    findings: &mut Findings,
) -> Option<Measure> {
    measure(element, attribute, Measure::Twips, findings)
}

fn measure(
    element: &Element,
    attribute: &str,
    unit: fn(i64) -> Measure,
    findings: &mut Findings,
) -> Option<Measure> {
    integer(element, attribute, findings).map(unit)
}

/// The first of `attributes` that is present, so `w:start` wins over its
/// Transitional alias `w:left` when a producer writes both.
fn first_measure(
    element: &Element,
    attributes: &[&str],
    unit: fn(i64) -> Measure,
    findings: &mut Findings,
) -> Option<Measure> {
    let name = attributes
        .iter()
        .find(|name| element.attribute(Some(W), name).is_some())?;
    if attributes
        .iter()
        .filter(|name| element.attribute(Some(W), name).is_some())
        .count()
        > 1
    {
        // The other spelling is kept only in the preserved element.
        findings.gap();
    }
    measure(element, name, unit, findings)
}

/// Chooses between a relative (`*Chars`, `*Lines`) and an absolute value of
/// one property. ECMA-376 lets a specified relative value win; Word ignores
/// a zero relative value ([MS-OI29500] notes on Part 1 §17.3.1.12 and
/// §17.3.1.33), which is followed here. When both are present the element
/// is preserved so the value not chosen is not lost.
fn relative_or_absolute(
    relative: Option<Measure>,
    absolute: Option<Measure>,
    findings: &mut Findings,
) -> Option<Measure> {
    if relative.is_some() && absolute.is_some() {
        findings.gap();
    }
    match (relative, absolute) {
        (Some(Measure::CharHundredths(0) | Measure::LineHundredths(0)), Some(absolute)) => {
            Some(absolute)
        }
        (relative, absolute) => relative.or(absolute),
    }
}

/// `w:ind` (§17.3.1.12); `left`/`right` and their `*Chars` forms are the
/// Transitional spellings of `start`/`end` (Part 4).
fn indent(element: &Element, findings: &mut Findings) -> Indent {
    let mut pick = |chars: &[&str], twips: &[&str]| {
        let relative = first_measure(element, chars, Measure::CharHundredths, findings);
        let absolute = first_measure(element, twips, Measure::Twips, findings);
        relative_or_absolute(relative, absolute, findings)
    };
    Indent {
        start: pick(&["startChars", "leftChars"], &["start", "left"]),
        end: pick(&["endChars", "rightChars"], &["end", "right"]),
        first_line: pick(&["firstLineChars"], &["firstLine"]),
        hanging: pick(&["hangingChars"], &["hanging"]),
    }
}

/// `w:spacing` (§17.3.1.33). `beforeLines`/`afterLines` and `before`/`after`
/// are chosen like indentation characters. A missing `lineRule` means
/// `auto`, where `line` is in 240ths of a line; otherwise it is in twips.
fn spacing(element: &Element, findings: &mut Findings) -> Spacing {
    let mut pick = |lines: &str, twips: &str| {
        let relative = measure(element, lines, Measure::LineHundredths, findings);
        let absolute = measure(element, twips, Measure::Twips, findings);
        relative_or_absolute(relative, absolute, findings)
    };
    let before = pick("beforeLines", "before");
    let after = pick("afterLines", "after");
    let flag = |name: &str, findings: &mut Findings| {
        let value = element.attribute(Some(W), name)?;
        let parsed = on_off(Some(value));
        if parsed.is_none() {
            findings.invalid(element, &format!("w:{name}"), value);
        }
        parsed
    };
    let before_auto = flag("beforeAutospacing", findings);
    let after_auto = flag("afterAutospacing", findings);
    let line = integer(element, "line", findings).and_then(|line| {
        match element.attribute(Some(W), "lineRule").unwrap_or("auto") {
            "auto" => Some(LineSpacing::Auto(line)),
            "exact" => Some(LineSpacing::Exact(Measure::Twips(line))),
            "atLeast" => Some(LineSpacing::AtLeast(Measure::Twips(line))),
            other => {
                findings.invalid(element, "w:lineRule", other);
                None
            }
        }
    });
    Spacing {
        before,
        after,
        before_auto,
        after_auto,
        line,
    }
}

fn justification(element: &Element, findings: &mut Findings) -> Option<Justification> {
    let value = val(element).unwrap_or_default();
    let justification = match value {
        "start" | "left" => Justification::Start,
        "end" | "right" => Justification::End,
        "center" => Justification::Center,
        "both" => Justification::Both,
        "distribute" => Justification::Distribute,
        // Kashida, Thai distribution and numTab have no model value yet.
        _ => {
            findings.invalid(element, "w:val", value);
            return None;
        }
    };
    Some(justification)
}

fn numbering(element: &Element, findings: &mut Findings) -> Option<NumberingRef> {
    let number = |local: &str, findings: &mut Findings| {
        let child = element.child(W, local)?;
        let value = val(child).unwrap_or_default();
        let parsed = value.parse::<u32>().ok();
        if parsed.is_none() {
            findings.invalid(child, "w:val", value);
        }
        parsed
    };
    let num_id = number("numId", findings);
    let level = number("ilvl", findings);
    if element
        .elements()
        .any(|child| !child.is(W, "numId") && !child.is(W, "ilvl"))
    {
        findings.gap();
    }
    match (num_id, level.map(u8::try_from)) {
        (Some(num_id), Some(Ok(level))) => Some(NumberingRef { num_id, level }),
        _ => {
            // A reference without both values is kept only in the source.
            findings.gap();
            None
        }
    }
}

fn tabs(element: &Element, findings: &mut Findings) -> Vec<TabStop> {
    let mut stops = Vec::new();
    for child in element.elements() {
        if !child.is(W, "tab") {
            findings.gap();
            continue;
        }
        let Some(position) = integer(child, "pos", findings) else {
            findings.gap();
            continue;
        };
        let Some(value) = child.attribute(Some(W), "val") else {
            findings.gap();
            continue;
        };
        let align = match value {
            "left" => TabAlign::Left,
            "center" => TabAlign::Center,
            "right" => TabAlign::Right,
            "decimal" => TabAlign::Decimal,
            "bar" => TabAlign::Bar,
            "num" => TabAlign::Num,
            "clear" => TabAlign::Clear,
            other => {
                findings.invalid(child, "w:val", other);
                continue;
            }
        };
        let leader = match child.attribute(Some(W), "leader").unwrap_or("none") {
            "none" => TabLeader::None,
            "dot" => TabLeader::Dot,
            "hyphen" => TabLeader::Hyphen,
            "underscore" => TabLeader::Underscore,
            "middleDot" => TabLeader::MiddleDot,
            "heavy" => TabLeader::Heavy,
            other => {
                findings.invalid(child, "w:leader", other);
                continue;
            }
        };
        stops.push(TabStop {
            position: Measure::Twips(position),
            align,
            leader,
        });
    }
    stops
}

/// Mapped paragraph properties and the children the caller handles.
pub(crate) struct Para<'e> {
    pub props: ParaProps,
    pub style: Option<StyleId>,
    /// `w:rPr`: paragraph mark run properties.
    pub mark: Option<&'e Element>,
    /// `w:sectPr`: the paragraph ends a section.
    pub section: Option<&'e Element>,
}

/// `w:pPr` (§17.3.1.26) or a style's / default `w:pPr`.
pub(crate) fn para<'e>(element: &'e Element, findings: &mut Findings) -> Para<'e> {
    let mut out = Para {
        props: ParaProps::default(),
        style: None,
        mark: None,
        section: None,
    };
    let props = &mut out.props;
    for child in element.elements() {
        if child.namespace.as_deref() != Some(W) {
            findings.gap();
            continue;
        }
        match &*child.local {
            "pStyle" => out.style = val(child).map(StyleId::new),
            "keepNext" => props.keep_next = toggle(child, findings),
            "keepLines" => props.keep_lines = toggle(child, findings),
            "widowControl" => props.widow_control = toggle(child, findings),
            "pageBreakBefore" => props.page_break_before = toggle(child, findings),
            "snapToGrid" => props.snap_to_grid = toggle(child, findings),
            "jc" => props.justification = justification(child, findings),
            "ind" => props.indent = indent(child, findings),
            "spacing" => props.spacing = spacing(child, findings),
            "outlineLvl" => {
                let value = val(child).unwrap_or_default();
                match value.parse::<u8>() {
                    Ok(level) if level <= 9 => props.outline_level = Some(level),
                    _ => findings.invalid(child, "w:val", value),
                }
            }
            "numPr" => props.numbering = numbering(child, findings),
            "tabs" => props.tabs = Some(tabs(child, findings)),
            "rPr" => out.mark = Some(child),
            "sectPr" => out.section = Some(child),
            _ => findings.gap(),
        }
    }
    out
}

fn theme_font(value: &str) -> Option<ThemeFont> {
    Some(match value {
        "majorAscii" => ThemeFont::MajorAscii,
        "majorHAnsi" => ThemeFont::MajorHAnsi,
        "majorEastAsia" => ThemeFont::MajorEastAsia,
        "majorBidi" => ThemeFont::MajorBidi,
        "minorAscii" => ThemeFont::MinorAscii,
        "minorHAnsi" => ThemeFont::MinorHAnsi,
        "minorEastAsia" => ThemeFont::MinorEastAsia,
        "minorBidi" => ThemeFont::MinorBidi,
        _ => return None,
    })
}

/// `w:rFonts` (§17.3.2.26): a theme attribute takes precedence over the
/// named font of the same slot.
fn fonts(element: &Element, out: &mut RunProps, findings: &mut Findings) {
    let slot = |named: &str, theme: &str, findings: &mut Findings| -> Option<FontRef> {
        let named_value = element.attribute(Some(W), named);
        if let Some(value) = element.attribute(Some(W), theme) {
            if named_value.is_some() {
                findings.gap();
            }
            return match theme_font(value) {
                Some(theme) => Some(FontRef::Theme(theme)),
                None => {
                    findings.invalid(element, &format!("w:{theme}"), value);
                    None
                }
            };
        }
        named_value.map(|name| FontRef::Named(Arc::from(name)))
    };
    out.fonts.ascii = slot("ascii", "asciiTheme", findings);
    out.fonts.h_ansi = slot("hAnsi", "hAnsiTheme", findings);
    out.fonts.east_asia = slot("eastAsia", "eastAsiaTheme", findings);
    out.fonts.complex_script = slot("cs", "cstheme", findings);
    if let Some(value) = element.attribute(Some(W), "hint") {
        out.fonts.hint = match value {
            "default" => Some(FontHint::Default),
            "eastAsia" => Some(FontHint::EastAsia),
            "cs" => Some(FontHint::ComplexScript),
            other => {
                findings.invalid(element, "w:hint", other);
                None
            }
        };
    }
}

fn underline(element: &Element, findings: &mut Findings) -> Option<Underline> {
    if element.attribute(Some(W), "color").is_some()
        || element.attribute(Some(W), "themeColor").is_some()
    {
        findings.gap();
    }
    let value = val(element).unwrap_or_default();
    Some(match value {
        "none" => Underline::None,
        "single" => Underline::Single,
        "double" => Underline::Double,
        "thick" => Underline::Thick,
        "dotted" => Underline::Dotted,
        "dash" => Underline::Dash,
        "wave" => Underline::Wave,
        "words" => Underline::Words,
        _ => {
            findings.invalid(element, "w:val", value);
            return None;
        }
    })
}

fn color(element: &Element, findings: &mut Findings) -> Option<Color> {
    let theme_name = element.attribute(Some(W), "themeColor");
    let tinted = element.attribute(Some(W), "themeTint").is_some()
        || element.attribute(Some(W), "themeShade").is_some();
    if let Some(name) = theme_name {
        return match ThemeColorSlot::from_word_name(name) {
            Some(slot) => {
                let tint = theme_byte(element, "themeTint", findings);
                let shade = theme_byte(element, "themeShade", findings);
                if tinted && element.attribute(Some(W), "themeTint").is_some() && tint.is_none() {
                    findings.gap();
                }
                if tinted && element.attribute(Some(W), "themeShade").is_some() && shade.is_none() {
                    findings.gap();
                }
                Some(Color::Theme { slot, tint, shade })
            }
            None => {
                findings.invalid(element, "w:themeColor", name);
                None
            }
        };
    }
    if element.attributes.iter().any(|attribute| {
        attribute.namespace.as_deref() == Some(W) && attribute.local.starts_with("theme")
    }) {
        findings.gap();
    }
    cached_rgb(element, findings)
}

fn theme_byte(element: &Element, name: &str, findings: &mut Findings) -> Option<u8> {
    let value = element.attribute(Some(W), name)?;
    match u8::from_str_radix(value, 16) {
        Ok(byte) if value.len() == 2 => Some(byte),
        _ => {
            findings.invalid(element, name, value);
            None
        }
    }
}

fn cached_rgb(element: &Element, findings: &mut Findings) -> Option<Color> {
    let value = val(element).unwrap_or_default();
    if value == "auto" {
        return Some(Color::Auto);
    }
    let hex = |range: std::ops::Range<usize>| {
        value
            .get(range)
            .and_then(|digits| u8::from_str_radix(digits, 16).ok())
    };
    match (value.len(), hex(0..2), hex(2..4), hex(4..6)) {
        (6, Some(r), Some(g), Some(b)) => Some(Color::Rgb(r, g, b)),
        _ => {
            findings.invalid(element, "w:val", value);
            None
        }
    }
}

/// `w:rPr` (§17.3.2.28), a paragraph mark's or a style's / default `w:rPr`.
pub(crate) fn run(element: &Element, findings: &mut Findings) -> (RunProps, Option<StyleId>) {
    let mut out = RunProps::default();
    let mut style = None;
    for child in element.elements() {
        if child.namespace.as_deref() == Some(W14) && w14_run_child_layout_neutral(child) {
            findings.neutral_gap();
            continue;
        }
        if child.namespace.as_deref() != Some(W) {
            findings.gap();
            continue;
        }
        match &*child.local {
            "rStyle" => style = val(child).map(StyleId::new),
            "b" => out.bold = toggle(child, findings),
            "i" => out.italic = toggle(child, findings),
            "strike" => out.strike = toggle(child, findings),
            "vanish" => out.hidden = toggle(child, findings),
            "u" => out.underline = underline(child, findings),
            "sz" => out.size = measure(child, "val", Measure::HalfPoints, findings),
            "rFonts" => fonts(child, &mut out, findings),
            "color" => out.color = color(child, findings),
            "lang" => {
                let tag = |name: &str| child.attribute(Some(W), name).map(Arc::from);
                out.languages.latin = tag("val");
                out.languages.east_asia = tag("eastAsia");
                out.languages.bidi = tag("bidi");
            }
            local if run_child_layout_neutral(local) => findings.neutral_gap(),
            _ => findings.gap(),
        }
    }
    (out, style)
}

/// `w:rPr` children with no model field that change no advance and paint
/// nothing in this layout. `w:szCs`, `w:bCs` and `w:iCs` format only
/// complex-script text, which layout sizes and styles from `w:sz`, `w:b` and
/// `w:i`; `w:kern` is the kerning threshold, and the shaper already kerns.
fn run_child_layout_neutral(local: &str) -> bool {
    matches!(local, "szCs" | "bCs" | "iCs" | "kern" | "noProof")
}

/// `w14:` run properties ([MS-DOCX] §2.6) at the values that match the
/// shaper's default OpenType features.
fn w14_run_child_layout_neutral(child: &Element) -> bool {
    let value = child.attribute(Some(W14), "val");
    match &*child.local {
        "ligatures" => value == Some("standardContextual"),
        "numForm" | "numSpacing" => value == Some("default"),
        "cntxtAlts" => on_off(value) == Some(true),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::XmlReader;
    use joyoffice_opc::Budget;

    fn element(xml: &str) -> Element {
        let xml = format!(r#"<x xmlns:w="{W}">{xml}</x>"#);
        let root = XmlReader::new("/t.xml", xml.as_bytes(), &Budget::default())
            .unwrap()
            .whole()
            .unwrap();
        root.elements().next().unwrap().clone()
    }

    #[test]
    fn on_off_accepts_transitional_spellings() {
        for (value, expected) in [
            (None, Some(true)),
            (Some("1"), Some(true)),
            (Some("on"), Some(true)),
            (Some("true"), Some(true)),
            (Some("0"), Some(false)),
            (Some("off"), Some(false)),
            (Some("false"), Some(false)),
            (Some("yes"), None),
        ] {
            assert_eq!(on_off(value), expected, "{value:?}");
        }
    }

    #[test]
    fn run_subset_keeps_unset_off_and_flags_unmapped_content() {
        let mut findings = Findings::default();
        let rpr = element(
            r#"<w:rPr><w:rStyle w:val="a5"/><w:b/><w:i w:val="0"/><w:sz w:val="21"/><w:rFonts w:ascii="Times New Roman" w:eastAsia="宋体" w:hint="eastAsia"/><w:color w:val="FF0000"/><w:lang w:val="en-US" w:eastAsia="zh-CN"/></w:rPr>"#,
        );
        let (props, style) = run(&rpr, &mut findings);
        assert!(findings.complete, "{findings:?}");
        assert_eq!(style, Some(StyleId::new("a5")));
        assert_eq!(
            (props.bold, props.italic, props.strike),
            (Some(true), Some(false), None)
        );
        assert_eq!(props.size, Some(Measure::HalfPoints(21)));
        assert_eq!(props.fonts.east_asia, Some(FontRef::Named("宋体".into())));
        assert_eq!(props.fonts.hint, Some(FontHint::EastAsia));
        assert_eq!(props.color, Some(Color::Rgb(0xFF, 0, 0)));
        assert_eq!(props.languages.east_asia.as_deref(), Some("zh-CN"));

        let mut findings = Findings::default();
        let rpr = element(
            r#"<w:rPr><w:szCs w:val="21"/><w:rFonts w:ascii="A" w:asciiTheme="minorHAnsi"/><w:sz w:val="12pt"/><w:b w:val="maybe"/></w:rPr>"#,
        );
        let (props, _) = run(&rpr, &mut findings);
        assert!(!findings.complete);
        assert_eq!(
            props.fonts.ascii,
            Some(FontRef::Theme(ThemeFont::MinorHAnsi))
        );
        assert_eq!(props.size, None);
        assert_eq!(props.bold, None);
        assert_eq!(findings.invalid.len(), 2);
    }

    /// Word's document defaults carry `w:kern`, `w:szCs` and
    /// `w14:ligatures`; preserving them must not stop layout.
    #[test]
    fn invisible_run_children_are_preserved_as_layout_neutral() {
        let w14 = r#"xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml""#;
        let classify = |inner: &str| {
            let mut findings = Findings::default();
            let rpr = element(&format!(r#"<w:rPr {w14}>{inner}</w:rPr>"#));
            let _ = run(&rpr, &mut findings);
            (findings.complete, findings.layout_neutral())
        };
        assert_eq!(classify(r#"<w:sz w:val="22"/>"#), (true, false));
        assert_eq!(
            classify(
                r#"<w:kern w:val="2"/><w:sz w:val="22"/><w:szCs w:val="24"/><w:bCs/><w:noProof/><w14:ligatures w14:val="standardContextual"/>"#
            ),
            (false, true)
        );
        for relevant in [
            r#"<w:kern w:val="2"/><w:spacing w:val="20"/>"#,
            r#"<w14:ligatures w14:val="none"/>"#,
            r#"<w14:textOutline/>"#,
            r#"<w:szCs w:val="24"/><w:sz w:val="12pt"/>"#,
            r#"<w:szCs w:val="24"/><w:rFonts w:ascii="A" w:asciiTheme="minorHAnsi"/>"#,
        ] {
            assert_eq!(classify(relevant), (false, false), "{relevant}");
        }
    }

    #[test]
    fn paragraph_subset_keeps_source_units() {
        let mut findings = Findings::default();
        let ppr = element(
            r#"<w:pPr><w:pStyle w:val="1"/><w:jc w:val="left"/><w:ind w:left="420" w:firstLineChars="200"/><w:spacing w:before="156" w:line="360"/><w:outlineLvl w:val="0"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="3"/></w:numPr><w:rPr><w:b/></w:rPr></w:pPr>"#,
        );
        let mapped = para(&ppr, &mut findings);
        assert!(findings.complete, "{findings:?}");
        assert_eq!(mapped.style, Some(StyleId::new("1")));
        let props = mapped.props;
        assert_eq!(props.justification, Some(Justification::Start));
        assert_eq!(props.indent.start, Some(Measure::Twips(420)));
        assert_eq!(props.indent.first_line, Some(Measure::CharHundredths(200)));
        assert_eq!(props.spacing.before, Some(Measure::Twips(156)));
        assert_eq!(props.spacing.line, Some(LineSpacing::Auto(360)));
        assert_eq!(props.outline_level, Some(0));
        assert_eq!(
            props.numbering,
            Some(NumberingRef {
                num_id: 3,
                level: 0
            })
        );
        assert!(mapped.mark.is_some());

        let mut findings = Findings::default();
        let ppr = element(
            r#"<w:pPr><w:ind w:firstLine="420" w:firstLineChars="200"/><w:jc w:val="thaiDistribute"/><w:spacing w:line="12" w:lineRule="sometimes"/></w:pPr>"#,
        );
        let props = para(&ppr, &mut findings).props;
        assert!(!findings.complete);
        assert_eq!(props.indent.first_line, Some(Measure::CharHundredths(200)));
        assert_eq!(props.justification, None);
        assert_eq!(props.spacing.line, None);

        let mut findings = Findings::default();
        let ppr = element(
            r#"<w:pPr><w:ind w:firstLine="420" w:firstLineChars="0"/><w:spacing w:before="100" w:beforeLines="0"/></w:pPr>"#,
        );
        let props = para(&ppr, &mut findings).props;
        assert!(!findings.complete);
        assert_eq!(props.indent.first_line, Some(Measure::Twips(420)));
        assert_eq!(props.spacing.before, Some(Measure::Twips(100)));
    }
}
