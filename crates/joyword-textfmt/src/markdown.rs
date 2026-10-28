//! Markdown (CommonMark + GFM subset) import and export with security filtering (FORMAT-006).

use std::sync::Arc;

use joyword_model::{
    Block, Cell, Document, Paragraph, Row, Run, RunProps, SeqTree, Table, TableWidth, VerticalMerge,
};

use crate::loss::{LossKind, LossReport};

pub fn import_markdown(md_text: &str) -> (Document, LossReport) {
    let mut report = LossReport::new();
    let mut doc = Document::new_blank();
    let mut blocks = Vec::new();

    let lines: Vec<&str> = md_text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        // 1. Code blocks ```
        if trimmed.starts_with("```") {
            let mut code_lines = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim().starts_with("```") {
                code_lines.push(lines[i]);
                i += 1;
            }
            if i < lines.len() {
                i += 1; // skip closing ```
            }
            let node_id = doc.ids().node();
            let mut para = Paragraph::new(node_id);
            let code_content = code_lines.join("\n");
            let _ = para.push_text(&code_content, None, Arc::default());
            blocks.push(Block::Paragraph(Arc::new(para)));
            continue;
        }

        // 2. GFM Table (| ... |)
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let mut table_rows_raw = Vec::new();
            while i < lines.len() {
                let l = lines[i].trim();
                if l.starts_with('|') && l.ends_with('|') {
                    table_rows_raw.push(l);
                    i += 1;
                } else {
                    break;
                }
            }
            if let Some(table) = parse_gfm_table(&mut doc, &table_rows_raw, &mut report) {
                blocks.push(Block::Table(Arc::new(table)));
            }
            continue;
        }

        // 3. Headings (#)
        if trimmed.starts_with('#') {
            let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
            if hash_count <= 6 && trimmed[hash_count..].starts_with(' ') {
                let heading_text = trimmed[hash_count..].trim();
                let node_id = doc.ids().node();
                let mut para = Paragraph::new(node_id);
                parse_inlines_into_para(&mut para, heading_text, &mut report);
                blocks.push(Block::Paragraph(Arc::new(para)));
                i += 1;
                continue;
            }
        }

        // 4. Blockquote (>)
        if let Some(stripped) = trimmed.strip_prefix('>') {
            let node_id = doc.ids().node();
            let mut para = Paragraph::new(node_id);
            parse_inlines_into_para(&mut para, stripped.trim(), &mut report);
            blocks.push(Block::Paragraph(Arc::new(para)));
            i += 1;
            continue;
        }

        // 5. Normal paragraph
        let node_id = doc.ids().node();
        let mut para = Paragraph::new(node_id);
        parse_inlines_into_para(&mut para, trimmed, &mut report);
        blocks.push(Block::Paragraph(Arc::new(para)));
        i += 1;
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

fn parse_gfm_table(
    doc: &mut Document,
    raw_lines: &[&str],
    report: &mut LossReport,
) -> Option<Table> {
    if raw_lines.len() < 2 {
        return None;
    }

    let mut rows = Vec::new();

    for (row_idx, line) in raw_lines.iter().enumerate() {
        // Skip delimiter line (e.g. |---|---|)
        if row_idx == 1 && line.contains("---") {
            continue;
        }

        let cells_raw: Vec<&str> = line
            .trim_matches('|')
            .split('|')
            .map(|s| s.trim())
            .collect();

        let mut cells = Vec::new();
        for text in cells_raw {
            let node_id = doc.ids().node();
            let mut para = Paragraph::new(node_id);
            parse_inlines_into_para(&mut para, text, report);
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
            cells.push(cell);
        }

        let row_id = doc.ids().node();
        rows.push(Row {
            id: row_id,
            grid_before: 0,
            grid_after: 0,
            cells,
            header: row_idx == 0,
            cant_split: false,
            height: None,
            preserved: None,
            props_revision: None,
        });
    }

    let table_id = doc.ids().node();
    Some(Table {
        id: table_id,
        grid: Vec::new(),
        rows,
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
    })
}

fn extract_delimited<'a>(text: &'a str, delim: &str) -> Option<(&'a str, &'a str)> {
    let after = text.strip_prefix(delim)?;
    let end = after.find(delim)?;
    Some((&after[..end], &after[end + delim.len()..]))
}

