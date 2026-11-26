//! Chart parts (`c:chartSpace`) → [`Chart`]. The original XML is kept in `Chart.raw`.

use joydeck_model::chart::Chart;
use joydeck_model::style::Fill;

use crate::chart_axes::AxisIndex;
use crate::chart_cache::MAX_CHART_POINTS;
use crate::xml::El;

/// Fills of chart parts never reference images we keep; read colour and pattern fills only.
pub(crate) fn simple_fill(sp: Option<&El>) -> Option<Fill> {
    let sp = sp?;
    for e in sp.elements() {
        match e.local() {
            "noFill" => return Some(Fill::None),
            "solidFill" => return super::dml::color(e).map(|color| Fill::Solid { color }),
            "gradFill" => return Some(Fill::Gradient(super::dml::gradient(e))),
            "pattFill" => return Some(Fill::Pattern(super::dml::pattern(e))),
            _ => {}
        }
    }
    None
}

pub(crate) fn simple_line(sp: Option<&El>) -> Option<joydeck_model::Line> {
    let ln = sp?.child("ln")?;
    let mut l = joydeck_model::Line {
        width: super::dml::pt(ln, "w"),
        ..Default::default()
    };
    l.fill = simple_fill(Some(ln));
    Some(l)
}

pub(crate) fn rich_text(t: &El) -> Option<String> {
    let tx = t.child("tx")?;
    if let Some(r) = tx.child("rich") {
        let s = r
            .children_named("p")
            .map(|p| p.text())
            .collect::<Vec<_>>()
            .join("\n");
        return Some(s);
    }
    tx.find("v").map(|v| v.text())
}

/// Parse a chart part. Returns `None` for something that isn't a chart we can read.
pub fn read_chart_xml(bytes: &[u8]) -> Option<Chart> {
    let doc = crate::xml::parse(bytes).ok()?;
    let mut root = doc.root;
    super::resolve_mc(&mut root, 0);
    if !root.is("chartSpace") {
        return None;
    }
    let mut c = chart_from(&root)?;
    c.raw = Some(String::from_utf8_lossy(bytes).into_owned());
    Some(c)
}

pub(crate) fn chart_from(root: &El) -> Option<Chart> {
    chart_within(root, MAX_CHART_POINTS)
}

/// [`chart_from`] decoding at most `budget` data slots in all.
fn chart_within(root: &El, budget: usize) -> Option<Chart> {
    let ch = root.child("chart")?;
    let pa = ch.child("plotArea")?;
    let mut c = Chart {
        categories: vec![],
        series: vec![],
        title: None,
        legend: None,
        gridlines: false,
        ..Default::default()
    };
    c.style = root
        .child_val("style")
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    if let Some(t) = ch.child("title") {
        c.title = Some(rich_text(t).unwrap_or_else(|| "Chart Title".into()));
    }
    if ch.child("autoTitleDeleted").and_then(|a| a.bool("val")) == Some(true) {
        c.title = None;
    }
    c.legend = ch
        .child("legend")
        .map(|l| l.child_val("legendPos").unwrap_or("r").to_string());
    c.legend_overlay = ch
        .child("legend")
        .and_then(|l| l.child("overlay"))
        .and_then(|o| o.bool("val"))
        .unwrap_or(false);
    c.chart_fill = simple_fill(root.child("spPr"));
    c.plot_fill = simple_fill(pa.child("spPr"));

    let axes = AxisIndex::read(pa);
    crate::chart_series::read_plot(&mut c, pa, &axes, budget)?;
    // `autoTitleDeleted val="0"` without `c:title`: PowerPoint and LibreOffice draw the series name as the title of a single-series chart.
    if ch.child("title").is_none()
        && ch.child("autoTitleDeleted").and_then(|a| a.bool("val")) == Some(false)
        && c.series.len() == 1
    {
        c.title = c
            .series
            .first()
            .map(|s| s.name.clone())
            .filter(|n| !n.is_empty());
    }
    c.lang = root.child_val("lang").map(str::to_string);
    let (val_ax, cat_ax) = axes.apply(&mut c);
    read_text_styles(&mut c, root, ch, val_ax, cat_ax);
    Some(c)
}

