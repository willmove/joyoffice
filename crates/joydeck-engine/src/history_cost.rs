//! Adapted DeckCraft history accounting; IO and product services remain in shell.
use joydeck_model::Presentation;
use std::sync::Arc;
/// A rough size of a shape in memory: the struct, its text and kept XML, its children.
fn shape_cost(sh: &joydeck_model::Shape, depth: usize) -> usize {
    let text: usize = sh.text.as_ref().map_or(0, |t| {
        t.paragraphs
            .iter()
            .map(|p| 96 + p.runs.iter().map(|r| 128 + r.text.len()).sum::<usize>())
            .sum()
    });
    let kids: usize = if depth < 64 {
        sh.children().iter().map(|c| shape_cost(c, depth + 1)).sum()
    } else {
        0
    };
    768 + text + sh.raw_ext.as_ref().map_or(0, String::len) + kids
}

fn shapes_cost(shapes: &[joydeck_model::Shape]) -> usize {
    shapes.iter().map(|s| shape_cost(s, 0)).sum()
}

/// Estimated bytes `doc` keeps alive that `newer` doesn't share: its slides, masters and media
/// that aren't the same allocations (undo steps share everything an edit didn't touch).
pub fn undo_cost(doc: &Presentation, newer: &Presentation) -> usize {
    use std::collections::HashSet;
    let slides: HashSet<*const joydeck_model::Slide> =
        newer.slides.iter().map(Arc::as_ptr).collect();
    let masters: HashSet<*const joydeck_model::Master> =
        newer.masters.iter().map(Arc::as_ptr).collect();
    let media: HashSet<*const Vec<u8>> = newer.media.iter().map(|m| Arc::as_ptr(&m.data)).collect();
    let mut n = 64 * doc.slides.len();
    for s in doc
        .slides
        .iter()
        .filter(|s| !slides.contains(&Arc::as_ptr(s)))
    {
        n += 512 + shapes_cost(&s.shapes);
    }
    for m in doc
        .masters
        .iter()
        .filter(|m| !masters.contains(&Arc::as_ptr(m)))
    {
        n += 4096
            + shapes_cost(&m.shapes)
            + m.layouts
                .iter()
                .map(|l| 512 + shapes_cost(&l.shapes))
                .sum::<usize>();
    }
    for m in doc
        .media
        .iter()
        .filter(|m| !media.contains(&Arc::as_ptr(&m.data)))
    {
        n += m.data.len();
    }
    n
}
