//! Numbering part writeback (ECMA-376 Part 1 §17.9).
//!
//! `w:numbering` is `abstractNum*`, `num*`, `numIdMacAtCleanup?` (§17.9.17);
//! a `w:lvl` is `start`, `numFmt`, `lvlRestart?`, `pStyle?`, `isLgl?`,
//! `suff?`, `lvlText`, `lvlPicBulletId?`, `legacy?`, `lvlJc?`, `pPr?`, `rPr?`
//! (§17.9.11); a `w:num` is `abstractNumId` then `lvlOverride*` (§17.9.19).
//!
//! The store loses source details it does not model, so the part is never
//! regenerated. An unchanged store keeps the original bytes: this module
//! returns no write and the package writer copies the part verbatim. A store
//! that grew only by appended definitions is written by inserting markup for
//! the new `w:abstractNum` and `w:num` entries after the last of their kind;
//! every other byte of the part stays untouched, including unmodeled children
//! such as `nsid` or `lvlJc`. When the package has no numbering part, one is
//! created next to the main part and the content-types Override and the
//! relationship are appended, following the comment-part pattern.
//!
//! Anything else is refused with a reason, never silently written: modified
//! or removed existing entries, a changed `unmodeled` flag, linked
//! definitions (`numStyleLink`/`styleLink`, §17.9.14/§17.9.15), picture
//! bullets, and level properties outside the subset the importer keeps
//! (`NumberingLevel::unmodeled`). New-entry id allocation and the bullet
//! preset come from `NumberingStore::ensure_bullet`; the ids it picks
//! (highest existing id plus one) are a JoyWord assumption, not a Word
//! observation.

use joyoffice_opc::{Budget, Package, Part, PartName, Target};
use joyword_model::{
    AbstractNum, Document, FontRef, FontSlots, Indent, MAX_LEVEL, Measure, NumInstance,
    NumberSuffix, NumberingLevel, NumberingStore, ParaProps, RunProps, TabAlign, TabLeader,
};

use super::{ExportError, Names, splice, unsupported};
use crate::ns::REL_NUMBERING;
use crate::xml::{Element, XmlReader};
use crate::{ImportError, W};

/// Content type of the numbering part (ECMA-376 Part 2 §11.2).
const NUMBERING_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";

/// Why a store difference cannot be written.
const REWRITTEN: &str = "existing numbering definitions are not rewritten";
/// Why one new entry cannot be serialized.
const OUTSIDE: &str = "numbering definition is outside the written subset";

pub(super) struct NumberingWrite {
    /// Replacement of the package's existing numbering part. Mutually
    /// exclusive with `created`.
    pub parts: Vec<(PartName, Vec<u8>)>,
    /// ZIP item names of parts that do not exist in the package yet.
    pub created: Vec<(String, Vec<u8>)>,
    /// Byte growth of the parts relative to the package.
    pub delta: i128,
    /// True when a numbering relationship must be appended to the main
    /// part's relationships part.
    pub relationship: bool,
    /// The part whose content-types Override must be appended.
    pub content_type: Option<PartName>,
}

/// Plans the numbering part writeback, or `None` when the store is
/// unchanged and the original bytes are kept.
pub(super) fn plan(
    package: &Package,
    main_name: &PartName,
    original: &Document,
    current: &Document,
    budget: &Budget,
) -> Result<Option<NumberingWrite>, ExportError> {
    let before = original.numbering();
    let after = current.numbering();
    if before == after {
        return Ok(None);
    }
    let (abstracts, instances) = appended(before, after)?;
    let write = match existing_part(package, main_name)? {
        Some(part) => patch_part(part, &abstracts, &instances, after, budget)?,
        None => create_part(package, main_name, &abstracts, &instances, after, budget)?,
    };
    Ok(Some(write))
}

/// Appends the numbering relationship to the main part's relationships when
/// a part is created. `rels` is the byte state produced by earlier writers
/// (new hyperlinks), so their appended ids are kept and the numbering id
/// follows them.
pub(super) fn extend_relationships(
    package: &Package,
    main_name: &PartName,
    rels: Option<Vec<u8>>,
    write: &Option<NumberingWrite>,
    budget: &Budget,
) -> Result<Option<Vec<u8>>, ExportError> {
    let Some(write) = write else {
        return Ok(rels);
    };
    if !write.relationship {
        return Ok(rels);
    }
    let rels_name = main_name.relationships_part();
    let base = match rels {
        Some(bytes) => bytes,
        None => package
            .part(rels_name.as_str())
            .map(|part| part.data.to_vec())
            .ok_or_else(|| unsupported(None, "numbering relationships part is missing"))?,
    };
    let appended = add_relationship(&base, REL_NUMBERING, "numbering.xml")?;
    if appended.len() as u64 > budget.max_part_bytes {
        return Err(ExportError::Budget {
            part: rels_name.to_string(),
            limit: budget.max_part_bytes,
        });
    }
    Ok(Some(appended))
}

