//! T402 accept slice: re-validate the grant, then one undoable commit.
//! The in-memory history is the refusal double. The engine adapter is the
//! transaction the host is expected to perform.

use std::cell::{Cell, RefCell};
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use joyword_agent::{
    AI_REVISION_AUTHOR, AdminPolicy, AgentRuntime, AiConfig, ApplyBlocked, ChangeProposal,
    CommitError, DocumentCommit, DocumentId, DocumentPort, DocumentSnapshot, FakeProvider,
    GateError, Grant, MemoryCredentialStore, MemoryParagraph, PortError, ReplaceError,
    ReplaceProposal, ReplaceReceipt, ReplaceRequest, ScopeError, ScopeRange, SnapshotParagraph,
    TickClock, ToolAllowlist, ToolError, ToolId, ToolSession,
};
use joyword_engine::{
    AI_REVISION_AUTHOR as ENGINE_AUTHOR, Command, EditPolicy, Session, Transaction, TxError,
    UndoError, apply_ai_text_replace, body_final_paragraphs,
};
use joyword_model::{
    Bias, Block, BlockSeq, Document, Paragraph, Position, RevisionKind, ScalarOffset,
};

type TestResult = Result<(), Box<dyn Error>>;

const DOC_ID: &str = "CONFIDENTIAL-DOCUMENT-ID";
const BEFORE: &str = "CONFIDENTIAL-BEFORE-中";
const AFTER: &str = "CONFIDENTIAL-AFTER-文";
const REASON: &str = "CONFIDENTIAL-EXPLANATION";
const ID: &str = "CONFIDENTIAL-PROPOSAL-ID";

fn open_runtime() -> Result<AgentRuntime, Box<dyn Error>> {
    let mut config = AiConfig::default();
    config.user_enabled = true;
    config.provider = joyword_agent::ProviderSelection::named("fake")?;
    Ok(AgentRuntime::new(
        config,
        AdminPolicy::unmanaged(),
        Box::new(FakeProvider::bomb()),
        Box::new(MemoryCredentialStore::new()),
    ))
}

fn secrets() -> [&'static str; 4] {
    [DOC_ID, BEFORE, AFTER, REASON]
}

struct History {
    id: DocumentId,
    inner: Mutex<HistoryInner>,
    commits: AtomicUsize,
}

struct HistoryInner {
    paragraphs: Vec<MemoryParagraph>,
    version: u64,
    undo: Vec<Vec<MemoryParagraph>>,
    author: Option<&'static str>,
}

impl History {
    fn new(paragraphs: Vec<MemoryParagraph>) -> Result<Self, Box<dyn Error>> {
        let mut inner = HistoryInner {
            paragraphs: Vec::new(),
            version: 1,
            undo: Vec::new(),
            author: None,
        };
        for paragraph in paragraphs {
            inner.paragraphs.push(paragraph);
            inner.version += 1;
        }
        Ok(Self {
            id: DocumentId::parse(DOC_ID).map_err(|_| "invalid document id")?,
            inner: Mutex::new(inner),
            commits: AtomicUsize::new(0),
        })
    }

    fn commits(&self) -> usize {
        self.commits.load(Ordering::SeqCst)
    }

    fn text(&self) -> Result<String, CommitError> {
        Ok(self.snapshot_inner()?.text().to_owned())
    }

    fn hidden(&self) -> Result<(Vec<String>, Vec<String>), CommitError> {
        let inner = self.inner.lock().map_err(|_| CommitError::Poisoned)?;
        let mut deleted = Vec::new();
        let mut comments = Vec::new();
        for paragraph in &inner.paragraphs {
            deleted.extend(paragraph.deleted.iter().cloned());
            comments.extend(paragraph.comments.iter().cloned());
        }
        Ok((deleted, comments))
    }

    fn author(&self) -> Result<Option<&'static str>, CommitError> {
        Ok(self.inner.lock().map_err(|_| CommitError::Poisoned)?.author)
    }

    fn edit(&self, index: usize, visible: &str) -> Result<(), CommitError> {
        let mut inner = self.inner.lock().map_err(|_| CommitError::Poisoned)?;
        let paragraph = inner
            .paragraphs
            .get_mut(index)
            .ok_or(CommitError::Rejected)?;
        paragraph.visible = visible.to_owned();
        inner.version += 1;
        Ok(())
    }

    /// Same version, different bytes: the fingerprint check must refuse.
    fn corrupt_visible(&self, index: usize, visible: &str) -> Result<(), CommitError> {
        let mut inner = self.inner.lock().map_err(|_| CommitError::Poisoned)?;
        let paragraph = inner
            .paragraphs
            .get_mut(index)
            .ok_or(CommitError::Rejected)?;
        paragraph.visible = visible.to_owned();
        Ok(())
    }

    fn snapshot_inner(&self) -> Result<DocumentSnapshot, CommitError> {
        let inner = self.inner.lock().map_err(|_| CommitError::Poisoned)?;
        DocumentSnapshot::new(
            self.id.clone(),
            inner.version,
            inner
                .paragraphs
                .iter()
                .map(|paragraph| SnapshotParagraph {
                    heading_level: paragraph.heading_level,
                    visible: paragraph.visible.clone(),
                })
                .collect(),
        )
        .map_err(|_| CommitError::Rejected)
    }
}

impl DocumentPort for History {
    fn snapshot(&self, document: &DocumentId) -> Result<DocumentSnapshot, PortError> {
        if document != &self.id {
            return Err(PortError::UnknownDocument);
        }
        self.snapshot_inner().map_err(|_| PortError::Inaccessible)
    }
}

