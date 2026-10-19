//! Numbering definitions and list labels (ECMA-376 Part 1 §17.9).
//!
//! An abstract numbering definition holds up to nine levels. A numbering
//! instance points at one abstract definition and may override a level's
//! start, or the whole level. Paragraphs name an instance and a level with
//! `w:numPr`; a style may do the same, and a direct `numId` of 0 suppresses
//! the style. `numStyleLink` and `styleLink` are kept and not followed.
//!
//! Counter values follow the level's start and `lvlRestart`. The first use
//! after a restart displays `start` (or `startOverride`). A `%n` reference
//! to a level that has not been used displays that level's start and does
//! not consume it. Formats outside the set this module can render, and
//! values outside the range the specification's examples define, are
//! errors: the label is never a guessed string.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::block::{Block, Paragraph};
use crate::document::Document;
use crate::id::NodeId;
use crate::props::{
    FontRef, FontSlots, Indent, Measure, NumberingRef, ParaProps, RunProps, TabStop,
};
use crate::style::overlay_paragraph;
use crate::symbol_font::{self, is_mapped_family};

/// Highest `ilvl` ECMA-376 defines (0..=8).
pub const MAX_LEVEL: u8 = 8;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NumberFormat {
    Decimal,
    LowerLetter,
    UpperLetter,
    LowerRoman,
    UpperRoman,
    ChineseCounting,
    IdeographTraditional,
    ChineseLegalSimplified,
    /// `w:numFmt` `none`: the level contributes an empty string.
    None,
    /// `w:numFmt` `bullet`: the label is the literal `w:lvlText` after `%%`.
    Bullet,
    /// A token this slice does not render. The original token is kept.
    Other(Arc<str>),
}

impl NumberFormat {
    pub fn parse(token: &str) -> Self {
        match token {
            "decimal" => Self::Decimal,
            "lowerLetter" => Self::LowerLetter,
            "upperLetter" => Self::UpperLetter,
            "lowerRoman" => Self::LowerRoman,
            "upperRoman" => Self::UpperRoman,
            "chineseCounting" => Self::ChineseCounting,
            "ideographTraditional" => Self::IdeographTraditional,
            "chineseLegalSimplified" => Self::ChineseLegalSimplified,
            "none" => Self::None,
            "bullet" => Self::Bullet,
            other => Self::Other(Arc::from(other)),
        }
    }

    /// Draws `value` with this format. `None` when the format or the value
    /// is outside the set this slice can render, including an empty `none`.
    pub fn render(&self, value: i32) -> Option<String> {
        render_format(0, self, value)
            .ok()
            .filter(|text| !text.is_empty())
    }

    fn token(&self) -> Arc<str> {
        match self {
            Self::Decimal => Arc::from("decimal"),
            Self::LowerLetter => Arc::from("lowerLetter"),
            Self::UpperLetter => Arc::from("upperLetter"),
            Self::LowerRoman => Arc::from("lowerRoman"),
            Self::UpperRoman => Arc::from("upperRoman"),
            Self::ChineseCounting => Arc::from("chineseCounting"),
            Self::IdeographTraditional => Arc::from("ideographTraditional"),
            Self::ChineseLegalSimplified => Arc::from("chineseLegalSimplified"),
            Self::None => Arc::from("none"),
            Self::Bullet => Arc::from("bullet"),
            Self::Other(token) => Arc::clone(token),
        }
    }
}

/// `w:suff`. Omitted in the source means a tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NumberSuffix {
    Tab,
    Space,
    Nothing,
}

impl NumberSuffix {
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "tab" => Some(Self::Tab),
            "space" => Some(Self::Space),
            "nothing" => Some(Self::Nothing),
            _ => None,
        }
    }
}

/// One numbering level. `para` carries the level's indent and tab stops;
/// other paragraph properties on the level are left unresolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumberingLevel {
    pub start: i32,
    pub format: NumberFormat,
    /// `w:lvlText`. `%1` is level 0.
    pub text: Arc<str>,
    /// `w:lvlRestart`. `None` means the element was omitted: the level
    /// restarts when the previous level is used. `Some(0)` never restarts.
    /// `Some(n)` restarts when level `n - 1` is used.
    pub restart: Option<u8>,
    pub suffix: NumberSuffix,
    pub para: ParaProps,
    /// Level `w:rPr`. Size, bold and the named font are used for the label.
    pub run: RunProps,
    /// `w:lvlPicBulletId`. The label is refused; no glyph is invented.
    pub picture: Option<u32>,
    /// `w:pStyle`, `w:legacy`, `w:isLgl`, `w:lvlJc` or another child this
    /// slice does not apply. The numbering part bytes stay in the package.
    pub unmodeled: bool,
}

