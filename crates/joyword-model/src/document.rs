//! Document, stories, sections and snapshots (ARCHITECTURE §2–§3).
//!
//! Every field of [`Document`] is structurally shared, so [`Document::snapshot`]
//! is O(1) and an edit copies only what it touches. The model does not enforce
//! who may edit: the engine (T101) owns the working document and exposes only
//! transactions; background work receives immutable [`DocumentSnapshot`]s.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use crate::annotation::{Anchor, Annotation, AnnotationKind};
use crate::block::{Block, BlockSeq, Paragraph};
use crate::id::{AnnotationId, IdAllocator, NodeId, PreservedId, StoryId};
use crate::numbering::{NumberFormat, NumberingStore};
use crate::props::{Measure, Toggle};
use crate::resource::ResourceTable;
use crate::revision::MoveMarker;
use crate::style::{Style, StyleKind, StyleSheet};
use crate::validate::MAX_NESTING;

/// Compatibility mode JoyWord writes into new documents (FORMAT-001).
pub const NEW_DOCUMENT_COMPATIBILITY_MODE: u32 = 15;
/// Mode assumed when settings omit `compatibilityMode` (ARCHITECTURE §5).
pub const DEFAULT_COMPATIBILITY_MODE: u32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StoryKind {
    Body,
    Header,
    Footer,
    Footnote,
    Endnote,
    Comment,
    TextBox,
}

impl fmt::Display for StoryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            StoryKind::Body => "body",
            StoryKind::Header => "header",
            StoryKind::Footer => "footer",
            StoryKind::Footnote => "footnote",
            StoryKind::Endnote => "endnote",
            StoryKind::Comment => "comment",
            StoryKind::TextBox => "textbox",
        })
    }
}

#[derive(Clone, Debug)]
pub struct Story {
    pub id: StoryId,
    pub kind: StoryKind,
    pub blocks: BlockSeq,
}

/// `w:headerReference` / `w:footerReference` types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeaderFooterKind {
    Default,
    First,
    Even,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// How a section begins relative to the previous one (`w:type`, §17.6.22).
/// Omitted markup is [`SectionBreak::NextPage`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum SectionBreak {
    #[default]
    NextPage,
    Continuous,
    EvenPage,
    OddPage,
}

/// `w:pgNumType` format and start. An omitted format is decimal. An omitted
/// start continues from the previous section (the first section starts at 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageNumbering {
    pub format: NumberFormat,
    pub start: Option<i32>,
}

impl Default for PageNumbering {
    fn default() -> Self {
        Self {
            format: NumberFormat::Decimal,
            start: None,
        }
    }
}

/// Equal-width newspaper columns (`w:cols` with `equalWidth` true and no
/// `w:col` children). `count` is at least 2. `space` is the gap in twips.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EqualColumns {
    pub count: u16,
    pub space: i64,
}

/// One explicit newspaper column (`w:col`, ECMA-376 §17.6.3). `width` and
/// `space` are twips. `space` is the gap after this column; layout does not
/// place a gap after the last column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NewspaperColumn {
    pub width: i64,
    pub space: i64,
}

/// Unequal newspaper columns (`w:cols` with `equalWidth` false and one
/// `w:col/@w:w` per column). Length is 2..=45. A separator line is not stored.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UnequalColumns {
    pub columns: Vec<NewspaperColumn>,
}

/// `w:docGrid/@w:type` (ECMA-376 §17.6.5). An omitted type is the schema
/// value `default` and is stored as `None`, not as a line grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DocGridKind {
    Lines,
    LinesAndChars,
    SnapToChars,
}

/// `w:docGrid`. Lengths are twips as written. Layout reads a positive
/// `line_pitch` only for [`DocGridKind::Lines`] and
/// [`DocGridKind::LinesAndChars`]. `char_space` is kept and not applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DocGrid {
    pub kind: Option<DocGridKind>,
    pub line_pitch: Option<i64>,
    pub char_space: Option<i64>,
}

