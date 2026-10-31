use super::*;

#[test]
fn written_chain_resolves_and_hostile_chains_do_not() {
    let root = crate::xml::parse(checkbox_part().as_bytes()).unwrap_or_default();
    assert_eq!(checkbox_complements(&root), vec![true]);
    // Out-of-range, wrong-type and self references resolve to false without panicking.
    let bad = format!(
        "<FeaturePropertyBags xmlns=\"{NS}\"><bag type=\"XFControls\"><bagId k=\"CellControl\">99</bagId></bag><bag type=\"XFComplement\"><bagId k=\"XFControls\">1</bagId></bag><bag type=\"XFComplements\"><a k=\"MappedFeaturePropertyBags\"><bagId>1</bagId><bagId>4294967295</bagId><bagId>x</bagId></a></bag></FeaturePropertyBags>"
    );
    let root = crate::xml::parse(bad.as_bytes()).unwrap_or_default();
    assert_eq!(checkbox_complements(&root), vec![false, false, false]);
}
