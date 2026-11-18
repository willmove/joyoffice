//! Compare incremental and full pagination on a 1,000-page chapter or the numbered footnote in the native section fixture.
use joyword_layout::{LayoutCache, LayoutOptions, layout, layout_shared};
use joyword_model::{Document, Path, StoryRef, para::Paragraph, para_block};
use std::time::Instant;

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
    let (mut doc, story, path, expected_pages) = if let Some(filename) = std::env::args_os().nth(1)
    {
        let doc = joyword_docx::read(&std::fs::read(filename)?)?;
        let (&note, _) = doc
            .parts
            .iter()
            .find(|(id, _)| doc.plain_text(StoryRef::Part(**id)) == "Numbered note 2")
            .ok_or("expected the 69-page section fixture from check-section-reflow.py")?;
        (doc, StoryRef::Part(note), Path::top(0), 69)
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
    let change = "Extra words that wrap. ".repeat(if story == StoryRef::Body { 4 } else { 14 });
    let mut timings = Vec::new();
    for i in 0..20 {
        let begin = Instant::now();
        let para = doc.para_mut(story, &path)?;
        if i % 2 == 0 {
            para.insert_text(0, &change, &Default::default())?;
        } else {
            para.delete(0, change.len())?;
        }
        let next = layout_shared(&doc, &mut cache, &opts);
        let edit_ms = begin.elapsed().as_secs_f64() * 1000.;
        let changed = next.changed_pages_from(&previous).ok_or("full fallback")?;
        let begin = Instant::now();
        let full = layout(&doc, &mut full_cache, &opts);
        let full_ms = begin.elapsed().as_secs_f64() * 1000.;
        assert_eq!(next.pages.len(), full.pages.len());
        assert_eq!(next.index, full.index);
        if i < 2 {
            for (p, (a, b)) in next.pages.iter().zip(&full.pages).enumerate() {
                assert_eq!(format!("{a:?}"), format!("{b:?}"));
                for (x, y) in [(75., 60.), (145., 120.), (245., 160.)] {
                    assert_eq!(next.hit(p, x, y, story), full.hit(p, x, y, story));
                }
            }
            let pages = ((changed.start - 1)..=(changed.end)).collect::<Vec<_>>();
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
        timings.push(serde_json::json!({"editThroughLayoutMs":edit_ms,"fullLayoutMs":full_ms,"changedPages":changed.len()}));
        previous = next;
    }
    println!(
        "{}",
        serde_json::json!({"pages":expected_pages,"samples":timings,"allPageComparisons":expected_pages*2,"hitComparisons":expected_pages*6,"rasterAndPdfCompared":"affected pages and both neighboring pages for insertion and deletion"})
    );
    Ok(())
}
