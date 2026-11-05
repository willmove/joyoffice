//! Legacy mail merge option syntax and schema 1 reports.
use joyword_shell::mail_merge::MergeJob;
use serde_json::json;
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};

pub(crate) struct Options {
    template: PathBuf,
    data: PathBuf,
    mapping: BTreeMap<String, String>,
    mode: Mode,
}
enum Mode {
    Preview(usize),
    Batch(PathBuf),
}

pub(crate) fn options(
    mut args: impl Iterator<Item = OsString>,
) -> Result<Option<Options>, &'static str> {
    let mut template = None;
    let mut data = None;
    let mut mode = None;
    let mut mapping = BTreeMap::new();
    let mut any = false;
    while let Some(option) = args.next() {
        if (option == "--help" || option == "-h") && !any && args.next().is_none() {
            return Ok(None);
        }
        any = true;
        let value = args.next().ok_or("merge option needs a value")?;
        if option == "--template" && template.is_none() {
            template = Some(PathBuf::from(value));
        } else if option == "--data" && data.is_none() {
            data = Some(PathBuf::from(value));
        } else if option == "--preview-row" && mode.is_none() {
            let number = value
                .to_str()
                .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|n| (1..=1000).contains(n))
                .ok_or("preview row must be 1..1000")?;
            mode = Some(Mode::Preview(number - 1));
        } else if option == "--output-parent" && mode.is_none() {
            mode = Some(Mode::Batch(value.into()));
        } else if option == "--map" {
            let (field, column) = value
                .to_str()
                .and_then(|s| s.split_once('='))
                .filter(|(field, column)| !field.is_empty() && !column.is_empty())
                .ok_or("mapping must be FIELD=COLUMN")?;
            if mapping.insert(field.into(), column.into()).is_some() {
                return Err("duplicate field mapping");
            }
        } else {
            return Err("unknown, duplicate or conflicting merge option");
        }
    }
    Ok(Some(Options {
        template: template.ok_or("merge needs --template")?,
        data: data.ok_or("merge needs --data")?,
        mapping,
        mode: mode.ok_or("merge needs --preview-row or --output-parent")?,
    }))
}

pub fn run(args: &[OsString]) -> Result<u8, String> {
    let Some(options) = options(args.iter().cloned())? else {
        println!(
            "merge --template DOCX --data CSV [--map FIELD=COLUMN] (--preview-row N | --output-parent DIR)"
        );
        return Ok(0);
    };
    let job = MergeJob::open(&options.template, &options.data, &options.mapping)?;
    super::report(match options.mode {
        Mode::Preview(row) => {
            let preview = job.preview(row)?;
            json!({"schema_version": 1, "mode": "mail-merge-preview", "row": row + 1,
                "records": job.rows(), "values": preview.values, "text": preview.text, "writes": 0})
        }
        Mode::Batch(parent) => {
            let result = job.write_batch(&parent)?;
            json!({"schema_version": 1, "mode": "mail-merge-batch", "records": result.files.len(),
                "directory": result.directory, "files": result.files, "bytes": result.bytes,
                "template_overwritten": false})
        }
    })?;
    Ok(0)
}
