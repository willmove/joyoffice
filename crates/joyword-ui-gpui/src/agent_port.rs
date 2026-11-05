//! Default-off host adapter for local, explicitly reviewed selection proposals.
use joyword_agent::{
    AgentRuntime, AiPhase, CommitError, DocumentCommit, DocumentId, DocumentPort, DocumentSnapshot,
    Grant, PortError, ReplaceCommit, ReplacePreview, ReplaceProposal, ReplaceReceipt,
    ReplaceRequest, ScopeRange, SnapshotParagraph, TickClock, ToolAllowlist, ToolError, ToolId,
};
use joyword_shell::{proposal_target::ProposalTarget, reader::Reader};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    time::Instant,
};

pub struct LocalAgent {
    runtime: AgentRuntime,
    sequence: u64,
}

impl Default for LocalAgent {
    fn default() -> Self {
        Self {
            runtime: AgentRuntime::boot_disabled(),
            sequence: 0,
        }
    }
}

pub struct PendingProposal {
    id: DocumentId,
    selection: ProposalTarget,
    grant: Grant,
    proposal: ReplaceProposal,
    started: Instant,
}

impl LocalAgent {
    pub fn enabled(&self) -> bool {
        matches!(
            self.runtime.phase(),
            Ok(AiPhase::EnabledIdle | AiPhase::Running)
        )
    }

    pub fn set_enabled(&mut self, on: bool) -> Result<(), String> {
        self.runtime.set_user_enabled(on).map_err(|e| e.to_string())
    }

    pub fn propose(
        &mut self,
        reader: &mut Reader,
        replacement: &str,
    ) -> Result<(PendingProposal, ReplacePreview), String> {
        if !self.enabled() {
            return Err("Local proposal review is disabled".into());
        }
        if replacement.chars().any(|ch| {
            ch.is_control() || matches!(ch, '\u{fffc}' | '\u{fdd0}' | '\u{2028}' | '\u{2029}')
        }) {
            return Err("Provide a single line of plain text".into());
        }
        let id = DocumentId::parse(&format!("document-{}", reader.engine().document_id()))
            .map_err(|_| "Invalid document identity")?;
        let selection = reader.proposal_target();
        let port = Port::new(reader, selection.clone());
        let snapshot = port.snapshot(&id).map_err(|e| e.to_string())?;
        let clock = TickClock::new();
        let range = ScopeRange::new(0, snapshot.char_len()).map_err(|e| e.to_string())?;
        let grant = Grant::issue(
            &port,
            &id,
            range,
            ToolAllowlist::from_slice(&[ToolId::ProposeReplace]),
            self.runtime.generation().map_err(|e| e.to_string())?,
            &clock,
            120,
        )
        .map_err(|e| e.to_string())?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("Proposal ID exhausted")?;
        let session = joyword_agent::ToolSession::new(&self.runtime, &port, grant.clone(), &clock);
        let proposal = session
            .propose_replace(ReplaceRequest {
                id: &format!("local-{}", self.sequence),
                replacement,
                explanation: "Local replacement draft",
            })
            .map_err(|e| e.to_string())?;
        let preview = session
            .preview_replace(&proposal)
            .map_err(|e| e.to_string())?;
        Ok((
            PendingProposal {
                id,
                selection,
                grant,
                proposal,
                started: Instant::now(),
            },
            preview,
        ))
    }