impl NumberingLevel {
    pub fn new(format: NumberFormat, text: &str) -> Self {
        Self {
            start: 0,
            format,
            text: Arc::from(text),
            restart: None,
            suffix: NumberSuffix::Tab,
            para: ParaProps::default(),
            run: RunProps::default(),
            picture: None,
            unmodeled: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelOverride {
    pub level: u8,
    pub start_override: Option<i32>,
    pub replacement: Option<NumberingLevel>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbstractNum {
    pub id: u32,
    pub levels: [Option<NumberingLevel>; 9],
    pub num_style_link: Option<Arc<str>>,
    pub style_link: Option<Arc<str>>,
    pub unmodeled: bool,
}

impl AbstractNum {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            levels: std::array::from_fn(|_| None),
            num_style_link: None,
            style_link: None,
            unmodeled: false,
        }
    }

    fn linked(&self) -> bool {
        self.num_style_link.is_some() || self.style_link.is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumInstance {
    pub id: u32,
    pub abstract_id: u32,
    pub overrides: Vec<LevelOverride>,
}

/// `numbering.xml` as far as labels and level paragraph properties need it.
/// The part itself is never regenerated from this store.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NumberingStore {
    abstracts: Vec<AbstractNum>,
    instances: Vec<NumInstance>,
    pub unmodeled: bool,
}

impl NumberingStore {
    pub fn is_empty(&self) -> bool {
        self.abstracts.is_empty() && self.instances.is_empty() && !self.unmodeled
    }

    pub fn abstracts(&self) -> &[AbstractNum] {
        &self.abstracts
    }

    pub fn instances(&self) -> &[NumInstance] {
        &self.instances
    }

    pub fn insert_abstract(&mut self, abstract_num: AbstractNum) -> bool {
        if self.abstracts.iter().any(|item| item.id == abstract_num.id) {
            return false;
        }
        self.abstracts.push(abstract_num);
        true
    }

    pub fn insert_instance(&mut self, instance: NumInstance) -> bool {
        if self.instances.iter().any(|item| item.id == instance.id) {
            return false;
        }
        self.instances.push(instance);
        true
    }

    /// Returns a level-0 bullet instance, creating one when none exists.
    ///
    /// The created label is U+2022, a tab suffix, and a hanging indent of
    /// 720 twips with 360 twips of hanging. Those distances are a JoyWord
    /// preset, not a measurement taken from Word. One undo of the command
    /// that calls this restores the previous store. Export still refuses a
    /// numbering store that differs from the package.
    pub fn ensure_bullet(&mut self) -> u32 {
        if let Some(id) = self.instances.iter().find_map(|instance| {
            let abstract_num = self
                .abstracts
                .iter()
                .find(|item| item.id == instance.abstract_id)?;
            if abstract_num.linked() {
                return None;
            }
            let level = abstract_num.levels[0].as_ref()?;
            (level.format == NumberFormat::Bullet && level.picture.is_none()).then_some(instance.id)
        }) {
            return id;
        }
        let abstract_id = self
            .abstracts
            .iter()
            .map(|item| item.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let mut level = NumberingLevel::new(NumberFormat::Bullet, "\u{2022}");
        level.start = 1;
        level.para.indent.start = Some(Measure::Twips(720));
        level.para.indent.hanging = Some(Measure::Twips(360));
        let mut abstract_num = AbstractNum::new(abstract_id);
        abstract_num.levels[0] = Some(level);
        self.insert_abstract(abstract_num);
        let id = self
            .instances
            .iter()
            .map(|item| item.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        self.insert_instance(NumInstance {
            id,
            abstract_id,
            overrides: Vec::new(),
        });
        id
    }

    pub fn instance(&self, id: u32) -> Option<&NumInstance> {
        self.instances.iter().find(|item| item.id == id)
    }

    pub fn abstracts_mut(&mut self) -> &mut [AbstractNum] {
        &mut self.abstracts
    }

    fn abstract_num(&self, id: u32) -> Option<&AbstractNum> {
        self.abstracts.iter().find(|item| item.id == id)
    }

    /// The level a paragraph displays, after instance overrides.
    pub fn resolved_level(
        &self,
        num_id: u32,
        ilvl: u8,
    ) -> Result<ResolvedLevel<'_>, NumberingProblem> {
        if ilvl > MAX_LEVEL {
            return Err(NumberingProblem::LevelRange(ilvl));
        }
        let Some(instance) = self.instance(num_id) else {
            return Err(NumberingProblem::MissingInstance(num_id));
        };
        let Some(abstract_num) = self.abstract_num(instance.abstract_id) else {
            return Err(NumberingProblem::MissingAbstract(num_id));
        };
        if abstract_num.linked() {
            return Err(NumberingProblem::UnresolvedLink(num_id));
        }
        let override_ = instance.overrides.iter().find(|item| item.level == ilvl);
        let level = override_
            .and_then(|item| item.replacement.as_ref())
            .or(abstract_num.levels[ilvl as usize].as_ref())
            .ok_or(NumberingProblem::MissingLevel {
                num_id,
                level: ilvl,
            })?;
        Ok(ResolvedLevel {
            level,
            start_override: override_.and_then(|item| item.start_override),
        })
    }
}

pub struct ResolvedLevel<'a> {
    pub level: &'a NumberingLevel,
    pub start_override: Option<i32>,
}

impl ResolvedLevel<'_> {
    fn initial(&self) -> i32 {
        self.start_override.unwrap_or(self.level.start)
    }
}

/// Why a paragraph has no list label. Layout records this and does not
/// invent a marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NumberingProblem {
    MissingInstance(u32),
    MissingAbstract(u32),
    MissingLevel {
        num_id: u32,
        level: u8,
    },
    /// `w:numStyleLink` or `w:styleLink` is not followed.
    UnresolvedLink(u32),
    UnsupportedFormat {
        num_id: u32,
        format: Arc<str>,
    },
    /// The value is outside the range this slice renders.
    Unrenderable {
        num_id: u32,
        value: i32,
    },
    /// `w:lvlPicBulletId` names a picture. No replacement glyph is drawn.
    PictureBullet {
        num_id: u32,
    },
    LevelRange(u8),
}

impl fmt::Display for NumberingProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInstance(id) => write!(f, "numbering instance {id} is missing"),
            Self::MissingAbstract(id) => {
                write!(f, "numbering instance {id} has no abstract numbering")
            }
            Self::MissingLevel { num_id, level } => {
                write!(f, "numbering instance {num_id} has no level {level}")
            }
            Self::UnresolvedLink(id) => {
                write!(f, "numbering instance {id} style link is unresolved")
            }
            Self::UnsupportedFormat { num_id, format } => {
                write!(
                    f,
                    "numbering instance {num_id} format {format} is unsupported"
                )
            }
            Self::Unrenderable { num_id, value } => {
                write!(f, "numbering instance {num_id} cannot render {value}")
            }
            Self::PictureBullet { num_id } => {
                write!(f, "numbering instance {num_id} picture bullet is not drawn")
            }
            Self::LevelRange(level) => write!(f, "numbering level {level} is outside 0..=8"),
        }
    }
}

