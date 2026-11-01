use super::*;

fn parse(body: &str) -> Result<Option<Preset>> {
    let text = format!(r#"<a:spPr xmlns:a="{A}">{body}</a:spPr>"#);
    let document = roxmltree::Document::parse(&text).unwrap();
    read(document.root_element())
}

#[test]
fn named_adjustments_are_mapped_by_index_instead_of_xml_order() {
    let p = parse(r#"<a:prstGeom prst="leftRightArrow"><a:avLst><a:gd name="adj2" fmla="val 30000"/><a:gd name="adj1" fmla="val 60000"/></a:avLst></a:prstGeom>"#).unwrap().unwrap();
    assert_eq!(p.adjustments, vec![60000., 30000.]);
}

#[test]
fn unknown_duplicate_and_nonliteral_geometry_is_explicitly_rejected() {
    for body in [
        r#"<a:prstGeom prst="unknown"><a:avLst/></a:prstGeom>"#,
        r#"<a:prstGeom prst="triangle"><a:avLst/><a:avLst/></a:prstGeom>"#,
        r#"<a:prstGeom prst="triangle" extension="unknown"><a:avLst/></a:prstGeom>"#,
        r#"<a:prstGeom prst="triangle"><a:avLst><a:gd name="adj" fmla="val 12"><a:unknown/></a:gd></a:avLst></a:prstGeom>"#,
        r#"<a:prstGeom prst="triangle"><a:avLst><a:gd name="adj" fmla="val 12"/><a:gd name="adj" fmla="val 13"/></a:avLst></a:prstGeom>"#,
        r#"<a:prstGeom prst="triangle"><a:avLst><a:gd name="adj" fmla="val 1 2"/></a:avLst></a:prstGeom>"#,
        r#"<a:prstGeom prst="triangle"><a:avLst><a:gd name="adj" fmla="val 1000000001"/></a:avLst></a:prstGeom>"#,
        r#"<a:prstGeom prst="triangle"><a:avLst><a:gd name="adj" fmla="*/ w 1 2"/></a:avLst></a:prstGeom>"#,
    ] {
        assert!(parse(body).is_err(), "{body}");
    }
}
