//! Read-only T210 slice. Projection is intentionally the imported body, not
//! Word's final/revision view. Importer owns ECMA-376 parsing and preservation.
use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::ExitCode;

use joyoffice_opc::Budget;
use joyword_model::{Block, BlockSeq, BreakKind, InlineKind, Segment};
use serde_json::json;

const MAX_INPUT: u64 = 32 << 20;
const MAX_TEXT: usize = 1 << 20;
const HELP: &str = "joyword-cli inspect|text FILE [--max-text-bytes N]\nRead-only DOCX; N is UTF-8 text bytes (0..1048576), default 1048576.\nProjection: imported main body, INCLUDING hidden text; deleted revisions,\ncomments and other stories excluded; inserted/moveTo text included.\nUnsupported XML is preserved by import\nbut omitted from text. This is not a complete final/revision view.\ninspect: schema 1 JSON, partial and truncated are independent.\ntext: UTF-8 stdout; projection/partial/truncated JSON on stderr.\nExit: 0 complete within projection; 2 error; 3 text truncated.\nrender and convert are not implemented and are refused.\n";

struct Options {
    inspect: bool,
    path: OsString,
    limit: usize,
}

fn options() -> Result<Option<Options>, &'static str> {
    let mut args = std::env::args_os().skip(1);
    let Some(command) = args.next() else {
        return Err("missing command; use --help");
    };
    if command == "--help" || command == "-h" {
        return if args.next().is_none() {
            Ok(None)
        } else {
            Err("unexpected arguments")
        };
    }
    let inspect = if command == "inspect" {
        true
    } else if command == "text" {
        false
    } else {
        return Err("unsupported command; use inspect or text (render/convert unavailable)");
    };
    let path = args.next().ok_or("missing DOCX path")?;
    let mut limit = MAX_TEXT;
    if let Some(flag) = args.next() {
        if flag != "--max-text-bytes" {
            return Err("unknown option");
        }
        limit = args
            .next()
            .and_then(|value| value.to_str().and_then(|s| s.parse().ok()))
            .filter(|value| *value <= MAX_TEXT)
            .ok_or("invalid text budget (0..1048576)")?;
    }
    if args.next().is_some() {
        return Err("unexpected arguments");
    }
    Ok(Some(Options {
        inspect,
        path,
        limit,
    }))
}

struct Text {
    value: String,
    limit: usize,
    truncated: bool,
    omitted_objects: usize,
    paragraphs: usize,
}
impl Text {
    fn push(&mut self, ch: char) {
        if self.truncated {
            return;
        }
        if self.value.len() + ch.len_utf8() > self.limit {
            self.truncated = true;
        } else {
            self.value.push(ch);
        }
    }
    fn blocks(&mut self, blocks: &BlockSeq) {
        for block in blocks.iter() {
            match block {
                Block::Paragraph(paragraph) => {
                    if self.paragraphs != 0 {
                        self.push('\n');
                    }
                    self.paragraphs += 1;
                    for segment in paragraph.segments() {
                        match segment {
                            Segment::Text(value) => {
                                for ch in value.chars() {
                                    self.push(ch);
                                }
                            }
                            Segment::Inline(object) => match object.kind {
                                InlineKind::Break(BreakKind::Line) => self.push('\n'),
                                InlineKind::Break(_) => self.push('\u{000c}'),
                                _ => self.omitted_objects += 1,
                            },
                        }
                    }
                }
                Block::Table(table) => {
                    for row in &table.rows {
                        for cell in &row.cells {
                            self.blocks(&cell.blocks);
                        }
                    }
                }
                Block::Sdt(sdt) => self.blocks(&sdt.blocks),
                Block::Opaque(_) => self.omitted_objects += 1,
            }
        }
    }
}

fn execute(options: Options) -> Result<u8, &'static str> {
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
    let budget = Budget {
        max_entries: 1024,
        max_part_bytes: 16 << 20,
        max_total_bytes: 64 << 20,
        max_xml_depth: 128,
        max_xml_nodes: 1_000_000,
        max_relationships: 4096,
        ..Budget::default()
    };
    // Deliberately avoid echoing attacker-controlled package paths/XML in errors.
    let imported = joyword_docx::import(&bytes, &budget)
        .map_err(|_| "DOCX refused: invalid, unsupported or over budget")?;
    let mut text = Text {
        value: String::new(),
        limit: options.limit,
        truncated: false,
        omitted_objects: 0,
        paragraphs: 0,
    };
    text.blocks(&imported.document.body().blocks);
    let preserved = imported.backing.fragments().count();
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
        "partial": preserved != 0 || !imported.diagnostics.is_empty() || text.omitted_objects != 0,
        "truncated": text.truncated,
        "text_bytes_limit": options.limit,
        "text_bytes": text.value.len(),
        "paragraphs": text.paragraphs,
        "omitted_objects": text.omitted_objects,
        "preserved_fragments": preserved,
        "diagnostics": { "reported": imported.diagnostics.len(), "suppressed": imported.diagnostics.suppressed() },
        "read_only": imported.editability.is_read_only(),
    });
    let mut stdout = std::io::stdout().lock();
    if options.inspect {
        let mut report = report;
        report["text"] = json!(text.value);
        serde_json::to_writer(&mut stdout, &report).map_err(|_| "cannot write JSON")?;
        stdout.write_all(b"\n").map_err(|_| "cannot write stdout")?;
    } else {
        let mut stderr = std::io::stderr().lock();
        serde_json::to_writer(&mut stderr, &report)
            .map_err(|_| "cannot write projection metadata")?;
        stderr.write_all(b"\n").map_err(|_| "cannot write stderr")?;
        stdout
            .write_all(text.value.as_bytes())
            .map_err(|_| "cannot write stdout")?;
    }
    stdout.flush().map_err(|_| "cannot flush stdout")?;
    Ok(if text.truncated { 3 } else { 0 })
}

fn main() -> ExitCode {
    let result = match options() {
        Ok(Some(options)) => execute(options),
        Ok(None) => std::io::stdout()
            .write_all(HELP.as_bytes())
            .map(|()| 0)
            .map_err(|_| "cannot write help"),
        Err(error) => Err(error),
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            let _ = writeln!(std::io::stderr().lock(), "joyword-cli: {error}");
            ExitCode::from(2)
        }
    }
}