/// The marker drawn before a paragraph. Indent and tabs come from the
/// resolved paragraph, which already includes the level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumberLabel {
    pub text: String,
    pub suffix: NumberSuffix,
    /// Level `w:sz`, when the level specifies one. The body size is used otherwise.
    pub size: Option<Measure>,
    /// Level `w:b` when it is explicitly on. The label does not inherit body bold.
    pub bold: bool,
}

/// Direct `w:numPr`, or the paragraph style's, ignoring `numId` 0.
pub fn list_reference(document: &Document, paragraph: &Paragraph) -> Option<NumberingRef> {
    if let Some(direct) = paragraph.props.numbering {
        return (direct.num_id != 0).then_some(direct);
    }
    let resolved = document
        .styles()
        .resolve_paragraph(paragraph.style.as_ref(), &ParaProps::default());
    resolved.props.numbering.filter(|item| item.num_id != 0)
}

/// Document defaults, then the style, then the numbering level's indent and
/// tabs, then direct paragraph properties.
pub fn displayed_paragraph(document: &Document, paragraph: &Paragraph) -> ParaProps {
    let mut props = document
        .styles()
        .resolve_paragraph(paragraph.style.as_ref(), &ParaProps::default())
        .props;
    let reference = if let Some(direct) = paragraph.props.numbering {
        (direct.num_id != 0).then_some(direct)
    } else {
        props.numbering.filter(|item| item.num_id != 0)
    };
    if let Some(reference) = reference
        && let Ok(resolved) = document
            .numbering()
            .resolved_level(reference.num_id, reference.level)
    {
        let layer = ParaProps {
            indent: resolved.level.para.indent,
            tabs: resolved.level.para.tabs.clone(),
            ..ParaProps::default()
        };
        overlay_paragraph(&mut props, &layer);
    }
    overlay_paragraph(&mut props, &paragraph.props);
    props
}

/// One label per numbered paragraph in the body, in document order.
/// Counters are per numbering instance and follow restart rules.
pub fn list_marks(document: &Document) -> BTreeMap<NodeId, Result<NumberLabel, NumberingProblem>> {
    let mut counters: BTreeMap<u32, [Option<i32>; 9]> = BTreeMap::new();
    let mut marks = BTreeMap::new();
    walk_paragraphs(&document.body().blocks, &mut |paragraph| {
        let Some(reference) = list_reference(document, paragraph) else {
            return;
        };
        let label = label_one(document, &mut counters, reference);
        marks.insert(paragraph.id(), label);
    });
    marks
}

