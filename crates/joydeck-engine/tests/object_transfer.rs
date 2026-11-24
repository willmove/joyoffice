#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "Test assertions"
)]
use joydeck_engine::{Command, Session, clipboard, media_refs, original_bindings, transfer};
use joydeck_model::{
    MediaId, PhType, Placeholder, Presentation, Rgba, SchemeSlot, Shape, ShapeId, ShapeKind, Xfrm,
    style::{ColorRef, Fill, PictureFill, ShapeStyle},
    text::{Action, Hyperlink, LevelStyle, RunProps, TextBody},
};
use std::sync::Arc;

fn shape(id: u32, name: &str, kind: ShapeKind) -> Shape {
    Shape {
        id: ShapeId(id),
        name: name.into(),
        xfrm: Some(Xfrm::new(10., 20., 40., 30.)),
        kind,
        ..Default::default()
    }
}

fn link(action: Action, sound: Option<MediaId>) -> Hyperlink {
    Hyperlink {
        action,
        sound,
        tooltip: String::new(),
        highlight_click: false,
        stop_sound: false,
        zoom_return: None,
    }
}

fn picture(media: MediaId) -> ShapeKind {
    ShapeKind::Picture {
        fill: PictureFill {
            media,
            ..Default::default()
        },
    }
}

fn paste(transfer: transfer::Transfer) -> Command {
    Command::PasteObjects {
        transfer: Box::new(transfer),
    }
}

