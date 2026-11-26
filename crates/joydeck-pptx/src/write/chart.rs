//! Classic DrawingML chart space and chart-wide formatting.

use joydeck_model::chart::Chart;
use joydeck_model::style::Fill;

use crate::chart_workbook::ChartWorkbook;
use crate::opc::{NS_A, NS_C, NS_R};
use crate::xml::{A, W};

fn simple_fill(w: &mut W, f: &Fill) {
    match f {
        Fill::None => w.empty0("a:noFill"),
        Fill::Solid { color } => super::dml::solid(w, color),
        Fill::Pattern(p) => {
            w.open(
                "a:pattFill",
                A::new().a(
                    "prst",
                    if p.preset.is_empty() {
                        "pct5"
                    } else {
                        &p.preset
                    },
                ),
            );
            w.open0("a:fgClr");
            super::dml::color(w, &p.fg);
            w.close("a:fgClr");
            w.open0("a:bgClr");
            super::dml::color(w, &p.bg);
            w.close("a:bgClr");
            w.close("a:pattFill");
        }
        Fill::Gradient(_) | Fill::Picture(_) | Fill::Group | Fill::Background => {
            if let Fill::Gradient(g) = f
                && let Some(s) = g.stops.first()
            {
                super::dml::solid(w, &s.color);
            }
        }
    }
}

pub(crate) fn sp_pr(w: &mut W, fill: Option<&Fill>, line: Option<&joydeck_model::Line>) {
    if fill.is_none() && line.is_none() {
        return;
    }
    w.open0("c:spPr");
    if let Some(f) = fill {
        simple_fill(w, f);
    }
    if let Some(l) = line {
        w.open("a:ln", A::new().o("w", l.width.map(super::dml::emu_pos)));
        if let Some(f) = &l.fill {
            simple_fill(w, f);
        }
        w.close("a:ln");
    }
    w.close("c:spPr");
}

/// `a:defRPr` carrying the chart's text colour, where PowerPoint keeps it: the paragraph
/// defaults of a title's rich text and of every `c:txPr`.
fn def_rpr(w: &mut W, color: Option<&joydeck_model::ColorRef>) {
    match color {
        Some(c) => {
            w.open0("a:defRPr");
            super::dml::solid(w, c);
            w.close("a:defRPr");
        }
        None => w.empty0("a:defRPr"),
    }
}

/// `c:txPr` with the chart's text colour (`Chart::text_color`) as the default run properties, on
/// the axes, the legend, the data labels and the chart itself, as PowerPoint writes it. Nothing
/// without a colour: the theme's default then applies on reading.
pub(crate) fn tx_pr(w: &mut W, c: &Chart) {
    let Some(color) = &c.text_color else { return };
    w.open0("c:txPr");
    w.empty0("a:bodyPr");
    w.empty0("a:lstStyle");
    w.open0("a:p");
    w.open0("a:pPr");
    def_rpr(w, Some(color));
    w.close("a:pPr");
    w.empty("a:endParaRPr", A::new().a("lang", "en-US"));
    w.close("a:p");
    w.close("c:txPr");
}

fn rich(w: &mut W, text: &str, color: Option<&joydeck_model::ColorRef>) {
    w.open0("c:tx");
    w.open0("c:rich");
    w.empty0("a:bodyPr");
    w.empty0("a:lstStyle");
    for line in text.split('\n') {
        w.open0("a:p");
        if color.is_some() {
            w.open0("a:pPr");
            def_rpr(w, color);
            w.close("a:pPr");
        }
        w.open0("a:r");
        if let Some(c) = color {
            w.open("a:rPr", A::new().a("lang", "en-US"));
            super::dml::solid(w, c);
            w.close("a:rPr");
        }
        w.elt("a:t", line);
        w.close("a:r");
        w.close("a:p");
    }
    w.close("c:rich");
    w.close("c:tx");
}

