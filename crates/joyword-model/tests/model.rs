//! T100 integration tests: original corpus seeds, structure preservation,
//! bounded invalid-tree diagnostics and snapshot structural sharing.

use std::path::Path;
use std::sync::Arc;

use joyword_model::{
    Anchor, AnnotationKind, Bias, Block, BlockSeq, BreakKind, Cell, CellBorders, Document,
    DumpOptions, IdAllocator, InlineKind, InlineObject, Issue, MAX_DIAGNOSTICS, MAX_NESTING,
    Measure, OpaqueBlock, Paragraph, Position, Preserved, Row, RunProps, ScalarOffset, Sdt,
    SdtLock, SdtProps, Section, Segment, StoryKind, Style, StyleId, StyleKind, Table, Utf16Offset,
    VerticalMerge,
};

fn seed(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/text")
        .join(name);
    std::fs::read_to_string(path).expect("seed registered in tests/fixtures/manifest.json")
}

/// One paragraph per line, as a plain-text import would produce.
fn document_from_lines(text: &str) -> Document {
    let mut document = Document::new_blank();
    let mut blocks = Vec::new();
    for line in text.lines() {
        let mut paragraph = Paragraph::new(document.ids().node());
        paragraph
            .push_text(line, None, Arc::default())
            .expect("seed text contains no reserved code point");
        blocks.push(Block::from(paragraph));
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    document
}

fn body_texts(document: &Document) -> Vec<String> {
    document
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(|paragraph| paragraph.plain_text(|_| String::new()))
        .collect()
}

fn issues(document: &Document) -> Vec<Issue> {
    document
        .validate()
        .iter()
        .map(|d| d.issue.clone())
        .collect()
}

fn paragraph(ids: &mut IdAllocator, text: &str) -> Block {
    let mut paragraph = Paragraph::new(ids.node());
    paragraph.push_text(text, None, Arc::default()).unwrap();
    paragraph.into()
}

#[test]
fn blank_document_is_valid_and_writes_compatibility_mode_15() {
    let document = Document::new_blank();
    let diagnostics = document.validate();
    assert!(
        diagnostics.is_empty(),
        "{:?}",
        diagnostics.iter().collect::<Vec<_>>()
    );
    assert_eq!(document.settings().effective_compatibility_mode(), 15);
    assert_eq!(document.body().blocks.len(), 1);
    // Existing documents without the setting are interpreted as mode 12.
    let mut imported = Document::new_blank();
    imported.settings_mut().compatibility_mode = None;
    assert_eq!(imported.settings().effective_compatibility_mode(), 12);
}

#[test]
fn corpus_seeds_keep_every_code_point() {
    for name in [
        "cjk-contract.txt",
        "unicode.txt",
        "review.txt",
        "agent-untrusted.txt",
    ] {
        let text = seed(name);
        let document = document_from_lines(&text);
        assert!(!document.validate().has_errors(), "{name}");
        let expected: Vec<&str> = text.lines().collect();
        assert_eq!(body_texts(&document), expected, "{name}");
    }
}

#[test]
fn unicode_seed_offsets_round_trip_through_utf16() {
    let document = document_from_lines(&seed("unicode.txt"));
    for block in document.body().blocks.iter() {
        let text = block.as_paragraph().unwrap().text();
        for scalar in 0..=text.len_scalars() {
            let utf16 = text.scalar_to_utf16(ScalarOffset(scalar)).unwrap();
            assert_eq!(text.utf16_to_scalar(utf16), Ok(ScalarOffset(scalar)));
        }
        let total = text.summary().utf16;
        let mut rejected = 0;
        for unit in 0..=total {
            rejected += usize::from(text.utf16_to_scalar(Utf16Offset(unit)).is_err());
        }
        // Each supplementary-plane scalar has exactly one interior position.
        assert_eq!(rejected, total - text.len_scalars());
    }
    // Decomposed é (e + U+0301) and precomposed é stay distinct.
    let first = body_texts(&document).remove(0);
    assert!(first.contains("e\u{301}") && first.contains('\u{e9}'));
}

#[test]
fn contract_structure_preserves_unsupported_nodes() {
    let mut document = Document::new_blank();
    let ids = document.ids();
    let row = |ids: &mut IdAllocator, left: &str, right: &str| Row {
        id: ids.node(),
        grid_before: 0,
        grid_after: 0,
        cells: vec![
            Cell {
                id: ids.node(),
                grid_span: 1,
                vertical_merge: VerticalMerge::None,
                width: Some(Measure::Twips(2_000)),
                borders: CellBorders::default(),
                shading: None,
                margin: None,
                vertical_align: None,
                blocks: BlockSeq::from_vec(vec![paragraph(ids, left)]),
                preserved: None,
            },
            Cell {
                id: ids.node(),
                grid_span: 1,
                vertical_merge: VerticalMerge::None,
                width: Some(Measure::Twips(3_000)),
                borders: CellBorders::default(),
                shading: None,
                margin: None,
                vertical_align: None,
                blocks: BlockSeq::from_vec(vec![paragraph(ids, right)]),
                preserved: None,
            },
        ],
        header: false,
        cant_split: false,
        height: None,
        preserved: None,
    };
    let table = Table {
        id: ids.node(),
        grid: vec![Measure::Twips(2_000), Measure::Twips(3_000)],
        rows: vec![row(ids, "项目", "金额"), row(ids, "设计费", "12,345.67")],
        style: None,
        width: None,
        layout: None,
        alignment: None,
        indent: None,
        look: None,
        cell_margin: None,
        preserved: None,
        source: None,
    };
    let control = Sdt {
        id: ids.node(),
        props: SdtProps {
            tag: Some("party-a".into()),
            alias: Some("甲方".into()),
            lock: SdtLock::SdtLocked,
            preserved: None,
        },
        blocks: BlockSeq::from_vec(vec![paragraph(ids, "青禾工作室")]),
        source: None,
    };
    let equation = OpaqueBlock {
        id: ids.node(),
        preserved: Preserved {
            element: "m:oMathPara".into(),
            handle: ids.preserved(),
        },
        source: None,
    };
    let mut signed = Paragraph::new(ids.node());
    signed.push_text("签字：", None, Arc::default()).unwrap();
    signed.push_inline(
        InlineObject {
            id: ids.node(),
            kind: InlineKind::Preserved(Preserved {
                element: "w:object".into(),
                handle: ids.preserved(),
            }),
            source: None,
        },
        None,
        Arc::default(),
    );
    signed.push_inline(
        InlineObject {
            id: ids.node(),
            kind: InlineKind::Break(BreakKind::Line),
            source: None,
        },
        None,
        Arc::default(),
    );
    let blocks = vec![
        paragraph(ids, "合作协议"),
        table.into(),
        control.into(),
        equation.into(),
        signed.into(),
    ];
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);

    let diagnostics = document.validate();
    assert!(
        diagnostics.is_empty(),
        "{:?}",
        diagnostics.iter().collect::<Vec<_>>()
    );
    let dump = document.canonical_dump(DumpOptions { node_ids: false });
    for expected in [
        "table style=- grid=[2000tw,3000tw]",
        "cell span=1 vmerge=None width=3000tw",
        "\"12,345.67\"",
        "sdt tag=Some(\"party-a\") alias=Some(\"甲方\") lock=SdtLocked",
        "opaque element=\"m:oMathPara\"",
        "<preserved:element=\"w:object\"",
        "<break:Line>",
    ] {
        assert!(dump.contains(expected), "missing {expected:?} in\n{dump}");
    }
    assert_eq!(
        dump,
        document.canonical_dump(DumpOptions { node_ids: false })
    );
    // Consumers see typed segments, never the internal object marker.
    let last = document
        .body()
        .blocks
        .last()
        .unwrap()
        .as_paragraph()
        .unwrap();
    assert!(matches!(last.segments()[1], Segment::Inline(_)));
    assert!(
        !last
            .plain_text(|_| "□".into())
            .contains(joyword_model::OBJECT_MARKER)
    );
}

