//! Schema 1 text exchange over the R8 document and format adapters.

use joyword_engine::{
    doc::{CharProps, Document, Paragraph, para_block},
    io::{open_bytes, read_regular_file_within, save_bytes},
};
use serde_json::json;
use std::{ffi::OsString, path::Path};

const MAX_INPUT: u64 = 4 << 20;
const MAX_OUTPUT: usize = 64 << 20;
const MAX_PARAGRAPHS: usize = 10_000;
const MAX_RUNS: usize = 20_000;
const USAGE: &str =
    "exchange FILE --from FORMAT --to FORMAT (--plan | --output FILE [--accept-losses])";

fn format(arg: &OsString) -> Result<&str, String> {
    match arg.to_str() {
        Some(name @ ("docx" | "txt" | "markdown" | "html" | "rtf")) => Ok(name),
        _ => Err("invalid exchange format".into()),
    }
}

pub fn run(args: &[OsString]) -> Result<u8, String> {
    if matches!(args, [arg] if arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(0);
    }
    let (input, mut args) = args.split_first().ok_or(USAGE)?;
    let (mut from, mut to, mut output) = (None, None, None);
    let (mut plan, mut accept) = (false, false);
    while let Some((flag, rest)) = args.split_first() {
        args = rest;
        match flag.to_str() {
            Some("--plan") if !plan => plan = true,
            Some("--accept-losses") if !accept => accept = true,
            Some("--from" | "--to" | "--output") => {
                let (value, rest) = args.split_first().ok_or("missing exchange option value")?;
                args = rest;
                let duplicate = if flag == "--from" {
                    from.replace(format(value)?).is_some()
                } else if flag == "--to" {
                    to.replace(format(value)?).is_some()
                } else {
                    output.replace(Path::new(value)).is_some()
                };
                if duplicate {
                    return Err("duplicate exchange option".into());
                }
            }
            _ => return Err("unknown or duplicate exchange option".into()),
        }
    }
    if plan == output.is_some() || (plan && accept) {
        return Err("choose --plan or --output [--accept-losses]".into());
    }
    let (from, to) = (from.ok_or("missing --from")?, to.ok_or("missing --to")?);
    if let Some(path) = output {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let valid = match to {
            "html" => matches!(extension.as_str(), "html" | "htm"),
            "markdown" => matches!(extension.as_str(), "md" | "markdown"),
            _ => extension == to,
        };
        if !valid {
            return Err("output extension does not match exchange format".into());
        }
    }
    let input = read_regular_file_within(Path::new(input), MAX_INPUT)
        .ok_or("exchange requires a regular input file of at most 4 MiB")?;
    let source = import(from, &input)?;
    let document = paragraph_projection(&source)?;
    let bytes = save_bytes(&format!("exchange.{to}"), &document)?;
    if bytes.len() > MAX_OUTPUT {
        return Err("exchange output exceeds 64 MiB budget".into());
    }
    let mut report = json!({
        "schema_version": 1, "mode": "plan", "from": from, "to": to,
        "bytes": bytes.len(), "paragraphs": document.body.len(),
        "losses": [{"kind": "FormattingDropped", "description":
            "Paragraph text and basic character formatting only. Page layout, fonts, styles, metadata and unsupported source syntax are not preserved."}],
        "writes": 0, "source_overwritten": false,
    });
    let mut status = 0;
    if let Some(path) = output {
        if accept {
            joyword_shell::reader::publish_new(path, &bytes)?;
            report["mode"] = json!("export");
            report["output"] = json!(path);
            report["writes"] = json!(1);
        } else {
            status = 2;
        }
    }
    super::report(report)?;
    Ok(status)
}

fn import(format: &str, bytes: &[u8]) -> Result<Document, String> {
    match format {
        "docx" => {
            let projection = joyword_docx::legacy_inspection::project(bytes, 0)?;
            if projection.read_only || projection.preserved_fragments != 0 {
                return Err(
                    "restricted or partially imported DOCX cannot use text exchange".into(),
                );
            }
            joyword_docx::read(bytes).map_err(|error| error.to_string())
        }
        "rtf" => open_bytes("exchange.rtf", bytes),
        _ => {
            let text = decode_text(bytes)?;
            if text.lines().count() > MAX_PARAGRAPHS
                || text
                    .bytes()
                    .filter(|byte| matches!(byte, b'<' | b'*' | b'`' | b'['))
                    .count()
                    > MAX_RUNS
            {
                return Err("text syntax work budget exceeded".into());
            }
            if format == "html" && html_requires_projection(&text) {
                return Err(
                    "styled, hidden or reviewed HTML requires an explicit projection".into(),
                );
            }
            // The strict decoding above is authoritative, including for HTML with a
            // conflicting charset declaration.
            let mut utf8 = String::from("\u{feff}");
            utf8.push_str(&text);
            open_bytes(&format!("exchange.{format}"), utf8.as_bytes())
        }
    }
}

