//! Slide placement and DrawingML connector bindings for coordinate edits.

use std::collections::HashMap;

use joydeck_geom::{Affine, Point, Size, point_sized_box, preset, pt_to_emu};

use crate::{Geom, Shape, ShapeId, ShapeKind, Xfrm, resolve};

// Match the imported and rendered group tree.
const MAX_GROUP_DEPTH: usize = 32;

#[derive(Clone, Copy)]
struct Placement<'a> {
    shape: &'a Shape,
    frame: Xfrm,
    parent: Affine,
}

type Placements<'a> = HashMap<ShapeId, Placement<'a>>;
type Binding = (ShapeId, u32);

/// Update persisted connector frames when their bound sites move relative to the connector's
/// parent. Unaffected endpoints keep their authored geometry, including unresolved bindings.
pub fn retarget_connectors(ctx: &resolve::Ctx, before: &[Shape], after: &mut [Shape]) {
    let mut bindings = Vec::new();
    crate::walk(before, &mut |shape, depth| {
        if depth <= MAX_GROUP_DEPTH
            && matches!(shape.kind, ShapeKind::Connector { start, end } if start.is_some() || end.is_some())
        {
            bindings.push(shape.id);
        }
    });
    if bindings.is_empty() {
        return;
    }

    let mut previous = Placements::new();
    let mut current = Placements::new();
    index(ctx, before, Affine::IDENTITY, 0, &mut previous);
    index(ctx, after, Affine::IDENTITY, 0, &mut current);
    let mut old_sites = HashMap::new();
    let mut new_sites = HashMap::new();
    let mut updates = HashMap::new();
    for id in bindings {
        let (Some(old), Some(new)) = (previous.get(&id), current.get(&id)) else {
            continue;
        };
        let ShapeKind::Connector { start, end } = new.shape.kind else {
            continue;
        };
        let Geom::Preset { name, .. } = &new.shape.geom else {
            continue;
        };
        let Some(local) = preset::connector_endpoints(name, new.frame.w, new.frame.h) else {
            continue;
        };
        let (Some(old_inverse), Some(new_inverse)) = (inverse(old.parent), inverse(new.parent))
        else {
            continue;
        };
        let original = local.map(|point| new.frame.affine() * point);
        let mut desired = original;
        for (point, binding) in desired.iter_mut().zip([start, end]) {
            let Some(binding) = binding else { continue };
            let old_site = *old_sites
                .entry(binding)
                .or_insert_with(|| site(&previous, binding));
            let new_site = *new_sites
                .entry(binding)
                .or_insert_with(|| site(&current, binding));
            if let (Some(old_site), Some(new_site)) = (old_site, new_site) {
                let target = new_inverse * new_site;
                if target.is_finite()
                    && (old.frame != new.frame || moved(old_inverse * old_site, target))
                {
                    *point = target;
                }
            }
        }
        if moved(original[0], desired[0]) || moved(original[1], desired[1]) {
            let fitted = new
                .frame
                .fit_diagonal(desired[0], desired[1], name == "lineInv");
            // Keep the persisted frame representable by DrawingML ST_Coordinate in EMUs.
            if fitted.is_finite()
                && [fitted.x, fitted.y, fitted.w, fitted.h]
                    .into_iter()
                    .all(|value| {
                        (-27_273_042_329_600..=27_273_042_316_900).contains(&pt_to_emu(value))
                    })
            {
                updates.insert(id, fitted);
            }
        }
    }
    if !updates.is_empty() {
        apply_frames(after, &updates, 0);
    }
}

fn index<'a>(
    ctx: &resolve::Ctx,
    shapes: &'a [Shape],
    parent: Affine,
    depth: usize,
    out: &mut Placements<'a>,
) {
    if depth > MAX_GROUP_DEPTH || !parent.is_finite() {
        return;
    }
    for shape in shapes {
        let frame = resolve::xfrm(ctx, shape);
        if !frame.is_finite() {
            continue;
        }
        out.insert(
            shape.id,
            Placement {
                shape,
                frame,
                parent,
            },
        );
        if let ShapeKind::Group { children, child } = &shape.kind {
            index(
                ctx,
                children,
                parent * frame.child_affine(child),
                depth + 1,
                out,
            );
        }
    }
}

fn site(placements: &Placements, (id, index): Binding) -> Option<Point> {
    let placement = placements.get(&id)?;
    let Geom::Preset { name, adj } = &placement.shape.geom else {
        return None;
    };
    if !matches!(
        placement.shape.kind,
        ShapeKind::Shape | ShapeKind::Picture { .. }
    ) {
        return None;
    }
    let to_slide = placement.parent * placement.frame.affine();
    let size = Size::new(placement.frame.w, placement.frame.h);
    let (size, to_slide) = point_sized_box(to_slide, to_slide, size);
    let geometry = preset::build(name, size.width, size.height, adj)?;
    let point = to_slide * *geometry.sites.get(usize::try_from(index).ok()?)?;
    point.is_finite().then_some(point)
}

fn inverse(transform: Affine) -> Option<Affine> {
    let inverse = transform.inverse();
    inverse.is_finite().then_some(inverse)
}

fn moved(before: Point, after: Point) -> bool {
    before.distance(after) > 1e-8
}

fn apply_frames(shapes: &mut [Shape], updates: &HashMap<ShapeId, Xfrm>, depth: usize) {
    if depth > MAX_GROUP_DEPTH {
        return;
    }
    for shape in shapes {
        if let Some(frame) = updates.get(&shape.id) {
            shape.xfrm = Some(*frame);
        }
        if let ShapeKind::Group { children, .. } = &mut shape.kind {
            apply_frames(children, updates, depth + 1);
        }
    }
}
