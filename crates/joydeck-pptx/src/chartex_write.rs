//! Office extended chart parts and their literal or workbook-backed dimensions.

use joydeck_model::chart::{Chart, ChartType, MAX_LEVELS};

use crate::chart_cache::format_number;
use crate::chart_workbook::{ChartWorkbook, name_cell, range};
use crate::opc::{NS_A, NS_R};
use crate::xml::{A, El, W};

pub(super) const NS_CX: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";

pub(super) fn series_count(chart: &Chart) -> usize {
    if chart.kind == ChartType::BoxWhisker {
        chart.series.len()
    } else {
        chart.series.len().min(1)
    }
}

pub(super) fn retained(chart: &Chart) -> Option<Vec<u8>> {
    Layout::of(chart.kind)?;
    let raw = chart.raw.as_ref()?;
    let doc = crate::xml::parse(raw.as_bytes()).ok()?;
    let mut resolved = doc.root.clone();
    crate::read::resolve_mc(&mut resolved, 0);
    let original = crate::read::chartex_from(&resolved)?;
    let mut current = chart.clone();
    current.raw = None;
    current.kept = Default::default();
    if original != current {
        return None;
    }
    let mut root = doc.root;
    crate::chart_sidecars::drop_missing_refs(&mut root, chart);
    // Part XML is retained independently of the new slide's relationship numbering.
    let mut output = W::new();
    output.raw(&root.to_xml());
    Some(output.finish())
}

#[derive(Clone, Copy)]
struct Layout {
    role: &'static str,
    name: &'static str,
    axes: u8,
    label_position: &'static str,
    category_labels: bool,
}

impl Layout {
    fn of(kind: ChartType) -> Option<Self> {
        use ChartType::*;
        let (name, role, axes, label_position, category_labels) = match kind {
            Treemap => ("treemap", "size", 0, "inEnd", true),
            Sunburst => ("sunburst", "size", 0, "ctr", true),
            RegionMap => ("regionMap", "colorVal", 0, "ctr", true),
            Funnel => ("funnel", "val", 1, "ctr", false),
            Waterfall => ("waterfall", "val", 2, "outEnd", false),
            Histogram => ("clusteredColumn", "val", 2, "outEnd", false),
            Pareto => ("clusteredColumn", "val", 3, "outEnd", false),
            BoxWhisker => ("boxWhisker", "val", 2, "outEnd", false),
            _ => return None,
        };
        Some(Self {
            role,
            name,
            axes,
            label_position,
            category_labels,
        })
    }
}