impl DocumentCommit for History {
    fn commit_replace(
        &self,
        change: &joyword_agent::ReplaceCommit<'_>,
        authorize: &mut dyn FnMut() -> Result<(), ToolError>,
    ) -> Result<ReplaceReceipt, CommitError> {
        authorize().map_err(|_| CommitError::Unauthorized)?;
        if change.document_id() != &self.id {
            return Err(CommitError::Rejected);
        }
        if !text_allowed(change.replacement()) {
            return Err(CommitError::InvalidReplacement);
        }
        let mut inner = self.inner.lock().map_err(|_| CommitError::Poisoned)?;
        if inner.version != change.base_version() {
            return Err(CommitError::Stale);
        }
        let snapshot = DocumentSnapshot::new(
            self.id.clone(),
            inner.version,
            inner
                .paragraphs
                .iter()
                .map(|paragraph| SnapshotParagraph {
                    heading_level: paragraph.heading_level,
                    visible: paragraph.visible.clone(),
                })
                .collect(),
        )
        .map_err(|_| CommitError::Rejected)?;
        if change.range().end() > snapshot.char_len() {
            return Err(CommitError::RangeRefused);
        }
        let actual = snapshot
            .slice_chars(change.range())
            .ok_or(CommitError::RangeRefused)?;
        if actual != change.expected() {
            return Err(CommitError::Mismatch);
        }
        // The replacement may cover paragraph separators. Splice the whole
        // final-view text, then split it back into paragraphs: the covered
        // group collapses into one paragraph that keeps the first covered
        // paragraph's heading, mirroring the engine's merge rule.
        let joined = inner
            .paragraphs
            .iter()
            .map(|paragraph| paragraph.visible.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let start = usize::try_from(change.range().start()).map_err(|_| CommitError::Rejected)?;
        let end = usize::try_from(change.range().end()).map_err(|_| CommitError::Rejected)?;
        let spliced =
            splice(&joined, start, end, change.replacement()).ok_or(CommitError::RangeRefused)?;
        if change.range().is_empty() {
            return Err(CommitError::RangeRefused);
        }
        let outline = snapshot.outline();
        // The collapsed group starts at the paragraph holding the range
        // start (a start on a separator is that paragraph's mark) and ends
        // at the paragraph holding the last covered character; a last
        // character on a separator belongs to the paragraph before it.
        let Some(first_covered) = outline.iter().position(|node| {
            node.start <= change.range().start() && change.range().start() <= node.end
        }) else {
            return Err(CommitError::RangeRefused);
        };
        let last_char = change.range().end() - 1;
        let last_covered = outline
            .iter()
            .position(|node| node.start <= last_char && last_char < node.end)
            .or_else(|| outline.iter().rposition(|node| node.end == last_char))
            .ok_or(CommitError::RangeRefused)?;
        if last_covered < first_covered {
            return Err(CommitError::RangeRefused);
        }
        let collapsed = last_covered - first_covered;
        let mut next = Vec::with_capacity(inner.paragraphs.len() - collapsed);
        for (slot, part) in spliced.split('\n').enumerate() {
            // Slots before and at the collapsed group map one to one; the
            // ones after it skip the paragraphs the group absorbed. The
            // collapsed slot aggregates the covered paragraphs' hidden
            // content, the way merged engine paragraphs keep their runs.
            let mut paragraph = if slot != first_covered {
                let index = if slot <= first_covered {
                    slot
                } else {
                    slot + collapsed
                };
                inner
                    .paragraphs
                    .get(index)
                    .ok_or(CommitError::Rejected)?
                    .clone()
            } else {
                let mut merged = inner
                    .paragraphs
                    .get(first_covered)
                    .ok_or(CommitError::Rejected)?
                    .clone();
                for extra in inner
                    .paragraphs
                    .get(first_covered + 1..=last_covered)
                    .unwrap_or(&[])
                {
                    merged.deleted.extend(extra.deleted.iter().cloned());
                    merged.comments.extend(extra.comments.iter().cloned());
                }
                merged
            };
            if part.len() > joyword_agent::MAX_PARAGRAPH_BYTES || inner.version == u64::MAX {
                return Err(CommitError::Rejected);
            }
            paragraph.visible = part.to_owned();
            next.push(paragraph);
        }
        let saved = inner.paragraphs.clone();
        inner.undo.push(saved);
        inner.paragraphs = next;
        inner.version += 1;
        inner.author = Some(AI_REVISION_AUTHOR);
        let version = inner.version;
        let undo_depth = u64::try_from(inner.undo.len()).map_err(|_| CommitError::Rejected)?;
        drop(inner);
        self.commits.fetch_add(1, Ordering::SeqCst);
        Ok(ReplaceReceipt::new(version, undo_depth))
    }

    fn undo_replace(&self) -> Result<(), CommitError> {
        let mut inner = self.inner.lock().map_err(|_| CommitError::Poisoned)?;
        let previous = inner.undo.pop().ok_or(CommitError::NothingToUndo)?;
        if inner.version == u64::MAX {
            inner.undo.push(previous);
            return Err(CommitError::Rejected);
        }
        inner.paragraphs = previous;
        inner.version += 1;
        inner.author = None;
        Ok(())
    }
}

fn text_allowed(text: &str) -> bool {
    !text.chars().any(|ch| {
        ch == '\u{FDD0}'
            || ch == '\u{FFFC}'
            || ch == '\u{2028}'
            || ch == '\u{2029}'
            || (ch.is_control() && ch != '\t')
    })
}

fn splice(text: &str, start: usize, end: usize, replacement: &str) -> Option<String> {
    if start > end {
        return None;
    }
    let mut out = String::new();
    let mut index = 0usize;
    for ch in text.chars() {
        if index == start {
            out.push_str(replacement);
        }
        if index < start || index >= end {
            out.push(ch);
        }
        index += 1;
    }
    if index < end {
        return None;
    }
    if start == index {
        out.push_str(replacement);
    }
    Some(out)
}

fn selected(text: &str) -> MemoryParagraph {
    let mut paragraph = MemoryParagraph::body(text);
    paragraph.deleted.push("HIDDEN-DELETED".to_owned());
    paragraph.comments.push("HIDDEN-COMMENT".to_owned());
    paragraph
}

fn history(text: &str) -> Result<History, Box<dyn Error>> {
    History::new(vec![
        MemoryParagraph::body("OUTSIDE-LEFT"),
        selected(text),
        MemoryParagraph::body("OUTSIDE-RIGHT"),
    ])
}

fn paragraph_range(doc: &History, index: usize) -> Result<ScopeRange, Box<dyn Error>> {
    let snapshot = doc.snapshot(&doc.id)?;
    let node = snapshot.outline().get(index).ok_or("missing paragraph")?;
    Ok(ScopeRange::new(node.start, node.end)?)
}

fn grant_for(
    runtime: &AgentRuntime,
    doc: &History,
    range: ScopeRange,
    clock: &TickClock,
) -> Result<Grant, Box<dyn Error>> {
    Ok(Grant::issue(
        doc,
        &doc.id,
        range,
        ToolAllowlist::from_slice(&[ToolId::ProposeReplace]),
        runtime.generation()?,
        clock,
        100,
    )?)
}

fn request<'a>(id: &'a str, replacement: &'a str) -> ReplaceRequest<'a> {
    ReplaceRequest {
        id,
        replacement,
        explanation: REASON,
    }
}

fn propose(
    runtime: &AgentRuntime,
    doc: &History,
    clock: &TickClock,
    id: &str,
    replacement: &str,
) -> Result<(Grant, ReplaceProposal), Box<dyn Error>> {
    let grant = grant_for(runtime, doc, paragraph_range(doc, 1)?, clock)?;
    let proposal = ToolSession::new(runtime, doc, grant.clone(), clock)
        .propose_replace(request(id, replacement))?;
    Ok((grant, proposal))
}

struct EngineDoc {
    id: DocumentId,
    session: RefCell<Session>,
    commits: Cell<usize>,
}

impl EngineDoc {
    fn new(lines: &[&str]) -> Result<Self, Box<dyn Error>> {
        let mut document = Document::new_blank();
        let mut blocks = Vec::new();
        for line in lines {
            let mut paragraph = Paragraph::new(document.ids().node());
            paragraph
                .push_text(line, None, Arc::default())
                .map_err(|_| "fixture text")?;
            blocks.push(Block::from(paragraph));
        }
        let body = document.body_id();
        document.story_mut(body).ok_or("missing body")?.blocks = BlockSeq::from_vec(blocks);
        let mut session = Session::new(document);
        session.set_policy(EditPolicy {
            read_only: false,
            agent_writes: true,
        });
        session.set_revision_author("用户");
        Ok(Self {
            id: DocumentId::parse(DOC_ID).map_err(|_| "invalid document id")?,
            session: RefCell::new(session),
            commits: Cell::new(0),
        })
    }

