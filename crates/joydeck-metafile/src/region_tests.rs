//! Compatibility cases retained from the original rectangle region implementation.
use super::*;

#[test]
fn intersect_shrinks_to_the_overlap() {
    let r = intersect(None, [0.0, 0.0, 100.0, 50.0]);
    let r = intersect(r, [40.0, 10.0, 80.0, 40.0]);
    assert_eq!(r, Some(vec![[40.0, 10.0, 80.0, 40.0]]));
    assert_eq!(intersect(r, [0.0, 0.0, 10.0, 10.0]), Some(vec![]));
    assert_eq!(
        intersect(Some(vec![[0.0, 0.0, 10.0, 20.0]]), [40.0, 0.0, 80.0, 20.0]),
        Some(vec![])
    );
}

#[test]
fn modes_copy_and_or_diff_and_xor() {
    let a = Some(vec![[0.0, 0.0, 100.0, 40.0]]);
    assert_eq!(
        combine(a.clone(), vec![[10.0, 10.0, 30.0, 20.0]], RGN_COPY, None),
        Some(vec![[10.0, 10.0, 30.0, 20.0]])
    );
    let and = combine(a.clone(), vec![[50.0, 0.0, 150.0, 40.0]], RGN_AND, None);
    assert_eq!(and, Some(vec![[50.0, 0.0, 100.0, 40.0]]));
    let or = combine(a.clone(), vec![[200.0, 0.0, 210.0, 10.0]], RGN_OR, None);
    assert_eq!(or.map(|rectangles| rectangles.len()), Some(2));
    let diff = combine(a.clone(), vec![[20.0, 0.0, 40.0, 40.0]], RGN_DIFF, None);
    assert_eq!(
        diff,
        Some(vec![[0.0, 0.0, 20.0, 40.0], [40.0, 0.0, 100.0, 40.0]])
    );
    let xor = combine(a, vec![[80.0, 0.0, 120.0, 40.0]], RGN_XOR, None);
    assert!(
        xor.as_ref()
            .is_some_and(|rectangles| rectangles.contains(&[0.0, 0.0, 80.0, 40.0]))
    );
    assert!(
        xor.as_ref()
            .is_some_and(|rectangles| rectangles.contains(&[100.0, 0.0, 120.0, 40.0]))
    );
}

#[test]
fn excluding_from_the_default_clip_uses_the_surface() {
    let r = exclude(None, [10.0, 0.0, 20.0, 40.0], Some([0.0, 0.0, 40.0, 40.0]));
    assert_eq!(
        r,
        Some(vec![[0.0, 0.0, 10.0, 40.0], [20.0, 0.0, 40.0, 40.0]])
    );
    assert_eq!(exclude(None, [10.0, 0.0, 20.0, 40.0], None), None);
}

#[test]
fn a_zero_area_rectangle_clears_an_intersection() {
    assert_eq!(
        intersect(Some(vec![[0.0, 0.0, 10.0, 10.0]]), [5.0, 5.0, 5.0, 8.0]),
        Some(vec![])
    );
}

#[test]
fn reaching_the_rectangle_budget_does_not_skip_later_exclusions() {
    let rectangles = vec![[0.0, 0.0, 10.0, 10.0]; MAX_CLIP_RECTS];
    let cuts = vec![[4.0, 4.0, 6.0, 6.0], [0.0, 0.0, 10.0, 10.0]];
    assert_eq!(
        combine(Some(rectangles), cuts, RGN_DIFF, None),
        Some(vec![])
    );
}
