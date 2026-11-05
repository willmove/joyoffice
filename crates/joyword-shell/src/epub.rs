//! Reflowable EPUB 3.3 export from the R8 model, using the package and navigation
//! structure from JoyWord's original exporter (W3C EPUB 3.3, sections 4–7).
use crate::{
    reader::{Reader, publish_verified},
    storage::SourceStamp,
};
use joyoffice_opc::{Method, write_archive};
use joyword_engine::doc::{
    Align, Block, Document, Paragraph,
    link::is_web_or_mail_link,
    para::{COLUMN_BREAK, InlineObject, OBJ, PAGE_BREAK, Wrap},
    props::{TextColor, Underline, VMerge},
    styles::TablePart,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt::Write, path::Path, sync::atomic::AtomicBool};

const MAX_OUTPUT: usize = 64 << 20;
const CSS: &str = "body{line-height:1.5}p,h1,h2,h3,h4,h5,h6{white-space:pre-wrap}img{max-width:100%;height:auto}table{border-collapse:collapse}td{border:1px solid;padding:.3em}";

#[derive(serde::Serialize)]
pub struct Loss {
    pub location: String,
    pub detail: &'static str,
}

pub struct Plan {
    bytes: Vec<u8>,
    pub losses: Vec<Loss>,
    pub paragraphs: usize,
    pub images: usize,
}

impl Plan {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn publish(&self, path: &Path, cancel: &AtomicBool) -> Result<(), String> {
        if crate::advanced::extension(path) != "epub" {
            return Err("EPUB destination must end in .epub".into());
        }
        publish_verified(path, &self.bytes, cancel)
    }
}

pub fn modified_now() -> Result<String, String> {
    // The renderer accepts only four-digit UTC timestamps. Use the same
    // calendar conversion as JoyWord's pre-R8 EPUB exporter.
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs();
    let mut days = seconds / 86400;
    let mut year = 1970;
    while days >= 365 + u64::from(leap(year)) {
        days -= 365 + u64::from(leap(year));
        year += 1;
        if year > 9999 {
            return Err("date outside four-digit year range".into());
        }
    }
    let mut month = 1;
    while days >= month_days(year, month) {
        days -= month_days(year, month);
        month += 1;
    }
    let time = seconds % 86400;
    Ok(format!(
        "{year:04}-{month:02}-{:02}T{:02}:{:02}:{:02}Z",
        days + 1,
        time / 3600,
        time / 60 % 60,
        time % 60
    ))
}

fn leap(year: u64) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

fn month_days(year: u64, month: u64) -> u64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => 28 + u64::from(leap(year)),
        _ => 0,
    }
}

fn valid_modified(value: &str) -> bool {
    let b = value.as_bytes();
    if b.len() != 20
        || !b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'Z',
            _ => c.is_ascii_digit(),
        })
    {
        return false;
    }
    let n = |start, end| {
        value
            .get(start..end)
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0)
    };
    let year = n(0, 4);
    year > 0
        && (1..=month_days(year, n(5, 7))).contains(&n(8, 10))
        && n(11, 13) < 24
        && n(14, 16) < 60
        && n(17, 19) < 60
}

fn xml(out: &mut String, text: &str) -> Result<(), String> {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(c),
            '\0'..='\u{1f}' | '\u{fffe}' | '\u{ffff}' => {
                return Err("character cannot be represented in XML 1.0".into());
            }
            _ => out.push(c),
        }
    }
    Ok(())
}

pub fn open_docx(path: &Path) -> Result<Document, String> {
    let bytes = joyword_engine::io::read_regular_file_within(path, 32 << 20)
        .ok_or("EPUB input must be a regular DOCX file of at most 32 MiB")?;
    validate_docx_source(&bytes)?;
    joyword_docx::read(&bytes).map_err(|error| error.to_string())
}

pub fn validate_docx_source(bytes: &[u8]) -> Result<(), String> {
    let inspection = joyword_docx::legacy_inspection::project(bytes, 0)?;
    if !inspection.unsigned_docx || inspection.read_only || inspection.unsupported_body {
        return Err("restricted or unsupported DOCX cannot be converted to EPUB".into());
    }
    Ok(())
}

