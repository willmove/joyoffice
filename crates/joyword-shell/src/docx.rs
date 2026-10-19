//! DOCX-to-engine session boundary (T103/T106).
//!
//! Import restrictions are enforced before the first transaction. Until the
//! engine supports local locks, any locked node makes the whole session
//! read-only. The mutable engine is private so callers cannot clear that
//! policy or substitute a document while retaining an unrelated backing.
//! [`DocxSession`] performs no IO. [`BoundDocx`] is the open path: `open_source`
//! then [`DocxSession::open`], and only then a sibling JoyWord lock. A failed
//! open installs nothing and creates no lock. Save As calls [`save_export_copy`]
//! and leaves the bound path, stamp, backing and dirty version untouched.
//! [`BoundDocx::save_copy_job`] detaches that copy of one committed version so
//! planning, verification and IO can run on another thread while editing
//! continues. `SaveOriginalJob` additionally binds the opened source and lock;
//! a published result installs a verified `ExportBinding` without resetting
//! the live engine document or its undo history.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::storage::{
    OriginalChoice, SaveCopyError, SaveCopyReceipt, SaveOriginalError, SaveOriginalReceipt,
    SourceStamp, open_source, save_export_copy, save_original_with_choice,
};
use joyoffice_opc::Budget;
use joyword_docx::{
    Diagnostics, Editability, ExportBinding, ExportError, ExportPlan, ImportError, PackageBacking,
    import, plan_export,
};
use joyword_engine::{EditPolicy, Outcome, Session, Transaction, TxError, UndoError, UndoResult};
use joyword_model::{DocumentSnapshot, DocumentVersion};

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
    // Shared with detached copy jobs. Import never mutates it afterwards.
    backing: Arc<PackageBacking>,
    diagnostics: Diagnostics,
    editability: Editability,
    imported_version: DocumentVersion,
    budget: Budget,
    rebound: Option<ExportBinding>,
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
            backing: Arc::new(result.backing),
            diagnostics: result.diagnostics,
            editability: result.editability,
            imported_version,
            budget: budget.clone(),
            rebound: None,
        })
    }

    /// [`Self::open`] with the default untrusted-input budget.
    pub fn open_default(bytes: &[u8]) -> Result<Self, ImportError> {
        Self::open(bytes, &Budget::default())
    }

    /// A detached Save As copy of the current committed version, for sessions
    /// without a bound path. Same guarantees as [`BoundDocx::save_copy_job`].
    pub fn save_copy_job(&self) -> SaveCopyJob {
        SaveCopyJob {
            backing: Arc::clone(&self.backing),
            snapshot: self.session.document().snapshot(),
            budget: self.budget.clone(),
            rebound: self.rebound.clone(),
        }
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

    fn rebind(&mut self, rebound: ReboundPackage) {
        self.backing = rebound.backing;
        self.rebound = Some(rebound.binding);
        self.diagnostics = rebound.diagnostics;
        self.editability = rebound.editability;
        self.session.set_policy(EditPolicy {
            read_only: !self.editability.is_unrestricted(),
            agent_writes: false,
        });
    }

    pub fn apply(&mut self, transaction: Transaction) -> Result<Outcome, TxError> {
        self.session.apply(transaction)
    }

    /// Input handoff retains this adapter's read-only policy and package backing.
    pub fn apply_with_mapped_selection(
        &mut self,
        transaction: Transaction,
        target: joyword_engine::Selection,
    ) -> Result<Outcome, TxError> {
        self.session
            .apply_with_mapped_selection(transaction, target)
    }

    pub fn undo(&mut self) -> Result<UndoResult, UndoError> {
        self.session.undo()
    }

    pub fn redo(&mut self) -> Result<UndoResult, UndoError> {
        self.session.redo()
    }

    /// Close the open typing/deleting undo group (see
    /// [`Session::seal_undo_group`]).
    pub fn seal_undo_group(&mut self) {
        self.session.seal_undo_group();
    }

    /// Author stored on revisions this session creates. Blank uses `JoyWord`.
    pub fn set_revision_author(&mut self, author: &str) {
        self.session.set_revision_author(author);
    }

    /// Build a verified export for the current immutable version, using the
    /// original import budget. This does not save or change session state.
    pub fn export_plan(&self) -> Result<ExportPlan, ExportError> {
        let snapshot = self.session.document().snapshot();
        match &self.rebound {
            Some(binding) => binding.plan(&snapshot, &self.budget),
            None => plan_export(&self.backing, &snapshot, &self.budget),
        }
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

static LOCK_SEQ: AtomicU64 = AtomicU64::new(1);

/// A document opened from a real path. The stamp and path stay on the file
/// that was opened. Save As writes a different path and does not clear dirty,
/// because this type does not track a saved version at all.
pub struct BoundDocx {
    inner: DocxSession,
    stamp: SourceStamp,
    lock_path: PathBuf,
    lock_token: String,
}

/// Why `open_source` never became an installed session.
#[derive(Debug)]
pub enum BoundOpenError {
    Source(SaveOriginalError),
    Import(ImportError),
    Export(ExportError),
    /// The plan's origin bytes are not the opened package.
    OriginMismatch,
    /// The file name cannot be turned into the JoyWord lock sibling.
    LockName,
    Lock(std::io::Error),
}

impl fmt::Display for BoundOpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(f, "{error}"),
            Self::Import(error) => write!(f, "{error}"),
            Self::Export(error) => write!(f, "{error}"),
            Self::OriginMismatch => f.write_str("opened package is not the export plan origin"),
            Self::LockName => f.write_str("source file name cannot own a JoyWord lock"),
            Self::Lock(error) => write!(f, "JoyWord lock: {error}"),
        }
    }
}