pub(super) fn encode(
    chart: &Chart,
    path: &str,
    workbook: Option<(&ChartWorkbook<'_>, &str)>,
) -> Option<Vec<u8>> {
    let layout = Layout::of(chart.kind)?;
    let series_count = series_count(chart);
    if series_count == 0 {
        return None;
    }
    let mut xml = W::new();
    xml.open(
        "cx:chartSpace",
        A::new()
            .a("xmlns:a", NS_A)
            .a("xmlns:r", NS_R)
            .a("xmlns:cx", NS_CX),
    );
    xml.open0("cx:chartData");
    if let Some((_, link)) = workbook {
        xml.empty(
            "cx:externalData",
            A::new().a("r:id", link).a("autoUpdate", 0),
        );
    }
    for (id, series) in chart.series.iter().take(series_count).enumerate() {
        xml.open("cx:data", A::new().a("id", id));
        let sheets = workbook.map(|(sheet, _)| sheet);
        if !chart.categories.is_empty() || !chart.category_levels.is_empty() {
            xml.open("cx:strDim", A::new().a("type", "cat"));
            let longest = chart
                .category_levels
                .iter()
                .take(MAX_LEVELS)
                .map(Vec::len)
                .chain(std::iter::once(chart.categories.len()))
                .max()
                .unwrap_or_default();
            if let Some(sheet) = sheets {
                formula(&mut xml, &sheet.categories(longest));
            }
            for level in std::iter::once(&chart.categories)
                .chain(chart.category_levels.iter().take(MAX_LEVELS))
            {
                xml.open("cx:lvl", A::new().a("ptCount", level.len()));
                for (index, value) in level.iter().enumerate() {
                    point(&mut xml, index, value);
                }
                xml.close("cx:lvl");
            }
            xml.close("cx:strDim");
        }
        xml.open("cx:numDim", A::new().a("type", layout.role));
        if let Some(column) = sheets.and_then(|sheet| sheet.columns(id)) {
            formula(
                &mut xml,
                &range(column.values, column.values, series.values.len()),
            );
        }
        xml.open(
            "cx:lvl",
            A::new().a("ptCount", series.values.len()).a(
                "formatCode",
                series.number_format.as_deref().unwrap_or("General"),
            ),
        );
        for (index, value) in series.values.iter().enumerate() {
            if let Some(value) = value.filter(|number| number.is_finite()) {
                point(&mut xml, index, &format_number(value));
            }
        }
        xml.close("cx:lvl");
        xml.close("cx:numDim");
        xml.close("cx:data");
    }
    xml.close("cx:chartData");
    xml.open0("cx:chart");
    if let Some(caption) = &chart.title {
        xml.open(
            "cx:title",
            A::new().a("pos", "t").a("align", "ctr").a("overlay", 0),
        );
        rich_title(&mut xml, caption);
        xml.close("cx:title");
    }
    xml.open0("cx:plotArea");
    xml.open0("cx:plotAreaRegion");
    let raw = chart
        .raw
        .as_ref()
        .and_then(|raw| crate::xml::parse(raw.as_bytes()).ok());
    let old_series: Vec<&El> = raw
        .as_ref()
        .and_then(|doc| doc.root.path(&["chart", "plotArea", "plotAreaRegion"]))
        .into_iter()
        .flat_map(|region| region.children_named("series"))
        .filter(|series| series.attr("layoutId") == Some(layout.name))
        .collect();
    for (id, series) in chart.series.iter().take(series_count).enumerate() {
        let old = old_series.get(id).copied();
        let unique_id = old
            .and_then(|node| node.attr("uniqueId"))
            .filter(|id| id.len() == 38 && id.starts_with('{') && id.ends_with('}'))
            .map_or_else(
                || crate::tables::guid(&format!("{path}:series:{id}")),
                str::to_owned,
            );
        xml.open(
            "cx:series",
            A::new().a("layoutId", layout.name).a("uniqueId", unique_id),
        );
        if !series.name.is_empty() {
            xml.open0("cx:tx");
            xml.open0("cx:txData");
            if let Some(column) = workbook.and_then(|(sheet, _)| sheet.columns(id)) {
                formula(&mut xml, &name_cell(column.values));
            }
            xml.elt("cx:v", &series.name);
            xml.close("cx:txData");
            xml.close("cx:tx");
        }
        if chart.series_data_labels(series) {
            xml.open("cx:dataLabels", A::new().a("pos", layout.label_position));
            xml.empty(
                "cx:visibility",
                A::new()
                    .a("seriesName", 0)
                    .a("categoryName", u8::from(layout.category_labels))
                    .a("value", u8::from(!layout.category_labels)),
            );
            xml.close("cx:dataLabels");
        }
        xml.val("cx:dataId", id);
        series_layout(
            &mut xml,
            chart,
            old,
            raw.as_ref().map(crate::xml::Doc::ns_decls).as_deref(),
        );
        xml.close("cx:series");
    }
    if chart.kind == ChartType::Pareto {
        xml.open(
            "cx:series",
            A::new()
                .a("layoutId", "paretoLine")
                .a("ownerIdx", 0)
                .a("uniqueId", crate::tables::guid(&format!("{path}:pareto"))),
        );
        xml.val("cx:axisId", 2);
        xml.close("cx:series");
    }
    xml.close("cx:plotAreaRegion");
    write_axes(&mut xml, chart, layout);
    xml.close("cx:plotArea");
    if let Some(pos) = &chart.legend {
        let pos = match pos.as_str() {
            side @ ("b" | "t" | "l" | "r") => side,
            _ => "r",
        };
        xml.empty(
            "cx:legend",
            A::new()
                .a("pos", pos)
                .a("align", "ctr")
                .a("overlay", u8::from(chart.legend_overlay)),
        );
    }
    xml.close("cx:chart");
    xml.close("cx:chartSpace");
    Some(xml.finish())
}

fn formula(xml: &mut W, value: &str) {
    xml.open("cx:f", A::new().a("dir", "col"));
    xml.text(value);
    xml.close("cx:f");
}

fn point(xml: &mut W, index: usize, value: &str) {
    xml.open("cx:pt", A::new().a("idx", index));
    xml.text(value);
    xml.close("cx:pt");
}

fn rich_title(xml: &mut W, text: &str) {
    xml.open0("cx:tx");
    xml.open0("cx:rich");
    xml.empty0("a:bodyPr");
    xml.empty0("a:lstStyle");
    for line in text.split('\n') {
        xml.open0("a:p");
        xml.open0("a:r");
        xml.elt("a:t", line);
        xml.close("a:r");
        xml.close("a:p");
    }
    xml.close("cx:rich");
    xml.close("cx:tx");
}

fn series_layout(xml: &mut W, chart: &Chart, old: Option<&El>, ns: Option<&[(String, String)]>) {
    match chart.kind {
        ChartType::Treemap => {
            let label = old
                .and_then(|series| series.path(&["layoutPr", "parentLabelLayout"]))
                .and_then(|node| node.attr("val"))
                .filter(|value| matches!(*value, "banner" | "none" | "overlapping"))
                .unwrap_or("overlapping");
            xml.open0("cx:layoutPr");
            xml.val("cx:parentLabelLayout", label);
            xml.close("cx:layoutPr");
        }
        ChartType::Waterfall if !chart.subtotals.is_empty() => {
            xml.open0("cx:layoutPr");
            xml.open0("cx:subtotals");
            for &index in &chart.subtotals {
                xml.val("cx:idx", index);
            }
            xml.close("cx:subtotals");
            xml.close("cx:layoutPr");
        }
        ChartType::Histogram | ChartType::Pareto => {
            xml.open0("cx:layoutPr");
            xml.open("cx:binning", A::new().a("intervalClosed", "r"));
            if let Some(width) = chart
                .bin_size
                .filter(|size| size.is_finite() && *size > 0.0)
            {
                // PowerPoint 16 reads these values from the val attribute.
                xml.val("cx:binSize", format_number(width));
            } else if let Some(count) = chart.bin_count.filter(|count| *count > 0) {
                xml.val("cx:binCount", count);
            }
            xml.close("cx:binning");
            xml.close("cx:layoutPr");
        }
        ChartType::BoxWhisker => {
            xml.open0("cx:layoutPr");
            xml.empty(
                "cx:visibility",
                A::new()
                    .a("meanLine", 0)
                    .a("meanMarker", 1)
                    .a("nonoutliers", 0)
                    .a("outliers", 1),
            );
            let method = if chart.inclusive_quartiles {
                "inclusive"
            } else {
                "exclusive"
            };
            xml.empty("cx:statistics", A::new().a("quartileMethod", method));
            xml.close("cx:layoutPr");
        }
        ChartType::RegionMap => {
            if let Some(layout) = old.and_then(|series| series.child("layoutPr"))
                && !crate::read::has_rel_refs(layout)
            {
                xml.raw(&crate::read::keep_xml(layout, ns.unwrap_or_default()));
            }
        }
        _ => {}
    }
}

fn write_axes(xml: &mut W, chart: &Chart, layout: Layout) {
    if layout.axes == 0 {
        return;
    }
    xml.open("cx:axis", A::new().a("id", 0));
    let gap = if matches!(chart.kind, ChartType::Histogram | ChartType::Pareto) {
        "0"
    } else {
        "0.5"
    };
    xml.empty("cx:catScaling", A::new().a("gapWidth", gap));
    xml.empty0("cx:tickLabels");
    xml.close("cx:axis");
    if layout.axes >= 2 {
        xml.open("cx:axis", A::new().a("id", 1));
        xml.empty0("cx:valScaling");
        if chart.gridlines {
            xml.empty0("cx:majorGridlines");
        }
        xml.empty0("cx:tickLabels");
        xml.close("cx:axis");
    }
    if layout.axes == 3 {
        xml.open("cx:axis", A::new().a("id", 2));
        xml.empty("cx:valScaling", A::new().a("max", 1).a("min", 0));
        xml.empty0("cx:tickLabels");
        xml.close("cx:axis");
    }
}
