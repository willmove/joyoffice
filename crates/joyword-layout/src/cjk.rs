//! Evidence-scoped Chinese layout calculations (T207, LAYOUT-008).
//!
//! Pure functions return [`CjkIssue`] for unsupported pairs and label
//! extrapolations as inferred. [`compress_shaped_run`] is the only adapter
//! that rewrites a shaped run: [`crate::layout_document`] and
//! [`crate::wrap_mixed`] call it so the shared page scene uses a measured
//! width only when the face name is DengXian and the width is narrower than
//! the shaper. Other faces and unverified pairs keep the shaped advance.
//!
//! Sources: `docs/joyword/layout_rules.md` R1, R3, R4, R7 and R9 (Word 2024
//! build 16.0.20430.20182, 2026-10-18), and ECMA-376 Part 1 5th edition
//! `*Chars` / `*Lines` attributes (hundredths). The line grid pitch is the
//! measured `w:docGrid w:linePitch="312"`.

use std::fmt;

use joyword_foundation::Emu;
use joyword_model::{DocGrid, DocGridKind, ThemeFont};
use read_fonts::TableProvider;

use crate::shape::ShapedRun;
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

/// Line grid for one section. A positive `linePitch` on `lines` or
/// `linesAndChars` replaces the measured 312 twip default. `snapToChars`,
/// an omitted type, and a missing element keep that default. `charSpace`
/// is not a line pitch. Evidence stays [`Evidence::Inferred`], because only
/// the 312 twip grid was fitted to Word.
pub fn line_grid_for(grid: Option<DocGrid>, snap: bool) -> Result<LineGrid, CjkIssue> {
    if !snap {
        return measured_line_grid(false);
    }
    let pitch_twips = match grid {
        Some(DocGrid {
            kind: Some(DocGridKind::Lines | DocGridKind::LinesAndChars),
            line_pitch: Some(pitch),
            ..
        }) => {
            if pitch <= 0 {
                return Err(CjkIssue::IllegalGrid);
            }
            pitch
        }
        _ => MEASURED_LINE_PITCH_TWIPS,
    };
    let pitch = Emu::from_twips(pitch_twips).ok_or(CjkIssue::Overflow)?;
    Ok(LineGrid { pitch, snap: true })
}

/// Twips of [`line_grid_for`] with snapping on. Cache keys store this value.
pub fn snapped_pitch_twips(grid: Option<DocGrid>) -> Result<i64, CjkIssue> {
    match line_grid_for(grid, true)?
        .pitch
        .raw()
        .checked_div(Emu::PER_TWIP)
    {
        Some(twips) => Ok(twips),
        None => Err(CjkIssue::Overflow),
    }
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

/// One glyph offered to [`justify_line`]: the first scalar of its cluster,
/// the cluster's byte offset and the shaped advance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JustifyGlyph {
    pub ch: char,
    pub cluster: usize,
    pub advance: Emu,
}

/// Extra width after each glyph of one `both` line, indexed like `glyphs`.
///
/// R3 measured pure Chinese lines, where every inter-character gap takes an
/// equal share; such a line gets exactly the gaps of
/// [`justify_intercharacter`]. Mixed lines are inferred, not measured: only
/// gaps that touch a CJK character and the gap after each space take a share,
/// so Latin words and numbers keep their shaped spacing. Trailing whitespace
/// hangs past the edge without a share. A line with no such gap is
/// [`CjkIssue::UnverifiedJustify`] and stays as shaped.
pub fn justify_line(
    glyphs: &[JustifyGlyph],
    content_width: Emu,
    last_line: bool,
) -> Result<Vec<Emu>, CjkIssue> {
    if glyphs.len() > MAX_JUSTIFY_ADVANCES {
        return Err(CjkIssue::BudgetExceeded);
    }
    if glyphs.iter().any(|glyph| glyph.advance.raw() < 0) {
        return Err(CjkIssue::IllegalMeasure);
    }
    let mut gaps = vec![Emu::new(0); glyphs.len()];
    let Some(end) = glyphs.iter().rposition(|glyph| !glyph.ch.is_whitespace()) else {
        return Ok(gaps);
    };
    let mut segments = Vec::new();
    let mut segment_ends = Vec::new();
    let mut width = 0i64;
    for (index, glyph) in glyphs[..=end].iter().enumerate() {
        width = width
            .checked_add(glyph.advance.raw())
            .ok_or(CjkIssue::Overflow)?;
        let closes = match glyphs.get(index + 1) {
            Some(next) if index < end => {
                next.cluster != glyph.cluster
                    && (glyph.ch.is_whitespace()
                        || justifies_between(glyph.ch)
                        || justifies_between(next.ch))
            }
            _ => true,
        };
        if closes {
            segments.push(Emu::new(width));
            segment_ends.push(index);
            width = 0;
        }
    }
    let justified = justify_intercharacter(&segments, content_width, last_line)?;
    for (gap, index) in justified.gaps.into_iter().zip(segment_ends) {
        gaps[index] = gap;
    }
    Ok(gaps)
}

