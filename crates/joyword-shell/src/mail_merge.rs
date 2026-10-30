//! Bounded, local mail merge (EDIT-012). CSV follows RFC 4180 quoting, with
//! UTF-8/BOM and LF records also accepted. `{{field}}` is JoyWord's explicit
//! text-template syntax, not a claim to evaluate Word MERGEFIELD instructions.
//! All document edits go through engine transactions and verified DOCX export.
mod csv;

use crate::docx::DocxSession;
use joyword_engine::{Command, Transaction};
use joyword_model::{Bias, Block, BlockSeq, BreakKind, InlineKind, Position, ScalarOffset};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub use csv::CsvData;

#[derive(Clone, Debug)]
pub struct Limits {
    pub csv_bytes: usize,
    pub template_bytes: usize,
    pub rows: usize,
    pub columns: usize,
    pub cell_bytes: usize,
    pub fields: usize,
    pub output_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            csv_bytes: 8 << 20,
            template_bytes: 32 << 20,
            rows: 1000,
            columns: 128,
            cell_bytes: 16 << 10,
            fields: 2048,
            output_bytes: 128 << 20,
        }
    }
}

impl Limits {
    fn validate(&self) -> Result<(), MergeError> {
        let cap = Self::default();
        for (value, maximum) in [
            (self.csv_bytes, cap.csv_bytes),
            (self.template_bytes, cap.template_bytes),
            (self.rows, cap.rows),
            (self.columns, cap.columns),
            (self.cell_bytes, cap.cell_bytes),
            (self.fields, cap.fields),
            (self.output_bytes, cap.output_bytes),
        ] {
            if value == 0 || value > maximum {
                return Err(MergeError::Invalid("invalid mail merge budget"));
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum MergeError {
    Invalid(&'static str),
    Csv {
        record: usize,
        reason: &'static str,
    },
    MissingColumn(String),
    UnknownField(String),
    Document(String),
    Io(std::io::Error),
    Cancelled,
    Cleanup {
        directory: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(reason) => f.write_str(reason),
            Self::Csv { record, reason } => write!(f, "CSV record {record}: {reason}"),
            Self::MissingColumn(field) => write!(f, "missing CSV column for field {field:?}"),
            Self::UnknownField(field) => write!(f, "unknown template field in mapping: {field:?}"),
            Self::Document(reason) => write!(f, "template/export refused: {reason}"),
            Self::Io(error) => write!(f, "local mail merge I/O: {error}"),
            Self::Cancelled => f.write_str("mail merge cancelled"),
            Self::Cleanup { directory, source } => write!(
                f,
                "merge failed; remove incomplete directory {}: {source}",
                directory.display()
            ),
        }
    }
}

impl std::error::Error for MergeError {}
impl From<std::io::Error> for MergeError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, Debug)]
struct Field {
    name: String,
    start: Position,
    end: usize,
}

#[derive(Clone)]
pub struct MergeTemplate {
    bytes: Arc<[u8]>,
    fields: Vec<Field>,
    names: Vec<String>,
    limits: Limits,
}

fn document_error(error: impl fmt::Display) -> MergeError {
    MergeError::Document(error.to_string())
}

impl MergeTemplate {
    pub fn parse(bytes: &[u8], limits: &Limits) -> Result<Self, MergeError> {
        limits.validate()?;
        if bytes.len() > limits.template_bytes {
            return Err(MergeError::Invalid("template byte budget exceeded"));
        }
        let session = DocxSession::open_default(bytes).map_err(document_error)?;
        if !session.editability().is_unrestricted() {
            return Err(MergeError::Invalid(
                "template contains protected or unsupported editable content",
            ));
        }
        if session.session().document().settings().track_revisions == Some(true) {
            return Err(MergeError::Invalid(
                "tracked templates must be resolved before mail merge",
            ));
        }
        if session.export_plan().map_err(document_error)?.target()
            != joyword_docx::ExportTarget::Docx
        {
            return Err(MergeError::Invalid(
                "mail merge requires a non-macro DOCX template",
            ));
        }
        let mut fields = Vec::new();
        for story in session.session().document().stories() {
            collect(&story.blocks, story.id, 0, &mut fields, limits)?;
        }
        if fields.is_empty() {
            return Err(MergeError::Invalid(
                "template has no {{field}} placeholders",
            ));
        }
        let names = fields
            .iter()
            .map(|field| field.name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(Self {
            bytes: Arc::from(bytes),
            fields,
            names,
            limits: limits.clone(),
        })
    }

    pub fn fields(&self) -> &[String] {
        &self.names
    }

    pub fn bind(
        &self,
        data: CsvData,
        mapping: &BTreeMap<String, String>,
    ) -> Result<MergeJob, MergeError> {
        for field in mapping.keys() {
            if !self.names.contains(field) {
                return Err(MergeError::UnknownField(field.clone()));
            }
        }
        let mut columns = BTreeMap::new();
        for field in &self.names {
            let column = mapping.get(field).unwrap_or(field);
            let index = data
                .headers
                .iter()
                .position(|header| header == column)
                .ok_or_else(|| MergeError::MissingColumn(field.clone()))?;
            columns.insert(field.clone(), index);
        }
        if data.rows.len() > self.limits.rows
            || data.headers.len() > self.limits.columns
            || data.bytes > self.limits.csv_bytes
            || data
                .headers
                .iter()
                .chain(data.rows.iter().flatten())
                .any(|value| value.len() > self.limits.cell_bytes)
        {
            return Err(MergeError::Invalid("CSV exceeds template job budget"));
        }
        Ok(MergeJob {
            template: self.clone(),
            data,
            columns,
        })
    }
}

fn collect(
    blocks: &BlockSeq,
    story: joyword_model::StoryId,
    depth: usize,
    fields: &mut Vec<Field>,
    limits: &Limits,
) -> Result<(), MergeError> {
    if depth > joyword_model::MAX_NESTING {
        return Err(MergeError::Invalid("template nesting budget exceeded"));
    }
    for block in blocks.iter() {
        match block {
            Block::Paragraph(paragraph) => {
                if paragraph.mark_revision.is_some()
                    || paragraph.props_revision.is_some()
                    || paragraph
                        .runs()
                        .iter()
                        .any(|run| run.revision.is_some() || run.format_revision.is_some())
                {
                    return Err(MergeError::Invalid(
                        "template contains unresolved revisions",
                    ));
                }
                // Engine positions count inline objects as one scalar. Keep
                // those markers while finding fields, including before them;
                // a field crossing an object is rejected by name validation.
                let text = paragraph.text().to_raw_string();
                let initial_fields = fields.len();
                let mut at = 0;
                let mut scalar = 0;
                while at < text.len() {
                    let rest = &text[at..];
                    let Some(open) = rest.find("{{") else {
                        if rest.contains("}}") {
                            return Err(MergeError::Invalid(
                                "unmatched template closing delimiter",
                            ));
                        }
                        break;
                    };
                    if rest[..open].contains("}}") {
                        return Err(MergeError::Invalid("unmatched template closing delimiter"));
                    }
                    scalar += rest[..open].chars().count();
                    at += open;
                    let end = text[at + 2..]
                        .find("}}")
                        .ok_or(MergeError::Invalid("unclosed template field"))?
                        + at
                        + 2;
                    let name = text[at + 2..end].trim();
                    if name.is_empty()
                        || name.chars().count() > 128
                        || name.chars().any(|ch| {
                            ch.is_control()
                                || matches!(ch, '{' | '}' | joyword_model::OBJECT_MARKER)
                        })
                    {
                        return Err(MergeError::Invalid("invalid template field name"));
                    }
                    let length = text[at..end + 2].chars().count();
                    fields.push(Field {
                        name: name.to_owned(),
                        start: Position {
                            story,
                            paragraph: paragraph.id(),
                            offset: ScalarOffset(scalar),
                            bias: Bias::After,
                        },
                        end: scalar + length,
                    });
                    if fields.len() > limits.fields {
                        return Err(MergeError::Invalid("template field budget exceeded"));
                    }
                    scalar += length;
                    at = end + 2;
                }
                if fields.len() != initial_fields && !paragraph.inlines().is_empty() {
                    return Err(MergeError::Invalid(
                        "template fields in paragraphs with existing inline objects are unsupported",
                    ));
                }
            }
            Block::Table(table) => {
                if table.props_revision.is_some()
                    || table.rows.iter().any(|row| {
                        row.props_revision.is_some()
                            || row.cells.iter().any(|cell| cell.props_revision.is_some())
                    })
                {
                    return Err(MergeError::Invalid(
                        "template contains unresolved revisions",
                    ));
                }
                for row in &table.rows {
                    for cell in &row.cells {
                        collect(&cell.blocks, story, depth + 1, fields, limits)?;
                    }
                }
            }
            Block::Sdt(sdt) => {
                if !sdt.props.lock.content_editable() {
                    return Err(MergeError::Invalid(
                        "template contains locked content controls",
                    ));
                }
                collect(&sdt.blocks, story, depth + 1, fields, limits)?;
            }
            Block::Opaque(_) => {
                return Err(MergeError::Invalid(
                    "opaque template blocks cannot be merged",
                ));
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct MergeJob {
    template: MergeTemplate,
    data: CsvData,
    columns: BTreeMap<String, usize>,
}

pub struct Preview {
    pub row: usize,
    pub values: BTreeMap<String, String>,
    pub text: String,
    pub docx: Vec<u8>,
}

pub struct BatchReceipt {
    pub directory: PathBuf,
    pub files: Vec<PathBuf>,
    pub bytes: usize,
}

impl MergeJob {
    pub fn rows(&self) -> usize {
        self.data.rows.len()
    }

    pub fn preview(&self, row: usize) -> Result<Preview, MergeError> {
        let record = self.data.rows.get(row).ok_or(MergeError::Invalid(
            "preview row is outside the data source",
        ))?;
        let mut session =
            DocxSession::open_default(&self.template.bytes).map_err(document_error)?;
        let mut values = BTreeMap::new();
        for (name, column) in &self.columns {
            values.insert(name.clone(), record[*column].clone());
        }
        // Reverse source order keeps every original position valid. Values are
        // never scanned as template syntax, so a CSV cell cannot inject fields.
        let mut commands = Vec::new();
        for field in self.template.fields.iter().rev() {
            let value = &values[&field.name];
            let mut at = field.start;
            at.offset.0 += 1;
            let mut pending = String::new();
            let mut value_len = 0;
            for ch in value.chars() {
                if ch == '\n' {
                    if !pending.is_empty() {
                        let len = pending.chars().count();
                        commands.push(Command::InsertText {
                            at,
                            text: std::mem::take(&mut pending),
                        });
                        at.offset.0 += len;
                    }
                    commands.push(Command::InsertInline {
                        at,
                        kind: InlineKind::Break(BreakKind::Line),
                    });
                    at.offset.0 += 1;
                } else {
                    pending.push(ch);
                }
                value_len += 1;
            }
            if !pending.is_empty() {
                commands.push(Command::InsertText { at, text: pending });
            }
            // Insert inside the first opening brace to inherit its full run
            // formatting, then remove the original suffix and first brace.
            let mut suffix = field.start;
            suffix.offset.0 += 1 + value_len;
            let mut end = field.start;
            end.offset.0 = field.end + value_len;
            commands.push(Command::DeleteRange { start: suffix, end });
            let mut brace_end = field.start;
            brace_end.offset.0 += 1;
            commands.push(Command::DeleteRange {
                start: field.start,
                end: brace_end,
            });
            if commands.len() > 32768 {
                return Err(MergeError::Invalid("merge command budget exceeded"));
            }
        }
        session
            .apply(Transaction::new(session.session().version(), commands))
            .map_err(document_error)?;
        let plan = session.export_plan().map_err(document_error)?;
        if plan.bytes().len() > self.template.limits.output_bytes {
            return Err(MergeError::Invalid("output byte budget exceeded"));
        }
        let mut paragraphs = Vec::new();
        for story in session.session().document().stories() {
            preview_text(&story.blocks, &mut paragraphs);
        }
        Ok(Preview {
            row,
            values,
            text: paragraphs.join("\n"),
            docx: plan.bytes().to_vec(),
        })
    }

    /// Each batch owns a new directory, never a caller's existing file. Error
    /// or cancellation explicitly closes it; cleanup failures report its path.
    /// Temporary directory visibility/power-loss atomicity is not promised.
    pub fn write_batch(
        &self,
        parent: &Path,
        cancel: &AtomicBool,
        mut progress: impl FnMut(usize),
    ) -> Result<BatchReceipt, MergeError> {
        if cancel.load(Ordering::Acquire) {
            return Err(MergeError::Cancelled);
        }
        let parent = std::fs::canonicalize(parent)?;
        let directory = tempfile::Builder::new()
            .prefix("JoyWord-merge-")
            .tempdir_in(parent)?;
        let path = directory.path().to_path_buf();
        let mut files = Vec::new();
        let mut total = 0usize;
        let result = (|| {
            for row in 0..self.rows() {
                if cancel.load(Ordering::Acquire) {
                    return Err(MergeError::Cancelled);
                }
                let preview = self.preview(row)?;
                total = total
                    .checked_add(preview.docx.len())
                    .ok_or(MergeError::Invalid("output byte budget exceeded"))?;
                if total > self.template.limits.output_bytes {
                    return Err(MergeError::Invalid("output byte budget exceeded"));
                }
                let name = path.join(format!("letter-{:04}.docx", row + 1));
                let mut file = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .open(&name)?;
                file.write_all(&preview.docx)?;
                file.sync_all()?;
                crate::storage::verify_bytes(&mut file, &preview.docx)?;
                files.push(name);
                progress(row + 1);
            }
            if cancel.load(Ordering::Acquire) {
                return Err(MergeError::Cancelled);
            }
            Ok(())
        })();
        if let Err(error) = result {
            directory.close().map_err(|source| MergeError::Cleanup {
                directory: path,
                source,
            })?;
            return Err(error);
        }
        let directory = directory.keep();
        Ok(BatchReceipt {
            directory,
            files,
            bytes: total,
        })
    }
}

fn preview_text(blocks: &BlockSeq, lines: &mut Vec<String>) {
    for block in blocks.iter() {
        match block {
            Block::Paragraph(p) => lines.push(p.plain_text(|inline| match &inline.kind {
                InlineKind::Break(_) => "\n".into(),
                InlineKind::PageField(field) => {
                    field.cached.as_deref().unwrap_or("[page]").to_owned()
                }
                _ => "[object]".into(),
            })),
            Block::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        preview_text(&cell.blocks, lines);
                    }
                }
            }
            Block::Sdt(s) => preview_text(&s.blocks, lines),
            Block::Opaque(_) => {}
        }
    }
}

/// Bounded snapshots: no file dialogs, external data resolution, or network.
pub fn read_local(path: &Path, maximum: usize) -> Result<Vec<u8>, MergeError> {
    if maximum == 0 || maximum > Limits::default().template_bytes {
        return Err(MergeError::Invalid("invalid local input budget"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(MergeError::Invalid("local input byte budget exceeded"));
    }
    Ok(bytes)
}
