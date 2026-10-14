//! DOCX-to-engine session boundary (T103/T106).
//!
//! Import restrictions are enforced before the first transaction. Until the
//! engine supports local locks, any locked node makes the whole session
//! read-only. The mutable engine is private so callers cannot clear that
//! policy or substitute a document while retaining an unrelated backing.
//! This adapter performs no IO, save, network access or implicit budget lift.

use std::fmt;

use joyoffice_opc::Budget;
use joyword_docx::{
    Diagnostics, Editability, ExportError, ExportPlan, ImportError, PackageBacking, import,
    plan_export,
};
use joyword_engine::{EditPolicy, Outcome, Session, Transaction, TxError, UndoError, UndoResult};
use joyword_model::DocumentVersion;

/// An original package cannot represent a session that has been edited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditedPackage {
    pub imported: DocumentVersion,
    pub current: DocumentVersion,
}

impl fmt::Display for EditedPackage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "original DOCX is at version {}; session is at {} and needs an edited export",
            self.imported, self.current
        )
    }
}

impl std::error::Error for EditedPackage {}

/// One imported package and its engine session, kept together for their
/// entire lifetime. Export plans provide verified bytes; file-system saving
/// and saved-version tracking belong to a separate storage boundary.
pub struct DocxSession {
    session: Session,
    backing: PackageBacking,
    diagnostics: Diagnostics,
    editability: Editability,
    imported_version: DocumentVersion,
    budget: Budget,
}

impl DocxSession {
    /// Parse untrusted bytes under the caller's explicit OPC budget.
    /// Refused imports never create a blank or partially editable session.
    pub fn open(bytes: &[u8], budget: &Budget) -> Result<Self, ImportError> {
        let result = import(bytes, budget)?;
        let mut session = Session::new(result.document);
        session.set_policy(EditPolicy {
            read_only: !result.editability.is_unrestricted(),
            agent_writes: false,
        });
        let imported_version = session.version();
        Ok(Self {
            session,
            backing: result.backing,
            diagnostics: result.diagnostics,
            editability: result.editability,
            imported_version,
            budget: budget.clone(),
        })
    }

    /// Read-only engine access for document snapshots, versions, selection
    /// and command availability. Mutations must use this adapter's methods.
    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn editability(&self) -> &Editability {
        &self.editability
    }

    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    /// Kept parts and fragments for inspection; their original bytes do not
    /// constitute an export of the current edited document.
    pub fn backing(&self) -> &PackageBacking {
        &self.backing
    }

    pub fn apply(&mut self, transaction: Transaction) -> Result<Outcome, TxError> {
        self.session.apply(transaction)
    }

    pub fn undo(&mut self) -> Result<UndoResult, UndoError> {
        self.session.undo()
    }

    pub fn redo(&mut self) -> Result<UndoResult, UndoError> {
        self.session.redo()
    }

    /// Build a verified export for the current immutable version, using the
    /// original import budget. This does not save or change session state.
    pub fn export_plan(&self) -> Result<ExportPlan, ExportError> {
        plan_export(
            &self.backing,
            &self.session.document().snapshot(),
            &self.budget,
        )
    }

    /// Return the exact source package only if no edit was ever committed.
    /// A rejected or empty transaction leaves this available. Undo publishes
    /// a new version, so a full undo must still use the verified export-plan
    /// path rather than this version-only original-byte accessor.
    /// This returns bytes; it does not write a file or mark the session saved.
    pub fn unchanged_package_bytes(&self) -> Result<&[u8], EditedPackage> {
        let current = self.session.version();
        if current != self.imported_version {
            return Err(EditedPackage {
                imported: self.imported_version,
                current,
            });
        }
        Ok(self.backing.original_bytes())
    }
}
