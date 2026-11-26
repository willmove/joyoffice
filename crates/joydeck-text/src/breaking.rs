use super::style::{Cell, ParaShaped};
use super::{FontFace, ParaProps};

pub(crate) fn measure_str(face: &FontFace, size: f64, s: &str) -> (f64, Vec<(u32, f64)>) {
    let g = joydeck_fonts::shape(face, s, &[], |c| c);
    let k = size / face.upem.max(1.0);
    let mut x = 0.0;
    let mut out = vec![];
    for gl in g {
        out.push((gl.gid, x + gl.x_offset as f64 * k));
        x += gl.x_advance as f64 * k;
    }
    (x, out)
}

/// Single line spacing as a multiple of the font size. PowerPoint spaces single-spaced lines 1.2
/// times the size apart in every font (observed on twenty Latin, CJK, Thai and Indic fonts at 18
/// and 40 pt), whatever the font's own ascent and descent.
///
/// Caveat: this was measured on one Windows install of desktop PowerPoint. It may be a Windows or
/// machine setting; PowerPoint on macOS or the web may space lines by the font's own metrics.
/// Keep it a single tunable value, and re-measure on the other platform before changing it.
pub(crate) const LINE_PITCH: f64 = 1.2;

/// A line's height above and below the baseline at `size`: [`LINE_PITCH`] × size, split where
/// the font's Windows ascent and descent put the baseline.
pub(crate) fn line_metrics(face: &FontFace, size: f64) -> (f64, f64) {
    let h = size * LINE_PITCH;
    (h * face.line_split, h * (1.0 - face.line_split))
}

/// Hangul syllables and jamo: PowerPoint wraps Korean at word boundaries (spaces), not between
/// syllables.
fn is_hangul(c: char) -> bool {
    matches!(c, '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' | '\u{A960}'..='\u{A97F}' | '\u{AC00}'..='\u{D7FF}')
}

/// East Asian commas and full stops, the punctuation PowerPoint lets hang past the end of a line
/// (`hangingPunct`). Closing brackets don't hang: "「。」" overflowing by its "」" moves to the next
/// line whole.
fn is_hanging_punct(c: char) -> bool {
    matches!(c, '、' | '。' | '，' | '．' | '､' | '｡')
}

/// Small kana, the prolonged sound mark and iteration marks: PowerPoint keeps them off the start
/// of a line (strict Japanese line breaking), where UAX #14's default allows a break before them.
fn is_cj_nonstarter(c: char) -> bool {
    matches!(
        c,
        'ぁ' | 'ぃ'
            | 'ぅ'
            | 'ぇ'
            | 'ぉ'
            | 'っ'
            | 'ゃ'
            | 'ゅ'
            | 'ょ'
            | 'ゎ'
            | 'ゕ'
            | 'ゖ'
            | 'ァ'
            | 'ィ'
            | 'ゥ'
            | 'ェ'
            | 'ォ'
            | 'ッ'
            | 'ャ'
            | 'ュ'
            | 'ョ'
            | 'ヮ'
            | 'ヵ'
            | 'ヶ'
            | 'ー'
            | '々'
            | '〻'
            | 'ゝ'
            | 'ゞ'
            | 'ヽ'
            | 'ヾ'
            | '\u{31F0}'..='\u{31FF}'
            | 'ｧ'..='ｰ'
    )
}

/// A character that continues a Latin-style word: a letter or digit that Unicode does not break
/// between, as opposed to CJK ideographs, kana, spaces, punctuation and symbols such as `&` or `×`
/// (which share the AL and AI line-break classes with letters).
fn is_word_char(c: char) -> bool {
    use unicode_linebreak::BreakClass::{Alphabetic, Ambiguous, HebrewLetter, Numeric};
    matches!(
        unicode_linebreak::break_property(c as u32),
        Alphabetic | Ambiguous | HebrewLetter | Numeric
    ) && c.is_alphanumeric()
}

