//! Scene images are painted by the same raster consumer as glyphs.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LoadedFont, PageScene, SceneImage,
    SingleFontProfile, layout_document,
};
use joyword_model::{
    AnchorWrap, BlockSeq, Document, HeaderFooterKind, ImageAnchor, InlineKind, InlineObject,
    Measure, PageSetup, Paragraph, SquareWrap, StoryKind, WrapSide, sample_png_1x1,
};
use joyword_pdf::{inspect_pdf, render};
use joyword_render::{Projection, RasterCancellation, RasterLimits, rasterize};

fn pt(value: i64) -> Emu {
    Emu::new(value * Emu::PER_POINT)
}

#[test]
fn a_red_image_paints_a_non_white_pixel() {
    let page = PageScene {
        width: pt(40),
        height: pt(40),
        runs: Vec::new(),
        rects: Vec::new(),
        images: vec![SceneImage {
            media_type: Arc::from("image/png"),
            bytes: Arc::from(sample_png_1x1()),
            x: pt(8),
            y: pt(8),
            width: pt(16),
            height: pt(16),
        }],
    };
    let raster = rasterize(
        &page,
        &[],
        Projection::new(72.0, 1.0).unwrap(),
        RasterLimits::default(),
        &RasterCancellation::default(),
    )
    .unwrap();
    let red = raster
        .rgba()
        .as_chunks::<4>()
        .0
        .iter()
        .any(|pixel| pixel[0] > 200 && pixel[1] < 40 && pixel[2] < 40 && pixel[3] == 255);
    assert!(red, "the admitted red pixel was not painted");
}

#[test]
fn a_floating_anchor_and_a_header_image_share_pdf_and_cpu_geometry() {
    let font = LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .unwrap();
    let mut page = PageSetup::a4_zh_cn();
    let length = |value: i64| Measure::Emu(pt(value).raw());
    page.width = length(200);
    page.height = length(200);
    page.left = length(24);
    page.right = length(24);
    page.top = length(40);
    page.bottom = length(40);
    page.header = length(16);
    page.footer = length(16);
    page.gutter = length(0);
    let mut document = Document::new(page);
    let png = sample_png_1x1();
    let mut body = Paragraph::new(document.ids().node());
    body.push_text("A", None, Arc::default()).unwrap();
    let resource = document.ids().resource();
    document
        .resources_mut()
        .insert(resource, "image/png", Arc::from(png.as_slice()), None);
    let side = 20 * Emu::PER_POINT;
    body.push_inline(
        InlineObject {
            id: document.ids().node(),
            kind: InlineKind::Image {
                resource,
                width: Measure::Emu(side),
                height: Measure::Emu(side),
                anchor: Some(ImageAnchor {
                    x: length(72),
                    y: length(80),
                    wrap: AnchorWrap::None,
                }),
            },
            source: None,
        },
        None,
        Arc::default(),
    );
    let body_id = document.body_id();
    document.story_mut(body_id).unwrap().blocks = BlockSeq::from_vec(vec![body.into()]);
    let mut header = Paragraph::new(document.ids().node());
    let header_resource = document.ids().resource();
    document.resources_mut().insert(
        header_resource,
        "image/png",
        Arc::from(png.as_slice()),
        None,
    );
    header.push_inline(
        InlineObject {
            id: document.ids().node(),
            kind: InlineKind::Image {
                resource: header_resource,
                width: length(12),
                height: length(12),
                anchor: None,
            },
            source: None,
        },
        None,
        Arc::default(),
    );
    let story = document.insert_story(StoryKind::Header, vec![header.into()]);
    document.sections_mut()[0]
        .headers
        .push((HeaderFooterKind::Default, story));
    let laid = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font),
        SingleFontProfile {
            size: pt(12),
            leading: pt(18),
            baseline: pt(14),
            features: FeatureSet::default(),
        },
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    let scene = &laid.pages()[0];
    assert_eq!(scene.images.len(), 2);
    let floating = scene
        .images
        .iter()
        .find(|image| image.x == pt(72))
        .expect("floating image");
    assert_eq!(floating.y, pt(80));
    assert_eq!(floating.width, Emu::new(side));
    let pdf = render(laid.pages(), std::slice::from_ref(&font)).unwrap();
    assert!(inspect_pdf(&pdf).unwrap().image_xobjects >= 1);
    let mut blank = scene.clone();
    blank.images.clear();
    let plain = render(&[blank], std::slice::from_ref(&font)).unwrap();
    assert_ne!(pdf, plain);
    let mut header_only = scene.clone();
    header_only
        .images
        .retain(|image| image.y != floating.y || image.x != floating.x);
    let mut floating_only = scene.clone();
    floating_only
        .images
        .retain(|image| image.x == floating.x && image.y == floating.y);
    assert_eq!(header_only.images.len(), 1);
    assert_eq!(floating_only.images.len(), 1);
    let header_pdf = render(
        std::slice::from_ref(&header_only),
        std::slice::from_ref(&font),
    )
    .unwrap();
    let floating_pdf = render(
        std::slice::from_ref(&floating_only),
        std::slice::from_ref(&font),
    )
    .unwrap();
    assert!(inspect_pdf(&header_pdf).unwrap().image_xobjects >= 1);
    assert!(inspect_pdf(&floating_pdf).unwrap().image_xobjects >= 1);
    assert_ne!(header_pdf, plain);
    assert_ne!(floating_pdf, plain);
    let raster = rasterize(
        scene,
        &[font],
        Projection::new(72.0, 1.0).unwrap(),
        RasterLimits::default(),
        &RasterCancellation::default(),
    )
    .unwrap();
    let px = |length: Emu| (length.raw() as f64 / Emu::PER_INCH as f64 * 72.0).round() as usize;
    let column = px(floating.x) + px(floating.width) / 2;
    let row = px(floating.y) + px(floating.height) / 2;
    let width = raster.width() as usize;
    let pixel = &raster.rgba()[(row * width + column) * 4..][..4];
    assert!(pixel[0] > 200 && pixel[1] < 40 && pixel[3] == 255);
}

