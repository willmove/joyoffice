//! Numbering part (ECMA-376 Part 1 §17.9).
//!
//! Abstract definitions, instances, level overrides and the level properties
//! this slice renders are copied into the model. `numStyleLink` and
//! `styleLink` are stored and not followed. Other children (`nsid`,
//! `multiLevelType`, `lvlJc`, `rPr`, `legacy`, `isLgl`, …) set
//! [`NumberingStore::unmodeled`] or the level's flag. The part bytes stay in
//! the package; export inserts definitions appended to the store into
//! `numbering.xml` without rewriting any other byte (see `export::numbering`)
//! and refuses a store whose existing entries changed.

use joyword_model::{
    AbstractNum, FontRef, LevelOverride, NumInstance, NumberFormat, NumberSuffix, NumberingLevel,
    NumberingStore, ParaProps, RunProps,
};

use crate::ns::W;
use crate::props::{self, Findings, val};
use crate::xml::Element;

pub(crate) fn read(root: &Element) -> NumberingStore {
    let mut store = NumberingStore::default();
    for child in root.elements() {
        if !child.is(W, "abstractNum") && !child.is(W, "num") {
            store.unmodeled = true;
            continue;
        }
        if child.is(W, "abstractNum") {
            match abstract_num(child) {
                Some(item) => {
                    if !store.insert_abstract(item) {
                        store.unmodeled = true;
                    }
                }
                None => store.unmodeled = true,
            }
        } else {
            match instance(child) {
                Some((item, extra)) => {
                    store.unmodeled |= extra;
                    if !store.insert_instance(item) {
                        store.unmodeled = true;
                    }
                }
                None => store.unmodeled = true,
            }
        }
    }
    store
}

fn abstract_num(element: &Element) -> Option<AbstractNum> {
    let id = attr_u32(element, "abstractNumId")?;
    let mut abstract_num = AbstractNum::new(id);
    for child in element.elements() {
        if child.namespace.as_deref() != Some(W) {
            abstract_num.unmodeled = true;
            continue;
        }
        match &*child.local {
            "lvl" => match parse_level(child) {
                Some((index, level)) => abstract_num.levels[index as usize] = Some(level),
                None => abstract_num.unmodeled = true,
            },
            "numStyleLink" => {
                abstract_num.num_style_link = val(child).map(Into::into);
                if abstract_num.num_style_link.is_none() {
                    abstract_num.unmodeled = true;
                }
            }
            "styleLink" => {
                abstract_num.style_link = val(child).map(Into::into);
                if abstract_num.style_link.is_none() {
                    abstract_num.unmodeled = true;
                }
            }
            _ => abstract_num.unmodeled = true,
        }
    }
    Some(abstract_num)
}

fn instance(element: &Element) -> Option<(NumInstance, bool)> {
    let id = attr_u32(element, "numId")?;
    let mut abstract_id = None;
    let mut overrides = Vec::new();
    let mut unmodeled = false;
    for child in element.elements() {
        if child.namespace.as_deref() != Some(W) {
            unmodeled = true;
            continue;
        }
        match &*child.local {
            "abstractNumId" => {
                abstract_id = val(child).and_then(|value| value.parse().ok());
                if abstract_id.is_none() {
                    unmodeled = true;
                }
            }
            "lvlOverride" => match level_override(child) {
                Some(item) => overrides.push(item),
                None => unmodeled = true,
            },
            _ => unmodeled = true,
        }
    }
    let abstract_id = abstract_id?;
    Some((
        NumInstance {
            id,
            abstract_id,
            overrides,
        },
        unmodeled,
    ))
}

fn level_override(element: &Element) -> Option<LevelOverride> {
    let level = attr_u8(element, "ilvl")?;
    if level > joyword_model::MAX_LEVEL {
        return None;
    }
    let mut start_override = None;
    let mut replacement = None;
    for child in element.elements() {
        if !child.is(W, "startOverride") && !child.is(W, "lvl") {
            return None;
        }
        if child.is(W, "startOverride") {
            let parsed = val(child).and_then(|value| value.parse::<i32>().ok())?;
            start_override = Some(parsed);
        } else if let Some((_, parsed)) = parse_level(child) {
            replacement = Some(parsed);
        } else {
            return None;
        }
    }
    Some(LevelOverride {
        level,
        start_override,
        replacement,
    })
}