impl Reader {
    /// Export can inspect the same bytes the session opened without reopening the path.
    pub fn epub_export_source(&self) -> Option<SourceStamp> {
        self.source
            .as_ref()
            .filter(|s| joyword_engine::io::is_word_package(&crate::advanced::extension(s.path())))
            .cloned()
    }
}

struct Image<'a> {
    name: String,
    media_type: &'static str,
    bytes: &'a [u8],
}

struct Writer<'a> {
    doc: &'a Document,
    body: String,
    headings: Vec<(usize, String)>,
    images: BTreeMap<&'a str, Image<'a>>,
    losses: Vec<Loss>,
    paragraphs: usize,
    blocks: usize,
    runs: usize,
    text: usize,
    resource_bytes: usize,
    resource_pixels: u64,
}

impl<'a> Writer<'a> {
    fn loss(&mut self, location: &str, detail: &'static str) {
        self.losses.push(Loss {
            location: location.into(),
            detail,
        });
    }

    fn blocks(
        &mut self,
        blocks: &'a [std::sync::Arc<Block>],
        path: &str,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 24 {
            return Err("EPUB table nesting budget exceeded".into());
        }
        for (index, block) in blocks.iter().enumerate() {
            self.blocks += 1;
            if self.blocks > 20_000 || self.body.len() > MAX_OUTPUT {
                return Err("EPUB output budget exceeded".into());
            }
            let path = format!("{path}/block[{}]", index + 1);
            match block.as_ref() {
                Block::Para(paragraph) => self.paragraph(paragraph, &path)?,
                Block::Table(table) => {
                    if table.props.fmt_change.is_some()
                        || !table.controls.is_empty()
                        || table.rows.is_empty()
                    {
                        return Err(format!("{path}: unsupported table"));
                    }
                    if table
                        .props
                        .style
                        .as_deref()
                        .and_then(|id| self.doc.styles.table_style(id))
                        .is_some_and(|style| {
                            style.chr.hidden == Some(true)
                                || TablePart::ALL
                                    .into_iter()
                                    .any(|part| style.parts.part(part).chr.hidden == Some(true))
                        })
                    {
                        return Err(format!("{path}: table style can hide content"));
                    }
                    self.loss(&path, "table geometry and styles replaced by a reflow grid");
                    self.body.push_str("<table><tbody>");
                    for (r, row) in table.rows.iter().enumerate() {
                        if row.props.fmt_change.is_some()
                            || !row.controls.is_empty()
                            || row.cells.is_empty()
                        {
                            return Err(format!("{path}: unsupported row"));
                        }
                        self.body.push_str("<tr>");
                        for (c, cell) in row.cells.iter().enumerate() {
                            if cell.props.fmt_change.is_some()
                                || !cell.controls.is_empty()
                                || cell.props.vmerge != VMerge::None
                            {
                                return Err(format!(
                                    "{path}: unsupported or vertically merged cell"
                                ));
                            }
                            // Charge even empty cells to bound table traversal and generated markup.
                            self.blocks += 1;
                            if self.blocks > 20_000 {
                                return Err("EPUB block budget exceeded".into());
                            }
                            let _ = write!(self.body, "<td colspan=\"{}\">", cell.span());
                            self.blocks(
                                &cell.blocks,
                                &format!("{path}/row[{}]/cell[{}]", r + 1, c + 1),
                                depth + 1,
                            )?;
                            self.body.push_str("</td>");
                        }
                        self.body.push_str("</tr>");
                    }
                    self.body.push_str("</tbody></table>");
                }
            }
        }
        Ok(())
    }

