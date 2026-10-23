//! Model bookmarks and anchor-only links, not DOCX bookmark import or PDF tags.
//!
//! Sources: ISO 32000-1:2008 §12.3.2 (destinations), §12.5.6.5 (links),
//! §12.6.4.2 (Go-To); https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/
//! PDF32000_2008.pdf. API/coordinate contract checked against krilla 0.8.2
//! `interactive/{action,destination}.rs`: XYZ takes y-down points; pdf-writer
//! 0.15.0 `structure.rs::Destination::xyz` writes the absent zoom as zero:
//! `[page-ref /XYZ x (page-height-y) 0]` (retain zoom). Names are exact,
//! case-sensitive, never URLs. Unreferenced bookmarks are registered too.

use std::collections::{BTreeMap, BTreeSet};

use joyword_layout::{LayoutSnapshot, PageScene, to_points};
use joyword_model::{
    Anchor, AnnotationKind, Bias, Block, Document, NodeId, Paragraph, Position, ScalarOffset,
    StoryId,
};
use krilla::destination::{NamedDestination, XyzDestination};
use krilla::geom::Point;

/// Hard export budgets; no partial navigation is returned on exhaustion.
pub const MAX_PDF_BOOKMARKS: usize = 512;
pub const MAX_PDF_LINKS: usize = 512;
pub const MAX_PDF_NAME_BYTES: usize = 2048;
/// PDF 1.7 default user-space page limit (200 inches at 72 points/inch).
pub const MAX_PDF_PAGE_POINTS: f32 = 14_400.0;
const MAX_INPUT_ITEMS: usize = 16_384;
const MAX_INPUT_BYTES: usize = 8 << 20;
const MAX_GEOMETRY_ITEMS: usize = 262_144;

/// A physical destination. All coordinates are points from the page's top left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PdfDestination {
    pub page: usize,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PdfBookmark {
    pub name: String,
    pub destination: PdfDestination,
}

/// One exact laid-out rectangle, with an explicit (not URL/named) GoTo target.
#[derive(Clone, Debug, PartialEq)]
pub struct PdfGoToLink {
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub destination: PdfDestination,
}

/// Additive companion to `PdfExport`: existing URI callers need no changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PdfNavigation {
    pub bookmarks: Vec<PdfBookmark>,
    pub links: Vec<PdfGoToLink>,
    /// Anchor-only links whose target is not a bookmark of this document. They
    /// have no destination to point at, so no annotation is written for them.
    /// A DOCX keeps its bookmarks and its anchor links in separate parts, and
    /// only the links are imported, so a real document can carry these.
    pub unresolved_links: usize,
}

/// Deterministic refusal, without echoing untrusted names or URI contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationError {
    Budget,
    EmptyName,
    InvalidName,
    DuplicateName,
    DetachedBookmark,
    InvalidPosition,
    StaleLayout,
    InvalidGeometry,
    InvalidUri,
}

fn spend(remaining: &mut usize, amount: usize) -> Result<(), NavigationError> {
    *remaining = remaining
        .checked_sub(amount)
        .ok_or(NavigationError::Budget)?;
    Ok(())
}

fn check_name(name: &str) -> Result<(), NavigationError> {
    if name.len() > MAX_PDF_NAME_BYTES {
        return Err(NavigationError::Budget);
    }
    if name.trim().is_empty() {
        return Err(NavigationError::EmptyName);
    }
    if name.chars().any(char::is_control) {
        return Err(NavigationError::InvalidName);
    }
    Ok(())
}

pub(crate) fn page_size(page: &PageScene) -> Result<(f32, f32), NavigationError> {
    let width = to_points(page.width) as f32;
    let height = to_points(page.height) as f32;
    if ![width, height]
        .into_iter()
        .all(|n| n.is_finite() && n > 0.0 && n <= MAX_PDF_PAGE_POINTS)
    {
        return Err(NavigationError::InvalidGeometry);
    }
    Ok((width, height))
}

pub(crate) fn check_point(
    pages: &[PageScene],
    point: PdfDestination,
) -> Result<(), NavigationError> {
    let (width, height) = page_size(
        pages
            .get(point.page)
            .ok_or(NavigationError::InvalidGeometry)?,
    )?;
    if !point.x.is_finite()
        || !point.y.is_finite()
        || point.x < 0.0
        || point.y < 0.0
        || point.x > width
        || point.y > height
    {
        return Err(NavigationError::InvalidGeometry);
    }
    Ok(())
}