/// Appends the content-types Override for a created part. `base` is the
/// byte state produced by earlier writers (comment parts), so their
/// Overrides are kept.
pub(super) fn extend_content_types(
    package: &Package,
    base: Option<Vec<u8>>,
    write: &Option<NumberingWrite>,
    budget: &Budget,
) -> Result<Option<Vec<u8>>, ExportError> {
    let Some(write) = write else {
        return Ok(base);
    };
    let Some(name) = &write.content_type else {
        return Ok(base);
    };
    let xml = base
        .as_deref()
        .unwrap_or_else(|| package.content_types_xml());
    let updated = add_override(xml, name.as_str(), NUMBERING_CONTENT_TYPE)?;
    if updated.len() as u64 > budget.max_part_bytes {
        return Err(ExportError::Budget {
            part: "[Content_Types].xml".into(),
            limit: budget.max_part_bytes,
        });
    }
    Ok(Some(updated))
}

/// The store difference must be pure growth: the imported entries are an
/// unchanged prefix of the current entries in the same order, and the new
/// entries are the suffix. A modified, removed or reordered entry, or a
/// changed `unmodeled` flag, is refused; the part is never rebuilt.
fn appended<'a>(
    before: &NumberingStore,
    after: &'a NumberingStore,
) -> Result<(Vec<&'a AbstractNum>, Vec<&'a NumInstance>), ExportError> {
    if before.unmodeled != after.unmodeled {
        return Err(unsupported(None, REWRITTEN));
    }
    let old = before.abstracts();
    let all = after.abstracts();
    if all.len() < old.len() || !old.iter().zip(all).all(|(left, right)| left == right) {
        return Err(unsupported(None, REWRITTEN));
    }
    let new_abstracts = &all[old.len()..];
    let old = before.instances();
    let all = after.instances();
    if all.len() < old.len() || !old.iter().zip(all).all(|(left, right)| left == right) {
        return Err(unsupported(None, REWRITTEN));
    }
    let new_instances = &all[old.len()..];
    if new_abstracts.is_empty() && new_instances.is_empty() {
        return Err(unsupported(None, REWRITTEN));
    }
    Ok((
        new_abstracts.iter().collect(),
        new_instances.iter().collect(),
    ))
}

/// The part the importer read: the first internal numbering relationship of
/// the main part whose target exists. A relationship whose target is
/// missing is refused rather than worked around, because appending to a
/// dangling relationship or creating a rival part would both leave the
/// re-import reading something other than what was written.
fn existing_part<'a>(
    package: &'a Package,
    main_name: &PartName,
) -> Result<Option<&'a Part>, ExportError> {
    let mut dangling = false;
    for relationship in package.relationships(Some(main_name)) {
        if relationship.kind.as_ref() != REL_NUMBERING {
            continue;
        }
        let Target::Internal(target) = &relationship.target else {
            continue;
        };
        if let Some(part) = package.part(target.as_str()) {
            return Ok(Some(part));
        }
        dangling = true;
    }
    if dangling {
        return Err(unsupported(
            None,
            "numbering relationship target is missing",
        ));
    }
    Ok(None)
}

fn patch_part(
    part: &Part,
    abstracts: &[&AbstractNum],
    instances: &[&NumInstance],
    store: &NumberingStore,
    budget: &Budget,
) -> Result<NumberingWrite, ExportError> {
    let root = XmlReader::new(part.name.as_str(), &part.data, budget)
        .and_then(XmlReader::whole)
        .map_err(|error| ImportError::Xml {
            part: part.name.to_string(),
            error,
        })?;
    if !root.is(W, "numbering") {
        return Err(unsupported(None, "numbering part is not w:numbering"));
    }
    let names = Names::in_scope(&[&root]);
    let (abstract_markup, num_markup) = entries_markup(&names, abstracts, instances, store)?;
    let bytes = splice_numbering(&part.data, &root, &abstract_markup, &num_markup)?;
    if bytes.len() as u64 > budget.max_part_bytes {
        return Err(ExportError::Budget {
            part: part.name.to_string(),
            limit: budget.max_part_bytes,
        });
    }
    let delta = bytes.len() as i128 - part.data.len() as i128;
    Ok(NumberingWrite {
        parts: vec![(part.name.clone(), bytes)],
        created: Vec::new(),
        delta,
        relationship: false,
        content_type: None,
    })
}