fn label_one(
    document: &Document,
    counters: &mut BTreeMap<u32, [Option<i32>; 9]>,
    reference: NumberingRef,
) -> Result<NumberLabel, NumberingProblem> {
    let (initial, suffix, template, format, picture, size, bold, fonts) = {
        let resolved = document
            .numbering()
            .resolved_level(reference.num_id, reference.level)?;
        (
            resolved.initial(),
            resolved.level.suffix,
            Arc::clone(&resolved.level.text),
            resolved.level.format.clone(),
            resolved.level.picture,
            resolved.level.run.size,
            resolved.level.run.bold == Some(true),
            resolved.level.run.fonts.clone(),
        )
    };
    let state = counters.entry(reference.num_id).or_insert([None; 9]);
    restart_deeper(document, reference.num_id, reference.level, state)?;
    let next = match state[reference.level as usize] {
        None => initial,
        Some(value) => value.saturating_add(1),
    };
    state[reference.level as usize] = Some(next);
    if picture.is_some() {
        return Err(NumberingProblem::PictureBullet {
            num_id: reference.num_id,
        });
    }
    if let NumberFormat::Other(token) = format {
        return Err(NumberingProblem::UnsupportedFormat {
            num_id: reference.num_id,
            format: token,
        });
    }
    let raw = if matches!(format, NumberFormat::Bullet) {
        bullet_literal(&template)
    } else {
        substitute(&template, |slot| {
            render_slot(document, reference.num_id, slot, state)
        })?
    };
    let text = map_label_font(&fonts, &raw);
    Ok(NumberLabel {
        text,
        suffix,
        size,
        bold,
    })
}

fn map_label_font(fonts: &FontSlots, text: &str) -> String {
    for slot in [&fonts.ascii, &fonts.h_ansi, &fonts.east_asia] {
        if let Some(FontRef::Named(name)) = slot
            && is_mapped_family(name)
        {
            return symbol_font::map_private_use(name, text);
        }
    }
    text.to_owned()
}

/// `%%` becomes `%`. A bullet's `%n` stays literal text.
fn bullet_literal(template: &str) -> String {
    let mut out = String::new();
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' && chars.peek().copied() == Some('%') {
            chars.next();
            out.push('%');
        } else {
            out.push(ch);
        }
    }
    out
}

fn restart_deeper(
    document: &Document,
    num_id: u32,
    used: u8,
    state: &mut [Option<i32>; 9],
) -> Result<(), NumberingProblem> {
    for deeper in (used + 1)..=MAX_LEVEL {
        let Ok(resolved) = document.numbering().resolved_level(num_id, deeper) else {
            continue;
        };
        let reset = match resolved.level.restart {
            None => deeper == used + 1,
            Some(0) => false,
            Some(trigger) => trigger.saturating_sub(1) == used,
        };
        if reset {
            state[deeper as usize] = None;
        }
    }
    Ok(())
}

fn render_slot(
    document: &Document,
    num_id: u32,
    ilvl: u8,
    state: &[Option<i32>; 9],
) -> Result<String, NumberingProblem> {
    let resolved = document.numbering().resolved_level(num_id, ilvl)?;
    let value = state[ilvl as usize].unwrap_or_else(|| resolved.initial());
    render_format(num_id, &resolved.level.format, value)
}

fn substitute(
    template: &str,
    mut value: impl FnMut(u8) -> Result<String, NumberingProblem>,
) -> Result<String, NumberingProblem> {
    let mut out = String::new();
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            match chars.peek().copied() {
                Some('%') => {
                    chars.next();
                    out.push('%');
                }
                Some(digit) if ('1'..='9').contains(&digit) => {
                    chars.next();
                    out.push_str(&value(digit as u8 - b'1')?);
                }
                _ => out.push('%'),
            }
        } else {
            out.push(ch);
        }
    }
    Ok(out)
}

fn render_format(
    num_id: u32,
    format: &NumberFormat,
    value: i32,
) -> Result<String, NumberingProblem> {
    let unsupported = || NumberingProblem::UnsupportedFormat {
        num_id,
        format: format.token(),
    };
    let unrenderable = || NumberingProblem::Unrenderable { num_id, value };
    match format {
        NumberFormat::Decimal => Ok(value.to_string()),
        NumberFormat::None => Ok(String::new()),
        NumberFormat::Bullet => Ok(String::new()),
        NumberFormat::Other(_) => Err(unsupported()),
        NumberFormat::LowerLetter => letter(value, false).ok_or_else(unrenderable),
        NumberFormat::UpperLetter => letter(value, true).ok_or_else(unrenderable),
        NumberFormat::LowerRoman => roman(value, false).ok_or_else(unrenderable),
        NumberFormat::UpperRoman => roman(value, true).ok_or_else(unrenderable),
        NumberFormat::ChineseCounting => chinese_counting(value).ok_or_else(unrenderable),
        NumberFormat::ChineseLegalSimplified => {
            chinese_legal_simplified(value).ok_or_else(unrenderable)
        }
        NumberFormat::IdeographTraditional => ideograph(value).ok_or_else(unrenderable),
    }
}

/// Repeated-letter scheme from the numbering examples: 1 = a, 27 = aa, 54 = bbb.
/// Repeats are capped so a huge `i32` stays [`NumberingProblem::Unrenderable`].
const LETTER_REPEAT_MAX: i32 = 64;