fn parse_inlines_into_para(para: &mut Paragraph, text: &str, report: &mut LossReport) {
    let mut rest = text;
    while !rest.is_empty() {
        if let Some((bold_text, remainder)) = extract_delimited(rest, "**") {
            let props = RunProps {
                bold: Some(true),
                ..RunProps::default()
            };
            let _ = para.push_text(bold_text, None, Arc::new(props));
            rest = remainder;
            continue;
        }
        if let Some((italic_text, remainder)) = extract_delimited(rest, "*") {
            let props = RunProps {
                italic: Some(true),
                ..RunProps::default()
            };
            let _ = para.push_text(italic_text, None, Arc::new(props));
            rest = remainder;
            continue;
        }
        if let Some((strike_text, remainder)) = extract_delimited(rest, "~~") {
            let props = RunProps {
                strike: Some(true),
                ..RunProps::default()
            };
            let _ = para.push_text(strike_text, None, Arc::new(props));
            rest = remainder;
            continue;
        }
        if let Some((code_text, remainder)) = extract_delimited(rest, "`") {
            let _ = para.push_text(code_text, None, Arc::default());
            rest = remainder;
            continue;
        }
        // Link [text](url)
        if let Some(link_info) = rest.strip_prefix('[').and_then(parse_markdown_link) {
            let (link_text, url, remainder) = link_info;
            let url_lower = url.to_lowercase();
            if url_lower.starts_with("javascript:")
                || url_lower.starts_with("file:")
                || url_lower.starts_with("\\\\")
            {
                report.record(
                    LossKind::ActiveContentStripped,
                    format!("Blocked unsafe markdown link URI: {url}"),
                );
            }
            let _ = para.push_text(link_text, None, Arc::default());
            rest = remainder;
            continue;
        }

        let next_special = rest
            .char_indices()
            .skip(1)
            .find(|&(_, c)| c == '*' || c == '`' || c == '~' || c == '[')
            .map(|(idx, _)| idx)
            .unwrap_or(rest.len());

        let plain = &rest[..next_special];
        let _ = para.push_text(plain, None, Arc::default());
        rest = &rest[next_special..];
    }
}

fn parse_markdown_link(after_bracket: &str) -> Option<(&str, &str, &str)> {
    let text_end = after_bracket.find("](")?;
    let link_text = &after_bracket[..text_end];
    let after_paren = &after_bracket[text_end + 2..];
    let url_end = after_paren.find(')')?;
    let url = &after_paren[..url_end];
    let remainder = &after_paren[url_end + 1..];
    Some((link_text, url, remainder))
}

pub fn export_markdown(doc: &Document) -> (String, LossReport) {
    let mut report = LossReport::new();
    let mut out = String::new();

    let body = doc.body();
    for block in body.blocks.iter() {
        match block {
            Block::Paragraph(p) => {
                if !p.inlines().is_empty() {
                    report.record(
                        LossKind::ImageOmitted,
                        "Images or complex drawings in paragraph omitted from markdown",
                    );
                }

                let chars: Vec<char> = p.raw_text().chars().collect();
                let mut offset = 0;
                for run in p.runs() {
                    let end = (offset + run.len).min(chars.len());
                    let run_text: String = chars[offset..end].iter().collect();
                    offset = end;
                    render_run_markdown(run, &run_text, &mut out, &mut report);
                }
                out.push_str("\n\n");
            }
            Block::Table(tbl) => {
                out.push('\n');
                for (r_idx, row) in tbl.rows.iter().enumerate() {
                    out.push('|');
                    for cell in &row.cells {
                        out.push(' ');
                        let mut cell_text = String::new();
                        for b in cell.blocks.iter() {
                            if let Block::Paragraph(cp) = b {
                                cell_text.push_str(&cp.raw_text());
                            }
                        }
                        out.push_str(cell_text.trim());
                        out.push_str(" |");
                    }
                    out.push('\n');

                    if r_idx == 0 {
                        out.push('|');
                        for _ in &row.cells {
                            out.push_str(" --- |");
                        }
                        out.push('\n');
                    }
                }
                out.push('\n');
            }
            Block::Sdt(sdt) => {
                for b in sdt.blocks.iter() {
                    if let Block::Paragraph(p) = b {
                        out.push_str(&p.raw_text());
                        out.push_str("\n\n");
                    }
                }
            }
            Block::Opaque(_) => {
                report.record(
                    LossKind::UnsupportedElement,
                    "Opaque block omitted from markdown",
                );
            }
        }
    }

    (out.trim_end().to_string(), report)
}

fn render_run_markdown(run: &Run, run_text: &str, out: &mut String, _report: &mut LossReport) {
    let is_bold = run.props.bold == Some(true);
    let is_italic = run.props.italic == Some(true);
    let is_strike = run.props.strike == Some(true);

    if is_bold {
        out.push_str("**");
    }
    if is_italic {
        out.push('*');
    }
    if is_strike {
        out.push_str("~~");
    }

    out.push_str(run_text);

    if is_strike {
        out.push_str("~~");
    }
    if is_italic {
        out.push('*');
    }
    if is_bold {
        out.push_str("**");
    }
}
