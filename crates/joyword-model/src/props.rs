//! Formatting properties in their source units (ARCHITECTURE §2, STYLE-001).
//!
//! Lengths keep the unit the document used. Character and line units,
//! percentages and `auto` depend on fonts and the document grid, so only layout
//! may resolve them; importers must not fold them into EMU. Every property is
//! optional so "unset" (inherit) stays distinct from an explicit value,
//! including an explicit `false` for toggle properties.
//!
//! Unit definitions follow ECMA-376 Part 1 (5th edition) simple types:
//! ST_TwipsMeasure (1/20 pt), ST_HpsMeasure (1/2 pt), ST_EighthPointMeasure,
//! and the `*Chars` / `*Lines` attributes of `w:ind` and `w:spacing`
//! (hundredths of a character or line).

use std::fmt;
use std::sync::Arc;

use joyword_foundation::Emu;

use crate::id::PreservedId;
use crate::theme::ThemeColorSlot;

/// A length or size in its source unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Measure {
    Twips(i64),
    HalfPoints(i64),
    EighthPoints(i64),
    Emu(i64),
    /// Hundredths of a character width (e.g. `w:firstLineChars`).
    CharHundredths(i64),
    /// Hundredths of a line (e.g. `w:beforeLines`).
    LineHundredths(i64),
    /// Fiftieths of a percent (`pct` widths).
    PercentFiftieths(i64),
    Auto,
}

impl Measure {
    /// Absolute lengths in EMU; relative units need layout context. Twips and
    /// half points convert exactly. An odd number of eighth points is half an
    /// EMU (1/8 pt = 1587.5 EMU) and rounds half away from zero.
    pub fn absolute(self) -> Option<Emu> {
        match self {
            Measure::Twips(v) => v.checked_mul(Emu::PER_TWIP).map(Emu::new),
            Measure::HalfPoints(v) => v.checked_mul(Emu::PER_POINT / 2).map(Emu::new),
            Measure::EighthPoints(v) => {
                let emu = v.checked_mul(Emu::PER_POINT)?;
                let (quotient, remainder) = (emu / 8, emu % 8);
                Some(Emu::new(if remainder.abs() >= 4 {
                    quotient + remainder.signum()
                } else {
                    quotient
                }))
            }
            Measure::Emu(v) => Some(Emu::new(v)),
            _ => None,
        }
    }
}

impl fmt::Display for Measure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Measure::Twips(v) => write!(f, "{v}tw"),
            Measure::HalfPoints(v) => write!(f, "{v}hp"),
            Measure::EighthPoints(v) => write!(f, "{v}ep"),
            Measure::Emu(v) => write!(f, "{v}emu"),
            Measure::CharHundredths(v) => write!(f, "{v}ch%"),
            Measure::LineHundredths(v) => write!(f, "{v}ln%"),
            Measure::PercentFiftieths(v) => write!(f, "{v}pct50"),
            Measure::Auto => f.write_str("auto"),
        }
    }
}

/// Toggle property: `None` inherits, `Some(false)` is an explicit "off".
/// Resolution against the style hierarchy follows the ECMA-376 toggle rules
/// (Part 1 §17.7) and belongs to style resolution, not storage.
pub type Toggle = Option<bool>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    Auto,
    Rgb(u8, u8, u8),
    /// `w:themeColor` plus optional `w:themeTint` / `w:themeShade` bytes.
    /// Resolution substitutes the scheme colour and applies ECMA-376
    /// §17.3.2.6. A missing scheme entry stays here.
    Theme {
        slot: ThemeColorSlot,
        tint: Option<u8>,
        shade: Option<u8>,
    },
}

impl Color {
    pub fn theme(slot: ThemeColorSlot) -> Self {
        Self::Theme {
            slot,
            tint: None,
            shade: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Underline {
    None,
    Single,
    Double,
    Thick,
    Dotted,
    Dash,
    Wave,
    Words,
}

/// Theme font reference (`w:asciiTheme` and friends).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThemeFont {
    MajorAscii,
    MajorHAnsi,
    MajorEastAsia,
    MajorBidi,
    MinorAscii,
    MinorHAnsi,
    MinorEastAsia,
    MinorBidi,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontRef {
    Named(Arc<str>),
    Theme(ThemeFont),
}

/// `w:hint`: which slot ambiguous characters use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontHint {
    Default,
    EastAsia,
    ComplexScript,
}

/// The four font slots of `w:rFonts` (LAYOUT-002).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FontSlots {
    pub ascii: Option<FontRef>,
    pub h_ansi: Option<FontRef>,
    pub east_asia: Option<FontRef>,
    pub complex_script: Option<FontRef>,
    pub hint: Option<FontHint>,
}

/// BCP 47 language tags of `w:lang`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Languages {
    pub latin: Option<Arc<str>>,
    pub east_asia: Option<Arc<str>>,
    pub bidi: Option<Arc<str>>,
}

