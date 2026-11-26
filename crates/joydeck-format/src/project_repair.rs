//! Normalize legacy model identities, layout references, and numbers at the native format boundary.

use std::collections::HashSet;
use std::sync::Arc;

use joydeck_model::style::{Effects, Line, Reflection, Shadow};
use joydeck_model::text::{ListStyle, RunProps, TextBody};
use joydeck_model::{
    Layout, LayoutId, Presentation, Shape, ShapeId, ShapeKind, Size, SlideId, Xfrm, defaults,
};

pub const SLIDE_SIDE: std::ops::RangeInclusive<f64> = 72.0..=4032.0;
const MAX_COORD: f64 = 1.0e6;
const MAX_SHAPE_DEPTH: usize = 64;

pub fn repair(presentation: &mut Presentation) {
    if presentation.masters.is_empty() {
        presentation.masters =
            defaults::blank_presentation(presentation.slide_size, Default::default(), false)
                .masters;
    }
    presentation.fix_next_id();
    let mut layouts: HashSet<_> = presentation
        .masters
        .iter()
        .flat_map(|master| master.layouts.iter().map(|layout| layout.id))
        .collect();
    let fallback = presentation
        .masters
        .iter()
        .find_map(|master| master.layouts.first().map(|layout| layout.id));
    let fallback = fallback.or_else(|| {
        let id = presentation.alloc_id().map(LayoutId)?;
        Arc::make_mut(presentation.masters.first_mut()?)
            .layouts
            .push(Layout {
                id,
                ..Layout::default()
            });
        layouts.insert(id);
        Some(id)
    });
    let mut slide_ids = HashSet::new();
    let mut next = presentation.next_id;
    for slide in &mut presentation.slides {
        let replace_id = if slide_ids.insert(slide.id) {
            None
        } else {
            allocate(&mut next).map(SlideId)
        };
        let replace_layout = fallback.filter(|_| !layouts.contains(&slide.layout));
        if replace_id.is_some() || replace_layout.is_some() {
            let slide = Arc::make_mut(slide);
            slide.id = replace_id.unwrap_or(slide.id);
            slide.layout = replace_layout.unwrap_or(slide.layout);
        }
    }
    for slide in &mut presentation.slides {
        if contains_duplicates(&slide.shapes) {
            reassign(&mut Arc::make_mut(slide).shapes, &mut next);
        }
    }
    for master in presentation
        .masters
        .iter_mut()
        .chain(presentation.notes_master.iter_mut())
        .chain(presentation.handout_master.iter_mut())
    {
        if !contains_duplicates(&master.shapes)
            && !master
                .layouts
                .iter()
                .any(|layout| contains_duplicates(&layout.shapes))
        {
            continue;
        }
        let master = Arc::make_mut(master);
        reassign(&mut master.shapes, &mut next);
        for layout in &mut master.layouts {
            reassign(&mut layout.shapes, &mut next);
        }
    }
    presentation.next_id = next;
    presentation.slide_size = sane_slide_size(presentation.slide_size);
    if !valid_size(presentation.notes_size) {
        presentation.notes_size = Presentation::default().notes_size;
    }
    sane_numbers(presentation);
}

fn allocate(next: &mut u32) -> Option<u32> {
    *next = (*next).max(1).checked_add(1)?;
    Some(*next)
}

fn contains_duplicates(shapes: &[Shape]) -> bool {
    let mut seen = HashSet::new();
    let mut duplicate = false;
    joydeck_model::walk(shapes, &mut |shape, _| duplicate |= !seen.insert(shape.id));
    duplicate
}

fn reassign(shapes: &mut [Shape], next: &mut u32) {
    // References continue to select the first occurrence. If the identifier space is exhausted,
    // allocation stays exhausted and the duplicate remains visible.
    let mut seen = HashSet::<ShapeId>::new();
    for_each_shape(shapes, 0, &mut |shape| {
        if !seen.insert(shape.id)
            && let Some(id) = allocate(next).map(ShapeId)
        {
            shape.id = id;
            seen.insert(id);
        }
    });
}

fn for_each_shape(shapes: &mut [Shape], depth: usize, visit: &mut impl FnMut(&mut Shape)) {
    for shape in shapes {
        visit(shape);
        if depth < MAX_SHAPE_DEPTH
            && let ShapeKind::Group { children, .. } = &mut shape.kind
        {
            for_each_shape(children, depth + 1, visit);
        }
    }
}

fn valid_size(size: Size) -> bool {
    [size.width, size.height]
        .iter()
        .all(|dimension| dimension.is_finite() && *dimension > 0.0)
}

pub fn sane_slide_size(size: Size) -> Size {
    if !valid_size(size) {
        return defaults::WIDE;
    }
    let clamp = |value: f64| value.clamp(*SLIDE_SIDE.start(), *SLIDE_SIDE.end());
    Size::new(clamp(size.width), clamp(size.height))
}

