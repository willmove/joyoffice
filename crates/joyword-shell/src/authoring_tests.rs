use super::*;
use std::sync::atomic::AtomicBool;
fn blank() -> Reader {
    Reader::blank(&AtomicBool::new(false)).unwrap()
}
fn cmd(r: &mut Reader, id: &str, args: Value) -> Value {
    r.execute(id, &args).unwrap()
}
fn reopen(r: &Reader) -> Reader {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("中文 authoring.docx");
    r.save_job(path.clone(), true).unwrap().run().unwrap();
    Reader::open(&path, &AtomicBool::new(false)).unwrap()
}

#[test]
fn header_and_footer_workflow_restores_body_selection_and_round_trips() {
    use joyword_engine::doc::{StoryRef, para::InlineObject};

    let mut reader = blank();
    cmd(
        &mut reader,
        "document.setText",
        json!({"text":"Body selection"}),
    );
    cmd(&mut reader, "select.text", json!({"text":"selection"}));
    let body = reader.engine().sel.clone();
    cmd(
        &mut reader,
        "layout.differentFirstPage",
        json!({"value":true}),
    );
    cmd(
        &mut reader,
        "layout.differentOddEven",
        json!({"value":true}),
    );
    cmd(&mut reader, "insert.editHeader", json!({}));
    cmd(&mut reader, "text.insert", json!({"text":"首页页眉"}));
    cmd(&mut reader, "insert.editFooter", json!({"page":0}));
    cmd(&mut reader, "text.insert", json!({"text":"Page "}));
    cmd(
        &mut reader,
        "insert.pageNumber",
        json!({"position":"current"}),
    );
    cmd(&mut reader, "insert.closeHeader", json!({}));
    assert_eq!(reader.engine().sel, body);

    let back = reopen(&reader);
    assert_eq!(back.header_footer_flags(), (true, true));
    let doc = &back.engine().doc;
    assert_eq!(doc.plain_text(StoryRef::Body), "Body selection");
    let section = &doc.last_section;
    let header = section.headers.first.and_then(|id| doc.parts.get(&id));
    let footer = section.footers.first.and_then(|id| doc.parts.get(&id));
    assert!(
        header
            .and_then(|part| part.blocks.first())
            .and_then(|block| block.as_para())
            .is_some_and(|para| para.text == "首页页眉")
    );
    assert!(footer.and_then(|part| part.blocks.first()).and_then(|block| block.as_para()).is_some_and(|para| {
        para.text.starts_with("Page ") && para.objects.iter().any(|object| matches!(object, InlineObject::Field { instr, .. } if instr == "PAGE"))
    }));
}