#[test]
fn a_wrap_square_rectangle_is_shared_by_pdf_and_the_cpu() {
    let font = LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .unwrap();
    let mut page = PageSetup::a4_zh_cn();
    let length = |value: i64| Measure::Emu(pt(value).raw());
    page.width = length(200);
    page.height = length(360);
    page.left = length(24);
    page.right = length(24);
    page.top = length(40);
    page.bottom = length(40);
    page.header = length(16);
    page.footer = length(16);
    page.gutter = length(0);
    let mut document = Document::new(page);
    let mut paragraph = Paragraph::new(document.ids().node());
    let resource = document.ids().resource();
    document
        .resources_mut()
        .insert(resource, "image/png", Arc::from(sample_png_1x1()), None);
    let side = 60 * Emu::PER_POINT;
    paragraph.push_inline(
        InlineObject {
            id: document.ids().node(),
            kind: InlineKind::Image {
                resource,
                width: Measure::Emu(side),
                height: Measure::Emu(side),
                anchor: Some(ImageAnchor {
                    x: length(24),
                    y: length(40),
                    wrap: AnchorWrap::Square(SquareWrap {
                        top: 0,
                        bottom: 0,
                        left: 0,
                        right: 6 * Emu::PER_POINT,
                        side: WrapSide::Larger,
                    }),
                }),
            },
            source: None,
        },
        None,
        Arc::default(),
    );
    paragraph
        .push_text(
            &"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".repeat(2),
            None,
            Arc::default(),
        )
        .unwrap();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let laid = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font),
        SingleFontProfile {
            size: pt(12),
            leading: pt(18),
            baseline: pt(14),
            features: FeatureSet::default(),
        },
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    let scene = &laid.pages()[0];
    assert_eq!(scene.images.len(), 1);
    let image = &scene.images[0];
    assert_eq!(image.x, pt(24));
    assert_eq!(image.y, pt(40));
    assert_eq!(image.width, Emu::new(side));
    assert_eq!(image.height, Emu::new(side));
    let pdf = render(std::slice::from_ref(scene), std::slice::from_ref(&font)).unwrap();
    assert!(inspect_pdf(&pdf).unwrap().image_xobjects >= 1);
    let mut blank = scene.clone();
    blank.images.clear();
    assert_ne!(
        pdf,
        render(std::slice::from_ref(&blank), std::slice::from_ref(&font)).unwrap()
    );
    let raster = rasterize(
        scene,
        std::slice::from_ref(&font),
        Projection::new(72.0, 1.0).unwrap(),
        RasterLimits::default(),
        &RasterCancellation::default(),
    )
    .unwrap();
    let px = |length: Emu| (length.raw() as f64 / Emu::PER_INCH as f64 * 72.0).round() as usize;
    let column = px(image.x) + px(image.width) / 2;
    let row = px(image.y) + px(image.height) / 2;
    let width = raster.width() as usize;
    let pixel = &raster.rgba()[(row * width + column) * 4..][..4];
    assert!(
        pixel[0] > 200 && pixel[1] < 40 && pixel[3] == 255,
        "cpu pixel {pixel:?} is not the shared rectangle"
    );
}
