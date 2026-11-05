//! Local schema 1 mail merge templates using the current edit session.

mod csv;

use joyword_engine::{
    Selection, Session,
    doc::{Document, Pos, StoryRef, para::OBJ},
    io::read_regular_file_within,
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const MAX_CSV: u64 = 8 << 20;
const MAX_RECORDS: usize = 1_000;
const MAX_TEMPLATE: u64 = 32 << 20;
const MAX_OUTPUT: usize = 128 << 20;

pub struct MergeJob {
    template: Document,
    fields: Vec<Field>,
    columns: BTreeMap<String, usize>,
    records: Vec<Vec<String>>,
}

struct Field {
    name: String,
    start: Pos,
    end: Pos,
}

pub struct Preview {
    pub values: BTreeMap<String, String>,
    pub text: String,
}

pub struct BatchReceipt {
    pub directory: PathBuf,
    pub files: Vec<PathBuf>,
    pub bytes: usize,
}

impl MergeJob {
    pub fn open(
        template: &Path,
        data: &Path,
        mapping: &BTreeMap<String, String>,
    ) -> Result<Self, String> {
        let template = read_regular_file_within(template, MAX_TEMPLATE)
            .ok_or("merge template must be a regular file of at most 32 MiB")?;
        let projection = joyword_docx::legacy_inspection::project(&template, 0)?;
        if !projection.unsigned_docx || projection.read_only || projection.preserved_fragments != 0
        {
            return Err("template contains protected or unsupported editable content".into());
        }
        let template = joyword_docx::read(&template).map_err(|error| error.to_string())?;
        if template.settings.track_changes
            || template.settings.protection.is_some()
            || !template.revisions.is_empty()
            || template.passthrough.contains_key("word/vbaProject.bin")
        {
            return Err(
                "mail merge requires an unprotected DOCX without macros or revisions".into(),
            );
        }
        let fields = collect_fields(&template)?;
        if fields.is_empty() {
            return Err("template has no {{field}} placeholders".into());
        }
        let data = read_regular_file_within(data, MAX_CSV)
            .ok_or("merge data must be a regular CSV file of at most 8 MiB")?;
        let data = csv::CsvData::parse(&data)?;
        let mut columns = BTreeMap::new();
        for field in &fields {
            if !columns.contains_key(&field.name) {
                let column = mapping.get(&field.name).unwrap_or(&field.name);
                let index = data
                    .headers
                    .iter()
                    .position(|header| header == column)
                    .ok_or_else(|| format!("missing CSV column for field {:?}", field.name))?;
                columns.insert(field.name.clone(), index);
            }
        }
        if let Some(unknown) = mapping.keys().find(|field| !columns.contains_key(*field)) {
            return Err(format!("unknown template field in mapping: {unknown:?}"));
        }
        Ok(Self {
            template,
            fields,
            columns,
            records: data.rows,
        })
    }

    pub fn rows(&self) -> usize {
        self.records.len()
    }

    fn record(&self, row: usize) -> Result<&[String], String> {
        self.records
            .get(row)
            .map(Vec::as_slice)
            .ok_or_else(|| "preview row is outside the data source".into())
    }

    fn merge(&self, record: &[String]) -> Result<Document, String> {
        let mut session = Session::new(self.template.clone());
        let mut expanded = 0;
        for field in self.fields.iter().rev() {
            let value = &record[self.columns[&field.name]];
            expanded += value.len();
            if expanded > 32 << 20 {
                return Err("merged text exceeds 32 MiB".into());
            }
            // Insert inside the first brace to inherit its formatting. Removing the original
            // braces in reverse source order keeps positions valid and treats CSV text literally.
            let inside = Pos {
                off: field.start.off + 1,
                ..field.start.clone()
            };
            if !value.is_empty() {
                session.sel = Selection::caret(inside.clone());
                session
                    .run("text.insert", &json!({"text": value, "raw": true}))
                    .map_err(|error| error.to_string())?;
                if session.sel.focus.path != field.start.path
                    || session.sel.focus.off != inside.off + value.len()
                {
                    return Err("merge value changed the template's paragraph structure".into());
                }
            }
            session.sel = Selection {
                anchor: Pos {
                    off: inside.off + value.len(),
                    ..inside.clone()
                },
                focus: Pos {
                    off: field.end.off + value.len(),
                    ..field.end.clone()
                },
            };
            session
                .run("text.delete", &json!({}))
                .map_err(|error| error.to_string())?;
            session.sel = Selection {
                anchor: field.start.clone(),
                focus: inside,
            };
            session
                .run("text.delete", &json!({}))
                .map_err(|error| error.to_string())?;
        }
        Ok(session.doc)
    }

    pub fn preview(&self, row: usize) -> Result<Preview, String> {
        let record = self.record(row)?;
        let document = self.merge(record)?;
        let values = self
            .columns
            .iter()
            .map(|(name, &column)| (name.clone(), record[column].clone()))
            .collect();
        let text = story_refs(&document)
            .map(|story| document.plain_text(story))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(Preview { values, text })
    }

    pub fn write_batch(&self, parent: &Path) -> Result<BatchReceipt, String> {
        let parent = parent.canonicalize().map_err(|error| error.to_string())?;
        let directory = tempfile::Builder::new()
            .prefix("JoyWord-merge-")
            .tempdir_in(parent)
            .map_err(|error| error.to_string())?;
        let result = (|| {
            let mut files = Vec::new();
            let mut total = 0;
            for (row, record) in self.records.iter().enumerate() {
                let doc = self.merge(record)?;
                let bytes = joyword_docx::write(&doc).map_err(|error| error.to_string())?;
                total += bytes.len();
                if total > MAX_OUTPUT {
                    return Err("merge output exceeds 128 MiB".into());
                }
                let reopened = joyword_docx::read(&bytes).map_err(|error| error.to_string())?;
                if reopened.plain_text(StoryRef::Body) != doc.plain_text(StoryRef::Body) {
                    return Err("merge DOCX read-back changed the body text".into());
                }
                let file = directory.path().join(format!("letter-{:04}.docx", row + 1));
                crate::reader::publish_new(&file, &bytes)?;
                files.push(file);
            }
            Ok((files, total))
        })();
        match result {
            Ok((files, bytes)) => Ok(BatchReceipt {
                directory: directory.keep(),
                files,
                bytes,
            }),
            Err(error) => {
                let path = directory.path().to_path_buf();
                directory.close().map_err(|cleanup| {
                    format!(
                        "{error}; cannot remove incomplete batch {}: {cleanup}",
                        path.display()
                    )
                })?;
                Err(error)
            }
        }
    }
}

fn story_refs(document: &Document) -> impl Iterator<Item = StoryRef> + '_ {
    std::iter::once(StoryRef::Body).chain(document.parts.keys().copied().map(StoryRef::Part))
}

