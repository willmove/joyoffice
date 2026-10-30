use joyoffice_opc::{Budget, Method, ZipArchive};
use joyword_epub::{Metadata, export, utc_timestamp};
use joyword_model::{
    BlockSeq, BreakKind, Document, InlineKind, InlineObject, Measure, Paragraph, RunProps,
};
use std::sync::Arc;

fn metadata() -> Metadata<'static> {
    Metadata {
        title: "原创 & <示例>",
        language: "zh-CN",
        modified: "2026-10-30T10:00:00Z",
    }
}
fn document() -> Document {
    let mut doc = Document::new_blank();
    let mut heading = Paragraph::new(doc.ids().node());
    Arc::make_mut(&mut heading.props).outline_level = Some(0);
    heading
        .push_text(
            "第一章 中文🙂",
            None,
            Arc::new(RunProps {
                bold: Some(false),
                ..Default::default()
            }),
        )
        .unwrap();
    let mut body = Paragraph::new(doc.ids().node());
    body.push_text(
        "A & <B> العربية e\u{301}",
        None,
        Arc::new(RunProps {
            bold: Some(true),
            underline: Some(joyword_model::Underline::None),
            ..Default::default()
        }),
    )
    .unwrap();
    body.push_inline(
        InlineObject {
            id: doc.ids().node(),
            kind: InlineKind::Break(BreakKind::Line),
            source: None,
        },
        None,
        Arc::default(),
    );
    body.push_text("下一行", None, Arc::default()).unwrap();
    let id = doc.body_id();
    doc.story_mut(id).unwrap().blocks = BlockSeq::from_vec(vec![heading.into(), body.into()]);
    doc
}
fn part(bytes: &[u8], name: &str) -> String {
    let archive = ZipArchive::new(bytes, &Budget::default()).unwrap();
    let index = archive
        .entries()
        .iter()
        .position(|e| e.name == name)
        .unwrap();
    String::from_utf8(archive.read(index).unwrap()).unwrap()
}
#[test]
fn archive_is_repeatable_and_mimetype_is_first_stored_without_extra() {
    let doc = document();
    let before = doc.snapshot();
    let first = export(&doc, &metadata()).unwrap();
    let second = export(&doc, &metadata()).unwrap();
    assert_eq!(first.bytes(), second.bytes());
    assert_eq!(first.bytes()[8..10], [0, 0]);
    assert_eq!(first.bytes()[28..30], [0, 0]);
    let archive = ZipArchive::new(first.bytes(), &Budget::default()).unwrap();
    assert_eq!(archive.entries()[0].name, "mimetype");
    assert_eq!(archive.entries()[0].method, Method::Stored);
    assert_eq!(archive.read(0).unwrap(), b"application/epub+zip");
    assert!(doc.semantic_difference(&before).is_none());
}
#[test]
fn navigation_unicode_inline_positions_and_explicit_off_style_survive() {
    let result = export(&document(), &metadata()).unwrap();
    let body = part(result.bytes(), "EPUB/content.xhtml");
    assert!(body.contains("A &amp; &lt;B&gt; العربية e\u{301}"));
    assert!(body.contains("<br/>"));
    assert!(body.contains("下一行"));
    assert!(body.contains("font-weight:normal"));
    assert!(body.contains("font-weight:bold"));
    assert!(!body.contains("text-decoration:underline"));
    let nav = part(result.bytes(), "EPUB/nav.xhtml");
    assert!(nav.contains("content.xhtml#p1"));
    assert!(body.contains("id=\"p1\""));
    assert!(nav.contains("第一章 中文🙂"));
    assert!(nav.contains("epub:type=\"toc\""));
    let ncx = part(result.bytes(), "EPUB/toc.ncx");
    assert!(ncx.contains("content.xhtml#p1"));
    assert!(ncx.contains("第一章 中文🙂"));
    assert!(part(result.bytes(), "EPUB/package.opf").contains("spine toc=\"ncx\""));
    assert_eq!(result.paragraphs, 2);
    assert!(
        result
            .losses
            .iter()
            .any(|loss| loss.location == "document/sections")
    );
}
#[test]
fn embedded_image_is_verified_packaged_and_reported_for_alt_text_review() {
    let mut doc = document();
    let resource = doc.ids().resource();
    doc.resources_mut().insert(
        resource,
        "image/png",
        joyword_model::sample_png_1x1().into(),
        None,
    );
    let mut p = Paragraph::new(doc.ids().node());
    p.push_inline(
        InlineObject {
            id: doc.ids().node(),
            kind: InlineKind::Image {
                resource,
                width: Measure::Twips(100),
                height: Measure::Twips(100),
                anchor: None,
            },
            source: None,
        },
        None,
        Arc::default(),
    );
    let mut blocks: Vec<_> = doc.body().blocks.iter().cloned().collect();
    blocks.push(p.into());
    let id = doc.body_id();
    doc.story_mut(id).unwrap().blocks = BlockSeq::from_vec(blocks);
    let result = export(&doc, &metadata()).unwrap();
    assert_eq!(result.images, 1);
    assert!(part(result.bytes(), "EPUB/content.xhtml").contains("images/image-1.png"));
    assert!(part(result.bytes(), "EPUB/package.opf").contains("media-type=\"image/png\""));
    assert!(
        result
            .losses
            .iter()
            .any(|l| l.detail.contains("alternative text"))
    );
}
#[test]
fn hidden_text_revisions_and_foreign_stories_are_not_silently_flattened() {
    let mut doc = Document::new_blank();
    let mut p = Paragraph::new(doc.ids().node());
    p.push_text(
        "secret",
        None,
        Arc::new(RunProps {
            hidden: Some(true),
            ..Default::default()
        }),
    )
    .unwrap();
    let id = doc.body_id();
    doc.story_mut(id).unwrap().blocks = BlockSeq::from_vec(vec![p.into()]);
    assert!(
        export(&doc, &metadata())
            .unwrap_err()
            .detail
            .contains("hidden")
    );
    let mut doc = document();
    let p = Paragraph::new(doc.ids().node());
    let mut p = p;
    p.push_text("footnote", None, Arc::default()).unwrap();
    doc.insert_story(joyword_model::StoryKind::Footnote, vec![p.into()]);
    assert!(
        export(&doc, &metadata())
            .unwrap_err()
            .detail
            .contains("non-body")
    );
    let bytes = include_bytes!("../../../tests/fixtures/feature/DOCX-FEAT-T300-REVIEW.docx");
    let imported = joyword_docx::import(bytes, &Budget::default()).unwrap();
    assert!(export(&imported.document, &metadata()).is_err());
}
#[test]
fn metadata_controls_invalid_dates_and_language_are_refused() {
    for date in [
        "2026-02-30T10:00:00Z",
        "2026-10-30T99:00:00Z",
        "2026-10-30T10:00:00+08:00",
        "2026-xx-30T10:00:00Z",
    ] {
        assert!(
            export(
                &document(),
                &Metadata {
                    modified: date,
                    ..metadata()
                }
            )
            .is_err()
        );
    }
    for language in ["", "1", "en--US", "zh\" onload=\"x"] {
        assert!(
            export(
                &document(),
                &Metadata {
                    language,
                    ..metadata()
                }
            )
            .is_err()
        );
    }
    assert!(
        export(
            &document(),
            &Metadata {
                title: "bad\0title",
                ..metadata()
            }
        )
        .is_err()
    );
}
#[test]
fn calendar_handles_epoch_leap_centuries_and_refuses_unbounded_dates() {
    assert_eq!(utc_timestamp(0).unwrap(), "1970-01-01T00:00:00Z");
    assert_eq!(utc_timestamp(951782400).unwrap(), "2000-02-29T00:00:00Z");
    assert_eq!(utc_timestamp(4107542400).unwrap(), "2100-03-01T00:00:00Z");
    assert!(utc_timestamp(u64::MAX).is_err());
}