/// Text sizes and colours as the file sets them.
fn read_text_styles(c: &mut Chart, root: &El, ch: &El, val_ax: Option<&El>, cat_ax: Option<&El>) {
    // Text sizes as the file sets them (PowerPoint draws chart text at these sizes, not scaled to
    // the chart): the axes' and legend's, else the whole chart's; the title's own.
    let chart_size = text_size(root.child("txPr"));
    c.text_size = [val_ax, cat_ax, ch.child("legend")]
        .into_iter()
        .flatten()
        .find_map(|e| text_size(e.child("txPr")))
        .or(chart_size);
    // Text colour likewise: the axes' and legend's, else the whole chart's.
    c.text_color = [val_ax, cat_ax, ch.child("legend"), Some(root)]
        .into_iter()
        .flatten()
        .find_map(|e| text_color(e.child("txPr")));
    c.title_size = ch.child("title").and_then(|t| {
        text_size(t.child("tx").and_then(|x| x.child("rich")))
            .or_else(|| text_size(t.child("txPr")))
    });
}

/// The colour of the first `a:defRPr` or `a:rPr` with a solid fill under `e`.
fn text_color(e: Option<&El>) -> Option<joydeck_model::ColorRef> {
    fn rec(e: &El, depth: usize) -> Option<joydeck_model::ColorRef> {
        if depth > 8 {
            return None;
        }
        for c in e.elements() {
            if (c.is("defRPr") || c.is("rPr"))
                && let Some(Fill::Solid { color }) = simple_fill(Some(c))
            {
                return Some(color);
            }
            if let Some(v) = rec(c, depth + 1) {
                return Some(v);
            }
        }
        None
    }
    rec(e?, 0)
}

/// The size in points of the first `a:defRPr` or `a:rPr` with one under `e` (`sz` in hundredths).
fn text_size(e: Option<&El>) -> Option<f64> {
    fn rec(e: &El, depth: usize) -> Option<f64> {
        if depth > 8 {
            return None;
        }
        for c in e.elements() {
            if (c.is("defRPr") || c.is("rPr"))
                && let Some(v) = c
                    .attr("sz")
                    .and_then(|v| v.parse::<f64>().ok())
                    .filter(|v| v.is_finite() && *v > 0.0)
            {
                return Some((v / 100.0).clamp(1.0, 400.0));
            }
            if let Some(v) = rec(c, depth + 1) {
                return Some(v);
            }
        }
        None
    }
    rec(e?, 0)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test assertions"
)]
mod tests {
    use super::*;
    use joydeck_model::chart::{ChartType, TrendlineType};

    #[test]
    fn sparse_caches_share_one_chart_wide_budget() {
        // Every series declares 100000 points but writes one: without a chart-wide budget each
        // would expand to the full count.
        let ser = |n: usize| {
            format!(
                r#"<c:ser><c:idx val="{n}"/><c:val><c:numRef><c:numCache><c:ptCount val="100000"/><c:pt idx="0"><c:v>{n}</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser>"#
            )
        };
        let sers: String = (0..5).map(ser).collect();
        let xml = format!(
            r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea><c:barChart><c:barDir val="col"/>{sers}</c:barChart></c:plotArea></c:chart></c:chartSpace>"#
        );
        let doc = crate::xml::parse(xml.as_bytes()).unwrap();
        let c = chart_within(&doc.root, 250_000).unwrap();
        let total: usize = c.series.iter().map(|s| s.values.len()).sum();
        assert!(total <= 250_000, "{total}");
        assert_eq!(c.series.len(), 5);
        assert_eq!(c.series[0].values.len(), 100_000);
        assert_eq!(c.series[0].values[0], Some(0.0));
        assert!(
            c.series[4].values.is_empty(),
            "the budget ran out before the last series"
        );
        // A small chart is untouched by the default budget.
        let c = chart_from(&doc.root).unwrap();
        assert!(c.series.iter().all(|s| s.values.len() == 100_000));
    }

