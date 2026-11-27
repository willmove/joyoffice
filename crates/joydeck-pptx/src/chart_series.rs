//! Classic DrawingML plot groups and series, with file order and per-axis bindings.

use joydeck_model::chart::{Chart, ChartType, ErrorBars, Series, Trendline, TrendlineType};

use crate::chart_axes::{AxisIndex, AxisPair};
use crate::chart_cache::{format_number, num_lit, numbers, strings, write_numbers, write_strings};
use crate::chart_workbook::{ChartWorkbook, name_cell, range, size_cache, x_cache};
use crate::read::chart::{simple_fill, simple_line};
use crate::write::chart::{sp_pr, tx_pr};
use crate::xml::{A, El, W};

const MAX_SERIES: usize = 1000;
const MAX_TRENDS: usize = 16;

pub(super) fn read_plot(
    chart: &mut Chart,
    plot: &El,
    axes: &AxisIndex<'_>,
    mut points: usize,
) -> Option<()> {
    let mut first_kind = None;
    let mut different_kinds = false;
    let mut series = Vec::new();
    for group in plot.elements() {
        let Some(kind) = kind_from(group) else {
            continue;
        };
        different_kinds |= *first_kind.get_or_insert(kind) != kind;
        let (group_labels, group_percent_labels) = group_settings(chart, group, kind);
        let secondary = axes.secondary(group);
        for node in group
            .children_named("ser")
            .take(MAX_SERIES.saturating_sub(series.len()))
        {
            let order = node.child("order").and_then(|item| item.u32("val"));
            let mut item = read_series(
                node,
                group,
                kind,
                group_labels,
                group_percent_labels,
                &mut chart.categories,
                &mut points,
            );
            item.kind = Some(kind);
            item.secondary_axis = secondary;
            series.push((order, item));
        }
    }
    if series.iter().all(|(order, _)| order.is_some()) {
        series.sort_by_key(|(order, _)| *order);
    }
    let combo = different_kinds || series.iter().any(|(_, item)| item.secondary_axis);
    chart.kind = if combo { ChartType::Combo } else { first_kind? };
    chart.series = series
        .into_iter()
        .map(|(_, mut item)| {
            if !combo {
                item.kind = None;
                item.secondary_axis = false;
            }
            item
        })
        .collect();
    Some(())
}

pub(super) fn kind_from(group: &El) -> Option<ChartType> {
    use ChartType::*;
    let grouping = group.child_val("grouping");
    let stacked = matches!(grouping, Some("stacked" | "percentStacked"));
    Some(match group.local() {
        "barChart" | "bar3DChart" => match (group.child_val("barDir") == Some("bar"), grouping) {
            (false, Some("stacked")) => StackedColumn,
            (false, Some("percentStacked")) => PercentColumn,
            (false, _) => Column,
            (true, Some("stacked")) => StackedBar,
            (true, Some("percentStacked")) => PercentBar,
            (true, _) => Bar,
        },
        "lineChart" | "line3DChart" if stacked => StackedLine,
        "lineChart" | "line3DChart" => {
            let markers = matches!(group.child_val("marker"), Some("1" | "true"))
                && group.children_named("ser").all(|node| {
                    node.path(&["marker", "symbol"])
                        .and_then(|symbol| symbol.attr("val"))
                        != Some("none")
                });
            if markers { LineMarkers } else { Line }
        }
        "areaChart" | "area3DChart" if stacked => StackedArea,
        "areaChart" | "area3DChart" => Area,
        "pieChart" | "pie3DChart" | "ofPieChart" => Pie,
        "doughnutChart" => Doughnut,
        "scatterChart" => Scatter,
        "bubbleChart" => Bubble,
        "radarChart" if group.child_val("radarStyle") == Some("filled") => FilledRadar,
        "radarChart" => Radar,
        "stockChart" => Stock,
        "surfaceChart" | "surface3DChart" => Surface,
        _ => return None,
    })
}

fn number(node: &El, name: &str) -> Option<f64> {
    node.child(name)?.f64("val")
}

fn labels(node: &El) -> Option<bool> {
    node.path(&["dLbls", "showVal"])
        .map(|value| value.bool("val").unwrap_or(true))
}

