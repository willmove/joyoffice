//! Controlled and sanitized bidirectional HTML conversion (FORMAT-006).

use std::sync::Arc;

use joyword_model::{
    Block, Cell, Document, Paragraph, Row, Run, RunProps, SeqTree, Table, TableWidth, VerticalMerge,
};

use crate::loss::{LossKind, LossReport};

pub fn import_html(html: &str) -> (Document, LossReport) {
    let mut report = LossReport::new();
    let mut doc = Document::new_blank();
    let mut blocks = Vec::new();

    let mut rest = html;
    let mut current_para: Option<Paragraph> = None;
    let mut bold = false;
    let mut italic = false;
    let mut strike = false;

    while !rest.is_empty() {
        if let Some(tag_start) = rest.find('<') {
            let plain_prefix = &rest[..tag_start];
            if !plain_prefix.is_empty() {
                let unescaped = unescape_html(plain_prefix);
                let para = current_para.get_or_insert_with(|| Paragraph::new(doc.ids().node()));
                let mut props = RunProps::default();
                if bold {
                    props.bold = Some(true);
                }
                if italic {
                    props.italic = Some(true);
                }
                if strike {
                    props.strike = Some(true);
                }
                let _ = para.push_text(&unescaped, None, Arc::new(props));
            }

            let after_bracket = &rest[tag_start + 1..];
            if let Some(tag_end) = after_bracket.find('>') {
                let tag_content = after_bracket[..tag_end].trim();
                rest = &after_bracket[tag_end + 1..];

                let is_closing = tag_content.starts_with('/');
                let clean_tag = tag_content
                    .trim_start_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_lowercase();

                // Security check: strip dangerous tags
                match clean_tag.as_str() {
                    "script" | "style" | "iframe" | "object" | "embed" | "applet" => {
                        report.record(
                            LossKind::ActiveContentStripped,
                            format!("Stripped dangerous HTML element <{clean_tag}>"),
                        );
                        let close_pat = format!("</{clean_tag}>");
                        if let Some(close_idx) = rest.to_lowercase().find(&close_pat) {
                            rest = &rest[close_idx + close_pat.len()..];
                        }
                        continue;
                    }
                    _ => {}
                }

                // Table parsing
                if clean_tag == "table" && !is_closing {
                    if let Some(para) = current_para.take() {
                        blocks.push(Block::Paragraph(Arc::new(para)));
                    }
                    if let Some(close_tbl) = rest.to_lowercase().find("</table>") {
                        let tbl_content = &rest[..close_tbl];
                        rest = &rest[close_tbl + 8..];
                        if let Some(table) = parse_html_table(&mut doc, tbl_content) {
                            blocks.push(Block::Table(Arc::new(table)));
                        }
                    }
                    continue;
                }

                match clean_tag.as_str() {
                    "b" | "strong" => bold = !is_closing,
                    "i" | "em" => italic = !is_closing,
                    "s" | "del" | "strike" => strike = !is_closing,
                    "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "div" => {
                        if is_closing {
                            if let Some(para) = current_para.take() {
                                blocks.push(Block::Paragraph(Arc::new(para)));
                            }
                        } else {
                            if let Some(para) = current_para.take() {
                                blocks.push(Block::Paragraph(Arc::new(para)));
                            }
                            current_para = Some(Paragraph::new(doc.ids().node()));
                        }
                    }
                    "br" => {
                        let para =
                            current_para.get_or_insert_with(|| Paragraph::new(doc.ids().node()));
                        let _ = para.push_text("\n", None, Arc::default());
                    }
                    "a" if !is_closing && tag_content.contains("javascript:") => {
                        report.record(
                            LossKind::ActiveContentStripped,
                            "Blocked javascript: URI in <a> tag",
                        );
                    }
                    _ => {}
                }
            } else {
                break;
            }
        } else {
            let unescaped = unescape_html(rest);
            let para = current_para.get_or_insert_with(|| Paragraph::new(doc.ids().node()));
            let _ = para.push_text(&unescaped, None, Arc::default());
            break;
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

    (doc, report)
}

fn parse_html_table(doc: &mut Document, tbl_html: &str) -> Option<Table> {
    let mut rows = Vec::new();
    let mut rest = tbl_html;

    while let Some(tr_start) = rest.to_lowercase().find("<tr") {
        let after_tr = &rest[tr_start..];
        let tr_open_end = after_tr.find('>')?;
        let after_open = &after_tr[tr_open_end + 1..];
        let tr_close = after_open.to_lowercase().find("</tr>")?;
        let row_content = &after_open[..tr_close];
        rest = &after_open[tr_close + 5..];

        let mut cells = Vec::new();
        let mut cell_rest = row_content;
        while let Some(td_start) = cell_rest.to_lowercase().find("<td") {
            let after_td = &cell_rest[td_start..];
            let td_open_end = after_td.find('>')?;
            let after_td_open = &after_td[td_open_end + 1..];
            let td_close = after_td_open.to_lowercase().find("</td>")?;
            let cell_text = unescape_html(&after_td_open[..td_close]);
            cell_rest = &after_td_open[td_close + 5..];

            let node_id = doc.ids().node();
            let mut para = Paragraph::new(node_id);
            let _ = para.push_text(&cell_text, None, Arc::default());

            let cell_id = doc.ids().node();
            cells.push(Cell {
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
            });
        }

        if !cells.is_empty() {
            let row_id = doc.ids().node();
            rows.push(Row {
                id: row_id,
                grid_before: 0,
                grid_after: 0,
                cells,
                header: false,
                cant_split: false,
                height: None,
                preserved: None,
                props_revision: None,
            });
        }
    }

    if rows.is_empty() {
        return None;
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

pub fn export_html(doc: &Document) -> (String, LossReport) {
    let mut report = LossReport::new();
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html>\n<head>\n  <meta charset=\"UTF-8\">\n  <title>JoyWord Export</title>\n</head>\n<body>\n");

    let body = doc.body();
    for block in body.blocks.iter() {
        match block {
            Block::Paragraph(p) => {
                out.push_str("  <p>");
                let chars: Vec<char> = p.raw_text().chars().collect();
                let mut offset = 0;
                for run in p.runs() {
                    let end = (offset + run.len).min(chars.len());
                    let run_text: String = chars[offset..end].iter().collect();
                    offset = end;
                    render_run_html(run, &run_text, &mut out);
                }
                out.push_str("</p>\n");
            }
            Block::Table(tbl) => {
                out.push_str("  <table border=\"1\" cellspacing=\"0\" cellpadding=\"4\">\n");
                for row in &tbl.rows {
                    out.push_str("    <tr>\n");
                    for cell in &row.cells {
                        out.push_str("      <td>");
                        for b in cell.blocks.iter() {
                            if let Block::Paragraph(p) = b {
                                out.push_str(&escape_html(&p.raw_text()));
                            }
                        }
                        out.push_str("</td>\n");
                    }
                    out.push_str("    </tr>\n");
                }
                out.push_str("  </table>\n");
            }
            Block::Sdt(sdt) => {
                for b in sdt.blocks.iter() {
                    if let Block::Paragraph(p) = b {
                        out.push_str("  <p>");
                        out.push_str(&escape_html(&p.raw_text()));
                        out.push_str("</p>\n");
                    }
                }
            }
            Block::Opaque(_) => {
                report.record(
                    LossKind::UnsupportedElement,
                    "Opaque block omitted from HTML export",
                );
            }
        }
    }

    out.push_str("</body>\n</html>\n");
    (out, report)
}

fn render_run_html(run: &Run, text: &str, out: &mut String) {
    let escaped = escape_html(text);
    let is_bold = run.props.bold == Some(true);
    let is_italic = run.props.italic == Some(true);
    let is_strike = run.props.strike == Some(true);

    if is_bold {
        out.push_str("<strong>");
    }
    if is_italic {
        out.push_str("<em>");
    }
    if is_strike {
        out.push_str("<del>");
    }

    out.push_str(&escaped);

    if is_strike {
        out.push_str("</del>");
    }
    if is_italic {
        out.push_str("</em>");
    }
    if is_bold {
        out.push_str("</strong>");
    }
}

pub fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn unescape_html(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}