    fn commits(&self) -> usize {
        self.commits.get()
    }

    fn final_text(&self) -> Vec<String> {
        body_final_paragraphs(self.session.borrow().document()).unwrap()
    }

    fn undo_depth(&self) -> usize {
        self.session.borrow().undo_depth()
    }

    fn authors(&self) -> Vec<(RevisionKind, String)> {
        self.session
            .borrow()
            .document()
            .body()
            .blocks
            .iter()
            .filter_map(Block::as_paragraph)
            .flat_map(|paragraph| paragraph.runs().iter())
            .filter_map(|run| {
                run.revision
                    .as_ref()
                    .map(|revision| (revision.kind, revision.author.to_string()))
            })
            .collect()
    }

    fn user_insert_first(&self, text: &str) -> Result<(), TxError> {
        let mut session = self.session.borrow_mut();
        let id = session
            .document()
            .body()
            .blocks
            .iter()
            .next()
            .map(Block::id)
            .ok_or(TxError::AiRangeRefused)?;
        let len = session
            .document()
            .body()
            .blocks
            .iter()
            .next()
            .and_then(Block::as_paragraph)
            .map(|paragraph| paragraph.len())
            .ok_or(TxError::AiRangeRefused)?;
        let version = session.version();
        let story = session.document().body_id();
        session.apply(Transaction::new(
            version,
            vec![Command::InsertText {
                at: Position {
                    story,
                    paragraph: id,
                    offset: ScalarOffset(len),
                    bias: Bias::After,
                },
                text: text.to_owned(),
            }],
        ))?;
        Ok(())
    }
}

impl DocumentPort for EngineDoc {
    fn snapshot(&self, document: &DocumentId) -> Result<DocumentSnapshot, PortError> {
        if document != &self.id {
            return Err(PortError::UnknownDocument);
        }
        let session = self.session.borrow();
        let paragraphs =
            body_final_paragraphs(session.document()).map_err(|_| PortError::Inaccessible)?;
        DocumentSnapshot::new(
            self.id.clone(),
            session.version().get(),
            paragraphs
                .into_iter()
                .map(|visible| SnapshotParagraph {
                    heading_level: 0,
                    visible,
                })
                .collect(),
        )
        .map_err(|_| PortError::Inaccessible)
    }
}

impl DocumentCommit for EngineDoc {
    fn commit_replace(
        &self,
        change: &joyword_agent::ReplaceCommit<'_>,
        authorize: &mut dyn FnMut() -> Result<(), ToolError>,
    ) -> Result<ReplaceReceipt, CommitError> {
        authorize().map_err(|_| CommitError::Unauthorized)?;
        if change.document_id() != &self.id {
            return Err(CommitError::Rejected);
        }
        let mut session = self.session.borrow_mut();
        if session.version().get() != change.base_version() {
            return Err(CommitError::Stale);
        }
        let paragraphs =
            body_final_paragraphs(session.document()).map_err(|_| CommitError::RangeRefused)?;
        let snapshot = DocumentSnapshot::new(
            self.id.clone(),
            session.version().get(),
            paragraphs
                .iter()
                .map(|visible| SnapshotParagraph {
                    heading_level: 0,
                    visible: visible.clone(),
                })
                .collect(),
        )
        .map_err(|_| CommitError::Rejected)?;
        let actual = snapshot
            .slice_chars(change.range())
            .ok_or(CommitError::RangeRefused)?;
        if actual != change.expected() {
            return Err(CommitError::Mismatch);
        }
        let depth = session.undo_depth();
        let version = session.version();
        apply_ai_text_replace(
            &mut session,
            version,
            change.range().start(),
            change.range().end(),
            change.replacement(),
        )
        .map_err(map_tx)?;
        if session.undo_depth() != depth + 1 {
            return Err(CommitError::Rejected);
        }
        let version = session.version().get();
        let undo_depth = u64::try_from(session.undo_depth()).map_err(|_| CommitError::Rejected)?;
        drop(session);
        self.commits.set(self.commits.get() + 1);
        Ok(ReplaceReceipt::new(version, undo_depth))
    }

