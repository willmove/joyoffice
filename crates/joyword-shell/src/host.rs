//! Document commands for open, save-copy, recent files, switching and close.
//! Menus and programmatic callers both go through [`Office::invoke`].

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use joyword_docx::{DiagnosticKind, Diagnostics};
use joyword_engine::{ParagraphChange, RunChange, TableMerge};
use joyword_model::Justification;

use crate::diag::{AppLog, Event, Level};

use crate::command::{
    ArgValue, Availability, CommandHost, CommandId, DOCUMENT_SWITCH, EDIT_ALIGN_CENTER, EDIT_BOLD,
    EDIT_COPY, EDIT_INSERT, EDIT_INSERT_IMAGE, EDIT_ITALIC, EDIT_PASTE, EDIT_SELECT_ALL,
    ExecuteError, FILE_CLOSE, FILE_OPEN, FILE_OPEN_RECENT, FILE_SAVE, FILE_SAVE_COPY, FIND_NEXT,
    FIND_PREVIOUS, FIND_REPLACE, FIND_REPLACE_ALL, FORMAT_CANCEL, FORMAT_COPY, FORMAT_FONT,
    FORMAT_PAINT, FORMAT_PARAGRAPH, InvokeError, LIST_APPLY, LIST_LEVEL, LIST_REMOVE, PAGE_BREAK,
    PAGE_MARGINS, PAGE_ORIENTATION, PAGE_SIZE, Registry, STYLE_APPLY, STYLE_LIST,
    TABLE_COLUMN_DELETE, TABLE_COLUMN_INSERT, TABLE_INSERT, TABLE_MERGE_DOWN, TABLE_MERGE_RIGHT,
    TABLE_ROW_DELETE, TABLE_ROW_INSERT, TABLE_SELECT, TABLE_SPLIT, VIEW_FIELD_CODES,
    VIEW_FORMATTING_MARKS, VIEW_HIDDEN_TEXT,
};
use crate::dialogs::{ViewToggles, font_draft_from_decision, paragraph_draft_from_decision};
use crate::docx::BoundDocx;
use crate::input::{EditRequest, InputError, InputLimits, InputSource, TextInput};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentId(pub u64);

struct Opened {
    id: DocumentId,
    /// `None` while the window is showing this document.
    input: Option<TextInput<BoundDocx>>,
    saved: u64,
    source: PathBuf,
}

struct PendingOpen {
    generation: u64,
    buffered: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveTicket {
    pub document: DocumentId,
    pub version: u64,
    pub generation: u64,
    destination: PathBuf,
}

/// The shipped multi-document session. File and edit commands run here.
pub struct Office {
    documents: Vec<Opened>,
    active: Option<DocumentId>,
    next_id: u64,
    clipboard: String,
    recent: Vec<PathBuf>,
    opening: Option<PendingOpen>,
    open_generation: u64,
    save_generation: u64,
    recovery_dir: Option<PathBuf>,
    diag: Option<Arc<AppLog>>,
    /// Names from the last `style.list` on the active document.
    style_list: Vec<String>,
    view: ViewToggles,
}

impl Default for Office {
    fn default() -> Self {
        Self::new()
    }
}

impl Office {
    pub fn new() -> Self {
        Self {
            documents: Vec::new(),
            active: None,
            next_id: 1,
            clipboard: String::new(),
            recent: Vec::new(),
            opening: None,
            open_generation: 0,
            save_generation: 0,
            recovery_dir: None,
            diag: None,
            style_list: Vec::new(),
            view: ViewToggles::default(),
        }
    }

    /// Formatting marks, hidden text and field codes. These do not change
    /// the document version.
    pub fn view_toggles(&self) -> ViewToggles {
        self.view
    }

    /// Styles reported by the last [`STYLE_LIST`] command.
    pub fn listed_styles(&self) -> &[String] {
        &self.style_list
    }

    /// Attaches the application log. Command failures and import diagnostic
    /// counts are written there; document text, comments and revisions are not.
    pub fn set_diagnostics(&mut self, log: Arc<AppLog>) {
        self.diag = Some(log);
    }

