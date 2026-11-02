//! Bounded, explicit-loss text interchange (T600/T601). No IO or source edits.
//!
//! This first product adapter accepts paragraph-only documents. Review data,
//! hidden text, fields, objects and containers require their own projections;
//! they are refused rather than accidentally exposed by a raw-text exporter.

use std::sync::Arc;

use joyoffice_opc::Budget;
use joyword_model::{Block, Document, Paragraph, RunProps, SeqTree};
use joyword_textfmt::{LossKind, LossReport, PlainTextOptions};

pub const MAX_INPUT: usize = 4 << 20;
const MAX_OUTPUT: usize = 64 << 20;
const MAX_PARAGRAPHS: usize = 10_000;
const MAX_RUNS: usize = 20_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Docx,
    Txt,
    Markdown,
    Html,
    Rtf,
}

impl Format {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "docx" => Some(Self::Docx),
            "txt" => Some(Self::Txt),
            "markdown" => Some(Self::Markdown),
            "html" => Some(Self::Html),
            "rtf" => Some(Self::Rtf),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Docx => "docx",
            Self::Txt => "txt",
            Self::Markdown => "markdown",
            Self::Html => "html",
            Self::Rtf => "rtf",
        }
    }
}

/// Immutable bytes and disclosures from the same conversion. A UI must show
/// `losses` before publishing; CLI requires --accept-losses for publication.
pub struct Plan {
    bytes: Vec<u8>,
    target: Format,
    pub losses: LossReport,
    pub paragraphs: usize,
}

impl Plan {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn target(&self) -> Format {
        self.target
    }
}

pub fn plan(input: &[u8], from: Format, to: Format) -> Result<Plan, &'static str> {
    if input.len() > MAX_INPUT {
        return Err("exchange input exceeds 4 MiB budget");
    }
    let (document, losses) = match from {
        Format::Docx => {
            let budget = Budget {
                max_entries: 1024,
                max_part_bytes: 4 << 20,
                max_total_bytes: 16 << 20,
                max_xml_nodes: 100_000,
                ..Budget::default()
            };
            let imported = joyword_docx::import(input, &budget)
                .map_err(|_| "DOCX invalid, unsupported or over exchange budget")?;
            if !imported.editability.is_unrestricted() || !imported.diagnostics.is_empty() {
                return Err("restricted or partially imported DOCX cannot use text exchange");
            }
            (imported.document, LossReport::new())
        }
        Format::Rtf => {
            check_rtf_subset(input)?;
            joyword_rtf::import_rtf(input, joyword_rtf::RtfBudget::default())
                .map_err(|_| "RTF invalid, unsupported or over budget")?
        }
        Format::Txt | Format::Markdown | Format::Html => {
            let decoded = joyword_textfmt::encoding::decode_and_normalize(input);
            if decoded.unmapped_chars != 0 {
                return Err("invalid text encoding; conversion would replace source characters");
            }
            if decoded
                .text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
            {
                return Err("unsupported control character in text input");
            }
            if decoded.text.lines().count() > MAX_PARAGRAPHS
                || decoded
                    .text
                    .bytes()
                    .filter(|b| matches!(b, b'<' | b'*' | b'`' | b'['))
                    .count()
                    > MAX_RUNS
            {
                return Err("text syntax work budget exceeded");
            }
            if from == Format::Html {
                // The controlled HTML importer has no CSS cascade. Exposing
                // hidden content without one would be unsafe; refuse that
                // source class rather than silently treating it as visible.
                if html_requires_projection(&decoded.text) {
                    return Err("styled, hidden or reviewed HTML requires an explicit projection");
                }
            }
            match from {
                Format::Txt => joyword_textfmt::import_plain_text(decoded.text.as_bytes()),
                Format::Markdown => joyword_textfmt::import_markdown(&decoded.text),
                Format::Html => joyword_textfmt::import_html(&decoded.text),
                _ => return Err("invalid text input format"),
            }
        }
    };
    export_document(&document, to, losses)
}

/// Converts the committed model only; does not mutate a session or saved version.
pub fn plan_document(document: &Document, to: Format) -> Result<Plan, &'static str> {
    export_document(document, to, LossReport::new())
}

fn export_document(
    document: &Document,
    to: Format,
    mut losses: LossReport,
) -> Result<Plan, &'static str> {
    let projected = paragraph_projection(document)?;
    // These libraries implement a deliberately declared formatting subset.
    // Always disclose its boundary, including information absent from the
    // imported model. An empty per-element report is not a fidelity claim.
    losses.record(LossKind::FormattingDropped,
        "Paragraph text and direct bold/italic/strike only; page layout, fonts, styles, metadata and unsupported source syntax are not preserved. This is a conversion copy, not a round-trip save.");
    let (output, output_losses) = match to {
        Format::Txt => joyword_textfmt::export_plain_text(&projected, PlainTextOptions::default()),
        Format::Markdown => joyword_textfmt::export_markdown(&projected),
        Format::Html => joyword_textfmt::export_html(&projected),
        Format::Rtf => joyword_rtf::export_rtf(&projected),
        Format::Docx => {
            let budget = Budget {
                max_part_bytes: MAX_OUTPUT as u64,
                max_total_bytes: MAX_OUTPUT as u64,
                ..Budget::default()
            };
            let bytes = joyword_docx::create_text_document(&projected, &budget)?;
            if bytes.len() > MAX_OUTPUT {
                return Err("exchange output exceeds 64 MiB budget");
            }
            return Ok(Plan {
                bytes,
                target: to,
                losses,
                paragraphs: projected.body().blocks.len(),
            });
        }
    };
    losses.items.extend(output_losses.items);
    if output.len() > MAX_OUTPUT {
        return Err("exchange output exceeds 64 MiB budget");
    }
    Ok(Plan {
        bytes: output.into_bytes(),
        target: to,
        losses,
        paragraphs: projected.body().blocks.len(),
    })
}