/// Classical roman numerals. The specification example does not state an
/// upper bound; values outside 1..=[`ROMAN_MAX`] stay unrenderable. This
/// slice does not invent vinculum forms or `MMMM`.
pub const ROMAN_MAX: i32 = 3999;

fn letter(value: i32, upper: bool) -> Option<String> {
    if value < 1 {
        return None;
    }
    let index = (value - 1) % 26;
    let repeat = (value - 1) / 26 + 1;
    if repeat > LETTER_REPEAT_MAX {
        return None;
    }
    let base = if upper { b'A' } else { b'a' };
    let ch = char::from(base + index as u8);
    Some(ch.to_string().repeat(repeat as usize))
}

fn roman(value: i32, upper: bool) -> Option<String> {
    if !(1..=ROMAN_MAX).contains(&value) {
        return None;
    }
    const PARTS: [(i32, &str); 13] = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut rest = value;
    let mut out = String::new();
    for (amount, glyph) in PARTS {
        while rest >= amount {
            out.push_str(glyph);
            rest -= amount;
        }
    }
    if upper {
        out.make_ascii_uppercase();
    }
    Some(out)
}

const IDEAGRAPH: [char; 10] = ['甲', '乙', '丙', '丁', '戊', '己', '庚', '辛', '壬', '癸'];

fn ideograph(value: i32) -> Option<String> {
    if !(1..=10).contains(&value) {
        return None;
    }
    Some(IDEAGRAPH[(value as usize) - 1].to_string())
}

fn chinese_counting(value: i32) -> Option<String> {
    const DIGITS: [&str; 10] = ["零", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
    const UNITS: [&str; 4] = ["", "十", "百", "千"];
    chinese_group(value, &DIGITS, &UNITS, "万")
}

fn chinese_legal_simplified(value: i32) -> Option<String> {
    const DIGITS: [&str; 10] = ["零", "壹", "贰", "叁", "肆", "伍", "陆", "柒", "捌", "玖"];
    const UNITS: [&str; 4] = ["", "拾", "佰", "仟"];
    chinese_group(value, &DIGITS, &UNITS, "万")
}

/// Chinese counting for 1..=99999. The specification's example is 一, 二, 三;
/// values outside this range are not rendered. Tens from 10 to 19 omit the
/// leading 一 only when 十 is the first character of the whole number.
fn chinese_group(value: i32, digits: &[&str; 10], units: &[&str; 4], wan: &str) -> Option<String> {
    if !(1..=99_999).contains(&value) {
        return None;
    }
    let value = value as u32;
    let high = value / 10_000;
    let low = value % 10_000;
    let mut out = String::new();
    if high > 0 {
        out.push_str(digits[high as usize]);
        out.push_str(wan);
        if low > 0 && low < 1000 {
            out.push_str(digits[0]);
        }
    }
    if low > 0 {
        out.push_str(&chinese_section(low, digits, units, high == 0));
    }
    Some(out)
}

fn chinese_section(
    value: u32,
    digits: &[&str; 10],
    units: &[&str; 4],
    omit_leading_ten: bool,
) -> String {
    let mut places = [0u32; 4];
    let mut rest = value;
    for place in &mut places {
        *place = rest % 10;
        rest /= 10;
    }
    let mut out = String::new();
    let mut started = false;
    let mut pending_zero = false;
    for index in (0..4).rev() {
        let digit = places[index];
        if digit == 0 {
            if started {
                pending_zero = true;
            }
            continue;
        }
        if pending_zero {
            out.push_str(digits[0]);
            pending_zero = false;
        }
        let leading_ten = omit_leading_ten && !started && index == 1 && digit == 1;
        if !leading_ten {
            out.push_str(digits[digit as usize]);
        }
        if index > 0 {
            out.push_str(units[index]);
        }
        started = true;
    }
    out
}

fn walk_paragraphs(blocks: &crate::block::BlockSeq, visit: &mut dyn FnMut(&Paragraph)) {
    for block in blocks.iter() {
        match block {
            Block::Paragraph(paragraph) => visit(paragraph),
            Block::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        walk_paragraphs(&cell.blocks, visit);
                    }
                }
            }
            Block::Sdt(sdt) => walk_paragraphs(&sdt.blocks, visit),
            Block::Opaque(_) => {}
        }
    }
}

