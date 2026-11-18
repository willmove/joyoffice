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
fn distant_fields_reflow_their_sections_and_fall_back_beyond_the_page_budget()
-> Result<(), Box<dyn std::error::Error>> {
    let field = |instr: &str| InlineObject::Field {
        instr: instr.into(),
        result: "old".into(),
        locked: false,
        dirty: false,
        code: false,
        form: String::new(),
    };
    let mut doc = Document::new();
    doc.body.clear();
    doc.styles.default_chr.font = Some("Source Sans 3".into());
    for i in 0..68 {
        let mut p = Paragraph::with_text("Section start", Default::default());
        if i == 0 || i == 64 {
            for instr in ["NUMPAGES", "PAGE \\* ROMAN", "SECTIONPAGES", "SECTION"] {
                p.insert_object(0, field(instr), &Default::default())?;
            }
        }
        doc.body.push(joyword_model::para_block(p));
        let mut last = Paragraph::with_text("End", Default::default());
        if i < 67 {
            last.section = Some(Box::new(doc.last_section.clone()));
        }
        doc.body.push(joyword_model::para_block(last));
    }
    let opts = LayoutOptions::default();
    for many in [false, true] {
        let mut before = doc.clone();
        if many {
            for i in (2..32).step_by(2) {
                // Keep each following section's entering paragraph a plain sentinel.
                before.insert_object(
                    &Pos::body(i * 2 + 1, 0),
                    field("NUMPAGES"),
                    &Default::default(),
                )?;
            }
        }
        let mut edited = before.clone();
        edited.insert_text(
            &Pos::body(68, 0),
            &"Extra text ".repeat(1200),
            &Default::default(),
        )?;
        let mut cache = LayoutCache::new();
        let mut last = layout_shared(&before, &mut cache, &opts);
        let initial_pages = last.pages.len();
        for state in [&edited, &before, &edited] {
            let next = layout_shared(state, &mut cache, &opts);
            let full = layout(state, &mut LayoutCache::new(), &opts);
            assert_eq!(next.pages.len(), full.pages.len());
            assert_eq!(
                next.pages.len() > initial_pages,
                std::ptr::eq(state, &edited)
            );
            let dirty = (!many).then_some(0..next.pages.len().max(last.pages.len()));
            assert_eq!(next.changed_pages_from(&last), dirty);
            assert_eq!(next.index, full.index);
            for (pi, (actual, expected)) in next.pages.iter().zip(&full.pages).enumerate() {
                assert_eq!(format!("{actual:?}"), format!("{expected:?}"), "page {pi}");
                assert_eq!(
                    next.hit(pi, 130., 78., StoryRef::Body),
                    full.hit(pi, 130., 78., StoryRef::Body)
                );
            }
            last = next;
        }
    }
    Ok(())
}

