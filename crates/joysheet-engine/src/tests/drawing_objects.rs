#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::*;
use joysheet_model::{DrawingObject, MathExpr, TimelineLevel};

fn shape(s: &Session) -> joysheet_model::Shape {
    s.active().unwrap().wb.active().unwrap().shapes.last().unwrap().clone()
}
fn roundtrip(s: &Session) -> (joysheet_model::Workbook, joysheet_xlsx::ReadReport) {
    joysheet_xlsx::read_xlsx(&joysheet_xlsx::write_xlsx(&s.active().unwrap().wb).unwrap()).unwrap()
}
#[test]
fn structured_objects_edit_undo_and_native_roundtrip() {
    let mut s = s();
    for (command, params) in [
        ("insert.smartArt", json!({"layout":"hierarchy","text":"团队\n产品\n工程"})),
        ("insert.wordArt", json!({"text":"JoyOffice","warp":"textWave1","shadow":true})),
        ("insert.equation", json!({"text":"x=\\frac{-b+\\sqrt{b^{2}-4ac}}{2a}"})),
    ] {
        s.execute(command, params).unwrap();
        let original = shape(&s);
        let id = original.id;
        s.execute("object.edit", json!({"id":id,"text":"x^{3}"})).unwrap();
        assert_eq!(shape(&s).text, "x^{3}");
        s.execute("edit.undo", json!({})).unwrap();
        assert_eq!(shape(&s), original);
        s.execute("edit.redo", json!({})).unwrap();
        assert_eq!(shape(&s).text, "x^{3}");
        s.execute("edit.undo", json!({})).unwrap();
    }
    let (wb, report) = roundtrip(&s);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let before = &s.active().unwrap().wb.active().unwrap().shapes;
    let after = &wb.active().unwrap().shapes;
    assert_eq!(before.len(), after.len());
    for (a, b) in before.iter().zip(after) {
        assert_eq!(a.text, b.text);
        assert_eq!(a.format.object, b.format.object);
    }
    for sh in after {
        assert!(!crate::drawing_scene::render(&wb, 0, sh).unwrap().is_empty());
    }
    // A second save must remain an editable semantic diagram, not turn it into a plain shape.
    let again = joysheet_xlsx::read_xlsx(&joysheet_xlsx::write_xlsx(&wb).unwrap()).unwrap().0;
    assert_eq!(after[0].format.object, again.active().unwrap().shapes[0].format.object);
}
#[test]
fn embedded_package_preserves_arbitrary_original_bytes_and_deletion() {
    let mut s = s();
    let bytes: Vec<u8> = (0..=255).cycle().take(9000).collect();
    let b64 = crate::io::base64_encode(&bytes);
    let id = s.execute("insert.object", json!({"base64":b64,"name":"原始文件.bin"})).unwrap()["shape"].as_u64().unwrap();
    let (wb, r) = roundtrip(&s);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    let Some(DrawingObject::Embedded { name, data, .. }) = &wb.active().unwrap().shapes[0].format.object else { panic!("missing embedded file") };
    assert_eq!(name, "原始文件.bin");
    assert_eq!(data.as_slice(), bytes);
    let second = joysheet_xlsx::write_xlsx(&wb).unwrap();
    let (second, r) = joysheet_xlsx::read_xlsx(&second).unwrap();
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    assert_eq!(second.active().unwrap().shapes[0].format.object, wb.active().unwrap().shapes[0].format.object);
    s.execute("object.delete", json!({"id":id,"kind":"shape"})).unwrap();
    assert!(s.active().unwrap().undo.last().unwrap().retained_bytes >= bytes.len());
    assert!(shape_count(&s) == 0);
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(shape(&s).format.object, wb.active().unwrap().shapes[0].format.object);
}
fn shape_count(s: &Session) -> usize {
    s.active().unwrap().wb.active().unwrap().shapes.len()
}
#[test]
fn equation_parser_is_bounded_and_invalid_edits_are_atomic() {
    let mut s = s();
    s.execute("insert.equation", json!({"text":"\\frac{a_{i}}{\\sqrt{x}}"})).unwrap();
    let original = shape(&s);
    for text in ["\\frac{x}", "x^", &"{".repeat(33), &"x".repeat(4097)] {
        assert!(s.execute("object.edit", json!({"id":original.id,"text":text})).is_err());
        assert_eq!(shape(&s), original);
    }
    s.execute("shape.setText", json!({"id":original.id,"text":"a^{2}"})).unwrap();
    assert_eq!(shape(&s).format.object, Some(DrawingObject::Equation { expression: MathExpr::parse("a^{2}").unwrap() }));
    s.execute("review.protectSheet", json!({})).unwrap();
    assert!(s.execute("object.edit", json!({"id":original.id,"text":"b"})).is_err());
}
fn date_pivot() -> Session {
    let mut s = s();
    s.execute(
        "range.setValues",
        json!({"range":"A1","values":[["Date","Region","Sales"],["1/1/2026","East",10],["2/1/2026","East",20],["3/1/2026","West",30]]}),
    )
    .unwrap();
    s.execute(
        "insert.pivotTable",
        json!({"source":"Sheet1!A1:C4","destination":"Sheet1!F3","name":"SalesPivot","rows":["Region"],"values":["Sales"]}),
    )
    .unwrap();
    s
}
#[test]
fn timeline_filters_report_recalculates_dependents_and_roundtrips() {
    let mut s = date_pivot();
    let r = s.execute("insert.timeline", json!({"pivot":"SalesPivot","column":"Date","at":"J2"})).unwrap();
    let id = r["shape"].as_u64().unwrap();
    let object = shape(&s).format.object.unwrap();
    let periods = crate::drawing_scene::timeline_periods(&s.active().unwrap().wb, 0, &object);
    assert_eq!(periods.len(), 3);
    let report = s.active().unwrap().wb.active().unwrap().pivots[0].last_range.unwrap();
    let total = CellRef::new(report.start.row + 1, report.end.col);
    s.execute("cell.set", json!({"cell":"K15","input":format!("={}*2",total.a1())})).unwrap();
    assert_eq!(v(&s, "K15"), Value::Number(60.));
    s.execute("timeline.filter", json!({"id":id,"start":periods[0].0,"end":periods[0].1})).unwrap();
    assert_eq!(v(&s, "K15"), Value::Number(20.));
    let (wb, r) = roundtrip(&s);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    assert_eq!(shape(&s).format.object, wb.active().unwrap().shapes[0].format.object);
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(v(&s, "K15"), Value::Number(60.));
    s.execute("edit.redo", json!({})).unwrap();
    assert_eq!(v(&s, "K15"), Value::Number(20.));
    s.execute("timeline.filter", json!({"id":id,"clear":true})).unwrap();
    assert_eq!(v(&s, "K15"), Value::Number(60.));
    s.execute("timeline.level", json!({"id":id,"level":"years"})).unwrap();
    assert!(matches!(shape(&s).format.object, Some(DrawingObject::Timeline { level: TimelineLevel::Years, .. })));
    assert!(s.execute("insert.timeline", json!({"column":"Sales"})).is_err());
    s.execute("timeline.level", json!({"id":id,"level":"days"})).unwrap();
    let before = crate::drawing_scene::timeline_periods(&s.active().unwrap().wb, 0, &shape(&s).format.object.unwrap());
    s.execute("timeline.scroll", json!({"id":id,"delta":6})).unwrap();
    let after = crate::drawing_scene::timeline_periods(&s.active().unwrap().wb, 0, &shape(&s).format.object.unwrap());
    assert!(after[0].0 > before[0].0);
    assert_eq!(after.len(), 12);
    assert_eq!(roundtrip(&s).0.active().unwrap().shapes[0].format.object, shape(&s).format.object);
    assert!(s.execute("timeline.filter", json!({"id":id,"start":"2026-02-31","end":"2026-03-01"})).is_err());
    s.execute("timeline.filter", json!({"id":id,"start":"2026-01-01","end":"2026-01-31"})).unwrap();
}
#[test]
fn moved_page_break_is_atomic_and_saved() {
    let mut s = s();
    s.execute("pageLayout.breaks", json!({"cell":"D20","insert":true})).unwrap();
    s.execute("pageLayout.moveBreak", json!({"axis":"row","from":19,"to":25})).unwrap();
    assert_eq!(s.active().unwrap().wb.active().unwrap().print.row_breaks, vec![25]);
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.active().unwrap().wb.active().unwrap().print.row_breaks, vec![19]);
    s.execute("edit.redo", json!({})).unwrap();
    assert_eq!(roundtrip(&s).0.active().unwrap().print.row_breaks, vec![25]);
}

