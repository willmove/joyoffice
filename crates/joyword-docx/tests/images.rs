//! Inline picture import and the explicit export refusal (T203).

use joyoffice_opc::{Budget, Method, write_archive};
use joyword_docx::{DiagnosticKind, ExportError, ImageOutcome, import, plan_export};
use joyword_model::{
    Block, ImageAnchor, InlineKind, Measure, StoryKind, sample_png_1x1, sample_png_overwide,
};
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
        anchor,
    } = image.inlines()[0].kind
    else {
        panic!("expected an image inline");
    };
    assert!(anchor.is_none());
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

fn anchor_document(attrs: &str, position: &str, wrap: &str, blip: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}" xmlns:wp="{WP}" xmlns:a="{A}" xmlns:pic="{PIC}"><w:body><w:p><w:r><w:drawing><wp:anchor {attrs}>{position}<wp:extent cx="457200" cy="228600"/>{wrap}<a:graphic><a:graphicData uri="{PIC}"><pic:pic><pic:blipFill><a:blip {blip}/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r></w:p></w:body></w:document>"#
    )
}

fn page_offsets(x: i64, y: i64) -> String {
    format!(
        r#"<wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:posOffset>{x}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>{y}</wp:posOffset></wp:positionV>"#
    )
}

fn image_of(paragraph: &joyword_model::Paragraph) -> (ImageAnchor, Measure, Measure) {
    let InlineKind::Image {
        anchor: Some(anchor),
        width,
        height,
        ..
    } = paragraph.inlines()[0].kind
    else {
        panic!("expected a floating image");
    };
    (anchor, width, height)
}

#[test]
fn a_page_anchor_imports_and_wrap_or_remote_targets_stay_preserved() {
    let png = sample_png_1x1();
    let bytes = package(
        &anchor_document(
            r#"behindDoc="0" simplePos="0""#,
            &page_offsets(914_400, 1_828_800),
            "<wp:wrapNone/>",
            r#"r:embed="rId1""#,
        ),
        &image_rel("media/pixel.png", false),
        Some(&png),
    );
    let result = import(&bytes, &Budget::default()).unwrap();
    assert!(!result.document.validate().has_errors());
    let paragraph = result
        .document
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap();
    let (anchor, width, height) = image_of(paragraph);
    assert_eq!(
        anchor,
        ImageAnchor {
            x: Measure::Emu(914_400),
            y: Measure::Emu(1_828_800),
        }
    );
    assert_eq!(width, Measure::Emu(457_200));
    assert_eq!(height, Measure::Emu(228_600));
    assert!(!result.editability.locked().contains(&paragraph.id()));

    let simple = package(
        &anchor_document(
            r#"behindDoc="0" simplePos="1""#,
            r#"<wp:simplePos x="100" y="200"/>"#,
            "<wp:wrapNone/>",
            r#"r:embed="rId1""#,
        ),
        &image_rel("media/pixel.png", false),
        Some(&png),
    );
    let result = import(&simple, &Budget::default()).unwrap();
    let paragraph = result
        .document
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap();
    let (anchor, _, _) = image_of(paragraph);
    assert_eq!(anchor.x, Measure::Emu(100));
    assert_eq!(anchor.y, Measure::Emu(200));

    let remote = package(
        &anchor_document(
            r#"behindDoc="0" simplePos="0""#,
            &page_offsets(0, 0),
            "<wp:wrapNone/>",
            r#"r:link="rId1""#,
        ),
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
    assert!(result.diagnostics.iter().all(|item| match &item.kind {
        DiagnosticKind::Image { relationship, .. } => {
            !relationship.contains("example.invalid") && !relationship.contains("http")
        }
        _ => true,
    }));

    for (position, wrap, attrs) in [
        (
            page_offsets(0, 0),
            "<wp:wrapSquare/>",
            r#"behindDoc="0" simplePos="0""#,
        ),
        (
            page_offsets(0, 0),
            "<wp:wrapTight/>",
            r#"behindDoc="0" simplePos="0""#,
        ),
        (
            page_offsets(0, 0),
            "<wp:wrapThrough/>",
            r#"behindDoc="0" simplePos="0""#,
        ),
        (
            page_offsets(0, 0),
            "<wp:wrapTopAndBottom/>",
            r#"behindDoc="0" simplePos="0""#,
        ),
        (
            page_offsets(0, 0),
            "<wp:wrapNone/>",
            r#"behindDoc="1" simplePos="0""#,
        ),
        (
            r#"<wp:positionH relativeFrom="column"><wp:posOffset>0</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>0</wp:posOffset></wp:positionV>"#.to_owned(),
            "<wp:wrapNone/>",
            r#"behindDoc="0" simplePos="0""#,
        ),
        (
            r#"<wp:positionH relativeFrom="page"><wp:align>left</wp:align></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>0</wp:posOffset></wp:positionV>"#.to_owned(),
            "<wp:wrapNone/>",
            r#"behindDoc="0" simplePos="0""#,
        ),
    ] {
        let bytes = package(
            &anchor_document(attrs, &position, wrap, r#"r:embed="rId1""#),
            &image_rel("media/pixel.png", false),
            Some(&png),
        );
        let result = import(&bytes, &Budget::default()).unwrap();
        assert!(
            result.document.resources().is_empty(),
            "{attrs} {wrap} imported a resource"
        );
        let paragraph = result.document.body().blocks.get(0).unwrap();
        assert!(result.editability.locked().contains(&paragraph.id()));
        assert!(paragraph.as_paragraph().unwrap().inlines().iter().any(
            |inline| matches!(inline.kind, InlineKind::Preserved(_))
        ));
        assert!(result.diagnostics.iter().any(|item| matches!(
            item.kind,
            DiagnosticKind::Image {
                outcome: ImageOutcome::Unsupported,
                ..
            }
        )));
        assert!(result.diagnostics.iter().all(|item| match &item.kind {
            DiagnosticKind::Image { relationship, .. } => {
                !relationship.contains("http") && !relationship.contains("example.invalid")
            }
            _ => true,
        }));
    }
}

#[test]
fn an_anchor_refuses_svg_emf_wmf_and_ole_without_fetching() {
    let cases = [
        (
            "image/svg+xml",
            b"<svg xmlns='http://example.invalid/x'></svg>".to_vec(),
        ),
        ("image/x-emf", {
            let mut bytes = vec![0u8; 44];
            bytes[40..44].copy_from_slice(b" EMF");
            bytes
        }),
        ("image/x-wmf", vec![0xD7, 0xCD, 0xC6, 0x9A, 0, 0, 0, 0]),
        (
            "application/vnd.ms-office.activeX",
            vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1],
        ),
    ];
    for (content_type, media) in cases {
        let types = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="bin" ContentType="{content_type}"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#
        );
        let document = anchor_document(
            r#"behindDoc="0" simplePos="0""#,
            &page_offsets(0, 0),
            "<wp:wrapNone/>",
            r#"r:embed="rId1""#,
        );
        let rels = image_rel("media/pixel.bin", false);
        let names = [
            "[Content_Types].xml",
            "_rels/.rels",
            "word/document.xml",
            "word/_rels/document.xml.rels",
            "word/media/pixel.bin",
        ];
        let package_rels = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#
        );
        let owned = [
            types.into_bytes(),
            package_rels.into_bytes(),
            document.into_bytes(),
            rels.into_bytes(),
            media,
        ];
        let items: Vec<(&str, &[u8], Method)> = names
            .iter()
            .zip(owned.iter())
            .map(|(name, data)| (*name, data.as_slice(), Method::Stored))
            .collect();
        let bytes = write_archive(&items).unwrap();
        let result = import(&bytes, &Budget::default()).unwrap();
        assert!(result.document.resources().is_empty(), "{content_type}");
        assert!(result.diagnostics.iter().any(|item| matches!(
            item.kind,
            DiagnosticKind::Image {
                outcome: ImageOutcome::Active,
                ..
            }
        )));
        assert!(
            result
                .diagnostics
                .iter()
                .all(|item| !format!("{item:?}").contains("example.invalid"))
        );
        let paragraph = result.document.body().blocks.get(0).unwrap().id();
        assert!(result.editability.locked().contains(&paragraph));
    }
}