    #[test]
    fn minor_gridlines_are_read_and_written_back() {
        let mut c = Chart::sample(ChartType::Line);
        c.minor_gridlines = true;
        let back = read_chart_xml(&crate::write::chart::chart_xml(&c).0).unwrap();
        assert!(back.gridlines && back.minor_gridlines);
        c.minor_gridlines = false;
        assert!(
            !read_chart_xml(&crate::write::chart::chart_xml(&c).0)
                .unwrap()
                .minor_gridlines
        );
    }

    #[test]
    fn series_pattern_fills_are_read_and_written_back() {
        // A pattern-filled series and an empty one with an outline, as PowerPoint writes them.
        let xml = br#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><c:chart><c:plotArea><c:barChart><c:barDir val="col"/><c:ser><c:idx val="0"/><c:spPr><a:pattFill prst="openDmnd"><a:fgClr><a:schemeClr val="accent4"/></a:fgClr><a:bgClr><a:schemeClr val="bg1"/></a:bgClr></a:pattFill></c:spPr><c:val><c:numLit><c:ptCount val="1"/><c:pt idx="0"><c:v>5</c:v></c:pt></c:numLit></c:val></c:ser><c:ser><c:idx val="1"/><c:spPr><a:noFill/><a:ln w="19050"><a:solidFill><a:schemeClr val="accent4"/></a:solidFill></a:ln></c:spPr><c:val><c:numLit><c:ptCount val="1"/><c:pt idx="0"><c:v>3</c:v></c:pt></c:numLit></c:val></c:ser></c:barChart></c:plotArea></c:chart></c:chartSpace>"#;
        let c = read_chart_xml(xml).unwrap();
        let accent4 = joydeck_model::ColorRef::scheme(joydeck_color::SchemeSlot::Accent4);
        let pattern = joydeck_model::style::PatternFill {
            preset: "openDmnd".into(),
            fg: accent4.clone(),
            bg: joydeck_model::ColorRef::scheme(joydeck_color::SchemeSlot::Bg1),
        };
        assert_eq!(c.series[0].fill, Some(Fill::Pattern(pattern.clone())));
        assert_eq!(c.series[1].fill, Some(Fill::None));
        assert_eq!(
            c.series[1].line.as_ref().and_then(|l| l.fill.clone()),
            Some(Fill::solid(accent4))
        );
        // Written out from the model (an edited chart), the pattern survives.
        let mut edited = c.clone();
        edited.raw = None;
        edited.title = Some("Edited".into());
        let back = read_chart_xml(&crate::write::chart::chart_xml(&edited).0).unwrap();
        assert_eq!(back.series[0].fill, Some(Fill::Pattern(pattern)));
        assert_eq!(back.series[1].fill, Some(Fill::None));
    }

    #[test]
    fn log_base_is_read() {
        let xml = br#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea><c:lineChart><c:axId val="1"/><c:axId val="2"/></c:lineChart><c:catAx><c:axId val="1"/></c:catAx><c:valAx><c:axId val="2"/><c:scaling><c:logBase val="10"/><c:orientation val="minMax"/></c:scaling></c:valAx></c:plotArea></c:chart></c:chartSpace>"#;
        assert_eq!(read_chart_xml(xml).unwrap().y_log_base, Some(10.0));
    }

    #[test]
    fn error_bars_are_read() {
        let xml = br#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea><c:barChart><c:barDir val="col"/><c:ser><c:idx val="0"/><c:errBars><c:errBarType val="plus"/><c:errValType val="percentage"/><c:noEndCap val="1"/><c:val val="20"/></c:errBars><c:val><c:numLit><c:ptCount val="1"/><c:pt idx="0"><c:v>5</c:v></c:pt></c:numLit></c:val></c:ser></c:barChart></c:plotArea></c:chart></c:chartSpace>"#;
        let c = read_chart_xml(xml).unwrap();
        let e = c.series[0].error_bars.clone().unwrap();
        assert_eq!(
            (e.bar_type.as_str(), e.val_type.as_str(), e.value, e.end_cap),
            ("plus", "percentage", 20.0, false)
        );
    }

