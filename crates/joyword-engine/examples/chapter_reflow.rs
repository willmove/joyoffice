//! Compare incremental and full pagination on a 1,000-page chapter or a native section fixture.
//! Pass a fixture followed by `--grow [initial-page-count]` to add and remove pages.
//! Append `--allow-full` when measuring a baseline that falls back to full pagination.
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
    let grow = std::env::args_os()
        .nth(2)
        .is_some_and(|arg| arg == "--grow");
    let allow_full = std::env::args_os().skip(2).any(|arg| arg == "--allow-full");
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
        let para = doc.para_mut(story, &path)?;
        if i % 2 == 0 {
            para.insert_text(0, &change, &Default::default())?;
        } else {
            para.delete(0, change.len())?;
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