fn create_part(
    package: &Package,
    main_name: &PartName,
    abstracts: &[&AbstractNum],
    instances: &[&NumInstance],
    store: &NumberingStore,
    budget: &Budget,
) -> Result<NumberingWrite, ExportError> {
    // `directory()` keeps its trailing slash (`/word/`).
    let name = PartName::new(&format!("{}numbering.xml", main_name.directory()))
        .map_err(|_| unsupported(None, "numbering part name is invalid"))?;
    if package.part(name.as_str()).is_some() {
        return Err(unsupported(
            None,
            "numbering part exists without a relationship",
        ));
    }
    let names = Names::Bound("w".into());
    let (abstract_markup, num_markup) = entries_markup(&names, abstracts, instances, store)?;
    let bytes =
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:numbering xmlns:w=\"{W}\">{abstract_markup}{num_markup}</w:numbering>")
            .into_bytes();
    if bytes.len() as u64 > budget.max_part_bytes {
        return Err(ExportError::Budget {
            part: name.to_string(),
            limit: budget.max_part_bytes,
        });
    }
    Ok(NumberingWrite {
        parts: Vec::new(),
        created: vec![(name.item_name().to_owned(), bytes.clone())],
        delta: bytes.len() as i128,
        relationship: true,
        content_type: Some(name),
    })
}

/// Serializes the new entries and marks where the `w:num` markup begins, so
/// an existing part can insert the two groups at their own positions.
fn entries_markup(
    names: &Names,
    abstracts: &[&AbstractNum],
    instances: &[&NumInstance],
    store: &NumberingStore,
) -> Result<(String, String), ExportError> {
    let mut abstract_markup = String::new();
    for item in abstracts {
        abstract_markup.push_str(&abstract_markup_of(names, item)?);
    }
    let mut num_markup = String::new();
    for item in instances {
        num_markup.push_str(&instance_markup(names, item, store)?);
    }
    Ok((abstract_markup, num_markup))
}

fn abstract_markup_of(names: &Names, item: &AbstractNum) -> Result<String, ExportError> {
    if item.unmodeled {
        return Err(unsupported(None, OUTSIDE));
    }
    if item.num_style_link.is_some() || item.style_link.is_some() {
        return Err(unsupported(
            None,
            "linked numbering definitions are not written",
        ));
    }
    let mut body = String::new();
    for (index, level) in item.levels.iter().enumerate() {
        let Some(level) = level else {
            continue;
        };
        if index as u8 > MAX_LEVEL {
            return Err(unsupported(None, OUTSIDE));
        }
        body.push_str(&level_markup(names, level, index as u8)?);
    }
    Ok(format!(
        "{}{}>{body}{}",
        names.start("abstractNum"),
        names.attribute("abstractNumId", &item.id.to_string()),
        names.end("abstractNum")
    ))
}

fn instance_markup(
    names: &Names,
    item: &NumInstance,
    store: &NumberingStore,
) -> Result<String, ExportError> {
    if !store
        .abstracts()
        .iter()
        .any(|entry| entry.id == item.abstract_id)
    {
        return Err(unsupported(
            None,
            "numbering instance names a missing definition",
        ));
    }
    let mut body = format!(
        "{}{}/>",
        names.start("abstractNumId"),
        names.attribute("val", &item.abstract_id.to_string())
    );
    for entry in &item.overrides {
        if entry.level > MAX_LEVEL {
            return Err(unsupported(None, OUTSIDE));
        }
        let mut inner = String::new();
        if let Some(start) = entry.start_override {
            inner.push_str(&void(names, "startOverride", &start.to_string()));
        }
        if let Some(level) = &entry.replacement {
            inner.push_str(&level_markup(names, level, entry.level)?);
        }
        body.push_str(&format!(
            "{}{}>{inner}{}",
            names.start("lvlOverride"),
            names.attribute("ilvl", &entry.level.to_string()),
            names.end("lvlOverride")
        ));
    }
    Ok(format!(
        "{}{}>{body}{}",
        names.start("num"),
        names.attribute("numId", &item.id.to_string()),
        names.end("num")
    ))
}