fn html_requires_projection(input: &str) -> bool {
    input.split('<').skip(1).any(|tail| {
        let tag = tail.split('>').next().unwrap_or("").to_ascii_lowercase();
        let mut tokens = tag
            .split(|c: char| c.is_ascii_whitespace() || c == '=' || c == '/')
            .filter(|s| !s.is_empty());
        let name = tokens.next().unwrap_or("");
        matches!(name, "style" | "link" | "del" | "ins")
            || tokens.any(|token| matches!(token, "style" | "class" | "hidden"))
    })
}

fn check_rtf_subset(input: &[u8]) -> Result<(), &'static str> {
    use joyword_rtf::token::{RtfToken, RtfTokenizer};
    let mut tokenizer = RtfTokenizer::new(input, joyword_rtf::RtfBudget::default());
    let mut tokens = 0;
    while let Some(token) = tokenizer.next_token().map_err(|_| "malformed RTF token")? {
        tokens += 1;
        if tokens > 100_000 {
            return Err("RTF syntax work budget exceeded");
        }
        if let RtfToken::ControlWord { name, .. } = token
            && !matches!(
                name.as_str(),
                "rtf"
                    | "ansi"
                    | "ansicpg"
                    | "deff"
                    | "nouicompat"
                    | "fonttbl"
                    | "f"
                    | "fnil"
                    | "fcharset"
                    | "b"
                    | "i"
                    | "strike"
                    | "v"
                    | "uc"
                    | "u"
                    | "par"
                    | "line"
                    | "tab"
            )
        {
            return Err("RTF control outside declared text-exchange subset");
        }
    }
    Ok(())
}

fn paragraph_projection(source: &Document) -> Result<Document, &'static str> {
    if !source.validate().is_empty() {
        return Err("invalid imported model");
    }
    if source.annotations().next().is_some()
        || !source.move_markers().is_empty()
        || source.has_table_row_insertions()
        || source.stories().any(|s| s.id != source.body_id())
    {
        return Err(
            "annotations, review data or additional stories require an explicit projection",
        );
    }
    if source.body().blocks.len() > MAX_PARAGRAPHS {
        return Err("exchange paragraph budget exceeded");
    }
    let mut output = Document::new_blank();
    let mut blocks = Vec::new();
    let mut run_count = 0usize;
    let mut text_bytes = 0usize;
    for block in source.body().blocks.iter() {
        let Block::Paragraph(paragraph) = block else {
            return Err(
                "tables, content controls and preserved blocks are not supported by this adapter",
            );
        };
        if !paragraph.inlines().is_empty()
            || paragraph.mark_revision.is_some()
            || paragraph.props_revision.is_some()
        {
            return Err("inline objects or paragraph revisions require an explicit projection");
        }
        text_bytes = text_bytes
            .checked_add(paragraph.text().summary().bytes)
            .filter(|n| *n <= MAX_INPUT)
            .ok_or("exchange model text budget exceeded")?;
        let chars: Vec<char> = paragraph.raw_text().chars().collect();
        let mut offset = 0;
        let mut converted = Paragraph::new(output.ids().node());
        for run in paragraph.runs() {
            run_count += 1;
            if run_count > MAX_RUNS {
                return Err("exchange run budget exceeded");
            }
            let resolved = source.styles().resolve_run(
                paragraph.style.as_ref(),
                run.style.as_ref(),
                &run.props,
            );
            if run.revision.is_some()
                || run.format_revision.is_some()
                || resolved.props.hidden == Some(true)
                || !resolved.problems.is_empty()
            {
                return Err(
                    "hidden text, review data or unresolved styles require an explicit projection",
                );
            }
            let end = offset + run.len;
            let text: String = chars
                .get(offset..end)
                .ok_or("invalid run range")?
                .iter()
                .collect();
            let props = RunProps {
                bold: resolved.props.bold,
                italic: resolved.props.italic,
                strike: resolved.props.strike,
                ..RunProps::default()
            };
            converted
                .push_text(&text, None, Arc::new(props))
                .map_err(|_| "unsupported imported text")?;
            offset = end;
        }
        if offset != chars.len() {
            return Err("incomplete run coverage");
        }
        blocks.push(Block::Paragraph(Arc::new(converted)));
    }
    let body = output.body_id();
    output.story_mut(body).ok_or("missing output story")?.blocks = SeqTree::from_vec(blocks);
    Ok(output)
}