/// `w:pgSz` and `w:pgMar` in source units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageSetup {
    pub width: Measure,
    pub height: Measure,
    pub orientation: Orientation,
    pub top: Measure,
    pub right: Measure,
    pub bottom: Measure,
    pub left: Measure,
    pub header: Measure,
    pub footer: Measure,
    pub gutter: Measure,
}

impl PageSetup {
    /// A4 portrait with JoyWord's zh-CN new-document margins (2.54 cm top and
    /// bottom, 3.17 cm left and right, header 1.5 cm, footer 1.75 cm). These
    /// are product defaults (PLATFORM-008); matching a specific Word template
    /// requires a T004 reference record.
    pub fn a4_zh_cn() -> Self {
        Self {
            width: Measure::Twips(11_906),
            height: Measure::Twips(16_838),
            orientation: Orientation::Portrait,
            top: Measure::Twips(1_440),
            right: Measure::Twips(1_800),
            bottom: Measure::Twips(1_440),
            left: Measure::Twips(1_800),
            header: Measure::Twips(851),
            footer: Measure::Twips(992),
            gutter: Measure::Twips(0),
        }
    }
}

/// A section. OOXML stores `w:sectPr` on the section's last paragraph and the
/// final one at the end of the body; the model records the boundary block.
#[derive(Clone, Debug)]
pub struct Section {
    pub id: NodeId,
    /// Last top-level body block of this section; `None` for the final section.
    pub last_block: Option<NodeId>,
    pub page: PageSetup,
    pub title_page: Toggle,
    pub headers: Vec<(HeaderFooterKind, StoryId)>,
    pub footers: Vec<(HeaderFooterKind, StoryId)>,
    pub break_kind: SectionBreak,
    pub numbering: PageNumbering,
    /// `None` is one column. Equal columns wider than the page are rejected
    /// at layout rather than stored as a single column.
    pub columns: Option<EqualColumns>,
    /// Explicit unequal widths. Import leaves `columns` empty when this is
    /// set. Layout rejects a section that has both.
    pub unequal_columns: Option<UnequalColumns>,
    /// `None` means the section has no `w:docGrid` element.
    pub doc_grid: Option<DocGrid>,
    pub preserved: Option<PreservedId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocumentSettings {
    /// `compatibilityMode` as written; existing documents keep their value.
    pub compatibility_mode: Option<u32>,
    pub even_and_odd_headers: Toggle,
    /// `w:trackRevisions` (ECMA-376 Part 1 §17.15.1.89). `Some(true)` makes
    /// user typing and deleting create text revisions. It does not by itself
    /// make the document read-only. Document protection still does.
    pub track_revisions: Toggle,
    /// `w:defaultTabStop`. Absent means 720 twips when layout places tabs.
    pub default_tab_stop: Option<Measure>,
    /// `w:attachedTemplate/@r:id`. The id is recorded and the target is never
    /// resolved (QUALITY-005). `Some("")` means the element had no id.
    pub attached_template: Option<Arc<str>>,
    pub preserved: Option<PreservedId>,
}

impl DocumentSettings {
    pub fn effective_compatibility_mode(&self) -> u32 {
        self.compatibility_mode
            .unwrap_or(DEFAULT_COMPATIBILITY_MODE)
    }
}

/// Monotonic document version; undo produces a new version, never an old one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentVersion(u64);

impl DocumentVersion {
    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for DocumentVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.0)
    }
}

#[derive(Clone, Debug)]
pub struct Document {
    version: DocumentVersion,
    ids: IdAllocator,
    body: StoryId,
    stories: Arc<BTreeMap<StoryId, Arc<Story>>>,
    sections: Arc<Vec<Section>>,
    styles: Arc<StyleSheet>,
    resources: Arc<ResourceTable>,
    annotations: Arc<BTreeMap<AnnotationId, Annotation>>,
    settings: Arc<DocumentSettings>,
    numbering: Arc<NumberingStore>,
    /// Move range markers. Text revisions live on runs.
    move_markers: Arc<Vec<MoveMarker>>,
    /// Next `w:id` to assign. Import sets this past every id it kept.
    next_revision: u32,
}