#[test]
fn canonical_dump_without_ids_ignores_identifier_allocation() {
    let text = seed("cjk-contract.txt");
    let a = document_from_lines(&text);
    let mut b = Document::new_blank();
    for _ in 0..50 {
        b.ids().node(); // shift every identifier
    }
    let mut blocks = Vec::new();
    for line in text.lines() {
        blocks.push(paragraph(b.ids(), line));
    }
    let body = b.body_id();
    b.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    let options = DumpOptions { node_ids: false };
    assert_eq!(a.canonical_dump(options), b.canonical_dump(options));
    assert_ne!(
        a.canonical_dump(DumpOptions { node_ids: true }),
        b.canonical_dump(DumpOptions { node_ids: true })
    );
}

#[test]
fn snapshots_share_structure_and_stay_immutable() {
    let lines: String = (0..10_000)
        .map(|i| format!("第{i}段 paragraph {i}\n"))
        .collect();
    let mut document = document_from_lines(&lines);
    let before = document.snapshot();
    let body = document.body_id();
    document
        .story_mut(body)
        .unwrap()
        .blocks
        .update(5_000, |block| {
            let Block::Paragraph(paragraph) = block else {
                panic!("expected paragraph")
            };
            Arc::make_mut(paragraph)
                .insert_text(ScalarOffset(0), "改：")
                .unwrap();
        });
    document.advance_version();
    let after = document.snapshot();
    assert!(after.version() > before.version());
    let copied = after.body().blocks.unshared_nodes(&before.body().blocks);
    assert!(copied <= 5, "copied {copied} block-tree nodes for one edit");
    let text = |snapshot: &Document| {
        snapshot
            .body()
            .blocks
            .get(5_000)
            .unwrap()
            .as_paragraph()
            .unwrap()
            .plain_text(|_| String::new())
    };
    assert_eq!(text(&before), "第5000段 paragraph 5000");
    assert_eq!(text(&after), "改：第5000段 paragraph 5000");
    // Untouched paragraphs are the same allocation in both versions.
    let (Block::Paragraph(a), Block::Paragraph(b)) = (
        before.body().blocks.get(4_999).unwrap(),
        after.body().blocks.get(4_999).unwrap(),
    ) else {
        panic!("expected paragraphs")
    };
    assert!(Arc::ptr_eq(a, b));
}

