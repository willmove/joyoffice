//! Open documents, unsaved-change tracking and the close/quit flows
//! (EDIT-001, ARCHITECTURE §4).
//!
//! A document is dirty when its current version differs from the version
//! last saved (or created). The save task reports the version it actually
//! wrote, so edits made while saving keep the document dirty. Closing a
//! dirty document asks first; cancelling keeps the session untouched. Save is
//! offered only when saving exists for that document.

use std::path::{Path, PathBuf};

use joyword_engine::Session;
use joyword_model::{Document, DocumentVersion};

use crate::l10n::Catalog;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentId(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    Untitled(u32),
    File(PathBuf),
}

pub struct OpenDocument {
    pub id: DocumentId,
    pub origin: Origin,
    pub session: Session,
    saved: DocumentVersion,
    /// Whether a writer exists for this document (DOCX save arrives in T103).
    pub can_save: bool,
}

impl OpenDocument {
    pub fn is_dirty(&self) -> bool {
        self.session.version() != self.saved
    }

    pub fn saved_version(&self) -> DocumentVersion {
        self.saved
    }

    /// Window title text.
    pub fn title(&self, catalog: &Catalog) -> String {
        match &self.origin {
            Origin::Untitled(number) => {
                catalog.format("document.untitled", &[("number", &number.to_string())])
            }
            Origin::File(path) => path
                .file_name()
                .map_or_else(|| path.to_string_lossy(), |name| name.to_string_lossy())
                .into_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CloseRequest {
    /// Nothing unsaved; the document was closed.
    Closed,
    /// Unsaved changes: ask the user, offering Save only when possible.
    Confirm {
        document: DocumentId,
        can_save: bool,
    },
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseChoice {
    Save,
    Discard,
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CloseOutcome {
    Closed,
    /// Cancelled: the document and its session are unchanged.
    Kept,
    /// The caller must save, then close again; nothing was closed.
    SaveFirst,
    /// Save was chosen but saving is unavailable for this document.
    CannotSave,
}

#[derive(Default)]
pub struct DocumentManager {
    documents: Vec<OpenDocument>,
    next_id: u64,
    next_untitled: u32,
}

impl DocumentManager {
    pub fn new() -> Self {
        Self::default()
    }

    fn allocate(&mut self) -> DocumentId {
        self.next_id += 1;
        DocumentId(self.next_id)
    }

    /// New untitled document; numbering never repeats within a run.
    pub fn new_document(&mut self, document: Document) -> DocumentId {
        self.next_untitled += 1;
        let origin = Origin::Untitled(self.next_untitled);
        self.insert(origin, document, false)
    }

    /// Registers a document opened from `path`; an already open file is
    /// returned instead of opening a second session on it.
    pub fn open_file(&mut self, path: &Path, document: Document, can_save: bool) -> DocumentId {
        if let Some(existing) = self.find_path(path) {
            return existing;
        }
        self.insert(Origin::File(path.to_path_buf()), document, can_save)
    }

    fn insert(&mut self, origin: Origin, document: Document, can_save: bool) -> DocumentId {
        let id = self.allocate();
        let session = Session::new(document);
        let saved = session.version();
        self.documents.push(OpenDocument {
            id,
            origin,
            session,
            saved,
            can_save,
        });
        id
    }

    pub fn find_path(&self, path: &Path) -> Option<DocumentId> {
        self.documents
            .iter()
            .find(|document| matches!(&document.origin, Origin::File(open) if open == path))
            .map(|document| document.id)
    }

    pub fn get(&self, id: DocumentId) -> Option<&OpenDocument> {
        self.documents.iter().find(|document| document.id == id)
    }

    pub fn get_mut(&mut self, id: DocumentId) -> Option<&mut OpenDocument> {
        self.documents.iter_mut().find(|document| document.id == id)
    }

    pub fn ids(&self) -> Vec<DocumentId> {
        self.documents.iter().map(|document| document.id).collect()
    }

    /// Records that `version` was written to `path`. Edits made after that
    /// version stay unsaved.
    pub fn mark_saved(&mut self, id: DocumentId, version: DocumentVersion, path: Option<PathBuf>) {
        if let Some(document) = self.get_mut(id) {
            document.saved = version;
            if let Some(path) = path {
                document.origin = Origin::File(path);
            }
        }
    }

    pub fn request_close(&mut self, id: DocumentId) -> CloseRequest {
        let Some(document) = self.get(id) else {
            return CloseRequest::Unknown;
        };
        if document.is_dirty() {
            return CloseRequest::Confirm {
                document: id,
                can_save: document.can_save,
            };
        }
        self.remove(id);
        CloseRequest::Closed
    }

    pub fn resolve_close(&mut self, id: DocumentId, choice: CloseChoice) -> CloseOutcome {
        let Some(document) = self.get(id) else {
            return CloseOutcome::Kept;
        };
        match choice {
            CloseChoice::Cancel => CloseOutcome::Kept,
            CloseChoice::Discard => {
                self.remove(id);
                CloseOutcome::Closed
            }
            CloseChoice::Save if document.can_save => CloseOutcome::SaveFirst,
            CloseChoice::Save => CloseOutcome::CannotSave,
        }
    }

    /// Documents that must be confirmed before quitting, in order.
    pub fn dirty_documents(&self) -> Vec<DocumentId> {
        self.documents
            .iter()
            .filter(|document| document.is_dirty())
            .map(|document| document.id)
            .collect()
    }

    fn remove(&mut self, id: DocumentId) {
        self.documents.retain(|document| document.id != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::l10n::Locale;
    use joyword_engine::{Command, Outcome, Transaction};
    use joyword_model::{Bias, Block, Position, ScalarOffset};

    fn type_text(document: &mut OpenDocument, text: &str) {
        let session = &mut document.session;
        let doc = session.document();
        let paragraph = doc.body().blocks.first().map(Block::id).unwrap();
        let at = Position {
            story: doc.body_id(),
            paragraph,
            offset: ScalarOffset(0),
            bias: Bias::After,
        };
        let transaction = Transaction::new(
            session.version(),
            vec![Command::InsertText {
                at,
                text: text.into(),
            }],
        );
        assert!(matches!(
            session.apply(transaction),
            Ok(Outcome::Applied(_))
        ));
    }

    #[test]
    fn untitled_titles_are_localized_and_never_reused() {
        let mut manager = DocumentManager::new();
        let first = manager.new_document(Document::new_blank());
        let second = manager.new_document(Document::new_blank());
        assert_eq!(manager.request_close(first), CloseRequest::Closed);
        let third = manager.new_document(Document::new_blank());
        let zh = Catalog::shipped(Locale::ZhCn);
        let en = Catalog::shipped(Locale::En);
        assert_eq!(manager.get(second).unwrap().title(zh), "未命名 2");
        assert_eq!(manager.get(third).unwrap().title(en), "Untitled 3");
        assert!(manager.get(first).is_none());
    }

    #[test]
    fn closing_dirty_documents_asks_and_cancel_keeps_the_session() {
        let mut manager = DocumentManager::new();
        let id = manager.new_document(Document::new_blank());
        type_text(manager.get_mut(id).unwrap(), "甲方");
        let version = manager.get(id).unwrap().session.version();
        assert_eq!(
            manager.request_close(id),
            CloseRequest::Confirm {
                document: id,
                can_save: false
            }
        );
        assert_eq!(
            manager.resolve_close(id, CloseChoice::Cancel),
            CloseOutcome::Kept
        );
        assert_eq!(manager.get(id).unwrap().session.version(), version);
        assert_eq!(
            manager.resolve_close(id, CloseChoice::Save),
            CloseOutcome::CannotSave
        );
        assert!(manager.get(id).is_some());
        assert_eq!(manager.dirty_documents(), vec![id]);
        assert_eq!(
            manager.resolve_close(id, CloseChoice::Discard),
            CloseOutcome::Closed
        );
        assert!(manager.get(id).is_none());
    }

    #[test]
    fn saved_state_binds_to_the_saved_version() {
        let mut manager = DocumentManager::new();
        let path = PathBuf::from("/合同/a.docx");
        let id = manager.open_file(&path, Document::new_blank(), true);
        assert_eq!(manager.open_file(&path, Document::new_blank(), true), id);
        type_text(manager.get_mut(id).unwrap(), "一");
        let saving = manager.get(id).unwrap().session.version();
        // The user keeps typing while the save task writes `saving`.
        type_text(manager.get_mut(id).unwrap(), "二");
        manager.mark_saved(id, saving, None);
        assert!(manager.get(id).unwrap().is_dirty());
        assert_eq!(
            manager.resolve_close(id, CloseChoice::Save),
            CloseOutcome::SaveFirst
        );
        let latest = manager.get(id).unwrap().session.version();
        manager.mark_saved(id, latest, None);
        assert!(!manager.get(id).unwrap().is_dirty());
        assert_eq!(manager.request_close(id), CloseRequest::Closed);
    }
}