#[test]
fn clipped_groups_linked_boxes_and_distant_endnotes_reflow_at_their_displayed_section()
-> Result<(), Box<dyn std::error::Error>> {
    use joyword_model::{
        PartKind,
        para::{CanvasStyle, Float, GroupChild, NoteKind, ShapeKind, Wrap},
        section::NotePos,
    };

    let paragraph =
        |text: &str| joyword_model::para_block(Paragraph::with_text(text, Default::default()));
    let mut doc = Document::new();
    doc.body.clear();
    doc.settings.endnote_pr.pos = Some(NotePos::DocEnd);
    for si in 0..70 {
        doc.body.extend([
            paragraph("Heading"),
            paragraph("Body"),
            paragraph("Frame anchor"),
            paragraph("End"),
        ]);
        if si < 69 {
            let mut sect = doc.last_section.clone();
            if si == 35 {
                sect.endnote_pr.pos = Some(NotePos::SectEnd);
            }
            doc.para_mut(StoryRef::Body, &Path::top(si * 4 + 3))?
                .section = Some(Box::new(sect));
        }
    }
    let head = doc.add_part(
        PartKind::TextBox,
        vec![paragraph(&"Linked text ".repeat(30))],
    );
    let tail = doc.add_part(PartKind::TextBox, vec![paragraph("")]);
    doc.parts.get_mut(&head).ok_or("missing frame")?.body.next = Some(tail);
    let shape = |id| InlineObject::Shape {
        kind: ShapeKind::TextBox,
        w: 90.,
        h: 40.,
        fill: None,
        stroke: None,
        stroke_width: 0.,
        float: Float {
            wrap: Wrap::Square,
            ..Default::default()
        },
        story: Some(id),
        effects: Default::default(),
        freeform: None,
        extra: Default::default(),
    };
    let group = InlineObject::Group {
        w: 140.,
        h: 70.,
        ch_w: 140.,
        ch_h: 70.,
        float: Float {
            wrap: Wrap::Square,
            rot: 14.,
            flip_h: true,
            ..Default::default()
        },
        canvas: Some(CanvasStyle::default()),
        children: vec![
            GroupChild {
                x: 12.,
                y: 8.,
                obj: shape(head),
            },
            GroupChild {
                x: 112.,
                y: 9.,
                obj: picture(48., 25., Default::default()),
            },
        ],
    };
    doc.insert_object(&Pos::body(34 * 4 + 2, 0), group, &Default::default())?;
    doc.insert_object(&Pos::body(35 * 4 + 2, 0), shape(tail), &Default::default())?;
    let mut ids = Vec::new();
    for (section, label) in [
        (2, "Earlier mark"),
        (35, "Local mark"),
        (54, "Document mark"),
        (63, "Last mark"),
    ] {
        let id = doc.add_part(PartKind::Endnote, vec![paragraph(label)]);
        doc.insert_object(
            &Pos::body(section * 4 + 1, 0),
            InlineObject::NoteRef {
                kind: NoteKind::Endnote,
                id,
                custom: String::new(),
            },
            &Default::default(),
        )?;
        ids.push(id);
    }
    let opts = LayoutOptions::default();
    let mut cache = LayoutCache::new();
    let mut old = layout_shared(&doc, &mut cache, &opts);
    assert_eq!(old.pages.len(), 70);
    for (&id, wanted) in ids.iter().zip([35, 35, 69, 69]) {
        let pages = &old.index[&(StoryRef::Part(id), Path::top(0))];
        assert_eq!(old.pages[pages[0].0].section, wanted);
    }
    assert!(
        old.pages
            .iter()
            .flat_map(|p| &p.items)
            .any(|it| matches!(it,Placed::Clip{items,..} if !items.is_empty()))
    );
    for (pos, text, first, last) in [
        (
            Pos::body(34 * 4 + 1, 0),
            "Expanded body ".repeat(1050),
            34,
            69,
        ),
        (
            Pos::new(StoryRef::Part(head), Path::top(0), 0),
            "Edit first frame ".repeat(5),
            34,
            35,
        ),
        (
            Pos::new(StoryRef::Part(ids[0]), Path::top(0), 0),
            "Edited deferred endnote ".to_string(),
            35,
            35,
        ),
        (
            Pos::new(StoryRef::Part(ids[2]), Path::top(0), 0),
            "Edited document endnote ".repeat(9),
            69,
            69,
        ),
    ] {
        let before = doc.clone();
        doc.insert_text(&pos, &text, &Default::default())?;
        for version in [&doc, &before, &doc] {
            let changed = layout_shared(version, &mut cache, &opts);
            let dirty = changed
                .changed_pages_from(&old)
                .ok_or_else(|| format!("full layout for {pos:?}"))?;
            assert!(
                changed.pages[dirty.start].section <= first,
                "{pos:?}: {dirty:?}"
            );
            assert!(
                dirty.end
                    >= changed
                        .pages
                        .iter()
                        .rposition(|p| p.section <= last)
                        .unwrap()
                        + 1
            );
            let rebuilt = layout(version, &mut LayoutCache::new(), &opts);
            assert_eq!(changed.index, rebuilt.index);
            assert_eq!(changed.pages.len(), rebuilt.pages.len());
            for (pi, (a, b)) in changed.pages.iter().zip(&rebuilt.pages).enumerate() {
                assert_eq!(format!("{a:?}"), format!("{b:?}"), "page {pi} {pos:?}");
                if (*a).section == 34 || a.section == 35 || a.section == 69 {
                    for story in [
                        StoryRef::Body,
                        StoryRef::Part(head),
                        StoryRef::Part(ids[0]),
                        StoryRef::Part(ids[2]),
                    ] {
                        for (x, y) in [(75., 78.), (120., 135.), (150., 190.)] {
                            assert_eq!(changed.hit(pi, x, y, story), rebuilt.hit(pi, x, y, story));
                        }
                    }
                }
            }
            old = changed;
        }
    }
    Ok(())
}
#[test]
fn a_remote_field_can_repaginate_its_own_section_and_invalidate_other_fields()
-> Result<(), Box<dyn std::error::Error>> {
    let mut original = Document::new();
    original.styles.default_chr.font = Some("Source Sans 3".into());
    original.body.clear();
    for section in 0..68 {
        let mut entry = Paragraph::with_text("Entry", Default::default());
        entry.props.space_after = Some(0.);
        original.body.push(joyword_model::para_block(entry));
        let mut row = Paragraph::with_text("", Default::default());
        row.props.space_after = Some(0.);
        let mut props = original.last_section.clone();
        if section == 0 {
            props.page_num_start = Some(35);
        }
        if section == 64 {
            props.page_w = 211.;
            props.page_h = 82.;
            props.margin_top = 20.;
            props.margin_bottom = 20.;
            props.margin_left = 20.;
            props.margin_right = 20.;
        }
        let (instr, count) = match section {
            64 => ("PAGE", 12),
            2 => ("NUMPAGES", 1),
            66 => ("PAGE", 1),
            _ => ("", 0),
        };
        for _ in 0..count {
            row.insert_object(
                row.len(),
                InlineObject::Field {
                    instr: instr.into(),
                    result: "99".into(),
                    locked: false,
                    dirty: false,
                    code: false,
                    form: String::new(),
                },
                &Default::default(),
            )?;
            row.insert_text(row.len(), " ", &Default::default())?;
        }
        if section < 67 {
            row.section = Some(Box::new(props));
        }
        original.body.push(joyword_model::para_block(row));
    }
    for (break_kind, continued_note) in [
        (SectionStart::NextPage, false),
        (SectionStart::EvenPage, false),
        (SectionStart::OddPage, false),
        (SectionStart::NextPage, true),
        (SectionStart::EvenPage, true),
    ] {
        let mut original = original.clone();
        let note = if continued_note {
            let blocks = (0..58)
                .map(|i| {
                    joyword_model::para_block(Paragraph::with_text(
                        &format!("Continued note {i}"),
                        Default::default(),
                    ))
                })
                .collect();
            let id = original.add_part(joyword_model::PartKind::Footnote, blocks);
            original.insert_object(
                &Pos::body(66 * 2 + 1, 0),
                InlineObject::NoteRef {
                    kind: joyword_model::para::NoteKind::Footnote,
                    id,
                    custom: String::new(),
                },
                &Default::default(),
            )?;
            Some(id)
        } else {
            None
        };
        if break_kind != SectionStart::NextPage {
            original.settings.mirror_margins = true;
            original.settings.even_odd_headers = true;
            let props = original
                .para_mut(StoryRef::Body, &Path::top(65 * 2 + 1))?
                .section
                .as_mut()
                .ok_or("missing section")?;
            props.start = break_kind;
            props.margin_left = 37.;
            props.margin_right = 61.;
            original.last_section.page_num_start = Some(7);
        }
        let mut edited = original.clone();
        edited.insert_text(
            &Pos::body(68, 0),
            &"Extra text ".repeat(900),
            &Default::default(),
        )?;
        let opts = LayoutOptions::default();
        let mut cache = LayoutCache::new();
        let mut last = layout_shared(&original, &mut cache, &opts);
        let remote_pages = |d: &DocLayout| d.pages.iter().filter(|p| p.section == 64).count();
        let initial_remote = if break_kind == SectionStart::OddPage {
            2
        } else {
            1
        };
        let edited_remote = if break_kind == SectionStart::EvenPage {
            3
        } else {
            2
        };
        assert_eq!(remote_pages(&last), initial_remote);
        if let Some(id) = note {
            let positions = last
                .index
                .iter()
                .filter(|((story, _), _)| *story == StoryRef::Part(id))
                .flat_map(|(_, locations)| locations.iter().map(|(page, _)| *page))
                .collect::<std::collections::BTreeSet<_>>();
            assert!(positions.len() > 1);
        }
        for (state, expected) in [
            (&edited, edited_remote),
            (&original, initial_remote),
            (&edited, edited_remote),
        ] {
            let next = layout_shared(state, &mut cache, &opts);
            let full = layout(state, &mut LayoutCache::new(), &opts);
            assert_eq!(remote_pages(&full), expected);
            assert_eq!(
                next.changed_pages_from(&last),
                Some(2..last.pages.len().max(full.pages.len()))
            );
            assert_eq!(next.pages.len(), full.pages.len());
            assert_eq!(next.index, full.index);
            for (i, (a, b)) in next.pages.iter().zip(&full.pages).enumerate() {
                assert_eq!(format!("{a:?}"), format!("{b:?}"), "page {i}");
                assert_eq!(
                    next.hit(i, 82., 51., StoryRef::Body),
                    full.hit(i, 82., 51., StoryRef::Body)
                );
            }
            last = next;
        }
    }
    Ok(())
}