#[test]
fn invalid_trees_report_bounded_errors() {
    let mut document = Document::new_blank();
    let mut foreign = IdAllocator::default();
    for _ in 0..1_000 {
        foreign.node(); // identifiers this document never allocated
    }
    let reused = document.body().blocks.first().unwrap().clone();
    let body = document.body_id();
    let ids = document.ids();
    let cell = |id, merge, blocks: Vec<Block>| Cell {
        id,
        grid_span: 1,
        vertical_merge: merge,
        width: None,
        borders: CellBorders::default(),
        shading: None,
        margin: None,
        vertical_align: None,
        blocks: BlockSeq::from_vec(blocks),
        preserved: None,
    };
    let table = Table {
        id: ids.node(),
        grid: vec![Measure::Twips(1_000); 3],
        rows: vec![
            Row {
                id: ids.node(),
                grid_before: 0,
                grid_after: 0,
                cells: vec![cell(ids.node(), VerticalMerge::None, vec![])],
                header: false,
                cant_split: false,
                height: None,
                preserved: None,
            },
            Row {
                id: ids.node(),
                grid_before: 1,
                grid_after: 1,
                cells: vec![cell(
                    ids.node(),
                    VerticalMerge::Continue,
                    vec![paragraph(ids, "x")],
                )],
                header: false,
                cant_split: false,
                height: None,
                preserved: None,
            },
        ],
        style: Some(StyleId::new("Normal")),
        width: None,
        layout: None,
        alignment: None,
        indent: None,
        look: None,
        cell_margin: None,
        preserved: None,
        source: None,
    };
    let unissued = paragraph(&mut foreign, "from another document");
    document.story_mut(body).unwrap().blocks =
        BlockSeq::from_vec(vec![reused.clone(), reused, table.into()]);
    document.insert_story(StoryKind::Header, vec![unissued]);
    let found = issues(&document);
    let expect = |predicate: &dyn Fn(&Issue) -> bool, what: &str| {
        assert!(found.iter().any(predicate), "missing {what}: {found:#?}");
    };
    expect(&|i| matches!(i, Issue::DuplicateId { .. }), "duplicate id");
    expect(&|i| matches!(i, Issue::UnissuedId { .. }), "unissued id");
    expect(&|i| matches!(i, Issue::EmptyCell { .. }), "empty cell");
    expect(
        &|i| {
            matches!(
                i,
                Issue::TableGridMismatch {
                    used: 1,
                    grid: 3,
                    ..
                }
            )
        },
        "grid",
    );
    expect(
        &|i| matches!(i, Issue::OrphanVerticalMerge { .. }),
        "vertical merge",
    );
    expect(
        &|i| matches!(i, Issue::StyleKindMismatch { .. }),
        "table style kind",
    );
    assert!(document.validate().has_errors());
}