    fn undo_replace(&self) -> Result<(), CommitError> {
        self.session
            .borrow_mut()
            .undo()
            .map_err(|error| match error {
                UndoError::Nothing => CommitError::NothingToUndo,
                UndoError::ReadOnly => CommitError::Rejected,
            })?;
        Ok(())
    }
}

fn map_tx(error: TxError) -> CommitError {
    match error {
        TxError::StaleVersion { .. } => CommitError::Stale,
        TxError::AiRangeRefused => CommitError::RangeRefused,
        TxError::AiReplacementRefused => CommitError::InvalidReplacement,
        TxError::ReadOnly
        | TxError::AgentWritesDisabled
        | TxError::PreconditionFailed { .. }
        | TxError::Command { .. }
        | TxError::Invariant(_)
        | TxError::Interrupted => CommitError::Rejected,
    }
}

fn quiet(value: &str) {
    for secret in secrets() {
        assert!(!value.contains(secret), "{value}");
    }
    assert!(!value.contains(ID));
}

#[test]
fn accept_on_the_engine_is_one_ai_authored_undo_step() -> TestResult {
    assert_eq!(AI_REVISION_AUTHOR, ENGINE_AUTHOR);
    let runtime = open_runtime()?;
    let doc = EngineDoc::new(&["OUTSIDE-LEFT", BEFORE, "OUTSIDE-RIGHT"])?;
    let clock = TickClock::new();
    let grant = Grant::issue(
        &doc,
        &doc.id,
        {
            let snapshot = doc.snapshot(&doc.id)?;
            let node = snapshot.outline().get(1).ok_or("paragraph")?;
            ScopeRange::new(node.start, node.end)?
        },
        ToolAllowlist::from_slice(&[ToolId::ProposeReplace]),
        runtime.generation()?,
        &clock,
        100,
    )?;
    let session = ToolSession::new(&runtime, &doc, grant, &clock);
    let proposal = session.propose_replace(request(ID, AFTER))?;
    let before = doc.final_text();
    assert_eq!(doc.undo_depth(), 0);
    assert_eq!(doc.commits(), 0);
    for _ in 0..3 {
        let preview = session.preview_replace(&proposal)?;
        assert_eq!(preview.before(), BEFORE);
        assert_eq!(preview.after(), AFTER);
    }
    assert_eq!(doc.final_text(), before);
    assert_eq!(doc.undo_depth(), 0);
    assert_eq!(runtime.provider_calls(), 0);
    assert_eq!(runtime.applied_replacements(), 0);
    assert_eq!(runtime.allowed_commits(), 0);
    assert_eq!(runtime.document_mutations(), 0);

    let receipt = session.accept_replace(&proposal, &doc)?;
    assert_eq!(receipt.undo_depth(), 1);
    assert_eq!(receipt.author(), AI_REVISION_AUTHOR);
    assert_eq!(
        doc.final_text(),
        vec![
            "OUTSIDE-LEFT".to_owned(),
            AFTER.to_owned(),
            "OUTSIDE-RIGHT".to_owned()
        ]
    );
    assert_eq!(doc.commits(), 1);
    assert_eq!(doc.undo_depth(), 1);
    assert_eq!(runtime.applied_replacements(), 1);
    assert_eq!(runtime.allowed_commits(), 0);
    assert_eq!(runtime.provider_calls(), 0);
    assert!(
        doc.session
            .borrow()
            .document()
            .settings()
            .track_revisions
            .is_none()
    );
    assert_eq!(doc.session.borrow().revision_author(), "用户");
    let marks = doc.authors();
    assert!(!marks.is_empty());
    assert!(marks.iter().all(|(_, author)| author == AI_REVISION_AUTHOR));
    assert!(marks.iter().any(|(kind, _)| *kind == RevisionKind::Insert));
    assert!(marks.iter().any(|(kind, _)| *kind == RevisionKind::Delete));
    quiet(&format!("{receipt:?}"));
    quiet(&format!("{proposal:?}"));

    assert_eq!(
        session.accept_replace(&proposal, &doc).map(|_| ()),
        Err(ToolError::Replace(ReplaceError::AlreadyApplied))
    );
    assert_eq!(doc.commits(), 1);
    assert_eq!(doc.undo_depth(), 1);
    assert_eq!(runtime.applied_replacements(), 1);

    doc.undo_replace()?;
    assert_eq!(doc.final_text(), before);
    assert!(doc.authors().is_empty());
    assert_eq!(doc.undo_depth(), 0);
    doc.session.borrow_mut().redo().unwrap();
    assert_eq!(
        doc.final_text(),
        vec![
            "OUTSIDE-LEFT".to_owned(),
            AFTER.to_owned(),
            "OUTSIDE-RIGHT".to_owned()
        ]
    );
    assert_eq!(doc.undo_depth(), 1);
    Ok(())
}

#[test]
fn engine_accept_collapses_a_cross_paragraph_selection() -> TestResult {
    let runtime = open_runtime()?;
    let doc = EngineDoc::new(&["OUTSIDE-LEFT", BEFORE, "OUTSIDE-RIGHT"])?;
    let clock = TickClock::new();
    let snapshot = doc.snapshot(&doc.id)?;
    let node = snapshot.outline().get(1).copied().ok_or("paragraph")?;
    // From inside the left neighbour across both separators into the right
    // one: the covered marks are deleted revisions and the final view joins
    // the leftovers into one paragraph.
    let grant = Grant::issue(
        &doc,
        &doc.id,
        ScopeRange::new(node.start.saturating_sub(3), node.end + 4)?,
        ToolAllowlist::from_slice(&[ToolId::ProposeReplace]),
        runtime.generation()?,
        &clock,
        100,
    )?;
    let session = ToolSession::new(&runtime, &doc, grant, &clock);
    let proposal = session.propose_replace(request(ID, "joined"))?;
    let preview = session.preview_replace(&proposal)?;
    assert!(preview.before().contains('\n'));
    assert_eq!(doc.commits(), 0);
    let receipt = session.accept_replace(&proposal, &doc)?;
    assert_eq!(doc.commits(), 1);
    assert_eq!(receipt.undo_depth(), 1);
    assert_eq!(doc.final_text(), vec!["OUTSIDE-LEjoinedSIDE-RIGHT"]);
    let marks = doc.authors();
    assert!(marks.iter().all(|(_, author)| author == AI_REVISION_AUTHOR));
    doc.undo_replace()?;
    assert_eq!(
        doc.final_text(),
        vec![
            "OUTSIDE-LEFT".to_owned(),
            BEFORE.to_owned(),
            "OUTSIDE-RIGHT".to_owned(),
        ]
    );
    Ok(())
}

#[test]
fn engine_accept_refuses_a_stale_document_without_a_partial_write() -> TestResult {
    let runtime = open_runtime()?;
    let doc = EngineDoc::new(&["甲乙丙"])?;
    let clock = TickClock::new();
    let snapshot = doc.snapshot(&doc.id)?;
    let node = snapshot.outline().first().ok_or("paragraph")?;
    let grant = Grant::issue(
        &doc,
        &doc.id,
        ScopeRange::new(node.start, node.end)?,
        ToolAllowlist::from_slice(&[ToolId::ProposeReplace]),
        runtime.generation()?,
        &clock,
        40,
    )?;
    let session = ToolSession::new(&runtime, &doc, grant, &clock);
    let proposal = session.propose_replace(request("stale-id", "丁"))?;
    doc.user_insert_first("戊")?;
    let edited = doc.final_text();
    let depth = doc.undo_depth();
    assert_eq!(
        session.accept_replace(&proposal, &doc).map(|_| ()),
        Err(ToolError::Scope(ScopeError::VersionChanged {
            expected: snapshot.version(),
            found: snapshot.version() + 1
        }))
    );
    assert_eq!(doc.final_text(), edited);
    assert_eq!(doc.undo_depth(), depth);
    assert_eq!(doc.commits(), 0);
    assert_eq!(runtime.applied_replacements(), 0);
    assert_eq!(runtime.provider_calls(), 0);
    Ok(())
}

#[test]
fn stopping_before_accept_writes_nothing() -> TestResult {
    let runtime = open_runtime()?;
    let doc = history(BEFORE)?;
    let clock = TickClock::new();
    let (grant, proposal) = propose(&runtime, &doc, &clock, ID, AFTER)?;
    let session = ToolSession::new(&runtime, &doc, grant, &clock);
    let original = doc.text()?;
    runtime.begin_stop()?;
    assert_eq!(
        session.accept_replace(&proposal, &doc).map(|_| ()),
        Err(ToolError::Gate(GateError::Stopping))
    );
    assert_eq!(doc.commits(), 0);
    assert_eq!(doc.text()?, original);
    assert_eq!(runtime.applied_replacements(), 0);
    assert_eq!(runtime.provider_calls(), 0);
    Ok(())
}

#[test]
fn grant_failures_leave_the_document_and_do_not_consume_the_id() -> TestResult {
    let mut runtime = open_runtime()?;
    let doc = history(BEFORE)?;
    let clock = TickClock::new();
    let (grant, proposal) = propose(&runtime, &doc, &clock, ID, AFTER)?;
    let original = doc.text()?;
    let (deleted, comments) = doc.hidden()?;

    runtime.kill_switch()?;
    assert_eq!(
        ToolSession::new(&runtime, &doc, grant.clone(), &clock)
            .accept_replace(&proposal, &doc)
            .map(|_| ()),
        Err(ToolError::Gate(GateError::Disabled))
    );

    runtime.set_user_enabled(true)?;
    assert_eq!(
        ToolSession::new(&runtime, &doc, grant.clone(), &clock)
            .accept_replace(&proposal, &doc)
            .map(|_| ()),
        Err(ToolError::Scope(ScopeError::StaleGeneration))
    );

    let runtime = open_runtime()?;
    let doc = history(BEFORE)?;
    let (grant, proposal) = propose(&runtime, &doc, &clock, ID, AFTER)?;
    clock.advance(100);
    assert_eq!(
        ToolSession::new(&runtime, &doc, grant, &clock)
            .accept_replace(&proposal, &doc)
            .map(|_| ()),
        Err(ToolError::Scope(ScopeError::Expired))
    );

    let runtime = open_runtime()?;
    let doc = history(BEFORE)?;
    let (grant, proposal) = propose(&runtime, &doc, &clock, ID, AFTER)?;
    doc.corrupt_visible(1, "DIFFERENT-VISIBLE")?;
    assert_eq!(
        ToolSession::new(&runtime, &doc, grant.clone(), &clock)
            .accept_replace(&proposal, &doc)
            .map(|_| ()),
        Err(ToolError::Scope(ScopeError::FingerprintChanged))
    );
    doc.corrupt_visible(1, BEFORE)?;

    doc.edit(1, "EDITED-VISIBLE")?;
    let edited = doc.text()?;
    assert_eq!(
        ToolSession::new(&runtime, &doc, grant.clone(), &clock)
            .accept_replace(&proposal, &doc)
            .map(|_| ()),
        Err(ToolError::Scope(ScopeError::VersionChanged {
            expected: grant.base_version(),
            found: grant.base_version() + 1
        }))
    );
    assert_eq!(doc.text()?, edited);
    assert_eq!(doc.commits(), 0);
    assert_eq!(runtime.applied_replacements(), 0);
    assert_eq!(runtime.provider_calls(), 0);
    assert_eq!(doc.hidden()?, (deleted, comments));

    let readonly = Grant::issue(
        &doc,
        &doc.id,
        paragraph_range(&doc, 1)?,
        ToolAllowlist::read_only(),
        runtime.generation()?,
        &clock,
        100,
    )?;
    assert_eq!(
        ToolSession::new(&runtime, &doc, readonly, &clock)
            .accept_replace(&proposal, &doc)
            .map(|_| ()),
        Err(ToolError::NotAllowed)
    );

    let other = open_runtime()?;
    let other_grant = grant_for(&other, &doc, paragraph_range(&doc, 1)?, &clock)?;
    assert_eq!(
        ToolSession::new(&other, &doc, other_grant, &clock)
            .accept_replace(&proposal, &doc)
            .map(|_| ()),
        Err(ToolError::Replace(ReplaceError::BindingMismatch))
    );
    assert_eq!(doc.commits(), 0);
    let _ = original;
    Ok(())
}

#[test]
fn cross_paragraph_replacement_collapses_the_covered_group() -> TestResult {
    let runtime = open_runtime()?;
    let doc = history(BEFORE)?;
    let original = doc.text()?;
    let clock = TickClock::new();
    // The tail of the left neighbour, the whole selection, and the head of
    // the right neighbour: the three paragraphs collapse into one.
    let snapshot = doc.snapshot(&doc.id)?;
    let node = snapshot.outline().get(1).copied().ok_or("missing node")?;
    let wide = ScopeRange::new(node.start.saturating_sub(3), node.end + 4)?;
    let grant = grant_for(&runtime, &doc, wide, &clock)?;
    let session = ToolSession::new(&runtime, &doc, grant, &clock);
    let proposal = session.propose_replace(request("wide", "joined"))?;
    let preview = session.preview_replace(&proposal)?;
    assert!(preview.before().contains('\n'));
    let receipt = session.accept_replace(&proposal, &doc)?;
    assert_eq!(doc.commits(), 1);
    assert_eq!(runtime.applied_replacements(), 1);
    assert_eq!(doc.text()?, "OUTSIDE-LEjoinedSIDE-RIGHT");
    assert_eq!(receipt.version(), doc.snapshot(&doc.id)?.version());
    doc.undo_replace()?;
    assert_eq!(doc.text()?, original);
    assert_eq!(doc.commits(), 1);
    Ok(())
}

#[test]
fn unstoreable_replacements_are_refused() -> TestResult {
    let runtime = open_runtime()?;
    let doc = history(BEFORE)?;
    let clock = TickClock::new();
    let (grant, proposal) = propose(&runtime, &doc, &clock, "newline", "a\nb")?;
    let session = ToolSession::new(&runtime, &doc, grant, &clock);
    session.preview_replace(&proposal)?;
    assert_eq!(
        session.accept_replace(&proposal, &doc).map(|_| ()),
        Err(ToolError::Replace(ReplaceError::InvalidReplacement))
    );
    let (grant, proposal) = propose(&runtime, &doc, &clock, "marker", "a\u{FDD0}b")?;
    assert_eq!(
        ToolSession::new(&runtime, &doc, grant, &clock)
            .accept_replace(&proposal, &doc)
            .map(|_| ()),
        Err(ToolError::Replace(ReplaceError::InvalidReplacement))
    );
    assert_eq!(doc.commits(), 0);
    assert!(doc.text()?.contains(BEFORE));
    assert_eq!(runtime.applied_replacements(), 0);
    Ok(())
}

#[test]
fn history_accept_is_one_undo_and_a_duplicate_does_not_apply_again() -> TestResult {
    let runtime = open_runtime()?;
    let doc = history(BEFORE)?;
    let clock = TickClock::new();
    let (grant, proposal) = propose(&runtime, &doc, &clock, ID, AFTER)?;
    let session = ToolSession::new(&runtime, &doc, grant, &clock);
    let (deleted, comments) = doc.hidden()?;
    let receipt = session.accept_replace(&proposal, &doc)?;
    assert_eq!(receipt.undo_depth(), 1);
    assert_eq!(receipt.author(), AI_REVISION_AUTHOR);
    assert_eq!(doc.author()?, Some(AI_REVISION_AUTHOR));
    assert_eq!(doc.commits(), 1);
    let text = doc.text()?;
    assert!(text.contains(AFTER));
    assert!(!text.contains(BEFORE));
    assert!(text.contains("OUTSIDE-LEFT"));
    assert!(text.contains("OUTSIDE-RIGHT"));
    assert_eq!(doc.hidden()?, (deleted.clone(), comments.clone()));
    assert_eq!(
        session.accept_replace(&proposal, &doc).map(|_| ()),
        Err(ToolError::Replace(ReplaceError::AlreadyApplied))
    );
    assert_eq!(doc.commits(), 1);
    assert_eq!(runtime.applied_replacements(), 1);
    doc.undo_replace()?;
    assert!(doc.text()?.contains(BEFORE));
    assert_eq!(doc.hidden()?, (deleted, comments));
    assert_eq!(doc.author()?, None);
    quiet(&format!("{receipt:?} {}", ReplaceError::AlreadyApplied));
    Ok(())
}

struct Flaky<'a> {
    inner: &'a History,
    failures: AtomicUsize,
}