pub(crate) fn check_rect(
    pages: &[PageScene],
    page: usize,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Result<(), NavigationError> {
    check_point(pages, PdfDestination { page, x, y })?;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(NavigationError::InvalidGeometry);
    }
    check_point(
        pages,
        PdfDestination {
            page,
            x: x + width,
            y: y + height,
        },
    )
}

pub(crate) fn xyz(destination: PdfDestination) -> XyzDestination {
    XyzDestination::new(
        destination.page,
        Point::from_xy(destination.x, destination.y),
    )
}

pub(crate) fn validate(
    pages: &[PageScene],
    navigation: &PdfNavigation,
) -> Result<(), NavigationError> {
    if navigation.bookmarks.len() > MAX_PDF_BOOKMARKS || navigation.links.len() > MAX_PDF_LINKS {
        return Err(NavigationError::Budget);
    }
    let mut names = BTreeSet::new();
    for bookmark in &navigation.bookmarks {
        check_name(&bookmark.name)?;
        if !names.insert(&bookmark.name) {
            return Err(NavigationError::DuplicateName);
        }
        check_point(pages, bookmark.destination)?;
    }
    for link in &navigation.links {
        check_rect(pages, link.page, link.x, link.y, link.width, link.height)?;
        check_point(pages, link.destination)?;
    }
    Ok(())
}

pub(crate) fn register(
    document: &mut krilla::Document,
    navigation: &PdfNavigation,
) -> Result<(), NavigationError> {
    for bookmark in &navigation.bookmarks {
        document
            .register_named_destination(NamedDestination::new(
                bookmark.name.clone(),
                xyz(bookmark.destination),
            ))
            .ok_or(NavigationError::DuplicateName)?;
    }
    Ok(())
}

type Paragraphs<'a> = BTreeMap<(StoryId, NodeId), (usize, &'a Paragraph)>;

fn collect<'a>(
    blocks: impl Iterator<Item = &'a Block>,
    story: StoryId,
    paragraphs: &mut Paragraphs<'a>,
    remaining: &mut usize,
    bytes: &mut usize,
    depth: usize,
) -> Result<(), NavigationError> {
    if depth > 8 {
        return Err(NavigationError::Budget);
    }
    for block in blocks {
        spend(remaining, 1)?;
        match block {
            Block::Paragraph(paragraph) => {
                for run in paragraph.runs() {
                    spend(remaining, 1)?;
                    // Scalar count bounds UTF-8 storage without materializing text.
                    spend(
                        bytes,
                        run.len.checked_mul(4).ok_or(NavigationError::Budget)?,
                    )?;
                }
                let order = paragraphs.len();
                if paragraphs
                    .insert((story, paragraph.id()), (order, paragraph))
                    .is_some()
                {
                    return Err(NavigationError::InvalidPosition);
                }
            }
            Block::Table(table) => {
                for row in &table.rows {
                    spend(remaining, 1)?;
                    for cell in &row.cells {
                        spend(remaining, 1)?;
                        collect(
                            cell.blocks.iter(),
                            story,
                            paragraphs,
                            remaining,
                            bytes,
                            depth + 1,
                        )?;
                    }
                }
            }
            Block::Sdt(sdt) => collect(
                sdt.blocks.iter(),
                story,
                paragraphs,
                remaining,
                bytes,
                depth + 1,
            )?,
            Block::Opaque(_) => {}
        }
    }
    Ok(())
}

fn position_key(
    paragraphs: &Paragraphs<'_>,
    position: Position,
) -> Result<(usize, ScalarOffset), NavigationError> {
    let (index, paragraph) = paragraphs
        .get(&(position.story, position.paragraph))
        .ok_or(NavigationError::InvalidPosition)?;
    if position.offset.0 > paragraph.len() {
        return Err(NavigationError::InvalidPosition);
    }
    Ok((*index, position.offset))
}