#[test]
fn style_cycles_and_missing_references_are_reported() {
    let mut document = Document::new_blank();
    for (id, base) in [("x", "y"), ("y", "x")] {
        let mut style = Style::new(id, StyleKind::Paragraph, None);
        style.based_on = Some(StyleId::new(base));
        document.styles_mut().insert(style).unwrap();
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Paragraph(paragraph) = block else {
            unreachable!()
        };
        Arc::make_mut(paragraph).style = Some(StyleId::new("gone"));
    });
    let diagnostics = document.validate();
    let errors: Vec<_> = diagnostics.errors().cloned().collect();
    assert_eq!(errors.len(), 1, "one cycle reported once: {errors:?}");
    assert!(matches!(errors[0], Issue::StyleChain { .. }));
    assert!(diagnostics.iter().any(|d| d.issue
        == Issue::MissingStyle {
            style: StyleId::new("gone")
        }));
}

#[test]
fn annotation_ranges_are_checked_against_story_text() {
    let mut document = document_from_lines(&seed("review.txt"));
    let body = document.body_id();
    let paragraphs: Vec<_> = document.body().blocks.iter().map(Block::id).collect();
    let at = |paragraph, offset, bias| Position {
        story: body,
        paragraph,
        offset: ScalarOffset(offset),
        bias,
    };
    let comment_body = {
        let block = paragraph(document.ids(), "请确认付款期限是否需要调整。");
        document.insert_story(StoryKind::Comment, vec![block])
    };
    let good = document.insert_annotation(
        AnnotationKind::Comment { body: comment_body },
        Anchor::Range {
            start: at(paragraphs[0], 7, Bias::After),
            end: at(paragraphs[0], 12, Bias::Before),
        },
    );
    let spanning = document.insert_annotation(
        AnnotationKind::Field {
            separator: Some(at(paragraphs[1], 2, Bias::After)),
        },
        Anchor::Range {
            start: at(paragraphs[0], 0, Bias::After),
            end: at(paragraphs[2], 1, Bias::Before),
        },
    );
    assert!(document.validate().is_empty(), "{:?}", issues(&document));
    let reversed = document.insert_annotation(
        AnnotationKind::Bookmark { name: "r".into() },
        Anchor::Range {
            start: at(paragraphs[1], 0, Bias::After),
            end: at(paragraphs[0], 3, Bias::Before),
        },
    );
    let beyond = document.insert_annotation(
        AnnotationKind::Bookmark { name: "b".into() },
        Anchor::Range {
            start: at(paragraphs[0], 0, Bias::After),
            end: at(paragraphs[0], 500, Bias::Before),
        },
    );
    let detached = document.insert_annotation(
        AnnotationKind::Comment { body: comment_body },
        Anchor::Detached,
    );
    document.annotation_mut(spanning).unwrap().kind = AnnotationKind::Field {
        separator: Some(at(paragraphs[3], 0, Bias::After)),
    };
    let found = issues(&document);
    assert!(found.contains(&Issue::ReversedRange {
        annotation: reversed
    }));
    assert!(found.contains(&Issue::InvalidPosition { annotation: beyond }));
    assert!(found.contains(&Issue::SeparatorOutsideField {
        annotation: spanning
    }));
    assert!(
        !found
            .iter()
            .any(|issue| format!("{issue:?}").contains(&format!("{good:?}")))
    );
    // A detached comment anchor is a visible state, not an error.
    assert!(
        !found
            .iter()
            .any(|issue| format!("{issue:?}").contains(&format!("{detached:?}")))
    );
    let dump = document.canonical_dump(DumpOptions { node_ids: false });
    assert!(dump.contains("annotation comment p0:7>..p0:12<"), "{dump}");
    assert!(dump.contains("annotation comment detached"), "{dump}");
}