    #[test]
    fn legend_position_and_overlay_are_read() {
        let legend = r#"<c:legend><c:legendPos val="r"/><c:overlay val="1"/></c:legend>"#;
        let xml = format!(
            r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea><c:barChart><c:barDir val="col"/></c:barChart></c:plotArea>{legend}</c:chart></c:chartSpace>"#
        );
        let c = read_chart_xml(xml.as_bytes()).unwrap();
        assert_eq!(c.legend.as_deref(), Some("r"));
        assert!(c.legend_overlay);
        let xml = br#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea><c:barChart><c:barDir val="col"/></c:barChart></c:plotArea><c:legend/></c:chart></c:chartSpace>"#;
        let c = read_chart_xml(xml).unwrap();
        assert_eq!(c.legend.as_deref(), Some("r"));
        assert!(!c.legend_overlay);
    }

    #[test]
    fn combo_chart_with_a_secondary_axis() {
        // Columns on axes 1/2, a line on axes 3/4 whose value axis is on the right.
        let xml = br##"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><c:lang val="ru-RU"/><c:chart><c:plotArea>
<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:ser><c:idx val="0"/><c:tx><c:v>Spend</c:v></c:tx><c:cat><c:strLit><c:ptCount val="2"/><c:pt idx="0"><c:v>W1</c:v></c:pt><c:pt idx="1"><c:v>W2</c:v></c:pt></c:strLit></c:cat><c:val><c:numLit><c:ptCount val="2"/><c:pt idx="0"><c:v>200000</c:v></c:pt><c:pt idx="1"><c:v>310000</c:v></c:pt></c:numLit></c:val></c:ser><c:axId val="1"/><c:axId val="2"/></c:barChart>
<c:lineChart><c:grouping val="standard"/><c:ser><c:idx val="1"/><c:tx><c:v>Clicks</c:v></c:tx><c:spPr><a:ln w="28575"><a:solidFill><a:srgbClr val="00B050"/></a:solidFill></a:ln></c:spPr><c:val><c:numLit><c:ptCount val="2"/><c:pt idx="0"><c:v>6000</c:v></c:pt><c:pt idx="1"><c:v>7400</c:v></c:pt></c:numLit></c:val></c:ser><c:marker val="1"/><c:axId val="3"/><c:axId val="4"/></c:lineChart>
<c:catAx><c:axId val="1"/><c:axPos val="b"/><c:crossAx val="2"/></c:catAx><c:valAx><c:axId val="2"/><c:axPos val="l"/><c:numFmt formatCode="#,##0" sourceLinked="1"/><c:crossAx val="1"/><c:majorUnit val="100000"/></c:valAx>
<c:valAx><c:axId val="4"/><c:scaling><c:max val="8000"/></c:scaling><c:axPos val="r"/><c:numFmt formatCode="0.0" sourceLinked="0"/><c:crossAx val="3"/></c:valAx><c:catAx><c:axId val="3"/><c:delete val="1"/><c:axPos val="b"/><c:crossAx val="4"/></c:catAx>
</c:plotArea></c:chart></c:chartSpace>"##;
        let c = read_chart_xml(xml).unwrap();
        assert_eq!(c.kind, ChartType::Combo);
        assert_eq!(c.series.len(), 2);
        assert!(!c.series[0].secondary_axis);
        assert!(c.series[1].secondary_axis);
        assert_eq!(c.series[1].kind, Some(ChartType::LineMarkers));
        assert!(
            c.series[1]
                .line
                .as_ref()
                .is_some_and(|l| l.width.is_some_and(|w| (w - 2.25).abs() < 0.01))
        );
        // Each value axis' number format, bounds and unit; the chart's language.
        assert_eq!(c.y_format.as_deref(), Some("#,##0"));
        assert_eq!(
            (c.y_major, c.y2_max, c.y2_format.as_deref()),
            (Some(100000.0), Some(8000.0), Some("0.0"))
        );
        assert_eq!(c.lang.as_deref(), Some("ru-RU"));
        // Group order doesn't decide: columns on the right-hand axis are the secondary series even
        // when their group comes first, and the line on the left is primary.
        let swapped = String::from_utf8_lossy(xml)
            .replace(
                "<c:axPos val=\"l\"/><c:numFmt formatCode=\"#,##0\"",
                "<c:axPos val=\"r\"/><c:crosses val=\"max\"/><c:numFmt formatCode=\"#,##0\"",
            )
            .replace(
                "<c:axPos val=\"r\"/><c:numFmt formatCode=\"0.0\"",
                "<c:axPos val=\"l\"/><c:numFmt formatCode=\"0.0\"",
            );
        let c2 = read_chart_xml(swapped.as_bytes()).unwrap();
        assert!(c2.series[0].secondary_axis && !c2.series[1].secondary_axis);
        assert_eq!(
            (c2.y_format.as_deref(), c2.y2_format.as_deref()),
            (Some("0.0"), Some("#,##0"))
        );
        // A chart of one type on one axis has no secondary series.
        let single = String::from_utf8_lossy(xml).replace(
            "<c:axId val=\"3\"/><c:axId val=\"4\"/></c:lineChart>",
            "<c:axId val=\"1\"/><c:axId val=\"2\"/></c:lineChart>",
        );
        let c = read_chart_xml(single.as_bytes()).unwrap();
        assert!(c.series.iter().all(|s| !s.secondary_axis));
    }

