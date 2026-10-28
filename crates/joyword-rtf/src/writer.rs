//! RTF 1.9 document generator producing valid standard RTF bytes (T600).

use joyword_model::{Block, Document, Run};
use joyword_textfmt::{LossKind, LossReport};

pub fn export_rtf(doc: &Document) -> (String, LossReport) {
    let mut report = LossReport::new();
    let mut out = String::new();

    // Standard RTF 1.9 Header
    out.push_str("{\\rtf1\\ansi\\ansicpg936\\deff0\\nouicompat\n");
    out.push_str(
        "{\\fonttbl{\\f0\\fnil\\fcharset134 Songti;}{\\f1\\fnil\\fcharset0 Times New Roman;}}\n",
    );

    let body = doc.body();
    for block in body.blocks.iter() {
        match block {
            Block::Paragraph(p) => {
                let chars: Vec<char> = p.raw_text().chars().collect();
                let mut offset = 0;
                for run in p.runs() {
                    let end = (offset + run.len).min(chars.len());
                    let run_text: String = chars[offset..end].iter().collect();
                    offset = end;
                    render_run_rtf(run, &run_text, &mut out);
                }
                out.push_str("\\par\n");
            }
            Block::Table(tbl) => {
                for row in &tbl.rows {
                    out.push_str("\\trowd\\trautofit1\n");
                    let mut cell_x = 1440;
                    for _ in &row.cells {
                        out.push_str(&format!("\\cellx{cell_x}"));
                        cell_x += 1440;
                    }
                    out.push('\n');

                    for cell in &row.cells {
                        out.push_str("\\intbl ");
                        for b in cell.blocks.iter() {
                            if let Block::Paragraph(p) = b {
                                out.push_str(&escape_rtf_text(&p.raw_text()));
                            }
                        }
                        out.push_str("\\cell\n");
                    }
                    out.push_str("\\row\n");
                }
            }
            Block::Sdt(sdt) => {
                for b in sdt.blocks.iter() {
                    if let Block::Paragraph(p) = b {
                        out.push_str(&escape_rtf_text(&p.raw_text()));
                        out.push_str("\\par\n");
                    }
                }
            }
            Block::Opaque(_) => {
                report.record(
                    LossKind::UnsupportedElement,
                    "Opaque block omitted from RTF export",
                );
            }
        }
    }

    out.push('}');
    (out, report)
}

fn render_run_rtf(run: &Run, text: &str, out: &mut String) {
    let is_bold = run.props.bold == Some(true);
    let is_italic = run.props.italic == Some(true);
    let is_strike = run.props.strike == Some(true);

    if is_bold {
        out.push_str("\\b ");
    }
    if is_italic {
        out.push_str("\\i ");
    }
    if is_strike {
        out.push_str("\\strike ");
    }

    out.push_str(&escape_rtf_text(text));

    if is_strike {
        out.push_str("\\strike0 ");
    }
    if is_italic {
        out.push_str("\\i0 ");
    }
    if is_bold {
        out.push_str("\\b0 ");
    }
}

pub fn escape_rtf_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\line "),
            '\t' => out.push_str("\\tab "),
            _ => {
                let code = c as u32;
                if code < 128 {
                    out.push(c);
                } else if code <= 0xFFFF {
                    let signed = if code > 32767 {
                        (code as i32) - 65536
                    } else {
                        code as i32
                    };
                    out.push_str(&format!("\\u{signed}?"));
                } else {
                    // Surrogate pair for astral plane
                    let cp = code - 0x10000;
                    let high = 0xD800 + ((cp >> 10) & 0x3FF);
                    let low = 0xDC00 + (cp & 0x3FF);
                    out.push_str(&format!("\\u{}?\\u{}?", (high as i16), (low as i16)));
                }
            }
        }
    }
    out
}
