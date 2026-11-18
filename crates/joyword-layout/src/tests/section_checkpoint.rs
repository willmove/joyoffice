use super::*;

#[test]
fn section_reflow_matches_complete_body_fields_running_stories_anchor_hits_and_edits()
-> Result<(), Box<dyn std::error::Error>> {
    use joyword_model::{
        PartKind,
        para::{Anchor, Float, Wrap},
    };
    let mut doc = Document::new();
    doc.styles.default_chr.font = Some("Source Sans 3".into());
    doc.body.clear();
    doc.last_section.page_w = 360.;
    doc.last_section.page_h = 276.;
    doc.last_section.margin_top = 48.;
    doc.last_section.margin_bottom = 42.;
    doc.last_section.margin_left = 42.;
    doc.last_section.margin_right = 34.;
    doc.last_section.header = 12.;
    doc.last_section.footer = 12.;
    let paragraph =
        |s: &str| joyword_model::para_block(Paragraph::with_text(s, Default::default()));
    let field = |name: &str| InlineObject::Field {
        instr: name.into(),
        result: "999".into(),
        locked: false,
        dirty: false,
        code: false,
        form: String::new(),
    };
    let header = doc.add_part(PartKind::Header, vec![paragraph("Total ")]);
    let footer = doc.add_part(PartKind::Footer, vec![paragraph("Footer")]);
    doc.insert_object(
        &Pos::new(StoryRef::Part(header), Path::top(0), 6),
        field("NUMPAGES"),
        &Default::default(),
    )?;
    for section in 0..72 {
        doc.body.extend([
            paragraph("Entry"),
            paragraph("Edit here"),
            paragraph("Room for a drawing"),
            paragraph("The end"),
        ]);
        let mut props = doc.last_section.clone();
        if section == 0 {
            props.headers.default = Some(header);
            props.footers.default = Some(footer);
        }
        if section < 71 {
            doc.para_mut(StoryRef::Body, &Path::top(section * 4 + 3))?
                .section = Some(Box::new(props));
        }
    }
    let row = 35 * 4;
    let bookmark = row + 2;
    doc.insert_object(
        &Pos::body(bookmark, 0),
        InlineObject::BookmarkStart {
            name: "AnchorPage".into(),
        },
        &Default::default(),
    )?;
    doc.insert_object(
        &Pos::body(bookmark, 4),
        InlineObject::BookmarkEnd {
            name: "AnchorPage".into(),
        },
        &Default::default(),
    )?;
    doc.para_mut(StoryRef::Body, &Path::top(row))?.props.style = Some("Heading1".into());
    for instr in ["PAGEREF AnchorPage", "STYLEREF \"Heading 1\""] {
        doc.insert_object(
            &Pos::new(StoryRef::Part(header), Path::top(0), 0),
            field(instr),
            &Default::default(),
        )?;
    }
    for name in ["PAGE", "SECTION", "SECTIONPAGES", "NUMPAGES"] {
        doc.insert_object(&Pos::body(row + 3, 0), field(name), &Default::default())?;
    }
    let float = Float {
        wrap: Wrap::Square,
        h_rel: Anchor::Column,
        v_rel: Anchor::Paragraph,
        x: 15.,
        y: 5.,
        ..Default::default()
    };
    doc.insert_object(
        &Pos::body(row + 2, 0),
        picture(55., 22., float),
        &Default::default(),
    )?;
    let opts = LayoutOptions::default();
    let mut cache = LayoutCache::new();
    let first = layout_shared(&doc, &mut cache, &opts);
    assert_eq!(first.pages.len(), 72);
    let original = doc.clone();
    doc.insert_text(
        &Pos::body(row + 1, 0),
        &"longer body ".repeat(190),
        &Default::default(),
    )?;
    let second = layout_shared(&doc, &mut cache, &opts);
    assert!(
        second.pages.len() > first.pages.len()
            && second.changed_pages_from(&first) == Some(0..second.pages.len())
    );
    let fresh = layout(&doc, &mut LayoutCache::new(), &opts);
    assert_eq!(second.index, fresh.index);
    for (pi, (a, b)) in second.pages.iter().zip(&fresh.pages).enumerate() {
        assert_eq!(format!("{a:?}"), format!("{b:?}"), "page {pi}");
        for (story, x, y) in [
            (StoryRef::Body, 60., 80.),
            (StoryRef::Part(header), 53., 18.),
            (StoryRef::Part(footer), 53., 255.),
        ] {
            assert_eq!(second.hit(pi, x, y, story), fresh.hit(pi, x, y, story));
        }
    }
    let restored = layout_shared(&original, &mut cache, &opts);
    assert_eq!(
        restored.changed_pages_from(&second),
        Some(0..second.pages.len())
    );
    assert_eq!(
        format!("{:?}", restored.pages),
        format!("{:?}", first.pages)
    );

    let mut changed = original.clone();
    changed.insert_text(
        &Pos::new(StoryRef::Part(footer), Path::top(0), 0),
        "New ",
        &Default::default(),
    )?;
    let story = layout_shared(&changed, &mut cache, &opts);
    assert_eq!(story.changed_pages_from(&restored), Some(0..72));
    let full = layout(&changed, &mut LayoutCache::new(), &opts);
    for (a, b) in story.pages.iter().zip(&full.pages) {
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }
    changed.insert_text(
        &Pos::new(StoryRef::Part(footer), Path::top(0), 0),
        &"tall ".repeat(380),
        &Default::default(),
    )?;
    let tall = layout_shared(&changed, &mut cache, &opts);
    assert_eq!(tall.changed_pages_from(&story), None);
    assert_eq!(
        tall.index,
        layout(&changed, &mut LayoutCache::new(), &opts).index
    );
    Ok(())
}

#[test]
fn body_total_before_the_window_never_keeps_a_stale_cached_value()
-> Result<(), Box<dyn std::error::Error>> {
    let mut doc = Document::new();
    doc.body.clear();
    doc.styles.default_chr.font = Some("Source Sans 3".into());
    for i in 0..68 {
        let mut p = Paragraph::with_text("Section start", Default::default());
        if i == 0 {
            p.insert_object(
                0,
                InlineObject::Field {
                    instr: "NUMPAGES".into(),
                    result: "old".into(),
                    locked: false,
                    dirty: false,
                    code: false,
                    form: String::new(),
                },
                &Default::default(),
            )?;
        }
        doc.body.push(joyword_model::para_block(p));
        let mut last = Paragraph::with_text("End", Default::default());
        if i < 67 {
            last.section = Some(Box::new(doc.last_section.clone()));
        }
        doc.body.push(joyword_model::para_block(last));
    }
    let opts = LayoutOptions::default();
    let mut cache = LayoutCache::new();
    let old = layout_shared(&doc, &mut cache, &opts);
    doc.insert_text(
        &Pos::body(68, 0),
        &"Extra text ".repeat(1200),
        &Default::default(),
    )?;
    let after = layout_shared(&doc, &mut cache, &opts);
    assert!(after.pages.len() > old.pages.len());
    assert_eq!(after.changed_pages_from(&old), None);
    let full = layout(&doc, &mut LayoutCache::new(), &opts);
    assert_eq!(
        format!("{:?}", after.pages[0]),
        format!("{:?}", full.pages[0])
    );
    assert_eq!(after.index, full.index);
    Ok(())
}
