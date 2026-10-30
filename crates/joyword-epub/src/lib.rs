//! Reflowable EPUB 3.3, independently implemented from W3C EPUB 3.3 §§4–7.
//! No input/package mutation, filesystem access, external resource fetching or scripts.
use joyoffice_opc::{Method, write_archive};
use joyword_model::{
    Block, BlockSeq, BreakKind, BuiltinStyle, Color, ContentHash, Document, InlineKind,
    Justification, Paragraph, ResourceId, StoryKind,
};
use std::collections::BTreeMap;
use std::fmt;

const MAX_TEXT: usize = 4 << 20;
const MAX_OUTPUT: usize = 64 << 20;
const MAX_BLOCKS: usize = 20000;
const CSS: &str = "body{line-height:1.5}p,h1,h2,h3,h4,h5,h6{white-space:pre-wrap}a{color:inherit;text-decoration:none}img{max-width:100%;height:auto}table{border-collapse:collapse}td{border:1px solid;padding:.3em}";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loss {
    pub location: String,
    pub detail: &'static str,
}

#[derive(Debug)]
pub struct Error {
    pub location: String,
    pub detail: &'static str,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.location, self.detail)
    }
}
impl std::error::Error for Error {}
fn error(location: impl Into<String>, detail: &'static str) -> Error {
    Error {
        location: location.into(),
        detail,
    }
}

pub struct Metadata<'a> {
    pub title: &'a str,
    pub language: &'a str,
    pub modified: &'a str,
}
#[derive(Debug)]
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
}

/// UTC timestamp with a bounded, independent Gregorian calendar conversion.
pub fn utc_timestamp(seconds: u64) -> Result<String, Error> {
    let mut days = seconds / 86400;
    let mut year = 1970;
    loop {
        let length = if leap(year) { 366 } else { 365 };
        if days < length {
            break;
        }
        days -= length;
        year += 1;
        if year > 9999 {
            return Err(error(
                "metadata/modified",
                "date outside four-digit year range",
            ));
        }
    }
    let lengths = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0;
    while days >= lengths[month] {
        days -= lengths[month];
        month += 1;
    }
    let time = seconds % 86400;
    Ok(format!(
        "{year:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        month + 1,
        days + 1,
        time / 3600,
        time / 60 % 60,
        time % 60
    ))
}
fn leap(year: u64) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

fn valid_language(value: &str) -> bool {
    let mut parts = value.split('-');
    value.len() <= 64
        && parts.next().is_some_and(|part| {
            (2..=8).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_alphabetic())
        })
        && parts.all(|part| {
            (1..=8).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_alphanumeric())
        })
}
fn valid_modified(value: &str) -> bool {
    if !joyword_model::is_strict_utc_date(value) {
        return false;
    }
    let year = value[0..4].parse::<u64>().unwrap_or(0);
    let month = value[5..7].parse::<usize>().unwrap_or(0);
    let day = value[8..10].parse::<u64>().unwrap_or(0);
    let lengths = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    year > 0
        && (1..=12).contains(&month)
        && day > 0
        && day <= lengths[month - 1]
        && value[11..13].parse::<u8>().is_ok_and(|v| v < 24)
        && value[14..16].parse::<u8>().is_ok_and(|v| v < 60)
        && value[17..19].parse::<u8>().is_ok_and(|v| v < 60)
}