#[test]
fn a_header_and_footer_inline_image_imports_with_the_part() {
    let png = sample_png_1x1();
    let types = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>"#;
    let package_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#
    );
    let document_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/></Relationships>"#
    );
    let picture = format!(
        r#"<w:r><w:drawing><wp:inline><wp:extent cx="152400" cy="152400"/><a:graphic><a:graphicData uri="{PIC}"><pic:pic><pic:blipFill><a:blip r:embed="rId1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
    );
    let header = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:hdr xmlns:w="{W}" xmlns:r="{R}" xmlns:wp="{WP}" xmlns:a="{A}" xmlns:pic="{PIC}"><w:p>{picture}</w:p></w:hdr>"#
    );
    let footer = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:ftr xmlns:w="{W}" xmlns:r="{R}" xmlns:wp="{WP}" xmlns:a="{A}" xmlns:pic="{PIC}"><w:p>{picture}</w:p></w:ftr>"#
    );
    let story_rels = image_rel("media/pixel.png", false);
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="{W}" xmlns:r="{R}"><w:body><w:p><w:r><w:t>Body</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rId2"/><w:footerReference w:type="default" r:id="rId3"/></w:sectPr></w:body></w:document>"#
    );
    let names = [
        "[Content_Types].xml",
        "_rels/.rels",
        "word/document.xml",
        "word/_rels/document.xml.rels",
        "word/header1.xml",
        "word/_rels/header1.xml.rels",
        "word/footer1.xml",
        "word/_rels/footer1.xml.rels",
        "word/media/pixel.png",
    ];
    let header_rels = story_rels.clone();
    let footer_rels = story_rels;
    let owned = [
        types.as_bytes().to_vec(),
        package_rels.into_bytes(),
        document.into_bytes(),
        document_rels.into_bytes(),
        header.into_bytes(),
        header_rels.into_bytes(),
        footer.into_bytes(),
        footer_rels.into_bytes(),
        png.clone(),
    ];
    let items: Vec<(&str, &[u8], Method)> = names
        .iter()
        .zip(owned.iter())
        .map(|(name, data)| (*name, data.as_slice(), Method::Stored))
        .collect();
    let bytes = write_archive(&items).unwrap();
    let result = import(&bytes, &Budget::default()).unwrap();
    assert!(!result.document.validate().has_errors());
    for (kind, refs) in [
        (StoryKind::Header, &result.document.sections()[0].headers),
        (StoryKind::Footer, &result.document.sections()[0].footers),
    ] {
        let story = result.document.story(refs[0].1).unwrap();
        assert_eq!(story.kind, kind);
        let paragraph = story.blocks.get(0).unwrap().as_paragraph().unwrap();
        let InlineKind::Image {
            resource,
            anchor,
            width,
            height,
        } = paragraph.inlines()[0].kind
        else {
            panic!("expected an inline image in {kind}");
        };
        assert!(anchor.is_none());
        assert_eq!(width, Measure::Emu(152_400));
        assert_eq!(height, Measure::Emu(152_400));
        assert_eq!(
            result.document.resources().get(resource).unwrap().data(),
            png.as_slice()
        );
    }
}