fn percent_labels(node: &El) -> Option<bool> {
    node.path(&["dLbls", "showPercent"])
        .map(|value| value.bool("val").unwrap_or(true))
}

fn group_settings(chart: &mut Chart, group: &El, kind: ChartType) -> (Option<bool>, Option<bool>) {
    chart.vary_colors = group
        .child_val("varyColors")
        .is_none_or(|value| matches!(value, "1" | "true"));
    if kind == ChartType::Stock {
        chart.up_down_bars = group.child("upDownBars").is_some();
    }
    let bar = kind.is_bar_like();
    if let Some(gap) = number(group, "gapWidth")
        .map(|value| value / 100.0)
        .or(bar.then_some(1.5))
    {
        chart.gap_width = gap.clamp(0.0, 5.0);
    }
    if let Some(overlap) = number(group, "overlap")
        .map(|value| value / 100.0)
        .or(bar.then_some(0.0))
    {
        chart.overlap = overlap.clamp(-1.0, 1.0);
    }
    if let Some(hole) = number(group, "holeSize") {
        chart.hole_size = (hole / 100.0).clamp(0.0, 0.9);
    }
    let value = labels(group);
    chart.data_labels |= value == Some(true);
    let pct = percent_labels(group);
    chart.data_label_percent |= pct == Some(true);
    (value, pct)
}

fn read_series(
    node: &El,
    group: &El,
    kind: ChartType,
    group_labels: Option<bool>,
    group_percent_labels: Option<bool>,
    chart_categories: &mut Vec<String>,
    points: &mut usize,
) -> Series {
    let name = node
        .child("tx")
        .and_then(|title| strings(title, points).into_iter().next())
        .unwrap_or_default();
    let category = node.child("cat").or_else(|| node.child("xVal"));
    let categories = category
        .map(|item| strings(item, points))
        .unwrap_or_default();
    let x_values = if xy(kind) {
        categories
            .iter()
            .map(|text| text.trim().parse::<f64>().ok().filter(|x| x.is_finite()))
            .collect()
    } else {
        Vec::new()
    };
    if chart_categories.is_empty() {
        *chart_categories = categories;
    }
    let data = node.child("val").or_else(|| node.child("yVal"));
    let values = data.map(|item| numbers(item, points)).unwrap_or_default();
    let error_bars = read_error_bars(node, kind, points);
    let sizes = node
        .child("bubbleSize")
        .map(|item| numbers(item, points))
        .unwrap_or_default();
    let source_format = || {
        let cache = data?.find("numCache").or_else(|| data?.find("numLit"))?;
        Some(cache.child("formatCode")?.text())
    };
    let number_format = [node, group]
        .into_iter()
        .find_map(|owner| {
            let format = owner.path(&["dLbls", "numFmt"])?;
            (format.bool("sourceLinked") != Some(true))
                .then(|| format.attr("formatCode"))
                .flatten()
                .map(str::to_owned)
        })
        .or_else(source_format)
        .filter(|format| !format.is_empty());
    Series {
        name,
        values,
        x_values,
        sizes,
        number_format,
        error_bars,
        fill: simple_fill(node.child("spPr")),
        line: simple_line(node.child("spPr")),
        point_fills: node
            .children_named("dPt")
            .filter_map(|point| {
                let index = point.child("idx")?.u32("val")? as usize;
                Some((index, simple_fill(point.child("spPr"))?))
            })
            .collect(),
        data_labels: labels(node).or(group_labels.filter(|enabled| !enabled)),
        data_label_percent: percent_labels(node)
            .or(group_percent_labels.filter(|enabled| !enabled)),
        smooth: node
            .child("smooth")
            .and_then(|value| value.bool("val"))
            .unwrap_or_default(),
        marker: node
            .path(&["marker", "symbol"])
            .and_then(|symbol| symbol.attr("val"))
            .filter(|symbol| *symbol != "none")
            .map(str::to_owned),
        trendlines: node
            .children_named("trendline")
            .take(MAX_TRENDS)
            .filter_map(read_trend)
            .collect(),
        ..Series::default()
    }
}