/// [`justify_line`] over `indexes` of `run`, in that order. A glyph whose
/// cluster offset is not a scalar boundary of the run text is
/// [`CjkIssue::UnverifiedJustify`].
pub fn justify_run(
    run: &ShapedRun,
    indexes: &[usize],
    content_width: Emu,
    last_line: bool,
) -> Result<Vec<Emu>, CjkIssue> {
    let glyphs = indexes
        .iter()
        .map(|index| {
            let glyph = run.glyphs.get(*index)?;
            let ch = run.text.get(glyph.cluster..)?.chars().next()?;
            Some(JustifyGlyph {
                ch,
                cluster: glyph.cluster,
                advance: glyph.advance,
            })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(CjkIssue::UnverifiedJustify)?;
    justify_line(&glyphs, content_width, last_line)
}

/// Han, kana, Hangul, CJK and full-width punctuation, and the curly quotes
/// R3 lines carry. Gaps beside these take a share of a justified line.
fn justifies_between(ch: char) -> bool {
    script_run(ch) == ScriptRun::Han
        || matches!(
            ch,
            '\u{2018}'
                | '\u{2019}'
                | '\u{201C}'
                | '\u{201D}'
                | '\u{2E80}'..='\u{2FFF}'
                | '\u{3000}'..='\u{33FF}'
                | '\u{AC00}'..='\u{D7AF}'
                | '\u{FE30}'..='\u{FE4F}'
                | '\u{FF00}'..='\u{FFEF}'
                | '\u{20000}'..='\u{3FFFF}'
        )
        || (!ch.is_ascii() && (prohibits_line_start(ch) || prohibits_line_end(ch)))
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

/// Families whose single-spacing height was fitted in R4.
///
/// 宋体、仿宋、楷体、等线 and 微软雅黑 are in the measured set. 黑体 is not,
/// and neither is a face whose name does not resolve to one of these.
pub fn measured_line_family(family: CjkFamily) -> bool {
    matches!(
        family,
        CjkFamily::SimSun
            | CjkFamily::FangSong
            | CjkFamily::KaiTi
            | CjkFamily::DengXian
            | CjkFamily::MicrosoftYaHei
    )
}

/// Family of `font` from name IDs 16 then 1. A name that is not one of the
/// six CJK families returns `None`; this does not load or substitute a face.
pub fn face_cjk_family(font: &LoadedFont) -> Option<CjkFamily> {
    use read_fonts::types::NameId;
    let table = font.font_ref().name().ok()?;
    let data = table.string_data();
    let mut typographic = None;
    let mut family = None;
    for record in table.name_record() {
        let id = record.name_id();
        if id != NameId::TYPOGRAPHIC_FAMILY_NAME && id != NameId::FAMILY_NAME {
            continue;
        }
        let Ok(value) = record.string(data) else {
            continue;
        };
        let parsed = parse_cjk_family(value.to_string().trim());
        if id == NameId::TYPOGRAPHIC_FAMILY_NAME {
            if typographic.is_none() {
                typographic = parsed;
            }
        } else if family.is_none() {
            family = parsed;
        }
    }
    typographic.or(family)
}

/// Replace a DengXian cluster advance when R7's width is narrower than the
/// shaper. Full-width and unverified results keep `shaped`, so a line that
/// already matched Word at the font's own full width does not move.
pub(crate) fn compressed_cluster_advance(
    ch: char,
    next: Option<char>,
    shaped: Emu,
    size: Emu,
    family: Option<CjkFamily>,
) -> Emu {
    if family != Some(CjkFamily::DengXian) {
        return shaped;
    }
    match punctuation_advance(ch, next, size) {
        Ok(resolved) if resolved.length.raw() < shaped.raw() => resolved.length,
        _ => shaped,
    }
}

/// Apply [`compressed_cluster_advance`] to one-glyph clusters in `run`.
///
/// `paragraph[slice_at..]` is `run.text`. The follower is the next scalar in
/// the paragraph, including one that will sit on the next line. A cluster
/// that is not one glyph for one scalar is left unchanged.
pub(crate) fn compress_shaped_run(
    paragraph: &str,
    slice_at: usize,
    run: &mut ShapedRun,
    family: Option<CjkFamily>,
) -> Result<(), CjkIssue> {
    if family != Some(CjkFamily::DengXian) || run.glyphs.is_empty() {
        return Ok(());
    }
    let slice_end = slice_at
        .checked_add(run.text.len())
        .ok_or(CjkIssue::Overflow)?;
    if paragraph.get(slice_at..slice_end) != Some(run.text.as_str()) {
        return Ok(());
    }
    let mut replacements = Vec::new();
    let mut glyph_index = 0;
    for (byte, ch) in run.text.char_indices() {
        while glyph_index < run.glyphs.len() && run.glyphs[glyph_index].cluster < byte {
            glyph_index += 1;
        }
        let start = glyph_index;
        while glyph_index < run.glyphs.len() && run.glyphs[glyph_index].cluster == byte {
            glyph_index += 1;
        }
        if glyph_index != start + 1 {
            continue;
        }
        let next_at = slice_at + byte + ch.len_utf8();
        let next = paragraph[next_at..].chars().next();
        let shaped = run.glyphs[start].advance;
        let advance = compressed_cluster_advance(ch, next, shaped, run.size, family);
        if advance != shaped {
            replacements.push((start, advance));
        }
    }
    if replacements.is_empty() {
        return Ok(());
    }
    for (index, advance) in replacements {
        run.glyphs[index].advance = advance;
    }
    let mut pen = 0i64;
    for glyph in &mut run.glyphs {
        glyph.x = Emu::new(pen);
        pen = pen
            .checked_add(glyph.advance.raw())
            .ok_or(CjkIssue::Overflow)?;
    }
    run.width = Emu::new(pen);
    Ok(())
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

// =========================================================================
// GB/T 9704-2012 Official Document & Advanced CJK Layout (T504)
// =========================================================================

/// GB/T 9704-2012 official document typography specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gbt9704Spec;

impl Gbt9704Spec {
    /// A4 paper width: 210 mm (11,906 twips).
    pub const PAGE_WIDTH_TWIPS: i64 = 11_906;
    /// A4 paper height: 297 mm (16,838 twips).
    pub const PAGE_HEIGHT_TWIPS: i64 = 16_838;
    /// Top margin (天头): 37 mm (2,098 twips).
    pub const TOP_MARGIN_TWIPS: i64 = 2_098;
    /// Bottom margin (地脚): 35 mm (1_984 twips).
    pub const BOTTOM_MARGIN_TWIPS: i64 = 1_984;
    /// Left margin (订口): 28 mm (1,587 twips).
    pub const LEFT_MARGIN_TWIPS: i64 = 1_587;
    /// Right margin (切口): 26 mm (1,474 twips).
    pub const RIGHT_MARGIN_TWIPS: i64 = 1_474;
    /// Standard lines per page: 22 lines.
    pub const LINES_PER_PAGE: u32 = 22;
    /// Standard characters per line: 28 characters.
    pub const CHARS_PER_LINE: u32 = 28;
    /// Line pitch: 580 twips (~29 pt, 12,756 twips / 22 lines).
    pub const LINE_PITCH_TWIPS: i64 = 580;
    /// Character pitch: 316 twips (~15.8 pt, 8,844 twips / 28 chars).
    pub const CHAR_PITCH_TWIPS: i64 = 316;
    /// Print area width: 156 mm (8,844 twips).
    pub const PRINT_AREA_WIDTH_TWIPS: i64 = 8_844;
    /// Print area height: 225 mm (12,756 twips).
    pub const PRINT_AREA_HEIGHT_TWIPS: i64 = 12_756;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gbt9704Issue {
    PaperDimensionsMismatch {
        width: i64,
        height: i64,
    },
    MarginMismatch {
        edge: &'static str,
        actual: i64,
        expected: i64,
    },
    GridMissing,
    GridKindMismatch,
    LinePitchMismatch {
        expected: i64,
        actual: i64,
    },
}

impl fmt::Display for Gbt9704Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PaperDimensionsMismatch { width, height } => {
                write!(
                    f,
                    "paper dimensions ({width}x{height}) do not match GB/T 9704 A4"
                )
            }
            Self::MarginMismatch {
                edge,
                actual,
                expected,
            } => {
                write!(
                    f,
                    "{edge} margin {actual} twips does not match expected {expected} twips"
                )
            }
            Self::GridMissing => write!(f, "section lacks document grid"),
            Self::GridKindMismatch => write!(f, "document grid kind is not linesAndChars"),
            Self::LinePitchMismatch { expected, actual } => {
                write!(f, "line pitch {actual} does not match expected {expected}")
            }
        }
    }
}

pub fn measure_twips(m: &joyword_model::Measure) -> Option<i64> {
    match m {
        joyword_model::Measure::Twips(t) => Some(*t),
        joyword_model::Measure::HalfPoints(hp) => Some(hp * 10),
        joyword_model::Measure::Emu(e) => Some(e / 635),
        other => other.absolute().map(|emu| emu.raw() / 635),
    }
}

/// Verify that a section conforms to GB/T 9704-2012 layout geometry and grid.
pub fn verify_gbt9704_section(section: &joyword_model::Section) -> Result<(), Gbt9704Issue> {
    const TOLERANCE: i64 = 15;

    let w = measure_twips(&section.page.width).unwrap_or(0);
    let h = measure_twips(&section.page.height).unwrap_or(0);
    if (w - Gbt9704Spec::PAGE_WIDTH_TWIPS).abs() > TOLERANCE
        || (h - Gbt9704Spec::PAGE_HEIGHT_TWIPS).abs() > TOLERANCE
    {
        return Err(Gbt9704Issue::PaperDimensionsMismatch {
            width: w,
            height: h,
        });
    }

    let top = measure_twips(&section.page.top).unwrap_or(0);
    if (top - Gbt9704Spec::TOP_MARGIN_TWIPS).abs() > TOLERANCE {
        return Err(Gbt9704Issue::MarginMismatch {
            edge: "top",
            actual: top,
            expected: Gbt9704Spec::TOP_MARGIN_TWIPS,
        });
    }

    let bottom = measure_twips(&section.page.bottom).unwrap_or(0);
    if (bottom - Gbt9704Spec::BOTTOM_MARGIN_TWIPS).abs() > TOLERANCE {
        return Err(Gbt9704Issue::MarginMismatch {
            edge: "bottom",
            actual: bottom,
            expected: Gbt9704Spec::BOTTOM_MARGIN_TWIPS,
        });
    }

    let left = measure_twips(&section.page.left).unwrap_or(0);
    if (left - Gbt9704Spec::LEFT_MARGIN_TWIPS).abs() > TOLERANCE {
        return Err(Gbt9704Issue::MarginMismatch {
            edge: "left",
            actual: left,
            expected: Gbt9704Spec::LEFT_MARGIN_TWIPS,
        });
    }

    let right = measure_twips(&section.page.right).unwrap_or(0);
    if (right - Gbt9704Spec::RIGHT_MARGIN_TWIPS).abs() > TOLERANCE {
        return Err(Gbt9704Issue::MarginMismatch {
            edge: "right",
            actual: right,
            expected: Gbt9704Spec::RIGHT_MARGIN_TWIPS,
        });
    }

    let grid = section.doc_grid.as_ref().ok_or(Gbt9704Issue::GridMissing)?;
    if grid.kind != Some(DocGridKind::LinesAndChars) {
        return Err(Gbt9704Issue::GridKindMismatch);
    }
    let pitch = grid.line_pitch.unwrap_or(0);
    if (pitch - Gbt9704Spec::LINE_PITCH_TWIPS).abs() > TOLERANCE {
        return Err(Gbt9704Issue::LinePitchMismatch {
            expected: Gbt9704Spec::LINE_PITCH_TWIPS,
            actual: pitch,
        });
    }

    Ok(())
}

/// Creates a standard GB/T 9704-2012 official document sample containing
/// document headers, numbers, titles, body with first-line indent,
/// Ruby (phonetic guide), CombineCharacters (two-lines-in-one),
/// EmphasisMark, and EnclosedCharacter.
pub fn create_gbt9704_document_sample() -> joyword_model::Document {
    use joyword_model::{
        BlockSeq, CombineBrackets, CombineCharacters, EmphasisMark, EncloseShape,
        EnclosedCharacter, InlineKind, InlineObject, PageSetup, Paragraph, Ruby, RubyAlign,
    };
    use std::sync::Arc;

    let mut doc = joyword_model::Document::new(PageSetup::a4_gbt9704());
    doc.sections_mut()[0].doc_grid = Some(DocGrid::gbt9704());

    let mut blocks = Vec::new();

    // 1. 发文字号 (Issue Number)
    let mut p_num = Paragraph::new(doc.ids().node());
    let _ = p_num.push_text("发文字号：京发〔2026〕1号", None, Arc::default());
    blocks.push(p_num.into());

    // 2. 标题 (Title - 2号小标宋, 居中)
    let mut p_title = Paragraph::new(doc.ids().node());
    Arc::make_mut(&mut p_title.props).justification = Some(joyword_model::Justification::Center);
    let _ = p_title.push_text("关于推进政务信息系统建设的通知", None, Arc::default());
    blocks.push(p_title.into());

    // 3. 正文 (Body - 3号仿宋, 首行缩进2字符 / 632 twips)
    let mut p_body = Paragraph::new(doc.ids().node());
    Arc::make_mut(&mut p_body.props).indent.first_line = Some(joyword_model::Measure::Twips(632));
    let _ = p_body.push_text(
        "各区人民政府，市政府各委、办、局：为贯彻落实",
        None,
        Arc::default(),
    );

    // 嵌入 Ruby 拼音指南
    p_body.push_inline(
        InlineObject {
            id: doc.ids().node(),
            kind: InlineKind::Ruby(Ruby {
                base: "中华人民共和国".to_string(),
                text: "zhōng huá rén mín gòng hé guó".to_string(),
                align: RubyAlign::Distribute,
                font_size_hps: Some(18),
                raise_hps: Some(10),
            }),
            source: None,
        },
        None,
        Arc::default(),
    );

    let _ = p_body.push_text(
        "国家政务信息化发展规划，特提出以下要求：",
        None,
        Arc::default(),
    );
    blocks.push(p_body.into());

    // 4. 第一条 (包含带圈字符与着重号)
    let mut p_item1 = Paragraph::new(doc.ids().node());
    Arc::make_mut(&mut p_item1.props).indent.first_line = Some(joyword_model::Measure::Twips(632));
    p_item1.push_inline(
        InlineObject {
            id: doc.ids().node(),
            kind: InlineKind::Enclosed(EnclosedCharacter {
                text: "1".to_string(),
                shape: EncloseShape::Circle,
            }),
            source: None,
        },
        None,
        Arc::default(),
    );
    let _ = p_item1.push_text(" 健全数据安全体系，确保", None, Arc::default());
    p_item1.push_inline(
        InlineObject {
            id: doc.ids().node(),
            kind: InlineKind::Emphasis {
                text: "核心数据".to_string(),
                mark: EmphasisMark::Dot,
            },
            source: None,
        },
        None,
        Arc::default(),
    );
    let _ = p_item1.push_text("绝对安全可靠。", None, Arc::default());
    blocks.push(p_item1.into());

    // 5. 第二条 (包含双行合一)
    let mut p_item2 = Paragraph::new(doc.ids().node());
    Arc::make_mut(&mut p_item2.props).indent.first_line = Some(joyword_model::Measure::Twips(632));
    p_item2.push_inline(
        InlineObject {
            id: doc.ids().node(),
            kind: InlineKind::Combine(CombineCharacters {
                text: "办公室信息科".to_string(),
                brackets: CombineBrackets::Square,
            }),
            source: None,
        },
        None,
        Arc::default(),
    );
    let _ = p_item2.push_text("做好每日协调督办。", None, Arc::default());
    blocks.push(p_item2.into());

    // 6. 发文机关 (Right-aligned -> End)
    let mut p_issuer = Paragraph::new(doc.ids().node());
    Arc::make_mut(&mut p_issuer.props).justification = Some(joyword_model::Justification::End);
    let _ = p_issuer.push_text("北京市人民政府办公厅", None, Arc::default());
    blocks.push(p_issuer.into());

    // 7. 发文日期 (Right-aligned -> End)
    let mut p_date = Paragraph::new(doc.ids().node());
    Arc::make_mut(&mut p_date.props).justification = Some(joyword_model::Justification::End);
    let _ = p_date.push_text("2026年10月28日", None, Arc::default());
    blocks.push(p_date.into());

    let body = doc.body_id();
    if let Some(story) = doc.story_mut(body) {
        story.blocks = BlockSeq::from_vec(blocks);
    }

    doc
}

/// Manuscript paper (稿纸) specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManuscriptPaperSpec {
    pub rows: u32,
    pub cols: u32,
    pub line_pitch_twips: i64,
    pub char_pitch_twips: i64,
}

impl ManuscriptPaperSpec {
    pub fn standard_400() -> Self {
        Self {
            rows: 20,
            cols: 20,
            line_pitch_twips: 600,
            char_pitch_twips: 440,
        }
    }

    pub fn standard_500() -> Self {
        Self {
            rows: 20,
            cols: 25,
            line_pitch_twips: 580,
            char_pitch_twips: 350,
        }
    }
}

/// Script classification for typography & layout restrictions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptKind {
    Common,
    Latin,
    EastAsian,
    BiDiRtl,
    ComplexIndic,
}

