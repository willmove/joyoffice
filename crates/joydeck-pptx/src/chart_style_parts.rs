//! Default ChartEx style and palette parts, following MS-ODRAWXML §2.8.

use crate::opc::{self, NS_A, PackageOut, RelsOut};
use crate::xml::{A, W};

pub(super) const STYLE_REL: &str =
    "http://schemas.microsoft.com/office/2011/relationships/chartStyle";
pub(super) const PALETTE_REL: &str =
    "http://schemas.microsoft.com/office/2011/relationships/chartColorStyle";
const NS_STYLE: &str = "http://schemas.microsoft.com/office/drawing/2012/chartStyle";

pub(super) fn ensure(
    package: &mut PackageOut,
    owner: &str,
    index: u32,
    relationships: &mut RelsOut,
) {
    for (kind, stem, content_type, build) in [
        (
            STYLE_REL,
            "style",
            "application/vnd.ms-office.chartstyle+xml",
            style as fn() -> Vec<u8>,
        ),
        (
            PALETTE_REL,
            "colors",
            "application/vnd.ms-office.chartcolorstyle+xml",
            colors,
        ),
    ] {
        relationships
            .items
            .retain(|(_, content, _, external)| content != kind || !external);
        if relationships.items.iter().any(|(_, ty, ..)| ty == kind) {
            continue;
        }
        let path = format!("ppt/charts/joydeck-{stem}-{index}.xml");
        package.xml_part(&path, content_type, build());
        crate::chart_sidecars::add_internal(relationships, kind, &opc::relative(owner, &path));
    }
}

fn style() -> Vec<u8> {
    let mut xml = W::new();
    xml.open(
        "cs:chartStyle",
        A::new()
            .a("xmlns:cs", NS_STYLE)
            .a("xmlns:a", NS_A)
            .a("id", 410),
    );
    // CT_ChartStyle has one required entry per role, in schema order.
    for name in [
        "axisTitle",
        "categoryAxis",
        "chartArea",
        "dataLabel",
        "dataLabelCallout",
        "dataPoint",
        "dataPoint3D",
        "dataPointLine",
        "dataPointMarker",
        "dataPointMarkerLayout",
        "dataPointWireframe",
        "dataTable",
        "downBar",
        "dropLine",
        "errorBar",
        "floor",
        "gridlineMajor",
        "gridlineMinor",
        "hiLoLine",
        "leaderLine",
        "legend",
        "plotArea",
        "plotArea3D",
        "seriesAxis",
        "seriesLine",
        "title",
        "trendline",
        "trendlineLabel",
        "upBar",
        "valueAxis",
        "wall",
    ] {
        let tag = format!("cs:{name}");
        if name == "dataPointMarkerLayout" {
            xml.empty(&tag, A::new().a("symbol", "circle").a("size", 5));
            continue;
        }
        let data = matches!(
            name,
            "dataPoint" | "dataPoint3D" | "dataPointMarker" | "downBar" | "upBar"
        );
        let data_line = matches!(name, "dataPointLine" | "seriesLine" | "trendline");
        let guide = matches!(
            name,
            "gridlineMajor" | "gridlineMinor" | "dropLine" | "leaderLine" | "errorBar" | "hiLoLine"
        );
        xml.open0(&tag);
        if data_line {
            automatic_ref(&mut xml, "cs:lnRef");
        } else if guide {
            xml.open("cs:lnRef", A::new().a("idx", 1));
            xml.open("a:schemeClr", A::new().a("val", "tx1"));
            xml.val(
                "a:alpha",
                if name == "gridlineMinor" {
                    14_000
                } else {
                    28_000
                },
            );
            xml.close("a:schemeClr");
            xml.close("cs:lnRef");
        } else {
            xml.empty("cs:lnRef", A::new().a("idx", 0));
        }
        if data {
            automatic_ref(&mut xml, "cs:fillRef");
        } else {
            xml.empty("cs:fillRef", A::new().a("idx", 0));
        }
        xml.empty("cs:effectRef", A::new().a("idx", 0));
        xml.open("cs:fontRef", A::new().a("idx", "minor"));
        xml.val("a:schemeClr", "tx1");
        xml.close("cs:fontRef");
        if let Some(size) = match name {
            "title" => Some(1800),
            "chartArea" => Some(1300),
            "dataLabel" | "dataLabelCallout" | "dataTable" | "legend" | "axisTitle"
            | "categoryAxis" | "seriesAxis" | "valueAxis" | "trendlineLabel" => Some(1100),
            _ => None,
        } {
            xml.empty("cs:defRPr", A::new().a("sz", size));
        }
        xml.close(&tag);
    }
    xml.close("cs:chartStyle");
    xml.finish()
}

fn automatic_ref(xml: &mut W, tag: &str) {
    xml.open(tag, A::new().a("idx", 1));
    xml.val("cs:styleClr", "auto");
    xml.close(tag);
}

fn colors() -> Vec<u8> {
    let mut xml = W::new();
    xml.open(
        "cs:colorStyle",
        A::new()
            .a("xmlns:cs", NS_STYLE)
            .a("xmlns:a", NS_A)
            .a("meth", "cycle")
            .a("id", 10),
    );
    for index in 1..=6 {
        xml.val("a:schemeClr", format!("accent{index}"));
    }
    xml.empty0("cs:variation");
    for shade in [80_000, 64_000, 46_000] {
        xml.open0("cs:variation");
        xml.val("a:lumMod", shade);
        xml.close("cs:variation");
    }
    xml.close("cs:colorStyle");
    xml.finish()
}
