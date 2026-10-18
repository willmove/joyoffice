//! Evidence-scoped Chinese layout calculations (T207, LAYOUT-008).
//!
//! These functions do not paginate. [`crate::layout_document`] and
//! [`crate::wrap_mixed`] keep the T105 break path until a later change wires
//! a result in. Unsupported pairs return [`CjkIssue`]; rule extrapolations
//! carry explicit inferred evidence instead of claiming a measured point.
//!
//! Sources: `docs/joyword/layout_rules.md` R1, R3, R4, R7 and R9 (Word 2024
//! build 16.0.20430.20182, 2026-10-18), and ECMA-376 Part 1 5th edition
//! `*Chars` / `*Lines` attributes (hundredths). The line grid pitch is the
//! measured `w:docGrid w:linePitch="312"`.

use std::fmt;

use joyword_foundation::Emu;
use joyword_model::ThemeFont;
use read_fonts::TableProvider;

use crate::{FontError, FontKey, LoadedFont};

/// Word 2024 default line grid on the measured pages, in twips (15.6 pt).
pub const MEASURED_LINE_PITCH_TWIPS: i64 = 312;

/// Hard ceiling for one justification request (at most 512 KiB of gaps).
pub const MAX_JUSTIFY_ADVANCES: usize = 65_536;

