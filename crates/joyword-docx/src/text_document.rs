//! New Transitional DOCX from a declared text projection. This is not an
//! original-package save: callers must disclose the conversion's losses.
//! ECMA-376 Part 1 §§17.2, 17.3, 17.6, 17.15 and Part 2 (OPC).

use joyoffice_opc::{Budget, Method, write_archive};
use joyword_model::{Block, Document, RunProps};

/// Creates a fresh A4 document with compatibilityMode=15, paragraph text and
/// bold/italic/strike. Unsupported source structures/properties are refused;
/// original-package preservation continues to use `plan_export` exclusively.
pub fn create_text_document(document: &Document, budget: &Budget) -> Result<Vec<u8>, &'static str> {
    if !document.validate().is_empty()
        || document.annotations().next().is_some()
        || document.stories().any(|s| s.id != document.body_id())
        || !document.move_markers().is_empty()
    {
        return Err("new text DOCX requires a valid standalone body projection");
    }
    let mut body = String::new();
    for block in document.body().blocks.iter() {
        let Block::Paragraph(paragraph) = block else {
            return Err("new text DOCX supports paragraphs only");
        };
        if !paragraph.inlines().is_empty()
            || paragraph.style.is_some()
            || paragraph.props_revision.is_some()
            || paragraph.mark_revision.is_some()
            || *paragraph.props != joyword_model::ParaProps::default()
        {
            return Err("new text DOCX requires projected plain paragraphs");
        }
        body.push_str("<w:p>");
        if paragraph.text().summary().bytes as u64 > budget.max_part_bytes {
            return Err("new text DOCX input paragraph exceeds part budget");
        }
        let characters: Vec<char> = paragraph.raw_text().chars().collect();
        let mut offset = 0usize;
        for run in paragraph.runs() {
            let supported = RunProps {
                bold: run.props.bold,
                italic: run.props.italic,
                strike: run.props.strike,
                ..RunProps::default()
            };
            if *run.props != supported
                || run.style.is_some()
                || run.revision.is_some()
                || run.format_revision.is_some()
                || run.link.is_some()
            {
                return Err("new text DOCX requires projected run formatting");
            }
            let end = offset.checked_add(run.len).ok_or("invalid run range")?;
            let text = characters.get(offset..end).ok_or("invalid run range")?;
            offset = end;
            body.push_str("<w:r><w:rPr>");
            for (name, value) in [
                ("b", supported.bold),
                ("i", supported.italic),
                ("strike", supported.strike),
            ] {
                if let Some(value) = value {
                    body.push_str(&format!(
                        "<w:{name} w:val=\"{}\"/>",
                        if value { "1" } else { "0" }
                    ));
                }
            }
            body.push_str("</w:rPr><w:t xml:space=\"preserve\">");
            for c in text {
                match c {
                    '&' => body.push_str("&amp;"),
                    '<' => body.push_str("&lt;"),
                    '>' => body.push_str("&gt;"),
                    '\t' => body.push_str("</w:t><w:tab/><w:t xml:space=\"preserve\">"),
                    '\n' => body.push_str("</w:t><w:br/><w:t xml:space=\"preserve\">"),
                    c if c.is_control() => return Err("unsupported XML control character"),
                    c => body.push(*c),
                }
                if body.len() as u64 > budget.max_part_bytes {
                    return Err("new text DOCX part budget exceeded");
                }
            }
            body.push_str("</w:t></w:r>");
        }
        body.push_str("</w:p>");
        if body.len() as u64 > budget.max_part_bytes {
            return Err("new text DOCX part budget exceeded");
        }
    }
    let w = crate::W;
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="{w}"><w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="851" w:footer="992" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#;
    let root_rels = format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{}" Target="word/document.xml"/></Relationships>"#,
        crate::REL_OFFICE_DOCUMENT
    );
    let document_rels = format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{}" Target="settings.xml"/></Relationships>"#,
        crate::REL_SETTINGS
    );
    let settings = format!(
        r#"<w:settings xmlns:w="{w}"><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>"#
    );
    let items = [
        ("[Content_Types].xml", types.as_bytes(), Method::Stored),
        ("_rels/.rels", root_rels.as_bytes(), Method::Stored),
        ("word/document.xml", xml.as_bytes(), Method::Stored),
        (
            "word/_rels/document.xml.rels",
            document_rels.as_bytes(),
            Method::Stored,
        ),
        ("word/settings.xml", settings.as_bytes(), Method::Stored),
    ];
    if items.len() as u64 > budget.max_entries
        || items
            .iter()
            .any(|(_, bytes, _)| bytes.len() as u64 > budget.max_part_bytes)
        || items
            .iter()
            .map(|(_, bytes, _)| bytes.len() as u64)
            .sum::<u64>()
            > budget.max_total_bytes
    {
        return Err("new text DOCX package budget exceeded");
    }
    let bytes = write_archive(&items).map_err(|_| "cannot build new text DOCX")?;
    let verified =
        crate::import(&bytes, budget).map_err(|_| "new text DOCX failed import verification")?;
    if !verified.editability.is_unrestricted() || !verified.diagnostics.is_empty() {
        return Err("new text DOCX failed editability verification");
    }
    Ok(bytes)
}
