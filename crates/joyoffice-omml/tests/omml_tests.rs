use joyoffice_omml::{MathElement, OmmlBudget, parse_omml_xml};

#[test]
fn test_fraction_and_linear_mapping() {
    let xml = r#"
    <m:oMath xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
        <m:f>
            <m:num><m:r><m:t>a + b</m:t></m:r></m:num>
            <m:den><m:r><m:t>c - d</m:t></m:r></m:den>
        </m:f>
    </m:oMath>
    "#;

    let budget = OmmlBudget::default();
    let elem = parse_omml_xml(xml, &budget).expect("parse omml");

    assert!(matches!(&elem, MathElement::Fraction { .. }));
    assert_eq!(elem.to_linear_text(), "(a + b)/(c - d)");

    let serialized = elem.to_omml_xml();
    assert!(serialized.contains("<m:f>"));
    assert!(serialized.contains("<m:num>"));
    assert!(serialized.contains("<m:den>"));
}

#[test]
fn test_subscript_superscript_and_radicals() {
    let xml = r#"
    <m:oMath xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
        <m:sSubSup>
            <m:e><m:r><m:t>x</m:t></m:r></m:e>
            <m:sub><m:r><m:t>i</m:t></m:r></m:sub>
            <m:sup><m:r><m:t>2</m:t></m:r></m:sup>
        </m:sSubSup>
    </m:oMath>
    "#;

    let budget = OmmlBudget::default();
    let elem = parse_omml_xml(xml, &budget).expect("parse subsup");
    assert_eq!(elem.to_linear_text(), "(x)_(i)^(2)");

    let sqrt_xml = r#"
    <m:oMath xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
        <m:rad>
            <m:radPr><m:degHide m:val="on"/></m:radPr>
            <m:deg/>
            <m:e><m:r><m:t>16</m:t></m:r></m:e>
        </m:rad>
    </m:oMath>
    "#;
    let sqrt_elem = parse_omml_xml(sqrt_xml, &budget).expect("parse radical");
    assert_eq!(sqrt_elem.to_linear_text(), "\\sqrt{16}");
}

#[test]
fn test_delimiter_and_budget() {
    let xml = r#"
    <m:oMath xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
        <m:d>
            <m:dPr>
                <m:begChr m:val="["/>
                <m:endChr m:val="]"/>
            </m:dPr>
            <m:e><m:r><m:t>matrix_row</m:t></m:r></m:e>
        </m:d>
    </m:oMath>
    "#;

    let budget = OmmlBudget::default();
    let elem = parse_omml_xml(xml, &budget).expect("parse delimiter");
    assert_eq!(elem.to_linear_text(), "[matrix_row]");

    // Test budget limit
    let tiny_budget = OmmlBudget {
        max_depth: 2,
        max_nodes: 2,
    };
    let err = parse_omml_xml(xml, &tiny_budget).unwrap_err();
    assert!(err.to_string().contains("exceeded limit"));
}
