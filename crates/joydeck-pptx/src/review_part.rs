//! Speaker notes and comment parts. Modern and legacy comments share an author inventory.

use std::collections::HashMap;

use joydeck_model::{Comment, PhType, Placeholder, Shape, ShapeId, Slide, TextBody};

use crate::opc::{self, NS_A, NS_P188, NS_R, Package};
use crate::read::{Imp, Part, dml};
use crate::write::{Exp, Out, csld, root_open};
use crate::xml::{A, Doc, El, W};

const MAX_THREADS: usize = 10_000;
const MAX_REPLIES: usize = 1000;

pub(super) struct Reader<'a> {
    source: &'a Part,
    notes: HashMap<String, TextBody>,
    authors: Option<HashMap<String, (String, String)>>,
}

impl<'a> Reader<'a> {
    pub(super) fn new(source: &'a Part) -> Self {
        Self {
            source,
            notes: HashMap::new(),
            authors: None,
        }
    }

    pub(super) fn attach(&mut self, imp: &mut Imp, part: &Part, slide: &mut Slide) {
        let package = imp.pkg;
        if let Some(target) = part.rels.first_of("notesSlide").filter(|rel| !rel.external) {
            let key = opc::norm(&target.target);
            if let Some(body) = self.notes.get(&key) {
                slide.notes = body.clone();
            } else if let Some(document) = crate::read::load(package, &target.target) {
                let owner = Part::new(package, &target.target, &document);
                slide.notes = decode_notes(imp, &owner, &document.root);
                self.notes.insert(key, slide.notes.clone());
            }
        }
        let comments: Vec<_> = part
            .rels
            .all_of("comments")
            .filter(|rel| !rel.external)
            .filter_map(|rel| {
                let doc = package.xml(&rel.target)?;
                let modern = package.content_type(&rel.target).as_deref()
                    == Some(crate::write::CT_COMMENTS_MODERN)
                    || modern_root(&doc);
                Some((doc, modern))
            })
            .collect();
        if comments.is_empty() {
            return;
        }
        let prefer_modern = comments.iter().any(|(_, modern)| *modern);
        let authors = self
            .authors
            .get_or_insert_with(|| read_authors(package, self.source));
        for (doc, modern) in comments {
            if modern == prefer_modern {
                slide
                    .comments
                    .extend(doc.root.children_named("cm").take(MAX_THREADS).map(|node| {
                        decode_comment(node, authors, if modern { 12_700.0 } else { 8.0 }, 0)
                    }));
            }
        }
    }
}

fn modern_root(doc: &Doc) -> bool {
    let prefix = doc.root.prefix();
    let declaration = if prefix.is_empty() {
        "xmlns".to_owned()
    } else {
        format!("xmlns:{prefix}")
    };
    doc.root
        .attrs
        .iter()
        .any(|(name, uri)| name == &declaration && uri == NS_P188)
        || prefix == "p188"
}

fn read_authors(package: &Package, presentation: &Part) -> HashMap<String, (String, String)> {
    let mut authors = HashMap::new();
    for kind in ["commentAuthors", "authors"] {
        if let Some(rel) = presentation.rels.first_of(kind).filter(|rel| !rel.external)
            && let Some(document) = package.xml(&rel.target)
        {
            for author in document.root.elements() {
                if let Some(id) = author.attr("id") {
                    authors.insert(
                        id.into(),
                        (value(author, "name"), value(author, "initials")),
                    );
                }
            }
        }
    }
    authors
}

fn value(node: &El, name: &str) -> String {
    node.attr(name).unwrap_or_default().into()
}

pub(super) fn speaker_shape(node: &El) -> bool {
    node.is("sp")
        && node
            .path(&["nvSpPr", "nvPr", "ph"])
            .is_some_and(|ph| matches!(ph.attr("type"), Some("body") | None))
}

fn decode_notes(imp: &mut Imp, part: &Part, root: &El) -> TextBody {
    let Some(body) = root.path(&["cSld", "spTree"]).and_then(|tree| {
        tree.elements()
            .filter(|node| speaker_shape(node))
            .find_map(|shape| shape.child("txBody"))
    }) else {
        return TextBody::default();
    };
    let mut text = dml::text_body(imp, part, body);
    text.list_style = Default::default();
    if text.is_empty() {
        TextBody::default()
    } else {
        text
    }
}