    fn paragraph(&mut self, p: &'a Paragraph, path: &str) -> Result<(), String> {
        self.text += p.text.len();
        self.runs += p.runs.len();
        if self.text > 4 << 20 || self.runs > 32_768 {
            return Err("EPUB text or run budget exceeded".into());
        }
        let props = self.doc.styles.resolve_para(&p.props);
        if p.props.fmt_change.is_some()
            || p.mark.ins.is_some()
            || p.mark.del.is_some()
            || p.mark.fmt_change.is_some()
            || props.numbering.is_some_and(|n| n.num != 0)
        {
            return Err(format!("{path}: numbered or revised paragraph"));
        }
        self.paragraphs += 1;
        let tag = match props.outline_level {
            Some(level) if level < 9 => {
                let label = p.text.replace([OBJ, PAGE_BREAK, COLUMN_BREAK], "");
                if !label.trim().is_empty() {
                    self.headings.push((self.paragraphs, label));
                }
                if level >= 6 {
                    self.loss(path, "heading levels above six use h6");
                }
                format!("h{}", level.min(5) + 1)
            }
            _ => "p".into(),
        };
        let align = match props.align {
            Align::Left => "start",
            Align::Right => "end",
            Align::Center => "center",
            Align::Justify | Align::Distribute => "justify",
        };
        let dir = if props.bidi { "rtl" } else { "auto" };
        let _ = write!(
            self.body,
            "<{tag} id=\"p{}\" dir=\"{dir}\" style=\"text-align:{align}\">",
            self.paragraphs
        );
        let mut objects = p.objects.iter();
        let mut end = 0;
        for (range, run) in p.run_ranges() {
            end = range.end;
            let value = p
                .text
                .get(range)
                .ok_or_else(|| format!("{path}: invalid run range"))?;
            let style = self.doc.styles.resolve_char(Some(&props.style), run);
            if style.hidden || style.ins.is_some() || style.del.is_some() || style.fmt.is_some() {
                return Err(format!("{path}: hidden text or unresolved revision"));
            }
            let link = style.link.as_deref();
            if let Some(link) = link {
                if !is_web_or_mail_link(link)
                    || link.chars().any(|ch| ch.is_whitespace() || ch.is_control())
                {
                    return Err(format!("{path}: unsupported hyperlink"));
                }
                self.body.push_str("<a href=\"");
                xml(&mut self.body, link)?;
                self.body.push_str("\">");
            }
            let weight = if style.bold { "bold" } else { "normal" };
            let slant = if style.italic { "italic" } else { "normal" };
            let _ = write!(
                self.body,
                "<span style=\"font-size:{:.2}pt;font-weight:{weight};font-style:{slant};",
                style.size
            );
            if style.underline != Underline::None || style.strike || style.double_strike {
                self.body.push_str("text-decoration:");
                if style.underline != Underline::None {
                    self.body.push_str("underline ");
                }
                if style.strike || style.double_strike {
                    self.body.push_str("line-through");
                }
                self.body.push(';');
            }
            if let TextColor::Rgb(rgb) = style.color {
                let _ = write!(self.body, "color:#{:02x}{:02x}{:02x};", rgb.0, rgb.1, rgb.2);
            }
            self.body.push_str("\">");
            let mut remaining = value;
            while let Some(index) = remaining.find([OBJ, '\n', PAGE_BREAK, COLUMN_BREAK]) {
                xml(&mut self.body, &remaining[..index])?;
                let rest = &remaining[index..];
                if let Some(after) = rest.strip_prefix(OBJ) {
                    self.image(objects.next(), path)?;
                    remaining = after;
                } else {
                    self.body.push_str("<br/>");
                    remaining = &rest[1..];
                }
                if self.body.len() > MAX_OUTPUT {
                    return Err("EPUB output budget exceeded".into());
                }
            }
            xml(&mut self.body, remaining)?;
            self.body.push_str("</span>");
            if link.is_some() {
                self.body.push_str("</a>");
            }
        }
        if end != p.text.len() || objects.next().is_some() {
            return Err(format!("{path}: invalid run or object coverage"));
        }
        let _ = writeln!(self.body, "</{tag}>");
        Ok(())
    }