impl std::error::Error for BoundOpenError {}

/// Save As failed before any session field changed.
#[derive(Debug)]
pub enum BoundSaveError {
    Export(ExportError),
    Copy(SaveCopyError),
}

impl fmt::Display for BoundSaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Export(error) => write!(f, "{error}"),
            Self::Copy(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for BoundSaveError {}

/// Original-save preparation or publication failed. Preparation always runs
/// before any write, including import of the backing that a published receipt
/// will install without disturbing the live engine history.
#[derive(Debug)]
pub enum BoundOriginalSaveError {
    Export(ExportError),
    Import(ImportError),
    Storage(SaveOriginalError),
    RebindMismatch,
}

impl fmt::Display for BoundOriginalSaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Export(error) => write!(f, "{error}"),
            Self::Import(error) => write!(f, "{error}"),
            Self::Storage(error) => write!(f, "{error}"),
            Self::RebindMismatch => {
                f.write_str("published package cannot be rebound to the live document")
            }
        }
    }
}

impl std::error::Error for BoundOriginalSaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Export(error) => Some(error),
            Self::Import(error) => Some(error),
            Self::Storage(error) => Some(error),
            Self::RebindMismatch => None,
        }
    }
}

struct ReboundPackage {
    backing: Arc<PackageBacking>,
    binding: ExportBinding,
    diagnostics: Diagnostics,
    editability: Editability,
}

/// Result of a detached original-save job. The prepared backing is private and
/// can only be installed by [`BoundDocx::apply_original_save`].
pub struct SaveOriginalJobOutput {
    result: Result<SaveOriginalReceipt, BoundOriginalSaveError>,
    rebound: Option<ReboundPackage>,
}

impl SaveOriginalJobOutput {
    pub fn result(&self) -> Result<&SaveOriginalReceipt, &BoundOriginalSaveError> {
        self.result.as_ref()
    }

    pub fn storage_error(&self) -> Option<&SaveOriginalError> {
        match &self.result {
            Err(BoundOriginalSaveError::Storage(error)) => Some(error),
            _ => None,
        }
    }

    pub fn published_receipt(&self) -> Option<&SaveOriginalReceipt> {
        match &self.result {
            Ok(receipt) => Some(receipt),
            Err(BoundOriginalSaveError::Storage(error)) => error.published(),
            _ => None,
        }
    }
}

