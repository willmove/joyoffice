//! Compare incremental and full pagination on a 1,000-page chapter or a native section fixture.
//! Pass a fixture followed by `--grow [initial-page-count]` to add and remove pages.
//! Add `--splice` to insert and merge two paragraphs in the middle section.
//! Pass `--section-splice` to benchmark section-break changes and reanchored drawings.
//! Append `--allow-full` when measuring a baseline that falls back to full pagination.
use joyword_layout::{LayoutCache, LayoutOptions, layout, layout_shared};
use joyword_model::{
    Document, InlineObject, Path, Pos, StoryRef,
    para::{Float, Paragraph, Wrap},
    para_block,
    section::SectionStart,
};
use std::time::Instant;

fn run_section_splice(allow_full: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut doc = Document::new();
    doc.body.clear();
    doc.styles.default_chr.font = Some("Source Sans 3".into());
    let add = |text: &str| para_block(Paragraph::with_text(text, Default::default()));
    for n in 0..68 {
        doc.body.extend([
            add(&format!("Heading {n}")),
            add("Before the break and after it"),
            add("Drawing anchor"),
            add("End"),
        ]);
        if n < 67 {
            doc.para_mut(StoryRef::Body, &Path::top(n * 4 + 3))?.section =
                Some(Box::new(doc.last_section.clone()));
        }
    }
    let at = 34 * 4 + 1;
    let shape = InlineObject::Shape {
        kind: joyword_model::para::ShapeKind::Rectangle,
        w: 73.,
        h: 51.,
        fill: None,
        stroke: None,
        stroke_width: 0.,
        float: Float {
            wrap: Wrap::Square,
            ..Default::default()
        },
        story: None,
        effects: Default::default(),
        freeform: None,
        extra: Default::default(),
    };
    doc.insert_object(&Pos::body(at + 1, 0), shape.clone(), &Default::default())?;
    let mut reanchored = doc.clone();
    reanchored.delete_range(
        &Pos::body(at + 1, 0),
        &Pos::body(at + 1, joyword_model::para::OBJ.len_utf8()),
    )?;
    reanchored.insert_object(&Pos::body(at, 4), shape, &Default::default())?;

    let mut states: Vec<(&str, Document)> =
        vec![("move_anchor", reanchored), ("restore_anchor", doc.clone())];
    for (label, start) in [
        ("insert_nextPage", SectionStart::NextPage),
        ("insert_continuous", SectionStart::Continuous),
        ("insert_oddPage", SectionStart::OddPage),
        ("insert_evenPage", SectionStart::EvenPage),
    ] {
        let mut split = doc.clone();
        let next = split.split_paragraph(&Pos::body(at, 10))?;
        split.para_mut(StoryRef::Body, &Path::top(at))?.section =
            Some(Box::new(doc.last_section.clone()));
        split.section_mut(next.path.last()).start = start;
        let mut removed = split.clone();
        assert!(removed.remove_section_break(at));
        states.push((label, split));
        states.push(("remove_boundary", removed));
        states.push(("restore_doc", doc.clone()));
    }

    let opts = LayoutOptions::default();
    let mut cache = LayoutCache::new();
    let mut full_cache = LayoutCache::new();
    let mut previous = layout_shared(&doc, &mut cache, &opts);
    let _ = layout(&doc, &mut full_cache, &opts);

    let mut timings = Vec::new();
    let mut compared = 0usize;
    let mut full_fallbacks = 0usize;
    for (step, (label, state)) in states.iter().enumerate() {
        let begin = Instant::now();
        let next = layout_shared(state, &mut cache, &opts);
        let edit_ms = begin.elapsed().as_secs_f64() * 1000.;
        let changed = match next.changed_pages_from(&previous) {
            Some(range) => range,
            None if allow_full => {
                full_fallbacks += 1;
                0..next.pages.len().max(previous.pages.len())
            }
            None => return Err(format!("full fallback at {label}").into()),
        };
        let begin = Instant::now();
        let full = layout(state, &mut full_cache, &opts);
        let full_ms = begin.elapsed().as_secs_f64() * 1000.;
        assert_eq!(next.pages.len(), full.pages.len(), "{label}");
        assert_eq!(next.index, full.index, "{label}");
        compared += next.pages.len();
        for (p, (a, b)) in next.pages.iter().zip(&full.pages).enumerate() {
            assert_eq!(format!("{a:?}"), format!("{b:?}"), "{label} page {p}");
            for (x, y) in [(75., 60.), (95., 115.), (245., 160.)] {
                assert_eq!(
                    next.hit(p, x, y, StoryRef::Body),
                    full.hit(p, x, y, StoryRef::Body),
                    "{label} page {p}"
                );
            }
        }
        if step < 6 {
            let pages: Vec<usize> = (changed.start.saturating_sub(1)
                ..=changed.end.min(next.pages.len() - 1))
                .take(8)
                .collect();
            for &p in &pages {
                let r_next =
                    joyword_render::render_page(state, &next.pages[p], 1., &Default::default())
                        .pixels;
                let r_full =
                    joyword_render::render_page(state, &full.pages[p], 1., &Default::default())
                        .pixels;
                assert_eq!(r_next, r_full, "{label} raster page {p}");
            }
            let pdf = joyword_pdf::PdfOptions {
                pages: Some(pages),
                ..Default::default()
            };
            assert!(
                joyword_pdf::export_fixed_layout(state, &next, &pdf, None)?
                    == joyword_pdf::export_fixed_layout(state, &full, &pdf, None)?
            );
        }
        timings.push(serde_json::json!({
            "step": label,
            "editThroughLayoutMs": edit_ms,
            "fullLayoutMs": full_ms,
            "changedPages": changed.len(),
            "pageCount": next.pages.len(),
        }));
        previous = next;
    }
    println!(
        "{}",
        serde_json::json!({
            "mode": "section-splice",
            "steps": states.len(),
            "fullFallbacks": full_fallbacks,
            "allPageComparisons": compared,
            "hitComparisons": compared * 3,
            "samples": timings,
        })
    );
    Ok(())
}

