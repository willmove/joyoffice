#![allow(clippy::unwrap_used, reason = "Test assertions")]
use crate::{SlideBinding, export, export_preserving, import};
use joydeck_model::{
    PhType, Placeholder, Presentation, Shape, ShapeId, SlideId, TextBody, text::RunKind,
};
use std::sync::Arc;

fn placeholder(p: &mut Presentation, kind: PhType, text: &str, field: Option<&str>) -> Shape {
    let mut body = TextBody::from_text(text);
    if let Some(field) = field {
        body.paragraphs[0].runs[0].kind = RunKind::Field {
            field: field.into(),
        };
    }
    Shape {
        id: ShapeId(p.alloc_id().unwrap()),
        ph: Some(Placeholder {
            kind,
            idx: 10,
            ..Default::default()
        }),
        text: Some(body),
        ..Default::default()
    }
}
fn fixture_with_date(format: &str) -> Vec<u8> {
    let mut p = Presentation::default();
    let first = vec![
        placeholder(&mut p, PhType::Footer, "First footer 中文", None),
        placeholder(&mut p, PhType::Date, "Fixed date", None),
        placeholder(&mut p, PhType::Header, "Header", None),
    ];
    Arc::make_mut(&mut p.slides[0]).shapes = first;
    let mut second = (*p.slides[0]).clone();
    second.id = SlideId(p.alloc_id().unwrap());
    second.shapes = vec![
        placeholder(&mut p, PhType::Footer, "Second footer 😀", None),
        placeholder(&mut p, PhType::Date, "Saved date", Some(format)),
        placeholder(&mut p, PhType::SlideNum, "2", Some("slidenum")),
    ];
    p.slides.push(Arc::new(second));
    export(&p).unwrap()
}
fn fixture() -> Vec<u8> {
    fixture_with_date("datetime13")
}

#[test]
fn footer_summary_keeps_header_and_automatic_date_format_without_flattening_slides() {
    let p = import(&fixture()).unwrap();
    let hf = &p.header_footer;
    assert!(hf.header);
    assert!(hf.footer && hf.date && hf.slide_number);
    assert_eq!(hf.footer_text, "First footer 中文");
    assert_eq!(hf.date_text, "Fixed date");
    assert_eq!(hf.date_format, "datetime13");
    assert_eq!(
        p.slides[1].shapes[0].text.as_ref().unwrap().text(),
        "Second footer 😀"
    );
    assert_eq!(
        p.slides[1].shapes[1].text.as_ref().unwrap().paragraphs[0].runs[0].kind,
        RunKind::Field {
            field: "datetime13".into()
        }
    );
}
#[test]
fn changing_one_footer_preserves_other_slide_and_automatic_date()
-> Result<(), Box<dyn std::error::Error>> {
    let source = with_visibility_xml(
        &fixture(),
        "ppt/slides/slide",
        "p:spTree",
        r#"<!--shape tree marker--><p:extLst><p:ext uri="urn:joydeck:test"><test:data xmlns:test="urn:joydeck:test" test:space="a&#9;b&#13;c">before<![CDATA[<&]]><?joydeck-test keep?><!--opaque--><test:child/>after</test:data></p:ext></p:extLst>"#,
    )?;
    let source = rewrite_xml(&source, "ppt/slides/slide1.xml", |xml| {
        xml.replace("<p:sld ", "<?vendor before?><!--outer--><p:sld ") + "<?vendor after?>"
    })?;
    let base = import(&source).unwrap();
    let bindings: Vec<_> = base
        .slides
        .iter()
        .map(|s| SlideBinding {
            slide: s.id,
            source: s.id,
            shapes: s.shapes.iter().map(|sh| (sh.id, sh.id)).collect(),
        })
        .collect();
    let mut current = base.clone();
    Arc::make_mut(&mut current.slides[0]).shapes[0]
        .text
        .as_mut()
        .unwrap()
        .paragraphs[0]
        .runs[0]
        .text = "Edited first footer".into();
    let saved = export_preserving(&source, &base, &current, &bindings).unwrap();
    let reopened = import(&saved).unwrap();
    assert_eq!(reopened.header_footer.footer_text, "Edited first footer");
    assert_eq!(reopened.header_footer.date_format, "datetime13");
    assert_eq!(reopened.slides[1], base.slides[1]);
    let original_pkg = crate::opc::Package::open(&source).unwrap();
    let saved_pkg = crate::opc::Package::open(&saved).unwrap();
    let before = original_pkg
        .xml("ppt/slides/slide1.xml")
        .ok_or("missing input slide")?;
    let after = saved_pkg
        .xml("ppt/slides/slide1.xml")
        .ok_or("missing output slide")?;
    assert_eq!(before.before_root, ["<?vendor before?>", "<!--outer-->"]);
    assert_eq!(before.after_root, ["<?vendor after?>"]);
    assert_eq!(before.before_root, after.before_root);
    assert_eq!(before.after_root, after.after_root);
    let before = before
        .root
        .path(&["cSld", "spTree"])
        .ok_or("missing input shape tree")?;
    let after = after
        .root
        .path(&["cSld", "spTree"])
        .ok_or("missing output shape tree")?;
    assert_eq!(
        after.elements().next().map(|e| e.local()),
        Some("nvGrpSpPr")
    );
    assert_eq!(after.elements().nth(1).map(|e| e.local()), Some("grpSpPr"));
    assert_eq!(after.elements().last().map(|e| e.local()), Some("extLst"));
    for tag in ["nvGrpSpPr", "grpSpPr", "extLst"] {
        assert_eq!(after.child(tag), before.child(tag));
    }
    assert_eq!(
        after.children.iter().rev().nth(1),
        before.children.iter().rev().nth(1)
    );
    assert!(
        matches!(after.children.iter().rev().nth(1), Some(crate::xml::Node::Markup(markup)) if markup == "<!--shape tree marker-->")
    );
    assert_eq!(
        original_pkg.get("ppt/slides/slide2.xml"),
        saved_pkg.get("ppt/slides/slide2.xml")
    );
    assert_eq!(import(&source).unwrap(), base);
    Ok(())
}

