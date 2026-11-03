use super::*;
use joyword_foundation::Emu;
#[test]
fn bundled_emoji_has_scalable_grinning_face_and_a_distinct_identity() {
    let emoji = emoji().expect("validated bundled outline font");
    let shaped = joyword_layout::shape(
        emoji,
        "😀",
        Emu::new(12 * Emu::PER_POINT),
        Default::default(),
    );
    assert!(shaped.missing.is_empty());
    assert!(shaped.glyphs.iter().all(|g| g.glyph_id != 0));
    assert!(emoji.key().index() == 0);
}
#[test]
fn mixed_text_uses_the_same_fallback_in_layout_screen_and_pdf() {
    let primary = LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .unwrap();
    let source = crate::document_canvas::source_with_text(primary, "AV 😀 fi").unwrap();
    assert!(source.layout.diagnostics().is_empty());
    source
        .layout
        .ensure_current_faces(
            &source.session.document().snapshot(),
            faces(&source.font),
            source.profile,
        )
        .unwrap();
    assert!(
        source
            .layout
            .ensure_current_faces(
                &source.session.document().snapshot(),
                FaceSet::uniform(&source.font),
                source.profile
            )
            .is_err()
    );
    assert!(
        source
            .layout
            .pages()
            .iter()
            .flat_map(|p| &p.runs)
            .any(|r| r.font == emoji().unwrap().key() && r.text.contains('😀'))
    );
    let projection = joyword_render::Projection::new(96., 1.).unwrap();
    let pages =
        crate::document_canvas::raster_pages(&source, projection, &Default::default()).unwrap();
    assert_eq!(pages.len(), 1);
    let bytes = crate::export::scene_pdf(
        &source.layout,
        &source.font,
        source.session.document(),
        None,
        0..1,
    )
    .unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.windows(10).any(|s| s == b"/ToUnicode"));
}