/// One `w:lvl` in the §17.9.11 sequence. Only the children the importer
/// keeps are written; a level carrying anything else (its `unmodeled` flag,
/// a picture bullet, unmapped `pPr`/`rPr` values) is refused.
fn level_markup(names: &Names, level: &NumberingLevel, index: u8) -> Result<String, ExportError> {
    if level.unmodeled {
        return Err(unsupported(None, OUTSIDE));
    }
    if level.picture.is_some() {
        return Err(unsupported(None, "picture bullets are not written"));
    }
    // The importer keeps indent and tabs on the level `pPr`, and bold, size
    // and the named latin/east-Asian fonts on the level `rPr`; anything else
    // set the `unmodeled` flag or was dropped, so an equal projection proves
    // nothing was lost.
    if level.para
        != (ParaProps {
            indent: level.para.indent,
            tabs: level.para.tabs.clone(),
            ..ParaProps::default()
        })
    {
        return Err(unsupported(None, OUTSIDE));
    }
    if level.run
        != (RunProps {
            bold: level.run.bold,
            size: level.run.size,
            fonts: FontSlots {
                ascii: level.run.fonts.ascii.clone(),
                h_ansi: level.run.fonts.h_ansi.clone(),
                east_asia: level.run.fonts.east_asia.clone(),
                ..FontSlots::default()
            },
            ..RunProps::default()
        })
    {
        return Err(unsupported(None, OUTSIDE));
    }
    for slot in [
        &level.run.fonts.ascii,
        &level.run.fonts.h_ansi,
        &level.run.fonts.east_asia,
    ] {
        if matches!(slot, Some(FontRef::Theme(_))) {
            return Err(unsupported(None, "theme fonts are not written"));
        }
    }
    let token = level.format.token();
    attribute_value(&token)?;
    if token.is_empty() {
        return Err(unsupported(None, OUTSIDE));
    }
    attribute_value(&level.text)?;
    let mut body = String::new();
    body.push_str(&void(names, "start", &level.start.to_string()));
    body.push_str(&void(names, "numFmt", &token));
    if let Some(restart) = level.restart {
        body.push_str(&void(names, "lvlRestart", &restart.to_string()));
    }
    if level.suffix != NumberSuffix::Tab {
        // `tab` is the §17.9.11 default and is left out, as on import.
        body.push_str(&void(names, "suff", level.suffix.token()));
    }
    body.push_str(&void(names, "lvlText", &level.text));
    body.push_str(&paragraph_markup(names, &level.para)?);
    body.push_str(&run_markup(names, &level.run)?);
    Ok(format!(
        "{}{}>{body}{}",
        names.start("lvl"),
        names.attribute("ilvl", &index.to_string()),
        names.end("lvl")
    ))
}

/// Level `w:pPr`: §17.9.11 defers to `CT_PPrBase`, whose sequence puts
/// `tabs` before `ind`.
fn paragraph_markup(names: &Names, para: &ParaProps) -> Result<String, ExportError> {
    let mut inner = String::new();
    if let Some(tabs) = &para.tabs {
        let mut body = String::new();
        for stop in tabs {
            let align = match stop.align {
                TabAlign::Left => "left",
                TabAlign::Center => "center",
                TabAlign::Right => "right",
                TabAlign::Decimal => "decimal",
                TabAlign::Bar => "bar",
                TabAlign::Num => "num",
                TabAlign::Clear => "clear",
            };
            let Measure::Twips(position) = stop.position else {
                return Err(unsupported(None, "numbering tab positions must be twips"));
            };
            let mut markup = format!(
                "{}{}{}",
                names.start("tab"),
                names.attribute("val", align),
                names.attribute("pos", &position.to_string())
            );
            // `none` is the §17.18.56 default and is left out, as on import.
            let leader = match stop.leader {
                TabLeader::None => None,
                TabLeader::Dot => Some("dot"),
                TabLeader::Hyphen => Some("hyphen"),
                TabLeader::Underscore => Some("underscore"),
                TabLeader::MiddleDot => Some("middleDot"),
                TabLeader::Heavy => Some("heavy"),
            };
            if let Some(leader) = leader {
                markup.push_str(&names.attribute("leader", leader));
            }
            markup.push_str("/>");
            body.push_str(&markup);
        }
        inner.push_str(&format!(
            "{}>{body}{}",
            names.start("tabs"),
            names.end("tabs")
        ));
    }
    inner.push_str(&indent_markup(names, &para.indent)?);
    if inner.is_empty() {
        return Ok(String::new());
    }
    Ok(format!(
        "{}>{inner}{}",
        names.start("pPr"),
        names.end("pPr")
    ))
}