    fn note_failure(&self, event: &str, error: &ExecuteError) {
        let Some(log) = &self.diag else {
            return;
        };
        let ExecuteError::Failed(message) = error else {
            return;
        };
        let _ = log.record_untrusted(Level::Error, event, message);
    }

    fn note_import(&self, diagnostics: &Diagnostics) {
        let Some(log) = &self.diag else {
            return;
        };
        let _ = log.record(Event {
            level: Level::Info,
            event: "import.diagnostics",
            kind: Some("count"),
            count: Some(diagnostics.len() as u64),
        });
        if diagnostics.suppressed() > 0 {
            let _ = log.record(Event {
                level: Level::Info,
                event: "import.diagnostics",
                kind: Some("suppressed"),
                count: Some(diagnostics.suppressed() as u64),
            });
        }
        let mut kinds: Vec<(&'static str, u64)> = Vec::new();
        for item in diagnostics.iter().take(64) {
            let name = diagnostic_kind_name(&item.kind);
            if let Some((_, count)) = kinds.iter_mut().find(|(kind, _)| *kind == name) {
                *count = count.saturating_add(1);
            } else if kinds.len() < 16 {
                kinds.push((name, 1));
            }
        }
        for (name, count) in kinds {
            let _ = log.record(Event {
                level: Level::Info,
                event: "import.diagnostic",
                kind: Some(name),
                count: Some(count),
            });
        }
    }

    /// Directory for the production recovery journal. Edits committed through
    /// this session write the original package and the document text there.
    pub fn set_recovery_directory(&mut self, directory: PathBuf) {
        self.recovery_dir = Some(directory);
    }

    fn journal_active(&self) -> io::Result<()> {
        let Some(directory) = &self.recovery_dir else {
            return Ok(());
        };
        let Some(input) = self.active() else {
            return Ok(());
        };
        let baseline = input.backend().session().backing().original_bytes();
        let text = input.view().index().text();
        crate::storage::commit_recovery(directory, baseline, text)?;
        Ok(())
    }

    /// Journal an arbitrary package. Encrypted packages write nothing.
    pub fn journal_bytes(&self, baseline: &[u8], text: &str) -> io::Result<PathBuf> {
        let directory = self
            .recovery_dir
            .as_deref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no recovery directory"))?;
        crate::storage::commit_recovery(directory, baseline, text)
    }

    /// Open the journaled package and apply its committed text as an engine
    /// transaction on that session.
    pub fn recover_from(directory: &Path) -> Result<Self, ExecuteError> {
        let recovered = crate::storage::recover(directory)
            .map_err(|error| ExecuteError::Failed(error.to_string()))?;
        let path = directory.join("recovered.docx");
        std::fs::write(&path, &recovered.baseline)
            .map_err(|error| ExecuteError::Failed(error.to_string()))?;
        let mut office = Self::new();
        let generation = office.begin_open();
        office.accept_open(generation, &path)?;
        let current = office
            .active()
            .ok_or_else(|| ExecuteError::Failed("no document".into()))?
            .view()
            .index()
            .text()
            .to_owned();
        if current != recovered.text {
            let len = office
                .active()
                .ok_or_else(|| ExecuteError::Failed("no document".into()))?
                .view()
                .index()
                .len_utf16();
            office
                .active_mut()
                .ok_or_else(|| ExecuteError::Failed("no document".into()))?
                .replace_from(InputSource::Command, Some(0..len), &recovered.text)
                .map_err(input_error)?;
        }
        Ok(office)
    }

    /// The editor stays read-only until [`Self::accept_open`] for this generation.
    /// Record a path the user opened without going through [`Self::accept_open`].
    pub fn remember_path(&mut self, path: &Path) {
        self.remember(path);
    }

    /// End the read-only open gate when `generation` is still current.
    pub fn finish_open(&mut self, generation: u64) -> Result<String, ExecuteError> {
        match self.opening.take() {
            Some(pending) if pending.generation == generation => Ok(pending.buffered),
            Some(pending) => {
                self.apply_buffered(&pending.buffered)?;
                Err(ExecuteError::Failed("stale open dropped".into()))
            }
            None => Err(ExecuteError::Failed("no open in progress".into())),
        }
    }

    pub fn begin_open(&mut self) -> u64 {
        self.open_generation += 1;
        self.opening = Some(PendingOpen {
            generation: self.open_generation,
            buffered: String::new(),
        });
        self.open_generation
    }

    pub fn is_opening(&self) -> bool {
        self.opening.is_some()
    }

    /// Input during open is kept and applied when that open is accepted.
    /// A stale acceptance applies it to the document that is still current.
    pub fn type_during_open(&mut self, text: &str) -> Result<(), InputError> {
        if let Some(opening) = &mut self.opening {
            opening.buffered.push_str(text);
            return Ok(());
        }
        self.active_mut()
            .ok_or(InputError::ReadOnly)?
            .replace(None, text)
            .map(|_| ())
    }

    pub fn accept_open(&mut self, generation: u64, path: &Path) -> Result<(), ExecuteError> {
        let Some(pending) = self.opening.take() else {
            return self.open_path(path);
        };
        if pending.generation != generation {
            self.apply_buffered(&pending.buffered)?;
            return Err(ExecuteError::Failed("stale open dropped".into()));
        }
        self.open_path(path)?;
        if !pending.buffered.is_empty() {
            self.active_mut()
                .ok_or_else(|| ExecuteError::Failed("no document".into()))?
                .replace_from(InputSource::Command, None, &pending.buffered)
                .map_err(input_error)?;
        }
        Ok(())
    }

    fn apply_buffered(&mut self, text: &str) -> Result<(), ExecuteError> {
        if text.is_empty() {
            return Ok(());
        }
        if let Some(active) = self.active_mut() {
            active
                .replace_from(InputSource::Command, None, text)
                .map_err(input_error)?;
        }
        Ok(())
    }

    pub fn start_save(&mut self, destination: &Path) -> Result<SaveTicket, ExecuteError> {
        let id = self
            .active
            .ok_or_else(|| ExecuteError::Failed("no document".into()))?;
        let version = self
            .documents
            .iter()
            .find(|document| document.id == id)
            .and_then(|document| document.input.as_ref())
            .map(|input| input.backend().session().session().version().get())
            .ok_or_else(|| ExecuteError::Failed("no document".into()))?;
        self.save_generation += 1;
        Ok(SaveTicket {
            document: id,
            version,
            generation: self.save_generation,
            destination: destination.to_path_buf(),
        })
    }

    /// Write the save-copy for this ticket, then clear the unsaved flag only
    /// when the session version is still the one that was saved. A stale
    /// ticket writes nothing and leaves the document dirty.
    pub fn finish_save(&mut self, ticket: SaveTicket) -> Result<bool, ExecuteError> {
        let Some(document) = self
            .documents
            .iter_mut()
            .find(|document| document.id == ticket.document)
        else {
            return Ok(false);
        };
        let Some(input) = document.input.as_mut() else {
            return Ok(false);
        };
        let current = input.backend().session().session().version().get();
        if current != ticket.version {
            return Ok(false);
        }
        let receipt = input
            .backend()
            .save_copy(&ticket.destination)
            .map_err(|error| ExecuteError::Failed(error.to_string()))?;
        let after = input.backend().session().session().version().get();
        if after != ticket.version {
            return Ok(false);
        }
        input.mark_saved(receipt.version());
        document.saved = after;
        Ok(true)
    }

    pub fn is_dirty(&self, id: DocumentId) -> bool {
        self.documents
            .iter()
            .find(|document| document.id == id)
            .is_some_and(|document| match &document.input {
                Some(input) => input.has_unsaved_changes(),
                None => true,
            })
    }

    pub fn invoke(
        &mut self,
        id: CommandId,
        args: &[(&'static str, ArgValue)],
    ) -> Result<(), InvokeError> {
        Registry::builtin().invoke_with(self, id, args)
    }

    pub fn recent(&self) -> &[PathBuf] {
        &self.recent
    }

    pub fn active_id(&self) -> Option<DocumentId> {
        self.active
    }

    pub fn document_ids(&self) -> Vec<DocumentId> {
        self.documents.iter().map(|document| document.id).collect()
    }

    pub fn active_mut(&mut self) -> Option<&mut TextInput<BoundDocx>> {
        let id = self.active?;
        self.documents
            .iter_mut()
            .find(|document| document.id == id)?
            .input
            .as_mut()
    }

    pub fn active(&self) -> Option<&TextInput<BoundDocx>> {
        let id = self.active?;
        self.documents
            .iter()
            .find(|document| document.id == id)?
            .input
            .as_ref()
    }

    /// Move the active document out so the window can show it.
    pub fn hold_active(&mut self) -> Option<TextInput<BoundDocx>> {
        let id = self.active?;
        self.documents
            .iter_mut()
            .find(|document| document.id == id)?
            .input
            .take()
    }

    /// Put a document the window was showing back into its session slot.
    pub fn restore(&mut self, id: DocumentId, input: TextInput<BoundDocx>) -> bool {
        let Some(document) = self.documents.iter_mut().find(|document| document.id == id) else {
            return false;
        };
        document.saved = input.saved_version().get();
        document.input = Some(input);
        true
    }

    /// Route a late original-save result to its owning session, never to the
    /// document that happens to be active when the IO completes.
    pub fn apply_original_save(
        &mut self,
        id: DocumentId,
        output: crate::docx::SaveOriginalJobOutput,
    ) -> Option<Result<crate::storage::SaveOriginalReceipt, crate::docx::BoundOriginalSaveError>>
    {
        let version = output
            .published_receipt()
            .map(|receipt| receipt.version().get());
        let document = self
            .documents
            .iter_mut()
            .find(|document| document.id == id)?;
        let result = document.input.as_mut()?.apply_original_save(output);
        if let Some(version) = version {
            document.saved = document.saved.max(version);
        }
        Some(result)
    }

    /// Register a document opened outside [`Self::accept_open`].
    pub fn attach(&mut self, input: TextInput<BoundDocx>) -> DocumentId {
        let source = input.backend().source_path().to_path_buf();
        let saved = input.saved_version().get();
        if let Some(id) = self
            .documents
            .iter()
            .find(|document| document.source == source || same_file(&document.source, &source))
            .map(|document| document.id)
        {
            if let Some(document) = self.documents.iter_mut().find(|document| document.id == id) {
                document.saved = saved;
                document.input = Some(input);
            }
            self.active = Some(id);
            self.remember(&source);
            return id;
        }
        let id = DocumentId(self.next_id);
        self.next_id += 1;
        self.documents.push(Opened {
            id,
            input: Some(input),
            saved,
            source: source.clone(),
        });
        self.active = Some(id);
        self.remember(&source);
        id
    }

    fn remember(&mut self, path: &Path) {
        let path = path.to_path_buf();
        self.recent.retain(|existing| existing != &path);
        self.recent.insert(0, path);
        self.recent.truncate(10);
    }

    fn open_path(&mut self, path: &Path) -> Result<(), ExecuteError> {
        let existing = self.documents.iter().find_map(|document| {
            (document.source == path || same_file(&document.source, path))
                .then(|| (document.id, document.source.clone()))
        });
        if let Some((id, source)) = existing {
            self.active = Some(id);
            self.remember(&source);
            return Ok(());
        }
        let bound = BoundDocx::open_default(path)
            .map_err(|error| ExecuteError::Failed(error.to_string()))?;
        let source = bound.source_path().to_path_buf();
        let input = TextInput::new(bound, InputLimits::default())
            .map_err(|error| ExecuteError::Failed(error.to_string()))?;
        self.note_import(input.backend().session().diagnostics());
        let saved = input.backend().session().session().version().get();
        let id = DocumentId(self.next_id);
        self.next_id += 1;
        self.documents.push(Opened {
            id,
            input: Some(input),
            saved,
            source: source.clone(),
        });
        self.active = Some(id);
        self.remember(&source);
        Ok(())
    }

    fn require_active(&mut self) -> Result<&mut TextInput<BoundDocx>, ExecuteError> {
        let id = self
            .active
            .ok_or_else(|| ExecuteError::Failed("no document".into()))?;
        self.documents
            .iter_mut()
            .find(|document| document.id == id)
            .and_then(|document| document.input.as_mut())
            .ok_or_else(|| ExecuteError::Failed("no document".into()))
    }
}

fn same_file(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn diagnostic_kind_name(kind: &DiagnosticKind) -> &'static str {
    match kind {
        DiagnosticKind::Package(_) => "package",
        DiagnosticKind::ExternalTarget { .. } => "external_target",
        DiagnosticKind::Preserved { .. } => "preserved",
        DiagnosticKind::ReadOnly(_) => "read_only",
        DiagnosticKind::MissingPart { .. } => "missing_part",
        DiagnosticKind::NotImported { .. } => "not_imported",
        DiagnosticKind::InvalidValue { .. } => "invalid_value",
        DiagnosticKind::StyleSkipped { .. } => "style_skipped",
        DiagnosticKind::SectionDefaults { .. } => "section_defaults",
        DiagnosticKind::PropertiesPreserved { .. } => "properties_preserved",
        DiagnosticKind::HintsSkipped { .. } => "hints_skipped",
        DiagnosticKind::UnpreservedWhitespace => "unpreserved_whitespace",
        DiagnosticKind::EmptyBody => "empty_body",
        DiagnosticKind::MacroProject => "macro_project",
        DiagnosticKind::Image { .. } => "image",
        DiagnosticKind::Model(_) => "model",
    }
}

fn input_error(error: InputError) -> ExecuteError {
    ExecuteError::Failed(error.to_string())
}

fn parse_u32(value: &ArgValue) -> Option<u32> {
    match value {
        ArgValue::Text(text) => text.parse().ok(),
        ArgValue::Path(_) | ArgValue::Id(_) | ArgValue::Bool(_) => None,
    }
}

fn parse_i64(value: &ArgValue) -> Option<i64> {
    match value {
        ArgValue::Text(text) => text.parse().ok(),
        ArgValue::Path(_) | ArgValue::Id(_) | ArgValue::Bool(_) => None,
    }
}

fn parse_u8(value: &ArgValue) -> Option<u8> {
    match value {
        ArgValue::Text(text) => text.parse().ok(),
        ArgValue::Path(_) | ArgValue::Id(_) | ArgValue::Bool(_) => None,
    }
}

fn flag(args: &BTreeMap<&'static str, ArgValue>, name: &str) -> bool {
    matches!(args.get(name), Some(ArgValue::Bool(true)))
}

fn table_axis(
    args: &BTreeMap<&'static str, ArgValue>,
    name: &'static str,
) -> Result<u32, ExecuteError> {
    match args.get(name) {
        None => Ok(2),
        Some(value) => parse_u32(value).ok_or_else(|| ExecuteError::Failed(name.into())),
    }
}

fn text_arg<'a>(args: &'a BTreeMap<&'static str, ArgValue>, name: &str) -> Option<&'a str> {
    match args.get(name) {
        Some(ArgValue::Text(text)) => Some(text),
        _ => None,
    }
}

impl CommandHost for Office {
    fn availability(&self, id: CommandId) -> Availability {
        if matches!(
            id,
            VIEW_FORMATTING_MARKS | VIEW_HIDDEN_TEXT | VIEW_FIELD_CODES
        ) {
            return Availability::Enabled;
        }
        if (matches!(
            id,
            FILE_SAVE
                | FILE_SAVE_COPY
                | STYLE_LIST
                | STYLE_APPLY
                | LIST_APPLY
                | LIST_LEVEL
                | LIST_REMOVE
                | FIND_NEXT
                | FIND_PREVIOUS
                | FIND_REPLACE
                | FIND_REPLACE_ALL
                | FORMAT_COPY
                | FORMAT_PAINT
                | FORMAT_CANCEL
                | TABLE_INSERT
                | TABLE_ROW_INSERT
                | TABLE_ROW_DELETE
                | TABLE_COLUMN_INSERT
                | TABLE_COLUMN_DELETE
                | TABLE_SELECT
                | TABLE_MERGE_RIGHT
                | TABLE_MERGE_DOWN
                | TABLE_SPLIT
                | FORMAT_FONT
                | FORMAT_PARAGRAPH
        ) || id.0.starts_with("edit.")
            || id.0.starts_with("page."))
            && self.active.is_none()
        {
            return Availability::Disabled("input.loading");
        }
        if self.opening.is_some()
            && (id.0.starts_with("edit.")
                || id.0.starts_with("page.")
                || matches!(
                    id,
                    STYLE_APPLY
                        | LIST_APPLY
                        | LIST_LEVEL
                        | LIST_REMOVE
                        | FIND_NEXT
                        | FIND_PREVIOUS
                        | FIND_REPLACE
                        | FIND_REPLACE_ALL
                        | FORMAT_COPY
                        | FORMAT_PAINT
                        | FORMAT_CANCEL
                        | TABLE_INSERT
                        | TABLE_ROW_INSERT
                        | TABLE_ROW_DELETE
                        | TABLE_COLUMN_INSERT
                        | TABLE_COLUMN_DELETE
                        | TABLE_SELECT
                        | TABLE_MERGE_RIGHT
                        | TABLE_MERGE_DOWN
                        | TABLE_SPLIT
                        | FORMAT_FONT
                        | FORMAT_PARAGRAPH
                ))
        {
            return Availability::Disabled("input.read_only");
        }
        if (matches!(
            id,
            EDIT_INSERT
                | EDIT_INSERT_IMAGE
                | EDIT_BOLD
                | EDIT_ITALIC
                | EDIT_ALIGN_CENTER
                | EDIT_PASTE
                | STYLE_APPLY
                | LIST_APPLY
                | LIST_LEVEL
                | LIST_REMOVE
                | FIND_REPLACE
                | FIND_REPLACE_ALL
                | FORMAT_PAINT
                | TABLE_INSERT
                | TABLE_ROW_INSERT
                | TABLE_ROW_DELETE
                | TABLE_COLUMN_INSERT
                | TABLE_COLUMN_DELETE
                | TABLE_MERGE_RIGHT
                | TABLE_MERGE_DOWN
                | TABLE_SPLIT
                | FORMAT_FONT
                | FORMAT_PARAGRAPH
        ) || id.0.starts_with("page."))
            && self.active().is_some_and(|input| input.read_only())
        {
            return Availability::Disabled("input.read_only");
        }
        Availability::Enabled
    }

    fn execute(
        &mut self,
        id: CommandId,
        args: &BTreeMap<&'static str, ArgValue>,
    ) -> Result<(), ExecuteError> {
        let result = self.dispatch(id, args);
        if let Err(error) = &result {
            self.note_failure(id.0, error);
        }
        result
    }
}

impl Office {
    fn dispatch(
        &mut self,
        id: CommandId,
        args: &BTreeMap<&'static str, ArgValue>,
    ) -> Result<(), ExecuteError> {
        match id {
            FILE_OPEN | FILE_OPEN_RECENT => {
                let Some(ArgValue::Path(path)) = args.get("path") else {
                    return Err(ExecuteError::Failed("path".into()));
                };
                if id == FILE_OPEN_RECENT
                    && !self
                        .recent
                        .iter()
                        .any(|recent| recent == path || same_file(recent, path))
                {
                    return Err(ExecuteError::Failed("path is not in recent files".into()));
                }
                let generation = self.begin_open();
                self.accept_open(generation, path)
            }
            FILE_SAVE => {
                match args.get("choice") {
                    Some(ArgValue::Text(choice)) if choice == "confirm" => {}
                    None => return Err(ExecuteError::Pending(vec!["choice"])),
                    _ => return Err(ExecuteError::Failed("unsupported save choice".into())),
                }
                self.require_active()?
                    .save_original(crate::storage::OriginalChoice::Confirm)
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            FILE_SAVE_COPY => {
                let Some(ArgValue::Path(path)) = args.get("destination") else {
                    return Err(ExecuteError::Failed("destination".into()));
                };
                let input = self.require_active()?;
                input
                    .backend()
                    .save_copy(path)
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            DOCUMENT_SWITCH => {
                let Some(ArgValue::Id(id)) = args.get("id") else {
                    return Err(ExecuteError::Failed("id".into()));
                };
                let id = DocumentId(*id);
                if !self.documents.iter().any(|document| document.id == id) {
                    return Err(ExecuteError::Failed("unknown document".into()));
                }
                self.active = Some(id);
                Ok(())
            }
            FILE_CLOSE => {
                let Some(ArgValue::Id(id)) = args.get("id") else {
                    return Err(ExecuteError::Failed("id".into()));
                };
                let id = DocumentId(*id);
                let Some(index) = self.documents.iter().position(|document| document.id == id)
                else {
                    return Err(ExecuteError::Failed("unknown document".into()));
                };
                let dirty = match &self.documents[index].input {
                    Some(input) => input.has_unsaved_changes(),
                    None => true,
                };
                if dirty {
                    let choice = match args.get("choice") {
                        Some(ArgValue::Text(choice)) => choice.as_str(),
                        Some(_) => return Err(ExecuteError::Failed("choice".into())),
                        None => return Err(ExecuteError::Pending(vec!["choice"])),
                    };
                    match choice {
                        "cancel" => return Ok(()),
                        "discard" => {}
                        other => {
                            return Err(ExecuteError::Failed(format!(
                                "unsupported close choice {other}"
                            )));
                        }
                    }
                }
                self.documents.remove(index);
                if self.active == Some(id) {
                    self.active = self.documents.last().map(|document| document.id);
                }
                Ok(())
            }
            EDIT_SELECT_ALL => {
                self.require_active()?.select_all().map_err(input_error)?;
                Ok(())
            }
            EDIT_INSERT => {
                let Some(ArgValue::Text(text)) = args.get("text") else {
                    return Err(ExecuteError::Failed("text".into()));
                };
                self.require_active()?
                    .replace_from(InputSource::Command, None, text)
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            EDIT_INSERT_IMAGE => {
                let Some(ArgValue::Path(path)) = args.get("path") else {
                    return Err(ExecuteError::Failed("path".into()));
                };
                self.require_active()?
                    .insert_image_file(path)
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            EDIT_BOLD => {
                self.require_active()?
                    .format_runs(RunChange::Bold(Some(true)))
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            EDIT_ITALIC => {
                self.require_active()?
                    .format_runs(RunChange::Italic(Some(true)))
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            EDIT_ALIGN_CENTER => {
                self.require_active()?
                    .format_paragraph(ParagraphChange::Justification(Some(Justification::Center)))
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            STYLE_LIST => {
                self.style_list = self.require_active()?.list_styles();
                Ok(())
            }
            STYLE_APPLY => {
                let Some(ArgValue::Text(style)) = args.get("style") else {
                    return Err(ExecuteError::Failed("style".into()));
                };
                self.require_active()?
                    .apply_style(style)
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            LIST_APPLY => {
                let Some(num_id) = args.get("num").and_then(parse_u32) else {
                    return Err(ExecuteError::Failed("num".into()));
                };
                self.require_active()?
                    .apply_list(num_id)
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            LIST_LEVEL => {
                let Some(level) = args.get("level").and_then(parse_u8) else {
                    return Err(ExecuteError::Failed("level".into()));
                };
                self.require_active()?
                    .set_list_level(level)
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            LIST_REMOVE => {
                self.require_active()?.remove_list().map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            PAGE_BREAK => {
                self.require_active()?
                    .insert_page_break()
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            PAGE_SIZE => {
                let Some(width) = args.get("width").and_then(parse_i64) else {
                    return Err(ExecuteError::Failed("width".into()));
                };
                let Some(height) = args.get("height").and_then(parse_i64) else {
                    return Err(ExecuteError::Failed("height".into()));
                };
                self.require_active()?
                    .set_page_size(width, height)
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            PAGE_MARGINS => {
                let Some(top) = args.get("top").and_then(parse_i64) else {
                    return Err(ExecuteError::Failed("top".into()));
                };
                let Some(right) = args.get("right").and_then(parse_i64) else {
                    return Err(ExecuteError::Failed("right".into()));
                };
                let Some(bottom) = args.get("bottom").and_then(parse_i64) else {
                    return Err(ExecuteError::Failed("bottom".into()));
                };
                let Some(left) = args.get("left").and_then(parse_i64) else {
                    return Err(ExecuteError::Failed("left".into()));
                };
                self.require_active()?
                    .set_page_margins(top, right, bottom, left)
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            PAGE_ORIENTATION => {
                let Some(ArgValue::Text(orientation)) = args.get("orientation") else {
                    return Err(ExecuteError::Failed("orientation".into()));
                };
                let landscape = match orientation.as_str() {
                    "landscape" => true,
                    "portrait" => false,
                    _ => return Err(ExecuteError::Failed("orientation".into())),
                };
                self.require_active()?
                    .set_page_orientation(landscape)
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            FIND_NEXT | FIND_PREVIOUS => {
                let Some(query) = text_arg(args, "query") else {
                    return Err(ExecuteError::Failed("query".into()));
                };
                let query = query.to_owned();
                self.require_active()?
                    .editing(EditRequest::Find {
                        query,
                        case_sensitive: flag(args, "case"),
                        whole_word: flag(args, "word"),
                        forward: id == FIND_NEXT,
                    })
                    .map_err(input_error)?;
                Ok(())
            }
            FIND_REPLACE | FIND_REPLACE_ALL => {
                let Some(query) = text_arg(args, "query") else {
                    return Err(ExecuteError::Failed("query".into()));
                };
                let Some(replacement) = text_arg(args, "replacement") else {
                    return Err(ExecuteError::Failed("replacement".into()));
                };
                let query = query.to_owned();
                let replacement = replacement.to_owned();
                self.require_active()?
                    .editing(EditRequest::Replace {
                        query,
                        replacement,
                        case_sensitive: flag(args, "case"),
                        whole_word: flag(args, "word"),
                        all: id == FIND_REPLACE_ALL,
                    })
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            FORMAT_COPY => {
                self.require_active()?
                    .editing(EditRequest::CopyFormat {
                        repeat: flag(args, "repeat"),
                    })
                    .map_err(input_error)?;
                Ok(())
            }
            FORMAT_PAINT => {
                self.require_active()?
                    .editing(EditRequest::PaintFormat)
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            FORMAT_CANCEL => {
                self.require_active()?
                    .editing(EditRequest::CancelFormat)
                    .map_err(input_error)?;
                Ok(())
            }
            TABLE_INSERT => {
                let rows = table_axis(args, "rows")?;
                let columns = table_axis(args, "columns")?;
                self.require_active()?
                    .insert_table(rows, columns)
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            TABLE_ROW_INSERT => {
                self.require_active()?
                    .insert_table_row()
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            TABLE_ROW_DELETE => {
                self.require_active()?
                    .delete_table_row()
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            TABLE_COLUMN_INSERT => {
                self.require_active()?
                    .insert_table_column()
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            TABLE_COLUMN_DELETE => {
                self.require_active()?
                    .delete_table_column()
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            TABLE_SELECT => {
                self.require_active()?
                    .select_table_cell()
                    .map_err(input_error)?;
                Ok(())
            }
            TABLE_MERGE_RIGHT | TABLE_MERGE_DOWN => {
                let direction = if id == TABLE_MERGE_RIGHT {
                    TableMerge::Right
                } else {
                    TableMerge::Down
                };
                self.require_active()?
                    .merge_table_cells(direction)
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            TABLE_SPLIT => {
                self.require_active()?
                    .split_table_cell()
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            EDIT_COPY => {
                self.clipboard = self
                    .require_active()?
                    .selected_text()
                    .map_err(input_error)?;
                Ok(())
            }
            FORMAT_FONT => {
                let Some(draft) = font_draft_from_decision(Some(args))
                    .map_err(|reason| ExecuteError::Failed(reason.into()))?
                else {
                    return Ok(());
                };
                self.require_active()?
                    .apply_font(&draft)
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            FORMAT_PARAGRAPH => {
                let Some(draft) = paragraph_draft_from_decision(Some(args))
                    .map_err(|reason| ExecuteError::Failed(reason.into()))?
                else {
                    return Ok(());
                };
                self.require_active()?
                    .apply_paragraph(&draft)
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            VIEW_FORMATTING_MARKS | VIEW_HIDDEN_TEXT | VIEW_FIELD_CODES => {
                self.view = self.view.toggled(id);
                Ok(())
            }
            EDIT_PASTE => {
                let text = self.clipboard.clone();
                self.require_active()?
                    .replace_from(InputSource::Paste, None, &text)
                    .map_err(input_error)?;
                self.journal_active()
                    .map_err(|error| ExecuteError::Failed(error.to_string()))?;
                Ok(())
            }
            _ => Err(ExecuteError::Failed(format!(
                "{id} is not a document command"
            ))),
        }
    }
}
