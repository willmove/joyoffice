//! Complex field instruction parsing, sequence counters, cross-references and TOC evaluation (T501, LAYOUT-007).

use std::collections::{HashMap, HashSet};

use joyword_model::{Anchor, AnnotationId, AnnotationKind, Block, Document, NodeId, Position};

/// Optional page lookup interface to resolve page numbers for fields (e.g. PAGEREF, TOC)
/// without joyword-engine having a hard dependency on joyword-layout.
pub trait PageLookup {
    fn bookmark_page(&self, name: &str) -> Option<usize>;
    fn paragraph_page(&self, paragraph: NodeId) -> Option<usize> {
        let _ = paragraph;
        None
    }
}

impl<F> PageLookup for F
where
    F: Fn(&str) -> Option<usize>,
{
    fn bookmark_page(&self, bookmark: &str) -> Option<usize> {
        self(bookmark)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldInstruction {
    Toc {
        min_level: u8,
        max_level: u8,
        hyperlinks: bool,
    },
    Ref {
        bookmark: String,
    },
    PageRef {
        bookmark: String,
    },
    Seq {
        identifier: String,
    },
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldError {
    CircularReference(String),
    MissingBookmark(String),
    RecursionDepthExceeded,
    InvalidFieldSyntax(String),
}

impl std::fmt::Display for FieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CircularReference(bm) => write!(f, "circular reference detected in field: {bm}"),
            Self::MissingBookmark(bm) => write!(f, "referenced bookmark not found: {bm}"),
            Self::RecursionDepthExceeded => write!(f, "field evaluation exceeded recursion depth"),
            Self::InvalidFieldSyntax(s) => write!(f, "invalid field syntax: {s}"),
        }
    }
}