/// `w:ind`. The importer prefers `w:start`/`w:end` over `w:left`/`w:right`
/// and relative `*Chars` values over absolute ones, so only the absolute
/// twips attributes are written and any other unit is refused.
fn indent_markup(names: &Names, indent: &Indent) -> Result<String, ExportError> {
    let mut markup = names.start("ind");
    let mut any = false;
    for (local, measure) in [
        ("start", indent.start),
        ("end", indent.end),
        ("firstLine", indent.first_line),
        ("hanging", indent.hanging),
    ] {
        let Some(measure) = measure else {
            continue;
        };
        let Measure::Twips(value) = measure else {
            return Err(unsupported(None, "numbering indents must be twips"));
        };
        markup.push_str(&names.attribute(local, &value.to_string()));
        any = true;
    }
    if !any {
        return Ok(String::new());
    }
    markup.push_str("/>");
    Ok(markup)
}

/// Level `w:rPr`: `CT_RPrBase` orders `rFonts` before `b` before `sz`.
fn run_markup(names: &Names, run: &RunProps) -> Result<String, ExportError> {
    let mut inner = String::new();
    if [&run.fonts.ascii, &run.fonts.h_ansi, &run.fonts.east_asia]
        .into_iter()
        .any(Option::is_some)
    {
        let mut markup = names.start("rFonts");
        for (local, slot) in [
            ("ascii", &run.fonts.ascii),
            ("hAnsi", &run.fonts.h_ansi),
            ("eastAsia", &run.fonts.east_asia),
        ] {
            if let Some(FontRef::Named(name)) = slot {
                attribute_value(name)?;
                markup.push_str(&names.attribute(local, name));
            }
        }
        markup.push_str("/>");
        inner.push_str(&markup);
    }
    match run.bold {
        // Absence imports as `None`; off needs the explicit `w:val="0"`.
        Some(true) => inner.push_str(&format!("{}/>", names.start("b"))),
        Some(false) => inner.push_str(&format!(
            "{}{}/>",
            names.start("b"),
            names.attribute("val", "0")
        )),
        None => {}
    }
    if let Some(size) = run.size {
        let Measure::HalfPoints(value) = size else {
            return Err(unsupported(
                None,
                "numbering label sizes must be half-points",
            ));
        };
        inner.push_str(&void(names, "sz", &value.to_string()));
    }
    if inner.is_empty() {
        return Ok(String::new());
    }
    Ok(format!(
        "{}>{inner}{}",
        names.start("rPr"),
        names.end("rPr")
    ))
}

fn void(names: &Names, local: &str, value: &str) -> String {
    format!("{}{}/>", names.start(local), names.attribute("val", value))
}

/// XML 1.0 §3.3.3 normalizes tab, CR and LF in attribute values to spaces
/// on the next read, so a value carrying them would re-import changed. The
/// C1 range and noncharacters are refused with it rather than written.
fn attribute_value(value: &str) -> Result<(), ExportError> {
    if value
        .chars()
        .any(|ch| ch.is_control() || matches!(ch, '\u{FFFE}' | '\u{FFFF}'))
    {
        return Err(unsupported(
            None,
            "numbering text contains a character XML cannot round-trip",
        ));
    }
    Ok(())
}