/// A title (the chart's or an axis's) in the chart's text colour.
pub(crate) fn title(w: &mut W, text: &str, c: &Chart) {
    w.open0("c:title");
    rich(w, text, c.text_color.as_ref());
    w.val("c:overlay", 0);
    w.close("c:title");
}

pub(crate) fn retained(chart: &Chart) -> Option<Vec<u8>> {
    let raw = chart.raw.as_ref()?;
    let mut doc = crate::xml::parse(raw.as_bytes()).ok()?;
    let mut resolved = doc.root.clone();
    crate::read::resolve_mc(&mut resolved, 0);
    let original = crate::read::chart_from_el(&resolved)?;
    let mut current = chart.clone();
    current.raw = None;
    current.kept = Default::default();
    if original != current {
        return None;
    }
    crate::chart_sidecars::drop_missing_refs(&mut doc.root, chart);
    let mut output = W::new();
    output.raw(&doc.root.to_xml());
    Some(output.finish())
}

pub(crate) fn build(c: &Chart, workbook: Option<(&ChartWorkbook<'_>, &str)>) -> Vec<u8> {
    let mut w = W::new();
    w.open(
        "c:chartSpace",
        A::new()
            .a("xmlns:c", NS_C)
            .a("xmlns:a", NS_A)
            .a("xmlns:r", NS_R),
    );
    w.val("c:date1904", 0);
    if let Some(language) = &c.lang {
        w.val("c:lang", language);
    }
    w.val("c:roundedCorners", 0);
    w.val("c:style", c.style.clamp(1, 48));
    w.open0("c:chart");
    match &c.title {
        Some(t) => {
            title(&mut w, t, c);
            w.val("c:autoTitleDeleted", 0);
        }
        None => w.val("c:autoTitleDeleted", 1),
    }
    w.open0("c:plotArea");
    w.empty0("c:layout");
    crate::chart_series::write_plot(&mut w, c, workbook.map(|(sheet, _)| sheet));
    if let Some(f) = &c.plot_fill {
        sp_pr(&mut w, Some(f), None);
    }
    w.close("c:plotArea");
    if let Some(pos) = &c.legend {
        let p = match pos.as_str() {
            "b" | "t" | "l" | "r" | "tr" => pos.as_str(),
            _ => "r",
        };
        w.open0("c:legend");
        w.val("c:legendPos", p);
        w.val("c:overlay", u8::from(c.legend_overlay));
        tx_pr(&mut w, c);
        w.close("c:legend");
    }
    w.val("c:plotVisOnly", 1);
    w.val("c:dispBlanksAs", "gap");
    w.close("c:chart");
    if let Some(f) = &c.chart_fill {
        sp_pr(&mut w, Some(f), None);
    }
    tx_pr(&mut w, c);

    if let Some((_, id)) = workbook {
        w.open("c:externalData", A::new().a("r:id", id));
        w.val("c:autoUpdate", 0);
        w.close("c:externalData");
    }
    w.close("c:chartSpace");
    w.finish()
}

#[cfg(test)]
pub(crate) fn chart_xml(c: &Chart) -> (Vec<u8>, bool) {
    match retained(c) {
        Some(bytes) => (bytes, true),
        None => (build(c, None), false),
    }
}

#[cfg(test)]
fn chartex_xml(chart: &Chart) -> Option<Vec<u8>> {
    crate::chartex_write::retained(chart)
        .or_else(|| crate::chartex_write::encode(chart, "ppt/charts/chartEx1.xml", None))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test assertions"
)]
mod region_map_tests {
    use super::*;
    use joydeck_model::chart::ChartType;

