//! Find-bar checks and headless rehearsals of the manual acceptance scenarios.
//!
//! The no-panic gate scans `main.rs` in full, including `#[cfg(test)]`.
//! A path ending in `tests.rs` is outside that scan.
//!
//! Rehearsals drive `Office` / `TextInput` directly. Each one is an automated
//! rehearsal and does not replace 人工验收.

use super::input_ui::edit_request_from_args;
use super::{
    ArgValue, CommandId, FIND_NEXT, FIND_PREVIOUS, FIND_REPLACE, FIND_REPLACE_ALL, FindBarFlags,
    supplement_find_flags,
};

// The panic gate scans export.rs including inline tests; keep the headless
// application-entry regressions in the existing test module instead.
mod scene_export {
    use std::sync::Arc;

    use joyword_foundation::Emu;
    use joyword_layout::{
        FaceSet, LayoutCancellation, LayoutInvalidation, LayoutLimits, LayoutReuse, LayoutSnapshot,
        ParagraphLayoutCache, ViewOptions, layout_incremental_with_view, layout_with_view,
        to_points,
    };
    use joyword_model::{
        AbstractNum, Anchor, AnnotationKind, Bias, BlockSeq, ChapterSeparator, Document, Hyperlink,
        InlineKind, InlineObject, Measure, NumInstance, NumberFormat, NumberSuffix, NumberingLevel,
        NumberingRef, PageField, PageFieldKind, PageSetup, Paragraph, Position, ScalarOffset,
        Style, StyleId, StyleKind,
    };
    use joyword_pdf::{
        PdfDestinationReport, PdfExport, inspect_navigation, inspect_pdf, links_from_document,
        outline_from_headings, render_export,
    };

    use super::{rehearsal_profile, test_font};
    use crate::export::scene_pdf;

    fn model(texts: &[&str]) -> Document {
        let measure = |points| Measure::Emu(Emu::PER_POINT * points);
        let mut page = PageSetup::a4_zh_cn();
        page.width = measure(240);
        page.height = measure(48);
        page.left = measure(6);
        page.right = page.left;
        page.top = page.left;
        page.bottom = page.left;
        page.header = measure(2);
        page.footer = page.header;
        page.gutter = measure(0);
        let mut document = Document::new(page);
        let mut blocks = Vec::new();
        for text in texts {
            let mut paragraph = Paragraph::new(document.ids().node());
            paragraph.push_text(text, None, Arc::default()).unwrap();
            blocks.push(paragraph.into());
        }
        let body = document.body_id();
        document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
        document
    }

    fn change(document: &mut Document, index: usize, f: impl FnOnce(&mut Paragraph)) {
        let body = document.body_id();
        document
            .story_mut(body)
            .unwrap()
            .blocks
            .update(index, |block| {
                let joyword_model::Block::Paragraph(paragraph) = block else {
                    panic!("test paragraph");
                };
                f(Arc::make_mut(paragraph));
            });
    }

    fn position(document: &Document, paragraph: usize, offset: usize) -> Position {
        Position {
            story: document.body_id(),
            paragraph: document.body().blocks.get(paragraph).unwrap().id(),
            offset: ScalarOffset(offset),
            bias: Bias::After,
        }
    }

    fn bookmark(document: &mut Document, name: &str, start: Position, end: Position) {
        document.insert_annotation(
            AnnotationKind::Bookmark {
                name: Arc::from(name),
            },
            Anchor::Range { start, end },
        );
    }

    fn layout(document: &Document, view: ViewOptions) -> LayoutSnapshot {
        layout_with_view(
            &document.snapshot(),
            FaceSet::uniform(&test_font()),
            rehearsal_profile(),
            LayoutLimits::default(),
            &LayoutCancellation::default(),
            view,
        )
        .unwrap()
    }

    fn chapter_model() -> Document {
        let mut document = model(&["One", "P", "Two", "AV URI P", "Three", "AV URI P"]);
        document.sections_mut()[0].numbering.chapter_style = Some(1);
        document
            .styles_mut()
            .insert(Style::new("h1", StyleKind::Paragraph, Some("heading 1")))
            .unwrap();
        let mut definition = AbstractNum::new(1);
        let mut level = NumberingLevel::new(NumberFormat::UpperRoman, "%1");
        level.start = 4;
        level.suffix = NumberSuffix::Nothing;
        definition.levels[0] = Some(level);
        assert!(document.numbering_mut().insert_abstract(definition));
        assert!(document.numbering_mut().insert_instance(NumInstance {
            id: 1,
            abstract_id: 1,
            overrides: Vec::new(),
        }));
        for index in [0, 2, 4] {
            change(&mut document, index, |paragraph| {
                paragraph.style = Some(StyleId::new("h1"));
                Arc::make_mut(&mut paragraph.props).numbering = Some(NumberingRef {
                    num_id: 1,
                    level: 0,
                });
            });
        }
        for index in [1, 3, 5] {
            let id = document.ids().node();
            change(&mut document, index, |paragraph| {
                paragraph.push_inline(
                    InlineObject {
                        id,
                        kind: InlineKind::PageField(PageField::new(PageFieldKind::Page)),
                        source: None,
                    },
                    None,
                    Arc::default(),
                );
                if index == 3 {
                    paragraph
                        .set_links(
                            ScalarOffset(0)..ScalarOffset(2),
                            Some(Arc::new(Hyperlink::for_edit(None, Some("target")).unwrap())),
                        )
                        .unwrap();
                }
                if index != 1 {
                    paragraph
                        .set_links(
                            ScalarOffset(3)..ScalarOffset(6),
                            Some(Arc::new(
                                Hyperlink::for_edit(Some("https://example.invalid/uri"), None)
                                    .unwrap(),
                            )),
                        )
                        .unwrap();
                }
            });
        }
        let target = position(&document, 4, 1);
        let end = position(&document, 4, 2);
        bookmark(&mut document, "target", target, end);
        document
    }

