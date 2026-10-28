//! RTF document parser producing a typed joyword_model::Document (T600).

use std::sync::Arc;

use joyword_model::{
    Block, Cell, Document, Paragraph, Row, RunProps, SeqTree, Table, TableWidth, VerticalMerge,
};
use joyword_textfmt::{LossKind, LossReport};

use crate::budget::{RtfBudget, RtfError};
use crate::token::{RtfToken, RtfTokenizer};

#[derive(Clone, Default)]
struct FormatState {
    bold: bool,
    italic: bool,
    strike: bool,
    uc: usize,
    skip_chars: usize,
    is_destination_skip: bool,
}

pub fn import_rtf(bytes: &[u8], budget: RtfBudget) -> Result<(Document, LossReport), RtfError> {
    let mut report = LossReport::new();
    let mut tokenizer = RtfTokenizer::new(bytes, budget);

    // Initial check: must start with GroupStart then \rtf
    let first = tokenizer.next_token()?;
    if first != Some(RtfToken::GroupStart) {
        return Err(RtfError::InvalidHeader);
    }
    let second = tokenizer.next_token()?;
    match second {
        Some(RtfToken::ControlWord { ref name, .. }) if name == "rtf" => {}
        _ => return Err(RtfError::InvalidHeader),
    }

    let mut doc = Document::new_blank();
    let mut blocks = Vec::new();

    let mut state_stack = Vec::new();
    let mut current_state = FormatState {
        bold: false,
        italic: false,
        strike: false,
        uc: 1,
        skip_chars: 0,
        is_destination_skip: false,
    };

    let mut current_para: Option<Paragraph> = None;
    let mut current_row_cells: Vec<Cell> = Vec::new();

    while let Some(token) = tokenizer.next_token()? {
        match token {
            RtfToken::GroupStart => {
                if state_stack.len() >= budget.max_group_depth {
                    return Err(RtfError::GroupDepthExceeded);
                }
                state_stack.push(current_state.clone());
            }
            RtfToken::GroupEnd => {
                if let Some(prev) = state_stack.pop() {
                    current_state = prev;
                }
            }
            RtfToken::ControlWord { name, param } => {
                if current_state.is_destination_skip {
                    continue;
                }

                match name.as_str() {
                    "*" => {
                        // Mark current group as ignorable destination
                        current_state.is_destination_skip = true;
                    }
                    "b" => current_state.bold = param.unwrap_or(1) != 0,
                    "i" => current_state.italic = param.unwrap_or(1) != 0,
                    "strike" => current_state.strike = param.unwrap_or(1) != 0,
                    "uc" => current_state.uc = param.unwrap_or(1).max(0) as usize,
                    "u" => {
                        if let Some(val) = param {
                            let code_point = if val < 0 {
                                (val as u16) as u32
                            } else {
                                val as u32
                            };
                            if let Some(ch) = char::from_u32(code_point) {
                                let para = current_para
                                    .get_or_insert_with(|| Paragraph::new(doc.ids().node()));
                                let mut props = RunProps::default();
                                if current_state.bold {
                                    props.bold = Some(true);
                                }
                                if current_state.italic {
                                    props.italic = Some(true);
                                }
                                if current_state.strike {
                                    props.strike = Some(true);
                                }
                                let _ = para.push_text(&ch.to_string(), None, Arc::new(props));
                                current_state.skip_chars = current_state.uc;
                            }
                        }
                    }
                    "par" => {
                        if let Some(para) = current_para.take() {
                            blocks.push(Block::Paragraph(Arc::new(para)));
                        }
                    }
                    "cell" => {
                        // Cell boundary in table
                        let para = current_para
                            .take()
                            .unwrap_or_else(|| Paragraph::new(doc.ids().node()));
                        let cell_id = doc.ids().node();
                        let cell = Cell {
                            id: cell_id,
                            grid_span: 1,
                            vertical_merge: VerticalMerge::None,
                            width: None,
                            borders: joyword_model::CellBorders::default(),
                            shading: None,
                            margin: None,
                            vertical_align: None,
                            blocks: SeqTree::from_vec(vec![Block::Paragraph(Arc::new(para))]),
                            preserved: None,
                            props_revision: None,
                        };
                        current_row_cells.push(cell);
                    }
                    "row" => {
                        // Row boundary in table
                        if !current_row_cells.is_empty() {
                            let row_id = doc.ids().node();
                            let row = Row {
                                id: row_id,
                                grid_before: 0,
                                grid_after: 0,
                                cells: std::mem::take(&mut current_row_cells),
                                header: false,
                                cant_split: false,
                                height: None,
                                preserved: None,
                                props_revision: None,
                            };
                            let table_id = doc.ids().node();
                            let table = Table {
                                id: table_id,
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
                    }
                    "pict" => {
                        report.record(
                            LossKind::ImageOmitted,
                            "RTF \\pict object omitted from typed document model",
                        );
                    }
                    _ => {}
                }
            }
            RtfToken::Text(text) => {
                if current_state.is_destination_skip {
                    continue;
                }
                let mut run_text = text;
                if current_state.skip_chars > 0 {
                    let chars: Vec<char> = run_text.chars().collect();
                    if chars.len() <= current_state.skip_chars {
                        current_state.skip_chars -= chars.len();
                        continue;
                    } else {
                        run_text = chars[current_state.skip_chars..].iter().collect();
                        current_state.skip_chars = 0;
                    }
                }

                let para = current_para.get_or_insert_with(|| Paragraph::new(doc.ids().node()));
                let mut props = RunProps::default();
                if current_state.bold {
                    props.bold = Some(true);
                }
                if current_state.italic {
                    props.italic = Some(true);
                }
                if current_state.strike {
                    props.strike = Some(true);
                }
                let _ = para.push_text(&run_text, None, Arc::new(props));
            }
        }
    }

    if let Some(para) = current_para.take() {
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

    Ok((doc, report))
}