#[test]
fn automatic_footer_date_summary_keeps_field_identifiers_and_ignores_other_fields() {
    for format in (1..=13).map(|n| format!("datetime{n}")).chain(
        ["datetime", "datetimeFigureOut", "datetime99"]
            .into_iter()
            .map(str::to_owned),
    ) {
        let p = import(&fixture_with_date(&format)).unwrap();
        assert_eq!(p.header_footer.date_format, format);
        assert_eq!(p.header_footer.date_text, "Fixed date");
    }
    let p = import(&fixture_with_date("slidenum")).unwrap();
    assert!(p.header_footer.date_format.is_empty());
}

fn with_layout_footer_flag(
    source: &[u8],
    enabled: bool,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    with_visibility_xml(
        source,
        "ppt/slideLayouts/slideLayout",
        "p:sldLayout",
        &format!("<p:hf ftr=\"{}\"/>", u8::from(enabled)),
    )
}

fn with_visibility_xml(
    source: &[u8],
    prefix: &str,
    tag: &str,
    fragment: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    rewrite_xml(source, prefix, |xml| {
        assert!(!xml.contains("<p:hf"));
        xml.replace(&format!("</{tag}>"), &format!("{fragment}</{tag}>"))
    })
}

fn rewrite_xml(
    source: &[u8],
    prefix: &str,
    change: impl Fn(String) -> String,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::io::{Cursor, Read, Write};
    let mut input = zip::ZipArchive::new(Cursor::new(source))?;
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..input.len() {
        let mut entry = input.by_index(i)?;
        let name = entry.name()?.into_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        if name.starts_with(prefix) && name.ends_with(".xml") {
            bytes = change(String::from_utf8(bytes)?).into_bytes();
        }
        output.start_file(name, zip::write::SimpleFileOptions::default())?;
        output.write_all(&bytes)?;
    }
    Ok(output.finish()?.into_inner())
}