impl DocumentPort for Flaky<'_> {
    fn snapshot(&self, document: &DocumentId) -> Result<DocumentSnapshot, PortError> {
        self.inner.snapshot(document)
    }
}

impl DocumentCommit for Flaky<'_> {
    fn commit_replace(
        &self,
        change: &joyword_agent::ReplaceCommit<'_>,
        authorize: &mut dyn FnMut() -> Result<(), ToolError>,
    ) -> Result<ReplaceReceipt, CommitError> {
        authorize().map_err(|_| CommitError::Unauthorized)?;
        if self.failures.load(Ordering::SeqCst) > 0 {
            self.failures.fetch_sub(1, Ordering::SeqCst);
            return Err(CommitError::Rejected);
        }
        self.inner.commit_replace(change, &mut || Ok(()))
    }

    fn undo_replace(&self) -> Result<(), CommitError> {
        self.inner.undo_replace()
    }
}

#[test]
fn a_rejected_commit_writes_nothing_and_one_retry_applies_once() -> TestResult {
    let runtime = open_runtime()?;
    let doc = history(BEFORE)?;
    let clock = TickClock::new();
    let (grant, proposal) = propose(&runtime, &doc, &clock, "retry-id", AFTER)?;
    let session = ToolSession::new(&runtime, &doc, grant, &clock);
    let flaky = Flaky {
        inner: &doc,
        failures: AtomicUsize::new(1),
    };
    assert_eq!(
        session.accept_replace(&proposal, &flaky).map(|_| ()),
        Err(ToolError::Commit(CommitError::Rejected))
    );
    assert_eq!(doc.commits(), 0);
    assert!(doc.text()?.contains(BEFORE));
    assert_eq!(runtime.applied_replacements(), 0);
    let receipt = session.accept_replace(&proposal, &flaky)?;
    assert_eq!(receipt.undo_depth(), 1);
    assert_eq!(doc.commits(), 1);
    assert_eq!(runtime.applied_replacements(), 1);
    assert_eq!(
        session.accept_replace(&proposal, &flaky).map(|_| ()),
        Err(ToolError::Replace(ReplaceError::AlreadyApplied))
    );
    assert_eq!(doc.commits(), 1);
    Ok(())
}

