//! MS-DOC (.doc) and WPS (.wps) document importer producing typed joyword_model::Document.

use joyoffice_cfb::{CfbBudget, CompoundFile};
use joyword_model::{
    Block, Cell, CellBorders, Document, Paragraph, Row, RunProps, SeqTree, Table, TableWidth,
    VerticalMerge,
};
use joyword_textfmt::{LossKind, LossReport};
use std::sync::Arc;

use crate::budget::{DocBudget, DocError};
use crate::clx::Clx;
use crate::fib::Fib;
use crate::wps::{DocMetadata, detect_format_and_preserve_streams};

pub fn import_doc(
    bytes: &[u8],
    budget: DocBudget,
) -> Result<(Document, LossReport, DocMetadata), DocError> {
    if bytes.len() > budget.max_file_bytes {
        return Err(DocError::BudgetExceeded("file bytes exceed budget"));
    }

    let cfb_budget = CfbBudget {
        max_file_size: budget.max_file_bytes,
        max_stream_size: budget.max_file_bytes,
        ..CfbBudget::default()
    };

    let cfb =
        CompoundFile::parse(bytes, cfb_budget).map_err(|e| DocError::CfbError(e.to_string()))?;

    let word_doc_bytes = cfb
        .read_stream("WordDocument")
        .map_err(|_| DocError::MissingWordDocumentStream)?;

    let fib = Fib::parse(&word_doc_bytes)?;

    let table_stream = cfb
        .read_stream(fib.which_table_stream)
        .map_err(|_| DocError::MissingTableStream(fib.which_table_stream))?;

    let clx = Clx::parse(&table_stream, fib.fc_clx, fib.lcb_clx, &budget)?;
    let raw_text = clx.extract_text(&word_doc_bytes, fib.ccp_text, &budget)?;

    let meta = detect_format_and_preserve_streams(&cfb, fib.is_template);

    let mut report = LossReport::new();
    let mut doc = Document::new_blank();
    let mut blocks = Vec::new();

    // Check for special control characters in raw text
    if raw_text.contains('\x13') || raw_text.contains('\x15') {
        report.record(
            LossKind::UnsupportedElement,
            "Binary DOC fields (0x13/0x15) flattened to display text",
        );
    }
    if raw_text.contains('\x08') {
        report.record(
            LossKind::ImageOmitted,
            "Binary DOC embedded picture/object anchor (0x08) omitted",
        );
    }

    // Split paragraphs by \r or \n
    let clean_text = raw_text.replace("\r\n", "\r").replace('\n', "\r");
    let trimmed = clean_text.trim_end_matches('\r');
    let para_texts: Vec<&str> = if trimmed.is_empty() {
        Vec::new()
    } else {
        trimmed.split('\r').collect()
    };

    let mut current_row_cells: Vec<Cell> = Vec::new();

    for para_str in para_texts {
        if blocks.len() > budget.max_paragraphs {
            return Err(DocError::BudgetExceeded("paragraph count exceeds budget"));
        }

        // Check if paragraph is table row content
        if para_str.contains('\x07') {
            // Table cell delimiters: split by 0x07
            let cells_text: Vec<&str> = para_str.split('\x07').collect();
            // The last entry after final 0x07 is empty or end of row
            for &cell_text in cells_text.iter().take(cells_text.len().saturating_sub(1)) {
                let mut cell_para = Paragraph::new(doc.ids().node());
                let sanitized = sanitize_text(cell_text);
                let _ = cell_para.push_text(&sanitized, None, Arc::new(RunProps::default()));

                let cell = Cell {
                    id: doc.ids().node(),
                    grid_span: 1,
                    vertical_merge: VerticalMerge::None,
                    width: None,
                    borders: CellBorders::default(),
                    shading: None,
                    margin: None,
                    vertical_align: None,
                    blocks: SeqTree::from_vec(vec![Block::Paragraph(Arc::new(cell_para))]),
                    preserved: None,
                    props_revision: None,
                };
                current_row_cells.push(cell);
            }

            if !current_row_cells.is_empty() {
                let row = Row {
                    id: doc.ids().node(),
                    grid_before: 0,
                    grid_after: 0,
                    cells: std::mem::take(&mut current_row_cells),
                    header: false,
                    cant_split: false,
                    height: None,
                    preserved: None,
                    props_revision: None,
                };
                let table = Table {
                    id: doc.ids().node(),
                    grid: Vec::new(),
                    rows: vec![row],
                    style: None,
                    width: Some(TableWidth::Auto),
                    layout: None,
                    alignment: None,
                    indent: None,
                    look: None,
                    cell_margin: None,
                    preserved: None,
                    props_revision: None,
                    source: None,
                };
                blocks.push(Block::Table(Arc::new(table)));
            }
        } else {
            let sanitized = sanitize_text(para_str);
            let mut para = Paragraph::new(doc.ids().node());
            if !sanitized.is_empty() {
                let _ = para.push_text(&sanitized, None, Arc::new(RunProps::default()));
            }
            blocks.push(Block::Paragraph(Arc::new(para)));
        }
    }

    if blocks.is_empty() {
        let node_id = doc.ids().node();
        blocks.push(Block::Paragraph(Arc::new(Paragraph::new(node_id))));
    }

    let body_id = doc.body_id();
    if let Some(body) = doc.story_mut(body_id) {
        body.blocks = SeqTree::from_vec(blocks);
    }

    Ok((doc, report, meta))
}

fn sanitize_text(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            c != '\x00'
                && c != '\x07'
                && c != '\x08'
                && c != '\x0C'
                && c != '\x13'
                && c != '\x14'
                && c != '\x15'
        })
        .collect()
}
