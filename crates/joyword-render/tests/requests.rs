use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LoadedFont, SingleFontProfile,
    layout_document,
};
use joyword_model::{Document, PageSetup};
use joyword_render::Projection;
use joyword_render::requests::{RasterRequestGate, RasterRequestKey};
use std::sync::Arc;

fn key(zoom: f64) -> RasterRequestKey {
    let font = LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .unwrap();
    let document = Document::new(PageSetup::a4_zh_cn());
    let snapshot = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font),
        SingleFontProfile {
            size: Emu::new(12 * Emu::PER_POINT),
            leading: Emu::new(18 * Emu::PER_POINT),
            baseline: Emu::new(14 * Emu::PER_POINT),
            features: FeatureSet::default(),
        },
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    RasterRequestKey {
        layout: snapshot.key().clone(),
        projection: Projection::new(96.0, zoom).unwrap(),
    }
}
#[test]
fn idle_or_failed_requests_are_not_rescheduled_without_new_input() {
    let mut gate = RasterRequestGate::default();
    let key = key(1.0);
    let ticket = gate.request(key.clone()).unwrap();
    for _ in 0..1000 {
        assert!(gate.request(key.clone()).is_none());
    }
    assert!(gate.accepts(&ticket, &key));
}
#[test]
fn late_results_from_prior_zoom_are_rejected_even_after_returning_to_it() {
    let mut gate = RasterRequestGate::default();
    let first = key(1.0);
    let other = key(1.5);
    let t1 = gate.request(first.clone()).unwrap();
    let t2 = gate.request(other.clone()).unwrap();
    assert!(!gate.accepts(&t1, &first));
    assert!(gate.accepts(&t2, &other));
    let t3 = gate.request(first.clone()).unwrap();
    assert!(!gate.accepts(&t1, &first));
    assert!(!gate.accepts(&t2, &other));
    assert!(gate.accepts(&t3, &first));
}
#[test]
fn current_document_font_profile_or_dpi_must_match_at_handoff() {
    let mut gate = RasterRequestGate::default();
    let original = key(1.0);
    let ticket = gate.request(original.clone()).unwrap();
    let mut other = original.clone();
    other.projection = Projection::new(192.0, 1.0).unwrap();
    assert!(!gate.accepts(&ticket, &other));
    other = original.clone();
    other.layout.profile.size = Emu::new(24 * Emu::PER_POINT);
    assert!(!gate.accepts(&ticket, &other));
    // A different actual face identity invalidates a same-version request.
    let bytes: Arc<[u8]> = Arc::from(
        include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
    );
    let mut changed = bytes.to_vec();
    changed.push(0);
    other = original.clone();
    other.layout.font = LoadedFont::from_bytes(changed.into(), 0).unwrap().key();
    assert!(!gate.accepts(&ticket, &other));
}
