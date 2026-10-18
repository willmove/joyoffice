//! Document session: atomic transactions, version checks, permissions,
//! annotation mapping and snapshot-based undo (ARCHITECTURE §3–§4).
//!
//! `apply` runs every command on a candidate copy (O(1) thanks to structural
//! sharing). Any failure drops the candidate, so the document, its version
//! and the undo history stay exactly as they were. Undo records keep the
//! shared before/after states, which restores content, formatting and anchors
//! exactly; restoring publishes a new version and never reuses an old one.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use joyword_model::{
    Anchor, AnnotationId, AnnotationKind, Bias, Block, Document, DocumentSnapshot, DocumentVersion,
    InlineKind, InlineObject, Issue, MAX_LEVEL, MAX_NESTING, NodeId, NumberingRef, ParaProps,
    Paragraph, Position, RunProps, ScalarOffset, StoryId, StyleId, StyleKind, StyleProblem,
    TextError,
};

thread_local! {
    static FAIL_NEXT_APPLY: Cell<bool> = const { Cell::new(false) };
}

/// Test hook: the next [`Session::apply`] panics inside the transaction guard.
#[cfg(test)]
fn fail_next_apply() {
    FAIL_NEXT_APPLY.with(|flag| flag.set(true));
}

use crate::command::{Command, CommandError, ParagraphChange, Precondition, check_text};
use crate::locate::{self, Location};
use crate::map::{MapStep, PositionMap, Selection};

/// Who asked for a change. Agent writes pass the AI switch (AI-005).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    User,
    Agent,
    System,
}

/// Session-wide permission switches checked before any command runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditPolicy {
    /// Protected, tracked-change or otherwise unsupported documents open
    /// read-only until the matching capability exists (FORMAT-002).
    pub read_only: bool,
    /// Off by default: AI is disabled until the user enables it (AI-001).
    pub agent_writes: bool,
}

/// How a committed transaction joins the undo history (T209, SPEC §6).
///
/// Only `Typing` and `Deleting` may coalesce, and only with the immediately
/// preceding history entry of the same kind and origin whose resulting
/// selection equals this transaction's starting selection. Undo, redo,
/// [`Session::seal_undo_group`] and any standalone commit close the group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UndoGroup {
    /// Its own undo step: composition commits, pastes, drag gestures,
    /// autocorrect, formatting, structural edits and every Agent change.
    #[default]
    Standalone,
    /// Consecutive keyboard typing.
    Typing,
    /// Consecutive keyboard deletion (backspace / forward delete).
    Deleting,
}

/// Upper bound of transactions coalesced into one undo step, so a long
/// typing burst still undoes in bounded chunks. The exact Word grouping
/// granularity is unobserved (behavior registry `EDIT-UNDO-TYPING-CAP`).
pub const MAX_UNDO_GROUP_STEPS: usize = 100;

#[derive(Clone, Debug, PartialEq)]
pub struct Transaction {
    pub base_version: DocumentVersion,
    pub origin: Origin,
    pub preconditions: Vec<Precondition>,
    pub commands: Vec<Command>,
    /// Selection before the change, restored by undo.
    pub selection: Option<Selection>,
    /// Undo coalescing class; `Standalone` unless the input layer says otherwise.
    pub undo_group: UndoGroup,
}

impl Transaction {
    pub fn new(base_version: DocumentVersion, commands: Vec<Command>) -> Self {
        Self {
            base_version,
            origin: Origin::User,
            preconditions: Vec::new(),
            commands,
            selection: None,
            undo_group: UndoGroup::Standalone,
        }
    }
}

/// What a transaction touched; drives layout invalidation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChangeSet {
    pub stories: BTreeSet<StoryId>,
    /// Paragraphs whose content or format changed.
    pub paragraphs: BTreeSet<NodeId>,
    pub inserted: BTreeSet<NodeId>,
    pub removed: BTreeSet<NodeId>,
    pub moved: BTreeSet<NodeId>,
    pub annotations: BTreeSet<AnnotationId>,
    /// Style definitions whose resolved properties changed. Layout invalidates
    /// the basedOn/link closure; it does not clear every fitted paragraph.
    pub styles: BTreeSet<StyleId>,
    /// `w:docDefaults` changed, so every paragraph's resolved style is stale.
    pub defaults_changed: bool,
    /// Tables whose grid, rows or cells changed.
    pub tables: BTreeSet<NodeId>,
}

impl ChangeSet {
    pub fn is_empty(&self) -> bool {
        self.paragraphs.is_empty()
            && self.inserted.is_empty()
            && self.removed.is_empty()
            && self.moved.is_empty()
            && self.annotations.is_empty()
            && self.styles.is_empty()
            && self.tables.is_empty()
            && !self.defaults_changed
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApplyResult {
    pub version: DocumentVersion,
    pub changes: ChangeSet,
    pub map: PositionMap,
    /// The transaction's selection mapped through the change.
    pub selection: Option<Selection>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Applied(Box<ApplyResult>),
    /// Nothing changed; the version and undo history are untouched.
    NoOp(DocumentVersion),
}

#[derive(Clone, Debug, PartialEq)]
pub enum TxError {
    StaleVersion {
        base: DocumentVersion,
        current: DocumentVersion,
    },
    ReadOnly,
    AgentWritesDisabled,
    PreconditionFailed {
        index: usize,
    },
    Command {
        index: usize,
        error: CommandError,
    },
    /// The candidate broke a model invariant; nothing was committed.
    Invariant(Vec<Issue>),
    /// A panic was caught inside the transaction guard. The session is unchanged.
    Interrupted,
}

impl fmt::Display for TxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TxError::StaleVersion { base, current } => {
                write!(f, "transaction based on {base}, document is at {current}")
            }
            TxError::ReadOnly => f.write_str("document is read-only"),
            TxError::AgentWritesDisabled => f.write_str("AI writes are disabled"),
            TxError::PreconditionFailed { index } => write!(f, "precondition {index} failed"),
            TxError::Command { index, error } => write!(f, "command {index}: {error}"),
            TxError::Invariant(issues) => write!(f, "invariant violated: {issues:?}"),
            TxError::Interrupted => f.write_str("transaction was interrupted and discarded"),
        }
    }
}

