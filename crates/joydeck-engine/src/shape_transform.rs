//! Transaction shared by direct frame edits and selection gestures.

use super::{BTreeSet, EditorSnapshot, checked};
use joydeck_model::{coordinates, resolve};
use std::sync::Arc;

pub(super) enum Change {
    Frame { size_delta: [f64; 2] },
    Selection { scale: [f64; 2] },
}

pub(super) fn apply(
    editor: &mut EditorSnapshot,
    movement: [f64; 2],
    rotation: f64,
    change: Change,
) -> Result<(), String> {
    let page = editor.selection.page;
    let source = editor.model.slides.get(page).ok_or("missing slide")?;
    let context = resolve::Ctx::for_slide(&editor.model, source).ok_or("missing layout")?;
    let is_selection = matches!(change, Change::Selection { .. });
    let parameters = match change {
        Change::Frame { size_delta } => size_delta,
        Change::Selection { scale } => scale,
    };
    let [dx, dy] = movement;
    let [a, b] = parameters;
    for value in [dx, dy, a, b, rotation] {
        checked(value)?;
    }
    if is_selection
        && parameters
            .iter()
            .any(|value| !(0.001..=1000.).contains(value))
    {
        return Err("selection scale outside budget".into());
    }
    let ids: BTreeSet<_> = editor.selection.shapes.iter().copied().collect();
    let mut selected = Vec::new();
    for (index, shape) in source
        .shapes
        .iter()
        .enumerate()
        .filter(|(_, s)| ids.contains(&s.id))
    {
        if shape.locked {
            return Err(if is_selection {
                "object is locked"
            } else {
                "selected object is locked"
            }
            .into());
        }
        selected.push((index, resolve::xfrm(&context, shape)));
    }
    if selected.len() != ids.len() {
        return Err("missing shape".into());
    }
    let Some((_, first)) = selected.first() else {
        return Err(if is_selection {
            "select at least one shape"
        } else {
            "select an object first"
        }
        .into());
    };

    let pivot = is_selection.then(|| {
        selected[1..]
            .iter()
            .map(|(_, frame)| frame.bounds())
            .fold(first.bounds(), |bounds, next| bounds.union(next))
            .center()
    });
    let (sin, cos) = rotation.to_radians().sin_cos();
    for (_, frame) in &mut selected {
        if let Some(pivot) = pivot {
            let center = frame.center();
            let x = (center.x - pivot.x) * a;
            let y = (center.y - pivot.y) * b;
            frame.w *= a;
            frame.h *= b;
            frame.x = pivot.x + x * cos - y * sin + dx - frame.w / 2.0;
            frame.y = pivot.y + x * sin + y * cos + dy - frame.h / 2.0;
        } else {
            frame.x += dx;
            frame.y += dy;
            frame.w += a;
            frame.h += b;
        }
        for value in [frame.x, frame.y, frame.w, frame.h] {
            checked(value)?;
        }
        frame.rot = (frame.rot + rotation).rem_euclid(360.0);
        if frame.w < 1.0 || frame.h < 1.0 {
            return Err("shape size must be at least 1 point".into());
        }
    }
    if selected
        .iter()
        .all(|(index, frame)| source.shapes[*index].xfrm == Some(*frame))
    {
        return Ok(());
    }

    let mut draft = source.as_ref().clone();
    for (index, frame) in selected {
        draft.shapes[index].xfrm = Some(frame);
    }
    coordinates::retarget_connectors(&context, &source.shapes, &mut draft.shapes);
    Arc::make_mut(&mut editor.model).slides[page] = Arc::new(draft);
    Ok(())
}
