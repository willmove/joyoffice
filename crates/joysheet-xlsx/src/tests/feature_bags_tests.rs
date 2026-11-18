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

#[test]
fn mapper_is_named_for_excel_and_still_reads_the_previous_writers_chain() {
    let source = checkbox_part();
    let root = crate::xml::parse(source.as_bytes()).unwrap();
    let mapper = root.kids("bag").find(|b| b.attr("type") == Some("XFComplements")).unwrap();
    assert_eq!(mapper.attr("extRef"), Some("XFComplementsMapperExtRef"));
    let previous = source.replace(" extRef=\"XFComplementsMapperExtRef\"", "");
    let other = "<bag type=\"XFComplements\" extRef=\"OtherMapper\"><a k=\"MappedFeaturePropertyBags\"><bagId>0</bagId></a></bag>";
    let two = source.replace("<bag type=\"XFComplements\"", &format!("{other}<bag type=\"XFComplements\""));
    let renamed = source.replace("XFComplementsMapperExtRef", "OtherMapper");
    for xml in [previous, two, renamed] {
        assert_eq!(checkbox_complements(&crate::xml::parse(xml.as_bytes()).unwrap()), [true]);
    }
    let forward = source.replace(">0</bagId>", ">4</bagId>").replace("</FeaturePropertyBags>", "<bag type=\"Checkbox\"/></FeaturePropertyBags>");
    assert_eq!(checkbox_complements(&crate::xml::parse(forward.as_bytes()).unwrap()), [false]);
}