struct KillThenAuthorize<'a> {
    inner: &'a History,
    runtime: &'a AgentRuntime,
}

impl DocumentPort for KillThenAuthorize<'_> {
    fn snapshot(&self, document: &DocumentId) -> Result<DocumentSnapshot, PortError> {
        self.inner.snapshot(document)
    }
}

impl DocumentCommit for KillThenAuthorize<'_> {
    fn commit_replace(
        &self,
        _change: &joyword_agent::ReplaceCommit<'_>,
        authorize: &mut dyn FnMut() -> Result<(), ToolError>,
    ) -> Result<ReplaceReceipt, CommitError> {
        self.runtime
            .begin_stop()
            .map_err(|_| CommitError::Rejected)?;
        authorize().map_err(|_| CommitError::Unauthorized)?;
        Err(CommitError::Rejected)
    }

    fn undo_replace(&self) -> Result<(), CommitError> {
        self.inner.undo_replace()
    }
}

struct AuthorizeThenStop<'a> {
    inner: &'a History,
    runtime: &'a AgentRuntime,
}

impl DocumentPort for AuthorizeThenStop<'_> {
    fn snapshot(&self, document: &DocumentId) -> Result<DocumentSnapshot, PortError> {
        self.inner.snapshot(document)
    }
}

impl DocumentCommit for AuthorizeThenStop<'_> {
    fn commit_replace(
        &self,
        change: &joyword_agent::ReplaceCommit<'_>,
        authorize: &mut dyn FnMut() -> Result<(), ToolError>,
    ) -> Result<ReplaceReceipt, CommitError> {
        let receipt = self.inner.commit_replace(change, authorize)?;
        self.runtime
            .begin_stop()
            .map_err(|_| CommitError::Rejected)?;
        Ok(receipt)
    }

    fn undo_replace(&self) -> Result<(), CommitError> {
        self.inner.undo_replace()
    }
}