fn read_error_bars(node: &El, kind: ChartType, points: &mut usize) -> Option<ErrorBars> {
    let bar = node.children_named("errBars").find(|element| {
        !xy(kind) || element.child_val("errDir").is_none_or(|value| value == "y")
    })?;
    Some(ErrorBars {
        bar_type: bar.child_val("errBarType").unwrap_or("both").into(),
        val_type: bar.child_val("errValType").unwrap_or("fixedVal").into(),
        value: number(bar, "val").unwrap_or(1.0),
        plus: bar
            .child("plus")
            .map(|node| numbers(node, points))
            .unwrap_or_default(),
        minus: bar
            .child("minus")
            .map(|node| numbers(node, points))
            .unwrap_or_default(),
        end_cap: bar.child("noEndCap").and_then(|cap| cap.bool("val")) != Some(true),
    })
}

fn read_trend(node: &El) -> Option<Trendline> {
    let kind = TrendlineType::from_xml(node.child_val("trendlineType")?)?;
    let opts = |tag, limit| {
        node.child(tag)
            .and_then(|value| value.u32("val"))
            .map(|num| num.clamp(2, limit))
    };
    Some(Trendline {
        kind,
        name: node.child("name").and_then(|name| clean_name(&name.text())),
        order: if kind == TrendlineType::Poly {
            opts("order", 6)
        } else {
            None
        },
        period: if kind == TrendlineType::MovingAvg {
            opts("period", 255)
        } else {
            None
        },
        forward: trend_number(number(node, "forward")),
        backward: trend_number(number(node, "backward")),
        intercept: trend_number(number(node, "intercept")),
        disp_eq: node
            .child("dispEq")
            .and_then(|value| value.bool("val"))
            .unwrap_or_default(),
        disp_r_sqr: node
            .child("dispRSqr")
            .and_then(|value| value.bool("val"))
            .unwrap_or_default(),
    })
}

fn clean_name(value: &str) -> Option<String> {
    let name: String = value
        .chars()
        .filter(|ch| {
            matches!(ch, '\t' | '\n' | '\r') || *ch >= ' ' && !matches!(ch, '\u{fffe}' | '\u{ffff}')
        })
        .take(255)
        .collect();
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

fn trend_number(value: Option<f64>) -> Option<f64> {
    value.filter(|number| number.is_finite() && number.abs() <= 1.0e12)
}

fn xy(kind: ChartType) -> bool {
    matches!(kind, ChartType::Scatter | ChartType::Bubble)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Bar,
    Line,
    Area,
    Pie,
    Doughnut,
    Scatter,
    Radar,
    Bubble,
    Stock,
}

impl Family {
    fn from_kind(kind: ChartType) -> Self {
        use ChartType::*;
        match kind {
            Line | LineMarkers | StackedLine => Self::Line,
            Area | StackedArea | Surface => Self::Area,
            Pie => Self::Pie,
            Doughnut | Sunburst => Self::Doughnut,
            Scatter => Self::Scatter,
            Radar | FilledRadar => Self::Radar,
            Bubble => Self::Bubble,
            Stock => Self::Stock,
            _ => Self::Bar,
        }
    }

    fn element(self) -> &'static str {
        match self {
            Self::Bar => "c:barChart",
            Self::Line => "c:lineChart",
            Self::Area => "c:areaChart",
            Self::Pie => "c:pieChart",
            Self::Doughnut => "c:doughnutChart",
            Self::Scatter => "c:scatterChart",
            Self::Radar => "c:radarChart",
            Self::Bubble => "c:bubbleChart",
            Self::Stock => "c:stockChart",
        }
    }

    fn has_axes(self) -> bool {
        !matches!(self, Self::Pie | Self::Doughnut)
    }
}

struct Plot<'a> {
    kind: ChartType,
    secondary: bool,
    series: Vec<(usize, &'a Series)>,
}

