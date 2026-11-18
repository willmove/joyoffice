//! Compare live section replacements, coordinates and repaint ranges to a fresh
//! full pagination of the same DOCX after inserting text, undoing and redoing.
use joyword_engine::doc::{Pos, StoryRef};
use joyword_layout::{LayoutCache, LayoutOptions, layout, layout_shared};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args_os().nth(1).ok_or("expected a DOCX path")?;
    let original = joyword_docx::read(&std::fs::read(input)?)?;
    let mut edited = original.clone();
    let block = original
        .body
        .iter()
        .position(|b| {
            b.as_para()
                .is_some_and(|p| p.text.starts_with("Section 34"))
        })
        .ok_or("missing edited section")?;
    edited.insert_text(
        &Pos::body(block, 0),
        &"Edited clause. ".repeat(740),
        &Default::default(),
    )?;
    let opts = LayoutOptions::default();
    let mut cache = LayoutCache::new();
    let mut previous = layout_shared(&original, &mut cache, &opts);
    for (label, doc) in [("insert", &edited), ("undo", &original), ("redo", &edited)] {
        let candidate = layout_shared(doc, &mut cache, &opts);
        let reference = layout(doc, &mut LayoutCache::new(), &opts);
        if candidate.index != reference.index || candidate.pages.len() != reference.pages.len() {
            return Err(format!("{label}: body index or page count differs").into());
        }
        for (i, (a, b)) in candidate.pages.iter().zip(&reference.pages).enumerate() {
            if format!("{a:?}") != format!("{b:?}") {
                return Err(format!("{label}: page {} differs", i + 1).into());
            }
            for story in [
                Some(StoryRef::Body),
                a.header_story.map(StoryRef::Part),
                a.footer_story.map(StoryRef::Part),
            ]
            .into_iter()
            .flatten()
            {
                for y in [a.body.y - 10., a.body.y + 15., a.body.y + a.body.h + 10.] {
                    for x in [a.body.x + 8., a.body.x + a.body.w / 2.] {
                        if candidate.hit(i, x, y, story) != reference.hit(i, x, y, story) {
                            return Err(format!("{label}: page {} hit differs", i + 1).into());
                        }
                    }
                }
            }
        }
        let dirty = candidate
            .changed_pages_from(&previous)
            .ok_or_else(|| format!("{label}: complete pagination fallback"))?;
        println!(
            "{label},{},{},{}",
            candidate.pages.len(),
            dirty.start + 1,
            dirty.end
        );
        previous = candidate;
    }
    Ok(())
}