fn collect_fields(document: &Document) -> Result<Vec<Field>, String> {
    let mut fields = Vec::new();
    for story in story_refs(document) {
        for path in document.para_paths(story) {
            let para = document
                .para(story, &path)
                .ok_or("invalid template paragraph")?;
            let mut offset = 0;
            let mut rest = para.text.as_str();
            while let Some((before, tail)) = rest.split_once("{{") {
                if before.contains("}}") {
                    return Err("unmatched template closing delimiter".into());
                }
                let (name, after) = tail.split_once("}}").ok_or("unclosed template field")?;
                let start = offset + before.len();
                offset = start + 4 + name.len();
                let name = name.trim();
                if name.is_empty()
                    || name.chars().count() > 128
                    || name
                        .chars()
                        .any(|c| c.is_control() || matches!(c, '{' | '}' | OBJ))
                    || !para.objects.is_empty()
                {
                    return Err("invalid template field or inline object in its paragraph".into());
                }
                if para.run_ranges().any(|(range, props)| {
                    range.start < offset
                        && range.end > start
                        && document
                            .styles
                            .resolve_char(para.props.style.as_deref(), props)
                            .hidden
                }) {
                    return Err("template field contains hidden text".into());
                }
                fields.push(Field {
                    name: name.into(),
                    start: Pos::new(story, path.clone(), start),
                    end: Pos::new(story, path.clone(), offset),
                });
                if fields.len() > 2048 {
                    return Err("template field budget exceeded".into());
                }
                rest = after;
            }
            if rest.contains("}}") {
                return Err("unmatched template closing delimiter".into());
            }
        }
    }
    Ok(fields)
}
