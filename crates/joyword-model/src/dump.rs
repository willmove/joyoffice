//! Deterministic text serialization of the model for golden tests and the T004
//! semantic comparator. It is not a file format and is never read back.
//!
//! With `node_ids: false` the dump omits allocated identifiers and names
//! paragraphs by their document-order index, so two imports of the same file
//! compare equal even though their identifiers differ.

use std::collections::HashMap;
use std::fmt::Write;

use crate::annotation::{Anchor, AnnotationKind, Position};
use crate::block::{
    Block, BlockSeq, BorderEdge, CellBorders, InlineKind, Paragraph, Preserved, TableWidth,
};
use crate::document::Document;
use crate::id::NodeId;
use crate::props::{Color, FontRef, FontSlots, Languages, ParaProps, RunProps, Toggle};
use crate::text::OBJECT_MARKER;

#[derive(Clone, Copy, Debug)]
pub struct DumpOptions {
    pub node_ids: bool,
}

fn edge(edge: Option<BorderEdge>) -> String {
    match edge {
        None => "-".into(),
        Some(edge) => format!("{:?}/{}:{:?}", edge.kind, edge.size_eighths, edge.color),
    }
}

fn border_edges(borders: &CellBorders) -> String {
    format!(
        "t={} l={} b={} r={}",
        edge(borders.top),
        edge(borders.left),
        edge(borders.bottom),
        edge(borders.right)
    )
}

fn toggle(out: &mut Vec<String>, name: &str, value: Toggle) {
    if let Some(on) = value {
        out.push(format!("{name}={}", u8::from(on)));
    }
}

fn font(value: &FontRef) -> String {
    match value {
        FontRef::Named(name) => format!("{name:?}"),
        FontRef::Theme(theme) => format!("theme:{theme:?}"),
    }
}

fn fonts(out: &mut Vec<String>, slots: &FontSlots) {
    for (name, slot) in [
        ("ascii", &slots.ascii),
        ("hAnsi", &slots.h_ansi),
        ("eastAsia", &slots.east_asia),
        ("cs", &slots.complex_script),
    ] {
        if let Some(value) = slot {
            out.push(format!("font.{name}={}", font(value)));
        }
    }
    if let Some(hint) = slots.hint {
        out.push(format!("font.hint={hint:?}"));
    }
}

fn languages(out: &mut Vec<String>, languages: &Languages) {
    for (name, tag) in [
        ("latin", &languages.latin),
        ("eastAsia", &languages.east_asia),
        ("bidi", &languages.bidi),
    ] {
        if let Some(tag) = tag {
            out.push(format!("lang.{name}={tag}"));
        }
    }
}

fn run_props(props: &RunProps) -> String {
    let mut out = Vec::new();
    toggle(&mut out, "b", props.bold);
    toggle(&mut out, "i", props.italic);
    toggle(&mut out, "strike", props.strike);
    toggle(&mut out, "vanish", props.hidden);
    if let Some(underline) = props.underline {
        out.push(format!("u={underline:?}"));
    }
    if let Some(size) = props.size {
        out.push(format!("sz={size}"));
    }
    fonts(&mut out, &props.fonts);
    match props.color {
        Some(Color::Auto) => out.push("color=auto".into()),
        Some(Color::Rgb(r, g, b)) => out.push(format!("color={r:02X}{g:02X}{b:02X}")),
        Some(Color::Theme(slot)) => out.push(format!("color=theme:{}", slot.word_name())),
        None => {}
    }
    languages(&mut out, &props.languages);
    if let Some(preserved) = props.preserved {
        out.push(format!("preserved={preserved}"));
    }
    format!("{{{}}}", out.join(" "))
}