#[test]
fn authorize_is_the_linearization_point() -> TestResult {
    let runtime = open_runtime()?;
    let doc = history(BEFORE)?;
    let clock = TickClock::new();
    let (grant, proposal) = propose(&runtime, &doc, &clock, "race-id", AFTER)?;
    let session = ToolSession::new(&runtime, &doc, grant, &clock);
    let killer = KillThenAuthorize {
        inner: &doc,
        runtime: &runtime,
    };
    assert_eq!(
        session.accept_replace(&proposal, &killer).map(|_| ()),
        Err(ToolError::Gate(GateError::Stopping))
    );
    assert_eq!(doc.commits(), 0);
    assert!(doc.text()?.contains(BEFORE));
    assert_eq!(runtime.applied_replacements(), 0);

    let mut runtime = open_runtime()?;
    let doc = history(BEFORE)?;
    let (grant, proposal) = propose(&runtime, &doc, &clock, "kept-id", AFTER)?;
    let session = ToolSession::new(&runtime, &doc, grant.clone(), &clock);
    let keeper = AuthorizeThenStop {
        inner: &doc,
        runtime: &runtime,
    };
    let receipt = session.accept_replace(&proposal, &keeper)?;
    assert_eq!(receipt.undo_depth(), 1);
    assert!(doc.text()?.contains(AFTER));
    assert_eq!(runtime.phase()?, joyword_agent::AiPhase::Stopping);
    assert_eq!(
        session.accept_replace(&proposal, &keeper).map(|_| ()),
        Err(ToolError::Replace(ReplaceError::AlreadyApplied))
    );
    runtime.kill_switch()?;
    runtime.set_user_enabled(true)?;
    let renewed = grant_for(&runtime, &doc, paragraph_range(&doc, 1)?, &clock)?;
    assert_eq!(
        ToolSession::new(&runtime, &doc, renewed, &clock)
            .accept_replace(&proposal, &doc)
            .map(|_| ()),
        Err(ToolError::Replace(ReplaceError::BindingMismatch))
    );
    assert_eq!(doc.commits(), 1);
    Ok(())
}