impl Document {
    /// One body story holding one empty paragraph, and one section.
    pub fn new(page: PageSetup) -> Self {
        let mut ids = IdAllocator::default();
        let body = ids.story();
        let paragraph = Paragraph::new(ids.node());
        let section = Section {
            id: ids.node(),
            last_block: None,
            page,
            title_page: None,
            headers: Vec::new(),
            footers: Vec::new(),
            break_kind: SectionBreak::NextPage,
            numbering: PageNumbering::default(),
            columns: None,
            unequal_columns: None,
            doc_grid: None,
            preserved: None,
        };
        let story = Story {
            id: body,
            kind: StoryKind::Body,
            blocks: BlockSeq::from_vec(vec![paragraph.into()]),
        };
        Self {
            version: DocumentVersion::default(),
            ids,
            body,
            stories: Arc::new(BTreeMap::from([(body, Arc::new(story))])),
            sections: Arc::new(vec![section]),
            styles: Arc::default(),
            resources: Arc::default(),
            annotations: Arc::default(),
            settings: Arc::default(),
            numbering: Arc::default(),
            move_markers: Arc::default(),
            next_revision: 1,
        }
    }

    /// New JoyWord document: A4, a default Normal style, compatibility mode 15.
    pub fn new_blank() -> Self {
        let mut document = Self::new(PageSetup::a4_zh_cn());
        let mut normal = Style::new("Normal", StyleKind::Paragraph, Some("Normal"));
        normal.is_default = true;
        document
            .styles_mut()
            .insert(normal)
            .expect("empty style sheet");
        document.settings_mut().compatibility_mode = Some(NEW_DOCUMENT_COMPATIBILITY_MODE);
        document
    }

    pub fn version(&self) -> DocumentVersion {
        self.version
    }

    /// Publishes a new version number. Called once per committed transaction.
    pub fn advance_version(&mut self) -> DocumentVersion {
        self.version = DocumentVersion(self.version.0.checked_add(1).expect("version overflow"));
        self.version
    }