fn decode_comment(
    node: &El,
    authors: &HashMap<String, (String, String)>,
    units: f64,
    depth: usize,
) -> Comment {
    let (author, initials) = node
        .attr("authorId")
        .and_then(|id| authors.get(id))
        .cloned()
        .unwrap_or_default();
    let text = node
        .child("text")
        .map(El::text)
        .or_else(|| {
            node.child("txBody").map(|body| {
                body.children_named("p")
                    .map(El::text)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        })
        .unwrap_or_default();
    let coord = |axis| {
        node.child("pos")
            .and_then(|point| point.f64(axis))
            .unwrap_or_default()
            / units
    };
    let replies = if depth < 4 {
        node.child("replyLst")
            .into_iter()
            .flat_map(|list| list.children_named("reply"))
            .take(MAX_REPLIES)
            .map(|reply| decode_comment(reply, authors, units, depth + 1))
            .collect()
    } else {
        Vec::new()
    };
    Comment {
        id: node
            .attr("id")
            .or_else(|| node.attr("idx"))
            .unwrap_or_default()
            .into(),
        author,
        initials,
        text,
        date: node
            .attr("dt")
            .or_else(|| node.attr("created"))
            .unwrap_or_default()
            .into(),
        x: coord("x"),
        y: coord("y"),
        resolved: node.attr("status") == Some("resolved"),
        replies,
        shape: None,
    }
}

pub(super) fn notes_xml(export: &mut Exp, out: &mut Out, text: &TextBody) -> Vec<u8> {
    let mut xml = W::new();
    root_open(&mut xml, "p:notes", A::new());
    let mut body = text.clone();
    body.body = Default::default();
    let shapes = [
        notes_placeholder(2, "Slide Image Placeholder 1", PhType::SlideImage, 0, None),
        notes_placeholder(3, "Notes Placeholder 2", PhType::Body, 1, Some(body)),
    ];
    csld(&mut xml, export, out, "", None, &shapes, &[]);
    xml.open0("p:clrMapOvr");
    xml.empty0("a:masterClrMapping");
    xml.close("p:clrMapOvr");
    xml.close("p:notes");
    xml.finish()
}

fn notes_placeholder(id: u32, name: &str, kind: PhType, idx: u32, text: Option<TextBody>) -> Shape {
    Shape {
        id: ShapeId(id),
        name: name.into(),
        ph: Some(Placeholder {
            kind,
            idx,
            ..Default::default()
        }),
        text,
        ..Default::default()
    }
}

#[derive(Default)]
pub(super) struct Writer {
    authors: Vec<Author>,
    ids: HashMap<String, usize>,
}

struct Author {
    name: String,
    initials: String,
    last_index: u32,
}

impl Writer {
    fn add_author(&mut self, comment: &Comment) -> usize {
        let name = if comment.author.is_empty() {
            "Author"
        } else {
            &comment.author
        };
        if let Some(&id) = self.ids.get(name) {
            return id;
        }
        let initials = if comment.initials.is_empty() {
            name.chars()
                .filter(|letter| letter.is_alphabetic())
                .take(2)
                .collect()
        } else {
            comment.initials.clone()
        };
        let id = self.authors.len();
        self.authors.push(Author {
            name: name.into(),
            initials,
            last_index: 0,
        });
        self.ids.insert(name.into(), id);
        id
    }

    pub(super) fn legacy(&mut self, threads: &[Comment]) -> Vec<u8> {
        let mut xml = W::new();
        root_open(&mut xml, "p:cmLst", A::new());
        for comment in threads.iter().take(MAX_THREADS).flat_map(|thread| {
            std::iter::once(thread).chain(thread.replies.iter().take(MAX_REPLIES))
        }) {
            let author_id = self.add_author(comment);
            let author = &mut self.authors[author_id];
            author.last_index += 1;
            xml.open(
                "p:cm",
                A::new()
                    .a("authorId", author_id)
                    .a("dt", timestamp(comment))
                    .a("idx", author.last_index),
            );
            xml.empty("p:pos", position(comment, 8.0));
            xml.elt("p:text", &comment.text);
            xml.close("p:cm");
        }
        xml.close("p:cmLst");
        xml.finish()
    }

    pub(super) fn modern(&mut self, threads: &[Comment], part: &str) -> Vec<u8> {
        let mut xml = W::new();
        xml.open(
            "p188:cmLst",
            A::new()
                .a("xmlns:p188", NS_P188)
                .a("xmlns:a", NS_A)
                .a("xmlns:r", NS_R),
        );
        for (index, thread) in threads.iter().take(MAX_THREADS).enumerate() {
            let key = format!("joydeck-comment:{part}:{index}");
            let attrs = self.comment_attrs(thread, &key);
            xml.open("p188:cm", attrs);
            xml.empty0("p188:unknownAnchor");
            xml.empty("p188:pos", position(thread, 12_700.0));
            if !thread.replies.is_empty() {
                xml.open0("p188:replyLst");
                for (index, reply) in thread.replies.iter().take(MAX_REPLIES).enumerate() {
                    xml.open(
                        "p188:reply",
                        self.comment_attrs(reply, &format!("{key}:{index}")),
                    );
                    write_body(&mut xml, &reply.text);
                    xml.close("p188:reply");
                }
                xml.close("p188:replyLst");
            }
            write_body(&mut xml, &thread.text);
            xml.close("p188:cm");
        }
        xml.close("p188:cmLst");
        xml.finish()
    }

    fn comment_attrs(&mut self, comment: &Comment, key: &str) -> A {
        let index = self.add_author(comment);
        A::new()
            .a("id", crate::tables::guid(key))
            .a("authorId", author_guid(&self.authors[index].name))
            .a("created", timestamp(comment))
            .o("status", comment.resolved.then_some("resolved"))
    }

    pub(super) fn author_parts(self) -> Option<(Vec<u8>, Vec<u8>)> {
        if self.authors.is_empty() {
            return None;
        }
        let mut legacy = W::new();
        root_open(&mut legacy, "p:cmAuthorLst", A::new());
        let mut modern = W::new();
        modern.open("p188:authorLst", A::new().a("xmlns:p188", NS_P188));
        for (index, author) in self.authors.iter().enumerate() {
            legacy.empty(
                "p:cmAuthor",
                A::new()
                    .a("id", index)
                    .a("name", &author.name)
                    .a("initials", &author.initials)
                    .a("lastIdx", author.last_index)
                    .a("clrIdx", index % 8),
            );
            modern.empty(
                "p188:author",
                A::new()
                    .a("id", author_guid(&author.name))
                    .a("name", &author.name)
                    .a("initials", &author.initials)
                    .a("userId", "")
                    .a("providerId", "None"),
            );
        }
        legacy.close("p:cmAuthorLst");
        modern.close("p188:authorLst");
        Some((legacy.finish(), modern.finish()))
    }
}

fn author_guid(name: &str) -> String {
    crate::tables::guid(&format!("joydeck-comment-author:{name}"))
}

fn timestamp(comment: &Comment) -> &str {
    if crate::write::valid_date(&comment.date) {
        &comment.date
    } else {
        "2024-01-01T00:00:00.000"
    }
}

fn position(comment: &Comment, units: f64) -> A {
    let encode = |value: f64| {
        if value.is_finite() {
            (value * units).round().clamp(0.0, 1.0e9) as i64
        } else {
            0
        }
    };
    A::new().a("x", encode(comment.x)).a("y", encode(comment.y))
}

fn write_body(xml: &mut W, text: &str) {
    xml.open0("p188:txBody");
    xml.empty0("a:bodyPr");
    for paragraph in text.split('\n') {
        xml.open0("a:p");
        xml.open0("a:r");
        xml.empty("a:rPr", A::new().a("lang", "en-US"));
        xml.elt("a:t", paragraph);
        xml.close("a:r");
        xml.close("a:p");
    }
    xml.close("p188:txBody");
}
