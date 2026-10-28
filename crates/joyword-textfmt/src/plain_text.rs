//! Plain text import and export with newline normalization and loss reports.

use std::sync::Arc;

use joyword_model::{Block, Document, Paragraph, SeqTree};

use crate::encoding::{DetectedEncoding, decode_and_normalize};
use crate::loss::{LossKind, LossReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlainTextOptions {
    pub line_ending: LineEnding,
}

impl Default for PlainTextOptions {
    fn default() -> Self {
        Self {
            line_ending: LineEnding::Lf,
        }
    }
}

pub fn import_plain_text(bytes: &[u8]) -> (Document, LossReport) {
    let mut report = LossReport::new();
    let decoded = decode_and_normalize(bytes);

    if decoded.encoding == DetectedEncoding::FallbackLossy {
        report.record(
            LossKind::FormattingDropped,
            format!(
                "Text contains {} unmapped lossy characters",
                decoded.unmapped_chars
            ),
        );
    }

    let mut doc = Document::new_blank();
    let lines: Vec<&str> = decoded.text.split('\n').collect();

    let mut blocks = Vec::with_capacity(lines.len());
    for line in lines {
        let node_id = doc.ids().node();
        let mut para = Paragraph::new(node_id);
        if !line.is_empty() {
            let _ = para.push_text(line, None, Arc::default());
        }
        blocks.push(Block::Paragraph(Arc::new(para)));
    }

    if blocks.is_empty() {
        let node_id = doc.ids().node();
        blocks.push(Block::Paragraph(Arc::new(Paragraph::new(node_id))));
    }

    let body_id = doc.body_id();
    if let Some(body) = doc.story_mut(body_id) {
        body.blocks = SeqTree::from_vec(blocks);
    }

    (doc, report)
}

pub fn export_plain_text(doc: &Document, options: PlainTextOptions) -> (String, LossReport) {
    let mut report = LossReport::new();
    let mut out = String::new();
    let newline = match options.line_ending {
        LineEnding::Lf => "\n",
        LineEnding::CrLf => "\r\n",
    };

    let body = doc.body();
    export_blocks(&body.blocks, &mut out, newline, &mut report);

    (out, report)
}

fn export_blocks(
    blocks: &SeqTree<Block>,
    out: &mut String,
    newline: &str,
    report: &mut LossReport,
) {
    for block in blocks.iter() {
        match block {
            Block::Paragraph(p) => {
                if !p.inlines().is_empty() {
                    report.record(
                        LossKind::ImageOmitted,
                        format!("Paragraph has {} inline objects omitted", p.inlines().len()),
                    );
                }
                out.push_str(&p.raw_text());
                out.push_str(newline);
            }
            Block::Table(tbl) => {
                report.record(
                    LossKind::TableFlattened,
                    format!(
                        "Table with {} rows flattened to tab-separated text",
                        tbl.rows.len()
                    ),
                );
                for row in &tbl.rows {
                    let mut first_cell = true;
                    for cell in &row.cells {
                        if !first_cell {
                            out.push('\t');
                        }
                        first_cell = false;
                        let mut cell_text = String::new();
                        export_blocks(&cell.blocks, &mut cell_text, " ", report);
                        out.push_str(cell_text.trim_end());
                    }
                    out.push_str(newline);
                }
            }
            Block::Sdt(sdt) => {
                export_blocks(&sdt.blocks, out, newline, report);
            }
            Block::Opaque(_) => {
                report.record(
                    LossKind::UnsupportedElement,
                    "Opaque reserved block omitted from plain text",
                );
            }
        }
    }
}