#[test]
fn grouped_transfer_rebinds_children_connections_and_media_in_one_edit() {
    let mut source = Presentation::default();
    let photo = source
        .add_media("source.png", "image/png", vec![3, 1, 4])
        .unwrap();
    let sound = source
        .add_media("click.wav", "audio/wav", vec![1, 7])
        .unwrap();
    let local_slide = source.slides[0].id;
    let first = shape(600, "photo", picture(photo));
    let mut caption = shape(601, "caption", ShapeKind::Shape);
    caption.click = Some(link(Action::Slide { slide: local_slide }, Some(sound)));
    let mut body = TextBody::from_text("portable link");
    body.paragraphs[0].runs[0].props.link = Some(link(
        Action::Url {
            url: "https://example.org/joydeck".into(),
        },
        None,
    ));
    caption.text = Some(body);
    let inner = shape(
        602,
        "nested",
        ShapeKind::Group {
            children: vec![caption],
            child: Xfrm::new(8., 9., 100., 90.),
        },
    );
    let connector = shape(
        603,
        "join",
        ShapeKind::Connector {
            start: Some((ShapeId(600), 1)),
            end: Some((ShapeId(601), 2)),
        },
    );
    let detached = shape(
        604,
        "outside",
        ShapeKind::Connector {
            start: Some((ShapeId(601), 3)),
            end: Some((ShapeId(606), 0)),
        },
    );
    let mut group = shape(
        605,
        "group",
        ShapeKind::Group {
            children: vec![first, inner, connector, detached],
            child: Xfrm::new(4., 6., 180., 160.),
        },
    );
    group.xfrm = Some(Xfrm::new(24., 48., 240., 120.));
    Arc::make_mut(&mut source.slides[0])
        .shapes
        .extend([group, shape(606, "not selected", ShapeKind::Shape)]);
    source.fix_next_id();
    let captured = transfer::capture(&source, 0, &[ShapeId(605)]).unwrap();
    assert_eq!(
        captured.media.iter().map(|m| m.id).collect::<Vec<_>>(),
        [photo, sound]
    );

    let mut same = captured.clone();
    assert!(!clipboard::scope_objects(&mut same, Some(&source)));
    assert_eq!(
        same.shapes[0].children()[1].children()[0]
            .click
            .as_ref()
            .unwrap()
            .sound,
        Some(sound)
    );
    let mut source_editor = Session::new(source.clone(), original_bindings(&source), true);
    assert!(source_editor.execute(paste(same)).unwrap());
    let pasted = source_editor.model().slides[0].shapes.last().unwrap();
    let retained = pasted.children()[1].children()[0].click.as_ref().unwrap();
    assert_eq!(retained.action, Action::Slide { slide: local_slide });
    assert_eq!(retained.sound, Some(sound));
    assert_eq!(source_editor.model().media.len(), 2);

    let mut copied: transfer::Transfer =
        serde_json::from_slice(&serde_json::to_vec(&captured).unwrap()).unwrap();
    assert!(clipboard::scope_objects(&mut copied, None));
    assert_eq!(
        copied.media.iter().map(|m| m.id).collect::<Vec<_>>(),
        [photo]
    );

    let mut destination = Presentation::default();
    assert_eq!(destination.slides[0].id, local_slide);
    let reused = destination
        .add_media("destination.png", "image/png", vec![3, 1, 4])
        .unwrap();
    Arc::make_mut(&mut destination.slides[0]).shapes.push(shape(
        600,
        "collision",
        ShapeKind::Shape,
    ));
    destination.fix_next_id();
    let mut editor = Session::new(destination.clone(), original_bindings(&destination), true);
    editor.select(0, vec![ShapeId(600)]).unwrap();
    let initial = editor.snapshot();
    assert!(editor.validate(paste(copied.clone())).unwrap());
    assert_eq!(editor.model(), initial.model.as_ref());
    assert!(editor.execute(paste(copied.clone())).unwrap());
    let group = editor.model().slides[0].shapes.last().unwrap();
    let transform = group.xfrm.unwrap();
    assert_eq!((transform.x, transform.y), (42., 66.));
    assert_eq!(editor.selection().shapes, [group.id]);
    assert_ne!(group.id, ShapeId(605));
    let ShapeKind::Group { child, children } = &group.kind else {
        panic!("group missing")
    };
    assert_eq!(*child, Xfrm::new(4., 6., 180., 160.));
    assert_eq!(children[0].xfrm, Some(Xfrm::new(10., 20., 40., 30.)));
    assert_ne!(children[0].id, ShapeId(600));
    assert_eq!(media_refs::shapes_media(&children[..1]), [reused]);
    let caption = &children[1].children()[0];
    assert!(caption.click.is_none());
    assert_eq!(
        caption.text.as_ref().unwrap().paragraphs[0].runs[0]
            .props
            .link
            .as_ref()
            .unwrap()
            .action,
        Action::Url {
            url: "https://example.org/joydeck".into()
        }
    );
    assert_eq!(
        children[2].kind,
        ShapeKind::Connector {
            start: Some((children[0].id, 1)),
            end: Some((caption.id, 2)),
        }
    );
    assert_eq!(
        children[3].kind,
        ShapeKind::Connector {
            start: Some((caption.id, 3)),
            end: None
        }
    );
    assert!(Arc::ptr_eq(
        &editor.model().media[0].data,
        &destination.media[0].data
    ));
    let pasted = editor.snapshot();

    assert!(editor.execute(paste(copied)).unwrap());
    assert_eq!(editor.model().media.len(), 1);
    assert!(editor.execute(Command::Undo).unwrap());
    assert_eq!(editor.model(), pasted.model.as_ref());
    assert!(editor.execute(Command::Undo).unwrap());
    assert_eq!(editor.model(), initial.model.as_ref());
    assert_eq!(editor.selection(), &initial.selection);
    assert!(!editor.dirty());
    assert!(editor.execute(Command::Redo).unwrap());
    assert_eq!(editor.model(), pasted.model.as_ref());
    assert_eq!(editor.selection(), &pasted.selection);
}