/// Classify a character for typography handling.
pub fn classify_script(ch: char) -> ScriptKind {
    let u = ch as u32;
    match u {
        0x0590..=0x05FF
        | 0x0600..=0x06FF
        | 0x0750..=0x077F
        | 0x08A0..=0x08FF
        | 0xFB50..=0xFDFF
        | 0xFE70..=0xFEFF => ScriptKind::BiDiRtl,
        0x0900..=0x0E7F => ScriptKind::ComplexIndic,
        0x3000..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2EBEF => ScriptKind::EastAsian,
        0x0000..=0x024F => ScriptKind::Latin,
        _ => ScriptKind::Common,
    }
}

/// Diagnostic notice when complex or BiDi text is retained with fidelity but
/// layout capabilities are restricted per PLATFORM-004 / LAYOUT-014.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComplexScriptDiagnostic {
    BiDiRtlPreservedWithLimitedLayout {
        range: std::ops::Range<usize>,
        script: &'static str,
    },
    ComplexIndicPreservedWithLimitedLayout {
        range: std::ops::Range<usize>,
        script: &'static str,
    },
}

pub fn scan_complex_scripts(text: &str) -> Vec<ComplexScriptDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut current_bidi = None;
    let mut current_indic = None;

    for (byte_idx, ch) in text.char_indices() {
        let script = classify_script(ch);
        match script {
            ScriptKind::BiDiRtl => {
                if current_bidi.is_none() {
                    current_bidi = Some(byte_idx);
                }
            }
            _ => {
                if let Some(start) = current_bidi.take() {
                    diagnostics.push(ComplexScriptDiagnostic::BiDiRtlPreservedWithLimitedLayout {
                        range: start..byte_idx,
                        script: "Arabic/Hebrew BiDi",
                    });
                }
            }
        }
        match script {
            ScriptKind::ComplexIndic => {
                if current_indic.is_none() {
                    current_indic = Some(byte_idx);
                }
            }
            _ => {
                if let Some(start) = current_indic.take() {
                    diagnostics.push(
                        ComplexScriptDiagnostic::ComplexIndicPreservedWithLimitedLayout {
                            range: start..byte_idx,
                            script: "Indic/Thai Complex",
                        },
                    );
                }
            }
        }
    }
    if let Some(start) = current_bidi {
        diagnostics.push(ComplexScriptDiagnostic::BiDiRtlPreservedWithLimitedLayout {
            range: start..text.len(),
            script: "Arabic/Hebrew BiDi",
        });
    }
    if let Some(start) = current_indic {
        diagnostics.push(
            ComplexScriptDiagnostic::ComplexIndicPreservedWithLimitedLayout {
                range: start..text.len(),
                script: "Indic/Thai Complex",
            },
        );
    }

    diagnostics
}