fn parse_level(element: &Element) -> Option<(u8, NumberingLevel)> {
    let index = attr_u8(element, "ilvl")?;
    if index > joyword_model::MAX_LEVEL {
        return None;
    }
    let mut level = NumberingLevel::new(NumberFormat::None, "");
    let mut saw_format = false;
    for child in element.elements() {
        if child.namespace.as_deref() != Some(W) {
            level.unmodeled = true;
            continue;
        }
        match &*child.local {
            "start" => {
                level.start = val(child).and_then(|value| value.parse().ok()).unwrap_or(0);
                if val(child)
                    .and_then(|value| value.parse::<i32>().ok())
                    .is_none()
                {
                    level.unmodeled = true;
                }
            }
            "numFmt" => {
                let token = val(child).unwrap_or("");
                if token.is_empty() {
                    level.unmodeled = true;
                }
                level.format = NumberFormat::parse(token);
                saw_format = true;
            }
            "lvlText" => {
                level.text = val(child).unwrap_or("").into();
                if val(child).is_none() {
                    level.unmodeled = true;
                }
            }
            "lvlRestart" => match val(child).and_then(|value| value.parse().ok()) {
                Some(value) => level.restart = Some(value),
                None => level.unmodeled = true,
            },
            "suff" => match val(child).and_then(NumberSuffix::parse) {
                Some(suffix) => level.suffix = suffix,
                None => level.unmodeled = true,
            },
            "pPr" => {
                let (para, extra) = level_paragraph(child);
                if extra {
                    level.unmodeled = true;
                }
                level.para = para;
            }
            "rPr" => {
                let (run, extra) = level_run(child);
                if extra {
                    level.unmodeled = true;
                }
                level.run = run;
            }
            "lvlPicBulletId" => match val(child).and_then(|value| value.parse().ok()) {
                Some(id) => level.picture = Some(id),
                None => level.unmodeled = true,
            },
            _ => level.unmodeled = true,
        }
    }
    if !saw_format {
        level.unmodeled = true;
    }
    Some((index, level))
}

/// Indent and tabs are applied. Any other paragraph property on the level is
/// reported and left out of the model layer.
fn level_paragraph(element: &Element) -> (ParaProps, bool) {
    let mut findings = Findings::default();
    let parsed = props::para(element, &mut findings);
    let source = parsed.props;
    let extra = !findings.complete
        || parsed.style.is_some()
        || parsed.mark.is_some()
        || parsed.section.is_some()
        || source.justification.is_some()
        || source.spacing != joyword_model::Spacing::default()
        || source.keep_next.is_some()
        || source.keep_lines.is_some()
        || source.widow_control.is_some()
        || source.page_break_before.is_some()
        || source.snap_to_grid.is_some()
        || source.outline_level.is_some()
        || source.numbering.is_some()
        || source.preserved.is_some();
    let para = ParaProps {
        indent: source.indent,
        tabs: source.tabs,
        ..ParaProps::default()
    };
    (para, extra)
}

/// Bold, size and named latin/east-Asian fonts are applied. Any other
/// character property is reported; the part bytes stay untouched.
fn level_run(element: &Element) -> (RunProps, bool) {
    let mut findings = Findings::default();
    let (run, style) = props::run(element, &mut findings);
    let extra = !findings.complete
        || style.is_some()
        || run.italic.is_some()
        || run.strike.is_some()
        || run.hidden.is_some()
        || run.underline.is_some()
        || run.color.is_some()
        || run.languages != joyword_model::Languages::default()
        || run.fonts.complex_script.is_some()
        || run.fonts.hint.is_some()
        || run.preserved.is_some()
        || font_is_theme(&run.fonts.ascii)
        || font_is_theme(&run.fonts.h_ansi)
        || font_is_theme(&run.fonts.east_asia);
    let kept = RunProps {
        bold: run.bold,
        size: run.size,
        fonts: joyword_model::FontSlots {
            ascii: named_font(run.fonts.ascii),
            h_ansi: named_font(run.fonts.h_ansi),
            east_asia: named_font(run.fonts.east_asia),
            ..joyword_model::FontSlots::default()
        },
        ..RunProps::default()
    };
    (kept, extra)
}

fn font_is_theme(font: &Option<FontRef>) -> bool {
    matches!(font, Some(FontRef::Theme(_)))
}

fn named_font(font: Option<FontRef>) -> Option<FontRef> {
    match font {
        Some(FontRef::Named(name)) => Some(FontRef::Named(name)),
        _ => None,
    }
}

fn attr_u32(element: &Element, name: &str) -> Option<u32> {
    element.attribute(Some(W), name)?.parse().ok()
}

fn attr_u8(element: &Element, name: &str) -> Option<u8> {
    element.attribute(Some(W), name)?.parse().ok()
}