/// Inserts the new `w:abstractNum` entries after the last existing one (or
/// before the first `w:num`, other child or the end tag) and the new `w:num`
/// entries after the last existing one (or before the first other child or
/// the end tag), keeping the §17.9.17 group order. All other bytes,
/// including whitespace and unmodeled children, stay untouched.
fn splice_numbering(
    xml: &[u8],
    root: &Element,
    abstracts: &str,
    nums: &str,
) -> Result<Vec<u8>, ExportError> {
    let raw = &xml[root.span.clone()];
    if raw.ends_with(b"/>") {
        // Self-closing root: only `/>` is replaced.
        let mut replacement = Vec::new();
        replacement.push(b'>');
        replacement.extend_from_slice(abstracts.as_bytes());
        replacement.extend_from_slice(nums.as_bytes());
        replacement.extend_from_slice(format!("</{}>", root.qname).as_bytes());
        return Ok(splice(
            xml,
            (root.span.end - 2)..root.span.end,
            &replacement,
        ));
    }
    let relative = raw
        .iter()
        .rposition(|&byte| byte == b'<')
        .ok_or_else(|| unsupported(None, "numbering element has no end tag"))?;
    let end_tag = root.span.start + relative;
    if !xml[end_tag..root.span.end].starts_with(b"</") {
        return Err(unsupported(None, "numbering element has no end tag"));
    }
    let mut last_abstract = None;
    let mut first_num = None;
    let mut last_num = None;
    let mut first_other = None;
    for child in root.elements() {
        if child.is(W, "abstractNum") {
            last_abstract = Some(child.span.end);
        } else if child.is(W, "num") {
            if first_num.is_none() {
                first_num = Some(child.span.start);
            }
            last_num = Some(child.span.end);
        } else if first_other.is_none() {
            first_other = Some(child.span.start);
        }
    }
    let abstract_at = (!abstracts.is_empty()).then(|| {
        last_abstract
            .or(first_num)
            .or(first_other)
            .unwrap_or(end_tag)
    });
    let num_at = (!nums.is_empty()).then(|| last_num.or(first_other).unwrap_or(end_tag));
    match (abstract_at, num_at) {
        (Some(at), None) => Ok(splice(xml, at..at, abstracts.as_bytes())),
        (None, Some(at)) => Ok(splice(xml, at..at, nums.as_bytes())),
        (Some(abstract_at), Some(num_at)) if abstract_at == num_at => Ok(splice(
            xml,
            abstract_at..abstract_at,
            format!("{abstracts}{nums}").as_bytes(),
        )),
        (Some(abstract_at), Some(num_at)) => {
            // Splice the later offset first so the earlier one stays valid.
            if abstract_at > num_at {
                let bytes = splice(xml, abstract_at..abstract_at, abstracts.as_bytes());
                Ok(splice(&bytes, num_at..num_at, nums.as_bytes()))
            } else {
                let bytes = splice(xml, num_at..num_at, nums.as_bytes());
                Ok(splice(
                    &bytes,
                    abstract_at..abstract_at,
                    abstracts.as_bytes(),
                ))
            }
        }
        (None, None) => Ok(xml.to_vec()),
    }
}

/// Appends an Override before `</Types>`, following the comment-part
/// pattern. An existing Override for the part name is left alone.
fn add_override(xml: &[u8], part: &str, content_type: &str) -> Result<Vec<u8>, ExportError> {
    let fail = || unsupported(None, "content types are not rewritten");
    let text = std::str::from_utf8(xml).map_err(|_| fail())?;
    if text.contains(&format!("PartName=\"{part}\"")) {
        return Ok(xml.to_vec());
    }
    let Some(pos) = text.rfind("</Types>") else {
        return Err(fail());
    };
    let extra = format!("<Override PartName=\"{part}\" ContentType=\"{content_type}\"/>");
    let mut out = String::with_capacity(text.len() + extra.len());
    out.push_str(&text[..pos]);
    out.push_str(&extra);
    out.push_str(&text[pos..]);
    Ok(out.into_bytes())
}

/// Appends a relationship before `</Relationships>`, following the
/// comment-part pattern. An existing relationship of the kind is left alone.
fn add_relationship(xml: &[u8], kind: &str, target: &str) -> Result<Vec<u8>, ExportError> {
    let fail = || unsupported(None, "numbering relationships part is not rewritten");
    let text = std::str::from_utf8(xml).map_err(|_| fail())?;
    if text.contains(kind) {
        return Ok(xml.to_vec());
    }
    let Some(pos) = text.rfind("</Relationships>") else {
        return Err(fail());
    };
    let id = next_relationship_id(text);
    let extra = format!("<Relationship Id=\"rId{id}\" Type=\"{kind}\" Target=\"{target}\"/>");
    let mut out = String::with_capacity(text.len() + extra.len());
    out.push_str(&text[..pos]);
    out.push_str(&extra);
    out.push_str(&text[pos..]);
    Ok(out.into_bytes())
}

/// One past the highest `rId` number in the part, as the comment writer
/// allocates ids.
fn next_relationship_id(xml: &str) -> u32 {
    let mut max = 0u32;
    let mut rest = xml;
    while let Some(pos) = rest.find("rId") {
        let after = &rest[pos + 3..];
        let digits: String = after.chars().take_while(|ch| ch.is_ascii_digit()).collect();
        if let Ok(value) = digits.parse::<u32>() {
            max = max.max(value);
        }
        rest = &rest[pos + 3..];
    }
    max.saturating_add(1)
}