#[test]
fn imported_timeline_viewport_outside_source_dates_remains_scrollable() {
    let mut s = date_pivot();
    s.execute("insert.timeline", json!({"pivot":"SalesPivot","column":"Date"})).unwrap();
    let mut wb = (*s.active().unwrap().wb).clone();
    let june = joysheet_core::date::serial_from_ymd(wb.date_system, 2026, 6, 30).unwrap() as i32;
    let sh = &mut wb.sheet_mut(0).unwrap().shapes[0];
    let id = sh.id;
    let Some(DrawingObject::Timeline { scroll_start, .. }) = &mut sh.format.object else { panic!("missing timeline") };
    // The locally authored Excel reference stores June 30 despite Jan–Mar data.
    *scroll_start = Some(june);
    let bytes = joysheet_xlsx::write_xlsx(&wb).unwrap();
    s.execute("file.open", json!({"name":"outside-viewport.xlsx","base64":crate::io::base64_encode(&bytes)})).unwrap();
    let before = crate::drawing_scene::timeline_periods(&s.active().unwrap().wb, 0, &shape(&s).format.object.unwrap());
    assert_eq!(before.len(), 1);
    s.execute("timeline.scroll", json!({"id":id,"delta":-1})).unwrap();
    let after = crate::drawing_scene::timeline_periods(&s.active().unwrap().wb, 0, &shape(&s).format.object.unwrap());
    assert!(after[0].0 < before[0].0);
    assert_eq!(after.len(), 2);
}