#[test]
fn sections_must_end_on_ordered_top_level_blocks() {
    let mut document = document_from_lines("一\n二\n三");
    let blocks: Vec<_> = document.body().blocks.iter().map(Block::id).collect();
    let template = document.sections()[0].clone();
    let header = {
        let block = paragraph(document.ids(), "页眉");
        document.insert_story(StoryKind::Header, vec![block])
    };
    let mut sections = Vec::new();
    for last in [Some(blocks[1]), Some(blocks[0]), None] {
        sections.push(Section {
            id: document.ids().node(),
            last_block: last,
            headers: vec![(joyword_model::HeaderFooterKind::Default, header)],
            footers: vec![(joyword_model::HeaderFooterKind::Default, header)],
            ..template.clone()
        });
    }
    *document.sections_mut() = sections;
    let found = issues(&document);
    let boundary: Vec<_> = found
        .iter()
        .filter(|issue| matches!(issue, Issue::SectionBoundary { .. }))
        .collect();
    assert_eq!(
        boundary.len(),
        1,
        "only the out-of-order section: {found:?}"
    );
    assert!(
        found
            .iter()
            .any(|issue| matches!(issue, Issue::StoryKindMismatch { .. }))
    );
}

#[test]
fn deep_nesting_and_diagnostic_flood_stay_bounded() {
    let mut document = Document::new_blank();
    let ids = document.ids();
    let mut inner: Block = paragraph(ids, "最深处");
    for _ in 0..MAX_NESTING + 10 {
        inner = Sdt {
            id: ids.node(),
            props: SdtProps::default(),
            blocks: BlockSeq::from_vec(vec![inner]),
            source: None,
        }
        .into();
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![inner]);
    let found = issues(&document);
    assert_eq!(
        found,
        vec![Issue::NestingTooDeep {
            node: found_node(&found)
        }]
    );

    let mut flood = Document::new_blank();
    let mut foreign = IdAllocator::default();
    let blocks: Vec<Block> = (0..1_000).map(|_| paragraph(&mut foreign, "x")).collect();
    let body = flood.body_id();
    flood.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    let diagnostics = flood.validate();
    assert_eq!(diagnostics.iter().count(), MAX_DIAGNOSTICS);
    assert!(diagnostics.truncated());
}

fn found_node(found: &[Issue]) -> joyword_model::NodeId {
    match found.first() {
        Some(Issue::NestingTooDeep { node }) => *node,
        other => panic!("expected nesting error, got {other:?}"),
    }
}

#[test]
fn image_references_must_resolve() {
    let mut document = Document::new_blank();
    let resource = document.ids().resource();
    let id = document.ids().node();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Paragraph(paragraph) = block else {
            unreachable!()
        };
        Arc::make_mut(paragraph).push_inline(
            InlineObject {
                id,
                kind: InlineKind::Image {
                    resource,
                    width: Measure::Emu(914_400),
                    height: Measure::Emu(457_200),
                },
                source: None,
            },
            None,
            Arc::new(RunProps::default()),
        );
    });
    assert!(issues(&document).contains(&Issue::MissingResource { node: id, resource }));
    document
        .resources_mut()
        .insert(resource, "image/png", Arc::from(&b"png"[..]), None);
    assert!(document.validate().is_empty());
}

#[test]
fn semantic_difference_ignores_identifiers_and_points_at_the_first_change() {
    let text = seed("cjk-contract.txt");
    let expected = document_from_lines(&text);
    let mut shifted = Document::new_blank();
    for _ in 0..10 {
        shifted.ids().node();
    }
    let blocks: Vec<Block> = text
        .lines()
        .map(|line| paragraph(shifted.ids(), line))
        .collect();
    let body = shifted.body_id();
    shifted.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    assert_eq!(expected.semantic_difference(&shifted), None);

    let changed: String = text.replacen("青禾", "青木", 1);
    let other = document_from_lines(&changed);
    let difference = expected
        .semantic_difference(&other)
        .expect("one paragraph differs");
    assert!(
        difference.expected.contains("青禾") && difference.actual.contains("青木"),
        "{difference:?}"
    );
}