const QUOTE_NUMERATOR: i64 = 431;
const QUOTE_DENOMINATOR: i64 = 1200;
const SINGLE_NUMERATOR: i64 = 13;
const SINGLE_DENOMINATOR: i64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Evidence {
    /// A registered Word 2024 rule, at a size that rule measured.
    Measured(&'static str),
    /// The same em fraction as a measured rule, at another size.
    Scaled {
        rule: &'static str,
        measured_size: Emu,
    },
    /// Calculation inferred from a registered rule; not a measured input point.
    Inferred(&'static str),
    /// Caller-supplied unit. Not a Word measurement.
    SuppliedBasis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CjkIssue {
    UnverifiedCharUnit,
    UnverifiedLineUnit,
    UnverifiedAutoSpace,
    UnverifiedAdjustRightInd,
    UnverifiedPunctuation,
    UnverifiedJustify,
    NotPunctuation,
    NotEastAsiaSlot,
    MissingFont,
    MissingGlyph,
    IllegalGrid,
    IllegalMeasure,
    DoesNotFit,
    Overflow,
    BudgetExceeded,
}

impl fmt::Display for CjkIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnverifiedCharUnit => "character-unit basis is not a measured Word rule",
            Self::UnverifiedLineUnit => "line-unit basis is not a measured Word rule",
            Self::UnverifiedAutoSpace => "CJK/Latin auto-spacing width is not measured",
            Self::UnverifiedAdjustRightInd => "adjustRightInd adjustment is not measured",
            Self::UnverifiedPunctuation => "this punctuation pair has no Word measurement",
            Self::UnverifiedJustify => "this line has no measured inter-character gap",
            Self::NotPunctuation => "character is not punctuation",
            Self::NotEastAsiaSlot => "theme slot is not an East Asian font",
            Self::MissingFont => "requested CJK family has no face",
            Self::MissingGlyph => "face is missing a glyph",
            Self::IllegalGrid => "document grid pitch is not a positive length",
            Self::IllegalMeasure => "length or font metric is not usable",
            Self::DoesNotFit => "advances are wider than the content box",
            Self::BudgetExceeded => "CJK calculation exceeds the line budget",
            Self::Overflow => "length calculation overflowed",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub length: Emu,
    pub evidence: Evidence,
}

pub type CjkResult = Result<Resolved, CjkIssue>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineGrid {
    pub pitch: Emu,
    pub snap: bool,
}

/// Non-negative vertical metrics in font units. `descent` is a magnitude.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerticalSpan {
    pub units_per_em: u16,
    pub ascent: i32,
    pub descent: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MetricSet {
    pub units_per_em: u16,
    pub hhea: VerticalSpan,
    pub typo: VerticalSpan,
    pub win: VerticalSpan,
}

/// The span R4's formula uses. hhea and typo stay available for comparison.
pub fn word_line_span(set: &MetricSet) -> VerticalSpan {
    set.win
}

/// hhea, OS/2 typo and OS/2 win spans of one face.
pub fn metric_set(font: &LoadedFont) -> Result<MetricSet, FontError> {
    let os2 = font
        .font_ref()
        .os2()
        .map_err(|_| FontError::MissingTable("OS/2"))?;
    let units = font.units_per_em;
    Ok(MetricSet {
        units_per_em: units,
        hhea: VerticalSpan {
            units_per_em: units,
            ascent: i32::from(font.ascender),
            descent: magnitude(i32::from(font.descender)),
        },
        typo: VerticalSpan {
            units_per_em: units,
            ascent: i32::from(os2.s_typo_ascender()),
            descent: magnitude(i32::from(os2.s_typo_descender())),
        },
        win: VerticalSpan {
            units_per_em: units,
            ascent: i32::from(os2.us_win_ascent()),
            descent: i32::from(os2.us_win_descent()),
        },
    })
}

fn magnitude(value: i32) -> i32 {
    value.saturating_abs()
}

/// Natural single-spacing height: 1.3 × win span × font size (R4).
/// The fitted formula is inferred: this API cannot identify the measured face.
pub fn natural_line_extent(size: Emu, win: VerticalSpan) -> CjkResult {
    if size.raw() <= 0 || win.units_per_em == 0 || win.ascent < 0 || win.descent < 0 {
        return Err(CjkIssue::IllegalMeasure);
    }
    let span = i64::from(win.ascent)
        .checked_add(i64::from(win.descent))
        .ok_or(CjkIssue::Overflow)?;
    if span == 0 {
        return Err(CjkIssue::IllegalMeasure);
    }
    let numer = span
        .checked_mul(SINGLE_NUMERATOR)
        .ok_or(CjkIssue::Overflow)?;
    let denom = i64::from(win.units_per_em)
        .checked_mul(SINGLE_DENOMINATOR)
        .ok_or(CjkIssue::Overflow)?;
    let length = mul_div_round(size.raw(), numer, denom).ok_or(CjkIssue::Overflow)?;
    Ok(Resolved {
        length: Emu::new(length),
        evidence: Evidence::Inferred("R4"),
    })
}

/// Snap a natural line height up to a whole number of grid lines (R4).
/// Arbitrary caller-supplied heights and pitches carry inferred evidence.
/// `snap == false` returns `natural` and ignores `pitch`.
pub fn snap_line_extent(natural: Emu, grid: LineGrid) -> CjkResult {
    if natural.raw() < 0 {
        return Err(CjkIssue::IllegalMeasure);
    }
    if !grid.snap {
        return Ok(Resolved {
            length: natural,
            evidence: Evidence::Inferred("R4"),
        });
    }
    if grid.pitch.raw() <= 0 {
        return Err(CjkIssue::IllegalGrid);
    }
    let length = ceil_to_multiple(natural.raw(), grid.pitch.raw()).ok_or(CjkIssue::Overflow)?;
    Ok(Resolved {
        length: Emu::new(length),
        evidence: Evidence::Inferred("R4"),
    })
}

pub fn single_line_height(size: Emu, win: VerticalSpan, grid: LineGrid) -> CjkResult {
    let natural = natural_line_extent(size, win)?;
    snap_line_extent(natural.length, grid)
}

pub fn measured_line_grid(snap: bool) -> Result<LineGrid, CjkIssue> {
    let pitch = Emu::from_twips(MEASURED_LINE_PITCH_TWIPS).ok_or(CjkIssue::Overflow)?;
    Ok(LineGrid { pitch, snap })
}

/// Advance of one punctuation character for the DengXian R7 profile.
///
/// The caller must establish the DengXian face; this function cannot identify
/// a font from an em length. Only observed consecutive-mark pairs are accepted.
/// Han followers use the measured class rule (unobserved individual pairs are
/// labelled inferred). Other followers, including end-of-line, are unverified.
/// Sizes other than 12 pt scale the measured fraction, not a new measurement.
pub fn punctuation_advance(ch: char, next: Option<char>, em: Emu) -> CjkResult {
    if em.raw() <= 0 {
        return Err(CjkIssue::IllegalMeasure);
    }
    if !is_unmeasured_punctuation(ch) {
        return Err(CjkIssue::NotPunctuation);
    }
    let Some(next) = next else {
        return Err(CjkIssue::UnverifiedPunctuation);
    };
    let observed = matches!(
        (ch, next),
        ('“', '成' | '这')
            | ('”', '和' | '，')
            | ('：', '：' | '；' | '甲' | '好' | '为' | '因')
            | ('；', '。')
            | ('。', '，' | '（')
            | ('，', '、' | '其' | '用')
            | ('、', '以' | '逗')
            | ('（', '括' | '同')
            | ('）', '：' | '。' | '他')
            | ('！', '再')
            | ('？', '答')
    );
    let evidence = if observed {
        r7_evidence(em)
    } else {
        Evidence::Inferred("R7")
    };
    let han_follower = script_run(next) == ScriptRun::Han;
    if is_measured_quote(ch) && (han_follower || observed) {
        return fraction_of_em(em, QUOTE_NUMERATOR, QUOTE_DENOMINATOR, evidence);
    }
    if measured_compressible(ch) && observed && measured_punct_follower(next) {
        return fraction_of_em(em, 1, 2, evidence);
    }
    if measured_full(ch) && han_follower {
        return fraction_of_em(em, 1, 1, evidence);
    }
    Err(CjkIssue::UnverifiedPunctuation)
}

fn r7_evidence(em: Emu) -> Evidence {
    let measured = Emu::new(12 * Emu::PER_POINT);
    if em == measured {
        Evidence::Measured("R7")
    } else {
        Evidence::Scaled {
            rule: "R7",
            measured_size: measured,
        }
    }
}

fn is_measured_quote(ch: char) -> bool {
    matches!(ch, '\u{201C}' | '\u{201D}')
}

fn measured_compressible(ch: char) -> bool {
    matches!(ch, '：' | '；' | '。' | '，' | '、' | '）')
}

fn measured_punct_follower(ch: char) -> bool {
    matches!(ch, '：' | '；' | '。' | '，' | '、' | '（' | '）')
}

fn measured_full(ch: char) -> bool {
    matches!(
        ch,
        '：' | '；' | '。' | '，' | '、' | '（' | '）' | '！' | '？'
    )
}

fn is_unmeasured_punctuation(ch: char) -> bool {
    ch.is_ascii_punctuation()
        || matches!(
            ch,
            '\u{2018}'..='\u{201F}'
                | '\u{3000}'..='\u{303F}'
                | '\u{FF01}'..='\u{FF0F}'
                | '\u{FF1A}'..='\u{FF20}'
                | '\u{FF3B}'..='\u{FF40}'
                | '\u{FF5B}'..='\u{FF65}'
        )
}

/// Extra gaps inserted between advances so a justified line fills `content_width`.
///
/// R3: a pure Chinese justified line reaches the right edge, and the last line
/// of the paragraph is not stretched. The fixture records the line edge, not
/// each gap; equal split (leftover EMU on the earlier gaps) is how the total
/// is met. A line with no inter-character gap is not given an edge gap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Justification {
    pub gaps: Vec<Emu>,
    pub evidence: Evidence,
}

pub fn justify_intercharacter(
    advances: &[Emu],
    content_width: Emu,
    last_line: bool,
) -> Result<Justification, CjkIssue> {
    if advances.is_empty() || content_width.raw() < 0 {
        return Err(CjkIssue::IllegalMeasure);
    }
    // Bound work and output allocation before scanning advances.
    if advances.len() > MAX_JUSTIFY_ADVANCES {
        return Err(CjkIssue::BudgetExceeded);
    }
    if advances.iter().any(|advance| advance.raw() < 0) {
        return Err(CjkIssue::IllegalMeasure);
    }
    let used = advances
        .iter()
        .try_fold(0i64, |sum, advance| sum.checked_add(advance.raw()))
        .ok_or(CjkIssue::Overflow)?;
    let extra = content_width
        .raw()
        .checked_sub(used)
        .ok_or(CjkIssue::Overflow)?;
    if extra < 0 {
        return Err(CjkIssue::DoesNotFit);
    }
    let gap_count = advances.len() - 1;
    if last_line || extra == 0 {
        return Ok(Justification {
            gaps: vec![Emu::new(0); gap_count],
            evidence: Evidence::Inferred("R3"),
        });
    }
    if gap_count == 0 {
        return Err(CjkIssue::UnverifiedJustify);
    }
    let gap_count_i = i64::try_from(gap_count).map_err(|_| CjkIssue::Overflow)?;
    let base = extra / gap_count_i;
    let remainder = extra % gap_count_i;
    let mut gaps = Vec::with_capacity(gap_count);
    for index in 0..gap_count {
        let index_i = i64::try_from(index).map_err(|_| CjkIssue::Overflow)?;
        let add = if index_i < remainder {
            base.checked_add(1).ok_or(CjkIssue::Overflow)?
        } else {
            base
        };
        gaps.push(Emu::new(add));
    }
    Ok(Justification {
        gaps,
        evidence: Evidence::Inferred("R3"),
    })
}

/// `hundredths` of `unit`. The unit is the caller's; this does not decide
/// what one character or one line is in Word.
pub fn scale_hundredths(hundredths: i64, unit: Emu) -> CjkResult {
    if unit.raw() <= 0 {
        return Err(CjkIssue::IllegalMeasure);
    }
    let length = mul_div_round(unit.raw(), hundredths, 100).ok_or(CjkIssue::Overflow)?;
    Ok(Resolved {
        length: Emu::new(length),
        evidence: Evidence::SuppliedBasis,
    })
}

pub fn resolve_char_hundredths(hundredths: i64, east_asia_em: Option<Emu>) -> CjkResult {
    match east_asia_em {
        Some(em) => scale_hundredths(hundredths, em),
        None => Err(CjkIssue::UnverifiedCharUnit),
    }
}

pub fn resolve_line_hundredths(hundredths: i64, line: Option<Emu>) -> CjkResult {
    match line {
        Some(line) => scale_hundredths(hundredths, line),
        None => Err(CjkIssue::UnverifiedLineUnit),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptRun {
    Han,
    Latin,
    Space,
    Other,
}

pub fn script_run(ch: char) -> ScriptRun {
    if matches!(ch, '\u{3400}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}') {
        ScriptRun::Han
    } else if ch.is_ascii_alphabetic() {
        ScriptRun::Latin
    } else if ch == ' ' {
        ScriptRun::Space
    } else {
        ScriptRun::Other
    }
}

pub fn needs_auto_space(left: ScriptRun, right: ScriptRun) -> bool {
    matches!(
        (left, right),
        (ScriptRun::Han, ScriptRun::Latin) | (ScriptRun::Latin, ScriptRun::Han)
    )
}

pub fn auto_space_width() -> CjkResult {
    Err(CjkIssue::UnverifiedAutoSpace)
}

pub fn adjust_right_indent() -> CjkResult {
    Err(CjkIssue::UnverifiedAdjustRightInd)
}

pub fn diagnose_missing_glyphs(count: usize) -> Result<(), CjkIssue> {
    if count == 0 {
        Ok(())
    } else {
        Err(CjkIssue::MissingGlyph)
    }
}

/// Closing punctuation must not start a line. The set is the one already
/// matched to Word on the R2 fixtures; R9 saw no hanging punctuation.
pub fn prohibits_line_start(ch: char) -> bool {
    matches!(
        ch,
        '，' | '。'
            | '！'
            | '？'
            | '；'
            | '：'
            | '、'
            | '）'
            | '】'
            | '》'
            | '」'
            | '』'
            | ','
            | '.'
            | '!'
            | '?'
            | ';'
            | ':'
            | ')'
    )
}

/// Opening punctuation must not end a line. Same measured set as
/// [`prohibits_line_start`].
pub fn prohibits_line_end(ch: char) -> bool {
    matches!(ch, '（' | '【' | '《' | '「' | '『' | '(' | '[')
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CjkFamily {
    SimSun,
    FangSong,
    KaiTi,
    SimHei,
    DengXian,
    MicrosoftYaHei,
}

impl CjkFamily {
    pub const ALL: [Self; 6] = [
        Self::SimSun,
        Self::FangSong,
        Self::KaiTi,
        Self::SimHei,
        Self::DengXian,
        Self::MicrosoftYaHei,
    ];

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::SimSun => "宋体",
            Self::FangSong => "仿宋",
            Self::KaiTi => "楷体",
            Self::SimHei => "黑体",
            Self::DengXian => "等线",
            Self::MicrosoftYaHei => "微软雅黑",
        }
    }

    fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::SimSun => &["宋体", "SimSun"],
            Self::FangSong => &["仿宋", "FangSong"],
            Self::KaiTi => &["楷体", "KaiTi"],
            Self::SimHei => &["黑体", "SimHei"],
            Self::DengXian => &["等线", "DengXian"],
            Self::MicrosoftYaHei => &["微软雅黑", "Microsoft YaHei", "MicrosoftYaHei"],
        }
    }
}

