//! Platform-neutral body text input (T102). UTF-16 is an API boundary;
//! persisted positions are model paragraph/scalar positions. Composition is
//! an immutable preview session and never modifies the committed undo stack.
use crate::docx::DocxSession;
use joyword_engine::{
    CellAddress, ChangeSet, Command, CommandError, FindError, FindHit, FindQuery, FormatBrush,
    Outcome, PaintError, ParagraphChange, RunChange, Selection, Session, Transaction, TxError,
    UndoError, UndoGroup, UndoResult, capture, cell_address, cell_caret, cell_selection,
    paint_commands, paragraph_order, replace_commands, search,
};
use joyword_model::{
    Bias, Block, BlockSeq, BreakKind, Color, Document, DocumentSnapshot, DocumentVersion, FontRef,
    InlineKind, MAX_NESTING, Measure, NodeId, NumberingRef, OBJECT_MARKER, Orientation, PageSetup,
    Paragraph, Position, RunProps, ScalarOffset, StoryKind, StyleId, StyleKind, Utf16Offset,
};
use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

pub trait InputBackend {
    fn session(&self) -> &Session;
    fn apply_mapped(
        &mut self,
        transaction: Transaction,
        target: Selection,
    ) -> Result<Outcome, TxError>;
    fn undo(&mut self) -> Result<UndoResult, UndoError>;
    fn redo(&mut self) -> Result<UndoResult, UndoError>;
    /// Close the open typing/deleting undo group (T209).
    fn seal_undo_group(&mut self);
    fn preview_undo(&self) -> Result<(DocumentSnapshot, Option<Selection>), UndoError> {
        self.session().preview_undo()
    }
    fn preview_redo(&self) -> Result<(DocumentSnapshot, Option<Selection>), UndoError> {
        self.session().preview_redo()
    }
}
impl InputBackend for Session {
    fn session(&self) -> &Session {
        self
    }
    fn apply_mapped(&mut self, t: Transaction, s: Selection) -> Result<Outcome, TxError> {
        self.apply_with_mapped_selection(t, s)
    }
    fn undo(&mut self) -> Result<UndoResult, UndoError> {
        Session::undo(self)
    }
    fn redo(&mut self) -> Result<UndoResult, UndoError> {
        Session::redo(self)
    }
    fn seal_undo_group(&mut self) {
        Session::seal_undo_group(self);
    }
}
impl InputBackend for crate::docx::BoundDocx {
    fn session(&self) -> &Session {
        crate::docx::BoundDocx::session(self).session()
    }
    fn apply_mapped(&mut self, t: Transaction, s: Selection) -> Result<Outcome, TxError> {
        self.session_mut().apply_with_mapped_selection(t, s)
    }
    fn undo(&mut self) -> Result<UndoResult, UndoError> {
        self.session_mut().undo()
    }
    fn redo(&mut self) -> Result<UndoResult, UndoError> {
        self.session_mut().redo()
    }
    fn seal_undo_group(&mut self) {
        self.session_mut().seal_undo_group();
    }
}
impl InputBackend for DocxSession {
    fn session(&self) -> &Session {
        DocxSession::session(self)
    }
    fn apply_mapped(&mut self, t: Transaction, s: Selection) -> Result<Outcome, TxError> {
        self.apply_with_mapped_selection(t, s)
    }
    fn undo(&mut self) -> Result<UndoResult, UndoError> {
        DocxSession::undo(self)
    }
    fn redo(&mut self) -> Result<UndoResult, UndoError> {
        DocxSession::redo(self)
    }
    fn seal_undo_group(&mut self) {
        DocxSession::seal_undo_group(self);
    }
}
/// Where a text replacement came from (SPEC §6 undo groups, T209). Only
/// plain keyboard typing and keyboard deletion coalesce into one undo step;
/// paste, drag, IME commits, Enter, autocorrect and menu commands each stay
/// a standalone step that also ends any open typing group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputSource {
    /// Keystrokes delivered by the platform text-input client.
    #[default]
    Keyboard,
    /// Clipboard paste or drag-and-drop insertion.
    Paste,
    /// Menu, toolbar, recovery or automation insertion.
    Command,
}
#[derive(Clone, Copy, Debug)]
pub struct InputLimits {
    pub max_paragraphs: usize,
    pub max_paragraph_bytes: usize,
    pub max_metadata_items: usize,
    pub max_text_bytes: usize,
    pub max_runs: usize,
    pub max_insert_bytes: usize,
    pub max_composition_bytes: usize,
    pub max_commands: usize,
}
impl Default for InputLimits {
    fn default() -> Self {
        Self {
            max_paragraphs: 10_000,
            max_paragraph_bytes: 65_536,
            max_metadata_items: 10_000,
            max_text_bytes: 16 << 20,
            max_runs: 100_000,
            max_insert_bytes: 65_536,
            max_composition_bytes: 8192,
            max_commands: 2048,
        }
    }
}
impl InputLimits {
    fn capped(self) -> Self {
        let d = Self::default();
        Self {
            max_paragraphs: self.max_paragraphs.min(d.max_paragraphs),
            max_paragraph_bytes: self.max_paragraph_bytes.min(d.max_paragraph_bytes),
            max_metadata_items: self.max_metadata_items.min(d.max_metadata_items),
            max_text_bytes: self.max_text_bytes.min(d.max_text_bytes),
            max_runs: self.max_runs.min(d.max_runs),
            max_insert_bytes: self.max_insert_bytes.min(d.max_insert_bytes),
            max_composition_bytes: self.max_composition_bytes.min(d.max_composition_bytes),
            max_commands: self.max_commands.min(d.max_commands),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum InputError {
    UnsupportedBody,
    InvalidDocument,
    InvalidRange,
    InsideSurrogate,
    UnknownPosition(Position),
    InvalidText(char),
    /// A find or format-painter request the editor will not apply.
    Rejected(&'static str),
    Budget(&'static str),
    ReadOnly,
    StaleComposition,
    GenerationOverflow,
    /// A still image was refused. The text does not include paths or bytes.
    Image(&'static str),
    Transaction(TxError),
    Undo(UndoError),
}
impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "text input: {self:?}")
    }
}
impl std::error::Error for InputError {}
impl From<TxError> for InputError {
    fn from(e: TxError) -> Self {
        Self::Transaction(e)
    }
}
impl From<UndoError> for InputError {
    fn from(e: UndoError) -> Self {
        Self::Undo(e)
    }
}

/// One clipboard entry the paste policy can see. Paths are never opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardOffer<'a> {
    Image { bytes: &'a [u8] },
    Text(&'a str),
    Paths,
}

/// What a paste should do. A refused image does not fall back to text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteChoice<'a> {
    Image(&'a [u8]),
    Text(&'a str),
    Empty,
    Refused(&'static str),
}

/// Prefer the first admitted image. If every image entry is refused, refuse
/// the paste. File paths are never opened. Empty content is a no-op.
pub fn choose_paste<'a>(entries: &[ClipboardOffer<'a>]) -> PasteChoice<'a> {
    let mut text = None;
    let mut saw_image = false;
    let mut saw_paths = false;
    for entry in entries {
        match *entry {
            ClipboardOffer::Image { bytes } => {
                saw_image = true;
                if joyword_model::accept_image(bytes, &joyword_model::ImageLimits::default())
                    .is_ok()
                {
                    return PasteChoice::Image(bytes);
                }
            }
            ClipboardOffer::Text(value) => {
                if text.is_none() && !value.is_empty() {
                    text = Some(value);
                }
            }
            ClipboardOffer::Paths => saw_paths = true,
        }
    }
    if saw_image {
        return PasteChoice::Refused("clipboard image was refused");
    }
    if let Some(value) = text {
        return PasteChoice::Text(value);
    }
    if saw_paths {
        return PasteChoice::Refused("clipboard file paths are not opened");
    }
    PasteChoice::Empty
}

fn read_image_file(path: &Path) -> Result<Vec<u8>, InputError> {
    let text = path.to_string_lossy();
    if text.contains("://")
        || text.contains(':')
        || text.starts_with("\\\\")
        || text.starts_with("//")
    {
        return Err(InputError::Image("remote image"));
    }
    let meta =
        std::fs::metadata(path).map_err(|_| InputError::Image("image file could not be read"))?;
    if !meta.is_file() {
        return Err(InputError::Image("image file could not be read"));
    }
    if meta.len() > joyword_model::MAX_IMAGE_BYTES {
        return Err(InputError::Image("image exceeds the byte budget"));
    }
    let mut file =
        std::fs::File::open(path).map_err(|_| InputError::Image("image file could not be read"))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(joyword_model::MAX_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| InputError::Image("image file could not be read"))?;
    if bytes.len() as u64 > joyword_model::MAX_IMAGE_BYTES {
        return Err(InputError::Image("image exceeds the byte budget"));
    }
    Ok(bytes)
}

#[derive(Clone, Debug)]
struct ParagraphEntry {
    id: NodeId,
    byte: usize,
    utf16: usize,
    len_utf16: usize,
    len_scalar: usize,
}
/// Flat platform projection joins body paragraphs, including paragraphs inside
/// table cells, with one LF. Opaque blocks, content controls and nested tables
/// stay unsupported.
#[derive(Clone, Debug)]
pub struct TextIndex {
    snapshot: DocumentSnapshot,
    text: Arc<str>,
    paragraphs: Vec<ParagraphEntry>,
    by_id: BTreeMap<NodeId, usize>,
    len_utf16: usize,
}
impl TextIndex {
    fn new(snapshot: DocumentSnapshot, limits: InputLimits) -> Result<Self, InputError> {
        if snapshot.body().blocks.len() > limits.max_paragraphs {
            return Err(InputError::Budget("paragraphs"));
        }
        // Header and footer stories are not part of the flat body projection.
        // Other stories (notes, comments, text boxes) stay rejected so their
        // text is not dropped without a diagnostic.
        if snapshot.stories().any(|story| {
            !matches!(
                story.kind,
                StoryKind::Body | StoryKind::Header | StoryKind::Footer
            )
        }) {
            return Err(InputError::UnsupportedBody);
        }
        if snapshot
            .styles()
            .len()
            .saturating_add(snapshot.resources().len())
            .saturating_add(snapshot.annotations().count())
            > limits.max_metadata_items
        {
            return Err(InputError::Budget("metadata"));
        }
        let mut text = String::new();
        let mut paragraphs = Vec::new();
        let mut by_id = BTreeMap::new();
        let mut utf16 = 0;
        let mut runs = 0usize;
        let mut push_paragraph = |p: &Paragraph| -> Result<(), InputError> {
            if p.inlines().iter().any(|inline| {
                !matches!(
                    inline.kind,
                    InlineKind::Break(BreakKind::Page) | InlineKind::Image { .. }
                )
            }) {
                return Err(InputError::UnsupportedBody);
            }
            if paragraphs.len() >= limits.max_paragraphs {
                return Err(InputError::Budget("paragraphs"));
            }
            let bytes = p.text().summary().bytes;
            if bytes > limits.max_paragraph_bytes {
                return Err(InputError::Budget("paragraph bytes"));
            }
            let separator = usize::from(!paragraphs.is_empty());
            if text.len().saturating_add(bytes).saturating_add(separator) > limits.max_text_bytes {
                return Err(InputError::Budget("text bytes"));
            }
            runs = runs.saturating_add(p.runs().len());
            if runs > limits.max_runs {
                return Err(InputError::Budget("runs"));
            }
            if separator != 0 {
                text.push('\n');
                utf16 += 1;
            }
            let content = p.plain_text(|object| {
                debug_assert!(matches!(
                    object.kind,
                    InlineKind::Break(BreakKind::Page) | InlineKind::Image { .. }
                ));
                OBJECT_MARKER.to_string()
            });
            let len = content.encode_utf16().count();
            let entry = ParagraphEntry {
                id: p.id(),
                byte: text.len(),
                utf16,
                len_utf16: len,
                len_scalar: p.len(),
            };
            if by_id.insert(p.id(), paragraphs.len()).is_some() {
                return Err(InputError::InvalidDocument);
            }
            paragraphs.push(entry);
            text.push_str(&content);
            utf16 += len;
            Ok(())
        };
        for block in snapshot.body().blocks.iter() {
            match block {
                Block::Paragraph(paragraph) => push_paragraph(paragraph)?,
                Block::Table(table) => {
                    for cell in table.rows.iter().flat_map(|row| row.cells.iter()) {
                        for inner in cell.blocks.iter() {
                            let Some(paragraph) = inner.as_paragraph() else {
                                return Err(InputError::UnsupportedBody);
                            };
                            push_paragraph(paragraph)?;
                        }
                    }
                }
                Block::Sdt(_) | Block::Opaque(_) => return Err(InputError::UnsupportedBody),
            }
        }
        if paragraphs.is_empty() || snapshot.validate().has_errors() {
            return Err(InputError::InvalidDocument);
        }
        Ok(Self {
            snapshot,
            text: text.into(),
            paragraphs,
            by_id,
            len_utf16: utf16,
        })
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn len_utf16(&self) -> usize {
        self.len_utf16
    }
    pub fn snapshot(&self) -> &DocumentSnapshot {
        &self.snapshot
    }
    pub fn position(&self, offset: usize, bias: Bias) -> Result<Position, InputError> {
        if offset > self.len_utf16 {
            return Err(InputError::InvalidRange);
        }
        let i = self
            .paragraphs
            .partition_point(|p| p.utf16 <= offset)
            .saturating_sub(1);
        let p = &self.paragraphs[i];
        let relative = offset - p.utf16;
        if relative > p.len_utf16 {
            return Err(InputError::InvalidRange);
        }
        let paragraph = paragraph_by_id(&self.snapshot, p.id).ok_or(InputError::InvalidDocument)?;
        let scalar = paragraph
            .text()
            .utf16_to_scalar(Utf16Offset(relative))
            .map_err(|_| InputError::InsideSurrogate)?;
        Ok(Position {
            story: self.snapshot.body_id(),
            paragraph: p.id,
            offset: scalar,
            bias,
        })
    }
    pub fn offset(&self, position: Position) -> Result<usize, InputError> {
        if position.story != self.snapshot.body_id() {
            return Err(InputError::UnknownPosition(position));
        }
        let i = *self
            .by_id
            .get(&position.paragraph)
            .ok_or(InputError::UnknownPosition(position))?;
        let p = &self.paragraphs[i];
        if position.offset.0 > p.len_scalar {
            return Err(InputError::UnknownPosition(position));
        }
        let paragraph =
            paragraph_by_id(&self.snapshot, p.id).ok_or(InputError::UnknownPosition(position))?;
        let n = paragraph
            .text()
            .scalar_to_utf16(position.offset)
            .map_err(|_| InputError::UnknownPosition(position))?;
        Ok(p.utf16 + n.0)
    }
    fn byte(&self, offset: usize) -> Result<usize, InputError> {
        let position = self.position(offset, Bias::After)?;
        let i = self.by_id[&position.paragraph];
        let p = paragraph_by_id(&self.snapshot, position.paragraph)
            .ok_or(InputError::InvalidDocument)?;
        let byte = p
            .text()
            .scalar_to_utf8(position.offset)
            .map_err(|_| InputError::InvalidRange)?;
        Ok(self.paragraphs[i].byte + byte.0)
    }
    pub fn text_for_range(&self, range: Range<usize>) -> Result<String, InputError> {
        self.check(&range)?;
        Ok(self.text[self.byte(range.start)?..self.byte(range.end)?].to_owned())
    }
    fn check(&self, range: &Range<usize>) -> Result<(), InputError> {
        if range.start > range.end {
            return Err(InputError::InvalidRange);
        }
        self.position(range.start, Bias::After)?;
        self.position(range.end, Bias::Before)?;
        Ok(())
    }
    pub fn selection_range(
        &self,
        selection: Selection,
    ) -> Result<(Range<usize>, bool), InputError> {
        let a = self.offset(selection.anchor)?;
        let h = self.offset(selection.head)?;
        Ok((a.min(h)..a.max(h), h < a))
    }
    fn selection(&self, range: Range<usize>, reversed: bool) -> Result<Selection, InputError> {
        self.check(&range)?;
        let a = self.position(range.start, Bias::After)?;
        let b = self.position(range.end, Bias::After)?;
        Ok(if reversed {
            Selection { anchor: b, head: a }
        } else {
            Selection { anchor: a, head: b }
        })
    }
}
#[derive(Clone, Debug)]
pub struct InputView {
    index: Arc<TextIndex>,
    selection: Selection,
    marked: Option<Range<usize>>,
    revision: u64,
    /// Engine changes since the layout that was acknowledged for this version.
    changes: ChangeSet,
}
impl InputView {
    pub fn index(&self) -> &TextIndex {
        &self.index
    }
    pub fn snapshot(&self) -> &DocumentSnapshot {
        self.index.snapshot()
    }
    pub fn selection(&self) -> Selection {
        self.selection
    }
    pub fn marked_range(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Changes the next layout should treat as dirty. A missing entry still
    /// misses the paragraph cache when the content key changed.
    pub fn layout_changes(&self) -> &ChangeSet {
        &self.changes
    }
    pub fn selected_range(&self) -> (Range<usize>, bool) {
        self.index
            .selection_range(self.selection)
            .expect("validated selection")
    }
}
#[derive(Clone)]
struct Composition {
    base_version: DocumentVersion,
    base_range: Range<usize>,
    text: String,
    marked: Range<usize>,
}
struct Prepared {
    index: Arc<TextIndex>,
    selection: Selection,
    transaction: Transaction,
    target: Selection,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Backward,
    Forward,
}
pub struct TextInput<B = Session> {
    backend: B,
    base: Arc<TextIndex>,
    selection: Selection,
    composition: Option<Composition>,
    view: InputView,
    limits: InputLimits,
    saved_version: DocumentVersion,
    /// Union of committed changes not yet covered by an installed layout.
    pending: ChangeSet,
    brush: Option<FormatBrush>,
    brush_repeat: bool,
}

/// One row of the style panel: identity plus the resolved preview.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleRow {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub preview: String,
}

/// Result of a find or format-painter command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditReport {
    pub matches: usize,
    pub skipped: u32,
    pub truncated: bool,
    pub replaced: usize,
    pub armed: bool,
}

/// Find, replace and format-painter requests shared by the shell and the GUI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditRequest {
    Find {
        query: String,
        case_sensitive: bool,
        whole_word: bool,
        forward: bool,
    },
    Replace {
        query: String,
        replacement: String,
        case_sensitive: bool,
        whole_word: bool,
        all: bool,
    },
    CopyFormat {
        repeat: bool,
    },
    PaintFormat,
    CancelFormat,
}
impl<B: InputBackend> TextInput<B> {
    pub fn new(backend: B, limits: InputLimits) -> Result<Self, InputError> {
        let limits = limits.capped();
        let saved_version = backend.session().version();
        let base = Arc::new(TextIndex::new(backend.session().snapshot(), limits)?);
        let selection = Selection::caret(base.position(0, Bias::After)?);
        let view = InputView {
            index: base.clone(),
            selection,
            marked: None,
            revision: 0,
            changes: ChangeSet::default(),
        };
        Ok(Self {
            backend,
            base,
            selection,
            composition: None,
            view,
            limits,
            saved_version,
            pending: ChangeSet::default(),
            brush: None,
            brush_repeat: false,
        })
    }
    pub fn backend(&self) -> &B {
        &self.backend
    }
    pub fn view(&self) -> &InputView {
        &self.view
    }
    fn note(&mut self, changes: ChangeSet) {
        self.pending.absorb(changes);
    }
    fn publish(
        &mut self,
        index: Arc<TextIndex>,
        selection: Selection,
        marked: Option<Range<usize>>,
        revision: u64,
    ) {
        self.view = InputView {
            index,
            selection,
            marked,
            revision,
            changes: self.pending.clone(),
        };
    }
    /// Drop pending invalidation once a layout of this committed version is
    /// installed. A newer edit, or an open composition preview, keeps it.
    pub fn acknowledge_layout(&mut self, version: u64) {
        if self.is_composing() || self.backend.session().version().get() != version {
            return;
        }
        self.pending = ChangeSet::default();
        self.view.changes = ChangeSet::default();
    }
    pub fn is_composing(&self) -> bool {
        self.composition.is_some()
    }
    pub fn has_unsaved_changes(&self) -> bool {
        self.backend.session().version() != self.saved_version
    }
    pub fn saved_version(&self) -> DocumentVersion {
        self.saved_version
    }
    /// Mark a version that was actually written to the bound source. Later
    /// edits remain dirty; an older asynchronous completion cannot move the
    /// saved baseline backwards.
    pub fn mark_saved(&mut self, version: DocumentVersion) {
        if version.get() > self.saved_version.get() {
            self.saved_version = version;
        }
    }
    pub fn can_undo(&self) -> bool {
        self.is_composing() || self.backend.session().can_undo()
    }
    pub fn can_redo(&self) -> bool {
        !self.is_composing() && self.backend.session().can_redo()
    }
    pub fn read_only(&self) -> bool {
        self.backend.session().policy().read_only
    }
    fn writable(&self) -> Result<(), InputError> {
        if self.read_only() {
            return Err(InputError::ReadOnly);
        }
        if self.base.snapshot.version() != self.backend.session().version()
            || self
                .composition
                .as_ref()
                .is_some_and(|c| c.base_version != self.backend.session().version())
        {
            return Err(InputError::StaleComposition);
        }
        Ok(())
    }
    fn next_revision(&self) -> Result<u64, InputError> {
        self.view
            .revision
            .checked_add(1)
            .ok_or(InputError::GenerationOverflow)
    }
    fn normalize(&self, text: &str, composing: bool) -> Result<String, InputError> {
        let max = if composing {
            self.limits.max_composition_bytes
        } else {
            self.limits.max_insert_bytes
        };
        if text.len() > max {
            return Err(InputError::Budget("insert bytes"));
        }
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        if let Some(ch) = text.chars().find(|&c| {
            (c.is_control() && c != '\n') || matches!(c, '\u{2028}' | '\u{2029}' | '\u{fffc}')
        }) {
            return Err(InputError::InvalidText(ch));
        }
        Ok(text)
    }
    fn prepare(
        &self,
        range: Range<usize>,
        text: &str,
        group: UndoGroup,
    ) -> Result<Prepared, InputError> {
        self.writable()?;
        self.base.check(&range)?;
        let start = self.base.position(range.start, Bias::After)?;
        let end = self.base.position(range.end, Bias::Before)?;
        let mut commands = Vec::new();
        if range.start != range.end {
            commands.push(Command::DeleteRange { start, end });
        }
        // Reverse insertion keeps every command anchored in the known first
        // paragraph, even when engine allocates new IDs for paragraph splits.
        let lines: Vec<_> = text.split('\n').collect();
        let command_count = commands
            .len()
            .saturating_add(lines.len().saturating_sub(1))
            .saturating_add(lines.iter().filter(|line| !line.is_empty()).count());
        if command_count > self.limits.max_commands {
            return Err(InputError::Budget("commands"));
        }
        for (i, line) in lines.iter().enumerate().rev() {
            if !line.is_empty() {
                commands.push(Command::InsertText {
                    at: start,
                    text: (*line).into(),
                });
            }
            if i > 0 {
                commands.push(Command::SplitParagraph { at: start });
            }
        }
        let mut transaction = Transaction::new(self.backend.session().version(), commands);
        transaction.selection = Some(self.selection);
        transaction.undo_group = group;
        let target = Selection::caret(start);
        let mut candidate = Session::new(self.backend.session().document().clone());
        candidate.set_policy(self.backend.session().policy().clone());
        let outcome = candidate.apply_with_mapped_selection(transaction.clone(), target)?;
        let selection = match outcome {
            Outcome::Applied(a) => a.selection.expect("valid target survived"),
            Outcome::NoOp(_) => target,
        };
        let index = Arc::new(TextIndex::new(candidate.snapshot(), self.limits)?);
        Ok(Prepared {
            index,
            selection,
            transaction,
            target,
        })
    }
    // Fold an update in virtual UTF-16 coordinates into a single replacement
    // of the committed base. This also handles partial or surrounding edits.
    fn fold(
        &self,
        requested: Option<Range<usize>>,
        new_text: &str,
    ) -> Result<(Range<usize>, String, Range<usize>), InputError> {
        let range = requested.unwrap_or_else(|| {
            self.composition
                .as_ref()
                .map(|c| c.marked.clone())
                .unwrap_or_else(|| self.view.selected_range().0)
        });
        self.view.index.check(&range)?;
        let mark = range.start..range.start + new_text.encode_utf16().count();
        let Some(old) = &self.composition else {
            return Ok((range, new_text.into(), mark));
        };
        let overlay = old.base_range.start..old.base_range.start + old.text.encode_utf16().count();
        let union = range.start.min(overlay.start)..range.end.max(overlay.end);
        let base_start = union.start;
        let base_end = old.base_range.end + (union.end - overlay.end);
        let mut text = self.view.index.text_for_range(union.start..range.start)?;
        text.push_str(new_text);
        text.push_str(&self.view.index.text_for_range(range.end..union.end)?);
        Ok((base_start..base_end, text, mark))
    }
    /// Replace a range with keyboard text; see [`Self::replace_from`].
    pub fn replace(&mut self, range: Option<Range<usize>>, text: &str) -> Result<bool, InputError> {
        self.replace_from(InputSource::Keyboard, range, text)
    }
    /// Replace a range (default: the marked range, else the selection) with
    /// `text` from `source`. Keyboard text without a paragraph break that does
    /// not commit a composition and replaces the current selection joins the
    /// open typing group; everything else, including a platform replacement
    /// of some other range, is a standalone undo step.
    pub fn replace_from(
        &mut self,
        source: InputSource,
        range: Option<Range<usize>>,
        text: &str,
    ) -> Result<bool, InputError> {
        let typing = source == InputSource::Keyboard
            && self.composition.is_none()
            && !text.is_empty()
            && !text.contains(['\n', '\r'])
            && range
                .as_ref()
                .is_none_or(|range| *range == self.view.selected_range().0);
        let group = if typing {
            UndoGroup::Typing
        } else {
            UndoGroup::Standalone
        };
        self.replace_grouped(range, text, group)
    }
    fn replace_grouped(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        group: UndoGroup,
    ) -> Result<bool, InputError> {
        let text = self.normalize(text, false)?;
        let (base_range, text, _) = self.fold(range, &text)?;
        if text.len() > self.limits.max_insert_bytes {
            return Err(InputError::Budget("folded insert bytes"));
        }
        let prepared = self.prepare(base_range, &text, group)?;
        let revision = self.next_revision()?;
        let outcome = self
            .backend
            .apply_mapped(prepared.transaction, prepared.target)?;
        let changed = matches!(outcome, Outcome::Applied(_));
        if let Outcome::Applied(applied) = &outcome {
            self.note(applied.changes.clone());
        }
        let selection = match outcome {
            Outcome::Applied(a) => a.selection.expect("validated target"),
            Outcome::NoOp(_) => prepared.selection,
        };
        self.base = Arc::new(TextIndex {
            snapshot: self.backend.session().snapshot(),
            ..(*prepared.index).clone()
        });
        self.selection = selection;
        self.composition = None;
        self.publish(self.base.clone(), selection, None, revision);
        Ok(changed)
    }

    /// Selected body text. An empty selection yields an empty string.
    /// Object markers (images and breaks) are omitted so a plain-text copy
    /// does not put U+FDD0 on the clipboard.
    pub fn selected_text(&self) -> Result<String, InputError> {
        let (range, _) = self.view.selected_range();
        let text = self.view.index.text_for_range(range)?;
        Ok(text.replace(OBJECT_MARKER, ""))
    }

    /// Insert admitted image bytes at the selection. A selection is deleted
    /// in the same transaction. A refused image does not change the document.
    pub fn insert_image_bytes(&mut self, bytes: &[u8]) -> Result<bool, InputError> {
        self.require_editable()?;
        joyword_model::accept_image(bytes, &joyword_model::ImageLimits::default())
            .map_err(|error| InputError::Image(error.reason()))?;
        let (range, _) = self.view.selected_range();
        self.view.index.check(&range)?;
        let start = self.view.index.position(range.start, Bias::After)?;
        let mut commands = Vec::new();
        if !range.is_empty() {
            let end = self.view.index.position(range.end, Bias::Before)?;
            commands.push(Command::DeleteRange { start, end });
        }
        commands.push(Command::InsertImage {
            at: Position {
                bias: Bias::After,
                ..start
            },
            data: Arc::from(bytes),
        });
        if commands.len() > self.limits.max_commands {
            return Err(InputError::Budget("commands"));
        }
        let mut transaction = Transaction::new(self.backend.session().version(), commands);
        transaction.selection = Some(self.selection);
        transaction.undo_group = UndoGroup::Standalone;
        let target = Selection::caret(Position {
            bias: Bias::After,
            ..start
        });
        self.commit_with(transaction, target, |_document, _changes, selection| {
            Ok(selection)
        })
    }

    /// Read one regular file, under the image byte cap, and insert it.
    /// URLs, UNC paths and non-files are refused without being opened as
    /// remote content.
    pub fn insert_image_file(&mut self, path: &Path) -> Result<bool, InputError> {
        let bytes = read_image_file(path)?;
        self.insert_image_bytes(&bytes)
    }

    /// Apply character formatting to the current selection. An empty selection
    /// changes nothing.
    pub fn format_runs(&mut self, change: RunChange) -> Result<bool, InputError> {
        self.require_editable()?;
        let (range, _) = self.view.selected_range();
        if range.is_empty() {
            return Ok(false);
        }
        let start = self.view.index.position(range.start, Bias::After)?;
        let end = self.view.index.position(range.end, Bias::Before)?;
        let transaction = Transaction::new(
            self.backend.session().version(),
            vec![Command::FormatText { start, end, change }],
        );
        self.commit(transaction)
    }

    /// Styles on the sheet, one line per style: id, kind, canonical name.
    pub fn list_styles(&self) -> Vec<String> {
        self.backend
            .session()
            .document()
            .styles()
            .iter()
            .map(|style| {
                let name = style.name.as_deref().unwrap_or(style.id.as_str());
                format!("{} {} {name}", style.id, style.kind)
            })
            .collect()
    }

    /// Apply a paragraph or character style to the selection. A caret takes
    /// a paragraph style; a character style on a caret changes nothing.
    pub fn apply_style(&mut self, id: &str) -> Result<bool, InputError> {
        self.require_editable()?;
        let (range, _) = self.view.selected_range();
        let (start, end) = if range.is_empty() {
            (self.selection.head, self.selection.head)
        } else {
            let start = self.view.index.position(range.start, Bias::After)?;
            let end = self.view.index.position(range.end, Bias::Before)?;
            (start, end)
        };
        let transaction = Transaction::new(
            self.backend.session().version(),
            vec![Command::ApplyStyle {
                start,
                end,
                style: StyleId::new(id),
            }],
        );
        self.commit(transaction)
    }

    /// Puts every paragraph the selection touches into numbering instance
    /// `num_id` at level 0.
    pub fn apply_list(&mut self, num_id: u32) -> Result<bool, InputError> {
        self.require_editable()?;
        let (start, end) = self.selection_positions()?;
        let transaction = Transaction::new(
            self.backend.session().version(),
            vec![Command::SetNumbering {
                start,
                end,
                numbering: Some(NumberingRef { num_id, level: 0 }),
            }],
        );
        self.commit(transaction)
    }

    /// Changes `ilvl` and keeps each paragraph's own `numId`.
    pub fn set_list_level(&mut self, level: u8) -> Result<bool, InputError> {
        self.require_editable()?;
        let (start, end) = self.selection_positions()?;
        let transaction = Transaction::new(
            self.backend.session().version(),
            vec![Command::SetNumberingLevel { start, end, level }],
        );
        self.commit(transaction)
    }

    /// Writes `numId` 0, which overrides a style's list.
    pub fn remove_list(&mut self) -> Result<bool, InputError> {
        self.require_editable()?;
        let (start, end) = self.selection_positions()?;
        let transaction = Transaction::new(
            self.backend.session().version(),
            vec![Command::SetNumbering {
                start,
                end,
                numbering: None,
            }],
        );
        self.commit(transaction)
    }

    /// Styles with the resolved preview the panel shows.
    pub fn style_rows(&self) -> Vec<StyleRow> {
        let sheet = self.backend.session().document().styles();
        sheet
            .iter()
            .map(|style| {
                let preview = match style.kind {
                    StyleKind::Paragraph => describe_run(
                        &sheet
                            .resolve_paragraph(
                                Some(&style.id),
                                &joyword_model::ParaProps::default(),
                            )
                            .run,
                    ),
                    StyleKind::Character => describe_run(
                        &sheet
                            .resolve_run(None, Some(&style.id), &RunProps::default())
                            .props,
                    ),
                    other => other.to_string(),
                };
                StyleRow {
                    id: style.id.to_string(),
                    kind: style.kind.to_string(),
                    name: style
                        .name
                        .as_deref()
                        .unwrap_or(style.id.as_str())
                        .to_owned(),
                    preview,
                }
            })
            .collect()
    }

    /// Paragraph style and character style at the caret.
    pub fn caret_styles(&self) -> (Option<String>, Option<String>) {
        let head = self.selection.head;
        let Some(paragraph) = paragraph_by_id(self.backend.session().document(), head.paragraph)
        else {
            return (None, None);
        };
        let (character, _) = paragraph.format_at(head.offset);
        (
            paragraph.style.as_ref().map(ToString::to_string),
            character.map(|style| style.to_string()),
        )
    }

    pub fn format_armed(&self) -> bool {
        self.brush.is_some()
    }

    /// Find, replace and the format painter. Find does not modify the document.
    /// Replace and paint each commit one transaction.
    pub fn editing(&mut self, request: EditRequest) -> Result<EditReport, InputError> {
        if !matches!(request, EditRequest::CancelFormat) && self.composition.is_some() {
            return Err(InputError::StaleComposition);
        }
        match request {
            EditRequest::Find {
                query,
                case_sensitive,
                whole_word,
                forward,
            } => self.find_move(&query, case_sensitive, whole_word, forward),
            EditRequest::Replace {
                query,
                replacement,
                case_sensitive,
                whole_word,
                all,
            } => {
                self.require_editable()?;
                self.replace_query(&query, &replacement, case_sensitive, whole_word, all)
            }
            EditRequest::CopyFormat { repeat } => {
                let (start, end) = self.selection_positions()?;
                let brush = capture(self.backend.session().document(), start, end)
                    .map_err(paint_input_error)?;
                self.brush = Some(brush);
                self.brush_repeat = repeat;
                Ok(EditReport {
                    matches: 0,
                    skipped: 0,
                    truncated: false,
                    replaced: 0,
                    armed: true,
                })
            }
            EditRequest::PaintFormat => {
                self.require_editable()?;
                let Some(brush) = self.brush.clone() else {
                    return Err(InputError::Rejected("no format copied"));
                };
                let (start, end) = self.selection_positions()?;
                let commands =
                    paint_commands(self.backend.session().document(), start, end, &brush)
                        .map_err(paint_input_error)?;
                let mut transaction = Transaction::new(self.backend.session().version(), commands);
                transaction.selection = Some(self.selection);
                self.commit(transaction)?;
                if !self.brush_repeat {
                    self.brush = None;
                }
                Ok(EditReport {
                    matches: 0,
                    skipped: 0,
                    truncated: false,
                    replaced: 0,
                    armed: self.brush.is_some(),
                })
            }
            EditRequest::CancelFormat => {
                self.brush = None;
                self.brush_repeat = false;
                Ok(EditReport {
                    matches: 0,
                    skipped: 0,
                    truncated: false,
                    replaced: 0,
                    armed: false,
                })
            }
        }
    }

    fn find_move(
        &mut self,
        query: &str,
        case_sensitive: bool,
        whole_word: bool,
        forward: bool,
    ) -> Result<EditReport, InputError> {
        let report = self.searched(query, case_sensitive, whole_word)?;
        let outcome = self.report_of(&report, 0);
        if report.hits.is_empty() {
            return Ok(outcome);
        }
        let (start, end) = self.selection_positions()?;
        let cursor = if forward { end } else { start };
        let order = paragraph_order(
            self.backend.session().document(),
            cursor.story,
            cursor.paragraph,
        )
        .unwrap_or(0);
        let chosen = if forward {
            report
                .hits
                .iter()
                .find(|hit| {
                    hit.order > order || (hit.order == order && hit.start.0 >= cursor.offset.0)
                })
                .or(report.hits.first())
        } else {
            report
                .hits
                .iter()
                .rev()
                .find(|hit| {
                    hit.order < order || (hit.order == order && hit.end.0 <= cursor.offset.0)
                })
                .or(report.hits.last())
        };
        if let Some(hit) = chosen {
            self.select_hit(hit)?;
        }
        Ok(outcome)
    }

    fn replace_query(
        &mut self,
        query: &str,
        replacement: &str,
        case_sensitive: bool,
        whole_word: bool,
        all: bool,
    ) -> Result<EditReport, InputError> {
        let report = self.searched(query, case_sensitive, whole_word)?;
        let hits = if all {
            report.hits.clone()
        } else {
            let (start, end) = self.selection_positions()?;
            let current = report.hits.iter().find(|hit| {
                hit.story == start.story
                    && hit.paragraph == start.paragraph
                    && hit.start == start.offset
                    && hit.end == end.offset
            });
            let order =
                paragraph_order(self.backend.session().document(), end.story, end.paragraph)
                    .unwrap_or(0);
            let next = report.hits.iter().find(|hit| {
                hit.order > order || (hit.order == order && hit.start.0 >= end.offset.0)
            });
            match current.or(next).or(report.hits.first()) {
                Some(hit) => vec![hit.clone()],
                None => Vec::new(),
            }
        };
        if hits.is_empty() {
            return Ok(self.report_of(&report, 0));
        }
        let chosen = hits[0].clone();
        let plan = replace_commands(self.backend.session().document(), &hits, replacement)
            .map_err(find_input_error)?;
        if plan.commands.is_empty() {
            return Ok(self.report_of(&report, 0));
        }
        let mut transaction = Transaction::new(self.backend.session().version(), plan.commands);
        transaction.selection = Some(self.selection);
        self.commit(transaction)?;
        if !all {
            let end = chosen.start.0 + replacement.chars().count();
            self.select_positions(Selection {
                anchor: Position {
                    story: chosen.story,
                    paragraph: chosen.paragraph,
                    offset: chosen.start,
                    bias: Bias::After,
                },
                head: Position {
                    story: chosen.story,
                    paragraph: chosen.paragraph,
                    offset: ScalarOffset(end),
                    bias: Bias::Before,
                },
            })?;
        }
        Ok(self.report_of(&report, plan.replaced))
    }

    fn searched(
        &self,
        query: &str,
        case_sensitive: bool,
        whole_word: bool,
    ) -> Result<joyword_engine::FindReport, InputError> {
        search(
            self.backend.session().document(),
            &FindQuery {
                text: query.to_owned(),
                case_sensitive,
                whole_word,
            },
        )
        .map_err(find_input_error)
    }

    fn report_of(&self, report: &joyword_engine::FindReport, replaced: usize) -> EditReport {
        EditReport {
            matches: report.hits.len(),
            skipped: report.skipped,
            truncated: report.truncated,
            replaced,
            armed: self.brush.is_some(),
        }
    }

    fn select_hit(&mut self, hit: &FindHit) -> Result<(), InputError> {
        self.select_positions(Selection {
            anchor: Position {
                story: hit.story,
                paragraph: hit.paragraph,
                offset: hit.start,
                bias: Bias::After,
            },
            head: Position {
                story: hit.story,
                paragraph: hit.paragraph,
                offset: hit.end,
                bias: Bias::Before,
            },
        })
    }

    fn selection_positions(&self) -> Result<(Position, Position), InputError> {
        let (range, _) = self.view.selected_range();
        if range.is_empty() {
            Ok((self.selection.head, self.selection.head))
        } else {
            let start = self.view.index.position(range.start, Bias::After)?;
            let end = self.view.index.position(range.end, Bias::Before)?;
            Ok((start, end))
        }
    }

    /// Inserts an explicit page break at the caret.
    pub fn insert_page_break(&mut self) -> Result<bool, InputError> {
        self.require_editable()?;
        let at = self.selection.head;
        let transaction = Transaction::new(
            self.backend.session().version(),
            vec![Command::InsertInline {
                at,
                kind: InlineKind::Break(BreakKind::Page),
            }],
        );
        self.commit(transaction)
    }

    /// Sets the current section's page size in twips. Orientation is left as stored.
    pub fn set_page_size(&mut self, width: i64, height: i64) -> Result<bool, InputError> {
        self.revise_page(|page| {
            page.width = Measure::Twips(width);
            page.height = Measure::Twips(height);
        })
    }

    /// Sets the current section's body margins in twips.
    pub fn set_page_margins(
        &mut self,
        top: i64,
        right: i64,
        bottom: i64,
        left: i64,
    ) -> Result<bool, InputError> {
        self.revise_page(|page| {
            page.top = Measure::Twips(top);
            page.right = Measure::Twips(right);
            page.bottom = Measure::Twips(bottom);
            page.left = Measure::Twips(left);
        })
    }

    /// Portrait or landscape. Swaps width and height when the flag changes.
    pub fn set_page_orientation(&mut self, landscape: bool) -> Result<bool, InputError> {
        self.revise_page(|page| {
            let want = if landscape {
                Orientation::Landscape
            } else {
                Orientation::Portrait
            };
            if page.orientation != want {
                std::mem::swap(&mut page.width, &mut page.height);
                page.orientation = want;
            }
        })
    }

    fn revise_page(&mut self, update: impl FnOnce(&mut PageSetup)) -> Result<bool, InputError> {
        self.require_editable()?;
        let at = self.selection.head;
        let (section, mut page) = {
            let document = self.backend.session().document();
            let section =
                section_for_paragraph(document, at.paragraph).ok_or(InputError::InvalidDocument)?;
            let page = document
                .sections()
                .iter()
                .find(|item| item.id == section)
                .ok_or(InputError::InvalidDocument)?
                .page
                .clone();
            (section, page)
        };
        update(&mut page);
        let transaction = Transaction::new(
            self.backend.session().version(),
            vec![Command::SetPageSetup { section, page }],
        );
        self.commit(transaction)
    }

    /// Apply paragraph formatting to the paragraph that holds the caret.
    pub fn format_paragraph(&mut self, change: ParagraphChange) -> Result<bool, InputError> {
        self.require_editable()?;
        let at = self.selection.head;
        let transaction = Transaction::new(
            self.backend.session().version(),
            vec![Command::FormatParagraph {
                story: at.story,
                paragraph: at.paragraph,
                change,
            }],
        );
        self.commit(transaction)
    }

    fn require_editable(&self) -> Result<(), InputError> {
        if self.read_only() {
            return Err(InputError::ReadOnly);
        }
        if self.composition.is_some() {
            return Err(InputError::StaleComposition);
        }
        Ok(())
    }

    fn commit(&mut self, transaction: Transaction) -> Result<bool, InputError> {
        let target = self.selection;
        self.commit_with(transaction, target, |_document, _changes, selection| {
            Ok(selection)
        })
    }

    fn commit_with(
        &mut self,
        transaction: Transaction,
        target: Selection,
        adjust: impl FnOnce(&Document, &ChangeSet, Selection) -> Result<Selection, InputError>,
    ) -> Result<bool, InputError> {
        let revision = self.next_revision()?;
        let outcome = self.backend.apply_mapped(transaction, target)?;
        let changed = matches!(outcome, Outcome::Applied(_));
        let selection = match outcome {
            Outcome::Applied(applied) => {
                self.note(applied.changes.clone());
                let mapped = applied.selection.unwrap_or(target);
                let snapshot = self.backend.session().snapshot();
                adjust(&snapshot, &applied.changes, mapped).unwrap_or(mapped)
            }
            Outcome::NoOp(_) => target,
        };
        self.base = Arc::new(TextIndex::new(
            self.backend.session().snapshot(),
            self.limits,
        )?);
        self.selection = selection;
        self.composition = None;
        self.publish(self.base.clone(), selection, None, revision);
        Ok(changed)
    }

    /// Inserts a table before the body block that contains the caret and
    /// places the caret in its first cell.
    pub fn insert_table(&mut self, rows: u32, columns: u32) -> Result<bool, InputError> {
        self.require_editable()?;
        let transaction = Transaction::new(
            self.backend.session().version(),
            vec![Command::InsertTable {
                at: self.selection.head,
                rows,
                columns,
            }],
        );
        let target = self.selection;
        self.commit_with(transaction, target, |document, changes, mapped| {
            let Some(table) = changes.tables.iter().next().copied() else {
                return Ok(mapped);
            };
            cell_caret(
                document,
                CellAddress {
                    story: document.body_id(),
                    table,
                    row: 0,
                    cell: 0,
                },
            )
            .or(Ok(mapped))
        })
    }

    /// Inserts a row after the caret's row and moves the caret into that row.
    pub fn insert_table_row(&mut self) -> Result<bool, InputError> {
        self.require_editable()?;
        let transaction = Transaction::new(
            self.backend.session().version(),
            vec![Command::InsertTableRow {
                at: self.selection.head,
            }],
        );
        let target = self.selection;
        self.commit_with(transaction, target, |document, changes, mapped| {
            let Some(paragraph) = changes.inserted.iter().next().copied() else {
                return Ok(mapped);
            };
            Ok(Selection::caret(Position {
                story: document.body_id(),
                paragraph,
                offset: ScalarOffset(0),
                bias: Bias::After,
            }))
        })
    }

    pub fn delete_table_row(&mut self) -> Result<bool, InputError> {
        self.require_editable()?;
        let transaction = Transaction::new(
            self.backend.session().version(),
            vec![Command::DeleteTableRow {
                at: self.selection.head,
            }],
        );
        self.commit(transaction)
    }

    /// Inserts a column after the caret's cell and moves the caret into it.
    pub fn insert_table_column(&mut self) -> Result<bool, InputError> {
        self.require_editable()?;
        let transaction = Transaction::new(
            self.backend.session().version(),
            vec![Command::InsertTableColumn {
                at: self.selection.head,
            }],
        );
        let target = self.selection;
        self.commit_with(transaction, target, |document, _changes, mapped| {
            let Ok(address) = cell_address(document, &mapped.head) else {
                return Ok(mapped);
            };
            cell_caret(
                document,
                CellAddress {
                    cell: address.cell + 1,
                    ..address
                },
            )
            .or(Ok(mapped))
        })
    }

    pub fn delete_table_column(&mut self) -> Result<bool, InputError> {
        self.require_editable()?;
        let transaction = Transaction::new(
            self.backend.session().version(),
            vec![Command::DeleteTableColumn {
                at: self.selection.head,
            }],
        );
        self.commit(transaction)
    }

    /// Selects every paragraph in the cell that contains the caret.
    pub fn select_table_cell(&mut self) -> Result<(), InputError> {
        if self.is_composing() {
            return Err(InputError::StaleComposition);
        }
        let document = self.backend.session().snapshot();
        let address = cell_address(&document, &self.selection.head).map_err(command_error)?;
        let selection = cell_selection(&document, address).map_err(command_error)?;
        self.select_positions(selection)
    }

    pub fn mark(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) -> Result<(), InputError> {
        self.writable()?;
        let text = self.normalize(text, true)?;
        let selected = selected.unwrap_or_else(|| {
            let n = text.encode_utf16().count();
            n..n
        });
        // The platform's selected range is relative to the new marked string.
        utf16_bytes(&text, selected.clone())?;
        let (base_range, overlay, marked) = self.fold(range, &text)?;
        if overlay.len() > self.limits.max_composition_bytes {
            return Err(InputError::Budget("folded composition bytes"));
        }
        if text.is_empty() {
            self.cancel_composition()?;
            return Ok(());
        }
        let prepared = self.prepare(base_range.clone(), &overlay, UndoGroup::Standalone)?;
        let selection = prepared.index.selection(
            marked.start + selected.start..marked.start + selected.end,
            false,
        )?;
        let revision = self.next_revision()?;
        self.composition = Some(Composition {
            base_version: self.backend.session().version(),
            base_range,
            text: overlay,
            marked: marked.clone(),
        });
        self.publish(prepared.index, selection, Some(marked), revision);
        Ok(())
    }
    /// AppKit unmark preserves the marked text; commit that preview once.
    pub fn unmark(&mut self) -> Result<bool, InputError> {
        let Some(c) = self.composition.clone() else {
            return Ok(false);
        };
        self.writable()?;
        // A committed composition is one standalone undo step (SPEC §6).
        let prepared = self.prepare(c.base_range, &c.text, UndoGroup::Standalone)?;
        let revision = self.next_revision()?;
        let outcome = self
            .backend
            .apply_mapped(prepared.transaction, prepared.target)?;
        if let Outcome::Applied(applied) = &outcome {
            self.note(applied.changes.clone());
        }
        let selection = match outcome {
            Outcome::Applied(a) => a.selection.expect("validated target"),
            Outcome::NoOp(_) => prepared.selection,
        };
        self.base = Arc::new(TextIndex {
            snapshot: self.backend.session().snapshot(),
            ..(*prepared.index).clone()
        });
        self.selection = selection;
        self.composition = None;
        self.publish(self.base.clone(), selection, None, revision);
        Ok(true)
    }
    pub fn cancel_composition(&mut self) -> Result<bool, InputError> {
        if self.composition.is_none() {
            return Ok(false);
        }
        let revision = self.next_revision()?;
        self.composition = None;
        self.publish(self.base.clone(), self.selection, None, revision);
        Ok(true)
    }
    pub fn select(&mut self, range: Range<usize>, reversed: bool) -> Result<(), InputError> {
        if self.is_composing() {
            return Err(InputError::StaleComposition);
        }
        let selection = self.base.selection(range, reversed)?;
        self.set_selection(selection);
        Ok(())
    }
    pub fn select_positions(&mut self, selection: Selection) -> Result<(), InputError> {
        if self.is_composing() {
            return Err(InputError::StaleComposition);
        }
        self.base.selection_range(selection)?;
        self.set_selection(selection);
        Ok(())
    }
    /// Moving the caret or selection ends the open typing/deleting group, so
    /// typing that later returns to the same place starts a new undo step.
    fn set_selection(&mut self, selection: Selection) {
        if selection != self.selection {
            self.backend.seal_undo_group();
        }
        self.selection = selection;
        self.view.selection = selection;
    }
    pub fn select_all(&mut self) -> Result<(), InputError> {
        self.cancel_composition()?;
        self.select(0..self.base.len_utf16, false)
    }
    pub fn select_word_at(&mut self, offset: usize) -> Result<(), InputError> {
        if self.is_composing() {
            return Err(InputError::StaleComposition);
        }
        let byte = self.base.byte(offset)?;
        let span = self
            .base
            .text
            .split_word_bound_indices()
            .find(|(i, s)| *i <= byte && byte < i + s.len())
            .or_else(|| self.base.text.split_word_bound_indices().next_back());
        let Some((start, text)) = span else {
            return self.select(0..0, false);
        };
        let first = self.base.text[..start].encode_utf16().count();
        self.select(first..first + text.encode_utf16().count(), false)
    }
    pub fn select_paragraph_at(&mut self, offset: usize) -> Result<(), InputError> {
        if self.is_composing() {
            return Err(InputError::StaleComposition);
        }
        let p = self.base.position(offset, Bias::After)?;
        let index = self.base.by_id[&p.paragraph];
        let start = self.base.paragraphs[index].utf16;
        let end = self
            .base
            .paragraphs
            .get(index + 1)
            .map_or(self.base.len_utf16, |p| p.utf16);
        self.select(start..end, false)
    }
    pub fn move_grapheme(&mut self, direction: Direction, extend: bool) -> Result<(), InputError> {
        self.cancel_composition()?;
        let (range, _) = self.view.selected_range();
        let current = self.base.offset(self.selection.head)?;
        let next = if !extend && !range.is_empty() {
            match direction {
                Direction::Backward => range.start,
                Direction::Forward => range.end,
            }
        } else {
            self.adjacent(current, direction)?
        };
        let position = self.base.position(
            next,
            match direction {
                Direction::Backward => Bias::Before,
                Direction::Forward => Bias::After,
            },
        )?;
        self.select_positions(if extend {
            Selection {
                anchor: self.selection.anchor,
                head: position,
            }
        } else {
            Selection::caret(position)
        })
    }
    fn adjacent(&self, offset: usize, direction: Direction) -> Result<usize, InputError> {
        let byte = self.base.byte(offset)?;
        let next = match direction {
            Direction::Backward => self
                .base
                .text
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .take_while(|i| *i < byte)
                .last()
                .unwrap_or(0),
            Direction::Forward => self
                .base
                .text
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .find(|i| *i > byte)
                .unwrap_or(self.base.text.len()),
        };
        Ok(self.base.text[..next].encode_utf16().count())
    }
    pub fn delete(&mut self, direction: Direction) -> Result<bool, InputError> {
        self.cancel_composition()?;
        let (range, _) = self.view.selected_range();
        // Even a programmatic scalar-valid selection inside a combining/ZWJ
        // sequence must not leave a broken fragment after a keyboard delete.
        let (mut left, mut right) = (self.base.byte(range.start)?, self.base.byte(range.end)?);
        for (start, grapheme) in self.base.text.grapheme_indices(true) {
            let end = start + grapheme.len();
            if start < left && left < end {
                left = start;
            }
            if start < right && right < end {
                right = end;
            }
        }
        let range = self.base.text[..left].encode_utf16().count()
            ..self.base.text[..right].encode_utf16().count();
        let range = if range.is_empty() {
            let next = self.adjacent(range.start, direction)?;
            next.min(range.start)..next.max(range.end)
        } else {
            range
        };
        self.replace_grouped(Some(range), "", UndoGroup::Deleting)
    }
    pub fn undo(&mut self) -> Result<(), InputError> {
        if self.cancel_composition()? {
            return Ok(());
        }
        self.writable()?;
        let revision = self.next_revision()?;
        let (snapshot, selection) = self.backend.preview_undo()?;
        let candidate = TextIndex::new(snapshot, self.limits)?;
        let selection = selection
            .unwrap_or_else(|| Selection::caret(candidate.position(0, Bias::After).expect("body")));
        candidate.selection_range(selection)?;
        let undone = self.backend.undo()?;
        self.note(undone.changes);
        let base = Arc::new(TextIndex {
            snapshot: self.backend.session().snapshot(),
            ..candidate
        });
        self.base = base;
        self.selection = selection;
        self.publish(self.base.clone(), selection, None, revision);
        Ok(())
    }
    pub fn redo(&mut self) -> Result<(), InputError> {
        self.writable()?;
        if self.is_composing() {
            return Err(InputError::StaleComposition);
        }
        let revision = self.next_revision()?;
        let (snapshot, selection) = self.backend.preview_redo()?;
        let candidate = TextIndex::new(snapshot, self.limits)?;
        let selection = selection
            .unwrap_or_else(|| Selection::caret(candidate.position(0, Bias::After).expect("body")));
        candidate.selection_range(selection)?;
        let redone = self.backend.redo()?;
        self.note(redone.changes);
        let base = Arc::new(TextIndex {
            snapshot: self.backend.session().snapshot(),
            ..candidate
        });
        self.base = base;
        self.selection = selection;
        self.publish(self.base.clone(), selection, None, revision);
        Ok(())
    }
}

impl TextInput<crate::docx::BoundDocx> {
    pub fn save_original_job(&self) -> crate::docx::SaveOriginalJob {
        self.backend.save_original_job()
    }

    /// Install a published result and advance the dirty baseline to exactly
    /// the version it wrote. Cleanup errors with a published receipt count too:
    /// the bytes are on disk even though the caller still receives the error.
    pub fn apply_original_save(
        &mut self,
        output: crate::docx::SaveOriginalJobOutput,
    ) -> Result<crate::storage::SaveOriginalReceipt, crate::docx::BoundOriginalSaveError> {
        let saved = output.published_receipt().map(|receipt| receipt.version());
        let result = self.backend.apply_original_save(output);
        if let Some(version) = saved {
            self.mark_saved(version);
        }
        result
    }

    pub fn save_original(
        &mut self,
        choice: crate::storage::OriginalChoice,
    ) -> Result<crate::storage::SaveOriginalReceipt, crate::docx::BoundOriginalSaveError> {
        let output = self.save_original_job().run(choice);
        self.apply_original_save(output)
    }
}

fn command_error(error: CommandError) -> InputError {
    InputError::Transaction(TxError::Command { index: 0, error })
}

fn find_input_error(error: FindError) -> InputError {
    match error {
        FindError::Empty => InputError::Rejected("empty query"),
        FindError::TooLong => InputError::Budget("query"),
        FindError::InvalidReplacement(ch) => InputError::InvalidText(ch),
    }
}

fn paint_input_error(error: PaintError) -> InputError {
    match error {
        PaintError::Unexportable(reason) => InputError::Rejected(reason),
        PaintError::Locked => InputError::Rejected("locked content"),
        PaintError::CrossContainer => InputError::Rejected("range crosses a container"),
        PaintError::Missing => InputError::InvalidRange,
        PaintError::Nothing => InputError::Rejected("nothing to paint"),
    }
}

fn describe_run(props: &RunProps) -> String {
    let mut parts = Vec::new();
    if props.bold == Some(true) {
        parts.push("bold".to_owned());
    }
    if props.italic == Some(true) {
        parts.push("italic".to_owned());
    }
    if let Some(Measure::HalfPoints(size)) = props.size {
        parts.push(format!("{size}hp"));
    }
    match props.color {
        Some(Color::Auto) => parts.push("auto".to_owned()),
        Some(Color::Rgb(red, green, blue)) => {
            parts.push(format!("#{red:02X}{green:02X}{blue:02X}"));
        }
        Some(Color::Theme(_)) => parts.push("theme".to_owned()),
        None => {}
    }
    if let Some(name) = named_font(&props.fonts.east_asia) {
        parts.push(name);
    }
    if let Some(name) = named_font(&props.fonts.ascii) {
        parts.push(name);
    }
    parts.join(" ")
}

fn named_font(font: &Option<FontRef>) -> Option<String> {
    match font {
        Some(FontRef::Named(name)) => Some(name.to_string()),
        Some(FontRef::Theme(_)) | None => None,
    }
}

fn paragraph_by_id(document: &Document, id: NodeId) -> Option<&Paragraph> {
    fn walk(blocks: &BlockSeq, id: NodeId, depth: usize) -> Option<&Paragraph> {
        if depth > MAX_NESTING {
            return None;
        }
        for block in blocks.iter() {
            match block {
                Block::Paragraph(paragraph) if paragraph.id() == id => return Some(paragraph),
                Block::Table(table) => {
                    for cell in table.rows.iter().flat_map(|row| row.cells.iter()) {
                        if let Some(found) = walk(&cell.blocks, id, depth + 1) {
                            return Some(found);
                        }
                    }
                }
                Block::Sdt(sdt) => {
                    if let Some(found) = walk(&sdt.blocks, id, depth + 1) {
                        return Some(found);
                    }
                }
                Block::Paragraph(_) | Block::Opaque(_) => {}
            }
        }
        None
    }
    for story in document.stories() {
        if let Some(found) = walk(&story.blocks, id, 0) {
            return Some(found);
        }
    }
    None
}

fn utf16_bytes(text: &str, range: Range<usize>) -> Result<Range<usize>, InputError> {
    if range.start > range.end {
        return Err(InputError::InvalidRange);
    }
    let byte = |wanted: usize| {
        let mut n = 0;
        for (i, ch) in text.char_indices() {
            if n == wanted {
                return Ok(i);
            }
            if n + ch.len_utf16() > wanted {
                return Err(InputError::InsideSurrogate);
            }
            n += ch.len_utf16();
        }
        if n == wanted {
            Ok(text.len())
        } else {
            Err(InputError::InvalidRange)
        }
    };
    Ok(byte(range.start)?..byte(range.end)?)
}

fn section_for_paragraph(document: &Document, paragraph: NodeId) -> Option<NodeId> {
    let blocks = &document.body().blocks;
    let index = blocks.iter().position(|block| block.id() == paragraph)?;
    for section in document.sections() {
        match section.last_block {
            Some(id) => {
                if let Some(end) = blocks.iter().position(|block| block.id() == id)
                    && index <= end
                {
                    return Some(section.id);
                }
            }
            None => return Some(section.id),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use joyword_model::Document;

    fn blank() -> TextInput {
        TextInput::new(Session::new(Document::new_blank()), InputLimits::default()).unwrap()
    }
    #[test]
    fn generation_exhaustion_is_checked_before_any_commit_or_preview_change() {
        let mut input = blank();
        input.mark(None, "ni", None).unwrap();
        input.view.revision = u64::MAX;
        let version = input.backend.version();
        assert_eq!(
            input.replace(None, "你"),
            Err(InputError::GenerationOverflow)
        );
        assert_eq!(
            input.mark(None, "你好", None),
            Err(InputError::GenerationOverflow)
        );
        assert_eq!(
            input.cancel_composition(),
            Err(InputError::GenerationOverflow)
        );
        assert_eq!(input.unmark(), Err(InputError::GenerationOverflow));
        assert_eq!(input.backend.version(), version);
        assert_eq!(input.view.index.text(), "ni");
        assert!(input.is_composing());
        assert!(!input.backend.can_undo());
    }
    #[test]
    fn externally_advanced_backend_cannot_accept_a_stale_preedit() {
        let mut input = blank();
        input.mark(None, "ni", None).unwrap();
        let at = input.base.position(0, Bias::After).unwrap();
        input
            .backend
            .apply(Transaction::new(
                input.backend.version(),
                vec![Command::InsertText {
                    at,
                    text: "x".into(),
                }],
            ))
            .unwrap();
        let version = input.backend.version();
        let revision = input.view.revision;
        assert_eq!(input.replace(None, "你"), Err(InputError::StaleComposition));
        assert_eq!(input.unmark(), Err(InputError::StaleComposition));
        assert_eq!(
            input.mark(None, "你好", None),
            Err(InputError::StaleComposition)
        );
        assert_eq!(input.backend.version(), version);
        assert_eq!(input.view.revision, revision);
        assert_eq!(input.view.index.text(), "ni");
        assert!(input.backend.can_undo());
    }
}