    #[test]
    fn linked_axis_formats_come_from_the_data() {
        // No c:numFmt on the value axis, or a source-linked one: the series' own format.
        let xml = |ax_fmt: &str| {
            format!(
                r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><c:chart><c:plotArea><c:barChart><c:barDir val="col"/><c:ser><c:idx val="0"/><c:val><c:numRef><c:numCache><c:formatCode>#,##0</c:formatCode><c:ptCount val="1"/><c:pt idx="0"><c:v>1200</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser><c:axId val="1"/><c:axId val="2"/></c:barChart><c:catAx><c:axId val="1"/></c:catAx><c:valAx><c:axId val="2"/>{ax_fmt}<c:axPos val="l"/></c:valAx></c:plotArea></c:chart></c:chartSpace>"#
            )
        };
        let fmt = |x: &str| read_chart_xml(xml(x).as_bytes()).unwrap().y_format;
        assert_eq!(fmt("").as_deref(), Some("#,##0"));
        assert_eq!(
            fmt(r#"<c:numFmt formatCode="General" sourceLinked="1"/>"#).as_deref(),
            Some("#,##0")
        );
        assert_eq!(
            fmt(r#"<c:numFmt formatCode="0.0" sourceLinked="0"/>"#).as_deref(),
            Some("0.0")
        );
    }

    #[test]
    fn text_colour_is_read_from_the_axes() {
        // LibreOffice's templates set light axis text for dark slides on the axes' c:txPr.
        let xml = r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><c:chart><c:plotArea><c:barChart><c:barDir val="col"/><c:ser><c:idx val="0"/><c:val><c:numRef><c:numCache><c:ptCount val="1"/><c:pt idx="0"><c:v>1</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser><c:axId val="1"/><c:axId val="2"/></c:barChart><c:catAx><c:axId val="1"/><c:txPr><a:bodyPr/><a:p><a:pPr><a:defRPr sz="1000"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:defRPr></a:pPr></a:p></c:txPr></c:catAx><c:valAx><c:axId val="2"/></c:valAx></c:plotArea></c:chart></c:chartSpace>"#;
        let c = read_chart_xml(xml.as_bytes()).unwrap();
        assert_eq!(
            c.text_color,
            Some(joydeck_model::ColorRef::rgb(joydeck_color::Rgba::rgb(
                255, 255, 255
            )))
        );
        assert!(!c.cat_reversed);
        let rev = read_chart_xml(
            xml.replace("<c:catAx><c:axId val=\"1\"/>", "<c:catAx><c:axId val=\"1\"/><c:scaling><c:orientation val=\"maxMin\"/></c:scaling>")
                .as_bytes(),
        )
        .unwrap();
        assert!(rev.cat_reversed, "maxMin reverses the category axis");
        // No c:gapWidth / c:overlap: the schema's 150 % and 0 %.
        assert_eq!((c.gap_width, c.overlap), (1.5, 0.0));
    }

    #[test]
    fn text_sizes_are_read_from_the_file() {
        // PowerPoint's new charts set the axes' and legend's text to 11.97 pt and the title's to
        // 18.62 pt; a chart that only sets its overall text size uses it everywhere.
        let ns = r#"xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#;
        let tx = |sz: u32| {
            format!(
                r#"<c:txPr><a:bodyPr/><a:p><a:pPr><a:defRPr sz="{sz}"/></a:pPr></a:p></c:txPr>"#
            )
        };
        let xml = format!(
            r#"<c:chartSpace {ns}><c:chart><c:title><c:tx><c:rich><a:bodyPr/><a:p><a:pPr><a:defRPr sz="1862"/></a:pPr><a:r><a:t>T</a:t></a:r></a:p></c:rich></c:tx></c:title><c:plotArea><c:barChart><c:barDir val="col"/><c:axId val="1"/><c:axId val="2"/></c:barChart><c:catAx><c:axId val="1"/>{}</c:catAx><c:valAx><c:axId val="2"/>{}</c:valAx></c:plotArea></c:chart></c:chartSpace>"#,
            tx(1197),
            tx(1197)
        );
        let c = read_chart_xml(xml.as_bytes()).unwrap();
        assert_eq!((c.text_size, c.title_size), (Some(11.97), Some(18.62)));
        let xml = format!(
            r#"<c:chartSpace {ns}><c:chart><c:plotArea><c:barChart><c:barDir val="col"/></c:barChart></c:plotArea></c:chart>{}</c:chartSpace>"#,
            tx(1400)
        );
        let c = read_chart_xml(xml.as_bytes()).unwrap();
        assert_eq!((c.text_size, c.title_size), (Some(14.0), None));
        let xml = format!(
            r#"<c:chartSpace {ns}><c:chart><c:plotArea><c:barChart><c:barDir val="col"/></c:barChart></c:plotArea></c:chart></c:chartSpace>"#
        );
        assert_eq!(
            read_chart_xml(xml.as_bytes()).unwrap().text_size,
            None,
            "no sizes: the renderer picks one"
        );
    }

    #[test]
    fn hidden_labels_and_unfilled_gridlines_are_read() {
        let xml = |ax: &str| {
            format!(
                r#"<c:chartSpace xmlns:c="{NS_C}" xmlns:a="{NS_A}"><c:chart><c:plotArea><c:barChart><c:barDir val="col"/><c:axId val="1"/><c:axId val="2"/></c:barChart><c:catAx><c:axId val="1"/></c:catAx><c:valAx><c:axId val="2"/>{ax}</c:valAx></c:plotArea></c:chart></c:chartSpace>"#
            )
        };
        let no_fill = r#"<c:spPr><a:ln w="9525"><a:noFill/></a:ln></c:spPr>"#;
        let c = read_chart_xml(
            xml(r#"<c:majorGridlines/><c:minorGridlines/><c:tickLblPos val="nextTo"/>"#).as_bytes(),
        )
        .unwrap();
        assert!(c.gridlines && c.minor_gridlines && !c.y_hidden);
        let c = read_chart_xml(
            xml(&format!(
                r#"<c:majorGridlines>{no_fill}</c:majorGridlines><c:minorGridlines>{no_fill}</c:minorGridlines><c:tickLblPos val="none"/>"#
            ))
            .as_bytes(),
        )
        .unwrap();
        assert!(!c.gridlines && !c.minor_gridlines && c.y_hidden);
    }

    #[test]
    fn trendlines_are_read() {
        let xml = br#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea><c:lineChart><c:grouping val="standard"/><c:ser><c:idx val="0"/><c:tx><c:v>Spend</c:v></c:tx>
<c:trendline><c:name>Fit &amp; forecast</c:name><c:trendlineType val="linear"/><c:forward val="1"/><c:backward val="0.5"/><c:intercept val="0"/><c:dispRSqr val="1"/><c:dispEq val="1"/></c:trendline>
<c:trendline><c:trendlineType val="poly"/><c:order val="9"/></c:trendline>
<c:trendline><c:trendlineType val="movingAvg"/><c:period val="3"/></c:trendline>
<c:trendline><c:trendlineType val="notAType"/></c:trendline>
<c:val><c:numLit><c:ptCount val="1"/><c:pt idx="0"><c:v>1</c:v></c:pt></c:numLit></c:val></c:ser></c:lineChart></c:plotArea></c:chart></c:chartSpace>"#;
        let c = read_chart_xml(xml).unwrap();
        let t = &c.series[0].trendlines;
        assert_eq!(t.len(), 3);
        assert_eq!(t[0].name.as_deref(), Some("Fit & forecast"));
        assert_eq!(t[0].kind, TrendlineType::Linear);
        assert_eq!(
            (t[0].forward, t[0].backward, t[0].intercept),
            (Some(1.0), Some(0.5), Some(0.0))
        );
        assert!(t[0].disp_eq && t[0].disp_r_sqr);
        assert_eq!((t[1].kind, t[1].order), (TrendlineType::Poly, Some(6)));
        assert_eq!(
            (t[2].kind, t[2].period),
            (TrendlineType::MovingAvg, Some(3))
        );
    }

    const NS_C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
    const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

    fn chart(auto_deleted: &str, title: &str, series: usize) -> Chart {
        let val = r#"<c:val><c:numLit><c:ptCount val="1"/><c:pt idx="0"><c:v>1</c:v></c:pt></c:numLit></c:val>"#;
        let ser: String = (0..series)
            .map(|i| {
                format!(r#"<c:ser><c:idx val="{i}"/><c:tx><c:v>S{i}</c:v></c:tx>{val}</c:ser>"#)
            })
            .collect();
        let plot = format!(
            r#"<c:plotArea><c:barChart><c:barDir val="col"/>{ser}</c:barChart></c:plotArea>"#
        );
        let xml = format!(
            r#"<c:chartSpace xmlns:c="{NS_C}" xmlns:a="{NS_A}"><c:chart>{title}{auto_deleted}{plot}</c:chart></c:chartSpace>"#
        );
        read_chart_xml(xml.as_bytes()).unwrap()
    }

    #[test]
    fn single_series_gets_automatic_title() {
        assert_eq!(
            chart(r#"<c:autoTitleDeleted val="0"/>"#, "", 1)
                .title
                .as_deref(),
            Some("S0")
        );
    }

    #[test]
    fn no_automatic_title_when_deleted_or_multi_series() {
        assert_eq!(chart(r#"<c:autoTitleDeleted val="1"/>"#, "", 1).title, None);
        assert_eq!(chart("", "", 1).title, None);
        assert_eq!(chart(r#"<c:autoTitleDeleted val="0"/>"#, "", 2).title, None);
    }

    #[test]
    fn explicit_title_wins() {
        let t = "<c:title><c:tx><c:rich><a:p><a:r><a:t>Mine</a:t></a:r></a:p></c:rich></c:tx></c:title>";
        assert_eq!(
            chart(r#"<c:autoTitleDeleted val="0"/>"#, t, 1)
                .title
                .as_deref(),
            Some("Mine")
        );
    }

    const NS_CHART: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";

    fn vary_xml(vary: &str) -> String {
        let ser = r#"<c:ser><c:idx val="0"/><c:order val="0"/></c:ser>"#;
        let bar = format!(
            r#"<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/>{vary}{ser}</c:barChart>"#
        );
        format!(
            r#"<c:chartSpace xmlns:c="{NS_CHART}"><c:chart><c:plotArea>{bar}</c:plotArea></c:chart></c:chartSpace>"#
        )
    }

    #[test]
    fn missing_vary_colors_means_vary_by_point() {
        assert!(read_chart_xml(vary_xml("").as_bytes()).is_some_and(|c| c.vary_colors));
    }

    #[test]
    fn explicit_vary_colors_is_respected() {
        assert!(
            read_chart_xml(vary_xml(r#"<c:varyColors val="0"/>"#).as_bytes())
                .is_some_and(|c| !c.vary_colors)
        );
        assert!(
            read_chart_xml(vary_xml(r#"<c:varyColors val="1"/>"#).as_bytes())
                .is_some_and(|c| c.vary_colors)
        );
    }
}
