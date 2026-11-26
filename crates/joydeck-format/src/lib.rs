//! JoyDeck model archives and the interchange formats used by the editor.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![forbid(unsafe_code)]

pub mod clipboard;
mod legacy_numbers;
mod manifest_json;
mod project;
mod project_repair;
pub use project_repair::{SLIDE_SIDE, repair, sane_numbers, sane_slide_size};
pub mod rtf;

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test assertions"
)]
mod chart_parts_tests;

use std::sync::Arc;

use joydeck_model::text::{Paragraph, TextBody};
use joydeck_model::{LayoutType, Presentation, ShapeKind, defaults};

#[cfg(test)]
use legacy_numbers::decode as parse_manifest;
#[cfg(test)]
use project::Manifest;
pub use project::{load_checked, load_parts, save};

#[cfg(test)]
use project::{Budget, load_within, read_entry, read_entry_max};
#[cfg(test)]
use std::io::{Cursor, Read, Write};

pub const MIMETYPE: &str = "application/vnd.storyteller.deckcraft+zip";
/// Files written before the SlideCraft → DeckCraft rename (`.slidecraft`) are still read.
pub const LEGACY_MIMETYPE: &str = concat!("application/vnd.storyteller.", "slide", "craft+zip");
pub const EXTENSION: &str = "deckcraft";
/// Format version written into the manifest.
pub const VERSION: u32 = 1;
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("not a DeckCraft file: {0}")]
    NotOurs(String),
    #[error("damaged file: {0}")]
    Damaged(String),
    #[error("write failed: {0}")]
    Write(String),
}

pub type Result<T> = std::result::Result<T, FormatError>;

/// Does this look like a `.deckcraft` file?
pub fn sniff(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK")
        && [MIMETYPE, LEGACY_MIMETYPE]
            .iter()
            .any(|m| bytes.windows(m.len()).take(200).any(|w| w == m.as_bytes()))
}

/// Read a presentation. Missing media/chart parts are left empty; use [`load_checked`] to
/// learn which. A damaged chart member refuses the file before further chart inflation.
pub fn load(bytes: &[u8]) -> Result<Presentation> {
    load_checked(bytes).map(|(p, _)| p)
}

/// Slides read from an outline: each title with its body paragraphs (level 0–8, text).
type OutlineSlides = Vec<(String, Vec<(u8, String)>)>;

/// Append a Title and Content slide per outline entry; the number made.
fn add_outline_slides(p: &mut Presentation, slides: OutlineSlides) -> usize {
    let Some(lay) = defaults::layout_of_kind(p, LayoutType::TitleAndContent) else {
        return 0;
    };
    let mut made = 0;
    for (title, body) in slides {
        // An exhausted id space makes no new slides: what fit is kept (#407).
        let Some(mut s) = defaults::new_slide(p, lay) else {
            break;
        };
        if let Some(t) = s.shapes.first_mut() {
            t.text = Some(TextBody::from_text(&title));
        }
        if let Some(b) = s.shapes.get_mut(1) {
            for (level, line) in body {
                let tb = b.text.get_or_insert_with(TextBody::default);
                let mut para = Paragraph::new(&line);
                para.level = level.min(8);
                tb.paragraphs.push(para);
            }
        }
        p.slides.push(Arc::new(s));
        made += 1;
    }
    made
}

/// Outline text: one slide per unindented line; tab-indented lines become body bullets at their
/// level (Insert ▸ Slides from Outline, View ▸ Outline copy).
pub fn outline_to_slides(p: &mut Presentation, text: &str) -> usize {
    let mut slides: OutlineSlides = Vec::new();
    for raw in text.lines() {
        if raw.trim().is_empty() {
            continue;
        }
        let tabs = raw.chars().take_while(|c| *c == '\t').count();
        let spaces = raw.chars().take_while(|c| *c == ' ').count() / 2;
        let level = tabs.max(spaces);
        let t = raw.trim();
        let line = match t.strip_prefix(['-', '*', '•']) {
            Some(rest) if rest.starts_with([' ', '\t']) => rest.trim(),
            _ => t,
        }
        .to_string();
        if level == 0 {
            slides.push((line, Vec::new()));
        } else if let Some((_, body)) = slides.last_mut() {
            body.push(((level - 1).min(8) as u8, line));
        }
    }
    add_outline_slides(p, slides)
}

/// An ATX heading's text (CommonMark: up to three spaces, one to six `#`, then a space or the end;
/// a closing run of `#` is dropped).
fn md_heading(line: &str) -> Option<&str> {
    let t = line
        .strip_prefix("   ")
        .or_else(|| line.strip_prefix("  "))
        .or_else(|| line.strip_prefix(' '))
        .unwrap_or(line);
    let hashes = t.chars().take_while(|c| *c == '#').count();
    let rest = t.get(hashes..)?;
    if !(1..=6).contains(&hashes) || !(rest.is_empty() || rest.starts_with([' ', '\t'])) {
        return None;
    }
    let rest = rest.trim();
    let closed = rest.trim_end_matches('#');
    Some(if closed.is_empty() || closed.ends_with([' ', '\t']) {
        closed.trim_end()
    } else {
        rest
    })
}