fn decode_text(bytes: &[u8]) -> Result<String, String> {
    const INVALID: &str = "invalid exchange text encoding";
    let text = if let Some((little_endian, bytes)) = bytes
        .strip_prefix(&[0xff, 0xfe])
        .map(|bytes| (true, bytes))
        .or_else(|| {
            bytes
                .strip_prefix(&[0xfe, 0xff])
                .map(|bytes| (false, bytes))
        }) {
        let (pairs, remainder) = bytes.as_chunks::<2>();
        if !remainder.is_empty() {
            return Err(INVALID.into());
        }
        let units = pairs.iter().map(|&pair| {
            if little_endian {
                u16::from_le_bytes(pair)
            } else {
                u16::from_be_bytes(pair)
            }
        });
        char::decode_utf16(units)
            .collect::<Result<String, _>>()
            .map_err(|_| INVALID)?
    } else {
        std::str::from_utf8(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes))
            .map_err(|_| INVALID)?
            .to_owned()
    };
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err("unsupported control character in text input".into());
    }
    Ok(text)
}

fn html_requires_projection(input: &str) -> bool {
    // Scan through the end of quoted attribute values as well as bare names.
    input.split('<').skip(1).any(|tail| {
        let mut quoted = None;
        let tag = tail
            .split_once(|c| match (quoted, c) {
                (None, '"' | '\'') => {
                    quoted = Some(c);
                    false
                }
                (Some(quote), c) if quote == c => {
                    quoted = None;
                    false
                }
                (None, '>') => true,
                _ => false,
            })
            .map_or(tail, |(tag, _)| tag);
        let mut tokens = tag
            .split(|c: char| c.is_ascii_whitespace() || matches!(c, '=' | '/'))
            .filter(|s| !s.is_empty());
        tokens.next().is_some_and(|tag| {
            ["style", "link", "del", "ins", "template", "noscript"]
                .iter()
                .any(|name| tag.eq_ignore_ascii_case(name))
        }) || tokens.any(|token| {
            ["style", "class", "hidden", "aria-hidden"]
                .iter()
                .any(|name| token.eq_ignore_ascii_case(name))
        })
    })
}

fn paragraph_projection(source: &Document) -> Result<Document, String> {
    if !source.comments.is_empty()
        || !source.revisions.is_empty()
        || !source.parts.is_empty()
        || source.settings.track_changes
        || source.settings.protection.is_some()
    {
        return Err(
            "reviewed or protected content and additional stories require an explicit projection"
                .into(),
        );
    }
    if source.body.len() > MAX_PARAGRAPHS {
        return Err("exchange paragraph budget exceeded".into());
    }
    let mut document = Document::new();
    document.body.clear();
    let mut runs = 0;
    for block in &source.body {
        let paragraph = block
            .as_para()
            .ok_or("text exchange requires paragraph-only content")?;
        if !paragraph.objects.is_empty() {
            return Err("objects and content controls require an explicit projection".into());
        }
        let mut projected = Paragraph::new();
        runs += paragraph.runs.len();
        if runs > MAX_RUNS {
            return Err("exchange run budget exceeded".into());
        }
        for (range, props) in paragraph.run_ranges() {
            let style = source
                .styles
                .resolve_char(paragraph.props.style.as_deref(), props);
            if style.hidden || style.del.is_some() || style.ins.is_some() || style.fmt.is_some() {
                return Err("hidden or reviewed text requires an explicit projection".into());
            }
            let text = paragraph.text.get(range).ok_or("invalid text run")?;
            if text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
            {
                return Err("special breaks require an explicit projection".into());
            }
            projected
                .insert_text(
                    projected.text.len(),
                    text,
                    &CharProps {
                        bold: Some(style.bold),
                        italic: Some(style.italic),
                        strike: Some(style.strike || style.double_strike),
                        ..Default::default()
                    },
                )
                .map_err(|error| error.to_string())?;
        }
        document.body.push(para_block(projected));
    }
    document.ensure_nonempty();
    Ok(document)
}
