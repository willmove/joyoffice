//! Whole-selection transforms and slide organization preserve IDs and references.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WorkspaceAction {
    DeleteSlides {
        slides: Vec<SlideId>,
    },
    DuplicateSlides {
        slides: Vec<SlideId>,
    },
    TransformSelection {
        dx: f64,
        dy: f64,
        scale_x: f64,
        scale_y: f64,
        rotation: f64,
    },
    ReorderSlides {
        slides: Vec<SlideId>,
        before: Option<SlideId>,
    },
    HideSlides {
        slides: Vec<SlideId>,
        hidden: bool,
    },
    Section {
        name: String,
        slides: Vec<SlideId>,
    },
}
pub fn apply(st: &mut EditorSnapshot, action: &WorkspaceAction) -> std::result::Result<(), String> {
    let page = st.selection.page;
    if let WorkspaceAction::TransformSelection {
        dx,
        dy,
        scale_x,
        scale_y,
        rotation,
    } = action
    {
        for n in [*dx, *dy, *scale_x, *scale_y, *rotation] {
            checked(n)?;
        }
        if !(0.001..=1000.).contains(scale_x) || !(0.001..=1000.).contains(scale_y) {
            return Err("selection scale outside budget".into());
        }
        let p = st.model.as_ref();
        let slide = p.slides.get(page).ok_or("missing slide")?;
        let ctx = resolve::Ctx::for_slide(p, slide).ok_or("missing layout")?;
        let mut original = vec![];
        for id in &st.selection.shapes {
            let shape = slide.shape(*id).ok_or("missing shape")?;
            if shape.locked {
                return Err("object is locked".into());
            }
            original.push((*id, resolve::xfrm(&ctx, shape)));
        }
        let bounds = original
            .iter()
            .map(|(_, x)| x.bounds())
            .reduce(|a, b| a.union(b))
            .ok_or("select at least one shape")?;
        let center = bounds.center();
        let angle = rotation.to_radians();
        let (sin, cos) = angle.sin_cos();
        let slide = Arc::make_mut(&mut Arc::make_mut(&mut st.model).slides[page]);
        for (id, mut x) in original {
            let c = x.center();
            let rx = (c.x - center.x) * scale_x;
            let ry = (c.y - center.y) * scale_y;
            x.w = checked(x.w * scale_x)?;
            x.h = checked(x.h * scale_y)?;
            if x.w < 1. || x.h < 1. {
                return Err("shape size must be at least 1 point".into());
            }
            x.x = checked(center.x + rx * cos - ry * sin + dx - x.w / 2.)?;
            x.y = checked(center.y + rx * sin + ry * cos + dy - x.h / 2.)?;
            x.rot = (x.rot + rotation).rem_euclid(360.);
            slide.shape_mut(id).ok_or("missing shape")?.xfrm = Some(x);
        }
        return Ok(());
    }
    let slides = match action {
        WorkspaceAction::DeleteSlides { slides }
        | WorkspaceAction::DuplicateSlides { slides }
        | WorkspaceAction::ReorderSlides { slides, .. }
        | WorkspaceAction::HideSlides { slides, .. }
        | WorkspaceAction::Section { slides, .. } => slides,
        _ => return Err("unsupported workspace action".into()),
    };
    let unique: std::collections::BTreeSet<_> = slides.iter().copied().collect();
    if slides.is_empty()
        || unique.len() != slides.len()
        || slides.iter().any(|id| st.model.slide(*id).is_none())
    {
        return Err("invalid slide selection".into());
    }
    if matches!(
        action,
        WorkspaceAction::DeleteSlides { .. } | WorkspaceAction::DuplicateSlides { .. }
    ) {
        if matches!(action, WorkspaceAction::DeleteSlides { .. })
            && slides.len() >= st.model.slides.len()
        {
            return Err("keep at least one slide".into());
        }
        for id in slides {
            st.selection.page = st.model.slide_index(*id).ok_or("missing slide")?;
            super::apply(
                st,
                if matches!(action, WorkspaceAction::DeleteSlides { .. }) {
                    Command::DeleteSlide
                } else {
                    Command::DuplicateSlide
                },
            )?;
        }
        return Ok(());
    }
    let active = st.model.slides[page].id;
    let p = Arc::make_mut(&mut st.model);
    match action {
        WorkspaceAction::ReorderSlides { before, .. } => {
            if before.is_some_and(|id| unique.contains(&id) || p.slide(id).is_none()) {
                return Err("drop target must be an unselected slide".into());
            }
            let moved: Vec<_> = p
                .slides
                .iter()
                .filter(|s| unique.contains(&s.id))
                .cloned()
                .collect();
            p.slides.retain(|s| !unique.contains(&s.id));
            let at = before
                .and_then(|id| p.slide_index(id))
                .unwrap_or(p.slides.len());
            p.slides.splice(at..at, moved);
            let order: BTreeMap<_, _> = p
                .slides
                .iter()
                .enumerate()
                .map(|(i, s)| (s.id, i))
                .collect();
            for section in &mut p.sections {
                section.slides.sort_by_key(|id| order.get(id).copied());
            }
            st.selection.page = p.slide_index(active).ok_or("active slide missing")?;
        }
        WorkspaceAction::HideSlides { hidden, .. } => {
            for slide in &mut p.slides {
                if unique.contains(&slide.id) {
                    Arc::make_mut(slide).hidden = *hidden;
                }
            }
        }
        WorkspaceAction::Section { name, .. } => {
            if name.trim().is_empty() || name.len() > 256 || p.sections.len() >= 256 {
                return Err("invalid section name or section budget".into());
            }
            for section in &mut p.sections {
                section.slides.retain(|id| !unique.contains(id));
            }
            p.sections.retain(|s| !s.slides.is_empty());
            p.sections.push(Section {
                name: name.trim().into(),
                slides: p
                    .slides
                    .iter()
                    .filter(|s| unique.contains(&s.id))
                    .map(|s| s.id)
                    .collect(),
            });
        }
        _ => return Err("unsupported workspace action".into()),
    }
    Ok(())
}
