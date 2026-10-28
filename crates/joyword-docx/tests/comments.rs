//! T302: comment import and export. Not a Word or WPS exchange test.

use std::sync::Arc;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{DiagnosticKind, import, plan_export};
use joyword_model::{
    Anchor, AnnotationKind, Bias, Block, Comment, Document, Paragraph, Position, ScalarOffset,
};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
const W15: &str = "http://schemas.microsoft.com/office/word/2012/wordml";
const W16CID: &str = "http://schemas.microsoft.com/office/word/2016/wordml/cid";
const W16CEX: &str = "http://schemas.microsoft.com/office/word/2018/wordml/cex";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";

fn archive(parts: &[(&str, &str)]) -> Vec<u8> {
    let stored: Vec<(&str, &[u8], Method)> = parts
        .iter()
        .map(|(name, data)| (*name, data.as_bytes(), Method::Deflated))
        .collect();
    write_archive(&stored).unwrap()
}

fn types(overrides: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="{CT}"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>{overrides}</Types>"#
    )
}

fn package_rels() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#
}

fn document(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>{body}</w:body></w:document>"#
    )
}

fn part_bytes(package: &[u8], name: &str) -> Vec<u8> {
    Package::read(package, &Budget::default())
        .unwrap()
        .part(name)
        .unwrap()
        .data
        .to_vec()
}

fn comment_of(document: &Document, source_id: u32) -> (Comment, Anchor) {
    document
        .annotations()
        .find_map(|annotation| {
            let AnnotationKind::Comment(comment) = &annotation.kind else {
                return None;
            };
            (comment.source_id == source_id).then(|| (comment.clone(), annotation.anchor.clone()))
        })
        .unwrap_or_else(|| panic!("missing comment {source_id}"))
}

fn has_kind(result: &joyword_docx::ImportResult, kind: &DiagnosticKind) -> bool {
    result.diagnostics.iter().any(|item| &item.kind == kind)
}

fn round_trip(bytes: &[u8], current: &Document) -> Vec<u8> {
    let source = import(bytes, &Budget::default()).unwrap();
    let plan = plan_export(&source.backing, &current.snapshot(), &Budget::default())
        .unwrap_or_else(|error| panic!("export failed: {error}"));
    let again = import(plan.bytes(), &Budget::default()).unwrap();
    let difference = current.semantic_difference(&again.document);
    assert!(
        difference.is_none(),
        "{difference:?}\nexpected:\n{}\nactual:\n{}",
        current.canonical_dump(joyword_model::DumpOptions { node_ids: false }),
        again
            .document
            .canonical_dump(joyword_model::DumpOptions { node_ids: false })
    );
    plan.bytes().to_vec()
}

fn body_paragraph(document: &mut Document, index: usize) -> &mut Paragraph {
    let body = document.body_id();
    let block = document
        .story_mut(body)
        .unwrap()
        .blocks
        .get_mut(index)
        .unwrap();
    match block {
        Block::Paragraph(paragraph) => Arc::make_mut(paragraph),
        _ => panic!("paragraph"),
    }
}

#[test]
fn range_comment_imports_author_date_and_cross_paragraph_ends() {
    let body = concat!(
        r#"<w:p><w:commentRangeStart w:id="0"/><w:r><w:t>hello</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>world</w:t></w:r><w:commentRangeEnd w:id="0"/>"#,
        r#"<w:r><w:commentReference w:id="0"/></w:r></w:p>"#,
    );
    let comments = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:comments xmlns:w="{W}" xmlns:w14="{W14}"><w:comment w:id="0" w:author="Ada" w:date="2020-01-02T03:04:05Z" w:initials="AL"><w:p w14:paraId="00000001"><w:r><w:t>请确认</w:t></w:r></w:p></w:comment></w:comments>"#
    );
    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#
    );
    let bytes = archive(&[
        (
            "[Content_Types].xml",
            &types(
                r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/>"#,
            ),
        ),
        ("_rels/.rels", package_rels()),
        ("word/document.xml", &document(body)),
        ("word/_rels/document.xml.rels", &rels),
        ("word/comments.xml", &comments),
    ]);
    let imported = import(&bytes, &Budget::default()).unwrap();
    assert!(!imported.editability.is_read_only());
    let (comment, anchor) = comment_of(&imported.document, 0);
    assert_eq!(comment.author.as_deref(), Some("Ada"));
    assert_eq!(comment.date.as_deref(), Some("2020-01-02T03:04:05Z"));
    assert_eq!(comment.initials.as_deref(), Some("AL"));
    assert_eq!(comment.para_id.as_deref(), Some("00000001"));
    assert!(!comment.resolved);
    let Anchor::Range { start, end } = anchor else {
        panic!("range");
    };
    assert_eq!(start.offset.0, 0);
    assert_eq!(start.bias, Bias::Before);
    assert_eq!(end.offset.0, 5);
    assert_eq!(end.bias, Bias::After);
    assert_ne!(start.paragraph, end.paragraph);
    let story = imported.document.story(comment.body).unwrap();
    let text = story
        .blocks
        .iter()
        .next()
        .unwrap()
        .as_paragraph()
        .unwrap()
        .plain_text(|_| String::new());
    assert_eq!(text, "请确认");
}

