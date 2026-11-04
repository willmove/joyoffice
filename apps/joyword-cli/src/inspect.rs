//! Pre-R8 schema and command-line contract (baseline 00fb0687).
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    io::{Read, Write},
};

const MAX_INPUT: u64 = 32 << 20;
const MAX_TEXT: usize = 1 << 20;
const HELP: &str = concat!(
    "joyword-cli inspect|text [--max-text-bytes N] [--compatibility] [--] FILE\n",
    "Options may precede or follow FILE. -- ends option parsing.\n",
    "UTF-8 byte budget: 0..1048576, default 1048576.\n",
    "inspect writes schema 1 JSON. text writes UTF-8 to stdout and metadata to stderr.\n",
    "Exit: 0 complete, 2 error, 3 truncated.\n",
    "Use inspect|text --session [--] INPUT for the R8 session projection.\n",
);

pub fn run(inspect: bool, args: &[OsString]) -> Result<u8, &'static str> {
    match options(inspect, args.iter().cloned())? {
        Some(options) => execute_text(options),
        None => std::io::stdout()
            .lock()
            .write_all(HELP.as_bytes())
            .map(|()| 0)
            .map_err(|_| "cannot write help"),
    }
}

/// Recursively re-insert every JSON object's keys in ascending order.
///
/// `serde_json::Map` is a sorted `BTreeMap` in a default-member build, but
/// `cargo test --workspace` pulls in `apps/joyword`, whose `gpui-pre`
/// dependency enables `serde_json/preserve_order`; feature unification then
/// switches the shared build to an insertion-ordered `IndexMap` and the CLI's
/// golden output bytes would silently change with it (T210 requires
/// byte-identical output for identical input). Sorting through a `BTreeMap`
/// makes the serialized key order identical under every feature combination:
/// a no-op for `BTreeMap`, canonical for `IndexMap`.
pub(crate) fn canonicalize_keys(value: &mut Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                canonicalize_keys(item);
            }
        }
        Value::Object(map) => {
            let sorted: std::collections::BTreeMap<String, Value> = std::mem::take(map)
                .into_iter()
                .map(|(key, mut item)| {
                    canonicalize_keys(&mut item);
                    (key, item)
                })
                .collect();
            for (key, item) in sorted {
                map.insert(key, item);
            }
        }
        Value::Number(_) | Value::String(_) | Value::Bool(_) | Value::Null => {}
    }
}

struct TextOptions {
    inspect: bool,
    path: OsString,
    limit: usize,
    compatibility: bool,
}

fn options(
    inspect: bool,
    mut args: impl Iterator<Item = OsString>,
) -> Result<Option<TextOptions>, &'static str> {
    let mut path = None;
    let mut limit = MAX_TEXT;
    let mut budget_seen = false;
    let mut compatibility = false;
    let mut positional_only = false;
    while let Some(arg) = args.next() {
        if !positional_only && arg == "--" {
            positional_only = true;
        } else if !positional_only && (arg == "--help" || arg == "-h") {
            return if path.is_none() && !budget_seen && args.next().is_none() {
                Ok(None)
            } else {
                Err("unexpected arguments")
            };
        } else if !positional_only && arg == "--compatibility" {
            if compatibility {
                return Err("duplicate compatibility option");
            }
            compatibility = true;
        } else if !positional_only && arg == "--max-text-bytes" {
            if budget_seen {
                return Err("duplicate text budget");
            }
            budget_seen = true;
            limit = args
                .next()
                .and_then(|value| value.to_str().and_then(|s| s.parse().ok()))
                .filter(|value| *value <= MAX_TEXT)
                .ok_or("invalid text budget (0..1048576)")?;
        } else if !positional_only && arg.as_encoded_bytes().starts_with(b"-") {
            return Err("unknown option; use -- before a path starting with '-'");
        } else if path.replace(arg).is_some() {
            return Err("unexpected arguments");
        }
    }
    let path = path.ok_or("missing DOCX path")?;
    Ok(Some(TextOptions {
        inspect,
        path,
        limit,
        compatibility,
    }))
}

fn execute_text(options: TextOptions) -> Result<u8, &'static str> {
    if !std::fs::metadata(&options.path)
        .map_err(|_| "cannot inspect input")?
        .is_file()
    {
        return Err("input must be a regular file");
    }
    let file = std::fs::File::open(&options.path).map_err(|_| "cannot open input")?;
    if !file
        .metadata()
        .map_err(|_| "cannot inspect input")?
        .is_file()
    {
        return Err("input must be a regular file");
    }
    let mut bytes = Vec::new();
    file.take(MAX_INPUT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read input")?;
    if bytes.len() as u64 > MAX_INPUT {
        return Err("input exceeds 32 MiB budget");
    }
    let text = joyword_docx::legacy_inspection::project(&bytes, options.limit)
        .map_err(|_| "DOCX refused: invalid, unsupported or over budget")?;
    let preserved = text.preserved_fragments;
    let report = json!({
        "schema_version": 1,
        "projection": {
            "name": "imported-main-body",
            "hidden_text": "included",
            "deleted_revisions": "excluded",
            "inserted_revisions": "included",
            "moved_to_revisions": "included",
            "moved_from_revisions": "excluded",
            "comments": "excluded",
            "other_stories": "excluded",
            "unsupported_xml": "omitted-preserved-in-source",
            "paragraph_separator": "LF",
            "cell_separator": "paragraph-LF",
            "line_break": "LF",
            "page_column_break": "FF",
        },
        "partial": preserved != 0 || text.diagnostics != 0 || text.omitted_objects != 0,
        "truncated": text.truncated,
        "text_bytes_limit": options.limit,
        "text_bytes": text.text.len(),
        "paragraphs": text.paragraphs,
        "omitted_objects": text.omitted_objects,
        "preserved_fragments": preserved,
        "diagnostics": { "reported": text.diagnostics.min(1024), "suppressed": text.diagnostics.saturating_sub(1024) },
        "read_only": text.read_only,
    });
    let mut report = report;
    if options.compatibility {
        report["compatibility"] = json!({
            "style_approximation_required": text.style_approximation_required,
        });
    }
    if options.inspect {
        report["text"] = json!(text.text);
    }
    canonicalize_keys(&mut report);
    let mut stdout = std::io::stdout().lock();
    if options.inspect {
        serde_json::to_writer(&mut stdout, &report).map_err(|_| "cannot write JSON")?;
        stdout.write_all(b"\n").map_err(|_| "cannot write stdout")?;
    } else {
        let mut stderr = std::io::stderr().lock();
        serde_json::to_writer(&mut stderr, &report)
            .map_err(|_| "cannot write projection metadata")?;
        stderr.write_all(b"\n").map_err(|_| "cannot write stderr")?;
        stderr.flush().map_err(|_| "cannot flush stderr")?;
        stdout
            .write_all(text.text.as_bytes())
            .map_err(|_| "cannot write stdout")?;
    }
    stdout.flush().map_err(|_| "cannot flush stdout")?;
    Ok(if text.truncated { 3 } else { 0 })
}
