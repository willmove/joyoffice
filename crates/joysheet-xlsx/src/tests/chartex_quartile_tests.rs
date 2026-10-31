use super::*;

#[test]
fn box_whisker_reads_quartile_method_and_defaults() {
    for (statistics, expected) in [
        ("", QuartileMethod::Exclusive),
        ("<cx:statistics/>", QuartileMethod::Exclusive),
        (r#"<cx:statistics quartileMethod="unknown"/>"#, QuartileMethod::Exclusive),
        (r#"<cx:statistics quartileMethod="exclusive"/>"#, QuartileMethod::Exclusive),
        (r#"<cx:statistics quartileMethod="inclusive"/>"#, QuartileMethod::Inclusive),
    ] {
        let xml = format!(
            r#"<cx:chartSpace xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex"><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="boxWhisker"><cx:layoutPr>{statistics}</cx:layoutPr></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"#
        );
        let root = crate::xml::parse(xml.as_bytes()).unwrap();
        let chart = read_chartex(&root).unwrap();
        assert_eq!(chart.series[0].quartiles, expected);
    }
}