fn para_props(props: &ParaProps) -> String {
    let mut out = Vec::new();
    if let Some(jc) = props.justification {
        out.push(format!("jc={jc:?}"));
    }
    let indent = &props.indent;
    for (name, value) in [
        ("ind.start", indent.start),
        ("ind.end", indent.end),
        ("ind.firstLine", indent.first_line),
        ("ind.hanging", indent.hanging),
        ("spacing.before", props.spacing.before),
        ("spacing.after", props.spacing.after),
    ] {
        if let Some(value) = value {
            out.push(format!("{name}={value}"));
        }
    }
    toggle(&mut out, "spacing.beforeAuto", props.spacing.before_auto);
    toggle(&mut out, "spacing.afterAuto", props.spacing.after_auto);
    if let Some(line) = props.spacing.line {
        out.push(format!("spacing.line={line:?}"));
    }
    toggle(&mut out, "keepNext", props.keep_next);
    toggle(&mut out, "keepLines", props.keep_lines);
    toggle(&mut out, "widowControl", props.widow_control);
    toggle(&mut out, "pageBreakBefore", props.page_break_before);
    toggle(&mut out, "snapToGrid", props.snap_to_grid);
    if let Some(level) = props.outline_level {
        out.push(format!("outlineLvl={level}"));
    }
    if let Some(numbering) = props.numbering {
        out.push(format!("num={}:{}", numbering.num_id, numbering.level));
    }
    if let Some(preserved) = props.preserved {
        out.push(format!("preserved={preserved}"));
    }
    format!("{{{}}}", out.join(" "))
}

fn preserved(value: &Preserved) -> String {
    format!("element={:?} handle={}", value.element, value.handle)
}

struct Dumper<'a> {
    options: DumpOptions,
    out: String,
    /// Paragraph document-order index, for identifier-free positions.
    order: HashMap<NodeId, usize>,
    document: &'a Document,
}