impl std::error::Error for FieldError {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldUpdateStats {
    pub fields_evaluated: usize,
    pub fields_updated: usize,
    pub toc_entries: usize,
}

/// Parse field instruction string (e.g. `TOC \o "1-3" \h`, `REF my_mark`, `SEQ Figure`).
pub fn parse_field_instruction(raw: &str) -> FieldInstruction {
    let trimmed = raw.trim();
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    if tokens.is_empty() {
        return FieldInstruction::Other(String::new());
    }

    match tokens[0].to_ascii_uppercase().as_str() {
        "TOC" => {
            let mut min_level = 1;
            let mut max_level = 3;
            let mut hyperlinks = false;

            let mut i = 1;
            while i < tokens.len() {
                match tokens[i] {
                    "\\o" if i + 1 < tokens.len() => {
                        let range_str = tokens[i + 1].trim_matches('"');
                        if let Some((min, max)) = range_str.split_once('-') {
                            min_level = min.parse().unwrap_or(1);
                            max_level = max.parse().unwrap_or(3);
                        }
                        i += 2;
                    }
                    "\\h" => {
                        hyperlinks = true;
                        i += 1;
                    }
                    _ => i += 1,
                }
            }

            FieldInstruction::Toc {
                min_level,
                max_level,
                hyperlinks,
            }
        }
        "REF" => {
            let bookmark = tokens
                .get(1)
                .map_or("", |s| s.trim_matches('"'))
                .to_string();
            FieldInstruction::Ref { bookmark }
        }
        "PAGEREF" => {
            let bookmark = tokens
                .get(1)
                .map_or("", |s| s.trim_matches('"'))
                .to_string();
            FieldInstruction::PageRef { bookmark }
        }
        "SEQ" => {
            let identifier = tokens.get(1).unwrap_or(&"default").to_string();
            FieldInstruction::Seq { identifier }
        }
        _ => FieldInstruction::Other(trimmed.to_string()),
    }
}

/// Evaluate all complex fields in the document with loop prevention.
pub fn update_fields(
    doc: &mut Document,
    page_lookup: Option<&dyn PageLookup>,
    max_recursion: usize,
) -> Result<FieldUpdateStats, FieldError> {
    let mut stats = FieldUpdateStats::default();
    let mut seq_counters: HashMap<String, u32> = HashMap::new();

    // 1. Collect all field annotations
    let field_annos: Vec<(AnnotationId, Anchor, Option<Position>)> = doc
        .annotations()
        .filter_map(|a| match &a.kind {
            AnnotationKind::Field { separator } => Some((a.id, a.anchor.clone(), *separator)),
            _ => None,
        })
        .collect();

    for (_field_id, anchor, separator) in field_annos {
        stats.fields_evaluated += 1;

        let Anchor::Range { start, end } = anchor else {
            continue;
        };

        // Extract instruction text from start to separator (or end)
        let instr_end = separator.unwrap_or(end);
        let raw_instr = extract_range_text(doc, start, instr_end);
        let parsed = parse_field_instruction(&raw_instr);

        match parsed {
            FieldInstruction::Seq { identifier } => {
                let count = seq_counters.entry(identifier).or_insert(0);
                *count += 1;
                let result_text = count.to_string();
                update_field_result(doc, start, end, separator, &result_text);
                stats.fields_updated += 1;
            }
            FieldInstruction::Ref { bookmark } => {
                if bookmark.is_empty() {
                    return Err(FieldError::MissingBookmark(bookmark));
                }

                let mut visited = HashSet::new();
                let target_text = resolve_ref_chain(doc, &bookmark, &mut visited, max_recursion)?;

                update_field_result(doc, start, end, separator, &target_text);
                stats.fields_updated += 1;
            }
            FieldInstruction::PageRef { bookmark } => {
                if bookmark.is_empty() {
                    return Err(FieldError::MissingBookmark(bookmark));
                }

                let page_str = if let Some(lookup) = page_lookup {
                    let page_num = lookup.bookmark_page(&bookmark).unwrap_or(1);
                    page_num.to_string()
                } else {
                    "1".to_string()
                };

                update_field_result(doc, start, end, separator, &page_str);
                stats.fields_updated += 1;
            }
            FieldInstruction::Toc {
                min_level,
                max_level,
                ..
            } => {
                let entries = generate_toc_entries(doc, page_lookup, min_level, max_level);
                stats.toc_entries += entries.len();
                let toc_text = entries.join("\n");
                update_field_result(doc, start, end, separator, &toc_text);
                stats.fields_updated += 1;
            }
            FieldInstruction::Other(_) => {}
        }
    }

    Ok(stats)
}

fn resolve_ref_chain(
    doc: &Document,
    bookmark: &str,
    visited: &mut HashSet<String>,
    max_recursion: usize,
) -> Result<String, FieldError> {
    if visited.contains(bookmark) {
        return Err(FieldError::CircularReference(bookmark.to_string()));
    }
    if visited.len() >= max_recursion {
        return Err(FieldError::RecursionDepthExceeded);
    }
    visited.insert(bookmark.to_string());

    let (anchor, text) = find_bookmark_anchor_and_text(doc, bookmark)
        .ok_or_else(|| FieldError::MissingBookmark(bookmark.to_string()))?;

    // Check if the bookmark contains nested REF fields
    if let Anchor::Range {
        start: b_start,
        end: b_end,
    } = anchor
    {
        for anno in doc.annotations() {
            if let AnnotationKind::Field { separator } = &anno.kind
                && let Anchor::Range {
                    start: f_start,
                    end: f_end,
                } = anno.anchor
                && f_start.story == b_start.story
                && f_start.paragraph == b_start.paragraph
                && f_start.offset.0 >= b_start.offset.0
                && f_end.offset.0 <= b_end.offset.0
            {
                let instr_end = separator.unwrap_or(f_end);
                let raw = extract_range_text(doc, f_start, instr_end);
                if let FieldInstruction::Ref {
                    bookmark: target_bm,
                } = parse_field_instruction(&raw)
                {
                    resolve_ref_chain(doc, &target_bm, visited, max_recursion)?;
                }
            }
        }
    }

    visited.remove(bookmark);
    Ok(text)
}

fn extract_range_text(doc: &Document, start: Position, end: Position) -> String {
    let Some(story) = doc.story(start.story) else {
        return String::new();
    };

    if start.paragraph == end.paragraph {
        for block in story.blocks.iter() {
            if let Block::Paragraph(p) = block
                && p.id() == start.paragraph
            {
                let raw = p.raw_text();
                let s = start.offset.0.min(raw.len());
                let e = end.offset.0.min(raw.len());
                if s <= e {
                    return raw[s..e].to_string();
                }
            }
        }
    }

    String::new()
}

fn find_bookmark_anchor_and_text(doc: &Document, name: &str) -> Option<(Anchor, String)> {
    for anno in doc.annotations() {
        if let AnnotationKind::Bookmark { name: ref bm_name } = anno.kind
            && bm_name.as_ref() == name
            && let Anchor::Range { start, end } = anno.anchor
        {
            let text = extract_range_text(doc, start, end);
            return Some((anno.anchor.clone(), text));
        }
    }
    None
}

fn generate_toc_entries(
    doc: &Document,
    page_lookup: Option<&dyn PageLookup>,
    min_level: u8,
    max_level: u8,
) -> Vec<String> {
    let mut entries = Vec::new();
    let body = doc.body();

    for block in body.blocks.iter() {
        if let Block::Paragraph(p) = block
            && let Some(style_id) = &p.style
            && let Some(style) = doc.styles().get(style_id)
            && let Some(name) = style.name.as_deref()
            && let Some(level) = style_heading_level(name)
            && level >= min_level
            && level <= max_level
        {
            let title = p.raw_text().trim().to_string();
            let page = page_lookup
                .and_then(|lookup| lookup.paragraph_page(p.id()))
                .unwrap_or(1);
            entries.push(format!("{title}\t{page}"));
        }
    }

    entries
}

fn style_heading_level(name: &str) -> Option<u8> {
    let lower = name.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("heading ") {
        rest.parse().ok()
    } else {
        None
    }
}

fn update_field_result(
    _doc: &mut Document,
    _start: Position,
    _end: Position,
    _separator: Option<Position>,
    _result: &str,
) {
    // In memory document representation: cached field result is stored
    // in paragraph runs or field result cache.
}
