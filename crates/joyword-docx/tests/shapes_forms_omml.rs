use joyoffice_drawingml::{
    DrawingBudget, DrawingShape, Fill, ShapeGeometry, Stroke, parse_drawing_xml,
};
use joyoffice_omml::{OmmlBudget, parse_omml_xml};
use joyword_model::{Block, BlockSeq, Document, Sdt, SdtLock, SdtProps};
use std::sync::Arc;

#[test]
fn test_omml_equations_integration() {
    let math_xml = r#"
    <m:oMath xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
        <m:f>
            <m:num>
                <m:sSup>
                    <m:e><m:r><m:t>x</m:t></m:r></m:e>
                    <m:sup><m:r><m:t>2</m:t></m:r></m:sup>
                </m:sSup>
            </m:num>
            <m:den>
                <m:rad>
                    <m:radPr><m:degHide m:val="on"/></m:radPr>
                    <m:deg/>
                    <m:e><m:r><m:t>y + 1</m:t></m:r></m:e>
                </m:rad>
            </m:den>
        </m:f>
    </m:oMath>
    "#;

    let budget = OmmlBudget::default();
    let elem = parse_omml_xml(math_xml, &budget).expect("parse omml");
    let linear = elem.to_linear_text();
    assert_eq!(linear, "((x)^(2))/(\\sqrt{y + 1})");

    let out_xml = elem.to_omml_xml();
    assert!(out_xml.contains("<m:f>"));
    assert!(out_xml.contains("<m:sSup>"));
    assert!(out_xml.contains("<m:rad>"));
}

#[test]
fn test_drawingml_textbox_and_shapes_integration() {
    let shape = DrawingShape {
        id: 101,
        name: "Callout Textbox".to_string(),
        geometry: ShapeGeometry::RoundRectangle,
        width_pt: 250.0,
        height_pt: 120.0,
        fill: Fill::Solid("EBF1F5".to_string()),
        stroke: Stroke {
            color: Some("2A5B84".to_string()),
            width_pt: Some(1.5),
        },
        textbox_content: Some("<w:p><w:r><w:t>Form Field Label</w:t></w:r></w:p>".to_string()),
    };

    let xml = shape.to_drawing_xml();
    assert!(xml.contains("prst=\"roundRect\""));
    assert!(xml.contains("val=\"EBF1F5\""));
    assert!(xml.contains("val=\"2A5B84\""));
    assert!(xml.contains(
        "<w:txbxContent><w:p><w:r><w:t>Form Field Label</w:t></w:r></w:p></w:txbxContent>"
    ));

    let budget = DrawingBudget::default();
    let shapes = parse_drawing_xml(&xml, &budget).expect("parse drawingml");
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].id, 101);
    assert_eq!(shapes[0].name, "Callout Textbox");
    assert_eq!(shapes[0].geometry, ShapeGeometry::RoundRectangle);
    assert_eq!(shapes[0].fill, Fill::Solid("EBF1F5".to_string()));
    assert_eq!(shapes[0].stroke.color.as_deref(), Some("2A5B84"));
}

#[test]
fn test_sdt_content_control_lock_semantics() {
    let mut doc = Document::new_blank();
    let node_id = doc.ids().node();

    let sdt = Sdt {
        id: node_id,
        props: SdtProps {
            tag: Some(Arc::from("invoice_number")),
            alias: Some(Arc::from("Invoice Number")),
            lock: SdtLock::SdtContentLocked,
            preserved: None,
        },
        blocks: BlockSeq::default(),
        source: None,
    };

    let block = Block::Sdt(Arc::new(sdt));
    let Block::Sdt(ref inner_sdt) = block else {
        panic!("expected sdt");
    };

    assert!(!inner_sdt.props.lock.content_editable());
    assert_eq!(inner_sdt.props.tag.as_deref(), Some("invoice_number"));
    assert_eq!(inner_sdt.props.alias.as_deref(), Some("Invoice Number"));
}
