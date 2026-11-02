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
    hidden: bool,
    uc: usize,
    skip_chars: usize,
    is_destination_skip: bool,
}

pub fn import_rtf(bytes: &[u8], budget: RtfBudget) -> Result<(Document, LossReport), RtfError> {
    if bytes.len() > budget.max_text_bytes {
        return Err(RtfError::BudgetExceeded("input bytes"));
    }
    if budget.max_group_depth == 0 {
        return Err(RtfError::GroupDepthExceeded);
    }
    let mut report = LossReport::new();
    let mut tokenizer = RtfTokenizer::new(bytes, budget);

    // Initial check: must start with GroupStart then \rtf
    let first = tokenizer.next_token()?;
    if first != Some(RtfToken::GroupStart) {
        return Err(RtfError::InvalidHeader);
    }
    let second = tokenizer.next_token()?;
    match second {
        Some(RtfToken::ControlWord {
            ref name,
            param: Some(1),
        }) if name == "rtf" => {}
        _ => return Err(RtfError::InvalidHeader),
    }

    let mut doc = Document::new_blank();
    let mut blocks = Vec::new();

    let mut state_stack = Vec::new();
    let mut current_state = FormatState {
        bold: false,
        italic: false,
        strike: false,
        hidden: false,
        uc: 1,
        skip_chars: 0,
        is_destination_skip: false,
    };

    let mut current_para: Option<Paragraph> = None;
    let mut current_row_cells: Vec<Cell> = Vec::new();
    let mut closed = false;
    let mut pending_high = None;
    let mut text_bytes = 0usize;

    while let Some(token) = tokenizer.next_token()? {
        if closed {
            if matches!(&token, RtfToken::Text(text) if text.trim().is_empty()) {
                continue;
            }
            return Err(RtfError::Unsupported("data after root group"));
        }
        if blocks.len() + current_row_cells.len() > 100_000 {
            return Err(RtfError::BudgetExceeded("block count"));
        }
        match token {
            RtfToken::GroupStart => {
                if pending_high.is_some() {
                    return Err(RtfError::Unsupported("split Unicode surrogate pair"));
                }
                if state_stack.len() >= budget.max_group_depth {
                    return Err(RtfError::GroupDepthExceeded);
                }
                state_stack.push(current_state.clone());
            }
            RtfToken::GroupEnd => {
                if pending_high.is_some() {
                    return Err(RtfError::Unsupported("unpaired Unicode surrogate"));
                }
                if let Some(prev) = state_stack.pop() {
                    current_state = prev;
                } else {
                    closed = true;
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
                    "v" => current_state.hidden = param.unwrap_or(1) != 0,
                    "deleted" | "revised" | "revauth" | "revdttm" => {
                        return Err(RtfError::Unsupported(
                            "review data requires an explicit projection",
                        ));
                    }
                    "fonttbl" | "colortbl" | "stylesheet" | "info" | "header" | "footer"
                    | "field" | "object" | "pict" => {
                        current_state.is_destination_skip = true;
                        if name != "fonttbl" {
                            report.record(
                                LossKind::UnsupportedElement,
                                format!("RTF destination {name} omitted"),
                            );
                        }
                    }
                    "uc" => current_state.uc = param.unwrap_or(1).max(0) as usize,
                    "u" => {
                        if param.is_none() {
                            return Err(RtfError::Unsupported(
                                "Unicode control requires a UTF-16 value",
                            ));
                        }
                        if let Some(val) = param {
                            if !(-32768..=65535).contains(&val) {
                                return Err(RtfError::Unsupported("Unicode value outside UTF-16"));
                            }
                            let unit = val as u16;
                            current_state.skip_chars = current_state.uc;
                            let code_point = if (0xd800..=0xdbff).contains(&unit) {
                                if pending_high.replace(unit).is_some() {
                                    return Err(RtfError::Unsupported(
                                        "unpaired Unicode surrogate",
                                    ));
                                }
                                continue;
                            } else if (0xdc00..=0xdfff).contains(&unit) {
                                let high = pending_high
                                    .take()
                                    .ok_or(RtfError::Unsupported("unpaired Unicode surrogate"))?;
                                0x10000 + ((u32::from(high) - 0xd800) << 10) + u32::from(unit)
                                    - 0xdc00
                            } else {
                                if pending_high.is_some() {
                                    return Err(RtfError::Unsupported(
                                        "unpaired Unicode surrogate",
                                    ));
                                }
                                u32::from(unit)
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
                                props.hidden = current_state.hidden.then_some(true);
                                para.push_text(&ch.to_string(), None, Arc::new(props))
                                    .map_err(|_| RtfError::Unsupported("invalid Unicode text"))?;
                                text_bytes += ch.len_utf8();
                                if text_bytes > budget.max_text_bytes {
                                    return Err(RtfError::BudgetExceeded("decoded text"));
                                }
                            }
                        }
                    }
                    "par" => {
                        let para = current_para
                            .take()
                            .unwrap_or_else(|| Paragraph::new(doc.ids().node()));
                        blocks.push(Block::Paragraph(Arc::new(para)));
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
                                insertion: None,
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
                                borders: [None; 6],
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
                    "line" | "tab" => {
                        let text = if name == "line" { "\n" } else { "\t" };
                        let para =
                            current_para.get_or_insert_with(|| Paragraph::new(doc.ids().node()));
                        let props = RunProps {
                            bold: current_state.bold.then_some(true),
                            italic: current_state.italic.then_some(true),
                            strike: current_state.strike.then_some(true),
                            hidden: current_state.hidden.then_some(true),
                            ..RunProps::default()
                        };
                        para.push_text(text, None, Arc::new(props))
                            .map_err(|_| RtfError::Unsupported("invalid whitespace"))?;
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
                if pending_high.is_some() {
                    return Err(RtfError::Unsupported(
                        "text interrupts Unicode surrogate pair",
                    ));
                }
                props.hidden = current_state.hidden.then_some(true);
                text_bytes += run_text.len();
                if text_bytes > budget.max_text_bytes {
                    return Err(RtfError::BudgetExceeded("decoded text"));
                }
                para.push_text(&run_text, None, Arc::new(props))
                    .map_err(|_| RtfError::Unsupported("invalid paragraph text"))?;
            }
        }
    }

    if !closed || !state_stack.is_empty() || pending_high.is_some() {
        return Err(RtfError::UnexpectedEof);
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