#[test]
fn preserve_rebind_uses_undeclared_layout_relationships_and_keeps_placeholder_inheritance()
-> Result<(), Box<dyn std::error::Error>> {
    let mut doc = Presentation::default();
    let original = Arc::make_mut(&mut doc.slides[0])
        .shapes
        .iter_mut()
        .find(|shape| shape.ph.is_some())
        .ok_or("first placeholder missing")?;
    let id = original.id;
    let transform = joydeck_model::Xfrm::new(12.0, 13.0, 150.0, 80.0);
    original.xfrm = Some(transform);
    let source = rewrite_xml(&export(&doc)?, "ppt/slideMasters/slideMaster", |xml| {
        if let (Some(start), Some(end)) = (
            xml.find("<p:sldLayoutIdLst>"),
            xml.find("</p:sldLayoutIdLst>"),
        ) {
            format!(
                "{}{}",
                &xml[..start],
                &xml[end + "</p:sldLayoutIdLst>".len()..]
            )
        } else {
            xml
        }
    })?;
    let base = import(&source)?;
    let layout = base.masters[0]
        .layouts
        .iter()
        .find(|item| {
            item.id != base.slides[0].layout
                && item
                    .shapes
                    .iter()
                    .any(|shape| shape.ph.is_some() && shape.xfrm.is_some())
        })
        .ok_or("alternate layout missing")?;
    let mut edited = base.clone();
    let slide = Arc::make_mut(&mut edited.slides[0]);
    slide.layout = layout.id;
    slide
        .shapes
        .iter_mut()
        .find(|shape| shape.id == id)
        .ok_or("source placeholder missing")?
        .xfrm = None;
    let bindings = [SlideBinding {
        slide: slide.id,
        source: slide.id,
        shapes: slide
            .shapes
            .iter()
            .map(|shape| (shape.id, shape.id))
            .collect(),
    }];
    let output = export_preserving(&source, &base, &edited, &bindings)?;
    let reopened = import(&output)?;
    assert_eq!(
        reopened
            .layout(reopened.slides[0].layout)
            .map(|(_, layout)| &layout.name),
        Some(&layout.name)
    );
    assert_eq!(reopened.slides[0].shapes.len(), base.slides[0].shapes.len());
    for (original, shape) in base.slides[0].shapes.iter().zip(&reopened.slides[0].shapes) {
        assert_eq!(original.ph, shape.ph);
        if original.id == id {
            assert_eq!(shape.xfrm, None);
            let ctx = joydeck_model::resolve::Ctx::for_slide(&reopened, &reopened.slides[0])
                .ok_or("placeholder resolution context missing")?;
            let (layout, master) = joydeck_model::resolve::parents(&ctx, shape);
            assert_eq!(
                joydeck_model::resolve::xfrm(&ctx, shape),
                layout
                    .and_then(|layer| layer.xfrm)
                    .or_else(|| master.and_then(|layer| layer.xfrm))
                    .ok_or("placeholder has no inherited transform")?
            );
            assert_ne!(joydeck_model::resolve::xfrm(&ctx, shape), transform);
        } else {
            assert_eq!(original.xfrm, shape.xfrm);
        }
    }
    Ok(())
}

#[test]
fn layout_footer_visibility_is_retained_during_unrelated_slide_edit()
-> Result<(), Box<dyn std::error::Error>> {
    let mut p = Presentation::default();
    Arc::make_mut(&mut p.slides[0]).shapes.clear();
    let layout = p
        .masters
        .iter()
        .flat_map(|m| &m.layouts)
        .find(|l| l.id == p.slides[0].layout)
        .ok_or("layout missing")?;
    let footers: Vec<_> = layout
        .shapes
        .iter()
        .filter(|s| s.ph_type() == Some(PhType::Footer))
        .collect();
    assert_eq!(footers.len(), 1);
    assert!(footers[0].xfrm.is_some());
    let plain = export(&p)?;
    let disabled = with_layout_footer_flag(&plain, false)?;
    let enabled = with_layout_footer_flag(&plain, true)?;
    assert_ne!(import(&disabled)?, import(&enabled)?);
    for source in [disabled, enabled] {
        let base = import(&source)?;
        assert!(base.slides[0].shapes.is_empty());
        let bindings = [SlideBinding {
            slide: base.slides[0].id,
            source: base.slides[0].id,
            shapes: Default::default(),
        }];
        let mut current = base.clone();
        Arc::make_mut(&mut current.slides[0]).name = "Renamed slide".into();
        let saved = export_preserving(&source, &base, &current, &bindings)?;
        let original_pkg = crate::opc::Package::open(&source)?;
        let layout_xml = std::str::from_utf8(
            original_pkg
                .get("ppt/slideLayouts/slideLayout1.xml")
                .ok_or("source layout missing")?,
        )?;
        assert!(layout_xml.contains("<p:hf ftr="));
        let saved_pkg = crate::opc::Package::open(&saved)?;
        assert_eq!(
            original_pkg
                .get("ppt/slideLayouts/slideLayout1.xml")
                .ok_or("source layout missing")?,
            saved_pkg
                .get("ppt/slideLayouts/slideLayout1.xml")
                .ok_or("saved layout missing")?
        );
        let reopened = import(&saved)?;
        assert!(reopened.slides[0].shapes.is_empty());
        assert_eq!(reopened.slides[0].name, "Renamed slide");
    }
    Ok(())
}

