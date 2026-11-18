use joyword_layout::{DocLayout, LayoutCache, LayoutOptions, layout, layout_shared};
use joyword_model::{
    Document, InlineObject, Paragraph, PartKind, Path, Pos, StoryRef, para::NoteKind, para_block,
    section::SectionStart,
};

/// A two-column chapter, a continuous section on its last page, and real new-page
/// sections on either side. The chapter begins well inside the document.
fn fixture() -> (Document, usize, u32) {
    let mut doc = Document::new();
    doc.body.clear();
    doc.styles.default_chr.font = Some("Source Sans 3".into());
    let mut target = 0;
    for section in 0..70 {
        let mut props = doc.last_section.clone();
        let mut texts = vec![
            format!("Section {section}"),
            "Text after the heading".to_owned(),
        ];
        if section == 34 {
            target = doc.body.len();
            props.columns.count = 2;
            texts.extend((0..8).map(|_| "Alpha beta gamma. ".repeat(6)));
        }
        if section == 35 {
            props.start = SectionStart::Continuous;
        }
        for (i, text) in texts.iter().enumerate() {
            let mut p = Paragraph::with_text(text, Default::default());
            if i + 1 == texts.len() && section != 69 {
                p.section = Some(Box::new(props.clone()));
            }
            doc.body.push(para_block(p));
        }
    }
    let note = doc.add_part(
        PartKind::Footnote,
        vec![para_block(Paragraph::with_text(
            "Local note.",
            Default::default(),
        ))],
    );
    doc.insert_object(
        &Pos::body(target, 5),
        InlineObject::NoteRef {
            kind: NoteKind::Footnote,
            id: note,
            custom: String::new(),
        },
        &Default::default(),
    )
    .unwrap();
    for (at, text) in [(4, "Earlier note"), (target + 20, "Later note")] {
        let id = doc.add_part(
            PartKind::Footnote,
            vec![para_block(Paragraph::with_text(text, Default::default()))],
        );
        doc.insert_object(
            &Pos::body(at, 2),
            InlineObject::NoteRef {
                kind: NoteKind::Footnote,
                id,
                custom: String::new(),
            },
            &Default::default(),
        )
        .unwrap();
    }
    (doc, target, note)
}

fn compare(
    doc: &Document,
    incremental: &DocLayout,
    full: &DocLayout,
    changed: std::ops::Range<usize>,
    note: u32,
) {
    assert_eq!(incremental.text_boxes, full.text_boxes);
    assert_eq!(incremental.index, full.index);
    assert_eq!(incremental.pages.len(), full.pages.len());
    for (i, (a, b)) in incremental.pages.iter().zip(&full.pages).enumerate() {
        assert_eq!(format!("{a:?}"), format!("{b:?}"), "page {}", i + 1);
        for (x, y) in [(78., 92.), (308., 194.), (90., 698.)] {
            for story in [StoryRef::Body, StoryRef::Part(note)] {
                assert_eq!(incremental.hit(i, x, y, story), full.hit(i, x, y, story));
            }
        }
    }
    let pages = (changed.start - 1..=changed.end).collect::<Vec<_>>();
    for &i in &pages {
        let bitmap = |l: &DocLayout| {
            joyword_render::render_page(doc, &l.pages[i], 1., &Default::default()).pixels
        };
        assert_eq!(bitmap(incremental), bitmap(full));
    }
    let pdf = joyword_pdf::PdfOptions {
        pages: Some(pages),
        ..Default::default()
    };
    assert!(
        joyword_pdf::export_fixed_layout(doc, incremental, &pdf, None).unwrap()
            == joyword_pdf::export_fixed_layout(doc, full, &pdf, None).unwrap()
    );
}

#[test]
fn footnote_height_and_rebalanced_columns_match_whole_document_outputs() {
    let (mut doc, block, note) = fixture();
    let (mut cache, opts) = (LayoutCache::new(), LayoutOptions::default());
    let mut last = layout_shared(&doc, &mut cache, &opts);
    assert_eq!(last.pages.len(), 69);
    let body = StoryRef::Body;
    let footnote = StoryRef::Part(note);
    let insert = "Edited clause. ".repeat(18);
    for (story, path, add) in [
        (footnote, Path::top(0), true),
        (footnote, Path::top(0), false),
        (body, Path::top(block + 2), true),
        (body, Path::top(block + 2), false),
    ] {
        let p = doc.para_mut(story, &path).unwrap();
        if add {
            p.insert_text(0, &insert, &Default::default()).unwrap();
        } else {
            p.delete(0, insert.len()).unwrap();
        }
        let next = layout_shared(&doc, &mut cache, &opts);
        let changed = next.changed_pages_from(&last).expect("section-local edit");
        assert_eq!(changed, 34..35);
        let full = layout(&doc, &mut LayoutCache::new(), &opts);
        compare(&doc, &next, &full, changed, note);
        last = next;
    }
    // A footnote that carries through the next fresh page must propagate globally.
    doc.para_mut(footnote, &Path::top(0))
        .unwrap()
        .insert_text(0, &insert.repeat(80), &Default::default())
        .unwrap();
    let oversized = layout_shared(&doc, &mut cache, &opts);
    assert!(oversized.changed_pages_from(&last).is_none());
    assert_eq!(
        oversized.index,
        layout(&doc, &mut LayoutCache::new(), &opts).index
    );
}

#[test]
fn nested_cells_and_floating_tables_retain_hit_paths_after_local_reflow() {
    use joyword_model::{Block, Cell, Table, props::TableFloat};
    use std::sync::Arc;

    for floating in [false, true] {
        let (mut doc, block, note) = fixture();
        let table_block = block + 2;
        let mut nested = Table::new(1, 1, 95.);
        nested.rows[0].cells[0] = Cell::with_text("Nested value");
        let mut outer = Table::new(1, 1, 115.);
        outer.props.float = floating.then(|| TableFloat {
            x: 6.,
            dist: [4.; 4],
            ..Default::default()
        });
        outer.rows[0].cells[0].blocks = vec![Arc::new(Block::Table(nested))];
        doc.body.insert(table_block, Arc::new(Block::Table(outer)));
        let (mut cache, opts) = (LayoutCache::new(), LayoutOptions::default());
        let mut last = layout_shared(&doc, &mut cache, &opts);
        let path = Path(vec![table_block as u32, 0, 0, 0, 0, 0, 0]);
        assert!(last.index.contains_key(&(StoryRef::Body, path.clone())));
        assert_eq!(
            last.pages[34]
                .float_tables
                .contains(&Path::top(table_block)),
            floating
        );
        let text = "Nested value changed. ".repeat(2);
        for add in [true, false] {
            let p = doc.para_mut(StoryRef::Body, &path).unwrap();
            if add {
                p.insert_text(0, &text, &Default::default()).unwrap();
            } else {
                p.delete(0, text.len()).unwrap();
            }
            let next = layout_shared(&doc, &mut cache, &opts);
            let changed = next
                .changed_pages_from(&last)
                .expect("section containing the table");
            assert_eq!(changed, 34..35);
            compare(
                &doc,
                &next,
                &layout(&doc, &mut LayoutCache::new(), &opts),
                changed,
                note,
            );
            last = next;
        }
    }
}