pub(super) fn write_plot(xml: &mut W, chart: &Chart, sheet: Option<&ChartWorkbook<'_>>) {
    let default = if chart.kind == ChartType::Combo {
        chart
            .series
            .first()
            .and_then(|series| series.kind)
            .unwrap_or(ChartType::Column)
    } else {
        chart.kind
    };
    let mut plots = Vec::<Plot>::new();
    for (index, item) in chart.series.iter().enumerate() {
        let (kind, secondary) = if chart.kind == ChartType::Combo {
            let kind = item.kind.unwrap_or(default);
            let kind = if matches!(
                Family::from_kind(kind),
                Family::Pie | Family::Doughnut | Family::Scatter | Family::Bubble
            ) {
                ChartType::Column
            } else {
                kind
            };
            (kind, item.secondary_axis)
        } else {
            (default, false)
        };
        if let Some(group) = plots
            .iter_mut()
            .find(|group| group.kind == kind && group.secondary == secondary)
        {
            group.series.push((index, item));
        } else {
            plots.push(Plot {
                kind,
                secondary,
                series: vec![(index, item)],
            });
        }
    }
    if plots.is_empty() {
        plots.push(Plot {
            kind: default,
            secondary: false,
            series: Vec::new(),
        });
    }
    // A lone axis group is the primary coordinate system even if old native data marks it secondary.
    let has_primary = plots.iter().any(|plot| !plot.secondary);
    for plot in &plots {
        write_group(
            xml,
            chart,
            plot,
            sheet,
            AxisPair::for_group(has_primary && plot.secondary),
        );
    }
    for secondary in [false, true] {
        if let Some(plot) = plots.iter().find(|plot| {
            (has_primary && plot.secondary) == secondary && Family::from_kind(plot.kind).has_axes()
        }) {
            AxisPair::for_group(secondary).write(xml, chart, plot.kind);
        }
    }
}

fn write_group(
    xml: &mut W,
    chart: &Chart,
    plot: &Plot<'_>,
    sheet: Option<&ChartWorkbook<'_>>,
    axes: AxisPair,
) {
    let family = Family::from_kind(plot.kind);
    xml.open0(family.element());
    if family == Family::Bar {
        xml.val(
            "c:barDir",
            if plot.kind.horizontal() { "bar" } else { "col" },
        );
    }
    if matches!(family, Family::Bar | Family::Line | Family::Area) {
        let grouping = if plot.kind.percent() {
            "percentStacked"
        } else if plot.kind.stacked() {
            "stacked"
        } else if family == Family::Bar {
            "clustered"
        } else {
            "standard"
        };
        xml.val("c:grouping", grouping);
    }
    if family == Family::Scatter {
        xml.val("c:scatterStyle", "lineMarker");
    }
    if family == Family::Radar {
        xml.val(
            "c:radarStyle",
            if plot.kind == ChartType::FilledRadar {
                "filled"
            } else {
                "marker"
            },
        );
    }
    if family != Family::Stock {
        xml.val("c:varyColors", u8::from(chart.vary_colors));
    }
    for &(index, item) in &plot.series {
        write_series(xml, chart, item, index, plot.kind, sheet);
    }
    if chart.data_labels || chart.data_label_percent {
        write_labels(
            xml,
            chart,
            chart.data_labels,
            chart.data_label_percent,
            None,
        );
    }
    match family {
        Family::Bar => {
            xml.val("c:gapWidth", percent(chart.gap_width, 0.0, 5.0));
            let overlap = if plot.kind.stacked() {
                100
            } else {
                percent(chart.overlap, -1.0, 1.0)
            };
            if overlap != 0 {
                xml.val("c:overlap", overlap);
            }
        }
        Family::Line => xml.val("c:marker", 1),
        Family::Pie | Family::Doughnut => {
            xml.val("c:firstSliceAng", 0);
            if family == Family::Doughnut {
                xml.val("c:holeSize", percent(chart.hole_size, 0.1, 0.9));
            }
        }
        Family::Stock => write_stock(xml, chart.up_down_bars),
        _ => {}
    }
    if family.has_axes() {
        xml.val("c:axId", axes.category);
        xml.val("c:axId", axes.value);
    }
    xml.close(family.element());
}

fn percent(number: f64, min: f64, max: f64) -> i64 {
    (number.clamp(min, max) * 100.0).round() as i64
}