/// A list item's nesting level (two spaces or a tab each) and text: `-`, `*` or `+`, or digits
/// then `.` or `)`, followed by a space.
fn md_list_item(line: &str) -> Option<(u8, &str)> {
    let indent: usize = line
        .chars()
        .take_while(|c| matches!(c, ' ' | '\t'))
        .map(|c| if c == '\t' { 2 } else { 1 })
        .sum();
    let t = line.trim_start();
    let rest = match t.strip_prefix(['-', '*', '+']) {
        Some(r) => r,
        None => {
            let digits = t.chars().take_while(char::is_ascii_digit).count();
            if digits == 0 || digits > 9 {
                return None;
            }
            t.get(digits..)?.strip_prefix(['.', ')'])?
        }
    };
    rest.starts_with([' ', '\t'])
        .then(|| ((indent / 2).min(8) as u8, rest.trim()))
}

/// Markdown outline text (`.md`): each heading starts a slide with its text as the title; list
/// items under it become bullets nested by their indentation, other lines body paragraphs. Text
/// before the first heading is skipped, as indented lines before the first title are in the
/// plain format. A file without headings is read as a plain outline ([`outline_to_slides`]).
pub fn markdown_to_slides(p: &mut Presentation, text: &str) -> usize {
    if !text.lines().any(|l| md_heading(l).is_some()) {
        return outline_to_slides(p, text);
    }
    let mut slides: OutlineSlides = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(title) = md_heading(line) {
            slides.push((title.to_string(), Vec::new()));
        } else if let Some((_, body)) = slides.last_mut() {
            let (level, text) = md_list_item(line).unwrap_or((0, line.trim()));
            body.push((level, text.to_string()));
        }
    }
    add_outline_slides(p, slides)
}

/// The deck as outline text (titles unindented, body paragraphs tab-indented by level).
pub fn slides_to_outline(p: &Presentation) -> String {
    let mut out = String::new();
    for s in &p.slides {
        // One line per title: a newline would start another slide on re-import.
        out.push_str(&s.title().replace('\n', " "));
        out.push('\n');
        for sh in &s.shapes {
            if sh.ph_type().is_some_and(|k| k.is_title()) {
                continue;
            }
            if let Some(t) = &sh.text {
                for para in &t.paragraphs {
                    if para.is_empty() {
                        continue;
                    }
                    for _ in 0..=para.level {
                        out.push('\t');
                    }
                    out.push_str(&para.text().replace('\u{b}', " "));
                    out.push('\n');
                }
            }
            if let ShapeKind::Table(t) = &sh.kind {
                for row in &t.rows {
                    let cells: Vec<String> = row
                        .cells
                        .iter()
                        .map(|c| c.text.text().replace(['\u{b}', '\n'], " "))
                        .collect();
                    out.push('\t');
                    out.push_str(&cells.join("\t"));
                    out.push('\n');
                }
            }
        }
    }
    out
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test assertions"
)]
mod tests;

#[test]
#[allow(clippy::unwrap_used, clippy::expect_used, reason = "Test assertions")]
fn outline_table_rows_are_indented_and_do_not_become_slides() {
    use joydeck_model::table::{Cell, Row, Table};
    use joydeck_model::{Shape, ShapeKind, Xfrm};
    let mut p = defaults::new_presentation(None);
    let layout = defaults::layout_of_kind(&p, LayoutType::TitleAndContent).unwrap();
    let mut s = defaults::new_slide(&mut p, layout).expect("slide");
    for sh in s
        .shapes
        .iter_mut()
        .filter(|sh| sh.ph_type().is_some_and(|k| k.is_title()))
    {
        sh.text = Some(TextBody::from_text("Slide title"));
    }
    let mut table = Table {
        cols: vec![200.0, 200.0],
        ..Default::default()
    };
    for texts in [["A1", "B1"], ["A2", "B2"]] {
        let cells: Vec<Cell> = texts
            .iter()
            .map(|t| Cell {
                text: TextBody::from_text(t),
                ..Default::default()
            })
            .collect();
        table.rows.push(Row {
            height: 30.0,
            cells,
        });
    }
    let mut tbl = Shape {
        xfrm: Some(Xfrm::new(0.0, 100.0, 400.0, 60.0)),
        ..Default::default()
    };
    tbl.kind = ShapeKind::Table(table);
    s.shapes.push(tbl);
    p.slides = vec![std::sync::Arc::new(s)];

    let out = slides_to_outline(&p);
    let mut q = defaults::new_presentation(None);
    q.slides.clear();
    assert_eq!(
        outline_to_slides(&mut q, &out),
        1,
        "table rows became slides: {out:?}"
    );
    assert_eq!(q.slides.len(), 1, "{out:?}");
    let body = q.slides[0]
        .shapes
        .iter()
        .filter_map(|sh| sh.text.as_ref())
        .map(|t| t.text())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(body.contains("A1\tB1"), "{body:?}");
    assert!(body.contains("A2\tB2"), "{body:?}");
}