/// Indent used when placing a list label and the body that follows it.
pub fn indent_edges(indent: &Indent) -> Option<IndentEdges> {
    let edge = |measure: Option<crate::props::Measure>| match measure {
        None => Some(0),
        Some(measure) => measure.absolute().map(|length| length.raw()),
    };
    Some(IndentEdges {
        start: edge(indent.start)?,
        end: edge(indent.end)?,
        first_line: edge(indent.first_line)?,
        hanging: edge(indent.hanging)?,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndentEdges {
    pub start: i64,
    pub end: i64,
    pub first_line: i64,
    pub hanging: i64,
}

impl IndentEdges {
    pub fn moves_text(self) -> bool {
        self.start != 0 || self.end != 0 || self.first_line != 0 || self.hanging != 0
    }
}

/// Resolved custom tab stops plus the default interval's continuation.
/// Positions are absolute EMU from the content left. `num` is reported as
/// left. Clear stops are not positions.
pub fn tab_positions(stops: &[TabStop], interval: i64, limit: i64) -> Option<Vec<PlacedTab>> {
    if interval <= 0 {
        return None;
    }
    let mut custom = Vec::new();
    for stop in stops {
        if matches!(stop.align, crate::props::TabAlign::Clear) {
            continue;
        }
        let position = stop.position.absolute()?;
        custom.push(PlacedTab {
            position: position.raw(),
            align: stop.align,
            leader: stop.leader,
        });
    }
    custom.sort_by_key(|stop| stop.position);
    let rightmost = custom.last().map(|stop| stop.position);
    let mut position = match rightmost {
        None => interval,
        Some(after) => {
            let remainder = after % interval;
            if remainder == 0 {
                after + interval
            } else {
                after - remainder + interval
            }
        }
    };
    let end = limit.max(position);
    while position <= end {
        custom.push(PlacedTab {
            position,
            align: crate::props::TabAlign::Left,
            leader: crate::props::TabLeader::None,
        });
        position = position.saturating_add(interval);
    }
    Some(custom)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedTab {
    pub position: i64,
    pub align: crate::props::TabAlign,
    pub leader: crate::props::TabLeader,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::props::{FontRef, Measure, TabAlign, TabLeader};
    use crate::text::TextError;

    fn render(format: NumberFormat, value: i32) -> Result<String, NumberingProblem> {
        render_format(0, &format, value)
    }

    #[test]
    fn chinese_counting_and_legal_follow_place_value() -> Result<(), NumberingProblem> {
        let counting = NumberFormat::ChineseCounting;
        let legal = NumberFormat::ChineseLegalSimplified;
        assert_eq!(render(counting.clone(), 1)?, "一");
        assert_eq!(render(counting.clone(), 4)?, "四");
        assert_eq!(render(counting.clone(), 10)?, "十");
        assert_eq!(render(counting.clone(), 11)?, "十一");
        assert_eq!(render(counting.clone(), 20)?, "二十");
        assert_eq!(render(counting.clone(), 101)?, "一百零一");
        assert_eq!(render(counting.clone(), 110)?, "一百一十");
        assert_eq!(render(counting.clone(), 1001)?, "一千零一");
        assert_eq!(render(counting.clone(), 1010)?, "一千零一十");
        assert_eq!(render(counting.clone(), 10_001)?, "一万零一");
        assert_eq!(render(counting.clone(), 10_010)?, "一万零一十");
        assert_eq!(render(counting.clone(), 10_100)?, "一万零一百");
        assert_eq!(render(counting.clone(), 11_000)?, "一万一千");
        assert_eq!(render(legal.clone(), 1)?, "壹");
        assert_eq!(render(legal.clone(), 10)?, "拾");
        assert_eq!(render(legal.clone(), 11)?, "拾壹");
        assert_eq!(render(legal.clone(), 20)?, "贰拾");
        assert_eq!(render(legal, 101)?, "壹佰零壹");
        assert!(render(counting.clone(), 0).is_err());
        assert!(render(counting, 100_000).is_err());
        Ok(())
    }

    #[test]
    fn letters_roman_and_ideographs_stop_where_the_example_stops() -> Result<(), NumberingProblem> {
        assert_eq!(render(NumberFormat::LowerLetter, 1)?, "a");
        assert_eq!(render(NumberFormat::LowerLetter, 26)?, "z");
        assert_eq!(render(NumberFormat::LowerLetter, 27)?, "aa");
        assert_eq!(render(NumberFormat::LowerLetter, 54)?, "bbb");
        assert_eq!(render(NumberFormat::UpperLetter, 27)?, "AA");
        assert!(render(NumberFormat::LowerLetter, 26 * LETTER_REPEAT_MAX + 2).is_err());
        assert_eq!(render(NumberFormat::UpperLetter, 2)?, "B");
        assert_eq!(render(NumberFormat::LowerRoman, 4)?, "iv");
        assert_eq!(render(NumberFormat::UpperRoman, 1994)?, "MCMXCIV");
        assert_eq!(render(NumberFormat::UpperRoman, ROMAN_MAX)?, "MMMCMXCIX");
        assert!(render(NumberFormat::LowerRoman, ROMAN_MAX + 1).is_err());
        assert_eq!(render(NumberFormat::IdeographTraditional, 1)?, "甲");
        assert_eq!(render(NumberFormat::IdeographTraditional, 10)?, "癸");
        assert!(render(NumberFormat::IdeographTraditional, 11).is_err());
        assert!(matches!(
            render(NumberFormat::parse("ordinal"), 1),
            Err(NumberingProblem::UnsupportedFormat { .. })
        ));
        assert_eq!(render(NumberFormat::None, 4)?, "");
        assert_eq!(render(NumberFormat::Decimal, 0)?, "0");
        Ok(())
    }

    #[test]
    fn bullets_use_lvl_text_and_picture_bullets_advance_without_a_glyph() -> Result<(), TextError> {
        let mut document = Document::new(crate::PageSetup::a4_zh_cn());
        let mut bullet = NumberingLevel::new(NumberFormat::Bullet, "\u{F0B7}%%");
        bullet.start = 1;
        bullet.run.fonts.ascii = Some(FontRef::Named(Arc::from("Symbol")));
        bullet.run.size = Some(Measure::HalfPoints(28));
        bullet.run.bold = Some(true);
        let mut picture = NumberingLevel::new(NumberFormat::Decimal, "%1");
        picture.start = 1;
        picture.picture = Some(4);
        picture.run.fonts.h_ansi = Some(FontRef::Named(Arc::from("Wingdings")));
        let mut follow = NumberingLevel::new(NumberFormat::Decimal, "%1");
        follow.start = 1;
        let mut abstract_num = AbstractNum::new(1);
        abstract_num.levels[0] = Some(picture);
        abstract_num.levels[1] = Some(follow);
        document.numbering_mut().insert_abstract(abstract_num);
        let mut plain = AbstractNum::new(2);
        plain.levels[0] = Some(bullet);
        document.numbering_mut().insert_abstract(plain);
        document.numbering_mut().insert_instance(NumInstance {
            id: 5,
            abstract_id: 1,
            overrides: Vec::new(),
        });
        document.numbering_mut().insert_instance(NumInstance {
            id: 6,
            abstract_id: 2,
            overrides: Vec::new(),
        });
        paragraph(&mut document, "pic", 0)?;
        paragraph(&mut document, "pic2", 0)?;
        let mut shown = Paragraph::new(document.ids().node());
        shown.insert_text(crate::ScalarOffset(0), "next")?;
        shown.props = Arc::new(ParaProps {
            numbering: Some(NumberingRef {
                num_id: 5,
                level: 1,
            }),
            ..ParaProps::default()
        });
        let mut marked = Paragraph::new(document.ids().node());
        marked.insert_text(crate::ScalarOffset(0), "dot")?;
        marked.props = Arc::new(ParaProps {
            numbering: Some(NumberingRef {
                num_id: 6,
                level: 0,
            }),
            ..ParaProps::default()
        });
        let body = document.body_id();
        let Some(story) = document.story_mut(body) else {
            return Err(TextError::OutOfBounds { offset: 0, len: 0 });
        };
        let mut blocks: Vec<Block> = story.blocks.iter().cloned().collect();
        blocks.push(shown.into());
        blocks.push(marked.into());
        story.blocks = crate::block::BlockSeq::from_vec(blocks);
        let marks = list_marks(&document);
        let values: Vec<_> = marks.values().collect();
        assert!(matches!(
            values[0],
            Err(NumberingProblem::PictureBullet { num_id: 5 })
        ));
        assert!(matches!(
            values[1],
            Err(NumberingProblem::PictureBullet { .. })
        ));
        let Ok(counted) = values[2].as_ref() else {
            return Err(TextError::OutOfBounds {
                offset: 2,
                len: values.len(),
            });
        };
        assert_eq!(counted.text, "2");
        let Ok(label) = values[3].as_ref() else {
            return Err(TextError::OutOfBounds {
                offset: 3,
                len: values.len(),
            });
        };
        assert_eq!(label.text, "\u{2022}%");
        assert_eq!(label.size, Some(Measure::HalfPoints(28)));
        assert!(label.bold);
        assert_eq!(document.numbering_mut().ensure_bullet(), 6);
        let mut fresh = NumberingStore::default();
        let created = fresh.ensure_bullet();
        assert_eq!(created, fresh.ensure_bullet());
        assert_eq!(fresh.instances().len(), 1);
        Ok(())
    }

    fn level(format: NumberFormat, text: &str, start: i32) -> NumberingLevel {
        let mut level = NumberingLevel::new(format, text);
        level.start = start;
        level
    }

    fn two_level_document(restart: Option<u8>) -> (Document, u32) {
        let mut abstract_num = AbstractNum::new(1);
        abstract_num.levels[0] = Some(level(NumberFormat::Decimal, "%1", 1));
        let mut second = level(NumberFormat::Decimal, "%1.%2", 1);
        second.restart = restart;
        abstract_num.levels[1] = Some(second);
        abstract_num.levels[2] = Some(level(NumberFormat::Decimal, "%3", 1));
        let mut document = Document::new(crate::PageSetup::a4_zh_cn());
        document.numbering_mut().insert_abstract(abstract_num);
        document.numbering_mut().insert_instance(NumInstance {
            id: 5,
            abstract_id: 1,
            overrides: vec![LevelOverride {
                level: 0,
                start_override: Some(3),
                replacement: None,
            }],
        });
        (document, 5)
    }

    fn paragraph(document: &mut Document, text: &str, level: u8) -> Result<(), TextError> {
        let mut paragraph = Paragraph::new(document.ids().node());
        paragraph.insert_text(crate::ScalarOffset(0), text)?;
        paragraph.props = Arc::new(ParaProps {
            numbering: Some(NumberingRef { num_id: 5, level }),
            ..ParaProps::default()
        });
        let Some(story) = document.story_mut(document.body_id()) else {
            return Ok(());
        };
        let mut blocks: Vec<Block> = story.blocks.iter().cloned().collect();
        if blocks.len() == 1
            && blocks[0]
                .as_paragraph()
                .is_some_and(|item| item.props.numbering.is_none() && item.is_empty())
        {
            blocks.clear();
        }
        blocks.push(paragraph.into());
        story.blocks = crate::block::BlockSeq::from_vec(blocks);
        Ok(())
    }

    fn labels(document: &Document) -> Result<Vec<String>, NumberingProblem> {
        let mut texts = Vec::new();
        for item in list_marks(document).values() {
            texts.push(item.as_ref().map_err(Clone::clone)?.text.clone());
        }
        Ok(texts)
    }

    #[test]
    fn counters_restart_only_the_level_that_names_the_trigger() -> Result<(), NumberingProblem> {
        let (mut document, _) = two_level_document(None);
        for (text, level) in [
            ("a", 0),
            ("b", 1),
            ("c", 1),
            ("d", 2),
            ("e", 0),
            ("f", 2),
            ("g", 1),
        ] {
            paragraph(&mut document, text, level)
                .map_err(|_| NumberingProblem::LevelRange(level))?;
        }
        // startOverride 3. Level 2 restarts only when level 1 is used, so the
        // second level-0 item does not reset it.
        assert_eq!(
            labels(&document)?,
            ["3", "3.1", "3.2", "1", "4", "2", "4.1"]
        );
        Ok(())
    }

    #[test]
    fn an_unused_higher_level_displays_its_start_without_incrementing()
    -> Result<(), NumberingProblem> {
        let (mut document, _) = two_level_document(None);
        for (text, level) in [("a", 1), ("b", 1), ("c", 0)] {
            paragraph(&mut document, text, level)
                .map_err(|_| NumberingProblem::LevelRange(level))?;
        }
        // Level 0's startOverride is 3, shown twice before it is consumed.
        assert_eq!(labels(&document)?, ["3.1", "3.2", "3"]);
        Ok(())
    }

    #[test]
    fn a_style_link_is_preserved_unresolved() -> Result<(), NumberingProblem> {
        let (mut document, _) = two_level_document(None);
        document
            .numbering_mut()
            .abstracts_mut()
            .iter_mut()
            .for_each(|item| item.num_style_link = Some(Arc::from("List")));
        paragraph(&mut document, "a", 0).map_err(|_| NumberingProblem::LevelRange(0))?;
        let marks = list_marks(&document);
        let Some(Err(NumberingProblem::UnresolvedLink(5))) = marks.values().next() else {
            return Err(NumberingProblem::UnresolvedLink(0));
        };
        Ok(())
    }

    #[test]
    fn num_id_zero_suppresses_a_style_list() -> Result<(), &'static str> {
        let (mut document, _) = two_level_document(None);
        let mut style = crate::Style::new("List", crate::StyleKind::Paragraph, Some("List"));
        style.para.numbering = Some(NumberingRef {
            num_id: 5,
            level: 0,
        });
        if document.styles_mut().insert(style).is_err() {
            return Err("style");
        }
        let mut paragraph = Paragraph::new(document.ids().node());
        paragraph.style = Some(crate::StyleId::new("List"));
        paragraph.props = Arc::new(ParaProps {
            numbering: Some(NumberingRef {
                num_id: 0,
                level: 0,
            }),
            ..ParaProps::default()
        });
        let Some(story) = document.story_mut(document.body_id()) else {
            return Err("body");
        };
        story.blocks = crate::block::BlockSeq::from_vec(vec![paragraph.into()]);
        if !list_marks(&document).is_empty() {
            return Err("suppressed list still has a label");
        }
        Ok(())
    }

    #[test]
    fn clear_removes_a_tab_and_a_later_stop_replaces_it() {
        let inherited = [TabStop {
            position: Measure::Twips(720),
            align: TabAlign::Left,
            leader: TabLeader::Dot,
        }];
        let cleared = crate::props::merge_tab_stops(
            &inherited,
            &[TabStop {
                position: Measure::Twips(720),
                align: TabAlign::Clear,
                leader: TabLeader::None,
            }],
        );
        assert!(cleared.is_empty());
        let replaced = crate::props::merge_tab_stops(
            &inherited,
            &[TabStop {
                position: Measure::Twips(720),
                align: TabAlign::Right,
                leader: TabLeader::Hyphen,
            }],
        );
        assert_eq!(replaced.len(), 1);
        assert_eq!(replaced[0].align, TabAlign::Right);
        assert_eq!(replaced[0].leader, TabLeader::Hyphen);
    }
}