#[test]
fn explicit_master_and_layout_visibility_survives_native_and_pptx_roundtrips()
-> Result<(), Box<dyn std::error::Error>> {
    let plain = export(&Presentation::default())?;
    let master = with_visibility_xml(
        &plain,
        "ppt/slideMasters/slideMaster",
        "p:sldMaster",
        "<p:hf dt=\"false\" ftr=\"0\" hdr=\"true\" sldNum=\"1\"/>",
    )?;
    let source = with_layout_footer_flag(&master, true)?;
    let model = import(&source)?;
    let master_flags = model.masters[0]
        .placeholder_visibility
        .as_ref()
        .ok_or("master flags missing")?;
    assert_eq!(master_flags.date, Some(false));
    assert_eq!(master_flags.footer, Some(false));
    assert_eq!(master_flags.header, Some(true));
    assert_eq!(master_flags.slide_number, Some(true));
    let layout_flags = model.masters[0].layouts[0]
        .placeholder_visibility
        .as_ref()
        .ok_or("layout flags missing")?;
    assert_eq!(layout_flags.footer, Some(true));
    assert_eq!(layout_flags.date, None);
    let native: Presentation = serde_json::from_slice(&serde_json::to_vec(&model)?)?;
    assert_eq!(native, model);
    let reopened = import(&export(&model)?)?;
    assert_eq!(
        reopened.masters[0].placeholder_visibility.as_ref(),
        Some(master_flags)
    );
    assert_eq!(
        reopened.masters[0].layouts[0]
            .placeholder_visibility
            .as_ref(),
        Some(layout_flags)
    );
    assert_ne!(
        import(&with_layout_footer_flag(&plain, false)?)?,
        import(&with_layout_footer_flag(&plain, true)?)?
    );
    Ok(())
}

#[test]
fn unrecognized_layout_visibility_is_not_treated_as_permission()
-> Result<(), Box<dyn std::error::Error>> {
    let plain = export(&Presentation::default())?;
    for fragment in [
        "<p:hf ftr=\"maybe\"/>",
        "<p:hf ftr=\"1\" custom=\"yes\"/>",
        "<p:hf><p:extLst/></p:hf>",
        "<p:hf/><p:hf/>",
        "<p:hf xmlns:p=\"urn:not-presentation\" ftr=\"1\"/>",
    ] {
        let source = with_visibility_xml(
            &plain,
            "ppt/slideLayouts/slideLayout",
            "p:sldLayout",
            fragment,
        )?;
        let model = import(&source)?;
        assert!(model.masters[0].layouts[0].placeholder_visibility.is_none());
        let mut current = model.clone();
        Arc::make_mut(&mut current.slides[0]).name = "Safe rename".into();
        let bindings = [SlideBinding {
            slide: model.slides[0].id,
            source: model.slides[0].id,
            shapes: Default::default(),
        }];
        let saved = export_preserving(&source, &model, &current, &bindings)?;
        let original = crate::opc::Package::open(&source)?;
        let reopened = crate::opc::Package::open(&saved)?;
        assert_eq!(
            original.get("ppt/slideLayouts/slideLayout1.xml"),
            reopened.get("ppt/slideLayouts/slideLayout1.xml")
        );
    }
    let empty = import(&with_visibility_xml(
        &plain,
        "ppt/slideLayouts/slideLayout",
        "p:sldLayout",
        "<p:hf/>",
    )?)?;
    assert_eq!(
        empty.masters[0].layouts[0].placeholder_visibility,
        Some(Default::default())
    );
    assert!(
        import(&plain)?.masters[0].layouts[0]
            .placeholder_visibility
            .is_none()
    );
    Ok(())
}