fn xml(text: &str) -> Result<String, Error> {
    let mut out = String::new();
    for ch in text.chars() {
        if (ch < '\u{20}' && !matches!(ch, '\n' | '\r' | '\t'))
            || matches!(ch, '\u{fffe}' | '\u{ffff}')
        {
            return Err(error("text", "character cannot be represented in XML 1.0"));
        }
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    Ok(out)
}

struct Writer<'a> {
    document: &'a Document,
    body: String,
    headings: Vec<(String, String)>,
    images: BTreeMap<ResourceId, (String, &'a str, Vec<u8>)>,
    losses: Vec<Loss>,
    paragraphs: usize,
    blocks: usize,
    runs: usize,
    text: usize,
    resource_bytes: usize,
}
impl Writer<'_> {
    fn blocks(&mut self, blocks: &BlockSeq, location: &str, depth: usize) -> Result<(), Error> {
        if depth > joyword_model::MAX_NESTING {
            return Err(error(location, "nesting budget exceeded"));
        }
        for (index, block) in blocks.iter().enumerate() {
            self.blocks += 1;
            let path = format!("{location}/block[{}]", index + 1);
            if self.blocks > MAX_BLOCKS {
                return Err(error(&path, "block budget exceeded"));
            }
            match block {
                Block::Paragraph(p) => self.paragraph(p, &path)?,
                Block::Table(t) => {
                    if t.props_revision.is_some() {
                        return Err(error(&path, "unresolved table revision"));
                    }
                    self.losses.push(Loss{ location: path.clone(), detail:"table geometry, borders, shading, alignment and table styles replaced by a reflow grid" });
                    self.body.push_str("<table><tbody>");
                    for (row_index, row) in t.rows.iter().enumerate() {
                        if row.props_revision.is_some() {
                            return Err(error(&path, "unresolved row revision"));
                        }
                        self.body.push_str("<tr>");
                        for (cell_index, cell) in row.cells.iter().enumerate() {
                            if cell.props_revision.is_some()
                                || cell.vertical_merge != joyword_model::VerticalMerge::None
                            {
                                return Err(error(
                                    &path,
                                    "revised or vertically merged cells are not supported",
                                ));
                            }
                            self.body
                                .push_str(&format!("<td colspan=\"{}\">", cell.grid_span));
                            self.blocks(
                                &cell.blocks,
                                &format!("{path}/row[{}]/cell[{}]", row_index + 1, cell_index + 1),
                                depth + 1,
                            )?;
                            self.body.push_str("</td>");
                        }
                        self.body.push_str("</tr>");
                    }
                    self.body.push_str("</tbody></table>");
                }
                Block::Sdt(_) => {
                    return Err(error(
                        &path,
                        "content controls require an explicit conversion policy",
                    ));
                }
                Block::Opaque(_) => {
                    return Err(error(&path, "preserved content cannot be silently omitted"));
                }
            }
            if self.body.len() > MAX_OUTPUT {
                return Err(error(&path, "output budget exceeded"));
            }
        }
        Ok(())
    }

    fn paragraph(&mut self, p: &Paragraph, path: &str) -> Result<(), Error> {
        if p.mark_revision.is_some() || p.props_revision.is_some() || p.props.preserved.is_some() {
            return Err(error(
                path,
                "revised or preserved paragraph properties are not supported",
            ));
        }
        self.text = self
            .text
            .checked_add(p.text().summary().bytes)
            .ok_or_else(|| error(path, "text budget exceeded"))?;
        if self.text > MAX_TEXT {
            return Err(error(path, "text budget exceeded"));
        }
        self.paragraphs += 1;
        let resolved = self
            .document
            .styles()
            .resolve_paragraph(p.style.as_ref(), &p.props);
        if !resolved.problems.is_empty() {
            return Err(error(path, "paragraph style could not be resolved"));
        }
        if resolved.props.numbering.is_some_and(|n| n.num_id != 0) {
            return Err(error(path, "numbered paragraphs are not supported yet"));
        }
        let level = resolved
            .props
            .outline_level
            .map(|v| v.saturating_add(1))
            .or_else(|| {
                match p
                    .style
                    .as_ref()
                    .and_then(|id| self.document.styles().get(id))
                    .and_then(|s| s.builtin())
                {
                    Some(BuiltinStyle::Heading(level)) => Some(level),
                    _ => None,
                }
            });
        let tag = level
            .map(|v| format!("h{}", v.clamp(1, 6)))
            .unwrap_or_else(|| "p".into());
        let id = format!("p{}", self.paragraphs);
        if level.is_some() && !p.raw_text().trim().is_empty() {
            self.headings.push((id.clone(), p.raw_text()));
            self.losses.push(Loss {
                location: path.into(),
                detail: "navigation uses a flat heading list; heading levels above six become h6",
            });
        }
        let align = match resolved.props.justification.unwrap_or(Justification::Start) {
            Justification::Start => "start",
            Justification::End => "end",
            Justification::Center => "center",
            _ => "justify",
        };
        self.body.push_str(&format!(
            "<{tag} id=\"{id}\" dir=\"auto\" style=\"text-align:{align}\">"
        ));
        if resolved.props.indent != Default::default()
            || resolved.props.spacing != Default::default()
            || resolved.props.tabs.is_some()
        {
            self.losses.push(Loss {
                location: path.into(),
                detail: "paragraph indents, spacing and tab stops use reader reflow defaults",
            });
        }
        let chars: Vec<char> = p.text().to_raw_string().chars().collect();
        let mut at: usize = 0;
        let mut inlines = p.inlines().iter();
        for (index, run) in p.runs().iter().enumerate() {
            self.runs += 1;
            if self.runs > 32768 {
                return Err(error(path, "run budget exceeded"));
            }
            let run_path = format!("{path}/run[{}]", index + 1);
            if run.revision.is_some()
                || run.format_revision.is_some()
                || run.props.preserved.is_some()
            {
                return Err(error(
                    &run_path,
                    "revised or preserved run content is not supported",
                ));
            }
            let resolved = self.document.styles().resolve_run(
                p.style.as_ref(),
                run.style.as_ref(),
                &run.props,
            );
            if !resolved.problems.is_empty() {
                return Err(error(&run_path, "run style could not be resolved"));
            }
            let props = resolved.props;
            if props.hidden == Some(true) {
                return Err(error(&run_path, "hidden text needs an explicit projection"));
            }
            if props.fonts != Default::default() {
                self.losses.push(Loss {
                    location: run_path.clone(),
                    detail: "source font bytes are not embedded; reader font fallback applies",
                });
            }
            let mut css = format!(
                "font-weight:{};font-style:{};",
                if props.bold == Some(true) {
                    "bold"
                } else {
                    "normal"
                },
                if props.italic == Some(true) {
                    "italic"
                } else {
                    "normal"
                }
            );
            let underline = props
                .underline
                .is_some_and(|u| u != joyword_model::Underline::None);
            if props.underline.is_some_and(|u| {
                !matches!(
                    u,
                    joyword_model::Underline::None | joyword_model::Underline::Single
                )
            }) {
                self.losses.push(Loss {
                    location: run_path.clone(),
                    detail: "underline variant simplified to a continuous underline",
                });
            }
            if underline || props.strike == Some(true) {
                css.push_str("text-decoration:");
                if underline {
                    css.push_str("underline ");
                }
                if props.strike == Some(true) {
                    css.push_str("line-through");
                }
                css.push(';');
            }
            if let Some(size) = props.size {
                let points = size
                    .absolute()
                    .map(|x| x.points())
                    .filter(|v| *v > 0.0 && *v <= 1000.0)
                    .ok_or_else(|| error(&run_path, "font size outside EPUB subset"))?;
                css.push_str(&format!("font-size:{points:.2}pt;"));
            }
            match props.color {
                Some(Color::Rgb(r, g, b)) => {
                    css.push_str(&format!("color:#{r:02x}{g:02x}{b:02x};"))
                }
                Some(Color::Theme { .. }) => {
                    return Err(error(&run_path, "unresolved theme color"));
                }
                _ => {}
            }
            let href = if let Some(link) = &run.link {
                if link.anchor.is_some() {
                    return Err(error(
                        &run_path,
                        "internal hyperlink anchors are not supported yet",
                    ));
                }
                Some(
                    joyword_model::open_target(link)
                        .map_err(|_| error(&run_path, "hyperlink target is not allowed"))?,
                )
            } else {
                None
            };
            if let Some(href) = href {
                self.body.push_str(&format!("<a href=\"{}\">", xml(href)?));
            }
            self.body.push_str(&format!("<span style=\"{css}\">"));
            let end = at
                .checked_add(run.len)
                .filter(|v| *v <= chars.len())
                .ok_or_else(|| error(&run_path, "invalid model run range"))?;
            for ch in &chars[at..end] {
                if *ch == joyword_model::OBJECT_MARKER {
                    let inline = inlines
                        .next()
                        .ok_or_else(|| error(&run_path, "missing inline object"))?;
                    match &inline.kind {
                        InlineKind::Break(BreakKind::Line) => self.body.push_str("<br/>"),
                        InlineKind::Break(_) => {
                            self.body.push_str("<br/>");
                            self.losses.push(Loss {
                                location: run_path.clone(),
                                detail: "physical page or column break becomes a reflow line break",
                            });
                        }
                        InlineKind::Image {
                            resource,
                            width,
                            anchor: None,
                            ..
                        } => self.image(*resource, *width, &run_path)?,
                        _ => {
                            return Err(error(
                                &run_path,
                                "inline object cannot be represented in this EPUB subset",
                            ));
                        }
                    }
                } else {
                    self.body.push_str(&xml(&ch.to_string())?);
                }
            }
            at = end;
            self.body.push_str("</span>");
            if href.is_some() {
                self.body.push_str("</a>");
            }
        }
        if at != chars.len() || inlines.next().is_some() {
            return Err(error(path, "model text and run coverage disagree"));
        }
        self.body.push_str(&format!("</{tag}>\n"));
        Ok(())
    }

    fn image(
        &mut self,
        id: ResourceId,
        width: joyword_model::Measure,
        path: &str,
    ) -> Result<(), Error> {
        if !self.images.contains_key(&id) {
            let resource = self
                .document
                .resources()
                .get(id)
                .ok_or_else(|| error(path, "missing image resource"))?;
            let extension = match resource.media_type() {
                "image/png" => "png",
                "image/jpeg" => "jpg",
                _ => return Err(error(path, "only PNG and JPEG images are supported")),
            };
            self.resource_bytes = self
                .resource_bytes
                .checked_add(resource.data().len())
                .ok_or_else(|| error(path, "resource budget exceeded"))?;
            if self.resource_bytes > 32 << 20 || self.images.len() >= 1024 {
                return Err(error(path, "resource budget exceeded"));
            }
            let accepted = joyword_model::accept_image(
                resource.data(),
                &joyword_model::ImageLimits {
                    max_bytes: 8 << 20,
                    max_pixels: 16_777_216,
                    max_dimension: 4096,
                },
            )
            .map_err(|_| error(path, "invalid or over-budget image"))?;
            if accepted.media_type != resource.media_type() {
                return Err(error(path, "image media type disagrees with bytes"));
            }
            let name = format!("images/image-{}.{}", self.images.len() + 1, extension);
            self.images
                .insert(id, (name, resource.media_type(), resource.data().to_vec()));
        }
        let name = &self.images[&id].0;
        let points = width
            .absolute()
            .map(|v| v.points())
            .filter(|v| *v > 0.0 && *v <= 10000.0)
            .ok_or_else(|| error(path, "image extent outside EPUB subset"))?;
        self.body.push_str(&format!(
            "<img src=\"{name}\" style=\"width:{points:.2}pt\" alt=\"Image\"/>"
        ));
        self.losses.push(Loss {
            location: path.into(),
            detail: "image uses responsive width; source height/stretch and alternative text require review",
        });
        Ok(())
    }
}