/// Character properties (`w:rPr`) of the editable subset.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RunProps {
    pub bold: Toggle,
    pub italic: Toggle,
    pub strike: Toggle,
    pub hidden: Toggle,
    pub underline: Option<Underline>,
    /// Font size, normally [`Measure::HalfPoints`].
    pub size: Option<Measure>,
    pub fonts: FontSlots,
    pub color: Option<Color>,
    pub languages: Languages,
    /// Unexplained `w:rPr` children kept by the format layer.
    pub preserved: Option<PreservedId>,
    /// `true` when everything kept in `preserved` changes no advance and
    /// paints nothing, such as `w:szCs` or `w:kern`. Layout refuses other
    /// preserved character properties.
    pub preserved_layout_neutral: bool,
}

impl RunProps {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Justification {
    Start,
    Center,
    End,
    Both,
    Distribute,
}

/// `w:ind`. First-line and hanging indents are mutually exclusive in the
/// source; both are kept as written and validation does not reconcile them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Indent {
    pub start: Option<Measure>,
    pub end: Option<Measure>,
    pub first_line: Option<Measure>,
    pub hanging: Option<Measure>,
}

/// `w:spacing@line` with `w:lineRule`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineSpacing {
    /// Multiple of the line height in 240ths (240 = single).
    Auto(i64),
    Exact(Measure),
    AtLeast(Measure),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Spacing {
    pub before: Option<Measure>,
    pub after: Option<Measure>,
    pub before_auto: Toggle,
    pub after_auto: Toggle,
    pub line: Option<LineSpacing>,
}

/// List membership as written (`w:numPr`). `num_id` 0 suppresses numbering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NumberingRef {
    pub num_id: u32,
    pub level: u8,
}

/// `w:tab@val` (ECMA-376 Part 1 §17.18.84). `Clear` removes a stop at the
/// same position while styles are layered. `Num` is a list tab and is placed
/// as [`TabAlign::Left`]. `Bar` is stored and not painted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TabAlign {
    Left,
    Center,
    Right,
    Decimal,
    Bar,
    Num,
    Clear,
}

/// `w:tab@leader`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TabLeader {
    None,
    Dot,
    Hyphen,
    Underscore,
    MiddleDot,
    Heavy,
}

/// One `w:tab` in source units. `None` on [`ParaProps::tabs`] means inherit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TabStop {
    pub position: Measure,
    pub align: TabAlign,
    pub leader: TabLeader,
}

/// Layer `delta` onto inherited stops. A clear removes the stop at that
/// position; any other alignment replaces it. Positions compare as absolute
/// EMU when both convert, otherwise as the source measure.
pub fn merge_tab_stops(inherited: &[TabStop], delta: &[TabStop]) -> Vec<TabStop> {
    let mut out = inherited.to_vec();
    for stop in delta {
        let index = out
            .iter()
            .position(|existing| same_tab_position(existing, stop));
        if stop.align == TabAlign::Clear {
            if let Some(index) = index {
                out.remove(index);
            }
        } else if let Some(index) = index {
            out[index] = *stop;
        } else {
            out.push(*stop);
        }
    }
    out.retain(|stop| stop.align != TabAlign::Clear);
    out.sort_by_key(|stop| {
        stop.position
            .absolute()
            .map(|length| length.raw())
            .unwrap_or(i64::MAX)
    });
    out
}

fn same_tab_position(left: &TabStop, right: &TabStop) -> bool {
    match (left.position.absolute(), right.position.absolute()) {
        (Some(left), Some(right)) => left == right,
        _ => left.position == right.position,
    }
}

/// Paragraph properties (`w:pPr`) of the editable subset.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ParaProps {
    pub justification: Option<Justification>,
    pub indent: Indent,
    pub spacing: Spacing,
    pub keep_next: Toggle,
    pub keep_lines: Toggle,
    pub widow_control: Toggle,
    pub page_break_before: Toggle,
    pub snap_to_grid: Toggle,
    pub outline_level: Option<u8>,
    pub numbering: Option<NumberingRef>,
    /// `None` inherits. `Some` is the full stop list after this layer's delta
    /// has been merged, including an explicit empty list.
    pub tabs: Option<Vec<TabStop>>,
    /// Unexplained `w:pPr` children kept by the format layer.
    pub preserved: Option<PreservedId>,
}

impl ParaProps {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_units_convert_exactly_and_relative_units_wait_for_layout() {
        assert_eq!(
            Measure::Twips(1440).absolute(),
            Some(Emu::new(Emu::PER_INCH))
        );
        assert_eq!(Measure::HalfPoints(21).absolute().unwrap().points(), 10.5);
        assert_eq!(Measure::EighthPoints(4).absolute().unwrap().points(), 0.5);
        assert_eq!(Measure::EighthPoints(1).absolute(), Some(Emu::new(1_588)));
        assert_eq!(Measure::EighthPoints(-1).absolute(), Some(Emu::new(-1_588)));
        assert_eq!(Measure::CharHundredths(200).absolute(), None);
        assert_eq!(Measure::LineHundredths(50).absolute(), None);
        assert_eq!(Measure::Auto.absolute(), None);
        assert_eq!(Measure::Twips(i64::MAX).absolute(), None);
    }

    #[test]
    fn explicit_off_differs_from_unset() {
        let unset = RunProps::default();
        let off = RunProps {
            bold: Some(false),
            ..RunProps::default()
        };
        assert!(unset.is_empty());
        assert!(!off.is_empty());
        assert_ne!(unset, off);
    }
}
