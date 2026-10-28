use joyoffice_drawingml::{
    DrawingBudget, DrawingShape, Fill, ShapeGeometry, Stroke, parse_drawing_xml,
};

#[test]
fn test_shape_serialization_and_parsing() {
    let shape = DrawingShape {
        id: 42,
        name: "Test Rectangle".to_string(),
        geometry: ShapeGeometry::Rectangle,
        width_pt: 200.0,
        height_pt: 150.0,
        fill: Fill::Solid("FF0000".to_string()),
        stroke: Stroke {
            color: Some("000000".to_string()),
            width_pt: Some(2.0),
        },
        textbox_content: Some("Sample Textbox Content".to_string()),
    };

    let xml = shape.to_drawing_xml();
    assert!(xml.contains("<w:drawing"));
    assert!(xml.contains("prst=\"rect\""));
    assert!(xml.contains("val=\"FF0000\""));
    assert!(xml.contains("<w:txbxContent>Sample Textbox Content</w:txbxContent>"));

    let budget = DrawingBudget::default();
    let parsed = parse_drawing_xml(&xml, &budget).expect("parse drawing");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].id, 42);
    assert_eq!(parsed[0].name, "Test Rectangle");
    assert_eq!(parsed[0].geometry, ShapeGeometry::Rectangle);
    assert_eq!(parsed[0].fill, Fill::Solid("FF0000".to_string()));
    assert_eq!(
        parsed[0].textbox_content.as_deref(),
        Some("Sample Textbox Content")
    );
}

#[test]
fn test_drawing_budget_protection() {
    let xml = r#"
    <w:drawing xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
               xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
               xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">
        <wp:inline>
            <wp:extent cx="1000" cy="1000"/>
            <wp:docPr id="1" name="Shape 1"/>
            <wps:wsp/>
        </wp:inline>
    </w:drawing>
    "#;

    let tiny_budget = DrawingBudget {
        max_shapes: 0,
        max_depth: 32,
    };
    let err = parse_drawing_xml(xml, &tiny_budget).unwrap_err();
    assert!(err.to_string().contains("budget exceeded"));
}