    /// A region map part as PowerPoint writes it (our own numbers; the cached geography is a
    /// stand-in blob).
    const MAP: &str = r#"<cx:chartSpace xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex"><cx:chartData><cx:data id="0"><cx:strDim type="cat"><cx:f>Sheet1!$A$2:$A$4</cx:f><cx:lvl ptCount="3"><cx:pt idx="0">Minsk</cx:pt><cx:pt idx="1">Brest</cx:pt><cx:pt idx="2">Gomel</cx:pt></cx:lvl></cx:strDim><cx:numDim type="colorVal"><cx:f>Sheet1!$B$2:$B$4</cx:f><cx:lvl ptCount="3" formatCode="General"><cx:pt idx="0">12</cx:pt><cx:pt idx="1">5</cx:pt><cx:pt idx="2">8</cx:pt></cx:lvl></cx:numDim></cx:data></cx:chartData><cx:chart><cx:title pos="t" align="ctr" overlay="0"/><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="regionMap" uniqueId="{00000000-0000-4000-8000-000000000001}"><cx:tx><cx:txData><cx:f>Sheet1!$B$1</cx:f><cx:v>Sales</cx:v></cx:txData></cx:tx><cx:dataId val="0"/><cx:layoutPr><cx:geography cultureLanguage="en-US" cultureRegion="BY" attribution="Powered by Bing"><cx:geoCache provider="{E9337A44-BEBE-4D9F-B70C-5C5E7DAFC167}"><cx:binary>ZGVja2NyYWZ0LXRlc3Q=</cx:binary></cx:geoCache></cx:geography></cx:layoutPr></cx:series></cx:plotAreaRegion></cx:plotArea><cx:legend pos="r" align="min" overlay="0"/></cx:chart></cx:chartSpace>"#;

    #[test]
    fn region_maps_keep_their_geography_through_save_and_edit() {
        let c = crate::read::read_chartex_xml(MAP.as_bytes()).expect("a region map is read");
        assert_eq!(c.kind, ChartType::RegionMap);
        assert_eq!(c.categories, ["Minsk", "Brest", "Gomel"]);
        assert_eq!(
            c.series.first().map(|s| s.values.clone()),
            Some(vec![Some(12.0), Some(5.0), Some(8.0)])
        );
        // Unchanged: written back as read.
        let kept = String::from_utf8(chartex_xml(&c).unwrap_or_default()).unwrap_or_default();
        assert!(
            kept.contains("ZGVja2NyYWZ0LXRlc3Q=") && kept.contains("cultureRegion=\"BY\""),
            "{kept}"
        );
        // Edited: rebuilt from the model, with the geography carried over.
        let mut e = c.clone();
        if let Some(s) = e.series.first_mut() {
            s.values[1] = Some(20.0);
        }
        let built = String::from_utf8(chartex_xml(&e).unwrap_or_default()).unwrap_or_default();
        assert!(
            built.contains("<cx:geoCache")
                && built.contains("ZGVja2NyYWZ0LXRlc3Q=")
                && built.contains("colorVal"),
            "{built}"
        );
        assert!(!built.contains("<cx:axis"), "{built}");
        let back = crate::read::read_chartex_xml(built.as_bytes()).expect("the rebuilt part reads");
        assert_eq!(back.kind, ChartType::RegionMap);
        assert_eq!(
            back.series
                .first()
                .and_then(|s| s.values.get(1).copied().flatten()),
            Some(20.0)
        );
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test assertions"
)]
mod tests {
    use joydeck_color::{ColorTransform, Rgba};
    use joydeck_model::ColorRef;
    use joydeck_model::chart::{Chart, ChartType};

