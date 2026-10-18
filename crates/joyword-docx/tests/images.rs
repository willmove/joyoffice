//! Inline picture import and the explicit export refusal (T203).

use joyoffice_opc::{Budget, Method, write_archive};
use joyword_docx::{DiagnosticKind, ExportError, ImageOutcome, import, plan_export};
use joyword_model::{Block, InlineKind, Measure, sample_png_1x1, sample_png_overwide};
use std::sync::Arc;

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const PIC: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const IMAGE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

fn content_types() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_owned()
}

fn package(document: &str, rels: &str, media: Option<&[u8]>) -> Vec<u8> {
    let mut names = vec![
        "[Content_Types].xml",
        "_rels/.rels",
        "word/document.xml",
        "word/_rels/document.xml.rels",
    ];
    let package_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#
    );
    let mut owned = vec![
        content_types().into_bytes(),
        package_rels.into_bytes(),
        document.as_bytes().to_vec(),
        rels.as_bytes().to_vec(),
    ];
    if let Some(media) = media {
        names.push("word/media/pixel.png");
        owned.push(media.to_vec());
    }
    let items: Vec<(&str, &[u8], Method)> = names
        .iter()
        .zip(owned.iter())
        .map(|(name, data)| (*name, data.as_slice(), Method::Stored))
        .collect();
    write_archive(&items).unwrap()
}

fn drawing(blip: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}" xmlns:wp="{WP}" xmlns:a="{A}" xmlns:pic="{PIC}"><w:body><w:p><w:r><w:drawing><wp:inline><wp:extent cx="9525" cy="9525"/><a:graphic><a:graphicData uri="{PIC}"><pic:pic><pic:blipFill><a:blip {blip}/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p><w:p><w:r><w:t>hello</w:t></w:r></w:p></w:body></w:document>"#
    )
}

fn image_rel(target: &str, external: bool) -> String {
    let mode = if external {
        r#" TargetMode="External""#
    } else {
        ""
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"><Relationship Id="rId1" Type="{IMAGE_REL}" Target="{target}"{mode}/></Relationships>"#
    )
}

#[test]
fn an_inline_drawing_imports_the_raster_and_round_trips_beside_a_text_edit() {
    let png = sample_png_1x1();
    let bytes = package(
        &drawing(r#"r:embed="rId1""#),
        &image_rel("media/pixel.png", false),
        Some(&png),
    );
    let result = import(&bytes, &Budget::default()).unwrap();
    assert!(!result.document.validate().has_errors());
    let image = result
        .document
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap();
    let InlineKind::Image {
        resource,
        width,
        height,
    } = image.inlines()[0].kind
    else {
        panic!("expected an image inline");
    };
    assert_eq!(width, Measure::Emu(9525));
    assert_eq!(height, Measure::Emu(9525));
    assert_eq!(
        result.document.resources().get(resource).unwrap().data(),
        png.as_slice()
    );
    let mut model = result.document.clone();
    let body = model.body_id();
    model.story_mut(body).unwrap().blocks.update(1, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        Arc::make_mut(paragraph)
            .insert_text(joyword_model::ScalarOffset(5), "!")
            .unwrap();
    });
    let plan = plan_export(&result.backing, &model.snapshot(), &Budget::default()).unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    let saved_image = saved
        .document
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap();
    assert!(matches!(
        saved_image.inlines()[0].kind,
        InlineKind::Image { .. }
    ));
    assert_eq!(
        saved
            .document
            .body()
            .blocks
            .get(1)
            .unwrap()
            .as_paragraph()
            .unwrap()
            .plain_text(|_| String::new()),
        "hello!"
    );
}

#[test]
fn a_remote_link_is_not_fetched_and_an_oversize_header_is_kept() {
    let remote = package(
        &drawing(r#"r:link="rId1""#),
        &image_rel("http://example.invalid/secret.png", true),
        None,
    );
    let result = import(&remote, &Budget::default()).unwrap();
    assert!(result.document.resources().is_empty());
    assert!(result.diagnostics.iter().any(|item| matches!(
        &item.kind,
        DiagnosticKind::Image {
            outcome: ImageOutcome::Remote,
            relationship,
        } if !relationship.contains("example.invalid") && !relationship.contains("http")
    )));

    let oversize = sample_png_overwide().expect("overwide png encodes");
    let bytes = package(
        &drawing(r#"r:embed="rId1""#),
        &image_rel("media/pixel.png", false),
        Some(&oversize),
    );
    let result = import(&bytes, &Budget::default()).unwrap();
    assert!(result.document.resources().is_empty());
    assert!(result.diagnostics.iter().any(|item| matches!(
        item.kind,
        DiagnosticKind::Image {
            outcome: ImageOutcome::Oversize,
            ..
        }
    )));
    let paragraph = result.document.body().blocks.get(0).unwrap().id();
    assert!(result.editability.locked().contains(&paragraph));
}

#[test]
fn a_new_image_resource_is_an_explicit_export_refusal() {
    let png = sample_png_1x1();
    let bytes = package(
        &drawing(r#"r:embed="rId1""#),
        &image_rel("media/pixel.png", false),
        Some(&png),
    );
    let result = import(&bytes, &Budget::default()).unwrap();
    let mut model = result.document.clone();
    let id = model.ids().resource();
    model
        .resources_mut()
        .insert(id, "image/png", Arc::<[u8]>::from(png), None);
    let error = plan_export(&result.backing, &model.snapshot(), &Budget::default()).unwrap_err();
    assert!(matches!(
        error,
        ExportError::Unsupported { reason, .. }
            if reason == "image resources changed and media parts are not rewritten"
    ));

    let mut edited = result.document.clone();
    let body = edited.body_id();
    edited.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        Arc::make_mut(paragraph)
            .insert_text(joyword_model::ScalarOffset(0), "x")
            .unwrap();
    });
    let error = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap_err();
    match error {
        ExportError::Unsupported { reason, .. } => {
            assert!(
                reason.contains("inline image") || reason.contains("image"),
                "{reason}"
            );
        }
        other => panic!("{other:?}"),
    }
}