#[test]
fn input_budgets_fail_with_locations_before_packaging() {
    let mut doc = Document::new_blank();
    let mut p = Paragraph::new(doc.ids().node());
    p.push_text(&"x".repeat((4 << 20) + 1), None, Arc::default())
        .unwrap();
    let id = doc.body_id();
    doc.story_mut(id).unwrap().blocks = BlockSeq::from_vec(vec![p.into()]);
    let failure = export(&doc, &metadata()).unwrap_err();
    assert_eq!(failure.location, "body/block[1]");
    assert_eq!(failure.detail, "text budget exceeded");
    let blocks = (0..20001)
        .map(|_| Paragraph::new(doc.ids().node()).into())
        .collect();
    doc.story_mut(id).unwrap().blocks = BlockSeq::from_vec(blocks);
    assert_eq!(
        export(&doc, &metadata()).unwrap_err().detail,
        "block budget exceeded"
    );
}

#[test]
fn corrupt_images_and_media_type_spoofing_are_refused() {
    for (media, bytes, expected) in [
        (
            "image/png",
            b"not a PNG".to_vec(),
            "invalid or over-budget image",
        ),
        (
            "image/jpeg",
            joyword_model::sample_png_1x1().to_vec(),
            "image media type disagrees with bytes",
        ),
    ] {
        let mut doc = Document::new_blank();
        let resource = doc.ids().resource();
        doc.resources_mut()
            .insert(resource, media, bytes.into(), None);
        let mut p = Paragraph::new(doc.ids().node());
        p.push_inline(
            InlineObject {
                id: doc.ids().node(),
                kind: InlineKind::Image {
                    resource,
                    width: Measure::Twips(100),
                    height: Measure::Twips(100),
                    anchor: None,
                },
                source: None,
            },
            None,
            Arc::default(),
        );
        let id = doc.body_id();
        doc.story_mut(id).unwrap().blocks = BlockSeq::from_vec(vec![p.into()]);
        assert_eq!(export(&doc, &metadata()).unwrap_err().detail, expected);
    }
}

#[test]
fn external_links_are_escaped_and_unsafe_or_unmapped_targets_refused() {
    for (external, anchor, accepted) in [
        (Some("https://example.org/?a=1&b=2"), None, true),
        (Some("javascript:alert(1)"), None, false),
        (Some("file:///C:/secret"), None, false),
        (None, Some("unknown-bookmark"), false),
    ] {
        let mut doc = Document::new_blank();
        let mut p = Paragraph::new(doc.ids().node());
        p.push_text("link", None, Arc::default()).unwrap();
        p.set_last_run_link(Some(Arc::new(joyword_model::Hyperlink {
            relationship: None,
            external: external.map(Arc::from),
            anchor: anchor.map(Arc::from),
        })));
        let id = doc.body_id();
        doc.story_mut(id).unwrap().blocks = BlockSeq::from_vec(vec![p.into()]);
        let result = export(&doc, &metadata());
        if accepted {
            assert!(
                part(result.unwrap().bytes(), "EPUB/content.xhtml")
                    .contains("https://example.org/?a=1&amp;b=2")
            );
        } else {
            assert!(result.is_err());
        }
    }
}