    fn image(&mut self, object: Option<&'a InlineObject>, path: &str) -> Result<(), String> {
        let Some(InlineObject::Image {
            media,
            w,
            alt,
            float,
            crop,
            ole: None,
            ..
        }) = object
        else {
            return Err(format!("{path}: unsupported inline object"));
        };
        if float.wrap != Wrap::Inline
            || float.hidden
            || float.rot != 0.0
            || float.flip_h
            || float.flip_v
            || *crop != [0.0; 4]
            || !w.is_finite()
            || !(0.01..=10000.0).contains(w)
        {
            return Err(format!("{path}: unsupported image placement or transform"));
        }
        if !self.images.contains_key(media.as_str()) {
            let bytes = self
                .doc
                .media
                .get(media)
                .ok_or_else(|| format!("{path}: missing image"))?;
            if bytes.len() > 8 << 20
                || self.resource_bytes + bytes.len() > 32 << 20
                || self.images.len() >= 1024
            {
                return Err("EPUB image budget exceeded".into());
            }
            let (ext, media_type) = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                ("png", "image/png")
            } else if bytes.starts_with(b"\xff\xd8\xff") {
                ("jpg", "image/jpeg")
            } else {
                return Err(format!("{path}: only PNG and JPEG images are supported"));
            };
            let probe =
                joyword_geom::picture::probe(bytes).map_err(|error| format!("{path}: {error}"))?;
            if probe.width > 4096 || probe.height > 4096 {
                return Err(format!("{path}: image exceeds 4096 pixels per side"));
            }
            self.resource_pixels += u64::from(probe.width) * u64::from(probe.height);
            if self.resource_pixels > 128 << 20 {
                return Err("EPUB decoded image budget exceeded".into());
            }
            probe
                .decode(160 << 20)
                .map_err(|error| format!("{path}: {error}"))?;
            self.resource_bytes += bytes.len();
            let name = format!("images/image-{}.{ext}", self.images.len() + 1);
            self.images.insert(
                media,
                Image {
                    name,
                    media_type,
                    bytes,
                },
            );
        }
        let image = &self.images[media.as_str()];
        let _ = write!(
            self.body,
            "<img src=\"{}\" style=\"width:{w:.2}pt\" alt=\"",
            image.name
        );
        let description = if float.alt.decorative {
            ""
        } else if alt.is_empty() {
            "Image"
        } else {
            alt
        };
        xml(&mut self.body, description)?;
        self.body.push_str("\"/>");
        self.loss(
            path,
            "image uses responsive sizing; review the alternative text",
        );
        Ok(())
    }
}

pub fn export(doc: &Document, title: &str, lang: &str, modified: &str) -> Result<Plan, String> {
    if title.trim().is_empty() || title.len() > 4096 {
        return Err("EPUB title required, maximum 4096 bytes".into());
    }
    let mut subtags = lang.split('-');
    if lang.len() > 64
        || !subtags.next().is_some_and(|s| {
            (2..=8).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_alphabetic())
        })
        || !subtags
            .all(|s| (1..=8).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_alphanumeric()))
        || !valid_modified(modified)
    {
        return Err("EPUB requires a language tag and a valid UTC date-time".into());
    }
    if doc.settings.protection.is_some()
        || !doc.comments.is_empty()
        || !doc.revisions.is_empty()
        || doc.parts.values().any(|part| {
            part.blocks
                .iter()
                .any(|b| b.as_para().is_none_or(|p| !p.text.is_empty()))
        })
    {
        return Err("EPUB does not support protected content, annotations, revisions or nonempty additional stories".into());
    }
    let mut writer = Writer {
        doc,
        body: String::new(),
        headings: Vec::new(),
        images: BTreeMap::new(),
        losses: Vec::new(),
        paragraphs: 0,
        blocks: 0,
        runs: 0,
        text: 0,
        resource_bytes: 0,
        resource_pixels: 0,
    };
    writer.loss("document/sections", "paper geometry and pagination are replaced by reflow; page and column breaks become line breaks");
    writer.loss("document/styles", "font bytes, language tags and formatting beyond basic text styling are omitted; headings use a flat navigation list");
    writer.loss(
        "document/package",
        "Word package metadata, unused parts and extensions are not copied",
    );
    writer.blocks(&doc.body, "body", 0)?;
    package(writer, title, lang, modified)
}