    /// O(1) immutable view sharing all structure with this document.
    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot(self.clone())
    }

    /// Replaces every piece of content with `source`'s while keeping this
    /// document's version counter and identifier allocator, so undo and redo
    /// never move either backwards. The caller advances the version.
    pub fn restore_content(&mut self, source: &Document) {
        let version = self.version;
        let mut ids = self.ids.clone();
        ids.merge(&source.ids);
        *self = source.clone();
        self.version = version;
        self.ids = ids;
    }

    pub fn ids(&mut self) -> &mut IdAllocator {
        &mut self.ids
    }

    pub fn id_allocator(&self) -> &IdAllocator {
        &self.ids
    }

    pub fn body_id(&self) -> StoryId {
        self.body
    }

    pub fn body(&self) -> &Story {
        self.story(self.body).expect("body story exists")
    }

    pub fn story(&self, id: StoryId) -> Option<&Story> {
        self.stories.get(&id).map(Arc::as_ref)
    }

    pub fn stories(&self) -> impl Iterator<Item = &Story> {
        self.stories.values().map(Arc::as_ref)
    }

    pub fn story_mut(&mut self, id: StoryId) -> Option<&mut Story> {
        Arc::make_mut(&mut self.stories)
            .get_mut(&id)
            .map(Arc::make_mut)
    }

    /// Adds a story. Stories must hold at least one block (checked by validate).
    pub fn insert_story(&mut self, kind: StoryKind, blocks: Vec<Block>) -> StoryId {
        let id = self.ids.story();
        let story = Story {
            id,
            kind,
            blocks: BlockSeq::from_vec(blocks),
        };
        Arc::make_mut(&mut self.stories).insert(id, Arc::new(story));
        id
    }

    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    pub fn sections_mut(&mut self) -> &mut Vec<Section> {
        Arc::make_mut(&mut self.sections)
    }

    pub fn styles(&self) -> &StyleSheet {
        &self.styles
    }

    pub fn styles_mut(&mut self) -> &mut StyleSheet {
        Arc::make_mut(&mut self.styles)
    }

    pub fn resources(&self) -> &ResourceTable {
        &self.resources
    }

    pub fn resources_mut(&mut self) -> &mut ResourceTable {
        Arc::make_mut(&mut self.resources)
    }

    pub fn settings(&self) -> &DocumentSettings {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut DocumentSettings {
        Arc::make_mut(&mut self.settings)
    }

    pub fn move_markers(&self) -> &[MoveMarker] {
        &self.move_markers
    }

    pub fn move_markers_mut(&mut self) -> &mut Vec<MoveMarker> {
        Arc::make_mut(&mut self.move_markers)
    }

    /// Remembers an imported `w:id` so the next allocated id does not reuse it.
    pub fn note_revision_id(&mut self, id: u32) {
        let next = id.saturating_add(1);
        if next > self.next_revision {
            self.next_revision = next;
        }
    }

    /// Allocates a new revision id. `None` when the counter is exhausted.
    pub fn allocate_revision_id(&mut self) -> Option<u32> {
        let id = self.next_revision;
        self.next_revision = self.next_revision.checked_add(1)?;
        Some(id)
    }

    pub fn numbering(&self) -> &NumberingStore {
        &self.numbering
    }

    pub fn numbering_mut(&mut self) -> &mut NumberingStore {
        Arc::make_mut(&mut self.numbering)
    }

    pub fn annotations(&self) -> impl Iterator<Item = &Annotation> {
        self.annotations.values()
    }

    pub fn annotation(&self, id: AnnotationId) -> Option<&Annotation> {
        self.annotations.get(&id)
    }

    pub fn annotation_mut(&mut self, id: AnnotationId) -> Option<&mut Annotation> {
        Arc::make_mut(&mut self.annotations).get_mut(&id)
    }

    pub fn insert_annotation(&mut self, kind: AnnotationKind, anchor: Anchor) -> AnnotationId {
        let id = self.ids.annotation();
        let annotation = Annotation {
            id,
            kind,
            anchor,
            source: None,
        };
        Arc::make_mut(&mut self.annotations).insert(id, annotation);
        id
    }

    /// Finds a paragraph anywhere in a story, including tables and content
    /// controls. Linear in story size; an index arrives with incremental
    /// layout (LAYOUT-010).
    pub fn find_paragraph(&self, story: StoryId, id: NodeId) -> Option<&Paragraph> {
        fn search(blocks: &BlockSeq, id: NodeId, depth: usize) -> Option<&Paragraph> {
            if depth > MAX_NESTING {
                return None;
            }
            blocks.iter().find_map(|block| match block {
                Block::Paragraph(paragraph) => (paragraph.id() == id).then_some(paragraph.as_ref()),
                Block::Table(table) => table
                    .rows
                    .iter()
                    .flat_map(|row| row.cells.iter())
                    .find_map(|cell| search(&cell.blocks, id, depth + 1)),
                Block::Sdt(sdt) => search(&sdt.blocks, id, depth + 1),
                Block::Opaque(_) => None,
            })
        }
        search(&self.story(story)?.blocks, id, 0)
    }
}

/// Immutable document view for background layout, export and Agent reads.
#[derive(Clone, Debug)]
pub struct DocumentSnapshot(Document);

impl Deref for DocumentSnapshot {
    type Target = Document;

    fn deref(&self) -> &Document {
        &self.0
    }
}