    fn near(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 0.002, "{actual} != {expected}");
    }

    fn check_target(
        read: &PdfDestinationReport,
        scene: &LayoutSnapshot,
        target: Position,
        start: usize,
    ) {
        let caret = scene.caret(target).unwrap();
        assert_eq!(read.page, caret.page - start);
        assert!(read.page_object > 0);
        near(read.x, to_points(caret.x) as f32);
        near(
            read.y,
            to_points(scene.pages()[caret.page].height) as f32 - to_points(caret.top) as f32,
        );
        assert_eq!(read.zoom, Some(0.0));
    }

    fn check_ranges(document: &Document, scene: &LayoutSnapshot) {
        let target = position(document, 4, 1);
        let source = position(document, 3, 0);
        assert_eq!(scene.caret(source).unwrap().page, 1);
        assert_eq!(scene.caret(target).unwrap().page, 2);
        for (range, gotos, bookmarks, uris) in [
            (0..3, 1, 1, 2),
            (1..3, 1, 1, 2),
            (1..2, 0, 0, 1),
            (2..3, 0, 1, 1),
            (0..1, 0, 0, 0),
        ] {
            let pdf = scene_pdf(scene, &test_font(), document, None, range.clone()).unwrap();
            let info = inspect_pdf(&pdf).unwrap();
            let read = inspect_navigation(&pdf).unwrap();
            assert_eq!(info.pages.len(), range.len());
            for ((width, height), page) in info.pages.iter().zip(&scene.pages()[range.clone()]) {
                near(*width, to_points(page.width) as f32);
                near(*height, to_points(page.height) as f32);
            }
            assert_eq!(info.outlines, ["One", "Two", "Three"][range.clone()]);
            assert_eq!(info.link_uris, vec!["https://example.invalid/uri"; uris]);
            assert_eq!(read.links.len(), gotos, "{range:?}");
            assert_eq!(read.bookmarks.len(), bookmarks, "{range:?}");
            if bookmarks != 0 {
                check_target(&read.bookmarks["target"], scene, target, range.start);
            }
            if gotos != 0 {
                let link = &read.links[0];
                assert_eq!(link.source_page, 1 - range.start);
                check_target(&link.destination, scene, target, range.start);
                assert_eq!(link.destination, read.bookmarks["target"]);
                let rect = scene
                    .selection_rects(source, position(document, 3, 2))
                    .unwrap()[0];
                let height = to_points(scene.pages()[rect.page].height) as f32;
                for (actual, expected) in link.rect.into_iter().zip([
                    to_points(rect.x) as f32,
                    height - to_points(rect.top) as f32 - to_points(rect.height) as f32,
                    to_points(rect.x) as f32 + to_points(rect.width) as f32,
                    height - to_points(rect.top) as f32,
                ]) {
                    near(actual, expected);
                }
            }
        }
    }

    #[test]
    fn chapter_bookmarks_warm_scene_and_selected_pdf_share_real_page_geometry() {
        let mut document = chapter_model();
        let font = test_font();
        let mut cache = ParagraphLayoutCache::default();
        let mut previous = None;
        // Metadata precedes every layout, including cold, warm and changed PAGE feedback.
        for step in 0..3 {
            if step == 2 {
                document.numbering_mut().abstracts_mut()[0].levels[0]
                    .as_mut()
                    .unwrap()
                    .start = 14;
                document.sections_mut()[0].numbering.chapter_separator = ChapterSeparator::Colon;
                document.advance_version();
            }
            assert!(!document.validate().has_errors());
            let full = layout(&document, ViewOptions::default());
            let warm = layout_incremental_with_view(
                &document.snapshot(),
                FaceSet::uniform(&font),
                rehearsal_profile(),
                LayoutLimits::default(),
                &LayoutCancellation::default(),
                LayoutReuse {
                    cache: &mut cache,
                    previous: previous.as_ref(),
                    invalidation: &LayoutInvalidation::default(),
                },
                ViewOptions::default(),
            )
            .unwrap();
            assert_eq!(full.key(), warm.key());
            assert_eq!(full.pages(), warm.pages());
            assert_eq!(full.lines(), warm.lines());
            warm.ensure_current(&document.snapshot(), &font, rehearsal_profile())
                .unwrap();
            assert_eq!(warm.pages().len(), 3);
            if step == 1 {
                assert!(warm.stats().cache_hits > 0);
            }
            let chapters = if step == 2 {
                ["XIV:1", "XV:2", "XVI:3"]
            } else {
                ["IV\u{2011}1", "V\u{2011}2", "VI\u{2011}3"]
            };
            for (page, chapter) in warm.pages().iter().zip(chapters) {
                let text: String = page.runs.iter().map(|run| run.text.as_str()).collect();
                assert!(text.ends_with(&format!("P{chapter}")), "{text}");
            }
            check_ranges(&document, &warm);
            assert_eq!(
                scene_pdf(&warm, &font, &document, None, 0..3).unwrap(),
                scene_pdf(&full, &font, &document, None, 0..3).unwrap(),
            );
            previous = Some(warm);
        }
    }

    #[test]
    fn plain_scene_pdf_keeps_the_existing_export_and_rejects_stale_versions_and_ranges() {
        let mut document = model(&["AV"]);
        change(&mut document, 0, |paragraph| {
            paragraph
                .set_links(
                    ScalarOffset(0)..ScalarOffset(2),
                    Some(Arc::new(
                        Hyperlink::for_edit(Some("https://example.invalid/plain"), None).unwrap(),
                    )),
                )
                .unwrap();
        });
        let scene = layout(&document, ViewOptions::default());
        let font = test_font();
        let pdf = scene_pdf(&scene, &font, &document, None, 0..1).unwrap();
        assert_eq!(
            pdf,
            render_export(
                scene.pages(),
                std::slice::from_ref(&font),
                &PdfExport {
                    title: None,
                    outline: outline_from_headings(&document, &scene),
                    links: links_from_document(&document, &scene),
                }
            )
            .unwrap()
        );
        let read = inspect_navigation(&pdf).unwrap();
        assert!(read.bookmarks.is_empty() && read.links.is_empty());
        assert_eq!(
            inspect_pdf(&pdf).unwrap().link_uris,
            ["https://example.invalid/plain"]
        );
        for range in [0..0, 1..1, 0..2] {
            assert_eq!(
                scene_pdf(&scene, &font, &document, None, range),
                Err("range".into())
            );
        }
        document.advance_version();
        assert_eq!(
            scene_pdf(&scene, &font, &document, None, 0..1),
            Err("navigation".into())
        );
    }

    #[test]
    fn scene_export_refuses_invalid_names_detached_and_duplicate_targets_before_slicing() {
        for case in 0..4 {
            let mut document = chapter_model();
            let point = position(&document, 0, 0);
            match case {
                0 => bookmark(&mut document, "", point, point),
                1 => bookmark(&mut document, "PRIVATE\u{0}NAME", point, point),
                2 => bookmark(&mut document, "target", point, point),
                _ => {
                    document.insert_annotation(
                        AnnotationKind::Bookmark {
                            name: Arc::from("PRIVATE"),
                        },
                        Anchor::Detached,
                    );
                }
            }
            let scene = layout(&document, ViewOptions::default());
            scene
                .ensure_current(&document.snapshot(), &test_font(), rehearsal_profile())
                .unwrap();
            for range in [0..3, 0..1] {
                assert_eq!(
                    scene_pdf(&scene, &test_font(), &document, None, range),
                    Err("navigation".into())
                );
            }
        }
    }

    #[test]
    #[test]
    fn a_selected_page_range_renumbers_both_ends_and_drops_a_link_it_cannot_keep() {
        let document = chapter_model();
        let scene = layout(&document, ViewOptions::default());
        scene
            .ensure_current(&document.snapshot(), &test_font(), rehearsal_profile())
            .unwrap();
        let font = test_font();
        let target = position(&document, 4, 1);
        let source = position(&document, 3, 0);
        // The bookmark sits on the third page and its link on the second.
        assert_eq!(scene.caret(source).unwrap().page, 1);
        assert_eq!(scene.caret(target).unwrap().page, 2);

        // The whole document keeps the link and the named destination.
        let full = scene_pdf(&scene, &font, &document, None, 0..3).unwrap();
        let read = inspect_navigation(&full).unwrap();
        assert_eq!(read.links.len(), 1);
        assert_eq!(read.bookmarks.len(), 1);
        check_target(&read.links[0].destination, &scene, target, 0);

        // The second page alone holds the source but not the target, so the
        // link has no destination left and is dropped rather than misdirected.
        let source_only = scene_pdf(&scene, &font, &document, None, 1..2).unwrap();
        let read = inspect_navigation(&source_only).unwrap();
        assert!(read.links.is_empty(), "{:?}", read.links);
        assert!(read.bookmarks.is_empty(), "{:?}", read.bookmarks);

        // The third page alone holds the target but not the source, so the
        // bookmark survives and no link points at it.
        let target_only = scene_pdf(&scene, &font, &document, None, 2..3).unwrap();
        let read = inspect_navigation(&target_only).unwrap();
        assert!(read.links.is_empty(), "{:?}", read.links);
        assert_eq!(read.bookmarks.len(), 1);
        check_target(&read.bookmarks["target"], &scene, target, 2);

        // The second and third pages together keep the link, and both ends
        // are renumbered against the exported page range.
        let both = scene_pdf(&scene, &font, &document, None, 1..3).unwrap();
        let read = inspect_navigation(&both).unwrap();
        assert_eq!(read.links.len(), 1);
        assert_eq!(read.bookmarks.len(), 1);
        check_target(&read.links[0].destination, &scene, target, 1);
        check_target(&read.bookmarks["target"], &scene, target, 1);
    }

    fn an_anchor_without_a_target_does_not_block_the_export() {
        // A DOCX keeps its bookmarks and its anchor links apart and only the
        // links are imported, so a real document can carry an anchor whose
        // target is not a bookmark of this document.
        let mut document = chapter_model();
        change(&mut document, 3, |paragraph| {
            paragraph
                .set_links(
                    ScalarOffset(0)..ScalarOffset(2),
                    Some(Arc::new(
                        Hyperlink::for_edit(None, Some("PRIVATE-missing")).unwrap(),
                    )),
                )
                .unwrap();
        });
        let scene = layout(&document, ViewOptions::default());
        scene
            .ensure_current(&document.snapshot(), &test_font(), rehearsal_profile())
            .unwrap();
        for range in [0..3, 0..1] {
            let pdf = scene_pdf(&scene, &test_font(), &document, None, range).unwrap();
            let read = inspect_navigation(&pdf).unwrap();
            assert!(read.links.is_empty(), "{:?}", read.links);
        }
    }

    #[test]
    fn bookmark_endpoints_require_visible_exact_carets_in_the_installed_view() {
        for (start, end) in [(3, 3), (0, 3)] {
            let mut document = model(&["AsecretV"]);
            change(&mut document, 0, |paragraph| {
                paragraph
                    .format_runs(ScalarOffset(1)..ScalarOffset(7), |props| {
                        props.hidden = Some(true)
                    })
                    .unwrap();
            });
            let from = position(&document, 0, start);
            let to = position(&document, 0, end);
            bookmark(&mut document, "target", from, to);
            let hidden = layout(&document, ViewOptions::default());
            assert!(hidden.caret(to).is_err());
            assert_eq!(
                scene_pdf(&hidden, &test_font(), &document, None, 0..1),
                Err("navigation".into())
            );
            let shown = layout(
                &document,
                ViewOptions {
                    show_hidden: true,
                    ..ViewOptions::default()
                },
            );
            shown
                .ensure_current(&document.snapshot(), &test_font(), rehearsal_profile())
                .unwrap();
            let pdf = scene_pdf(&shown, &test_font(), &document, None, 0..1).unwrap();
            check_target(
                &inspect_navigation(&pdf).unwrap().bookmarks["target"],
                &shown,
                from,
                0,
            );
        }
        let mut document = model(&["fi"]);
        let start = position(&document, 0, 0);
        let end = position(&document, 0, 1);
        bookmark(&mut document, "target", start, end);
        let mut profile = rehearsal_profile();
        profile.features.standard_ligatures = true;
        let scene = layout_with_view(
            &document.snapshot(),
            FaceSet::uniform(&test_font()),
            profile,
            LayoutLimits::default(),
            &LayoutCancellation::default(),
            ViewOptions::default(),
        )
        .unwrap();
        assert!(scene.caret(end).is_err());
        assert_eq!(
            scene_pdf(&scene, &test_font(), &document, None, 0..1),
            Err("navigation".into())
        );
    }
}