impl BoundDocx {
    /// `open_source`, then [`DocxSession::open`], then the sibling lock
    /// `.<file name>.joyword-lock`. Any error leaves the directory without
    /// that lock and returns no session.
    pub fn open(path: &Path, budget: &Budget) -> Result<Self, BoundOpenError> {
        let opened = open_source(path, budget).map_err(BoundOpenError::Source)?;
        let session = DocxSession::open(opened.bytes(), budget).map_err(BoundOpenError::Import)?;
        let plan = session.export_plan().map_err(BoundOpenError::Export)?;
        if plan.origin_bytes() != Some(opened.bytes()) {
            return Err(BoundOpenError::OriginMismatch);
        }
        let stamp = opened.into_stamp();
        let lock_path = joyword_lock_path(stamp.path()).ok_or(BoundOpenError::LockName)?;
        let lock_token = lock_token(stamp.path());
        write_lock(&lock_path, &lock_token)?;
        Ok(Self {
            inner: session,
            stamp,
            lock_path,
            lock_token,
        })
    }

    /// [`Self::open`] with [`Budget::default`]. The app shell calls this so it
    /// does not take a dependency on the OPC crate.
    pub fn open_default(path: &Path) -> Result<Self, BoundOpenError> {
        Self::open(path, &Budget::default())
    }

    pub fn session(&self) -> &DocxSession {
        &self.inner
    }

    pub fn session_mut(&mut self) -> &mut DocxSession {
        &mut self.inner
    }

    /// Canonical path of the opened file. Save As does not change it.
    pub fn source_path(&self) -> &Path {
        self.stamp.path()
    }

    pub fn stamp(&self) -> &SourceStamp {
        &self.stamp
    }

    /// The lock file this session created. Other opens of the same file see it.
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    /// Write `destination` as a new copy of the current verified plan.
    /// The bound path, stamp and engine version are not modified.
    pub fn save_copy(&self, destination: &Path) -> Result<SaveCopyReceipt, BoundSaveError> {
        self.save_copy_job().run(destination)
    }

    /// [`Self::save_copy`] of the current committed version, detached from
    /// this session. Taking it shares the immutable snapshot and backing and
    /// does no export work and no IO. Edits made afterwards are not in it.
    pub fn save_copy_job(&self) -> SaveCopyJob {
        SaveCopyJob {
            backing: Arc::clone(&self.inner.backing),
            snapshot: self.inner.session().snapshot(),
            budget: self.inner.budget.clone(),
            rebound: self.inner.rebound.clone(),
        }
    }

    /// Detach one committed version for an original-save precheck or write.
    /// Taking the job performs no export or IO and does not freeze later edits.
    pub fn save_original_job(&self) -> SaveOriginalJob {
        SaveOriginalJob {
            backing: Arc::clone(&self.inner.backing),
            snapshot: self.inner.session().snapshot(),
            budget: self.inner.budget.clone(),
            rebound: self.inner.rebound.clone(),
            stamp: self.stamp.clone(),
            lock_token: self.lock_token.clone(),
        }
    }

    /// Apply a detached result to the session that created it. Any published
    /// receipt, including one carried by a cleanup error, refreshes both the
    /// backing and source stamp. The live engine document and undo history are
    /// deliberately left alone, so later edits and late completions cannot be
    /// rolled back or attributed to another document.
    pub fn apply_original_save(
        &mut self,
        output: SaveOriginalJobOutput,
    ) -> Result<SaveOriginalReceipt, BoundOriginalSaveError> {
        let published = output.published_receipt().cloned();
        let SaveOriginalJobOutput { result, rebound } = output;
        if let Some(receipt) = published {
            let Some(rebound) = rebound else {
                return Err(BoundOriginalSaveError::RebindMismatch);
            };
            self.inner.rebind(rebound);
            self.stamp = receipt.stamp().clone();
        }
        result
    }

    pub fn save_original(
        &mut self,
        choice: OriginalChoice,
    ) -> Result<SaveOriginalReceipt, BoundOriginalSaveError> {
        let output = self.save_original_job().run(choice);
        self.apply_original_save(output)
    }
}

/// A Save As copy of one committed version. It owns everything [`SaveCopyJob::run`]
/// needs, so another thread can plan and write it while the session changes.
/// The session path, stamp, lock and saved version stay untouched.
#[derive(Clone)]
pub struct SaveCopyJob {
    backing: Arc<PackageBacking>,
    snapshot: DocumentSnapshot,
    budget: Budget,
    rebound: Option<ExportBinding>,
}