pub fn export(document: &Document, metadata: &Metadata<'_>) -> Result<Plan, Error> {
    if document.validate().has_errors() {
        return Err(error("document", "invalid model"));
    }
    if metadata.title.trim().is_empty() || metadata.title.len() > 4096 {
        return Err(error(
            "metadata/title",
            "title required, maximum 4096 bytes",
        ));
    }
    if !valid_language(metadata.language) {
        return Err(error(
            "metadata/language",
            "language tag must use ASCII subtags",
        ));
    }
    if !valid_modified(metadata.modified) {
        return Err(error("metadata/modified", "UTC date-time required"));
    }
    if document.annotations().next().is_some() || !document.move_markers().is_empty() {
        return Err(error(
            "document/annotations",
            "annotations and fields need an explicit conversion policy",
        ));
    }
    for story in document.stories().filter(|s| s.kind != StoryKind::Body) {
        if story
            .blocks
            .iter()
            .any(|b| b.as_paragraph().is_none_or(|p| !p.text().is_empty()))
        {
            return Err(error(
                format!("story/{:?}", story.id),
                "non-body story cannot be silently omitted",
            ));
        }
    }
    let mut writer = Writer {
        document,
        body: String::new(),
        headings: Vec::new(),
        images: BTreeMap::new(),
        losses: vec![
            Loss {
                location: "document/sections".into(),
                detail: "EPUB reflows pages; paper geometry, grids and pagination are not retained",
            },
            Loss {
                location: "document/package".into(),
                detail: "Word package metadata, unused parts and extensions are not copied; the original DOCX remains unchanged",
            },
        ],
        paragraphs: 0,
        blocks: 0,
        runs: 0,
        text: 0,
        resource_bytes: 0,
    };
    writer.blocks(&document.body().blocks, "body", 0)?;
    let title = xml(metadata.title)?;
    let lang = xml(metadata.language)?;
    let content = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><html xmlns=\"http://www.w3.org/1999/xhtml\" xml:lang=\"{lang}\" lang=\"{lang}\"><head><title>{title}</title><link rel=\"stylesheet\" type=\"text/css\" href=\"styles.css\"/></head><body>{}</body></html>",
        writer.body
    );
    let mut links = String::new();
    if writer.headings.is_empty() {
        links.push_str(&format!("<li><a href=\"content.xhtml\">{title}</a></li>"));
    }
    for (id, text) in &writer.headings {
        links.push_str(&format!(
            "<li><a href=\"content.xhtml#{id}\">{}</a></li>",
            xml(text)?
        ));
    }
    let nav = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"{lang}\" lang=\"{lang}\"><head><title>{title}</title></head><body><nav epub:type=\"toc\" id=\"toc\"><h1>{title}</h1><ol>{links}</ol></nav></body></html>"
    );
    let mut hash_input = content.clone();
    let mut image_manifest = String::new();
    for (i, (name, media, data)) in writer.images.values().enumerate() {
        hash_input.push_str(&ContentHash::of(data).to_string());
        image_manifest.push_str(&format!(
            "<item id=\"image{}\" href=\"{name}\" media-type=\"{media}\"/>",
            i + 1
        ));
    }
    let identifier = ContentHash::of(hash_input.as_bytes());
    // Optional EPUB 2 navigation for readers which do not expose the EPUB 3
    // navigation document. Both are derived from the same ordered heading list.
    let mut points = String::new();
    for (index, (id, text)) in writer.headings.iter().enumerate() {
        points.push_str(&format!("<navPoint id=\"n{}\" playOrder=\"{}\"><navLabel><text>{}</text></navLabel><content src=\"content.xhtml#{id}\"/></navPoint>", index + 1, index + 1, xml(text)?));
    }
    if points.is_empty() {
        points = format!(
            "<navPoint id=\"n1\" playOrder=\"1\"><navLabel><text>{title}</text></navLabel><content src=\"content.xhtml\"/></navPoint>"
        );
    }
    let ncx = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ncx xmlns=\"http://www.daisy.org/z3986/2005/ncx/\" version=\"2005-1\" xml:lang=\"{lang}\"><head><meta name=\"dtb:uid\" content=\"urn:joyword:sha256:{identifier}\"/><meta name=\"dtb:depth\" content=\"1\"/><meta name=\"dtb:totalPageCount\" content=\"0\"/><meta name=\"dtb:maxPageNumber\" content=\"0\"/></head><docTitle><text>{title}</text></docTitle><navMap>{points}</navMap></ncx>"
    );
    let opf = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><package xmlns=\"http://www.idpf.org/2007/opf\" unique-identifier=\"book-id\" version=\"3.0\" xml:lang=\"{lang}\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"book-id\">urn:joyword:sha256:{identifier}</dc:identifier><dc:title>{title}</dc:title><dc:language>{lang}</dc:language><meta property=\"dcterms:modified\">{}</meta></metadata><manifest><item id=\"content\" href=\"content.xhtml\" media-type=\"application/xhtml+xml\"/><item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/><item id=\"css\" href=\"styles.css\" media-type=\"text/css\"/>{image_manifest}</manifest><spine><itemref idref=\"content\"/></spine></package>",
        xml(metadata.modified)?
    );
    let opf = opf
        .replace(
            "</manifest>",
            r#"<item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/></manifest>"#,
        )
        .replace("<spine>", r#"<spine toc="ncx">"#);
    let mut entries = vec![("mimetype".into(),b"application/epub+zip".to_vec()),("META-INF/container.xml".into(),b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><container xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\" version=\"1.0\"><rootfiles><rootfile full-path=\"EPUB/package.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>".to_vec()),("EPUB/package.opf".into(),opf.into_bytes()),("EPUB/content.xhtml".into(),content.into_bytes()),("EPUB/nav.xhtml".into(),nav.into_bytes()),("EPUB/styles.css".into(),CSS.as_bytes().to_vec())];
    let images = writer.images.len();
    entries.push(("EPUB/toc.ncx".into(), ncx.into_bytes()));
    for (name, _, data) in writer.images.into_values() {
        entries.push((format!("EPUB/{name}"), data));
    }
    let total: usize = entries.iter().map(|(_, bytes)| bytes.len()).sum();
    if total > MAX_OUTPUT {
        return Err(error("container", "output byte budget exceeded"));
    }
    let refs: Vec<(&str, &[u8], Method)> = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice(), Method::Stored))
        .collect();
    let bytes = write_archive(&refs).map_err(|_| error("container", "ZIP write failed"))?;
    if bytes.len() > MAX_OUTPUT {
        return Err(error("container", "output byte budget exceeded"));
    }
    Ok(Plan {
        bytes,
        losses: writer.losses,
        paragraphs: writer.paragraphs,
        images,
    })
}