impl Dumper<'_> {
    fn line(&mut self, depth: usize, text: &str) {
        for _ in 0..depth {
            self.out.push_str("  ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn label(&self, kind: &str, id: NodeId) -> String {
        if self.options.node_ids {
            format!("{kind} {id}")
        } else {
            kind.to_string()
        }
    }

    fn blocks(&mut self, blocks: &BlockSeq, depth: usize) {
        for block in blocks.iter() {
            match block {
                Block::Paragraph(paragraph) => self.paragraph(paragraph, depth),
                Block::Table(table) => {
                    let grid: Vec<String> = table.grid.iter().map(ToString::to_string).collect();
                    let style = table
                        .style
                        .as_ref()
                        .map_or("-".into(), |s| format!("{s:?}"));
                    let head = self.label("table", table.id);
                    let width = table.width.map_or("-".into(), |width| match width {
                        TableWidth::Auto => "auto".into(),
                        TableWidth::Dxa(value) => format!("{value}dxa"),
                    });
                    self.line(
                        depth,
                        &format!("{head} style={style} grid=[{}] width={width}", grid.join(",")),
                    );
                    for row in &table.rows {
                        let head = self.label("row", row.id);
                        self.line(
                            depth + 1,
                            &format!(
                                "{head} before={} after={} header={}",
                                row.grid_before,
                                row.grid_after,
                                u8::from(row.header)
                            ),
                        );
                        for cell in &row.cells {
                            let head = self.label("cell", cell.id);
                            let width = cell.width.map_or("-".into(), |w| w.to_string());
                            self.line(
                                depth + 2,
                                &format!(
                                    "{head} span={} vmerge={:?} width={width} borders={} shading={}",
                                    cell.grid_span,
                                    cell.vertical_merge,
                                    border_edges(&cell.borders),
                                    cell.shading
                                        .map_or("-".into(), |shading| format!("{:?}", shading.fill))
                                ),
                            );
                            self.blocks(&cell.blocks, depth + 3);
                        }
                    }
                }
                Block::Sdt(sdt) => {
                    let head = self.label("sdt", sdt.id);
                    self.line(
                        depth,
                        &format!(
                            "{head} tag={:?} alias={:?} lock={:?}",
                            sdt.props.tag, sdt.props.alias, sdt.props.lock
                        ),
                    );
                    self.blocks(&sdt.blocks, depth + 1);
                }
                Block::Opaque(opaque) => {
                    let head = self.label("opaque", opaque.id);
                    self.line(depth, &format!("{head} {}", preserved(&opaque.preserved)));
                }
            }
        }
    }

    fn paragraph(&mut self, paragraph: &Paragraph, depth: usize) {
        let ordinal = self.order.len();
        self.order.insert(paragraph.id(), ordinal);
        let head = self.label("paragraph", paragraph.id());
        let style = paragraph
            .style
            .as_ref()
            .map_or("-".into(), |s| format!("{s:?}"));
        self.line(
            depth,
            &format!(
                "{head} p{ordinal} style={style} props={} mark={}",
                para_props(&paragraph.props),
                run_props(&paragraph.mark_props)
            ),
        );
        let chars: Vec<char> = paragraph.text().chars().collect();
        let mut inlines = paragraph.inlines().iter();
        let mut start = 0;
        for run in paragraph.runs() {
            let end = (start + run.len).min(chars.len());
            let style = run.style.as_ref().map_or("-".into(), |s| format!("{s:?}"));
            let mut parts = Vec::new();
            let mut text = String::new();
            for &ch in &chars[start..end] {
                if ch == OBJECT_MARKER {
                    if !text.is_empty() {
                        parts.push(format!("{:?}", std::mem::take(&mut text)));
                    }
                    parts.push(match inlines.next() {
                        Some(inline) => {
                            let kind = match &inline.kind {
                                InlineKind::Break(kind) => format!("break:{kind:?}"),
                                InlineKind::Image {
                                    resource,
                                    width,
                                    height,
                                } => format!("image:{resource}:{width}x{height}"),
                                InlineKind::Preserved(value) => {
                                    format!("preserved:{}", preserved(value))
                                }
                            };
                            if self.options.node_ids {
                                format!("<{} {kind}>", inline.id)
                            } else {
                                format!("<{kind}>")
                            }
                        }
                        None => "<missing>".into(),
                    });
                } else {
                    text.push(ch);
                }
            }
            if !text.is_empty() {
                parts.push(format!("{text:?}"));
            }
            self.line(
                depth + 1,
                &format!(
                    "run style={style} props={} {}",
                    run_props(&run.props),
                    parts.join(" ")
                ),
            );
            start = end;
        }
    }

    fn position(&self, position: &Position) -> String {
        let paragraph = match self.order.get(&position.paragraph) {
            Some(ordinal) if !self.options.node_ids => format!("p{ordinal}"),
            _ => position.paragraph.to_string(),
        };
        let bias = match position.bias {
            crate::Bias::Before => '<',
            crate::Bias::After => '>',
        };
        format!("{paragraph}:{}{bias}", position.offset.0)
    }
}

impl Document {
    pub fn canonical_dump(&self, options: DumpOptions) -> String {
        let mut dumper = Dumper {
            options,
            out: String::new(),
            order: HashMap::new(),
            document: self,
        };
        let document = dumper.document;
        dumper.line(0, "joyword-model-dump 1");
        let settings = document.settings();
        dumper.line(
            0,
            &format!(
                "settings compatibilityMode={:?} evenAndOddHeaders={:?} trackRevisions={:?}",
                settings.compatibility_mode,
                settings.even_and_odd_headers,
                settings.track_revisions
            ),
        );
        let defaults = &document.styles().defaults;
        dumper.line(
            0,
            &format!(
                "defaults run={} para={}",
                run_props(&defaults.run),
                para_props(&defaults.para)
            ),
        );
        for style in document.styles().iter() {
            let mut line = format!(
                "style {:?} {} name={:?}",
                style.id.as_str(),
                style.kind,
                style.name
            );
            for (name, value) in [
                ("basedOn", &style.based_on),
                ("next", &style.next),
                ("link", &style.link),
            ] {
                if let Some(value) = value {
                    let _ = write!(line, " {name}={:?}", value.as_str());
                }
            }
            if style.is_default {
                line.push_str(" default");
            }
            let _ = write!(
                line,
                " run={} para={}",
                run_props(&style.run),
                para_props(&style.para)
            );
            dumper.line(0, &line);
        }
        for story in document.stories() {
            let head = if options.node_ids {
                format!("story {} {}", story.id, story.kind)
            } else {
                format!("story {}", story.kind)
            };
            dumper.line(0, &head);
            dumper.blocks(&story.blocks, 1);
        }
        for section in document.sections() {
            let page = &section.page;
            let boundary = match section.last_block {
                None => "end".to_string(),
                Some(block) if options.node_ids => block.to_string(),
                Some(_) => "block".to_string(),
            };
            let head = dumper.label("section", section.id);
            dumper.line(
                0,
                &format!(
                    "{head} last={boundary} page={}x{} {:?} margins={},{},{},{} header={} footer={} gutter={} titlePg={:?} headers={} footers={}",
                    page.width,
                    page.height,
                    page.orientation,
                    page.top,
                    page.right,
                    page.bottom,
                    page.left,
                    page.header,
                    page.footer,
                    page.gutter,
                    section.title_page,
                    section.headers.len(),
                    section.footers.len()
                ),
            );
        }
        for annotation in document.annotations() {
            let kind = match &annotation.kind {
                AnnotationKind::Bookmark { name } => format!("bookmark name={name:?}"),
                AnnotationKind::Comment { .. } => "comment".to_string(),
                AnnotationKind::Field { separator } => match separator {
                    Some(separator) => format!("field separator={}", dumper.position(separator)),
                    None => "field".to_string(),
                },
                AnnotationKind::Permission { editor, group } => {
                    format!("permission editor={editor:?} group={group:?}")
                }
                AnnotationKind::ContentControl(props) => {
                    format!("sdt tag={:?} lock={:?}", props.tag, props.lock)
                }
            };
            let anchor = match &annotation.anchor {
                Anchor::Range { start, end } => {
                    format!("{}..{}", dumper.position(start), dumper.position(end))
                }
                Anchor::Detached => "detached".to_string(),
            };
            let head = if options.node_ids {
                format!("annotation {}", annotation.id)
            } else {
                "annotation".to_string()
            };
            dumper.line(0, &format!("{head} {kind} {anchor}"));
        }
        for resource in document.resources().iter() {
            let head = if options.node_ids {
                format!("resource {}", resource.id())
            } else {
                "resource".to_string()
            };
            dumper.line(
                0,
                &format!(
                    "{head} {} {:?} bytes={}",
                    resource.media_type(),
                    resource.hash(),
                    resource.data().len()
                ),
            );
        }
        dumper.out
    }
}

/// First difference between two documents' identifier-free dumps (T004
/// semantic comparison). Line numbers are 1-based; a missing line is empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticDifference {
    pub line: usize,
    pub expected: String,
    pub actual: String,
}

impl Document {
    /// `None` when both documents have the same semantic content, ignoring
    /// allocated identifiers.
    pub fn semantic_difference(&self, actual: &Document) -> Option<SemanticDifference> {
        let options = DumpOptions { node_ids: false };
        let expected = self.canonical_dump(options);
        let actual = actual.canonical_dump(options);
        let mut left = expected.lines();
        let mut right = actual.lines();
        let mut line = 0;
        loop {
            line += 1;
            match (left.next(), right.next()) {
                (None, None) => return None,
                (a, b) if a == b => continue,
                (a, b) => {
                    return Some(SemanticDifference {
                        line,
                        expected: a.unwrap_or_default().to_owned(),
                        actual: b.unwrap_or_default().to_owned(),
                    });
                }
            }
        }
    }
}