#[test]
fn reply_thread_resolve_and_reopen_round_trip_keeps_unknown_xml() {
    let bytes = thread_package();
    let imported = import(&bytes, &Budget::default()).unwrap();
    assert!(!imported.editability.is_read_only());
    assert!(has_kind(
        &imported,
        &DiagnosticKind::CommentExtensionUnmatched {
            para_id: "99999999".into(),
        }
    ));
    let (root, root_anchor) = comment_of(&imported.document, 0);
    let (reply, reply_anchor) = comment_of(&imported.document, 1);
    assert_eq!(root.author.as_deref(), Some("Ada"));
    assert!(!root.resolved);
    assert_eq!(reply.parent_para_id.as_deref(), Some("00000001"));
    assert!(reply.resolved);
    assert_eq!(reply_anchor, root_anchor);
    assert!(matches!(root_anchor, Anchor::Range { .. }));
    assert_eq!(
        root.durable_id.as_deref(),
        Some("{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}")
    );
    assert_eq!(root.date_utc.as_deref(), Some("2020-01-02T03:04:05Z"));
    assert!(root.extra_markup.as_deref().unwrap().contains("keep-me"));
    assert!(
        root.extra_attributes
            .iter()
            .any(|attribute| attribute.local.as_ref() == "extra")
    );

    let mut resolved = imported.document.clone();
    let root_id = resolved
        .annotations()
        .find(|annotation| {
            matches!(&annotation.kind, AnnotationKind::Comment(comment) if comment.source_id == 0)
        })
        .unwrap()
        .id;
    match &mut resolved.annotation_mut(root_id).unwrap().kind {
        AnnotationKind::Comment(comment) => comment.resolved = true,
        _ => panic!("root"),
    }
    let written = round_trip(&bytes, &resolved);
    assert_eq!(
        part_bytes(&bytes, "/word/comments.xml"),
        part_bytes(&written, "/word/comments.xml")
    );
    let extended = String::from_utf8(part_bytes(&written, "/word/commentsExtended.xml")).unwrap();
    assert!(extended.contains(r#"w15:paraId="00000001""#));
    assert!(extended.contains(r#"w15:done="1""#));
    assert!(extended.contains(r#"w15:extra="kept""#));
    assert!(extended.contains("99999999"));
    let again = import(&written, &Budget::default()).unwrap();
    assert!(comment_of(&again.document, 0).0.resolved);
    assert!(matches!(
        comment_of(&again.document, 0).1,
        Anchor::Range { .. }
    ));

    let mut reopened = imported.document.clone();
    let reply_id = reopened
        .annotations()
        .find(|annotation| {
            matches!(&annotation.kind, AnnotationKind::Comment(comment) if comment.source_id == 1)
        })
        .unwrap()
        .id;
    match &mut reopened.annotation_mut(reply_id).unwrap().kind {
        AnnotationKind::Comment(comment) => comment.resolved = false,
        _ => panic!("reply"),
    }
    let written = round_trip(&bytes, &reopened);
    assert_eq!(
        part_bytes(&bytes, "/word/comments.xml"),
        part_bytes(&written, "/word/comments.xml")
    );
    let extended = String::from_utf8(part_bytes(&written, "/word/commentsExtended.xml")).unwrap();
    assert!(extended.contains(r#"w15:paraId="00000002""#));
    assert!(extended.contains(r#"w15:done="0""#));
    assert!(
        extended.contains("keep-me")
            || part_bytes(&written, "/word/comments.xml")
                .windows(7)
                .any(|w| w == b"keep-me")
    );
    assert!(
        !comment_of(&import(&written, &Budget::default()).unwrap().document, 1)
            .0
            .resolved
    );
}

#[test]
fn deleting_the_anchor_exports_detached_without_resolving() {
    let bytes = thread_package();
    let imported = import(&bytes, &Budget::default()).unwrap();
    let mut current = imported.document.clone();
    let len = current
        .body()
        .blocks
        .iter()
        .next()
        .unwrap()
        .as_paragraph()
        .unwrap()
        .len();
    body_paragraph(&mut current, 0)
        .remove(ScalarOffset(0)..ScalarOffset(len))
        .unwrap();
    let id = current
        .annotations()
        .find(|annotation| {
            matches!(&annotation.kind, AnnotationKind::Comment(comment) if comment.source_id == 0)
        })
        .unwrap()
        .id;
    current.annotation_mut(id).unwrap().anchor = Anchor::Detached;
    let reply = current
        .annotations()
        .find(|annotation| {
            matches!(&annotation.kind, AnnotationKind::Comment(comment) if comment.source_id == 1)
        })
        .unwrap()
        .id;
    current.annotation_mut(reply).unwrap().anchor = Anchor::Detached;
    let written = round_trip(&bytes, &current);
    assert_eq!(
        part_bytes(&bytes, "/word/comments.xml"),
        part_bytes(&written, "/word/comments.xml")
    );
    let (root, anchor) = comment_of(&import(&written, &Budget::default()).unwrap().document, 0);
    assert_eq!(anchor, Anchor::Detached);
    assert!(!root.resolved);
    assert!(root.extra_markup.as_deref().unwrap().contains("keep-me"));
}

#[test]
fn text_inserted_inside_a_range_stays_inside_after_export() {
    let bytes = thread_package();
    let imported = import(&bytes, &Budget::default()).unwrap();
    let mut current = imported.document.clone();
    body_paragraph(&mut current, 0)
        .insert_text(ScalarOffset(1), "X")
        .unwrap();
    let ids: Vec<_> = current
        .annotations()
        .filter(|annotation| matches!(annotation.anchor, Anchor::Range { .. }))
        .map(|annotation| annotation.id)
        .collect();
    for id in ids {
        let Anchor::Range { mut end, .. } = current.annotation(id).unwrap().anchor else {
            continue;
        };
        end.offset.0 += 1;
        if let Anchor::Range { end: slot, .. } = &mut current.annotation_mut(id).unwrap().anchor {
            *slot = end;
        }
    }
    let written = round_trip(&bytes, &current);
    let again = import(&written, &Budget::default()).unwrap();
    let (_, anchor) = comment_of(&again.document, 0);
    let Anchor::Range { start, end } = anchor else {
        panic!("range");
    };
    assert_eq!((start.offset.0, end.offset.0), (0, 7));
    let text = again
        .document
        .body()
        .blocks
        .iter()
        .next()
        .unwrap()
        .as_paragraph()
        .unwrap()
        .plain_text(|_| String::new());
    assert_eq!(text, "aXbcdef");
}

#[test]
fn orphan_reply_and_missing_author_stay_visible() {
    let body = r#"<w:p><w:commentRangeStart w:id="3"/><w:r><w:t>anon</w:t></w:r><w:commentRangeEnd w:id="3"/><w:r><w:commentReference w:id="3"/></w:r></w:p>"#;
    let comments = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:comments xmlns:w="{W}" xmlns:w14="{W14}"><w:comment w:id="3"><w:p w14:paraId="00000004"><w:r><w:t>anon</w:t></w:r></w:p></w:comment><w:comment w:id="4" w:author="Cye"><w:p w14:paraId="00000005"><w:r><w:t>lost</w:t></w:r></w:p></w:comment></w:comments>"#
    );
    let extended = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w15:commentsEx xmlns:w15="{W15}"><w15:commentEx w15:paraId="00000005" w15:paraIdParent="DEADBEEF" w15:done="0"/></w15:commentsEx>"#
    );
    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/><Relationship Id="rId2" Type="http://schemas.microsoft.com/office/2011/relationships/commentsExtended" Target="commentsExtended.xml"/></Relationships>"#
    );
    let bytes = archive(&[
        (
            "[Content_Types].xml",
            &types(
                r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/><Override PartName="/word/commentsExtended.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.commentsExtended+xml"/>"#,
            ),
        ),
        ("_rels/.rels", package_rels()),
        ("word/document.xml", &document(body)),
        ("word/_rels/document.xml.rels", &rels),
        ("word/comments.xml", &comments),
        ("word/commentsExtended.xml", &extended),
    ]);
    let imported = import(&bytes, &Budget::default()).unwrap();
    assert!(!imported.editability.is_read_only());
    assert!(has_kind(
        &imported,
        &DiagnosticKind::CommentAuthorMissing { id: 3 }
    ));
    assert!(has_kind(
        &imported,
        &DiagnosticKind::OrphanReply {
            para_id: "DEADBEEF".into(),
        }
    ));
    let (anon, anon_anchor) = comment_of(&imported.document, 3);
    assert!(anon.author.is_none());
    assert!(matches!(anon_anchor, Anchor::Range { .. }));
    let (orphan, orphan_anchor) = comment_of(&imported.document, 4);
    assert_eq!(orphan.parent_para_id.as_deref(), Some("DEADBEEF"));
    assert_eq!(orphan_anchor, Anchor::Detached);
    assert!(!imported.document.validate().has_errors());
}

#[test]
fn opaque_table_comment_anchor_is_detached() {
    let body = concat!(
        r#"<w:tbl><w:tr><w:tc><w:p>"#,
        r#"<w:commentRangeStart w:id="0"/><w:r><w:t>cell</w:t></w:r><w:commentRangeEnd w:id="0"/>"#,
        r#"</w:p></w:tc></w:tr></w:tbl>"#,
        r#"<w:p><w:r><w:t>after</w:t></w:r></w:p>"#,
    );
    let comments = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:comments xmlns:w="{W}" xmlns:w14="{W14}"><w:comment w:id="0" w:author="Ada"><w:p w14:paraId="00000001"><w:r><w:t>cell note</w:t></w:r></w:p></w:comment></w:comments>"#
    );
    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#
    );
    let bytes = archive(&[
        (
            "[Content_Types].xml",
            &types(
                r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/>"#,
            ),
        ),
        ("_rels/.rels", package_rels()),
        ("word/document.xml", &document(body)),
        ("word/_rels/document.xml.rels", &rels),
        ("word/comments.xml", &comments),
    ]);
    let imported = import(&bytes, &Budget::default()).unwrap();
    assert!(!imported.editability.is_read_only());
    assert!(has_kind(
        &imported,
        &DiagnosticKind::CommentAnchorUnmapped { id: 0 }
    ));
    let (comment, anchor) = comment_of(&imported.document, 0);
    assert_eq!(comment.author.as_deref(), Some("Ada"));
    assert_eq!(anchor, Anchor::Detached);
}

#[test]
fn a_new_comment_and_reply_are_written_back() {
    let body = r#"<w:p><w:r><w:t>Hello</w:t></w:r></w:p>"#;
    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"></Relationships>"#
    );
    let bytes = archive(&[
        (
            "[Content_Types].xml",
            &types(
                r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>"#,
            ),
        ),
        ("_rels/.rels", package_rels()),
        ("word/document.xml", &document(body)),
        ("word/_rels/document.xml.rels", &rels),
    ]);
    let imported = import(&bytes, &Budget::default()).unwrap();
    let mut session = joyword_engine::Session::new(imported.document.clone());
    let paragraph = session.document().body().blocks.iter().next().unwrap().id();
    let story = session.document().body_id();
    session
        .apply(joyword_engine::Transaction::new(
            session.version(),
            vec![joyword_engine::Command::AddComment {
                start: Position {
                    story,
                    paragraph,
                    offset: ScalarOffset(1),
                    bias: Bias::Before,
                },
                end: Position {
                    story,
                    paragraph,
                    offset: ScalarOffset(4),
                    bias: Bias::After,
                },
                author: "Ada".into(),
                date: Some("2024-01-02T03:04:05Z".into()),
                initials: Some("AL".into()),
                text: "Note".into(),
            }],
        ))
        .unwrap();
    let root_id = session.document().annotations().next().unwrap().id;
    session
        .apply(joyword_engine::Transaction::new(
            session.version(),
            vec![joyword_engine::Command::ReplyComment {
                parent: root_id,
                author: "Bea".into(),
                date: Some("2024-01-03T04:05:06Z".into()),
                initials: None,
                text: "Reply".into(),
            }],
        ))
        .unwrap();
    let current = session.document().clone();
    let source = import(&bytes, &Budget::default()).unwrap();
    let plan = plan_export(&source.backing, &current.snapshot(), &Budget::default())
        .unwrap_or_else(|error| panic!("export failed: {error}"));
    let written = plan.bytes().to_vec();
    let package = Package::read(&written, &Budget::default()).unwrap();
    assert!(package.part("/word/comments.xml").is_some());
    assert!(package.part("/word/commentsExtended.xml").is_some());
    // New comments gain durable ids and UTC stamps this session allocated.
    let ids = String::from_utf8(
        package
            .part("/word/commentsIds.xml")
            .expect("ids part")
            .data
            .to_vec(),
    )
    .unwrap();
    assert_eq!(ids.matches("w16cid:commentId").count(), 2, "{ids}");
    let extensible = String::from_utf8(
        package
            .part("/word/commentsExtensible.xml")
            .expect("extensible part")
            .data
            .to_vec(),
    )
    .unwrap();
    assert!(
        extensible.contains(r#"w16cex:dateUtc="2024-01-02T03:04:05Z""#),
        "{extensible}"
    );
    let again = import(&written, &Budget::default()).unwrap();
    let (root, anchor) = comment_of(&again.document, 0);
    let (reply, reply_anchor) = comment_of(&again.document, 1);
    assert_eq!(root.author.as_deref(), Some("Ada"));
    assert!(root.durable_id.is_some());
    assert!(reply.durable_id.is_some());
    assert_eq!(reply.parent_para_id, root.para_id);
    assert_eq!(reply_anchor, anchor);
    let Anchor::Range { start, end } = anchor else {
        panic!("range");
    };
    assert_eq!((start.offset.0, end.offset.0), (1, 4));
    let text = again
        .document
        .body()
        .blocks
        .iter()
        .next()
        .unwrap()
        .as_paragraph()
        .unwrap()
        .plain_text(|_| String::new());
    assert_eq!(text, "Hello");
    let main = String::from_utf8(part_bytes(&written, "/word/document.xml")).unwrap();
    assert_eq!(main.matches("commentRangeStart").count(), 1);
    assert!(main.contains("commentReference"));
    // A mid-run range splits `w:t` in the package. The next open sees those
    // runs; a second save of that open must not move the comment again.
    round_trip(&written, &again.document);
}

#[test]
fn new_comments_append_durable_ids_and_extensible_dates_without_touching_entries() {
    let bytes = thread_package();
    let imported = import(&bytes, &Budget::default()).unwrap();
    let (root, root_anchor) = comment_of(&imported.document, 0);
    let root_id = imported
        .document
        .annotations()
        .find(|annotation| {
            matches!(&annotation.kind, AnnotationKind::Comment(comment) if comment.source_id == 0)
        })
        .unwrap()
        .id;
    let mut session = joyword_engine::Session::new(imported.document.clone());
    session
        .apply(joyword_engine::Transaction::new(
            session.version(),
            vec![joyword_engine::Command::ReplyComment {
                parent: root_id,
                author: "Cleo".into(),
                date: Some("2024-05-06T07:08:09Z".into()),
                initials: None,
                text: "Late".into(),
            }],
        ))
        .unwrap();
    let current = session.document().clone();
    let para_id = current
        .annotations()
        .find_map(|annotation| match &annotation.kind {
            AnnotationKind::Comment(comment)
                if comment.parent_para_id.is_some() && comment.source_id != 1 =>
            {
                comment.para_id.clone()
            }
            _ => None,
        })
        .expect("fresh reply para id");
    let written = round_trip(&bytes, &current);

    // commentsIds: exactly one appended entry; removing it restores the
    // original bytes verbatim.
    let original_ids = part_bytes(&bytes, "/word/commentsIds.xml");
    let updated_ids = String::from_utf8(part_bytes(&written, "/word/commentsIds.xml")).unwrap();
    let at = updated_ids
        .rfind("<w16cid:commentId")
        .expect("appended entry");
    let end = "</w16cid:commentsIds>";
    let rebuilt = format!(
        "{}{}",
        &updated_ids[..at],
        &updated_ids[updated_ids.len() - end.len()..]
    );
    assert_eq!(rebuilt.as_bytes(), original_ids);
    let appended = &updated_ids[at..updated_ids.len() - end.len()];
    assert!(
        appended.contains(&format!(r#"w16cid:paraId="{para_id}""#)),
        "{appended}"
    );
    let durable: String = appended
        .split(r#"w16cid:durableId=""#)
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();
    assert!(
        durable.bytes().all(|byte| byte.is_ascii_digit()) && !durable.is_empty(),
        "durable id is a decimal: {durable}"
    );

    // commentsExtensible: the same durable id carries the strict date; the
    // existing GUID entry keeps its bytes.
    let original_ex = part_bytes(&bytes, "/word/commentsExtensible.xml");
    let updated_ex =
        String::from_utf8(part_bytes(&written, "/word/commentsExtensible.xml")).unwrap();
    assert!(updated_ex.contains("{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}"));
    assert!(updated_ex.contains(r#"w16cex:dateUtc="2024-05-06T07:08:09Z""#));
    assert!(updated_ex.contains(&format!(r#"w16cex:durableId="{durable}""#)));
    let at = updated_ex
        .rfind("<w16cex:commentExtensible")
        .expect("appended entry");
    let end = "</w16cex:commentsExtensible>";
    let rebuilt = format!(
        "{}{}",
        &updated_ex[..at],
        &updated_ex[updated_ex.len() - end.len()..]
    );
    assert_eq!(rebuilt.as_bytes(), original_ex);

    // Reimport reads the durable id and the UTC date back onto the comment.
    let again = import(&written, &Budget::default()).unwrap();
    let found = again
        .document
        .annotations()
        .find_map(|annotation| match &annotation.kind {
            AnnotationKind::Comment(comment) if comment.para_id.as_deref() == Some(&para_id) => {
                Some(comment)
            }
            _ => None,
        })
        .expect("fresh comment survives reimport");
    assert_eq!(found.durable_id.as_deref(), Some(durable.as_str()));
    assert_eq!(found.date_utc.as_deref(), Some("2024-05-06T07:08:09Z"));
    let _ = (root, root_anchor);
}

#[test]
fn a_new_comment_without_a_strict_utc_date_leaves_comments_extensible_untouched() {
    let bytes = thread_package();
    let imported = import(&bytes, &Budget::default()).unwrap();
    let root_id = imported
        .document
        .annotations()
        .find(|annotation| {
            matches!(&annotation.kind, AnnotationKind::Comment(comment) if comment.source_id == 0)
        })
        .unwrap()
        .id;
    let mut session = joyword_engine::Session::new(imported.document.clone());
    session
        .apply(joyword_engine::Transaction::new(
            session.version(),
            vec![joyword_engine::Command::ReplyComment {
                parent: root_id,
                author: "Dee".into(),
                date: Some("last Tuesday".into()),
                initials: None,
                text: "NoDate".into(),
            }],
        ))
        .unwrap();
    let written = round_trip(&bytes, session.document());

    // The ids part still gains its entry; the extensible part keeps its
    // original bytes rather than inventing a timestamp for a loose date.
    let updated_ids = String::from_utf8(part_bytes(&written, "/word/commentsIds.xml")).unwrap();
    assert_eq!(updated_ids.matches("w16cid:commentId").count(), 2);
    assert_eq!(
        part_bytes(&written, "/word/commentsExtensible.xml"),
        part_bytes(&bytes, "/word/commentsExtensible.xml")
    );
}

#[test]
fn a_fresh_package_without_extension_parts_gains_ids_and_extensible() {
    let body = r#"<w:p><w:r><w:t>Hello</w:t></w:r></w:p>"#;
    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"></Relationships>"#
    );
    let bytes = archive(&[
        (
            "[Content_Types].xml",
            &types(
                r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>"#,
            ),
        ),
        ("_rels/.rels", package_rels()),
        ("word/document.xml", &document(body)),
        ("word/_rels/document.xml.rels", &rels),
    ]);
    let imported = import(&bytes, &Budget::default()).unwrap();
    let mut session = joyword_engine::Session::new(imported.document.clone());
    let paragraph = session.document().body().blocks.iter().next().unwrap().id();
    let story_of = session.document().body_id();
    session
        .apply(joyword_engine::Transaction::new(
            session.version(),
            vec![joyword_engine::Command::AddComment {
                start: Position {
                    story: story_of,
                    paragraph,
                    offset: ScalarOffset(0),
                    bias: Bias::Before,
                },
                end: Position {
                    story: story_of,
                    paragraph,
                    offset: ScalarOffset(5),
                    bias: Bias::After,
                },
                author: "Eve".into(),
                date: Some("2024-01-02T03:04:05Z".into()),
                initials: None,
                text: "Note".into(),
            }],
        ))
        .unwrap();
    let written = round_trip(&bytes, session.document());
    let package = Package::read(&written, &Budget::default()).unwrap();
    let ids = package.part("/word/commentsIds.xml").expect("ids part");
    let ids_text = String::from_utf8(ids.data.to_vec()).unwrap();
    assert!(ids_text.contains(r#"w16cid:paraId="#), "{ids_text}");
    let extensible = package
        .part("/word/commentsExtensible.xml")
        .expect("extensible part");
    let extensible_text = String::from_utf8(extensible.data.to_vec()).unwrap();
    assert!(extensible_text.contains(r#"w16cex:dateUtc="2024-01-02T03:04:05Z""#));
    // The durable id written to ids is the one the extensible entry keys on.
    let durable: String = ids_text
        .split(r#"w16cid:durableId=""#)
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();
    assert!(extensible_text.contains(&format!(r#"w16cex:durableId="{durable}""#)));
    // The new parts reimport with the durable id on the comment.
    let again = import(&written, &Budget::default()).unwrap();
    let (comment, _) = comment_of(&again.document, 0);
    assert_eq!(comment.durable_id.as_deref(), Some(durable.as_str()));
}

fn thread_package() -> Vec<u8> {
    let body = concat!(
        r#"<w:p><w:commentRangeStart w:id="0"/><w:r><w:t>abcdef</w:t></w:r>"#,
        r#"<w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r></w:p>"#,
    );
    let comments = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:comments xmlns:w="{W}" xmlns:w14="{W14}" xmlns:jw="urn:joyword:test"><w:comment w:id="0" w:author="Ada" w:date="2020-01-02T03:04:05Z" w:initials="AL" w:extra="kept"><w:p w14:paraId="00000001"><w:r><w:t>Note</w:t></w:r></w:p><jw:note>keep-me</jw:note></w:comment><w:comment w:id="1" w:author="Bea" w:date="2020-02-03T04:05:06Z"><w:p w14:paraId="00000002"><w:r><w:t>Reply</w:t></w:r></w:p></w:comment></w:comments>"#
    );
    let extended = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w15:commentsEx xmlns:w15="{W15}"><w15:commentEx w15:paraId="00000001" w15:done="0" w15:extra="kept"/><w15:commentEx w15:paraId="00000002" w15:paraIdParent="00000001" w15:done="1"/><w15:commentEx w15:paraId="99999999" w15:done="0"/></w15:commentsEx>"#
    );
    let ids = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w16cid:commentsIds xmlns:w16cid="{W16CID}"><w16cid:commentId w16cid:paraId="00000001" w16cid:durableId="{{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}}"/></w16cid:commentsIds>"#
    );
    let extensible = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w16cex:commentsExtensible xmlns:w16cex="{W16CEX}"><w16cex:commentExtensible w16cex:durableId="{{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}}" w16cex:dateUtc="2020-01-02T03:04:05Z"/></w16cex:commentsExtensible>"#
    );
    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/><Relationship Id="rId2" Type="http://schemas.microsoft.com/office/2011/relationships/commentsExtended" Target="commentsExtended.xml"/><Relationship Id="rId3" Type="http://schemas.microsoft.com/office/2016/09/relationships/commentsIds" Target="commentsIds.xml"/><Relationship Id="rId4" Type="http://schemas.microsoft.com/office/2018/08/relationships/commentsExtensible" Target="commentsExtensible.xml"/></Relationships>"#
    );
    let overrides = concat!(
        r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>"#,
        r#"<Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/>"#,
        r#"<Override PartName="/word/commentsExtended.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.commentsExtended+xml"/>"#,
        r#"<Override PartName="/word/commentsIds.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.commentsIds+xml"/>"#,
        r#"<Override PartName="/word/commentsExtensible.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.commentsExtensible+xml"/>"#,
    );
    archive(&[
        ("[Content_Types].xml", &types(overrides)),
        ("_rels/.rels", package_rels()),
        ("word/document.xml", &document(body)),
        ("word/_rels/document.xml.rels", &rels),
        ("word/comments.xml", &comments),
        ("word/commentsExtended.xml", &extended),
        ("word/commentsIds.xml", &ids),
        ("word/commentsExtensible.xml", &extensible),
    ])
}
