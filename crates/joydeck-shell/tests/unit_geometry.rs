extern crate joydeck_shell as joydeck;

use joydeck::edit::{self, Command, Session};
use joydeck::geometry::*;
#[test]
fn rotated_hit_and_gesture_single_history() {
    let mut s = Session::new();
    s.execute(s.id, Command::AddObject(0, edit::object("rect")))
        .unwrap();
    let old = s.deck.clone();
    let objects = &old.slides[0].objects;
    let moved = transform(objects, Mode::Move, (0., 0.), (100., 50.)).unwrap();
    assert_eq!(moved[0].x, 180.);
    s.execute(s.id, Command::ReplaceMany(0, moved)).unwrap();
    s.undo();
    assert!(std::sync::Arc::ptr_eq(&old, &s.deck));
    let o = &objects[0];
    let mut rot = o.clone();
    rot.rotation = 90.;
    assert!(hit(&rot, o.x + o.width / 2., o.y + o.height / 2.));
    assert!(!hit(&rot, o.x, o.y));
}
#[test]
fn composite_failure_preserves_snapshot_and_reorder_identity() {
    let mut s = Session::new();
    let old = s.deck.clone();
    assert!(
        s.execute(
            s.id,
            Command::Batch(vec![Command::AddPage, Command::DeletePage(100)])
        )
        .is_err()
    );
    assert!(std::sync::Arc::ptr_eq(&old, &s.deck));
    s.execute(
        s.id,
        Command::Batch(vec![Command::AddPage, Command::AddPage]),
    )
    .unwrap();
    let ids: Vec<_> = s.deck.slides.iter().map(|s| s.id.clone()).collect();
    s.execute(s.id, Command::ReorderPages(vec![0, 1], 3))
        .unwrap();
    assert_eq!(
        s.deck
            .slides
            .iter()
            .map(|s| s.id.clone())
            .collect::<Vec<_>>(),
        vec![ids[2].clone(), ids[0].clone(), ids[1].clone()]
    );
}
#[test]
fn all_handles_center_and_aspect_constraints() {
    let o = edit::object("rect");
    for handle in [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ] {
        let r = resize(std::slice::from_ref(&o), handle, (20., 15.), false, false).unwrap();
        assert!(r[0].width > 0. && r[0].height > 0.);
        if handle.0 < 0 {
            assert!((r[0].x + r[0].width - o.x - o.width).abs() < 0.01);
        }
        if handle.1 < 0 {
            assert!((r[0].y + r[0].height - o.y - o.height).abs() < 0.01);
        }
    }
    let r = resize(std::slice::from_ref(&o), (1, 1), (20., 10.), true, true).unwrap();
    assert!((r[0].x + r[0].width / 2. - o.x - o.width / 2.).abs() < 0.01);
    assert!((r[0].width / r[0].height - o.width / o.height).abs() < 0.01);
}