#[test]
fn placeholder_keeps_source_layout_and_master_appearance_without_copying_bindings() {
    let mut source = Presentation::default();
    let image = source
        .add_media("layout.png", "image/png", vec![2, 4, 6])
        .unwrap();
    let slide = Arc::make_mut(&mut source.slides[0]);
    let id = slide.shapes[0].id;
    let placeholder = Placeholder {
        kind: PhType::Title,
        idx: 0,
        ..Default::default()
    };
    let mut title = shape(id.0, "title", ShapeKind::Shape);
    title.xfrm = None;
    title.ph = Some(placeholder.clone());
    title.text = Some(TextBody::from_text("+mj-lt"));
    slide.shapes = vec![title];
    let layout_id = slide.layout;
    let mut accent = shape(900, "matrix color", ShapeKind::Shape);
    accent.style = Some(ShapeStyle::accent(SchemeSlot::Accent2));
    slide.shapes.push(accent);

    let master = Arc::make_mut(&mut source.masters[0]);
    master.theme.fonts.major.latin = "Source Heading".into();
    master
        .theme
        .colors
        .set(SchemeSlot::Accent2, Rgba::rgb(8, 73, 124));
    master
        .theme
        .colors
        .set(SchemeSlot::Accent3, Rgba::rgb(167, 94, 36));
    master.theme.format.fills[0] = Fill::solid(ColorRef::scheme(SchemeSlot::PhClr));
    master.shapes.clear();
    master.title_style.levels[0] = Some(LevelStyle {
        run: RunProps {
            font: Some("+mj-lt".into()),
            fill: Some(Fill::solid(ColorRef::scheme(SchemeSlot::Tx1))),
            ..Default::default()
        },
        ..Default::default()
    });
    let layout = master
        .layouts
        .iter_mut()
        .find(|layout| layout.id == layout_id)
        .unwrap();
    layout.color_map = Some(vec![("tx1".into(), "accent3".into())]);
    let position = Xfrm::new(51., 33., 160., 47.);
    layout.shapes = vec![Shape {
        xfrm: Some(position),
        ph: Some(placeholder),
        fill: Some(Fill::Picture(PictureFill {
            media: image,
            ..Default::default()
        })),
        ..Default::default()
    }];
    let clip = transfer::capture(&source, 0, &[id, ShapeId(900)]).unwrap();
    assert_eq!(clip.shapes[0].xfrm, Some(position));
    assert!(
        clip.shapes
            .iter()
            .all(|shape| shape.ph.is_none() && shape.style.is_none())
    );
    let text = clip.shapes[0].text.as_ref().unwrap();
    let run = &text.paragraphs[0].runs[0];
    assert_eq!(run.text, "+mj-lt");
    assert_eq!(run.props.font.as_deref(), Some("Source Heading"));
    assert_eq!(
        run.props.fill,
        Some(Fill::solid(ColorRef::rgb(Rgba::rgb(167, 94, 36))))
    );
    assert_eq!(
        clip.shapes[1].fill,
        Some(Fill::solid(ColorRef::rgb(Rgba::rgb(8, 73, 124))))
    );

    let mut destination = Presentation::default();
    Arc::make_mut(&mut destination.masters[0]).theme.name = "Target theme".into();
    let mut editor = Session::new(destination.clone(), original_bindings(&destination), true);
    assert!(editor.execute(paste(clip)).unwrap());
    let pasted = &editor.model().slides[0].shapes[destination.slides[0].shapes.len()..];
    assert_eq!(pasted[0].xfrm.unwrap().x, position.x + 18.);
    let ids = media_refs::shapes_media(pasted);
    assert_eq!(ids.len(), 1);
    assert_eq!(
        editor.model().media(ids[0]).unwrap().data.as_slice(),
        [2, 4, 6]
    );
    assert_eq!(
        editor.model().slides[0].layout,
        destination.slides[0].layout
    );
    assert_eq!(editor.model().masters, destination.masters);
}

#[test]
fn copy_and_paste_reject_invalid_groups_or_exhausted_ids_before_writing_media() {
    let mut source = Presentation::default();
    let image = source
        .add_media("new.png", "image/png", vec![1, 2, 3])
        .unwrap();
    let content = shape(700, "image", picture(image));
    let group = shape(
        701,
        "group",
        ShapeKind::Group {
            children: vec![content.clone()],
            child: Xfrm::new(0., 0., 50., 40.),
        },
    );
    Arc::make_mut(&mut source.slides[0]).shapes.push(group);
    let clip = transfer::capture(&source, 0, &[ShapeId(701)]).unwrap();
    let destination = Presentation {
        next_id: u32::MAX - 1,
        ..Default::default()
    };
    let mut draft = Session::new(destination, Default::default(), true).snapshot();
    let before = draft.clone();

    assert!(draft.stage_edit(paste(clip.clone())).is_err());
    assert!(Arc::ptr_eq(&draft.model, &before.model));
    assert_eq!(draft.selection, before.selection);

    let mut invalid = clip.clone();
    invalid.shapes[0].children_mut().unwrap().push(content);
    assert!(draft.stage_edit(paste(invalid)).is_err());
    assert!(Arc::ptr_eq(&draft.model, &before.model));
    let mut invalid = clip;
    invalid.shapes[0].children_mut().unwrap()[0].raw_ext = Some("<future/>".into());
    assert!(draft.stage_edit(paste(invalid)).is_err());
    assert!(Arc::ptr_eq(&draft.model, &before.model));
}
