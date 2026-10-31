use super::*;

#[test]
fn resolving() {
    assert_eq!(resolve("xl", "worksheets/sheet1.xml"), "xl/worksheets/sheet1.xml");
    assert_eq!(resolve("xl/worksheets", "../drawings/d1.xml"), "xl/drawings/d1.xml");
    assert_eq!(resolve("xl/worksheets", "/xl/tables/t.xml"), "xl/tables/t.xml");
    assert_eq!(resolve("", "xl/workbook.xml"), "xl/workbook.xml");
    assert_eq!(resolve("a", "../../../x"), "x");
    assert_eq!(split_dir("xl/workbook.xml"), ("xl", "workbook.xml"));
    assert_eq!(split_dir("x.xml"), ("", "x.xml"));
}

/// An over-budget read must spend the package's allowance once and for all: loops over many
/// broken parts (a sheet's optional VML relationships, for one) otherwise re-decompress the
/// remaining allowance again and again.
#[test]
fn a_failed_part_ends_the_package_budget() {
    let z = crate::tests::make_zip(&[("a.xml", b"0123456789"), ("b.xml", b"hello")]);
    let mut pkg = Package::open(&z).unwrap();
    assert!(pkg.read("a.xml").unwrap().is_some());
    // Nothing left to spend within the per-part limit either.
    pkg.total = MAX_TOTAL - 1;
    assert!(pkg.read("b.xml").is_err());
    assert_eq!(pkg.total, MAX_TOTAL, "the failed read is charged");
    // Oversized repeated attempts stay cheap errors now: no allowance is left to pay for.
    assert!(matches!(pkg.read("b.xml"), Err(IoError::TooLarge(_))));
}