#[test]
fn interop_fixture_contains_all_native_objects() {
    let mut s = date_pivot();
    for (id, p) in [
        ("insert.timeline", json!({"pivot":"SalesPivot","column":"Date","at":"J2"})),
        ("insert.smartArt", json!({"layout":"process","text":"计划\n执行\n验证","at":"A10"})),
        ("insert.wordArt", json!({"text":"JoyOffice","warp":"textArchUp","at":"A22"})),
        ("insert.equation", json!({"text":"x=\\frac{-b+\\sqrt{b^{2}-4ac}}{2a}","at":"F22"})),
        ("insert.object", json!({"name":"Example.txt","base64":crate::io::base64_encode(b"Embedded bytes: JoyOffice\r\n"),"at":"J12"})),
    ] {
        s.execute(id, p).unwrap();
    }
    s.execute("pageLayout.headerFooter", json!({"header":"&CJoyOffice","footer":"&C&P / &N"})).unwrap();
    let bytes = joysheet_xlsx::write_xlsx_named(&s.active().unwrap().wb, joysheet_xlsx::XlsxKind::Workbook, "objects-native.xlsx").unwrap();
    let (wb, r) = joysheet_xlsx::read_xlsx(&bytes).unwrap();
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    assert_eq!(wb.active().unwrap().shapes.len(), 5);
    if let Some(path) = std::env::var_os("JOYSHEET_INTEROP_OUTPUT_DIR") {
        let path = std::path::PathBuf::from(path);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("objects-native.xlsx"), bytes).unwrap();
        for kind in ["smartart", "wordart", "equation", "embedded", "timeline"] {
            let mut isolated = (*s.active().unwrap().wb).clone();
            isolated.sheet_mut(0).unwrap().shapes.retain(|shape| {
                matches!(
                    (kind, shape.format.object.as_ref()),
                    ("smartart", Some(DrawingObject::SmartArt { .. }))
                        | ("wordart", Some(DrawingObject::WordArt { .. }))
                        | ("equation", Some(DrawingObject::Equation { .. }))
                        | ("embedded", Some(DrawingObject::Embedded { .. }))
                        | ("timeline", Some(DrawingObject::Timeline { .. }))
                )
            });
            std::fs::write(path.join(format!("interop-{kind}.xlsx")), joysheet_xlsx::write_xlsx(&isolated).unwrap()).unwrap();
        }
    }
}