#[test]
fn style_inheritance_character_styles_and_roundtrip() {
    let mut r = blank();
    cmd(
        &mut r,
        "document.setText",
        json!({"text":"Style source\n中文 destination"}),
    );
    cmd(&mut r, "caret.docStart", json!({}));
    let parent = cmd(
        &mut r,
        "styles.create",
        json!({"name":"报告父样式","fromSelection":false}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    cmd(
        &mut r,
        "styles.modify",
        json!({"style":parent,"chr":{"bold":true,"size":20.},"para":{"spaceAfter":12.}}),
    );
    let child = cmd(
        &mut r,
        "styles.create",
        json!({"name":"报告子样式","basedOn":parent,"fromSelection":false}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(r.formatting().active("bold"));
    assert_eq!(r.formatting().character["size"], 20.);
    let rev = r.engine().rev();
    let depth = r.engine().undo_depth();
    for args in [
        json!({"style":parent,"basedOn":child}),
        json!({"style":child,"basedOn":child}),
        json!({"style":child,"basedOn":"missing"}),
        json!({"style":child,"name":"报告父样式"}),
    ] {
        assert!(r.execute("styles.modify", &args).is_err());
        assert_eq!((r.engine().rev(), r.engine().undo_depth()), (rev, depth));
    }
    cmd(
        &mut r,
        "styles.modify",
        json!({"style":child,"chr":{"bold":false}}),
    );
    assert!(!r.formatting().active("bold"));
    cmd(&mut r, "styles.modify", json!({"style":child,"reset":true}));
    assert!(r.formatting().active("bold"));
    cmd(&mut r, "select.text", json!({"text":"source"}));
    let char_id = cmd(
        &mut r,
        "styles.create",
        json!({"name":"强调字符","kind":"character","fromSelection":false}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    cmd(
        &mut r,
        "styles.modify",
        json!({"style":char_id,"chr":{"italic":true}}),
    );
    assert!(r.formatting().active("italic"));
    assert!(
        r.execute("styles.modify", &json!({"style":char_id,"basedOn":parent}))
            .is_err()
    );
    let mut back = reopen(&r);
    assert_eq!(
        back.engine()
            .doc
            .styles
            .get(&child)
            .unwrap()
            .based_on
            .as_deref(),
        Some(parent.as_str())
    );
    assert_eq!(
        back.engine().doc.styles.get(&char_id).unwrap().kind,
        joyword_engine::doc::styles::StyleKind::Character
    );
    cmd(&mut back, "select.text", json!({"text":"source"}));
    assert!(back.formatting().active("italic"));
    cmd(&mut back, "styles.delete", json!({"style":char_id}));
    assert!(back.engine().doc.styles.get(&char_id).is_none());
    cmd(&mut back, "edit.undo", json!({}));
    assert!(back.engine().doc.styles.get(&char_id).is_some());
}
#[test]
fn painter_copies_inherited_format_preserves_text_and_is_one_undo() {
    let mut r = blank();
    cmd(
        &mut r,
        "document.setText",
        json!({"text":"Alpha\nkeep Beta\nGamma"}),
    );
    cmd(&mut r, "caret.docStart", json!({}));
    let style = cmd(
        &mut r,
        "styles.create",
        json!({"name":"Brush source","fromSelection":false}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    cmd(
        &mut r,
        "styles.modify",
        json!({"style":style,"chr":{"bold":true,"size":24.,"highlight":"yellow"}}),
    );
    cmd(&mut r, "select.text", json!({"text":"Alpha"}));
    cmd(&mut r, "edit.formatPainter", json!({}));
    cmd(&mut r, "select.text", json!({"text":"Beta"}));
    let depth = r.engine().undo_depth();
    cmd(&mut r, "edit.pasteFormat", json!({}));
    assert_eq!(r.engine().undo_depth(), depth + 1);
    assert!(r.formatting().active("bold"));
    assert_eq!(r.formatting().character["size"], 24.);
    assert_eq!(r.formatting().character["highlight"], "FFFF00");
    assert!(r.engine().painter.is_none());
    assert_eq!(r.body_text(), "Alpha\nkeep Beta\nGamma");
    cmd(&mut r, "edit.undo", json!({}));
    cmd(&mut r, "select.text", json!({"text":"Beta"}));
    assert!(!r.formatting().active("bold"));
    cmd(&mut r, "select.text", json!({"text":"Alpha"}));
    cmd(&mut r, "edit.formatPainter", json!({"sticky":true}));
    for text in ["Beta", "Gamma"] {
        cmd(&mut r, "select.text", json!({"text":text}));
        cmd(&mut r, "edit.pasteFormat", json!({}));
        assert!(r.engine().painter.is_some());
    }
    cmd(&mut r, "edit.formatPainter", json!({}));
    assert!(r.engine().painter.is_none());
}
#[test]
fn painter_on_a_caret_preserves_destination_hyperlink() {
    let mut r = blank();
    cmd(&mut r, "document.setText", json!({"text":"Alpha\nBeta"}));
    cmd(&mut r, "select.text", json!({"text":"Beta"}));
    cmd(
        &mut r,
        "insert.link",
        json!({"url":"https://example.test/destination"}),
    );
    cmd(&mut r, "select.text", json!({"text":"Alpha"}));
    cmd(
        &mut r,
        "insert.link",
        json!({"url":"https://example.test/source"}),
    );
    cmd(&mut r, "format.bold", json!({"value":true}));
    cmd(&mut r, "edit.formatPainter", json!({}));
    r.select_positions(Pos::body(1, 1), Pos::body(1, 1))
        .unwrap();
    cmd(&mut r, "edit.pasteFormat", json!({}));
    let p = r.engine().doc.para_at(&Pos::body(1, 0)).unwrap();
    assert_eq!(
        p.props_of_char(1).link.as_deref(),
        Some("https://example.test/destination")
    );
    assert_eq!(p.props_of_char(1).bold, Some(true));
    assert_eq!(r.body_text(), "Alpha\nBeta");
}
#[test]
fn table_structure_cell_format_merge_split_and_roundtrip() {
    let mut r = blank();
    cmd(
        &mut r,
        "insert.table",
        insert_table_params(&["2".into(), "3".into()]).unwrap(),
    );
    cmd(&mut r, "text.insert", json!({"text":"中文单元格"}));
    cmd(&mut r, "table.insertRowBelow", json!({}));
    cmd(&mut r, "table.insertColumnRight", json!({}));
    assert_eq!(r.table_info().map(|t| (t.rows, t.cols)), Some((3, 4)));
    cmd(&mut r, "table.selectRow", json!({}));
    cmd(&mut r, "table.shading", json!({"color":"DDEBFF"}));
    cmd(&mut r, "table.merge", json!({}));
    let merged = reopen(&r);
    let table = merged
        .engine()
        .doc
        .table(StoryRef::Body, &DocPath(vec![0]))
        .unwrap();
    assert!(
        table
            .rows
            .iter()
            .flat_map(|r| &r.cells)
            .any(|c| c.props.span > 1)
    );
    let depth = r.engine().undo_depth();
    cmd(
        &mut r,
        "table.split",
        json!({"columns":4,"rows":1,"mergeFirst":true}),
    );
    assert_eq!(r.engine().undo_depth(), depth + 1);
    cmd(&mut r, "edit.undo", json!({}));
    cmd(&mut r, "edit.redo", json!({}));
    cmd(&mut r, "table.deleteRow", json!({}));
    assert_eq!(r.table_info().unwrap().rows, 2);
    let back = reopen(&r);
    assert_eq!(back.body_text(), r.body_text());
    assert_eq!(
        back.engine()
            .doc
            .table(StoryRef::Body, &DocPath(vec![0]))
            .unwrap()
            .rows
            .len(),
        2
    );
}
#[test]
fn image_import_size_crop_alt_wrap_and_roundtrip() {
    let source = Reader::from_bytes(
        "fixture",
        include_bytes!("../../../assets/samples/mixed-media.docx"),
        &AtomicBool::new(false),
    )
    .unwrap();
    let bytes = source
        .engine()
        .doc
        .media
        .values()
        .find(|b| joyword_geom::picture::probe(b).is_ok())
        .unwrap();
    let args = image_bytes(bytes).unwrap();
    let mut r = blank();
    cmd(&mut r, "insert.picture", args);
    r.select_object(Pos::body(0, 0)).unwrap();
    let before = r.picture_info().unwrap();
    let depth = r.engine().undo_depth();
    cmd(&mut r, "picture.size", json!({"width":120.}));
    assert_eq!(r.engine().undo_depth(), depth + 1);
    assert!((r.picture_info().unwrap().height / before.height - 120. / before.width).abs() < 0.001);
    cmd(
        &mut r,
        "picture.crop",
        picture_crop(&["10", "5", "15", "0"].map(String::from)).unwrap(),
    );
    cmd(
        &mut r,
        "object.altText",
        json!({"text":"中文图片说明","title":"测试插图"}),
    );
    cmd(&mut r, "arrange.wrap", json!({"wrap":"square"}));
    let mut back = reopen(&r);
    back.select_object(Pos::body(0, 0)).unwrap();
    let p = back.picture_info().unwrap();
    assert_eq!(p.alt, "中文图片说明");
    assert!((p.width - 120.).abs() < 0.1);
    assert!((p.crop[0] - 0.1).abs() < 0.001 && (p.crop[2] - 0.15).abs() < 0.001);
    assert_eq!(back.engine().doc.media.len(), 1);
    assert!(image_bytes(b"bad image").is_err());
    assert!(image_bytes(&vec![0; IMAGE_BYTES as usize + 1]).is_err());
}
#[test]
fn authoring_forms_reject_invalid_input_before_any_edit() {
    for (rows, cols) in [("NaN", "2"), ("1.2", "2"), ("100", "32"), ("0", "2")] {
        assert!(insert_table_params(&[rows.into(), cols.into()]).is_err());
    }
    for crop in [
        ["90", "0", "10", "0"],
        ["-1", "0", "0", "0"],
        ["NaN", "0", "0", "0"],
    ] {
        assert!(picture_crop(&crop.map(String::from)).is_err());
    }
    for name in ["", "bad\nname"] {
        assert!(style_name(name).is_err());
    }
}