impl SaveCopyJob {
    /// The committed version this job writes.
    pub fn version(&self) -> DocumentVersion {
        self.snapshot.version()
    }

    /// Same plan and copy as [`BoundDocx::save_copy`] at [`Self::version`].
    pub fn run(&self, destination: &Path) -> Result<SaveCopyReceipt, BoundSaveError> {
        let plan = match &self.rebound {
            Some(binding) => binding.plan(&self.snapshot, &self.budget),
            None => plan_export(&self.backing, &self.snapshot, &self.budget),
        }
        .map_err(BoundSaveError::Export)?;
        save_export_copy(&plan, destination).map_err(BoundSaveError::Copy)
    }
}

/// One committed version bound to the opened source and its own JoyWord lock.
/// Export, reimport preparation and storage can all run away from the UI thread.
#[derive(Clone)]
pub struct SaveOriginalJob {
    backing: Arc<PackageBacking>,
    snapshot: DocumentSnapshot,
    budget: Budget,
    rebound: Option<ExportBinding>,
    stamp: SourceStamp,
    lock_token: String,
}

impl SaveOriginalJob {
    pub fn version(&self) -> DocumentVersion {
        self.snapshot.version()
    }

    fn prepare(&self) -> Result<(ExportPlan, ReboundPackage), BoundOriginalSaveError> {
        let plan = match &self.rebound {
            Some(binding) => binding.plan(&self.snapshot, &self.budget),
            None => plan_export(&self.backing, &self.snapshot, &self.budget),
        }
        .map_err(BoundOriginalSaveError::Export)?;
        let imported =
            import(plan.bytes(), &self.budget).map_err(BoundOriginalSaveError::Import)?;
        let binding = ExportBinding::rebind(imported.backing, &self.snapshot, &self.budget)
            .map_err(BoundOriginalSaveError::Export)?;
        let backing = Arc::new(binding.backing().clone());
        Ok((
            plan,
            ReboundPackage {
                backing,
                binding,
                diagnostics: imported.diagnostics,
                editability: imported.editability,
            },
        ))
    }

    pub fn run(&self, choice: OriginalChoice) -> SaveOriginalJobOutput {
        let (plan, rebound) = match self.prepare() {
            Ok(prepared) => prepared,
            Err(error) => {
                return SaveOriginalJobOutput {
                    result: Err(error),
                    rebound: None,
                };
            }
        };
        let result = save_original_with_choice(&plan, &self.stamp, Some(&self.lock_token), choice)
            .map_err(BoundOriginalSaveError::Storage);
        SaveOriginalJobOutput {
            result,
            rebound: Some(rebound),
        }
    }
}

impl Drop for BoundDocx {
    fn drop(&mut self) {
        // Only the token this session wrote. A replaced lock is left alone,
        // and a crash before drop leaves the file for the user to resolve.
        if fs::read_to_string(&self.lock_path).ok().as_deref() == Some(self.lock_token.as_str()) {
            let _ = fs::remove_file(&self.lock_path);
        }
    }
}

/// Same spelling W07 detects: `.<file name>.joyword-lock` beside the file.
fn joyword_lock_path(canonical: &Path) -> Option<PathBuf> {
    let name = canonical.file_name()?.to_str()?;
    let parent = canonical.parent()?;
    Some(parent.join(format!(".{name}.joyword-lock")))
}

fn lock_token(canonical: &Path) -> String {
    let seq = LOCK_SEQ.fetch_add(1, Ordering::Relaxed);
    format!(
        "joyword-lock 1\npid {}\nseq {seq}\npath {}\n",
        std::process::id(),
        canonical.display()
    )
}

fn write_lock(path: &Path, token: &str) -> Result<(), BoundOpenError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(BoundOpenError::Lock)?;
    if let Err(error) = file
        .write_all(token.as_bytes())
        .and_then(|_| file.sync_all())
    {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(BoundOpenError::Lock(error));
    }
    Ok(())
}