    pub fn accept(&self, reader: &mut Reader, pending: &PendingProposal) -> Result<(), String> {
        let clock = TickClock::new();
        clock.advance(pending.started.elapsed().as_secs());
        let port = Port::new(reader, pending.selection.clone());
        if port.snapshot(&pending.id).is_err() {
            return Err("Document or selection changed".into());
        }
        joyword_agent::ToolSession::new(&self.runtime, &port, pending.grant.clone(), &clock)
            .accept_replace(&pending.proposal, &port)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn selection_proposal_is_default_off_scoped_and_one_review_transaction() {
        let mut reader = Reader::blank(&AtomicBool::new(false)).unwrap();
        reader
            .execute("text.insert", &json!({"text":"中文 e\u{301}"}))
            .unwrap();
        reader.execute("select.all", &json!({})).unwrap();
        let original = reader.engine().doc.clone();
        let mut agent = LocalAgent::default();
        assert!(agent.propose(&mut reader, "替换").is_err());
        agent.set_enabled(true).unwrap();
        let (old, _) = agent.propose(&mut reader, "旧提案").unwrap();
        agent.set_enabled(false).unwrap();
        agent.set_enabled(true).unwrap();
        assert!(agent.accept(&mut reader, &old).is_err());
        let (stale, _) = agent.propose(&mut reader, "旧选区").unwrap();
        reader.execute("caret.docEnd", &json!({})).unwrap();
        assert!(agent.accept(&mut reader, &stale).is_err());
        reader.execute("select.all", &json!({})).unwrap();
        let (proposal, preview) = agent.propose(&mut reader, "替换").unwrap();
        assert_eq!(preview.before(), "中文 e\u{301}");
        assert_eq!(preview.after(), "替换");
        assert_eq!(reader.engine().doc, original);
        assert!(
            reader
                .execute("text.applyAgentProposal", &json!({"text":"bypass"}))
                .is_err()
        );
        agent.accept(&mut reader, &proposal).unwrap();
        assert!(agent.accept(&mut reader, &proposal).is_err());
        let doc = &reader.engine().doc;
        assert!(!doc.settings.track_changes);
        assert_eq!(doc.revisions.len(), 2);
        assert!(
            doc.revisions
                .iter()
                .all(|r| r.author == joyword_agent::AI_REVISION_AUTHOR)
        );
        reader.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(reader.engine().doc, original);
    }
}

struct Port<'a> {
    reader: RefCell<&'a mut Reader>,
    selection: ProposalTarget,
    committed: Cell<Option<u64>>,
}

impl<'a> Port<'a> {
    fn new(reader: &'a mut Reader, selection: ProposalTarget) -> Self {
        Self {
            reader: RefCell::new(reader),
            selection,
            committed: Cell::new(None),
        }
    }
}

impl DocumentPort for Port<'_> {
    fn snapshot(&self, id: &DocumentId) -> Result<DocumentSnapshot, PortError> {
        let reader = self.reader.borrow();
        if id.as_str() != format!("document-{}", reader.engine().document_id()) {
            return Err(PortError::UnknownDocument);
        }
        let visible = self
            .selection
            .text(&reader)
            .ok_or(PortError::Inaccessible)?;
        DocumentSnapshot::new(
            id.clone(),
            reader.engine().rev(),
            vec![SnapshotParagraph {
                heading_level: 0,
                visible: visible.to_owned(),
            }],
        )
    }
}

impl DocumentCommit for Port<'_> {
    fn commit_replace(
        &self,
        change: &ReplaceCommit<'_>,
        authorize: &mut dyn FnMut() -> Result<(), ToolError>,
    ) -> Result<ReplaceReceipt, CommitError> {
        let snapshot = self
            .snapshot(change.document_id())
            .map_err(|_| CommitError::Stale)?;
        if snapshot.version() != change.base_version() {
            return Err(CommitError::Stale);
        }
        if snapshot.text() != change.expected() {
            return Err(CommitError::Mismatch);
        }
        authorize().map_err(|_| CommitError::Unauthorized)?;
        let mut reader = self.reader.borrow_mut();
        self.selection
            .apply(&mut reader, change.replacement())
            .map_err(|_| CommitError::Rejected)?;
        let s = reader.engine();
        self.committed.set(Some(s.rev()));
        Ok(ReplaceReceipt::new(s.rev(), s.undo_depth() as u64))
    }

    fn undo_replace(&self) -> Result<(), CommitError> {
        let mut reader = self.reader.borrow_mut();
        if self.committed.get() != Some(reader.engine().rev()) {
            return Err(CommitError::NothingToUndo);
        }
        reader
            .execute("edit.undo", &json!({}))
            .map_err(|_| CommitError::Rejected)?;
        self.committed.set(None);
        Ok(())
    }
}