/// East Asian family Word 2024 used when the document named none (R1).
pub fn default_east_asia_family() -> CjkFamily {
    CjkFamily::DengXian
}

pub fn parse_cjk_family(name: &str) -> Option<CjkFamily> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    CjkFamily::ALL.into_iter().find(|family| {
        family.aliases().iter().any(|alias| {
            if alias.is_ascii() {
                alias.eq_ignore_ascii_case(name)
            } else {
                *alias == name
            }
        })
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AvailableFace {
    pub family: CjkFamily,
    pub key: FontKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontMatch {
    Exact { family: CjkFamily, key: FontKey },
    Missing { family: CjkFamily },
}

/// Match a requested family to a face of that same family. A different
/// installed family is not a substitute.
pub fn match_family(family: CjkFamily, available: &[AvailableFace]) -> FontMatch {
    match available.iter().find(|face| face.family == family) {
        Some(face) => FontMatch::Exact {
            family,
            key: face.key,
        },
        None => FontMatch::Missing { family },
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EastAsiaTheme {
    pub major: Option<CjkFamily>,
    pub minor: Option<CjkFamily>,
}

pub fn theme_east_asia(slot: ThemeFont, theme: EastAsiaTheme) -> Result<CjkFamily, CjkIssue> {
    let family = match slot {
        ThemeFont::MajorEastAsia => theme.major,
        ThemeFont::MinorEastAsia => theme.minor,
        _ => return Err(CjkIssue::NotEastAsiaSlot),
    };
    family.ok_or(CjkIssue::MissingFont)
}

fn fraction_of_em(em: Emu, numer: i64, denom: i64, evidence: Evidence) -> CjkResult {
    let length = mul_div_round(em.raw(), numer, denom).ok_or(CjkIssue::Overflow)?;
    Ok(Resolved {
        length: Emu::new(length),
        evidence,
    })
}

fn mul_div_round(value: i64, numer: i64, denom: i64) -> Option<i64> {
    if denom <= 0 {
        return None;
    }
    let prod = i128::from(value).checked_mul(i128::from(numer))?;
    let denom = i128::from(denom);
    let half = denom / 2;
    let adjusted = if prod >= 0 {
        prod.checked_add(half)?
    } else {
        prod.checked_sub(half)?
    };
    i64::try_from(adjusted / denom).ok()
}

fn ceil_to_multiple(value: i64, step: i64) -> Option<i64> {
    if value < 0 || step <= 0 {
        return None;
    }
    let value = u128::try_from(value).ok()?;
    let step = u128::try_from(step).ok()?;
    let product = value.div_ceil(step).checked_mul(step)?;
    i64::try_from(product).ok()
}