fn write_series(
    xml: &mut W,
    chart: &Chart,
    item: &Series,
    index: usize,
    kind: ChartType,
    sheet: Option<&ChartWorkbook<'_>>,
) {
    let family = Family::from_kind(kind);
    let columns = sheet.and_then(|workbook| workbook.columns(index));
    xml.open0("c:ser");
    xml.val("c:idx", index);
    xml.val("c:order", index);
    xml.open0("c:tx");
    if let Some(columns) = columns {
        write_strings(
            xml,
            Some(&name_cell(columns.values)),
            std::slice::from_ref(&item.name),
        );
    } else {
        xml.elt("c:v", &item.name);
    }
    xml.close("c:tx");
    if family == Family::Stock && item.line.is_none() {
        xml.raw("<c:spPr><a:ln w=\"19050\"><a:noFill/></a:ln></c:spPr>");
    } else {
        sp_pr(xml, item.fill.as_ref(), item.line.as_ref());
    }
    if family == Family::Bar {
        xml.val("c:invertIfNegative", 0);
    }
    write_marker(xml, chart, item, family, kind, index);
    if matches!(family, Family::Bar | Family::Pie | Family::Doughnut) {
        for (point, fill) in item.point_fills.iter().take(MAX_SERIES) {
            xml.open0("c:dPt");
            xml.val("c:idx", point);
            xml.val(
                if family == Family::Bar {
                    "c:invertIfNegative"
                } else {
                    "c:bubble3D"
                },
                0,
            );
            sp_pr(xml, Some(fill), None);
            xml.close("c:dPt");
        }
    }
    if item.data_labels.is_some() || item.data_label_percent.is_some() {
        write_labels(
            xml,
            chart,
            item.data_labels.unwrap_or(chart.data_labels),
            item.data_label_percent.unwrap_or(chart.data_label_percent),
            item.number_format.as_deref(),
        );
    }
    if matches!(
        family,
        Family::Bar | Family::Line | Family::Area | Family::Scatter
    ) {
        for trend in item.trendlines.iter().take(MAX_TRENDS) {
            write_trend(xml, trend);
        }
    }
    if let Some(bars) = &item.error_bars {
        write_error_bars(xml, bars, kind);
    }
    let count = chart.categories.len().max(item.values.len());
    let format = item
        .number_format
        .as_deref()
        .filter(|value| !value.is_empty());
    let value_ref = columns.map(|column| range(column.values, column.values, count));
    if xy(kind) {
        let x = x_cache(item);
        let x_ref = columns
            .and_then(|column| column.x)
            .map(|column| range(column, column, x.len()));
        numeric_source(xml, "c:xVal", x_ref.as_deref(), &x, None);
        numeric_source(xml, "c:yVal", value_ref.as_deref(), &item.values, format);
        if kind == ChartType::Bubble {
            let sizes = size_cache(item);
            let size_ref = columns
                .and_then(|column| column.size)
                .map(|column| range(column, column, sizes.len()));
            numeric_source(xml, "c:bubbleSize", size_ref.as_deref(), &sizes, None);
            xml.val("c:bubble3D", 0);
        }
    } else {
        xml.open0("c:cat");
        let cat_ref = sheet.map(|worksheet| worksheet.categories(count));
        write_strings(xml, cat_ref.as_deref(), &chart.categories);
        xml.close("c:cat");
        numeric_source(xml, "c:val", value_ref.as_deref(), &item.values, format);
    }
    if matches!(family, Family::Line | Family::Scatter) {
        xml.val("c:smooth", u8::from(item.smooth));
    }
    xml.close("c:ser");
}

fn numeric_source(
    xml: &mut W,
    tag: &str,
    reference: Option<&str>,
    values: &[Option<f64>],
    format: Option<&str>,
) {
    xml.open0(tag);
    write_numbers(xml, reference, values, format);
    xml.close(tag);
}