fn thousand_page_chapter() -> (Document, Path) {
    let mut doc = Document::new();
    doc.body.clear();
    doc.styles.default_chr.font = Some("Source Sans 3".into());
    doc.last_section.page_h = 240.;
    doc.last_section.margin_top = 24.;
    doc.last_section.margin_bottom = 24.;
    let text = "Alpha beta gamma. ".repeat(9);
    let chapter = (0..12)
        .map(|i| {
            let mut p = Paragraph::with_text(&text, Default::default());
            p.props.page_break_before = Some(i == 0);
            p.props.space_before = Some(0.);
            para_block(p)
        })
        .collect::<Vec<_>>();
    doc.body = chapter.clone();
    let opts = LayoutOptions::default();
    let chapter_pages = layout(&doc, &mut LayoutCache::new(), &opts).pages.len();
    let start = 500;
    doc.body.clear();
    for page in 0..1000 - chapter_pages {
        if page == start {
            doc.body.extend(chapter.iter().cloned());
        }
        let mut p = Paragraph::with_text(&format!("Page {page}"), Default::default());
        p.props.page_break_before = Some(true);
        p.props.space_before = Some(0.);
        doc.body.push(para_block(p));
    }
    (doc, Path::top(start + 3))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let allow_full = std::env::args_os().skip(1).any(|arg| arg == "--allow-full");
    if std::env::args_os()
        .skip(1)
        .any(|arg| arg == "--section-splice")
    {
        return run_section_splice(allow_full);
    }
    let grow = std::env::args_os()
        .nth(2)
        .is_some_and(|arg| arg == "--grow");
    let splice = std::env::args_os().skip(2).any(|arg| arg == "--splice");
    let (mut doc, story, path, expected_pages) = if let Some(filename) = std::env::args_os().nth(1)
    {
        let doc = joyword_docx::read(&std::fs::read(filename)?)?;
        if grow {
            let expected_pages = std::env::args()
                .nth(3)
                .filter(|arg| arg != "--allow-full")
                .map(|pages| pages.parse::<usize>())
                .transpose()?
                .unwrap_or(69);
            let block = doc
                .body
                .iter()
                .position(|b| {
                    b.as_para()
                        .is_some_and(|p| p.text.starts_with("Section 34"))
                })
                .ok_or("missing middle heading in the section fixture")?;
            (doc, StoryRef::Body, Path::top(block), expected_pages)
        } else {
            let (&note, _) = doc
                .parts
                .iter()
                .find(|(id, _)| doc.plain_text(StoryRef::Part(**id)) == "Numbered note 2")
                .ok_or("expected the 69-page section fixture from check-section-reflow.py")?;
            (doc, StoryRef::Part(note), Path::top(0), 69)
        }
    } else {
        let (doc, path) = thousand_page_chapter();
        (doc, StoryRef::Body, path, 1000)
    };
    let opts = LayoutOptions::default();
    let mut cache = LayoutCache::new();
    let mut full_cache = LayoutCache::new();
    let mut previous = layout_shared(&doc, &mut cache, &opts);
    assert_eq!(previous.pages.len(), expected_pages);
    let _ = layout(&doc, &mut full_cache, &opts);
    let change = if grow {
        "Edited clause. ".repeat(740)
    } else {
        "Extra words that wrap. ".repeat(if story == StoryRef::Body { 4 } else { 14 })
    };
    let mut timings = Vec::new();
    let mut compared = 0;
    let mut full_fallbacks = 0;
    for i in 0..20 {
        let begin = Instant::now();
        if splice {
            let start = joyword_model::Pos::new(story, path.clone(), 0);
            if i % 2 == 0 {
                let heading = doc.split_paragraph(&start)?;
                doc.split_paragraph(&heading)?;
                doc.insert_text(&start, &change, &Default::default())?;
                doc.insert_text(&heading, "SpliceWitness", &Default::default())?;
            } else {
                doc.delete_range(&start, &joyword_model::Pos::body(path.last() + 2, 0))?;
            }
        } else {
            let para = doc.para_mut(story, &path)?;
            if i % 2 == 0 {
                para.insert_text(0, &change, &Default::default())?;
            } else {
                para.delete(0, change.len())?;
            }
        }
        let next = layout_shared(&doc, &mut cache, &opts);
        let edit_ms = begin.elapsed().as_secs_f64() * 1000.;
        let changed = match next.changed_pages_from(&previous) {
            Some(range) => range,
            None if allow_full => {
                full_fallbacks += 1;
                0..next.pages.len().max(previous.pages.len())
            }
            None => return Err("full fallback".into()),
        };
        let begin = Instant::now();
        let full = layout(&doc, &mut full_cache, &opts);
        let full_ms = begin.elapsed().as_secs_f64() * 1000.;
        assert_eq!(next.pages.len(), full.pages.len());
        if grow {
            assert_eq!(next.pages.len() > expected_pages, i % 2 == 0);
        }
        assert_eq!(next.index, full.index);
        if i < 2 {
            compared += next.pages.len();
            for (p, (a, b)) in next.pages.iter().zip(&full.pages).enumerate() {
                assert_eq!(format!("{a:?}"), format!("{b:?}"));
                for (x, y) in [(75., 60.), (145., 120.), (245., 160.)] {
                    assert_eq!(next.hit(p, x, y, story), full.hit(p, x, y, story));
                }
            }
            let pages = (changed.start.saturating_sub(1)..=changed.end.min(next.pages.len() - 1))
                .collect::<Vec<_>>();
            for &p in &pages {
                let raster = |layout: &joyword_layout::DocLayout| {
                    joyword_render::render_page(&doc, &layout.pages[p], 1., &Default::default())
                        .pixels
                };
                assert_eq!(raster(&next), raster(&full));
            }
            let pdf = joyword_pdf::PdfOptions {
                pages: Some(pages),
                ..Default::default()
            };
            assert!(
                joyword_pdf::export_fixed_layout(&doc, &next, &pdf, None)?
                    == joyword_pdf::export_fixed_layout(&doc, &full, &pdf, None)?
            );
        }
        timings.push(serde_json::json!({"editThroughLayoutMs":edit_ms,"fullLayoutMs":full_ms,"changedPages":changed.len(),"pageCount":next.pages.len()}));
        previous = next;
    }
    println!(
        "{}",
        serde_json::json!({"pages":expected_pages,"samples":timings,"fullFallbacks":full_fallbacks,"allPageComparisons":compared,"hitComparisons":compared*3,"rasterAndPdfCompared":"affected pages and available neighboring pages for insertion and deletion"})
    );
    Ok(())
}