    /// The chart's text colour is written where PowerPoint keeps it (`c:txPr` on the axes, legend,
    /// data labels and chart, the title's run defaults) and read back as the same colour.
    #[test]
    fn chart_text_colour_round_trips() {
        let mut c = Chart::sample(ChartType::Column);
        c.title = Some("Revenue".into());
        c.legend = Some("b".into());
        c.data_labels = true;
        c.axis_titles = (Some("Quarter".into()), Some("EUR".into()));
        let tinted = ColorRef::scheme(joydeck_model::SchemeSlot::Accent2)
            .with(ColorTransform::LumMod(75000));
        c.text_color = Some(tinted.clone());
        let xml = String::from_utf8(super::chart_xml(&c).0).unwrap();
        // Axes (2), legend, data labels and the chart root; a title keeps it in its rich text.
        assert!(xml.matches("<c:txPr>").count() >= 5, "{xml}");
        assert!(
            xml.contains(r#"<a:schemeClr val="accent2"><a:lumMod val="75000"/></a:schemeClr>"#),
            "{xml}"
        );
        assert!(
            xml.contains(
                "<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr><a:solidFill>"
            ),
            "{xml}"
        );
        assert_eq!(
            crate::read::chart::read_chart_xml(xml.as_bytes())
                .unwrap()
                .text_color,
            Some(tinted)
        );
        // An RGB colour and no colour at all round-trip too.
        c.text_color = Some(ColorRef::rgb(Rgba::rgb(255, 255, 255)));
        let xml = super::chart_xml(&c).0;
        assert_eq!(
            crate::read::chart::read_chart_xml(&xml).unwrap().text_color,
            c.text_color
        );
        c.text_color = None;
        let xml = String::from_utf8(super::chart_xml(&c).0).unwrap();
        assert!(!xml.contains("<c:txPr>"), "{xml}");
        assert_eq!(
            crate::read::chart::read_chart_xml(xml.as_bytes())
                .unwrap()
                .text_color,
            None
        );
    }

    /// An imported chart's `c:txPr` colour survives an edit that rebuilds the part from the model
    /// (the colour used to be read but never written, so a save after any edit lost it).
    #[test]
    fn imported_text_colour_survives_an_edit_and_a_save() {
        let xml = r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><c:chart><c:plotArea><c:barChart><c:barDir val="col"/><c:ser><c:idx val="0"/><c:val><c:numRef><c:numCache><c:ptCount val="1"/><c:pt idx="0"><c:v>1</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser><c:axId val="1"/><c:axId val="2"/></c:barChart><c:catAx><c:axId val="1"/><c:txPr><a:bodyPr/><a:p><a:pPr><a:defRPr><a:solidFill><a:schemeClr val="tx1"><a:lumMod val="65000"/><a:lumOff val="35000"/></a:schemeClr></a:solidFill></a:defRPr></a:pPr></a:p></c:txPr><c:crossAx val="2"/></c:catAx><c:valAx><c:axId val="2"/><c:crossAx val="1"/></c:valAx></c:plotArea></c:chart></c:chartSpace>"#;
        let c = crate::read::chart::read_chart_xml(xml.as_bytes()).unwrap();
        let grey = ColorRef::scheme(joydeck_model::SchemeSlot::Tx1)
            .with(ColorTransform::LumMod(65000))
            .with(ColorTransform::LumOff(35000));
        assert_eq!(c.text_color, Some(grey.clone()));
        // Unchanged: the part is written back as read.
        assert!(super::chart_xml(&c).1);
        // Edited (a title; the editor drops `raw` on any edit): rebuilt with the colour.
        let mut e = c.clone();
        e.title = Some("Units".into());
        e.raw = None;
        let (out, kept) = super::chart_xml(&e);
        assert!(!kept);
        let back = crate::read::chart::read_chart_xml(&out).unwrap();
        assert_eq!(back.text_color, Some(grey));
        assert_eq!(back.title.as_deref(), Some("Units"));
    }

    #[test]
    fn reversed_category_axis_round_trips() {
        let mut c = Chart::sample(ChartType::Bar);
        c.cat_reversed = true;
        let xml = super::chart_xml(&c).0;
        assert!(String::from_utf8_lossy(&xml).contains(r#"<c:orientation val="maxMin"/>"#));
        assert!(
            crate::read::chart::read_chart_xml(&xml)
                .unwrap()
                .cat_reversed
        );
        c.cat_reversed = false;
        assert!(
            !crate::read::chart::read_chart_xml(&super::chart_xml(&c).0)
                .unwrap()
                .cat_reversed
        );
    }
}