/// Resolve all model bookmarks, then anchor-only run links, transactionally.
///
/// The caller must supply the snapshot of this model's geometric content.
/// A version mismatch is rejected; same-version manual edits cannot be detected
/// by this API. Bookmark metadata may be attached after layout: the current
/// layout preflight does not admit bookmarks, although its positions do.
/// Bookmark ranges must be ordered and both endpoints must have exact caret
/// geometry. The start
/// (including its wrap-boundary bias) is the target. No cluster rounding,
/// detached/hidden/unplaced-story fallback, or duplicate-name first-wins rule.
/// An external+anchor link stays an external URI through `links_from_document`.
/// Pass both products to `render_export_with_navigation` for mixed navigation.
pub fn navigation_from_document(
    document: &Document,
    snapshot: &LayoutSnapshot,
) -> Result<PdfNavigation, NavigationError> {
    if document.version() != snapshot.key().version {
        return Err(NavigationError::StaleLayout);
    }
    let mut remaining = MAX_INPUT_ITEMS;
    let mut bytes = MAX_INPUT_BYTES;
    let mut paragraphs = Paragraphs::new();
    for story in document.stories() {
        spend(&mut remaining, 1)?;
        collect(
            story.blocks.iter(),
            story.id,
            &mut paragraphs,
            &mut remaining,
            &mut bytes,
            0,
        )?;
    }
    let mut geometry = MAX_GEOMETRY_ITEMS;
    for line in snapshot.lines() {
        spend(&mut geometry, 1)?;
        spend(&mut geometry, line.carets.len())?;
    }
    if snapshot.pages().len() > MAX_INPUT_ITEMS {
        return Err(NavigationError::Budget);
    }
    for page in snapshot.pages() {
        page_size(page)?;
    }
    let mut anchors = BTreeMap::new();
    for annotation in document.annotations() {
        spend(&mut remaining, 1)?;
        if let AnnotationKind::Bookmark { name } = &annotation.kind {
            check_name(name)?;
            if anchors.contains_key(name.as_ref()) {
                return Err(NavigationError::DuplicateName);
            }
            if anchors.len() >= MAX_PDF_BOOKMARKS {
                return Err(NavigationError::Budget);
            }
            anchors.insert(name.as_ref(), &annotation.anchor);
        }
    }
    let mut navigation = PdfNavigation::default();
    let mut destinations = BTreeMap::new();
    for (name, anchor) in anchors {
        let Anchor::Range { start, end } = anchor else {
            return Err(NavigationError::DetachedBookmark);
        };
        if start.story != end.story
            || position_key(&paragraphs, *start)? > position_key(&paragraphs, *end)?
        {
            return Err(NavigationError::InvalidPosition);
        }
        let caret = snapshot
            .caret(*start)
            .map_err(|_| NavigationError::InvalidPosition)?;
        let end_caret = snapshot
            .caret(*end)
            .map_err(|_| NavigationError::InvalidPosition)?;
        check_point(
            snapshot.pages(),
            PdfDestination {
                page: end_caret.page,
                x: to_points(end_caret.x) as f32,
                y: to_points(end_caret.top) as f32,
            },
        )?;
        let destination = PdfDestination {
            page: caret.page,
            x: to_points(caret.x) as f32,
            y: to_points(caret.top) as f32,
        };
        check_point(snapshot.pages(), destination)?;
        navigation.bookmarks.push(PdfBookmark {
            name: name.to_owned(),
            destination,
        });
        destinations.insert(name, destination);
    }
    let mut spans = 0;
    for ((story, id), (_, paragraph)) in paragraphs {
        let mut at = 0usize;
        for run in paragraph.runs() {
            let end = at.checked_add(run.len).ok_or(NavigationError::Budget)?;
            if let Some(link) = &run.link
                && link.external.is_none()
            {
                let name = link.anchor.as_deref().ok_or(NavigationError::EmptyName)?;
                check_name(name)?;
                let Some(destination) = destinations.get(name).copied() else {
                    // The target is not a bookmark of this document. The link
                    // cannot point anywhere, so it is left out and counted
                    // rather than failing the whole export.
                    navigation.unresolved_links += 1;
                    at = end;
                    continue;
                };
                spans += 1;
                if spans > MAX_PDF_LINKS {
                    return Err(NavigationError::Budget);
                }
                let start = Position {
                    story,
                    paragraph: id,
                    offset: ScalarOffset(at),
                    bias: Bias::After,
                };
                let end = Position {
                    offset: ScalarOffset(end),
                    bias: Bias::Before,
                    ..start
                };
                let rects = snapshot
                    .selection_rects(start, end)
                    .map_err(|_| NavigationError::InvalidPosition)?;
                if rects.is_empty() {
                    return Err(NavigationError::InvalidPosition);
                }
                if rects.len() > MAX_PDF_LINKS - navigation.links.len() {
                    return Err(NavigationError::Budget);
                }
                for rect in rects {
                    navigation.links.push(PdfGoToLink {
                        page: rect.page,
                        x: to_points(rect.x) as f32,
                        y: to_points(rect.top) as f32,
                        width: to_points(rect.width) as f32,
                        height: to_points(rect.height) as f32,
                        destination,
                    });
                }
            }
            at = end;
        }
    }
    validate(snapshot.pages(), &navigation)?;
    Ok(navigation)
}