#[test]
fn concurrent_accept_applies_at_most_once() -> TestResult {
    let runtime = open_runtime()?;
    let doc = history(BEFORE)?;
    let clock = TickClock::new();
    let (grant, proposal) = propose(&runtime, &doc, &clock, ID, AFTER)?;
    let proposal = Arc::new(proposal);
    let results = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..16 {
            let runtime = &runtime;
            let doc = &doc;
            let grant = grant.clone();
            let clock = &clock;
            let proposal = Arc::clone(&proposal);
            handles.push(scope.spawn(move || {
                ToolSession::new(runtime, doc, grant, clock).accept_replace(&proposal, doc)
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().map_err(|_| "accept thread failed"))
            .collect::<Result<Vec<_>, _>>()
    })?;
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| {
                matches!(
                    result,
                    Err(ToolError::Replace(ReplaceError::AlreadyApplied))
                )
            })
            .count(),
        15
    );
    assert_eq!(doc.commits(), 1);
    assert_eq!(runtime.applied_replacements(), 1);
    assert!(doc.text()?.contains(AFTER));
    assert!(!doc.text()?.contains(BEFORE));
    Ok(())
}

#[test]
fn legacy_change_proposal_apply_stays_deferred() {
    let proposal = ChangeProposal {
        proposal_id: ID.to_owned(),
        document_id: DOC_ID.to_owned(),
        base_version: 1,
        explanation: REASON.to_owned(),
    };
    assert_eq!(proposal.apply(), Err(ApplyBlocked::DeferredUntilT402));
    quiet(&format!(
        "{:?}",
        Err::<(), _>(ApplyBlocked::DeferredUntilT402)
    ));
}

#[test]
fn empty_replacement_deletes_the_selection_in_one_undo() -> TestResult {
    let runtime = open_runtime()?;
    let doc = history(BEFORE)?;
    let clock = TickClock::new();
    let (grant, proposal) = propose(&runtime, &doc, &clock, "delete-id", "")?;
    let session = ToolSession::new(&runtime, &doc, grant, &clock);
    session.accept_replace(&proposal, &doc)?;
    let text = doc.text()?;
    assert!(text.contains("OUTSIDE-LEFT"));
    assert!(!text.contains(BEFORE));
    assert_eq!(doc.commits(), 1);
    doc.undo_replace()?;
    assert!(doc.text()?.contains(BEFORE));
    Ok(())
}