pub fn sane_numbers(presentation: &mut Presentation) {
    for slide in &mut presentation.slides {
        let mut normalized = (**slide).clone();
        clean_tree(&mut normalized.shapes);
        if normalized != **slide {
            *slide = Arc::new(normalized);
        }
    }
    clean_list(&mut presentation.default_text_style);
    for master in presentation
        .masters
        .iter_mut()
        .chain(presentation.notes_master.iter_mut())
        .chain(presentation.handout_master.iter_mut())
    {
        let mut normalized = (**master).clone();
        clean_tree(&mut normalized.shapes);
        for layout in &mut normalized.layouts {
            clean_tree(&mut layout.shapes);
        }
        clean_list(&mut normalized.title_style);
        clean_list(&mut normalized.body_style);
        clean_list(&mut normalized.other_style);
        for effects in &mut normalized.theme.format.effects {
            clean_effects(effects);
        }
        if normalized != **master {
            *master = Arc::new(normalized);
        }
    }
}

fn clean_tree(shapes: &mut [Shape]) {
    for_each_shape(shapes, 0, &mut |shape| {
        if let Some(frame) = &mut shape.xfrm {
            clean_frame(frame);
        }
        if let Some(line) = &mut shape.line {
            clean_line(line);
        }
        if let Some(effects) = &mut shape.effects {
            clean_effects(effects);
        }
        if let Some(text) = &mut shape.text {
            clean_text(text);
        }
        match &mut shape.kind {
            ShapeKind::Picture { fill } => clean_crop(&mut fill.crop),
            ShapeKind::Media(clip) => clean_crop(&mut clip.crop),
            ShapeKind::Group { child, .. } => clean_frame(child),
            ShapeKind::Table(table) => {
                for cell in table.rows.iter_mut().flat_map(|row| &mut row.cells) {
                    clean_text(&mut cell.text);
                }
            }
            _ => {}
        }
    });
}

fn clean_text(text: &mut TextBody) {
    clean_list(&mut text.list_style);
    for paragraph in &mut text.paragraphs {
        clean_run(&mut paragraph.def_props);
        clean_run(&mut paragraph.end_props);
        for run in &mut paragraph.runs {
            clean_run(&mut run.props);
        }
    }
}

fn clean_list(list: &mut ListStyle) {
    for level in list.levels.iter_mut().flatten() {
        clean_run(&mut level.run);
    }
}

fn clean_run(run: &mut RunProps) {
    run.size = run
        .size
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.clamp(1.0, 4000.0));
    if let Some(outline) = &mut run.outline {
        clean_line(outline);
    }
    for shadow in run.shadow.iter_mut().chain(run.inner_shadow.iter_mut()) {
        clean_shadow(shadow);
    }
    if let Some(glow) = &mut run.glow {
        glow.radius = limit(glow.radius, 0.0, 150.0, 0.0);
    }
    if let Some(reflection) = &mut run.reflection {
        clean_reflection(reflection);
    }
    run.soft_edge = run.soft_edge.map(|value| limit(value, 0.0, 100.0, 0.0));
}

fn clean_effects(effects: &mut Effects) {
    for shadow in effects
        .outer_shadow
        .iter_mut()
        .chain(effects.inner_shadow.iter_mut())
    {
        clean_shadow(shadow);
    }
    if let Some(glow) = &mut effects.glow {
        glow.radius = limit(glow.radius, 0.0, 150.0, 0.0);
    }
    if let Some(reflection) = &mut effects.reflection {
        clean_reflection(reflection);
    }
    effects.soft_edge = effects.soft_edge.map(|value| limit(value, 0.0, 100.0, 0.0));
}

fn clean_frame(frame: &mut Xfrm) {
    for position in [&mut frame.x, &mut frame.y] {
        *position = limit(*position, -MAX_COORD, MAX_COORD, 0.0);
    }
    for extent in [&mut frame.w, &mut frame.h] {
        *extent = limit(*extent, 0.0, MAX_COORD, 0.0);
    }
    frame.rot = rotation(frame.rot);
    if frame.rot >= 360.0 {
        frame.rot = 0.0;
    }
}

fn clean_crop(crop: &mut [f64; 4]) {
    for value in crop {
        *value = limit(*value, -10.0, 1.0, 0.0);
    }
}

fn clean_line(line: &mut Line) {
    line.width = line
        .width
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| value.min(1584.0));
}

fn clean_shadow(shadow: &mut Shadow) {
    shadow.blur = limit(shadow.blur, 0.0, 200.0, 0.0);
    shadow.dist = limit(shadow.dist, 0.0, 200.0, 0.0);
    shadow.dir = rotation(shadow.dir);
    for scale in [&mut shadow.sx, &mut shadow.sy] {
        *scale = limit(*scale, -10.0, 10.0, 1.0);
    }
    for skew in [&mut shadow.kx, &mut shadow.ky] {
        *skew = limit(*skew, -89.0, 89.0, 0.0);
    }
}

fn clean_reflection(reflection: &mut Reflection) {
    reflection.blur = limit(reflection.blur, 0.0, 100.0, 0.0);
    reflection.dist = limit(reflection.dist, 0.0, 100.0, 0.0);
}

fn rotation(value: f64) -> f64 {
    if value.is_finite() {
        value.rem_euclid(360.0)
    } else {
        0.0
    }
}

fn limit(value: f64, minimum: f64, maximum: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value.clamp(minimum, maximum)
    } else {
        fallback
    }
}