impl std::error::Error for TxError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UndoError {
    Nothing,
    ReadOnly,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UndoResult {
    pub version: DocumentVersion,
    pub changes: ChangeSet,
    /// Selection to restore.
    pub selection: Option<Selection>,
}

#[derive(Clone, Debug)]
struct UndoEntry {
    before: Document,
    after: Document,
    /// Version of the state this entry can be applied to.
    version: DocumentVersion,
    selection_before: Option<Selection>,
    selection_after: Option<Selection>,
    /// Union of every coalesced transaction's changes.
    changes: ChangeSet,
    group: UndoGroup,
    origin: Origin,
    /// Transactions folded into this entry (1 for an ungrouped step).
    steps: usize,
}

impl ChangeSet {
    /// Union `other` into this set. Undo coalescing and the input layer both
    /// fold several transactions into the invalidation the next layout sees.
    pub fn absorb(&mut self, other: ChangeSet) {
        self.stories.extend(other.stories);
        self.paragraphs.extend(other.paragraphs);
        self.inserted.extend(other.inserted);
        self.removed.extend(other.removed);
        self.moved.extend(other.moved);
        self.annotations.extend(other.annotations);
        self.styles.extend(other.styles);
        self.tables.extend(other.tables);
        self.defaults_changed |= other.defaults_changed;
    }
}

pub struct Session {
    document: Document,
    policy: EditPolicy,
    undo: Vec<UndoEntry>,
    redo: Vec<UndoEntry>,
    /// Whether the top undo entry may still absorb a coalescing transaction.
    group_open: bool,
}

impl Session {
    pub fn new(document: Document) -> Self {
        Self {
            document,
            policy: EditPolicy::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            group_open: false,
        }
    }

    /// Close the current typing/deleting group: the next transaction starts
    /// a new undo step. Callers use this at explicit operation boundaries
    /// such as caret moves, review-mode switches or author changes.
    pub fn seal_undo_group(&mut self) {
        self.group_open = false;
    }

