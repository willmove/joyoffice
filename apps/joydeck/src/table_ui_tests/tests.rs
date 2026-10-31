use super::*;

#[test]
fn stale_table_resize_is_rejected_without_changing_the_document() {
    let base = edit::object("rect");
    let resize = Resize {
        base: base.clone(),
        row: true,
        index: 0,
        start: 0.,
        delta: 10.,
        scale: 1.,
    };
    assert!(resize.result().is_err());
    assert_eq!(resize.base, base);
    let table = table::object(2, 2).unwrap();
    let resize = Resize {
        base: table.clone(),
        row: true,
        index: 3,
        start: 0.,
        delta: 10.,
        scale: 1.,
    };
    assert!(resize.result().is_err());
    assert_eq!(resize.base, table);
}

#[test]
fn valid_table_resize_remains_a_preview_until_committed() {
    let base = table::object(2, 2).unwrap();
    let original_height = base.height;
    let resize = Resize {
        base: base.clone(),
        row: true,
        index: 0,
        start: 0.,
        delta: 10.,
        scale: 1.,
    };
    let preview = resize.result().unwrap();
    assert_eq!(resize.base, base);
    assert_eq!(preview.height, original_height + 10.);
}
