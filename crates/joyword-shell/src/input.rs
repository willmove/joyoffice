//! Platform-neutral body text input (T102). UTF-16 is an API boundary;
//! persisted positions are model paragraph/scalar positions. Composition is
//! an immutable preview session and never modifies the committed undo stack.
use crate::docx::DocxSession;
use joyword_engine::{
    Command, Outcome, Selection, Session, Transaction, TxError, UndoError, UndoResult,
};
use joyword_model::{Bias, DocumentSnapshot, DocumentVersion, NodeId, Position, Utf16Offset};
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;
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
    Budget(&'static str),
    ReadOnly,
    StaleComposition,
    GenerationOverflow,
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

#[derive(Clone, Debug)]
struct ParagraphEntry {
    id: NodeId,
    byte: usize,
    utf16: usize,
    len_utf16: usize,
    len_scalar: usize,
}
/// Flat platform projection joins root body paragraphs with one LF. It never
/// invents text for tables, objects or additional stories.
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
        if snapshot.stories().count() != 1 {
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
        for block in snapshot.body().blocks.iter() {
            let p = block.as_paragraph().ok_or(InputError::UnsupportedBody)?;
            if !p.inlines().is_empty() {
                return Err(InputError::UnsupportedBody);
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
            let content = p.plain_text(|_| unreachable!("objects rejected"));
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
        let paragraph = self
            .snapshot
            .body()
            .blocks
            .get(i)
            .and_then(|b| b.as_paragraph())
            .expect("indexed paragraph");
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
        let paragraph = self
            .snapshot
            .body()
            .blocks
            .get(i)
            .and_then(|b| b.as_paragraph())
            .expect("indexed paragraph");
        let n = paragraph
            .text()
            .scalar_to_utf16(position.offset)
            .map_err(|_| InputError::UnknownPosition(position))?;
        Ok(p.utf16 + n.0)
    }
    fn byte(&self, offset: usize) -> Result<usize, InputError> {
        let position = self.position(offset, Bias::After)?;
        let i = self.by_id[&position.paragraph];
        let p = self
            .snapshot
            .body()
            .blocks
            .get(i)
            .and_then(|b| b.as_paragraph())
            .expect("indexed paragraph");
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
    initial_version: DocumentVersion,
}
impl<B: InputBackend> TextInput<B> {
    pub fn new(backend: B, limits: InputLimits) -> Result<Self, InputError> {
        let limits = limits.capped();
        let initial_version = backend.session().version();
        let base = Arc::new(TextIndex::new(backend.session().snapshot(), limits)?);
        let selection = Selection::caret(base.position(0, Bias::After)?);
        let view = InputView {
            index: base.clone(),
            selection,
            marked: None,
            revision: 0,
        };
        Ok(Self {
            backend,
            base,
            selection,
            composition: None,
            view,
            limits,
            initial_version,
        })
    }
    pub fn backend(&self) -> &B {
        &self.backend
    }
    pub fn view(&self) -> &InputView {
        &self.view
    }
    pub fn is_composing(&self) -> bool {
        self.composition.is_some()
    }
    pub fn has_unsaved_changes(&self) -> bool {
        self.backend.session().version() != self.initial_version
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
    fn prepare(&self, range: Range<usize>, text: &str) -> Result<Prepared, InputError> {
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
    pub fn replace(&mut self, range: Option<Range<usize>>, text: &str) -> Result<bool, InputError> {
        let text = self.normalize(text, false)?;
        let (base_range, text, _) = self.fold(range, &text)?;
        if text.len() > self.limits.max_insert_bytes {
            return Err(InputError::Budget("folded insert bytes"));
        }
        let prepared = self.prepare(base_range, &text)?;
        let revision = self.next_revision()?;
        let outcome = self
            .backend
            .apply_mapped(prepared.transaction, prepared.target)?;
        let changed = matches!(outcome, Outcome::Applied(_));
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
        self.view = InputView {
            index: self.base.clone(),
            selection,
            marked: None,
            revision,
        };
        Ok(changed)
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
        let prepared = self.prepare(base_range.clone(), &overlay)?;
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
        self.view = InputView {
            index: prepared.index,
            selection,
            marked: Some(marked),
            revision,
        };
        Ok(())
    }
    /// AppKit unmark preserves the marked text; commit that preview once.
    pub fn unmark(&mut self) -> Result<bool, InputError> {
        let Some(c) = self.composition.clone() else {
            return Ok(false);
        };
        self.writable()?;
        let prepared = self.prepare(c.base_range, &c.text)?;
        let revision = self.next_revision()?;
        let outcome = self
            .backend
            .apply_mapped(prepared.transaction, prepared.target)?;
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
        self.view = InputView {
            index: self.base.clone(),
            selection,
            marked: None,
            revision,
        };
        Ok(true)
    }
    pub fn cancel_composition(&mut self) -> Result<bool, InputError> {
        if self.composition.is_none() {
            return Ok(false);
        }
        let revision = self.next_revision()?;
        self.composition = None;
        self.view = InputView {
            index: self.base.clone(),
            selection: self.selection,
            marked: None,
            revision,
        };
        Ok(true)
    }
    pub fn select(&mut self, range: Range<usize>, reversed: bool) -> Result<(), InputError> {
        if self.is_composing() {
            return Err(InputError::StaleComposition);
        }
        let selection = self.base.selection(range, reversed)?;
        self.selection = selection;
        self.view.selection = selection;
        Ok(())
    }
    pub fn select_positions(&mut self, selection: Selection) -> Result<(), InputError> {
        if self.is_composing() {
            return Err(InputError::StaleComposition);
        }
        self.base.selection_range(selection)?;
        self.selection = selection;
        self.view.selection = selection;
        Ok(())
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
        self.replace(Some(range), "")
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
        self.backend.undo()?;
        let base = Arc::new(TextIndex {
            snapshot: self.backend.session().snapshot(),
            ..candidate
        });
        self.base = base;
        self.selection = selection;
        self.view = InputView {
            index: self.base.clone(),
            selection,
            marked: None,
            revision,
        };
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
        self.backend.redo()?;
        let base = Arc::new(TextIndex {
            snapshot: self.backend.session().snapshot(),
            ..candidate
        });
        self.base = base;
        self.selection = selection;
        self.view = InputView {
            index: self.base.clone(),
            selection,
            marked: None,
            revision,
        };
        Ok(())
    }
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