#[test]
fn section_lists_drop_caps_frames_and_table_fields_survive_edit_undo_and_redo()
-> Result<(), Box<dyn std::error::Error>> {
    use joyword_model::{
        ListKind,
        props::{NumRef, TextFrame},
    };
    let mut doc = Document::new();
    doc.body.clear();
    doc.styles.default_chr.font = Some("Source Sans 3".into());
    let num = doc.numbering.add_list(ListKind::Legal);
    let mut restart = doc.numbering.nums[0].clone();
    restart.id = num + 1;
    restart.start_overrides = vec![(0, 9)];
    doc.numbering.nums.push(restart);
    let mut edit = 0;
    for i in 0..68 {
        let mut p = Paragraph::with_text(&format!("Section {i}"), Default::default());
        p.props.numbering = Some(NumRef {
            num: if i < 35 { num } else { num + 1 },
            level: (i % 3) as u8,
        });
        if i == 34 {
            edit = doc.body.len();
            p.props.drop_cap = Some(3);
            p.props.drop_cap_distance = Some(5.);
        }
        doc.body.push(joyword_model::para_block(p));
        if i == 34 {
            let mut caption = Paragraph::with_text("Framed text", Default::default());
            caption.props.frame = Some(TextFrame {
                w: Some(72.),
                x: 18.,
                ..Default::default()
            });
            doc.body.push(joyword_model::para_block(caption));
        }
        if [2, 65].contains(&i) {
            let mut f = Paragraph::with_text("Table totals ", Default::default());
            for instr in ["PAGE", "NUMPAGES", "SECTION", "SECTIONPAGES"] {
                f.insert_object(
                    f.text.len(),
                    InlineObject::Field {
                        code: false,
                        form: String::new(),
                        instr: instr.into(),
                        result: "00".into(),
                        locked: false,
                        dirty: false,
                    },
                    &Default::default(),
                )?;
            }
            let mut outer = Table::new(1, 1, 320.);
            let mut inner = Table::new(1, 1, 280.);
            inner.rows[0].cells[0].blocks = vec![joyword_model::para_block(f)];
            outer.rows[0].cells[0].blocks = vec![Arc::new(Block::Table(inner))];
            doc.body.push(Arc::new(Block::Table(outer)));
        }
        let mut end = Paragraph::with_text("End", Default::default());
        if i < 67 {
            end.section = Some(Box::new(doc.last_section.clone()));
        }
        doc.body.push(joyword_model::para_block(end));
    }
    let opts = LayoutOptions::default();
    let mut edited = doc.clone();
    edited.insert_text(
        &Pos::body(edit, 0),
        &"Edited clause. ".repeat(740),
        &Default::default(),
    )?;
    let mut cache = LayoutCache::new();
    let mut old = layout_shared(&doc, &mut cache, &opts);
    let initial = old.pages.len();
    for state in [&edited, &doc, &edited] {
        let next = layout_shared(state, &mut cache, &opts);
        let full = layout(state, &mut LayoutCache::new(), &opts);
        assert_eq!(
            next.changed_pages_from(&old),
            Some(2..old.pages.len().max(next.pages.len()))
        );
        assert_eq!(next.pages.len() > initial, std::ptr::eq(state, &edited));
        assert_eq!(next.index, full.index);
        for (i, (candidate, expected)) in next.pages.iter().zip(&full.pages).enumerate() {
            assert_eq!(
                format!("{candidate:?}"),
                format!("{expected:?}"),
                "page {i}"
            );
            assert_eq!(
                next.hit(i, 82., 82., StoryRef::Body),
                full.hit(i, 82., 82., StoryRef::Body)
            );
        }
        old = next;
    }
    // A list-format change affects following sections and requires a fresh set of counters.
    let paragraph = edited.para_mut(StoryRef::Body, &Path::top(edit))?;
    paragraph.props.numbering = None;
    let reset = layout_shared(&edited, &mut cache, &opts);
    assert_eq!(reset.changed_pages_from(&old), None);
    let expected = layout(&edited, &mut LayoutCache::new(), &opts);
    assert_eq!(
        format!("{:?}", reset.pages),
        format!("{:?}", expected.pages)
    );
    Ok(())
}