fn mapped(args: &[(&'static str, ArgValue)]) -> std::collections::BTreeMap<&'static str, ArgValue> {
    args.iter().cloned().collect()
}

fn flag(args: &std::collections::BTreeMap<&'static str, ArgValue>, name: &str) -> bool {
    matches!(args.get(name), Some(ArgValue::Bool(true)))
}

fn request_width_regex(request: &joyword_shell::input::EditRequest) -> (bool, bool) {
    match request {
        joyword_shell::input::EditRequest::Find {
            fold_width, regex, ..
        }
        | joyword_shell::input::EditRequest::Replace {
            fold_width, regex, ..
        } => (*fold_width, *regex),
        _ => panic!("not a find request"),
    }
}

#[test]
fn find_bar_checkboxes_set_width_and_regex_on_every_find_command() {
    let off = FindBarFlags::default();
    assert_eq!(
        off,
        FindBarFlags {
            case_sensitive: false,
            whole_word: false,
            fold_width: false,
            regex: false,
        }
    );
    for replacement in [None, Some("x")] {
        let args = off.args("ABC", replacement);
        let map = mapped(&args);
        assert!(!flag(&map, "case"));
        assert!(!flag(&map, "word"));
        assert!(!flag(&map, "width"));
        assert!(!flag(&map, "regex"));
        assert_eq!(map.contains_key("replacement"), replacement.is_some());
    }

    let cases = [
        (
            FindBarFlags {
                fold_width: true,
                ..off
            },
            true,
            false,
        ),
        (FindBarFlags { regex: true, ..off }, false, true),
        (
            FindBarFlags {
                fold_width: true,
                regex: true,
                ..off
            },
            true,
            true,
        ),
    ];
    for (flags, width_on, regex_on) in cases {
        for (id, replacement) in [
            (FIND_NEXT, None),
            (FIND_PREVIOUS, None),
            (FIND_REPLACE, Some("x")),
            (FIND_REPLACE_ALL, Some("x")),
        ] {
            let map = mapped(&flags.args("ABC", replacement));
            assert_eq!(flag(&map, "width"), width_on, "{id}");
            assert_eq!(flag(&map, "regex"), regex_on, "{id}");
            let request = edit_request_from_args(id, &map).unwrap();
            assert_eq!(request_width_regex(&request), (width_on, regex_on), "{id}");
        }
    }

    // A caller that only sends the older keys (the ribbon binder) picks up
    // the checkboxes. An explicit false stays false.
    let older: Vec<_> = off
        .args("ABC", None)
        .into_iter()
        .filter(|(name, _)| *name != "width" && *name != "regex")
        .collect();
    let filled = supplement_find_flags(&older, true, true);
    let map = mapped(&filled);
    assert!(flag(&map, "width") && flag(&map, "regex"));
    let explicit = supplement_find_flags(&off.args("ABC", None), true, true);
    let map = mapped(&explicit);
    assert!(!flag(&map, "width") && !flag(&map, "regex"));
}

fn wide_editor() -> joyword_shell::input::TextInput<joyword_engine::Session> {
    use joyword_engine::{EditPolicy, Session};
    use joyword_model::{BlockSeq, Document, PageSetup, Paragraph};
    use std::sync::Arc;

    let mut document = Document::new(PageSetup::a4_zh_cn());
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph
        .push_text("ＡＢＣ 12 dog 34", None, Arc::default())
        .unwrap();
    let story = document.body_id();
    document.story_mut(story).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let mut session = Session::new(document);
    session.set_policy(EditPolicy {
        read_only: false,
        agent_writes: false,
    });
    joyword_shell::input::TextInput::new(session, joyword_shell::input::InputLimits::default())
        .unwrap()
}

fn body_text(input: &joyword_shell::input::TextInput<joyword_engine::Session>) -> String {
    input
        .backend()
        .document()
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap()
        .plain_text(|_| String::new())
}

fn invoke_find(
    input: &mut joyword_shell::input::TextInput<joyword_engine::Session>,
    id: CommandId,
    flags: FindBarFlags,
    query: &str,
    replacement: Option<&str>,
) -> Result<joyword_shell::input::EditReport, joyword_shell::input::InputError> {
    let map = mapped(&flags.args(query, replacement));
    let request = edit_request_from_args(id, &map).expect("find bar args");
    input.editing(request)
}

#[test]
fn find_bar_width_hits_fullwidth_and_regex_rejects_invalid_patterns() {
    let original = "ＡＢＣ 12 dog 34";
    let mut input = wide_editor();
    assert_eq!(body_text(&input), original);

    let miss = invoke_find(&mut input, FIND_NEXT, FindBarFlags::default(), "ABC", None).unwrap();
    assert_eq!(miss.matches, 0);
    assert_eq!(input.selected_text().unwrap(), "");
    assert_eq!(body_text(&input), original);

    let literal = invoke_find(
        &mut input,
        FIND_NEXT,
        FindBarFlags::default(),
        "ＡＢＣ",
        None,
    )
    .unwrap();
    assert_eq!(literal.matches, 1);
    assert_eq!(input.selected_text().unwrap(), "ＡＢＣ");

    let mut folded = wide_editor();
    let next = invoke_find(
        &mut folded,
        FIND_NEXT,
        FindBarFlags {
            fold_width: true,
            ..FindBarFlags::default()
        },
        "ABC",
        None,
    )
    .unwrap();
    assert_eq!(next.matches, 1);
    assert_eq!(folded.selected_text().unwrap(), "ＡＢＣ");
    let mut backward = wide_editor();
    let previous = invoke_find(
        &mut backward,
        FIND_PREVIOUS,
        FindBarFlags {
            fold_width: true,
            ..FindBarFlags::default()
        },
        "ABC",
        None,
    )
    .unwrap();
    assert_eq!(previous.matches, 1);
    assert_eq!(backward.selected_text().unwrap(), "ＡＢＣ");

    let replaced = invoke_find(
        &mut folded,
        FIND_REPLACE,
        FindBarFlags {
            fold_width: true,
            ..FindBarFlags::default()
        },
        "ABC",
        Some("Z"),
    )
    .unwrap();
    assert_eq!(replaced.replaced, 1);
    assert_eq!(body_text(&folded), "Z 12 dog 34");
    folded.undo().unwrap();
    assert_eq!(body_text(&folded), original);

    let literal_digits = invoke_find(
        &mut input,
        FIND_REPLACE_ALL,
        FindBarFlags::default(),
        r"\d+",
        Some("#"),
    )
    .unwrap();
    assert_eq!(literal_digits.replaced, 0);
    assert_eq!(body_text(&input), original);

    let once = invoke_find(
        &mut input,
        FIND_REPLACE,
        FindBarFlags {
            regex: true,
            ..FindBarFlags::default()
        },
        r"\d+",
        Some("#"),
    )
    .unwrap();
    assert_eq!(once.replaced, 1);
    assert_eq!(body_text(&input), "ＡＢＣ # dog 34");
    input.undo().unwrap();
    assert_eq!(body_text(&input), original);

    let all = invoke_find(
        &mut input,
        FIND_REPLACE_ALL,
        FindBarFlags {
            regex: true,
            ..FindBarFlags::default()
        },
        r"\d+",
        Some("#"),
    )
    .unwrap();
    assert_eq!(all.replaced, 2);
    assert_eq!(body_text(&input), "ＡＢＣ # dog #");
    input.undo().unwrap();
    assert_eq!(body_text(&input), original);

    let invalid = invoke_find(
        &mut input,
        FIND_NEXT,
        FindBarFlags {
            regex: true,
            ..FindBarFlags::default()
        },
        "(",
        None,
    )
    .unwrap_err();
    assert_eq!(
        invalid,
        joyword_shell::input::InputError::Rejected("query is not a valid pattern")
    );
    assert_eq!(body_text(&input), original);
}

// --- Headless rehearsals of T200, T201, T202, T209 and T300. ---
//
// These chains call the same registry and input objects the window uses.
// Drag, autocorrect, the IME candidate window, on-screen zoom and the Word
// or WPS side of an exchange are not rehearsable here.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use joyword_foundation::{Emu, ScreenScale};
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LayoutSnapshot, LoadedFont,
    SingleFontProfile, ViewOptions, layout_document_viewed, layout_with_view, to_screen,
};
use joyword_model::{
    Block, BorderKind, HeaderFooterKind, Justification, Orientation, PageSetup, RevisionKind,
    RevisionView, StoryKind, TableAlign, TableWidth, VerticalMerge,
};
use joyword_shell::command::{
    EDIT_BOLD, EDIT_JUSTIFY, FILE_OPEN, FILE_SAVE_COPY, FORMAT_COPY, FORMAT_PAINT, LIST_APPLY,
    LIST_LEVEL, PAGE_EDIT_BODY, PAGE_EDIT_HEADER, PAGE_MARGINS, PAGE_NUMBER, PAGE_ORIENTATION,
    REVIEW_ACCEPT, REVIEW_ACCEPT_ALL, REVIEW_FINAL, REVIEW_MARKUP, REVIEW_ORIGINAL, REVIEW_REJECT,
    REVIEW_REJECT_ALL, REVIEW_TRACK, STYLE_APPLY, STYLE_LIST, TABLE_COLUMN_INSERT, TABLE_INSERT,
    TABLE_MERGE_DOWN, TABLE_MERGE_RIGHT, TABLE_PROPERTIES, TABLE_ROW_DELETE, TABLE_ROW_INSERT,
    TABLE_SPLIT,
};
use joyword_shell::host::Office;
use joyword_shell::input::{Direction, InputSource};

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const CT_NS: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const REL_OFFICE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const REL_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
const REL_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
const REL_HEADER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
const REL_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
const CT_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const CT_STYLES: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
const CT_NUMBERING: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
const CT_HEADER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
const CT_SETTINGS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml";

/// ISO-HDLC / ITU-T V.42 CRC-32. The ZIP reader checks this value.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Stored ZIP (PKWARE APPNOTE). Method 0, UTF-8 names, no encryption.
fn store_zip(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut local = Vec::new();
    let mut central = Vec::new();
    for (name, data) in parts {
        let offset = u32::try_from(local.len()).expect("zip offset");
        let crc = crc32(data);
        let name_bytes = name.as_bytes();
        let name_len = u16::try_from(name_bytes.len()).expect("zip name");
        let size = u32::try_from(data.len()).expect("zip size");
        local.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        local.extend_from_slice(&20u16.to_le_bytes());
        local.extend_from_slice(&0x0800u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0x0021u16.to_le_bytes());
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&name_len.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(name_bytes);
        local.extend_from_slice(data);

        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0x0800u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0x0021u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&name_len.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }
    let central_offset = u32::try_from(local.len()).expect("central offset");
    let central_size = u32::try_from(central.len()).expect("central size");
    let count = u16::try_from(parts.len()).expect("zip count");
    let mut out = local;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&central_size.to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn content_types(overrides: &[(&str, &str)]) -> String {
    let mut body = String::new();
    for (part, kind) in overrides {
        body.push_str(&format!(
            r#"<Override PartName="{part}" ContentType="{kind}"/>"#
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>{body}</Types>"#
    )
}

fn relationships(items: &[(&str, &str, &str)]) -> String {
    let mut body = String::new();
    for (id, kind, target) in items {
        body.push_str(&format!(
            r#"<Relationship Id="{id}" Type="{kind}" Target="{target}"/>"#
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL_NS}">{body}</Relationships>"#
    )
}

fn document_xml(inner: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="{W_NS}" xmlns:r="{R_NS}"><w:body>{inner}</w:body></w:document>"#
    )
}

fn a4_sect(extra: &str) -> String {
    format!(
        r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838" w:orient="portrait"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="851" w:footer="992" w:gutter="0"/>{extra}</w:sectPr>"#
    )
}

fn scratch(label: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("joyword-s5-{label}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn recovery_failure_log_reads_back_both_slots_and_legacy_without_content() {
    use joyword_shell::diag::AppLog;
    for names in [
        vec!["recovery.jwr"],
        vec!["recovery-a.jwr"],
        vec!["recovery-b.jwr"],
        vec!["recovery-a.jwr", "recovery-b.jwr"],
    ] {
        let dir = scratch("recovery-failure");
        let log = AppLog::open(dir.join("logs")).unwrap();
        for name in names {
            std::fs::write(dir.join(name), b"PRIVATE-BODY token=PRIVATE-CREDENTIAL").unwrap();
        }
        assert!(super::take_recovered(&dir, Some(&log)).is_none());
        let text = std::fs::read_to_string(log.path()).unwrap();
        assert_eq!(text.matches("recovery.failed").count(), 1, "{text}");
        assert!(!text.contains("recovery.restored"));
        assert!(!text.contains("PRIVATE-BODY"));
        assert!(!text.contains("PRIVATE-CREDENTIAL"));
        assert!(!text.contains(&dir.to_string_lossy().to_string()));
        drop(log);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn recovery_log_distinguishes_absence_discard_restore_and_package_failure() {
    use joyword_shell::{diag::AppLog, storage};
    let dir = scratch("recovery-states");
    let journal = dir.join("journal");
    let log = AppLog::open(dir.join("logs")).unwrap();
    assert!(super::take_recovered(&journal, Some(&log)).is_none());
    std::fs::create_dir(&journal).unwrap();
    assert!(super::take_recovered(&journal, Some(&log)).is_none());
    let package = plain_package("<w:p><w:r><w:t>Recovered</w:t></w:r></w:p>");
    storage::commit_recovery_versioned(&journal, 1, &package, "Recovered").unwrap();
    storage::commit_recovery_versioned(&journal, 2, &package, "Recovered").unwrap();
    assert!(super::take_recovered(&journal, Some(&log)).is_some());
    storage::clear_recovery(&journal).unwrap();
    assert!(super::take_recovered(&journal, Some(&log)).is_none());
    let text = std::fs::read_to_string(log.path()).unwrap();
    assert!(!text.contains("recovery.failed"), "{text}");
    assert_eq!(text.matches("recovery.restored").count(), 1);
    // A valid checkpoint with an unreadable package is not absence/discard.
    storage::commit_recovery_versioned(&journal, 3, b"PK", "PRIVATE-BODY").unwrap();
    assert!(super::take_recovered(&journal, Some(&log)).is_none());
    let text = std::fs::read_to_string(log.path()).unwrap();
    assert_eq!(text.matches("recovery.failed").count(), 1, "{text}");
    assert!(!text.contains("PRIVATE-BODY"));
    drop(log);
    std::fs::remove_dir_all(dir).unwrap();
}

fn cmd(office: &mut Office, id: CommandId, args: &[(&'static str, ArgValue)]) {
    office
        .invoke(id, args)
        .unwrap_or_else(|error| panic!("{} failed: {error:?}", id.0));
}

fn open_package(bytes: &[u8], label: &str) -> (PathBuf, Office) {
    let dir = scratch(label);
    let source = dir.join("source.docx");
    std::fs::write(&source, bytes).unwrap();
    let mut office = Office::new();
    cmd(&mut office, FILE_OPEN, &[("path", ArgValue::Path(source))]);
    (dir, office)
}

fn save_and_reopen(office: &mut Office, dir: &Path, name: &str) {
    let destination = dir.join(name);
    cmd(
        office,
        FILE_SAVE_COPY,
        &[("destination", ArgValue::Path(destination.clone()))],
    );
    cmd(office, FILE_OPEN, &[("path", ArgValue::Path(destination))]);
}

fn active_document(office: &Office) -> &joyword_model::Document {
    office
        .active()
        .unwrap()
        .backend()
        .session()
        .session()
        .document()
}

fn story_text(office: &Office) -> String {
    office.active().unwrap().view().index().text().to_owned()
}

fn select_paragraph(office: &mut Office, index: usize) {
    let text = story_text(office);
    let mut start = 0usize;
    for (at, part) in text.split('\n').enumerate() {
        let len = part.encode_utf16().count();
        if at == index {
            office
                .active_mut()
                .unwrap()
                .select(start..start + len, false)
                .unwrap();
            return;
        }
        start += len + 1;
    }
    panic!("paragraph {index} missing from {text:?}");
}

fn caret_at(office: &mut Office, offset: usize) {
    office
        .active_mut()
        .unwrap()
        .select(offset..offset, false)
        .unwrap();
}

fn undo(office: &mut Office) {
    office.active_mut().unwrap().undo().unwrap();
}

fn redo(office: &mut Office) {
    office.active_mut().unwrap().redo().unwrap();
}

fn undo_all(office: &mut Office) {
    let mut steps = 0usize;
    while office.active().unwrap().can_undo() {
        undo(office);
        steps += 1;
        assert!(steps < 80, "undo did not drain");
    }
}

fn redo_all(office: &mut Office) {
    let mut steps = 0usize;
    while office.active().unwrap().can_redo() {
        redo(office);
        steps += 1;
        assert!(steps < 80, "redo did not drain");
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParaMark {
    text: String,
    style: Option<String>,
    numbering: Option<(u32, u8)>,
    bold: bool,
}

fn paragraph_marks(office: &Office) -> Vec<ParaMark> {
    active_document(office)
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(|paragraph| ParaMark {
            text: paragraph.plain_text(|_| String::new()),
            style: paragraph.style.as_ref().map(|id| id.as_str().to_owned()),
            numbering: paragraph
                .props
                .numbering
                .as_ref()
                .map(|item| (item.num_id, item.level)),
            bold: paragraph
                .runs()
                .iter()
                .any(|run| run.props.bold == Some(true)),
        })
        .collect()
}

fn test_font() -> LoadedFont {
    LoadedFont::from_bytes(
        std::sync::Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .expect("joyoffice-test-serif")
}

fn rehearsal_profile() -> SingleFontProfile {
    SingleFontProfile {
        size: Emu::new(12 * Emu::PER_POINT),
        leading: Emu::new(18 * Emu::PER_POINT),
        baseline: Emu::new(14 * Emu::PER_POINT),
        features: FeatureSet::default(),
    }
}

fn lay(office: &Office, view: ViewOptions) -> LayoutSnapshot {
    let font = test_font();
    let snapshot = active_document(office).snapshot();
    layout_with_view(
        &snapshot,
        FaceSet::uniform(&font),
        rehearsal_profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        view,
    )
    .unwrap_or_else(|error| panic!("layout failed: {error}"))
}

fn lay_review(office: &Office, view: RevisionView) -> LayoutSnapshot {
    let font = test_font();
    let snapshot = active_document(office).snapshot();
    layout_document_viewed(
        &snapshot,
        FaceSet::uniform(&font),
        rehearsal_profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        view,
    )
    .unwrap_or_else(|error| panic!("review layout failed: {error}"))
}

fn scene_text(snapshot: &LayoutSnapshot) -> String {
    snapshot
        .pages()
        .iter()
        .flat_map(|page| page.runs.iter().map(|run| run.text.as_str()))
        .collect()
}

fn body_page_text(snapshot: &LayoutSnapshot, page: usize) -> String {
    snapshot
        .lines()
        .iter()
        .filter(|line| line.page == page)
        .fold(String::new(), |mut text, line| {
            text.push_str(&snapshot.pages()[line.page].runs[line.run].text);
            text
        })
}

fn header_page_numbers(snapshot: &LayoutSnapshot) -> Vec<u32> {
    snapshot
        .pages()
        .iter()
        .map(|page| {
            let joined: String = page.runs.iter().map(|run| run.text.as_str()).collect();
            let start = joined
                .find('V')
                .unwrap_or_else(|| panic!("header run missing from {joined}"));
            let digits: String = joined[start + 'V'.len_utf8()..]
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect();
            digits
                .parse()
                .unwrap_or_else(|_| panic!("page number missing from {joined}"))
        })
        .collect()
}

fn styles_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:styles xmlns:w="{W_NS}"><w:style w:type="paragraph" w:styleId="Title1"><w:name w:val="heading 1"/><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Title2"><w:name w:val="heading 2"/><w:basedOn w:val="Title1"/><w:rPr><w:i/></w:rPr></w:style></w:styles>"#
    )
}

fn numbering_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:numbering xmlns:w="{W_NS}"><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1.%2."/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#
    )
}

fn plain_package(paragraphs: &str) -> Vec<u8> {
    let types = content_types(&[("/word/document.xml", CT_DOCUMENT)]);
    let package_rels = relationships(&[("rId1", REL_OFFICE, "word/document.xml")]);
    let document = document_xml(&format!("{paragraphs}{}", a4_sect("")));
    let owned = [types, package_rels, document];
    let parts = [
        ("[Content_Types].xml", owned[0].as_bytes()),
        ("_rels/.rels", owned[1].as_bytes()),
        ("word/document.xml", owned[2].as_bytes()),
    ];
    store_zip(&parts)
}

/// Automated rehearsal; does not replace 人工验收.
///
/// T200: headings, two list levels, the format painter, width-folded regex
/// replacement with `$1` and `$$`, then a full undo back to the opening text.
#[test]
fn t200_headings_lists_painter_and_regex_undo_round_trip() {
    let paragraphs = [
        "River brief",
        "Brook note",
        "First item",
        "Nested item",
        "ＡＡbbb fee",
    ]
    .into_iter()
    .map(|text| format!(r#"<w:p><w:r><w:t>{text}</w:t></w:r></w:p>"#))
    .collect::<String>();
    let types = content_types(&[
        ("/word/document.xml", CT_DOCUMENT),
        ("/word/styles.xml", CT_STYLES),
        ("/word/numbering.xml", CT_NUMBERING),
    ]);
    let package_rels = relationships(&[("rId1", REL_OFFICE, "word/document.xml")]);
    let document_rels = relationships(&[
        ("rId1", REL_STYLES, "styles.xml"),
        ("rId2", REL_NUMBERING, "numbering.xml"),
    ]);
    let document = document_xml(&format!("{paragraphs}{}", a4_sect("")));
    let styles = styles_xml();
    let numbering = numbering_xml();
    let owned = [
        types,
        package_rels,
        document_rels,
        document,
        styles,
        numbering,
    ];
    let bytes = store_zip(&[
        ("[Content_Types].xml", owned[0].as_bytes()),
        ("_rels/.rels", owned[1].as_bytes()),
        ("word/_rels/document.xml.rels", owned[2].as_bytes()),
        ("word/document.xml", owned[3].as_bytes()),
        ("word/styles.xml", owned[4].as_bytes()),
        ("word/numbering.xml", owned[5].as_bytes()),
    ]);
    let (dir, mut office) = open_package(&bytes, "t200");
    let opening = paragraph_marks(&office);
    assert_eq!(
        opening
            .iter()
            .map(|mark| mark.text.as_str())
            .collect::<Vec<_>>(),
        vec![
            "River brief",
            "Brook note",
            "First item",
            "Nested item",
            "ＡＡbbb fee"
        ]
    );
    assert!(
        opening
            .iter()
            .all(|mark| { mark.style.is_none() && mark.numbering.is_none() && !mark.bold })
    );

    cmd(&mut office, STYLE_LIST, &[]);
    let listed = office.listed_styles().join("\n");
    assert!(listed.contains("heading 1"), "{listed}");
    assert!(listed.contains("heading 2"), "{listed}");

    select_paragraph(&mut office, 0);
    cmd(&mut office, EDIT_BOLD, &[]);
    cmd(&mut office, FORMAT_COPY, &[]);
    assert!(office.active().unwrap().format_armed());
    select_paragraph(&mut office, 1);
    cmd(&mut office, FORMAT_PAINT, &[]);
    assert!(!office.active().unwrap().format_armed());
    assert!(paragraph_marks(&office)[0].bold);
    assert!(paragraph_marks(&office)[1].bold);

    select_paragraph(&mut office, 0);
    cmd(
        &mut office,
        STYLE_APPLY,
        &[("style", ArgValue::Text("Title1".into()))],
    );
    select_paragraph(&mut office, 1);
    cmd(
        &mut office,
        STYLE_APPLY,
        &[("style", ArgValue::Text("Title2".into()))],
    );
    assert_eq!(paragraph_marks(&office)[0].style.as_deref(), Some("Title1"));
    assert_eq!(paragraph_marks(&office)[1].style.as_deref(), Some("Title2"));

    select_paragraph(&mut office, 2);
    cmd(
        &mut office,
        LIST_APPLY,
        &[("num", ArgValue::Text("1".into()))],
    );
    select_paragraph(&mut office, 3);
    cmd(
        &mut office,
        LIST_APPLY,
        &[("num", ArgValue::Text("1".into()))],
    );
    cmd(
        &mut office,
        LIST_LEVEL,
        &[("level", ArgValue::Text("1".into()))],
    );
    assert_eq!(paragraph_marks(&office)[2].numbering, Some((1, 0)));
    assert_eq!(paragraph_marks(&office)[3].numbering, Some((1, 1)));

    // Half-width `A` does not hit `Ａ` until width folding is on.
    cmd(
        &mut office,
        FIND_REPLACE_ALL,
        &[
            ("query", ArgValue::Text("(A+)(b+)".into())),
            ("replacement", ArgValue::Text("$2-$$-$1".into())),
            ("regex", ArgValue::Bool(true)),
        ],
    );
    assert_eq!(paragraph_marks(&office)[4].text, "ＡＡbbb fee");
    cmd(
        &mut office,
        FIND_REPLACE_ALL,
        &[
            ("query", ArgValue::Text("(A+)(b+)".into())),
            ("replacement", ArgValue::Text("$2-$$-$1".into())),
            ("regex", ArgValue::Bool(true)),
            ("width", ArgValue::Bool(true)),
        ],
    );
    assert_eq!(paragraph_marks(&office)[4].text, "bbb-$-ＡＡ fee");
    let edited = paragraph_marks(&office);

    undo_all(&mut office);
    assert_eq!(paragraph_marks(&office), opening);
    redo_all(&mut office);
    assert_eq!(paragraph_marks(&office), edited);

    save_and_reopen(&mut office, &dir, "t200-copy.docx");
    assert_eq!(paragraph_marks(&office), edited);
    let _ = std::fs::remove_dir_all(&dir);
}

fn body_table(office: &Office) -> &joyword_model::Table {
    active_document(office)
        .body()
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table.as_ref()),
            _ => None,
        })
        .expect("contract table")
}

fn grid_fingerprint(office: &Office) -> String {
    let table = body_table(office);
    let mut out = format!(
        "cols={} width={:?} align={:?}\n",
        table.grid.len(),
        table.width,
        table.alignment
    );
    for row in &table.rows {
        for cell in &row.cells {
            let border = cell
                .borders
                .top
                .map(|edge| format!("{:?}", edge.kind))
                .unwrap_or_else(|| "-".into());
            out.push_str(&format!(
                " {}:{:?}:{border}",
                cell.grid_span, cell.vertical_merge
            ));
        }
        out.push('\n');
    }
    out
}

/// Automated rehearsal; does not replace 人工验收.
///
/// T201: insert a contract table, add and remove a row, add a column, set
/// properties, merge down, split, merge right, undoing each step, then
/// export and re-import the same grid.
#[test]
fn t201_contract_table_edits_undo_and_reimport() {
    let (dir, mut office) = open_package(
        &plain_package(r#"<w:p><w:r><w:t>Northwind supply note</w:t></w:r></w:p>"#),
        "t201",
    );
    assert_eq!(story_text(&office), "Northwind supply note");

    cmd(
        &mut office,
        TABLE_INSERT,
        &[
            ("rows", ArgValue::Text("2".into())),
            ("columns", ArgValue::Text("2".into())),
        ],
    );
    assert_eq!(body_table(&office).rows.len(), 2);
    assert_eq!(body_table(&office).grid.len(), 2);
    undo(&mut office);
    assert!(
        active_document(&office)
            .body()
            .blocks
            .iter()
            .all(|block| { !matches!(block, Block::Table(_)) })
    );
    redo(&mut office);
    assert_eq!(body_table(&office).rows.len(), 2);
    // Undo restores the pre-insert caret, which sits in the paragraph after
    // the table. Later table commands need a cell, so place it in the first.
    caret_at(&mut office, 0);

    cmd(&mut office, TABLE_ROW_INSERT, &[]);
    assert_eq!(body_table(&office).rows.len(), 3);
    undo(&mut office);
    assert_eq!(body_table(&office).rows.len(), 2);
    redo(&mut office);
    assert_eq!(body_table(&office).rows.len(), 3);
    caret_at(&mut office, 0);

    cmd(&mut office, TABLE_COLUMN_INSERT, &[]);
    assert_eq!(body_table(&office).grid.len(), 3);
    assert!(
        body_table(&office)
            .rows
            .iter()
            .all(|row| row.cells.len() == 3)
    );
    undo(&mut office);
    assert_eq!(body_table(&office).grid.len(), 2);
    redo(&mut office);
    caret_at(&mut office, 0);

    cmd(&mut office, TABLE_ROW_DELETE, &[]);
    assert_eq!(body_table(&office).rows.len(), 2);
    undo(&mut office);
    assert_eq!(body_table(&office).rows.len(), 3);
    caret_at(&mut office, 0);

    cmd(
        &mut office,
        TABLE_PROPERTIES,
        &[
            ("commit", ArgValue::Text("apply".into())),
            ("width", ArgValue::Text("dxa:4800".into())),
            ("align", ArgValue::Text("center".into())),
            ("border", ArgValue::Text("dashed".into())),
            ("border_size", ArgValue::Text("8".into())),
        ],
    );
    assert_eq!(body_table(&office).width, Some(TableWidth::Dxa(4800)));
    assert_eq!(body_table(&office).alignment, Some(TableAlign::Center));
    assert_eq!(
        body_table(&office).rows[0].cells[0]
            .borders
            .top
            .unwrap()
            .kind,
        BorderKind::Dashed
    );
    undo(&mut office);
    assert!(body_table(&office).width.is_none());
    redo(&mut office);
    caret_at(&mut office, 0);

    cmd(&mut office, TABLE_MERGE_DOWN, &[]);
    assert_eq!(
        body_table(&office).rows[0].cells[0].vertical_merge,
        VerticalMerge::Restart
    );
    assert_eq!(
        body_table(&office).rows[1].cells[0].vertical_merge,
        VerticalMerge::Continue
    );
    undo(&mut office);
    assert_eq!(
        body_table(&office).rows[0].cells[0].vertical_merge,
        VerticalMerge::None
    );
    redo(&mut office);
    caret_at(&mut office, 0);

    cmd(&mut office, TABLE_SPLIT, &[]);
    assert_eq!(
        body_table(&office).rows[0].cells[0].vertical_merge,
        VerticalMerge::None
    );
    assert_eq!(
        body_table(&office).rows[1].cells[0].vertical_merge,
        VerticalMerge::None
    );
    undo(&mut office);
    assert_eq!(
        body_table(&office).rows[0].cells[0].vertical_merge,
        VerticalMerge::Restart
    );
    redo(&mut office);
    caret_at(&mut office, 0);

    cmd(&mut office, TABLE_MERGE_RIGHT, &[]);
    assert_eq!(body_table(&office).rows[0].cells[0].grid_span, 2);
    assert_eq!(body_table(&office).rows[0].cells.len(), 2);
    undo(&mut office);
    assert_eq!(body_table(&office).rows[0].cells.len(), 3);
    redo(&mut office);
    let edited = grid_fingerprint(&office);
    assert!(edited.contains("cols=3"), "{edited}");
    assert!(edited.contains("Dxa(4800)"), "{edited}");
    assert!(edited.contains("Center"), "{edited}");
    assert!(story_text(&office).contains("Northwind supply note"));

    save_and_reopen(&mut office, &dir, "t201-copy.docx");
    assert_eq!(grid_fingerprint(&office), edited);
    let _ = std::fs::remove_dir_all(&dir);
}

fn section_page(office: &Office) -> PageSetup {
    active_document(office).sections()[0].page.clone()
}

fn header_story_present(office: &Office) -> bool {
    let section = &active_document(office).sections()[0];
    section.headers.iter().any(|(kind, story)| {
        *kind == HeaderFooterKind::Default
            && active_document(office)
                .story(*story)
                .is_some_and(|item| item.kind == StoryKind::Header)
    })
}

/// Automated rehearsal; does not replace 人工验收.
///
/// T202: margins, orientation and a page number, with the header story and
/// the laid-out PAGE values. Zoom is a screen projection. Formatting marks
/// are a view option that does not change the page count. The body PAGE
/// field that `page.number` inserts is saved as `w:fldSimple` and comes back
/// on re-import beside the header PAGE field.
#[test]
fn t202_margins_orientation_page_numbers_and_zoom() {
    let line = "A A A A A A A A A A";
    let mut paragraphs = String::new();
    for _ in 0..60 {
        paragraphs.push_str(&format!(r#"<w:p><w:r><w:t>{line}</w:t></w:r></w:p>"#));
    }
    let header = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:hdr xmlns:w="{W_NS}"><w:p><w:r><w:t>V</w:t></w:r><w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple></w:p></w:hdr>"#
    );
    let types = content_types(&[
        ("/word/document.xml", CT_DOCUMENT),
        ("/word/header1.xml", CT_HEADER),
    ]);
    let package_rels = relationships(&[("rId1", REL_OFFICE, "word/document.xml")]);
    let document_rels = relationships(&[("rId3", REL_HEADER, "header1.xml")]);
    let document = document_xml(&format!(
        "{paragraphs}{}",
        a4_sect(r#"<w:headerReference w:type="default" r:id="rId3"/>"#)
    ));
    let owned = [types, package_rels, document_rels, document, header];
    let bytes = store_zip(&[
        ("[Content_Types].xml", owned[0].as_bytes()),
        ("_rels/.rels", owned[1].as_bytes()),
        ("word/_rels/document.xml.rels", owned[2].as_bytes()),
        ("word/document.xml", owned[3].as_bytes()),
        ("word/header1.xml", owned[4].as_bytes()),
    ]);
    let (dir, mut office) = open_package(&bytes, "t202");
    assert!(header_story_present(&office));
    let opening = section_page(&office);
    let opening_pages = lay(&office, ViewOptions::default());
    let opening_count = opening_pages.pages().len();
    assert!(opening_count >= 2, "opening pages {opening_count}");
    let opening_numbers = header_page_numbers(&opening_pages);
    assert_eq!(
        opening_numbers,
        (1..=opening_count as u32).collect::<Vec<_>>()
    );

    cmd(
        &mut office,
        PAGE_MARGINS,
        &[
            ("top", ArgValue::Text("4000".into())),
            ("right", ArgValue::Text("1800".into())),
            ("bottom", ArgValue::Text("4000".into())),
            ("left", ArgValue::Text("1800".into())),
        ],
    );
    assert_eq!(
        section_page(&office).top,
        joyword_model::Measure::Twips(4000)
    );
    let narrow_count = lay(&office, ViewOptions::default()).pages().len();
    assert!(
        narrow_count > opening_count,
        "margins {opening_count} -> {narrow_count}"
    );
    undo(&mut office);
    assert_eq!(section_page(&office).top, opening.top);
    assert_eq!(
        lay(&office, ViewOptions::default()).pages().len(),
        opening_count
    );
    redo(&mut office);

    cmd(
        &mut office,
        PAGE_ORIENTATION,
        &[("orientation", ArgValue::Text("landscape".into()))],
    );
    let landscape = section_page(&office);
    assert_eq!(landscape.orientation, Orientation::Landscape);
    assert_eq!(landscape.width, opening.height);
    assert_eq!(landscape.height, opening.width);
    let landscape_count = lay(&office, ViewOptions::default()).pages().len();
    assert!(
        landscape_count > narrow_count,
        "landscape {narrow_count} -> {landscape_count}"
    );
    undo(&mut office);
    assert_eq!(section_page(&office).orientation, Orientation::Portrait);
    redo(&mut office);

    caret_at(&mut office, 0);
    cmd(&mut office, PAGE_NUMBER, &[]);
    let laid = lay(&office, ViewOptions::default());
    assert!(
        body_page_text(&laid, 0).contains('1'),
        "body {}",
        body_page_text(&laid, 0)
    );
    let numbers = header_page_numbers(&laid);
    assert_eq!(numbers, (1..=laid.pages().len() as u32).collect::<Vec<_>>());
    assert_eq!(laid.pages().len(), landscape_count);
    undo(&mut office);
    assert!(!body_page_text(&lay(&office, ViewOptions::default()), 0).contains('1'));
    redo(&mut office);
    assert!(body_page_text(&lay(&office, ViewOptions::default()), 0).contains('1'));
    office
        .active()
        .unwrap()
        .backend()
        .session()
        .export_plan()
        .expect("a PAGE field inserted into a body paragraph exports");

    let again = lay(&office, ViewOptions::default());
    let marks = lay(
        &office,
        ViewOptions {
            formatting_marks: true,
            ..ViewOptions::default()
        },
    );
    assert_eq!(again.pages().len(), marks.pages().len());
    assert_eq!(
        again
            .lines()
            .iter()
            .map(|line| line.page)
            .collect::<Vec<_>>(),
        marks
            .lines()
            .iter()
            .map(|line| line.page)
            .collect::<Vec<_>>()
    );
    assert_ne!(again.key().view, marks.key().view);
    let width = again.pages()[0].width;
    let half = to_screen(width, ScreenScale::new(0.5).unwrap());
    let full = to_screen(width, ScreenScale::new(1.0).unwrap());
    let twice = to_screen(width, ScreenScale::new(2.0).unwrap());
    assert!((full - half * 2.0).abs() < 0.01);
    assert!((twice - full * 2.0).abs() < 0.01);
    assert_eq!(again.pages().len(), landscape_count);

    save_and_reopen(&mut office, &dir, "t202-copy.docx");
    assert!(header_story_present(&office));
    assert_eq!(section_page(&office).orientation, Orientation::Landscape);
    assert_eq!(
        section_page(&office).top,
        joyword_model::Measure::Twips(4000)
    );
    let reopened = lay(&office, ViewOptions::default());
    assert_eq!(reopened.pages().len(), landscape_count);
    assert_eq!(
        header_page_numbers(&reopened),
        (1..=landscape_count as u32).collect::<Vec<_>>()
    );
    let first = active_document(&office)
        .body()
        .blocks
        .iter()
        .find_map(joyword_model::Block::as_paragraph)
        .unwrap();
    assert!(
        matches!(
            first.inlines(),
            [joyword_model::InlineObject {
                kind: joyword_model::InlineKind::PageField(joyword_model::PageField {
                    kind: joyword_model::PageFieldKind::Page,
                    cached: None,
                }),
                ..
            }]
        ),
        "{:?}",
        first.inlines()
    );
    assert_eq!(first.plain_text(|_| "#".into()), format!("#{line}"));
    assert!(
        body_page_text(&reopened, 0).starts_with('1'),
        "body {}",
        body_page_text(&reopened, 0)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Automated rehearsal; does not replace 人工验收.
///
/// T209: keyboard typing coalesces, and a deletion run, a paste, a command
/// insert and two marked-text commits are each their own undo step.
/// Drag and autocorrect are not implemented, so they are not rehearsable.
#[test]
fn t209_keyboard_paste_command_and_ime_undo_one_step_at_a_time() {
    let (dir, mut office) = open_package(
        &plain_package(r#"<w:p><w:r><w:t>Note</w:t></w:r></w:p>"#),
        "t209",
    );
    let end = office.active().unwrap().view().index().len_utf16();
    caret_at(&mut office, end);
    for ch in ['a', 'b'] {
        office
            .active_mut()
            .unwrap()
            .replace(None, &ch.to_string())
            .unwrap();
    }
    assert_eq!(story_text(&office), "Noteab");
    assert_eq!(
        office
            .active()
            .unwrap()
            .backend()
            .session()
            .session()
            .undo_depth(),
        1
    );
    office
        .active_mut()
        .unwrap()
        .delete(Direction::Backward)
        .unwrap();
    office
        .active_mut()
        .unwrap()
        .delete(Direction::Backward)
        .unwrap();
    assert_eq!(story_text(&office), "Note");
    office
        .active_mut()
        .unwrap()
        .replace_from(InputSource::Paste, None, "P")
        .unwrap();
    office
        .active_mut()
        .unwrap()
        .replace_from(InputSource::Command, None, "C")
        .unwrap();
    office.active_mut().unwrap().mark(None, "fa", None).unwrap();
    office.active_mut().unwrap().unmark().unwrap();
    office.active_mut().unwrap().mark(None, "ii", None).unwrap();
    office.active_mut().unwrap().replace(None, "i").unwrap();
    assert_eq!(story_text(&office), "NotePCfai");
    assert_eq!(
        office
            .active()
            .unwrap()
            .backend()
            .session()
            .session()
            .undo_depth(),
        6
    );

    let mut seen = Vec::new();
    while office.active().unwrap().can_undo() {
        undo(&mut office);
        seen.push(story_text(&office));
    }
    assert_eq!(
        seen,
        vec![
            "NotePCfa".to_owned(),
            "NotePC".to_owned(),
            "NoteP".to_owned(),
            "Note".to_owned(),
            "Noteab".to_owned(),
            "Note".to_owned(),
        ]
    );
    redo_all(&mut office);
    assert_eq!(story_text(&office), "NotePCfai");
    save_and_reopen(&mut office, &dir, "t209-copy.docx");
    assert_eq!(story_text(&office), "NotePCfai");
    let _ = std::fs::remove_dir_all(&dir);
}

fn run_kinds(office: &Office, header: bool) -> Vec<RevisionKind> {
    let document = active_document(office);
    let paragraphs: Vec<_> = if header {
        let story = document.sections()[0].headers[0].1;
        document.story(story).unwrap().blocks.iter().collect()
    } else {
        document.body().blocks.iter().collect()
    };
    paragraphs
        .into_iter()
        .filter_map(Block::as_paragraph)
        .flat_map(|paragraph| paragraph.runs().to_vec())
        .filter_map(|run| run.revision.map(|mark| mark.kind))
        .collect()
}

/// Automated rehearsal; does not replace 人工验收.
///
/// T300, JoyWord side only: track, type and delete in the body and the
/// header, accept and reject one change and the whole set, project the three
/// review views, undo, then keep `w:ins` / `w:del` / move pairs across export.
#[test]
fn t300_tracked_edits_views_and_revision_round_trip() {
    let body = r#"<w:p><w:moveFromRangeStart w:id="1" w:author="Ada" w:name="shift" w:date="2026-04-01T00:00:00Z"/><w:moveFrom w:id="2" w:author="Ada" w:date="2026-04-01T00:00:00Z"><w:r><w:delText>A</w:delText></w:r></w:moveFrom><w:moveFromRangeEnd w:id="3" w:author="Ada" w:name="shift" w:date="2026-04-01T00:00:00Z"/><w:moveToRangeStart w:id="4" w:author="Ada" w:name="shift" w:date="2026-04-01T00:00:00Z"/><w:moveTo w:id="5" w:author="Ada" w:date="2026-04-01T00:00:00Z"><w:r><w:t>V</w:t></w:r></w:moveTo><w:moveToRangeEnd w:id="6" w:author="Ada" w:name="shift" w:date="2026-04-01T00:00:00Z"/><w:r><w:t>i</w:t></w:r></w:p>"#;
    let header = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:hdr xmlns:w="{W_NS}"><w:p><w:r><w:t>i</w:t></w:r></w:p></w:hdr>"#
    );
    let settings = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="{W_NS}"></w:settings>"#
    );
    let types = content_types(&[
        ("/word/document.xml", CT_DOCUMENT),
        ("/word/header1.xml", CT_HEADER),
        ("/word/settings.xml", CT_SETTINGS),
    ]);
    let package_rels = relationships(&[("rId1", REL_OFFICE, "word/document.xml")]);
    let document_rels = relationships(&[
        ("rId2", REL_SETTINGS, "settings.xml"),
        ("rId3", REL_HEADER, "header1.xml"),
    ]);
    let document = document_xml(&format!(
        "{body}{}",
        a4_sect(r#"<w:headerReference w:type="default" r:id="rId3"/>"#)
    ));
    let owned = [
        types,
        package_rels,
        document_rels,
        document,
        header,
        settings,
    ];
    let bytes = store_zip(&[
        ("[Content_Types].xml", owned[0].as_bytes()),
        ("_rels/.rels", owned[1].as_bytes()),
        ("word/_rels/document.xml.rels", owned[2].as_bytes()),
        ("word/document.xml", owned[3].as_bytes()),
        ("word/header1.xml", owned[4].as_bytes()),
        ("word/settings.xml", owned[5].as_bytes()),
    ]);
    let (dir, mut office) = open_package(&bytes, "t300");
    assert_eq!(story_text(&office), "AVi");
    assert_eq!(active_document(&office).move_markers().len(), 4);
    assert!(header_story_present(&office));

    cmd(
        &mut office,
        REVIEW_TRACK,
        &[("enabled", ArgValue::Bool(true))],
    );
    assert!(office.active().unwrap().tracking_enabled());
    let end = office.active().unwrap().view().index().len_utf16();
    caret_at(&mut office, end);
    office.active_mut().unwrap().replace(None, "f").unwrap();
    caret_at(&mut office, 2);
    office
        .active_mut()
        .unwrap()
        .delete(Direction::Forward)
        .unwrap();
    assert_eq!(story_text(&office), "AVif");

    cmd(&mut office, PAGE_EDIT_HEADER, &[]);
    assert!(!office.active().unwrap().is_editing_body());
    let header_end = office.active().unwrap().view().index().len_utf16();
    caret_at(&mut office, header_end);
    office.active_mut().unwrap().replace(None, "f").unwrap();
    caret_at(&mut office, 0);
    office
        .active_mut()
        .unwrap()
        .delete(Direction::Forward)
        .unwrap();
    cmd(&mut office, PAGE_EDIT_BODY, &[]);
    assert!(office.active().unwrap().is_editing_body());

    let body_kinds = run_kinds(&office, false);
    let header_kinds = run_kinds(&office, true);
    assert!(body_kinds.contains(&RevisionKind::Insert), "{body_kinds:?}");
    assert!(body_kinds.contains(&RevisionKind::Delete), "{body_kinds:?}");
    assert!(
        body_kinds.contains(&RevisionKind::MoveFrom),
        "{body_kinds:?}"
    );
    assert!(body_kinds.contains(&RevisionKind::MoveTo), "{body_kinds:?}");
    assert!(
        header_kinds.contains(&RevisionKind::Insert),
        "{header_kinds:?}"
    );
    assert!(
        header_kinds.contains(&RevisionKind::Delete),
        "{header_kinds:?}"
    );

    for (id, view) in [
        (REVIEW_MARKUP, RevisionView::FinalWithMarkup),
        (REVIEW_FINAL, RevisionView::Final),
        (REVIEW_ORIGINAL, RevisionView::Original),
    ] {
        cmd(&mut office, id, &[]);
        assert_eq!(office.review_view(), view);
    }
    let markup = scene_text(&lay_review(&office, RevisionView::FinalWithMarkup));
    let final_text = scene_text(&lay_review(&office, RevisionView::Final));
    let original = scene_text(&lay_review(&office, RevisionView::Original));
    for letter in ['A', 'V', 'i', 'f'] {
        assert!(markup.contains(letter), "{letter} missing from {markup}");
    }
    assert!(
        final_text.contains('V') && final_text.contains('f'),
        "{final_text}"
    );
    assert!(!final_text.contains('A'), "{final_text}");
    assert!(
        original.contains('A') && original.contains('i'),
        "{original}"
    );
    assert!(
        !original.contains('V') && !original.contains('f'),
        "{original}"
    );
    cmd(&mut office, REVIEW_MARKUP, &[]);

    let end = office.active().unwrap().view().index().len_utf16();
    caret_at(&mut office, end);
    cmd(&mut office, REVIEW_ACCEPT, &[]);
    assert!(!run_kinds(&office, false).contains(&RevisionKind::Insert));
    undo(&mut office);
    assert!(run_kinds(&office, false).contains(&RevisionKind::Insert));

    caret_at(&mut office, 3);
    cmd(&mut office, REVIEW_REJECT, &[]);
    assert!(!run_kinds(&office, false).contains(&RevisionKind::Delete));
    undo(&mut office);
    assert!(run_kinds(&office, false).contains(&RevisionKind::Delete));

    cmd(&mut office, REVIEW_ACCEPT_ALL, &[]);
    assert!(run_kinds(&office, false).is_empty());
    assert!(active_document(&office).move_markers().is_empty());
    undo(&mut office);
    assert_eq!(active_document(&office).move_markers().len(), 4);
    assert!(run_kinds(&office, false).contains(&RevisionKind::MoveFrom));

    cmd(&mut office, REVIEW_REJECT_ALL, &[]);
    assert!(run_kinds(&office, false).is_empty());
    undo(&mut office);
    assert_eq!(run_kinds(&office, false), body_kinds);
    assert_eq!(run_kinds(&office, true), header_kinds);

    let plan = office
        .active()
        .unwrap()
        .backend()
        .session()
        .export_plan()
        .unwrap();
    let xml = String::from_utf8_lossy(plan.bytes());
    assert!(xml.contains("w:ins"), "export dropped insertions");
    assert!(xml.contains("w:del"), "export dropped deletions");
    assert!(xml.contains("w:moveFrom"), "export dropped moveFrom");
    assert!(xml.contains("w:moveTo"), "export dropped moveTo");

    save_and_reopen(&mut office, &dir, "t300-copy.docx");
    assert_eq!(run_kinds(&office, false), body_kinds);
    assert_eq!(run_kinds(&office, true), header_kinds);
    assert_eq!(active_document(&office).move_markers().len(), 4);
    assert_eq!(story_text(&office), "AVif");
    let _ = std::fs::remove_dir_all(&dir);
}

/// GUI smoke after #78: typing into a Chinese paragraph and pressing Ctrl+J
/// replaced the page with "inter-character justification" unsupported.
#[test]
fn justify_on_mixed_script_paragraphs_keeps_the_document_displayable() {
    let sample: &[u8] = include_bytes!("../../../tests/fixtures/docx/DOCX-SAMPLE-001.docx");
    let mixed = plain_package(
        "<w:p><w:r><w:t>甲方：青禾工作室；双方约定于 2026 年 10 月 13 日提交报告，金额为人民币 12,345.67 元。</w:t></w:r></w:p><w:p><w:r><w:t xml:space=\"preserve\">交付内容包括：正文、表格、图片以及 English summary and appendix notes。</w:t></w:r></w:p>",
    );
    for (label, bytes) in [
        ("justify-sample", sample),
        ("justify-mixed", mixed.as_slice()),
    ] {
        let (dir, mut office) = open_package(bytes, label);
        caret_at(&mut office, 0);
        office.active_mut().unwrap().replace(None, "x").unwrap();
        cmd(&mut office, EDIT_JUSTIFY, &[]);
        let first = active_document(&office)
            .body()
            .blocks
            .iter()
            .find_map(Block::as_paragraph)
            .unwrap();
        assert_eq!(first.props.justification, Some(Justification::Both));
        assert!(first.plain_text(|_| String::new()).starts_with('x'));
        let typed = lay(&office, ViewOptions::default());
        assert!(scene_text(&typed).starts_with('x'), "{label}");

        let end = office.active().unwrap().view().index().len_utf16();
        office.active_mut().unwrap().select(0..end, false).unwrap();
        cmd(&mut office, EDIT_JUSTIFY, &[]);
        let all = lay(&office, ViewOptions::default());
        assert_eq!(scene_text(&all), scene_text(&typed), "{label}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// GUI smoke after #78: `joyword sample.docx` titled the window and the close
/// prompt "未命名 1", because the argument was never opened.
#[test]
fn a_document_named_on_the_command_line_names_the_window_and_the_close_prompt() {
    use super::input_ui::{DocEditor, close_prompt_message};
    use super::{document_name, startup_document, window_title};
    use joyword_shell::l10n::Locale;
    use std::ffi::OsString;

    let dir = scratch("cli-title");
    let sample = dir.join("sample.docx");
    std::fs::write(
        &sample,
        include_bytes!("../../../tests/fixtures/reference/word2024/interop/word_edited.docx"),
    )
    .unwrap();
    let args = |items: &[&str]| -> Vec<OsString> { items.iter().map(OsString::from).collect() };
    let shown = sample.to_str().unwrap();
    assert_eq!(startup_document(args(&[])), None);
    assert_eq!(startup_document(args(&["--verbose"])), None);
    assert_eq!(
        startup_document(args(&["--verbose", shown, "other.docx"])),
        Some(sample.clone())
    );
    assert_eq!(
        startup_document(args(&["--", "-dash.docx"])),
        Some(PathBuf::from("-dash.docx"))
    );

    // The window's open for a chosen path.
    let path = startup_document(args(&[shown])).unwrap();
    let mut office = Office::new();
    let generation = office.begin_open();
    let bound = joyword_shell::docx::BoundDocx::open_default(&path).unwrap();
    office.accept_bound(generation, bound).unwrap();
    let mut editor: Box<dyn DocEditor> = Box::new(office.hold_active().unwrap());
    let name = document_name(Some(editor.as_ref()), Locale::ZhCn);
    assert_eq!(name.as_deref(), Some("sample.docx"));
    assert_eq!(
        window_title(name.as_deref(), false),
        "sample.docx - JoyWord"
    );
    editor.replace(None, "x").unwrap();
    assert!(editor.has_unsaved_changes());
    assert_eq!(
        window_title(name.as_deref(), true),
        "*sample.docx - JoyWord"
    );
    assert_eq!(
        close_prompt_message(Locale::ZhCn, "close.unsaved", name.as_deref()),
        "“sample.docx”有尚未保存的更改。"
    );
    assert_eq!(
        close_prompt_message(Locale::En, "close.dirty_after_save", name.as_deref()),
        "“sample.docx” changed after saving started. Those edits are not in this save."
    );

    // Only an editor never bound to a file is untitled.
    let startup_sample = joyword_shell::docx::DocxSession::open_default(include_bytes!(
        "../../../tests/fixtures/docx/DOCX-SAMPLE-001.docx"
    ))
    .unwrap();
    let untitled: Box<dyn DocEditor> = Box::new(
        joyword_shell::input::TextInput::new(
            startup_sample,
            joyword_shell::input::InputLimits::default(),
        )
        .unwrap(),
    );
    assert_eq!(
        document_name(Some(untitled.as_ref()), Locale::ZhCn).as_deref(),
        Some("未命名 1")
    );
    assert_eq!(document_name(None, Locale::ZhCn), None);
    drop(editor);
    let _ = std::fs::remove_dir_all(&dir);
}

/// GUI smoke after #78: Esc in the Font panel reported "no composition"
/// because Esc was bound to `CancelComposition` whether or not an IME
/// composition was open, and bindings run before the panel's key handlers.
#[test]
fn escape_is_bound_only_while_composing_so_panels_receive_it_otherwise() {
    use gpui::{KeyContext, Keymap, Keystroke};

    let keymap = Keymap::new(super::key_bindings(
        &joyword_shell::command::Registry::builtin(),
    ));
    let escape = [Keystroke::parse("escape").unwrap()];
    for idle in ["JoyWordInput", "JoyWordInput composing"] {
        let context = [KeyContext::parse(idle).unwrap()];
        let (bindings, pending) = keymap.bindings_for_input(&escape, &context);
        assert!(!pending, "{idle}");
        let cancels = bindings
            .iter()
            .any(|binding| binding.action().partial_eq(&super::CancelComposition));
        assert_eq!(cancels, idle.ends_with("composing"), "{idle}");
        if !cancels {
            assert!(bindings.is_empty(), "{idle}: Esc must stay unbound");
        }
    }
}

/// GUI retest after #79: Backspace, Delete, arrows, Home, End and the editing
/// chords in the Save As path field (and the other in-app fields) were bound
/// on the ancestor `JoyWordInput` context, so they edited the document.
#[test]
fn editing_keys_are_unbound_while_an_in_app_text_field_is_focused() {
    use gpui::{KeyContext, Keymap, Keystroke};
    use joyword_shell::command::{
        CommandId, EDIT_BACKSPACE, EDIT_BOLD, EDIT_COPY, EDIT_CUT, EDIT_DELETE,
        EDIT_DELETE_WORD_BACKWARD, EDIT_DOWN, EDIT_END, EDIT_ENTER, EDIT_EXTEND_DOCUMENT_END,
        EDIT_EXTEND_END, EDIT_EXTEND_LEFT, EDIT_HOME, EDIT_LEFT, EDIT_PASTE, EDIT_RIGHT,
        EDIT_SELECT_ALL, EDIT_UNDO, EDIT_UP, EDIT_WORD_LEFT, FILE_SAVE, Registry,
    };

    let registry = Registry::builtin();
    let keymap = Keymap::new(super::key_bindings(&registry));
    let chords = super::shortcut_chords(&registry);
    let document = [KeyContext::parse("JoyWordInput").unwrap()];
    let field = [
        KeyContext::parse("JoyWordInput").unwrap(),
        KeyContext::parse(super::field_edit::FIELD_CONTEXT).unwrap(),
    ];
    // Platform chords, not a Linux key list: macOS uses Command/Option where
    // the other platforms use Ctrl and Home/End.
    let editing = [
        EDIT_BACKSPACE,
        EDIT_DELETE,
        EDIT_LEFT,
        EDIT_RIGHT,
        EDIT_UP,
        EDIT_DOWN,
        EDIT_HOME,
        EDIT_END,
        EDIT_EXTEND_END,
        EDIT_EXTEND_LEFT,
        EDIT_SELECT_ALL,
        EDIT_WORD_LEFT,
        EDIT_EXTEND_DOCUMENT_END,
        EDIT_DELETE_WORD_BACKWARD,
        EDIT_COPY,
        EDIT_CUT,
        EDIT_PASTE,
        EDIT_UNDO,
        EDIT_BOLD,
        EDIT_ENTER,
    ];
    let of = |id: CommandId| -> Vec<&str> {
        chords
            .iter()
            .filter(|(_, command)| *command == id)
            .map(|(keys, _)| *keys)
            .collect()
    };
    for id in editing {
        let specs = of(id);
        assert!(!specs.is_empty(), "{id} has no chord");
        for spec in specs {
            let key = [Keystroke::parse(spec).unwrap()];
            let (in_document, _) = keymap.bindings_for_input(&key, &document);
            let (in_field, pending) = keymap.bindings_for_input(&key, &field);
            assert!(
                !in_document.is_empty(),
                "{spec} should still edit the document"
            );
            assert!(!pending, "{spec}");
            assert!(
                in_field.is_empty(),
                "{spec} must not reach the document while a field is focused: {in_field:?}"
            );
        }
    }
    // File chords stay global, so Save still works from the path field.
    for spec in of(FILE_SAVE) {
        let save = [Keystroke::parse(spec).unwrap()];
        let (in_field, _) = keymap.bindings_for_input(&save, &field);
        assert!(
            in_field
                .iter()
                .any(|binding| binding.action().partial_eq(&super::SaveOriginal)),
            "{spec} stays a file command"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn prompt_keys_answer_cancel_on_escape_and_ok_on_enter_only() {
    use super::prompt_ui::key_answer;
    use gpui::PromptButton;

    let close = [PromptButton::cancel("取消"), PromptButton::new("不保存")];
    assert_eq!(key_answer(&close, "escape"), Some(0));
    assert_eq!(key_answer(&close, "enter"), None, "Enter never discards");
    let original = [
        PromptButton::cancel("取消"),
        PromptButton::ok("另存为"),
        PromptButton::new("确认"),
    ];
    assert_eq!(key_answer(&original, "escape"), Some(0));
    assert_eq!(key_answer(&original, "enter"), Some(1));
    assert_eq!(key_answer(&original, "a"), None);
}

/// GUI retest after #79: `joyword word_edited.docx` showed the layout error
/// "preserved settings" instead of the document, because the ordinary Word
/// settings and document-default run properties were refused.
#[test]
fn a_word_saved_document_opens_lays_out_and_lays_out_again_after_an_edit() {
    let bytes =
        include_bytes!("../../../tests/fixtures/reference/word2024/interop/word_edited.docx");
    let (dir, mut office) = open_package(bytes, "word-edited");
    let opened = lay(&office, ViewOptions::default());
    let text = scene_text(&opened);
    assert!(text.starts_with("校验合作协议（原创测试文本）"), "{text}");
    assert!(
        text.contains("本段由 Word 2024 追加：已完成核对。"),
        "{text}"
    );
    assert_eq!(opened.omitted_blocks(), 0);

    caret_at(&mut office, 0);
    office.active_mut().unwrap().replace(None, "新").unwrap();
    assert!(scene_text(&lay(&office, ViewOptions::default())).starts_with("新校验"));
    save_and_reopen(&mut office, &dir, "word-edited-copy.docx");
    let reopened = lay(&office, ViewOptions::default());
    assert!(scene_text(&reopened).starts_with("新校验合作协议"));
    assert!(active_document(&office).settings().preserved_layout_neutral);
    let _ = std::fs::remove_dir_all(&dir);
}

/// GUI retest after #79: the close prompt's detail wrapped as
/// "…另存为始终写入新副本" / "。", leaving the full stop alone on a line.
/// The wrapper lives in the Linux prompt module, which other platforms do
/// not compile.
#[cfg(target_os = "linux")]
#[test]
fn prompt_lines_never_start_with_closing_punctuation() {
    use super::prompt_ui::wrap_ranges;
    use joyword_layout::cjk::prohibits_line_start;

    // A full-width character is 16 px, anything else 8 px.
    let advance = |ch: char| if ch.is_ascii() { 8.0 } else { 16.0 };
    let pen = |text: &str| {
        let text = text.to_owned();
        move |at: usize| text[..at].chars().map(advance).sum::<f32>()
    };
    let lines = |text: &str, width: f32| -> Vec<String> {
        wrap_ranges(text, width, pen(text))
            .into_iter()
            .map(|range| text[range].trim_end().to_owned())
            .collect()
    };

    let detail = "技术预览 · 保存原件前会列出元数据风险；另存为始终写入新副本。";
    let full: f32 = detail.chars().map(advance).sum();
    // Everything but the final 。 fits on the first line.
    let wrapped = lines(detail, full - 16.0);
    assert_eq!(wrapped.len(), 2, "{wrapped:?}");
    assert_eq!(wrapped[1], "本。");
    assert_eq!(wrapped.concat(), detail);
    assert_eq!(lines(detail, full), vec![detail.to_owned()]);

    // Every width keeps each line start off the closing-punctuation set and
    // loses no character.
    for text in [
        detail,
        "“sample.docx”有尚未保存的更改。",
        "甲方：青禾工作室；乙方：远山设计室。（附件）「完」",
    ] {
        for width in (16..=400).step_by(8) {
            let wrapped = lines(text, width as f32);
            assert!(
                wrapped.iter().skip(1).all(|line| line
                    .chars()
                    .next()
                    .is_some_and(|ch| !prohibits_line_start(ch))),
                "{width}: {wrapped:?}"
            );
            let joined: String = wrap_ranges(text, width as f32, pen(text))
                .into_iter()
                .map(|range| &text[range])
                .collect();
            assert_eq!(joined, text, "{width}");
        }
    }

    // Latin text breaks between words.
    assert_eq!(
        lines("Save changes to sample.docx?", 15.0 * 8.0),
        vec!["Save changes to", "sample.docx?"]
    );
}