fn write_marker(
    xml: &mut W,
    chart: &Chart,
    series: &Series,
    family: Family,
    kind: ChartType,
    index: usize,
) {
    if !matches!(
        family,
        Family::Stock | Family::Line | Family::Scatter | Family::Radar
    ) {
        return;
    }
    let explicit = series
        .marker
        .as_deref()
        .filter(|value| !value.is_empty() && value.bytes().all(|ch| ch.is_ascii_alphanumeric()));
    let symbol = explicit.or(match family {
        Family::Stock if !chart.up_down_bars && index == 2 && chart.series.len() == 3 => {
            Some("dash")
        }
        _ if matches!(kind, ChartType::LineMarkers | ChartType::Scatter) => None,
        _ => Some("none"),
    });
    if let Some(symbol) = symbol {
        xml.open0("c:marker");
        xml.val("c:symbol", symbol);
        if family == Family::Stock && symbol != "none" {
            xml.val("c:size", 7);
        }
        xml.close("c:marker");
    }
}

fn write_labels(
    xml: &mut W,
    chart: &Chart,
    show_val: bool,
    show_percent: bool,
    format: Option<&str>,
) {
    xml.open0("c:dLbls");
    if let Some(format) = format.filter(|value| !value.is_empty() && *value != "General") {
        xml.empty(
            "c:numFmt",
            A::new().a("formatCode", format).a("sourceLinked", 0),
        );
    }
    tx_pr(xml, chart);
    for (name, on) in [
        ("c:showLegendKey", false),
        ("c:showVal", show_val),
        ("c:showCatName", false),
        ("c:showSerName", false),
        ("c:showPercent", show_percent),
        ("c:showBubbleSize", false),
    ] {
        xml.val(name, u8::from(on));
    }
    xml.close("c:dLbls");
}

fn write_error_bars(xml: &mut W, bars: &ErrorBars, kind: ChartType) {
    let bar_type = match bars.bar_type.as_str() {
        "plus" => "plus",
        "minus" => "minus",
        _ => "both",
    };
    let value_type = match bars.val_type.as_str() {
        known @ ("percentage" | "stdDev" | "stdErr" | "cust") => known,
        _ => "fixedVal",
    };
    xml.open0("c:errBars");
    if xy(kind) {
        xml.val("c:errDir", "y");
    }
    xml.val("c:errBarType", bar_type);
    xml.val("c:errValType", value_type);
    xml.val("c:noEndCap", u8::from(!bars.end_cap));
    if value_type == "cust" {
        for (name, values) in [("c:plus", &bars.plus), ("c:minus", &bars.minus)] {
            xml.open0(name);
            num_lit(xml, values);
            xml.close(name);
        }
    } else if bars.value.is_finite() {
        xml.val("c:val", format_number(bars.value));
    }
    xml.close("c:errBars");
}

fn write_trend(xml: &mut W, trend: &Trendline) {
    xml.open0("c:trendline");
    if let Some(name) = trend.name.as_deref().and_then(clean_name) {
        xml.elt("c:name", &name);
    }
    xml.val("c:trendlineType", trend.kind.xml());
    if trend.kind == TrendlineType::Poly
        && let Some(order) = trend.order
    {
        xml.val("c:order", order.clamp(2, 6));
    }
    if trend.kind == TrendlineType::MovingAvg
        && let Some(period) = trend.period
    {
        xml.val("c:period", period.clamp(2, 255));
    }
    for (name, value) in [
        ("c:forward", trend.forward),
        ("c:backward", trend.backward),
        ("c:intercept", trend.intercept),
    ] {
        if let Some(value) = trend_number(value) {
            xml.val(name, format_number(value));
        }
    }
    if trend.disp_r_sqr {
        xml.val("c:dispRSqr", 1);
    }
    if trend.disp_eq {
        xml.val("c:dispEq", 1);
    }
    xml.close("c:trendline");
}

fn write_stock(xml: &mut W, up_down: bool) {
    xml.raw("<c:hiLowLines><c:spPr><a:ln w=\"9525\"><a:solidFill><a:schemeClr val=\"tx1\"/></a:solidFill></a:ln></c:spPr></c:hiLowLines>");
    if up_down {
        xml.raw("<c:upDownBars><c:gapWidth val=\"150\"/><c:upBars/><c:downBars/></c:upDownBars>");
    }
}
