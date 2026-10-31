use super::*;

#[test]
fn a_wide_range_beside_a_column_of_values_is_a_multi_level_axis() {
    let r = |s: &str| RangeRef::parse(s).unwrap();
    assert_eq!(category_levels(&r("A4:B39"), Some(&r("C4:C39"))), Some(CategoryLevels { by_columns: true, outer: 1 }));
    assert_eq!(category_levels(&r("A4:D39"), Some(&r("C4:C39"))), Some(CategoryLevels { by_columns: true, outer: 3 }));
    assert_eq!(category_levels(&r("A4:F39"), Some(&r("C4:C39"))).map(|l| l.outer), Some(3), "capped");
    assert_eq!(category_levels(&r("A4:A39"), Some(&r("C4:C39"))), None, "one column: a plain list");
    // A row of values: levels are rows.
    assert_eq!(category_levels(&r("B2:M3"), Some(&r("B4:M4"))), Some(CategoryLevels { by_columns: false, outer: 1 }));
    assert_eq!(category_levels(&r("B3:M3"), Some(&r("B4:M4"))), None);
    // Literal values: the categories' own shape decides.
    assert_eq!(category_levels(&r("A1:B6"), None), Some(CategoryLevels { by_columns: true, outer: 1 }));
    assert_eq!(category_levels(&r("A1:F2"), None), Some(CategoryLevels { by_columns: false, outer: 1 }));
    assert_eq!(category_levels(&r("A:B"), Some(&r("C:C"))), None, "whole columns");
}