    /// Number of undo steps (coalesced groups count once).
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Number of redo steps.
    pub fn redo_depth(&self) -> usize {
        self.redo.len()
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn snapshot(&self) -> DocumentSnapshot {
        self.document.snapshot()
    }

    pub fn version(&self) -> DocumentVersion {
        self.document.version()
    }

    pub fn policy(&self) -> &EditPolicy {
        &self.policy
    }

    pub fn set_policy(&mut self, policy: EditPolicy) {
        self.policy = policy;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn apply(&mut self, transaction: Transaction) -> Result<Outcome, TxError> {
        self.guard(|session| session.apply_inner(transaction, None))
    }

    /// Keep `transaction.selection` for undo, while mapping a separate
    /// pre-transaction target selection to the committed state for redo.
    /// A text replacement can thus restore the original range on undo and
    /// the insertion-end caret on redo. Existing `apply` behavior is unchanged.
    pub fn apply_with_mapped_selection(
        &mut self,
        transaction: Transaction,
        target: Selection,
    ) -> Result<Outcome, TxError> {
        self.guard(|session| session.apply_inner(transaction, Some(target)))
    }

    fn guard(
        &mut self,
        body: impl FnOnce(&mut Self) -> Result<Outcome, TxError>,
    ) -> Result<Outcome, TxError> {
        let document = self.document.clone();
        let policy = self.policy.clone();
        let undo = self.undo.clone();
        let redo = self.redo.clone();
        let group_open = self.group_open;
        match catch_unwind(AssertUnwindSafe(|| body(self))) {
            Ok(result) => result,
            Err(_) => {
                self.document = document;
                self.policy = policy;
                self.undo = undo;
                self.redo = redo;
                self.group_open = group_open;
                Err(TxError::Interrupted)
            }
        }
    }

    fn apply_inner(
        &mut self,
        transaction: Transaction,
        target: Option<Selection>,
    ) -> Result<Outcome, TxError> {
        if FAIL_NEXT_APPLY.with(|flag| flag.replace(false)) {
            panic!("injected transaction panic");
        }
        let current = self.document.version();
        if transaction.base_version != current {
            return Err(TxError::StaleVersion {
                base: transaction.base_version,
                current,
            });
        }
        if transaction.commands.is_empty() {
            return Ok(Outcome::NoOp(current));
        }
        if self.policy.read_only {
            return Err(TxError::ReadOnly);
        }
        if transaction.origin == Origin::Agent && !self.policy.agent_writes {
            return Err(TxError::AgentWritesDisabled);
        }
        for (index, precondition) in transaction.preconditions.iter().enumerate() {
            if !holds(&self.document, precondition) {
                return Err(TxError::PreconditionFailed { index });
            }
        }

        let mut candidate = self.document.clone();
        let mut map = PositionMap::new();
        let mut changes = ChangeSet::default();
        for (index, command) in transaction.commands.iter().enumerate() {
            let mut step = PositionMap::new();
            execute(&mut candidate, command, &mut step, &mut changes)
                .map_err(|error| TxError::Command { index, error })?;
            remap_annotations(&mut candidate, &step, &mut changes);
            map = map.then(&step);
        }
        if changes.is_empty() {
            return Ok(Outcome::NoOp(current));
        }
        let issues = touched_issues(&candidate, &changes);
        if !issues.is_empty() {
            return Err(TxError::Invariant(issues));
        }

        let version = candidate.advance_version();
        let selection = target
            .or(transaction.selection)
            .and_then(|selection| selection.map(&map));
        let merges = self.coalesces_with_top(&transaction);
        let before = std::mem::replace(&mut self.document, candidate);
        let after = Document::clone(&self.document);
        match self.undo.last_mut() {
            Some(top) if merges => {
                // The group keeps its first `before` state and selection; it
                // now ends at this transaction's result.
                top.after = after;
                top.version = version;
                top.selection_after = selection;
                top.changes.absorb(changes.clone());
                top.steps += 1;
            }
            _ => self.undo.push(UndoEntry {
                before,
                after,
                version,
                selection_before: transaction.selection,
                selection_after: selection,
                changes: changes.clone(),
                group: transaction.undo_group,
                origin: transaction.origin,
                steps: 1,
            }),
        }
        self.group_open = transaction.undo_group != UndoGroup::Standalone;
        self.redo.clear();
        Ok(Outcome::Applied(Box::new(ApplyResult {
            version,
            changes,
            map,
            selection,
        })))
    }

    /// Coalescing never crosses an undo/redo, a seal, a standalone commit,
    /// another kind or origin, a discontinuous selection, or the step cap.
    /// Only user input coalesces; Agent and system changes stay separate.
    fn coalesces_with_top(&self, transaction: &Transaction) -> bool {
        let Some(top) = self.undo.last() else {
            return false;
        };
        self.group_open
            && transaction.undo_group != UndoGroup::Standalone
            && transaction.origin == Origin::User
            && top.group == transaction.undo_group
            && top.origin == transaction.origin
            && top.steps < MAX_UNDO_GROUP_STEPS
            && transaction.selection.is_some()
            && transaction.selection == top.selection_after
    }

    pub fn undo(&mut self) -> Result<UndoResult, UndoError> {
        self.step_history(true)
    }

    pub fn redo(&mut self) -> Result<UndoResult, UndoError> {
        self.step_history(false)
    }

    /// Content and view selection that undo would restore, without consuming
    /// history. The snapshot retains its stored version; a real undo still
    /// publishes a new monotonic version. Consumers can preflight budgets.
    pub fn preview_undo(&self) -> Result<(DocumentSnapshot, Option<Selection>), UndoError> {
        self.preview_history(true)
    }
    /// Read-only counterpart of `preview_undo` for the redo stack.
    pub fn preview_redo(&self) -> Result<(DocumentSnapshot, Option<Selection>), UndoError> {
        self.preview_history(false)
    }
    fn preview_history(
        &self,
        backwards: bool,
    ) -> Result<(DocumentSnapshot, Option<Selection>), UndoError> {
        if self.policy.read_only {
            return Err(UndoError::ReadOnly);
        }
        let entry = if backwards {
            self.undo.last()
        } else {
            self.redo.last()
        }
        .ok_or(UndoError::Nothing)?;
        Ok(if backwards {
            (entry.before.snapshot(), entry.selection_before)
        } else {
            (entry.after.snapshot(), entry.selection_after)
        })
    }

    fn step_history(&mut self, backwards: bool) -> Result<UndoResult, UndoError> {
        if self.policy.read_only {
            return Err(UndoError::ReadOnly);
        }
        let (from, to) = if backwards {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        let mut entry = from.pop().ok_or(UndoError::Nothing)?;
        // History navigation is an explicit boundary: typing afterwards
        // never folds into an entry that was undone or redone.
        self.group_open = false;
        debug_assert_eq!(
            entry.version,
            self.document.version(),
            "history out of step"
        );
        let (state, selection) = if backwards {
            (&entry.before, entry.selection_before)
        } else {
            (&entry.after, entry.selection_after)
        };
        self.document.restore_content(state);
        let version = self.document.advance_version();
        // Both the moved entry and the new top of `from` now apply to the
        // state just published.
        entry.version = version;
        if let Some(next) = from.last_mut() {
            next.version = version;
        }
        let changes = entry.changes.clone();
        to.push(entry);
        Ok(UndoResult {
            version,
            changes,
            selection,
        })
    }
}

fn holds(document: &Document, precondition: &Precondition) -> bool {
    let Precondition::Text {
        story,
        paragraph,
        start,
        end,
        expected,
    } = precondition;
    let Some(paragraph) = document.find_paragraph(*story, *paragraph) else {
        return false;
    };
    if start > end || end.0 > paragraph.len() {
        return false;
    }
    let text = paragraph.plain_text(|_| "\u{FFFC}".into());
    let actual: String = text.chars().skip(start.0).take(end.0 - start.0).collect();
    actual == *expected
}

fn locate_paragraph(document: &Document, at: &Position) -> Result<Location, CommandError> {
    if document.story(at.story).is_none() {
        return Err(CommandError::UnknownStory(at.story));
    }
    let location = locate::locate(document, at.story, at.paragraph)
        .ok_or(CommandError::UnknownNode(at.paragraph))?;
    if locate::paragraph(document, &location).is_none() {
        return Err(CommandError::NotAParagraph(at.paragraph));
    }
    Ok(location)
}

fn locate_block(
    document: &Document,
    story: StoryId,
    block: NodeId,
) -> Result<Location, CommandError> {
    if document.story(story).is_none() {
        return Err(CommandError::UnknownStory(story));
    }
    locate::locate(document, story, block).ok_or(CommandError::UnknownNode(block))
}

fn check_offset(
    document: &Document,
    location: &Location,
    at: &Position,
) -> Result<usize, CommandError> {
    let len = locate::paragraph(document, location)
        .expect("located paragraph")
        .len();
    if at.offset.0 > len {
        return Err(CommandError::Text(TextError::OutOfBounds {
            offset: at.offset.0,
            len,
        }));
    }
    Ok(len)
}

fn execute(
    document: &mut Document,
    command: &Command,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    match command {
        Command::InsertText { at, text } => insert_text(document, at, text, map, changes),
        Command::InsertInline { at, kind } => insert_inline(document, at, kind, map, changes),
        Command::InsertImage { at, data } => insert_image(document, at, data, map, changes),
        Command::DeleteRange { start, end } => delete_range(document, start, end, map, changes),
        Command::SplitParagraph { at } => split_paragraph(document, at, map, changes),
        Command::FormatText { start, end, change } => format_text(
            document,
            start,
            end,
            &mut |props| change.apply(props),
            changes,
        ),
        Command::FormatParagraph {
            story,
            paragraph,
            change,
        } => format_paragraph(document, *story, *paragraph, change, changes),
        Command::MoveBlock {
            story,
            block,
            before,
        } => move_block(document, *story, *block, *before, changes),
        Command::RemoveBlock { story, block } => {
            remove_block(document, *story, *block, map, changes)
        }
        Command::ApplyStyle { start, end, style } => {
            apply_style(document, start, end, style, changes)
        }
        Command::SetNumbering {
            start,
            end,
            numbering,
        } => set_numbering(document, start, end, *numbering, None, changes),
        Command::SetNumberingLevel { start, end, level } => {
            set_numbering(document, start, end, None, Some(*level), changes)
        }
        Command::InsertTable { at, rows, columns } => {
            crate::table::insert_table(document, at, *rows, *columns, changes)
        }
        Command::InsertTableRow { at } => crate::table::insert_row(document, at, changes),
        Command::DeleteTableRow { at } => crate::table::delete_row(document, at, map, changes),
        Command::InsertTableColumn { at } => crate::table::insert_column(document, at, changes),
        Command::DeleteTableColumn { at } => {
            crate::table::delete_column(document, at, map, changes)
        }
        Command::MergeTableCells { at, direction } => {
            crate::table::merge_cells(document, at, *direction, changes)
        }
        Command::SplitTableCell { at } => crate::table::split_cell(document, at, changes),
        Command::SetPageSetup { section, page } => {
            set_page_setup(document, *section, page, changes)
        }
    }
}

fn usable_page(page: &joyword_model::PageSetup) -> bool {
    let abs =
        |measure: joyword_model::Measure| measure.absolute().filter(|length| length.raw() >= 0);
    let Some(width) = abs(page.width) else {
        return false;
    };
    let Some(height) = abs(page.height) else {
        return false;
    };
    let Some(left) = abs(page.left) else {
        return false;
    };
    let Some(right) = abs(page.right) else {
        return false;
    };
    let Some(top) = abs(page.top) else {
        return false;
    };
    let Some(bottom) = abs(page.bottom) else {
        return false;
    };
    let Some(header) = abs(page.header) else {
        return false;
    };
    let Some(footer) = abs(page.footer) else {
        return false;
    };
    let Some(gutter) = abs(page.gutter) else {
        return false;
    };
    if width.raw() <= 0 || height.raw() <= 0 {
        return false;
    }
    let Some(across) = left
        .raw()
        .checked_add(gutter.raw())
        .and_then(|left| left.checked_add(right.raw()))
    else {
        return false;
    };
    let Some(down) = top.raw().checked_add(bottom.raw()) else {
        return false;
    };
    across < width.raw()
        && down < height.raw()
        && header.raw() < height.raw()
        && footer.raw() < height.raw()
}

fn set_page_setup(
    document: &mut Document,
    section: NodeId,
    page: &joyword_model::PageSetup,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    if !usable_page(page) {
        return Err(CommandError::InvalidPage);
    }
    let changed = {
        let sections = document.sections_mut();
        let Some(found) = sections.iter_mut().find(|item| item.id == section) else {
            return Err(CommandError::UnknownNode(section));
        };
        if found.page == *page {
            false
        } else {
            found.page = page.clone();
            true
        }
    };
    if changed {
        for block in document.body().blocks.iter() {
            changes.paragraphs.insert(block.id());
        }
    }
    Ok(())
}

fn insert_text(
    document: &mut Document,
    at: &Position,
    text: &str,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    check_text(text)?;
    let location = locate_paragraph(document, at)?;
    if location.content_locked {
        return Err(CommandError::Locked(at.paragraph));
    }
    check_offset(document, &location, at)?;
    if text.is_empty() {
        return Ok(());
    }
    let paragraph = locate::paragraph_mut(document, &location).expect("located paragraph");
    paragraph.insert_text(at.offset, text)?;
    map.push(MapStep::Insert {
        paragraph: at.paragraph,
        at: at.offset.0,
        len: text.chars().count(),
    });
    changes.stories.insert(at.story);
    changes.paragraphs.insert(at.paragraph);
    Ok(())
}

fn insert_image(
    document: &mut Document,
    at: &Position,
    data: &Arc<[u8]>,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let accepted = joyword_model::accept_image(data, &joyword_model::ImageLimits::default())
        .map_err(|error| CommandError::ImageRefused(error.reason()))?;
    let (width, height) = joyword_model::display_extent(accepted.width_px, accepted.height_px);
    let resource = if let Some(existing) = document
        .resources()
        .find_same(accepted.media_type, accepted.data.as_ref())
    {
        existing
    } else {
        let id = document.ids().resource();
        document
            .resources_mut()
            .insert(id, accepted.media_type, accepted.data, None);
        id
    };
    insert_inline(
        document,
        at,
        &InlineKind::Image {
            resource,
            width,
            height,
        },
        map,
        changes,
    )
}

fn insert_inline(
    document: &mut Document,
    at: &Position,
    kind: &InlineKind,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let location = locate_paragraph(document, at)?;
    if location.content_locked {
        return Err(CommandError::Locked(at.paragraph));
    }
    check_offset(document, &location, at)?;
    if let InlineKind::Image { resource, .. } = kind
        && document.resources().get(*resource).is_none()
    {
        return Err(CommandError::MissingResource);
    }
    let id = document.ids().node();
    let paragraph = locate::paragraph_mut(document, &location).expect("located paragraph");
    let (style, props) = paragraph.format_at(at.offset);
    let object = InlineObject {
        id,
        kind: kind.clone(),
        source: None,
    };
    paragraph.insert_inline(at.offset, object, style, props)?;
    map.push(MapStep::Insert {
        paragraph: at.paragraph,
        at: at.offset.0,
        len: 1,
    });
    changes.stories.insert(at.story);
    changes.paragraphs.insert(at.paragraph);
    changes.inserted.insert(id);
    Ok(())
}

/// Locates both ends of a range in one container, in order.
fn locate_range(
    document: &Document,
    start: &Position,
    end: &Position,
) -> Result<(Location, Location), CommandError> {
    if start.story != end.story {
        return Err(CommandError::CrossContainer);
    }
    let first = locate_paragraph(document, start)?;
    let last = locate_paragraph(document, end)?;
    if !first.same_container(&last) {
        return Err(CommandError::CrossContainer);
    }
    check_offset(document, &first, start)?;
    check_offset(document, &last, end)?;
    if first.index > last.index || (first.index == last.index && start.offset > end.offset) {
        return Err(CommandError::ReversedRange);
    }
    Ok((first, last))
}

fn delete_range(
    document: &mut Document,
    start: &Position,
    end: &Position,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let (first, last) = locate_range(document, start, end)?;
    if first.content_locked {
        return Err(CommandError::Locked(start.paragraph));
    }
    if first.index == last.index {
        if start.offset == end.offset {
            return Ok(());
        }
        let paragraph = locate::paragraph_mut(document, &first).expect("located paragraph");
        paragraph.remove(start.offset..end.offset)?;
        map.push(MapStep::Delete {
            paragraph: start.paragraph,
            start: start.offset.0,
            end: end.offset.0,
        });
        changes.stories.insert(start.story);
        changes.paragraphs.insert(start.paragraph);
        return Ok(());
    }

    let story = start.story;
    let seq = locate::container(document, story, &first.containers).expect("located container");
    let mut removed_paragraphs = Vec::new();
    let mut removed_blocks = Vec::new();
    for index in first.index + 1..last.index {
        let block = seq.get(index).expect("index between located blocks");
        if let Some(locked) = locate::deletion_lock(block) {
            return Err(CommandError::Locked(locked));
        }
        locate::paragraphs_in(block, &mut removed_paragraphs);
        removed_blocks.push(block.id());
    }
    let first_len = seq
        .get(first.index)
        .and_then(Block::as_paragraph)
        .expect("located paragraph")
        .len();

    let seq = locate::container_mut(document, story, &first.containers).expect("located container");
    let Block::Paragraph(tail) = seq.remove(last.index) else {
        unreachable!("located paragraph");
    };
    for index in (first.index + 1..last.index).rev() {
        seq.remove(index);
    }
    let mut tail = Arc::unwrap_or_clone(tail);
    tail.remove(ScalarOffset(0)..end.offset)?;
    let Some(Block::Paragraph(head)) = seq.get_mut(first.index) else {
        unreachable!("located paragraph");
    };
    let head = Arc::make_mut(head);
    // Merged paragraph keeps the first paragraph's properties. Word's exact
    // rule is a black-box behavior to confirm in the T004 rule catalog.
    head.remove(start.offset..ScalarOffset(first_len))?;
    head.append(tail);

    map.push(MapStep::DeleteAcross {
        story,
        first: start.paragraph,
        start: start.offset.0,
        last: end.paragraph,
        end: end.offset.0,
        removed: removed_paragraphs.clone(),
    });
    changes.stories.insert(story);
    changes.paragraphs.insert(start.paragraph);
    changes.removed.insert(end.paragraph);
    changes.removed.extend(removed_paragraphs);
    changes.removed.extend(removed_blocks);
    Ok(())
}

fn split_paragraph(
    document: &mut Document,
    at: &Position,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let location = locate_paragraph(document, at)?;
    if location.content_locked {
        return Err(CommandError::Locked(at.paragraph));
    }
    check_offset(document, &location, at)?;
    let new = document.ids().node();
    let seq =
        locate::container_mut(document, at.story, &location.containers).expect("located container");
    let Some(Block::Paragraph(paragraph)) = seq.get_mut(location.index) else {
        unreachable!("located paragraph");
    };
    let tail = Arc::make_mut(paragraph).split_off(at.offset, new)?;
    seq.insert(location.index + 1, tail.into());
    map.push(MapStep::Split {
        paragraph: at.paragraph,
        at: at.offset.0,
        new,
    });
    changes.stories.insert(at.story);
    changes.paragraphs.insert(at.paragraph);
    changes.inserted.insert(new);
    Ok(())
}

fn format_text(
    document: &mut Document,
    start: &Position,
    end: &Position,
    change: &mut dyn FnMut(&mut RunProps),
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let (first, last) = locate_range(document, start, end)?;
    if first.content_locked {
        return Err(CommandError::Locked(start.paragraph));
    }
    let story = start.story;
    let seq = locate::container_mut(document, story, &first.containers).expect("located container");
    let mut touched = Vec::new();
    for index in first.index..=last.index {
        locate::for_each_paragraph_mut(seq, index, &mut |paragraph, locked| {
            let id = paragraph.id();
            let from = if id == start.paragraph {
                start.offset
            } else {
                ScalarOffset(0)
            };
            let to = if id == end.paragraph {
                end.offset
            } else {
                ScalarOffset(paragraph.len())
            };
            if from == to {
                return Ok(());
            }
            if locked {
                return Err(CommandError::Locked(id));
            }
            if paragraph.format_runs(from..to, &mut *change)? {
                touched.push(id);
            }
            Ok(())
        })?;
    }
    if !touched.is_empty() {
        changes.stories.insert(story);
        changes.paragraphs.extend(touched);
    }
    Ok(())
}

fn apply_style(
    document: &mut Document,
    start: &Position,
    end: &Position,
    style: &StyleId,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let (first, last) = locate_range(document, start, end)?;
    let found = document
        .styles()
        .get(style)
        .cloned()
        .ok_or_else(|| CommandError::UnknownStyle(style.clone()))?;
    if matches!(found.kind, StyleKind::Table | StyleKind::Numbering) {
        return Err(CommandError::UnresolvedStyle(style.clone()));
    }
    let chain = document.styles().chain(style);
    if matches!(
        chain.problem,
        Some(StyleProblem::Cycle(_) | StyleProblem::TooDeep | StyleProblem::KindMismatch { .. })
    ) {
        return Err(CommandError::StyleChain(style.clone()));
    }
    let coverage = selection_coverage(document, start, end, &last)?;
    let (paragraph_style, character_style) = match (found.kind, coverage) {
        (StyleKind::Paragraph, Coverage::Caret | Coverage::Whole) => (Some(style.clone()), None),
        (StyleKind::Paragraph, Coverage::Partial) => {
            let linked = linked_character(document, &found)?;
            (None, Some(linked))
        }
        (StyleKind::Character, Coverage::Caret) => return Ok(()),
        (StyleKind::Character, Coverage::Whole) => {
            if let Some(linked) = &found.link {
                let target = document
                    .styles()
                    .get(linked)
                    .ok_or_else(|| CommandError::UnknownStyle(linked.clone()))?;
                if target.kind != StyleKind::Paragraph {
                    return Err(CommandError::StyleTarget(style.clone()));
                }
                (Some(linked.clone()), None)
            } else {
                (None, Some(style.clone()))
            }
        }
        (StyleKind::Character, Coverage::Partial) => (None, Some(style.clone())),
        (StyleKind::Table | StyleKind::Numbering, _) => {
            return Err(CommandError::UnresolvedStyle(style.clone()));
        }
    };
    let story = start.story;
    let Some(seq) = locate::container_mut(document, story, &first.containers) else {
        return Err(CommandError::UnknownNode(start.paragraph));
    };
    let mut touched = Vec::new();
    for index in first.index..=last.index {
        locate::for_each_paragraph_mut(seq, index, &mut |paragraph, locked| {
            let id = paragraph.id();
            if locked {
                return Err(CommandError::Locked(id));
            }
            let mut changed = false;
            if let Some(style) = &paragraph_style
                && paragraph.style.as_ref() != Some(style)
            {
                paragraph.style = Some(style.clone());
                changed = true;
            }
            if let Some(style) = &character_style {
                let from = if id == start.paragraph {
                    start.offset
                } else {
                    ScalarOffset(0)
                };
                let to = if id == end.paragraph {
                    end.offset
                } else {
                    ScalarOffset(paragraph.len())
                };
                if from != to && paragraph.restyle_runs(from..to, Some(style.clone()))? {
                    changed = true;
                }
            }
            if changed {
                touched.push(id);
            }
            Ok(())
        })?;
    }
    if !touched.is_empty() {
        changes.stories.insert(story);
        changes.paragraphs.extend(touched);
    }
    Ok(())
}

fn set_numbering(
    document: &mut Document,
    start: &Position,
    end: &Position,
    numbering: Option<NumberingRef>,
    level_only: Option<u8>,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let (first, last) = locate_range(document, start, end)?;
    if let Some(level) = level_only
        && level > MAX_LEVEL
    {
        return Err(CommandError::NumberingLevel(level));
    }
    let assigned = match level_only {
        None => Some(assigned_numbering(document, numbering)?),
        Some(_) => None,
    };
    let instance_ids: Vec<u32> = document
        .numbering()
        .instances()
        .iter()
        .map(|item| item.id)
        .collect();
    let style_ids: Vec<StyleId> = document
        .styles()
        .iter()
        .map(|style| style.id.clone())
        .collect();
    let mut style_numbers = Vec::new();
    for id in &style_ids {
        let resolved = document
            .styles()
            .resolve_paragraph(Some(id), &ParaProps::default());
        if let Some(item) = resolved.props.numbering.filter(|item| item.num_id != 0) {
            style_numbers.push((id.clone(), item.num_id));
        }
    }
    let default_number = document
        .styles()
        .resolve_paragraph(None, &ParaProps::default())
        .props
        .numbering
        .filter(|item| item.num_id != 0)
        .map(|item| item.num_id);
    let story = start.story;
    let Some(seq) = locate::container_mut(document, story, &first.containers) else {
        return Err(CommandError::UnknownNode(start.paragraph));
    };
    let mut touched = Vec::new();
    for index in first.index..=last.index {
        locate::for_each_paragraph_mut(seq, index, &mut |paragraph, locked| {
            let id = paragraph.id();
            if locked {
                return Err(CommandError::Locked(id));
            }
            let written = if let Some(level) = level_only {
                let num_id = if let Some(direct) = paragraph.props.numbering {
                    direct.num_id
                } else if let Some(style) = &paragraph.style {
                    style_numbers
                        .iter()
                        .find(|(candidate, _)| candidate == style)
                        .map(|(_, num_id)| *num_id)
                        .ok_or(CommandError::NoNumbering(id))?
                } else {
                    default_number.ok_or(CommandError::NoNumbering(id))?
                };
                if num_id != 0 && !instance_ids.contains(&num_id) {
                    return Err(CommandError::UnknownNumbering(num_id));
                }
                NumberingRef { num_id, level }
            } else if let Some(written) = assigned {
                written
            } else {
                return Err(CommandError::NumberingLevel(0));
            };
            if paragraph.props.numbering == Some(written) {
                return Ok(());
            }
            let mut props = (*paragraph.props).clone();
            props.numbering = Some(written);
            paragraph.props = Arc::new(props);
            touched.push(id);
            Ok(())
        })?;
    }
    if !touched.is_empty() {
        changes.stories.insert(story);
        changes.paragraphs.extend(touched);
    }
    Ok(())
}

fn assigned_numbering(
    document: &Document,
    numbering: Option<NumberingRef>,
) -> Result<NumberingRef, CommandError> {
    match numbering {
        None => Ok(NumberingRef {
            num_id: 0,
            level: 0,
        }),
        Some(item) if item.num_id == 0 => Ok(NumberingRef {
            num_id: 0,
            level: 0,
        }),
        Some(item) => {
            if item.level > MAX_LEVEL {
                return Err(CommandError::NumberingLevel(item.level));
            }
            if document.numbering().instance(item.num_id).is_none() {
                return Err(CommandError::UnknownNumbering(item.num_id));
            }
            Ok(item)
        }
    }
}

fn linked_character(
    document: &Document,
    style: &joyword_model::Style,
) -> Result<StyleId, CommandError> {
    let Some(link) = style.link.clone() else {
        return Err(CommandError::StyleTarget(style.id.clone()));
    };
    let target = document
        .styles()
        .get(&link)
        .ok_or_else(|| CommandError::UnknownStyle(link.clone()))?;
    if target.kind != StyleKind::Character {
        return Err(CommandError::StyleTarget(style.id.clone()));
    }
    Ok(link)
}

enum Coverage {
    /// An empty selection. A paragraph style still applies to that paragraph.
    Caret,
    /// Every paragraph in the span is covered from its first position through
    /// its last.
    Whole,
    Partial,
}

fn selection_coverage(
    document: &Document,
    start: &Position,
    end: &Position,
    last: &locate::Location,
) -> Result<Coverage, CommandError> {
    if start.offset == end.offset && start.paragraph == end.paragraph {
        return Ok(Coverage::Caret);
    }
    let end_len = check_offset(document, last, end)?;
    if start.offset.0 == 0 && end.offset.0 == end_len {
        Ok(Coverage::Whole)
    } else {
        Ok(Coverage::Partial)
    }
}

fn format_paragraph(
    document: &mut Document,
    story: StoryId,
    paragraph: NodeId,
    change: &ParagraphChange,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let at = Position {
        story,
        paragraph,
        offset: ScalarOffset(0),
        bias: Bias::Before,
    };
    let location = locate_paragraph(document, &at)?;
    if location.content_locked {
        return Err(CommandError::Locked(paragraph));
    }
    let target = locate::paragraph_mut(document, &location).expect("located paragraph");
    let changed = match change {
        ParagraphChange::Style(style) => {
            let changed = target.style != *style;
            target.style = style.clone();
            changed
        }
        other => {
            let mut props = ParaProps::clone(&target.props);
            match other {
                ParagraphChange::Justification(value) => props.justification = *value,
                ParagraphChange::Indent(value) => props.indent = *value,
                ParagraphChange::Spacing(value) => props.spacing = *value,
                ParagraphChange::Style(_) => unreachable!("handled above"),
            }
            let changed = props != *target.props;
            if changed {
                target.props = Arc::new(props);
            }
            changed
        }
    };
    if changed {
        changes.stories.insert(story);
        changes.paragraphs.insert(paragraph);
    }
    Ok(())
}

fn move_block(
    document: &mut Document,
    story: StoryId,
    block: NodeId,
    before: Option<NodeId>,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let location = locate_block(document, story, block)?;
    if location.content_locked {
        return Err(CommandError::Locked(block));
    }
    let target = match before {
        Some(before) => {
            let target = locate_block(document, story, before)?;
            if !target.same_container(&location) {
                return Err(CommandError::CrossContainer);
            }
            target.index
        }
        None => locate::container(document, story, &location.containers)
            .expect("located container")
            .len(),
    };
    if target == location.index || target == location.index + 1 {
        return Ok(());
    }
    let seq =
        locate::container_mut(document, story, &location.containers).expect("located container");
    let moving = seq.remove(location.index);
    let index = if target > location.index {
        target - 1
    } else {
        target
    };
    seq.insert(index, moving);
    changes.stories.insert(story);
    changes.moved.insert(block);
    Ok(())
}

fn remove_block(
    document: &mut Document,
    story: StoryId,
    block: NodeId,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let location = locate_block(document, story, block)?;
    if location.content_locked {
        return Err(CommandError::Locked(block));
    }
    let seq = locate::container(document, story, &location.containers).expect("located container");
    if seq.len() == 1 {
        return Err(CommandError::LastBlock(block));
    }
    let removed = seq.get(location.index).expect("located block");
    if let Some(locked) = locate::deletion_lock(removed) {
        return Err(CommandError::Locked(locked));
    }
    let mut paragraphs = Vec::new();
    locate::paragraphs_in(removed, &mut paragraphs);
    // Positions inside collapse to the start of the next block, else the end
    // of the previous one.
    let fallback = seq
        .get(location.index + 1)
        .and_then(|next| locate::edge_paragraph(next, true))
        .or_else(|| {
            location
                .index
                .checked_sub(1)
                .and_then(|previous| seq.get(previous))
                .and_then(|previous| locate::edge_paragraph(previous, false))
        })
        .map(|(paragraph, offset)| (story, paragraph, offset));
    locate::container_mut(document, story, &location.containers)
        .expect("located container")
        .remove(location.index);
    for &paragraph in &paragraphs {
        map.push(MapStep::Remove {
            paragraph,
            fallback,
        });
    }
    changes.stories.insert(story);
    changes.removed.insert(block);
    changes.removed.extend(paragraphs);
    Ok(())
}

/// Maps every anchored annotation through one command's steps. A comment
/// whose non-empty range is deleted becomes visibly detached (REVIEW-006);
/// other annotations collapse to an empty range. Lost anchors detach.
fn remap_annotations(document: &mut Document, map: &PositionMap, changes: &mut ChangeSet) {
    if map.is_identity() {
        return;
    }
    let mut updates = Vec::new();
    for annotation in document.annotations() {
        let Anchor::Range { start, end } = annotation.anchor else {
            continue;
        };
        let mapped_start = map.map(start).position();
        let mapped_end = map.map(end).position();
        let mapped_kind = match &annotation.kind {
            AnnotationKind::Field {
                separator: Some(separator),
            } => AnnotationKind::Field {
                separator: map.map(*separator).position(),
            },
            other => other.clone(),
        };
        let anchor = match (mapped_start, mapped_end) {
            (Some(new_start), Some(new_end)) => {
                let emptied = start != end
                    && new_start.paragraph == new_end.paragraph
                    && new_start.offset == new_end.offset;
                if emptied && matches!(annotation.kind, AnnotationKind::Comment { .. }) {
                    Anchor::Detached
                } else {
                    Anchor::Range {
                        start: new_start,
                        end: new_end,
                    }
                }
            }
            _ => Anchor::Detached,
        };
        if anchor != annotation.anchor || mapped_kind != annotation.kind {
            updates.push((annotation.id, anchor, mapped_kind));
        }
    }
    for (id, anchor, kind) in updates {
        let annotation = document.annotation_mut(id).expect("annotation exists");
        annotation.anchor = anchor;
        annotation.kind = kind;
        changes.annotations.insert(id);
    }
}

/// Invariants of every surviving paragraph the transaction touched, found in
/// one walk per touched story.
fn touched_issues(document: &Document, changes: &ChangeSet) -> Vec<Issue> {
    fn walk(block: &Block, changes: &ChangeSet, issues: &mut Vec<Issue>, depth: usize) {
        if depth > MAX_NESTING {
            return;
        }
        match block {
            Block::Paragraph(paragraph) => check(paragraph, changes, issues),
            Block::Table(table) => {
                if changes.tables.contains(&table.id) {
                    issues.extend(joyword_model::table_structure(table));
                }
                for cell in table.rows.iter().flat_map(|row| row.cells.iter()) {
                    if cell.blocks.is_empty() && !changes.tables.contains(&table.id) {
                        issues.push(Issue::EmptyCell { cell: cell.id });
                    }
                    for inner in cell.blocks.iter() {
                        walk(inner, changes, issues, depth + 1);
                    }
                }
            }
            Block::Sdt(sdt) => {
                for inner in sdt.blocks.iter() {
                    walk(inner, changes, issues, depth + 1);
                }
            }
            Block::Opaque(_) => {}
        }
    }

    fn check(paragraph: &Paragraph, changes: &ChangeSet, issues: &mut Vec<Issue>) {
        let id = paragraph.id();
        if changes.paragraphs.contains(&id) || changes.inserted.contains(&id) {
            issues.extend(paragraph.invariant_issues());
        }
    }

    let mut issues = Vec::new();
    for &story in &changes.stories {
        let Some(found) = document.story(story) else {
            continue;
        };
        if found.blocks.is_empty() {
            issues.push(Issue::EmptyStory { story });
        }
        for block in found.blocks.iter() {
            walk(block, changes, &mut issues, 0);
        }
    }
    issues
}

#[cfg(test)]
mod panic_guard {
    use super::*;
    use joyword_model::{BlockSeq, PageSetup};
    use std::sync::Arc;

    #[test]
    fn a_panic_inside_apply_discards_the_candidate() {
        let mut document = Document::new(PageSetup::a4_zh_cn());
        let mut paragraph = Paragraph::new(document.ids().node());
        paragraph.push_text("甲", None, Arc::default()).unwrap();
        let body = document.body_id();
        document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
        let mut session = Session::new(document);
        let version = session.version();
        let at = {
            let paragraph = session.document().body().blocks.get(0).unwrap().id();
            Position {
                story: session.document().body_id(),
                paragraph,
                offset: ScalarOffset(0),
                bias: Bias::After,
            }
        };
        fail_next_apply();
        let error = session
            .apply(Transaction::new(
                version,
                vec![Command::InsertText {
                    at,
                    text: "乙".into(),
                }],
            ))
            .unwrap_err();
        assert!(matches!(error, TxError::Interrupted));
        let text = session
            .document()
            .body()
            .blocks
            .get(0)
            .unwrap()
            .as_paragraph()
            .unwrap()
            .plain_text(|_| String::new());
        assert_eq!(text, "甲");
        assert_eq!(session.version(), version);
        assert!(!session.can_undo());
    }
}