fn package(writer: Writer<'_>, title: &str, lang: &str, modified: &str) -> Result<Plan, String> {
    const DECL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>"#;
    let mut escaped_title = String::new();
    xml(&mut escaped_title, title)?;
    let title = escaped_title;
    let content = format!(
        r#"{DECL}<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="{lang}" lang="{lang}"><head><title>{title}</title><link rel="stylesheet" type="text/css" href="styles.css"/></head><body>{}</body></html>"#,
        writer.body
    );
    let mut links = String::new();
    let mut points = String::new();
    let headings = if writer.headings.is_empty() {
        vec![(String::from("content.xhtml"), title.clone())]
    } else {
        writer
            .headings
            .iter()
            .map(|(id, label)| {
                let mut text = String::new();
                xml(&mut text, label)?;
                Ok((format!("content.xhtml#p{id}"), text))
            })
            .collect::<Result<_, String>>()?
    };
    for (i, (target, text)) in headings.iter().enumerate() {
        let n = i + 1;
        let _ = write!(links, r#"<li><a href="{target}">{text}</a></li>"#);
        let _ = write!(
            points,
            r#"<navPoint id="n{n}" playOrder="{n}"><navLabel><text>{text}</text></navLabel><content src="{target}"/></navPoint>"#
        );
    }
    let nav = format!(
        r#"{DECL}<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{lang}" lang="{lang}"><head><title>{title}</title></head><body><nav epub:type="toc"><h1>{title}</h1><ol>{links}</ol></nav></body></html>"#
    );
    let mut digest = Sha256::new();
    digest.update(content.as_bytes());
    let mut image_manifest = String::new();
    for (index, image) in writer.images.values().enumerate() {
        digest.update(Sha256::digest(image.bytes));
        let _ = write!(
            image_manifest,
            r#"<item id="image{}" href="{}" media-type="{}"/>"#,
            index + 1,
            image.name,
            image.media_type
        );
    }
    let id = format!("urn:joyword:sha256:{:x}", digest.finalize());
    let ncx = format!(
        r#"{DECL}<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1" xml:lang="{lang}"><head><meta name="dtb:uid" content="{id}"/><meta name="dtb:depth" content="1"/><meta name="dtb:totalPageCount" content="0"/><meta name="dtb:maxPageNumber" content="0"/></head><docTitle><text>{title}</text></docTitle><navMap>{points}</navMap></ncx>"#
    );
    let opf = format!(
        r#"{DECL}<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="book-id" version="3.0" xml:lang="{lang}"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="book-id">{id}</dc:identifier><dc:title>{title}</dc:title><dc:language>{lang}</dc:language><meta property="dcterms:modified">{modified}</meta></metadata><manifest><item id="content" href="content.xhtml" media-type="application/xhtml+xml"/><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="css" href="styles.css" media-type="text/css"/><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>{image_manifest}</manifest><spine toc="ncx"><itemref idref="content"/></spine></package>"#
    );
    let container = format!(
        r#"{DECL}<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="EPUB/package.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#
    );
    let mut files: Vec<(&str, &[u8], Method)> = vec![
        ("mimetype", b"application/epub+zip", Method::Stored),
        (
            "META-INF/container.xml",
            container.as_bytes(),
            Method::Stored,
        ),
        ("EPUB/package.opf", opf.as_bytes(), Method::Stored),
        ("EPUB/content.xhtml", content.as_bytes(), Method::Stored),
        ("EPUB/nav.xhtml", nav.as_bytes(), Method::Stored),
        ("EPUB/toc.ncx", ncx.as_bytes(), Method::Stored),
        ("EPUB/styles.css", CSS.as_bytes(), Method::Stored),
    ];
    let names: Vec<_> = writer
        .images
        .values()
        .map(|i| format!("EPUB/{}", i.name))
        .collect();
    files.extend(
        names
            .iter()
            .zip(writer.images.values())
            .map(|(name, image)| (name.as_str(), image.bytes, Method::Stored)),
    );
    if files.iter().map(|(_, bytes, _)| bytes.len()).sum::<usize>() > MAX_OUTPUT {
        return Err("EPUB output budget exceeded".into());
    }
    let bytes = write_archive(&files).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_OUTPUT {
        return Err("EPUB output budget exceeded".into());
    }
    Ok(Plan {
        bytes,
        losses: writer.losses,
        paragraphs: writer.paragraphs,
        images: writer.images.len(),
    })
}