/// Join UAX #14 byte offsets with the paragraph's logical character boundaries.
/// Apply the document language settings without changing the order of shaped cells.
fn opportunities(text: &str, latin_break: bool) -> Vec<Opportunity> {
    use unicode_linebreak::{BreakOpportunity, linebreaks};

    let mut breaks = linebreaks(text).peekable();
    let mut previous = None;
    text.char_indices()
        .map(|(byte, character)| (byte, Some(character)))
        .chain(std::iter::once((text.len(), None)))
        .map(|(byte, after)| {
            let raw = breaks
                .next_if(|(offset, _)| *offset == byte)
                .map(|(_, kind)| kind);
            let before = previous;
            previous = after;
            match raw {
                Some(BreakOpportunity::Mandatory) => Opportunity::Required,
                Some(BreakOpportunity::Allowed)
                    if before
                        .zip(after)
                        .is_none_or(|(left, right)| permits_break(left, right, latin_break)) =>
                {
                    Opportunity::Allowed
                }
                _ => Opportunity::Blocked,
            }
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Opportunity {
    Blocked,
    Allowed,
    Required,
}

fn permits_break(left: char, right: char, latin_break: bool) -> bool {
    if (is_hangul(left) && is_hangul(right))
        || left == '\u{200b}'
        || right == '\u{2014}'
        || is_cj_nonstarter(right)
    {
        return false;
    }
    latin_break || !matches!(left, '/' | '\\' | '?' | '!' | '}' | '+') || !is_word_char(right)
}

#[cfg(test)]
pub(crate) fn break_opportunities(text: &str, latin_break: bool) -> (Vec<bool>, Vec<bool>) {
    opportunities(text, latin_break)
        .into_iter()
        .map(|kind| (kind != Opportunity::Blocked, kind == Opportunity::Required))
        .unzip()
}

pub(crate) struct RawLine {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// Available space and tab coordinates for the first and remaining lines.
pub(crate) struct BreakParams<'a> {
    pub(crate) first_w: f64,
    pub(crate) rest_w: f64,
    pub(crate) wrap: bool,
    pub(crate) tab_stops: &'a [Tab],
    pub(crate) default_tab: f64,
    pub(crate) x0_first: f64,
    pub(crate) x0_rest: f64,
}

pub(crate) fn break_lines(paragraph: &ParaShaped, params: &BreakParams<'_>) -> Vec<RawLine> {
    let cells = &paragraph.cells;
    if cells.is_empty() {
        return vec![RawLine { start: 0, end: 0 }];
    }
    let text: String = cells.iter().map(|cell| cell.ch).collect();
    let breaks = opportunities(&text, paragraph.props.latin_line_break.unwrap_or(false));
    let hanging = paragraph.props.hanging_punct.unwrap_or(false);
    let mut lines = Vec::new();
    let mut start = 0;
    while start < cells.len() {
        let (width, origin) = if lines.is_empty() {
            (params.first_w, params.x0_first)
        } else {
            (params.rest_w, params.x0_rest)
        };
        let (end, next) = line_end(cells, &breaks, start, (width, origin), params, hanging);
        lines.push(RawLine { start, end });
        start = next;
    }
    if cells.last().is_some_and(|cell| cell.ch == '\u{b}') {
        lines.push(RawLine { start, end: start });
    }
    lines
}

fn line_end(
    cells: &[Cell],
    breaks: &[Opportunity],
    start: usize,
    geometry: (f64, f64),
    params: &BreakParams<'_>,
    hanging: bool,
) -> (usize, usize) {
    let (width, origin) = geometry;
    let mut advance = 0.0;
    let mut legal = None;
    let mut cluster = None;
    for (index, cell) in cells.iter().enumerate().skip(start) {
        let character = cell.ch;
        if matches!(character, '\u{b}' | '\n') {
            return (index, index + 1);
        }
        if index > start && cell.breakable(index) {
            cluster = Some(index);
            if breaks
                .get(index)
                .is_some_and(|boundary| *boundary != Opportunity::Blocked)
            {
                legal = Some(index);
            }
        }
        let step = if character == '\t' {
            tab_advance(
                origin + advance,
                params.tab_stops,
                params.default_tab,
                tab_segment(cells, index),
            )
        } else {
            cell.adv
        };
        let limit = width + 0.01;
        if params.wrap && index > start && advance + step > limit {
            if hanging && advance <= limit && is_hanging_punct(character) {
                return (index + 1, index + 1);
            }
            let next = if character == '\t' {
                Some(index)
            } else if !character.is_whitespace() {
                Some(legal.or(cluster).unwrap_or_else(|| {
                    (index + 1..cells.len())
                        .find(|&next| cells[next].breakable(next))
                        .unwrap_or(cells.len())
                }))
            } else {
                None
            };
            if let Some(next) = next {
                let end = cells[start..next]
                    .iter()
                    .rposition(|item| item.ch != ' ')
                    .map_or(start, |last| start + last + 1);
                return (end, next);
            }
        }
        advance += step;
    }
    (cells.len(), cells.len())
}

/// How a tab stop aligns the text after it (`a:tab/@algn`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum TabAlign {
    Left,
    Center,
    Right,
    /// The first "." of the text sits at the stop (text without one ends there).
    Decimal,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tab {
    pos: f64,
    align: TabAlign,
}

pub(crate) fn tabs_of(props: &ParaProps) -> Vec<Tab> {
    props
        .tabs
        .as_ref()
        .map(|t| {
            t.iter()
                .map(|s| Tab {
                    pos: s.pos,
                    align: match s.align.as_str() {
                        "ctr" => TabAlign::Center,
                        "r" => TabAlign::Right,
                        "dec" => TabAlign::Decimal,
                        _ => TabAlign::Left,
                    },
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Width of the text after the tab at cell `i` up to the next tab or line break, and the width of
/// that text before its first ".".
pub(crate) fn tab_segment(cells: &[Cell], i: usize) -> (f64, f64) {
    let mut w = 0.0;
    let mut dec = None;
    for c in cells.iter().skip(i + 1) {
        if matches!(c.ch, '\t'..='\u{b}') {
            break;
        }
        if c.ch == '.' && dec.is_none() {
            dec = Some(w);
        }
        w += c.adv;
    }
    (w, dec.unwrap_or(w))
}

/// Advance of a tab at `x` (from the start edge) to the next stop, placing the text after it
/// (`seg`: its width, and its width before the decimal point) as the stop aligns it but never
/// moving back, as PowerPoint does; past the last stop, to the next default stop.
pub(crate) fn tab_advance(x: f64, stops: &[Tab], default_tab: f64, seg: (f64, f64)) -> f64 {
    for s in stops {
        if s.pos > x + 0.5 {
            let to = s.pos - x;
            return match s.align {
                TabAlign::Left => to,
                TabAlign::Right => (to - seg.0).max(0.0),
                TabAlign::Center => (to - seg.0 / 2.0).max(0.0),
                TabAlign::Decimal => (to - seg.1).max(0.0),
            };
        }
    }
    let d = if default_tab > 1.0 { default_tab } else { 72.0 };
    let next = ((x / d).floor() + 1.0) * d;
    (next - x).max(1.0)
}
