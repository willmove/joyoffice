//! Classic DrawingML axis references, scales and coordinate-system emission.

use std::collections::HashMap;

use joydeck_model::chart::{Chart, ChartType};

use crate::chart_cache::format_number;
use crate::read::chart::rich_text;
use crate::write::chart::{title, tx_pr};
use crate::xml::{A, El, W};

pub(super) struct AxisIndex<'a> {
    plot: &'a El,
    values_by_id: HashMap<&'a str, &'a El>,
    primary: Option<&'a El>,
    category: Option<&'a El>,
    secondary: Option<&'a El>,
}

impl<'a> AxisIndex<'a> {
    pub(super) fn read(plot: &'a El) -> Self {
        let values_by_id = plot
            .children_named("valAx")
            .filter_map(|axis| Some((axis.child_val("axId")?, axis)))
            .collect();
        let mut index = Self {
            plot,
            values_by_id,
            primary: None,
            category: None,
            secondary: None,
        };
        let groups = || {
            plot.elements()
                .filter(|node| node.local().ends_with("Chart"))
        };
        let mut used = groups().filter_map(|group| index.value_axis(group));
        let first = used.next();
        index.primary = first
            .filter(|axis| !opposite(axis))
            .or_else(|| used.find(|axis| !opposite(axis)))
            .or(first)
            .or_else(|| plot.children_named("valAx").find(|axis| vertical(axis)))
            .or_else(|| plot.children_named("valAx").last());
        let primary_id = index.primary.and_then(axis_id);
        index.secondary = groups()
            .filter_map(|group| index.value_axis(group))
            .find(|axis| axis_id(axis) != primary_id);
        let crossing = index.primary.and_then(|axis| axis.child_val("crossAx"));
        index.category = crossing
            .and_then(|id| {
                plot.elements().find(|axis| {
                    matches!(axis.local(), "catAx" | "dateAx") && axis_id(axis) == Some(id)
                })
            })
            .or_else(|| plot.child("catAx"))
            .or_else(|| plot.child("dateAx"))
            .or_else(|| plot.children_named("valAx").find(|axis| !vertical(axis)));
        index
    }

    fn value_axis(&self, group: &El) -> Option<&'a El> {
        let mut axes = group
            .children_named("axId")
            .filter_map(|reference| reference.attr("val"))
            .filter_map(|id| self.values_by_id.get(id).copied());
        let first = axes.next()?;
        if vertical(first) {
            Some(first)
        } else {
            axes.find(|axis| vertical(axis)).or(Some(first))
        }
    }

    pub(super) fn secondary(&self, group: &El) -> bool {
        self.value_axis(group)
            .and_then(axis_id)
            .is_some_and(|id| self.primary.and_then(axis_id) != Some(id))
    }

    pub(super) fn apply(&self, chart: &mut Chart) -> (Option<&'a El>, Option<&'a El>) {
        if let Some(axis) = self.primary {
            let scale = Scale::read(axis, self.plot);
            chart.y_min = scale.min;
            chart.y_max = scale.max;
            chart.y_major = scale.major;
            chart.y_log_base = axis
                .child("scaling")
                .and_then(|node| finite(node, "logBase"))
                .filter(|base| *base >= 2.0);
            chart.y_format = scale.format;
            chart.y_hidden = scale.hidden;
            chart.axis_titles.1 = caption(axis);
            chart.gridlines = grid_visible(axis, "majorGridlines");
            chart.minor_gridlines = grid_visible(axis, "minorGridlines");
        }
        if chart.series.iter().any(|series| series.secondary_axis)
            && let Some(axis) = self.secondary
        {
            let scale = Scale::read(axis, self.plot);
            chart.y2_min = scale.min;
            chart.y2_max = scale.max;
            chart.y2_major = scale.major;
            chart.y2_format = scale.format;
            chart.y2_hidden = scale.hidden;
        }
        if let Some(axis) = self.category {
            chart.cat_reversed = axis
                .child("scaling")
                .and_then(|scale| scale.child_val("orientation"))
                == Some("maxMin");
            chart.axis_titles.0 = caption(axis);
        }
        (self.primary, self.category)
    }
}

fn axis_id(axis: &El) -> Option<&str> {
    axis.child_val("axId")
}

fn vertical(axis: &El) -> bool {
    matches!(axis.child_val("axPos"), Some("l" | "r"))
}

fn opposite(axis: &El) -> bool {
    matches!(axis.child_val("axPos"), Some("r" | "t")) || axis.child_val("crosses") == Some("max")
}

fn caption(axis: &El) -> Option<String> {
    axis.child("title")
        .map(|node| rich_text(node).unwrap_or_else(|| "Axis Title".into()))
}

fn grid_visible(axis: &El, kind: &str) -> bool {
    axis.child(kind)
        .is_some_and(|lines| lines.path(&["spPr", "ln", "noFill"]).is_none())
}

fn finite(node: &El, name: &str) -> Option<f64> {
    node.child(name)?.f64("val")
}

struct Scale {
    min: Option<f64>,
    max: Option<f64>,
    major: Option<f64>,
    format: Option<String>,
    hidden: bool,
}

impl Scale {
    fn read(axis: &El, plot: &El) -> Self {
        let limits = axis.child("scaling");
        let number_format = axis.child("numFmt");
        let format = number_format
            .and_then(|node| node.attr("formatCode"))
            .map(str::to_owned);
        let inherited = || {
            let id = axis_id(axis)?;
            plot.elements()
                .filter(|group| {
                    group
                        .children_named("axId")
                        .any(|ref_id| ref_id.attr("val") == Some(id))
                })
                .flat_map(|group| group.children_named("ser"))
                .find_map(|series| {
                    let values = series.child("val").or_else(|| series.child("yVal"))?;
                    Some(values.find("numCache")?.child("formatCode")?.text())
                })
                .filter(|code| !code.is_empty() && code != "General")
        };
        Self {
            min: limits.and_then(|node| finite(node, "min")),
            max: limits.and_then(|node| finite(node, "max")),
            major: finite(axis, "majorUnit"),
            format: if number_format.is_none_or(|node| node.bool("sourceLinked") == Some(true)) {
                inherited().or(format)
            } else {
                format
            },
            hidden: axis.child("delete").and_then(|node| node.bool("val")) == Some(true)
                || axis.child_val("tickLblPos") == Some("none"),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct AxisPair {
    pub(super) category: u32,
    pub(super) value: u32,
    secondary: bool,
}

impl AxisPair {
    pub(super) fn for_group(secondary: bool) -> Self {
        let category = if secondary { 50_003 } else { 50_001 };
        Self {
            category,
            value: category + 1,
            secondary,
        }
    }

    pub(super) fn write(self, xml: &mut W, chart: &Chart, kind: ChartType) {
        let horizontal = matches!(
            kind,
            ChartType::Bar | ChartType::StackedBar | ChartType::PercentBar
        );
        let xy = matches!(kind, ChartType::Scatter | ChartType::Bubble);
        let (category_position, value_position) = match (horizontal, self.secondary) {
            (true, false) => ("l", "b"),
            (true, true) => ("r", "t"),
            (false, false) => ("b", "l"),
            (false, true) => ("t", "r"),
        };
        let category_tag = if xy { "c:valAx" } else { "c:catAx" };
        xml.open0(category_tag);
        xml.val("c:axId", self.category);
        scaling(xml, chart.cat_reversed && !xy, None, None, None);
        xml.val("c:delete", u8::from(self.secondary));
        xml.val("c:axPos", category_position);
        if !self.secondary
            && let Some(caption) = &chart.axis_titles.0
        {
            title(xml, caption, chart);
        }
        number_format(xml, None, "General");
        label_settings(xml, chart, if xy { "out" } else { "none" });
        crossing(xml, self.value, self.secondary);
        if xy {
            xml.val("c:crossBetween", "midCat");
        } else {
            xml.val("c:auto", 1);
            xml.val("c:lblAlgn", "ctr");
            xml.val("c:lblOffset", 100);
            xml.val("c:noMultiLvlLbl", 0);
        }
        xml.close(category_tag);

        xml.open0("c:valAx");
        xml.val("c:axId", self.value);
        let (min, max, major, logarithm, format, hidden) = if self.secondary {
            (
                chart.y2_min,
                chart.y2_max,
                chart.y2_major,
                None,
                chart.y2_format.as_deref(),
                chart.y2_hidden,
            )
        } else {
            (
                chart.y_min,
                chart.y_max,
                chart.y_major,
                chart.y_log_base,
                chart.y_format.as_deref(),
                chart.y_hidden,
            )
        };
        scaling(xml, false, min, max, logarithm);
        xml.val("c:delete", u8::from(hidden));
        xml.val("c:axPos", value_position);
        if !self.secondary {
            if chart.gridlines {
                xml.empty0("c:majorGridlines");
            }
            if chart.minor_gridlines {
                xml.empty0("c:minorGridlines");
            }
            if let Some(caption) = &chart.axis_titles.1 {
                title(xml, caption, chart);
            }
        }
        number_format(xml, format, if kind.percent() { "0%" } else { "General" });
        label_settings(xml, chart, "none");
        crossing(xml, self.category, self.secondary);
        let between = if matches!(
            kind,
            ChartType::Area | ChartType::StackedArea | ChartType::Scatter
        ) {
            "midCat"
        } else {
            "between"
        };
        xml.val("c:crossBetween", between);
        if let Some(unit) = major.filter(|unit| unit.is_finite() && *unit > 0.0) {
            xml.val("c:majorUnit", format_number(unit));
        }
        xml.close("c:valAx");
    }
}

fn scaling(
    xml: &mut W,
    reversed: bool,
    min: Option<f64>,
    max: Option<f64>,
    logarithm: Option<f64>,
) {
    xml.open0("c:scaling");
    if let Some(base) = logarithm.filter(|base| (2.0..=1000.0).contains(base)) {
        xml.val("c:logBase", format_number(base));
    }
    xml.val("c:orientation", if reversed { "maxMin" } else { "minMax" });
    for (tag, limit) in [("c:max", max), ("c:min", min)] {
        if let Some(limit) = limit.filter(|value| value.is_finite()) {
            xml.val(tag, format_number(limit));
        }
    }
    xml.close("c:scaling");
}

fn number_format(xml: &mut W, explicit: Option<&str>, default: &str) {
    let explicit = explicit.filter(|value| !value.is_empty());
    xml.empty(
        "c:numFmt",
        A::new()
            .a("formatCode", explicit.unwrap_or(default))
            .a("sourceLinked", u8::from(explicit.is_none())),
    );
}

fn label_settings(xml: &mut W, chart: &Chart, major_tick: &str) {
    xml.val("c:majorTickMark", major_tick);
    xml.val("c:minorTickMark", "none");
    xml.val("c:tickLblPos", "nextTo");
    tx_pr(xml, chart);
}

fn crossing(xml: &mut W, other: u32, secondary: bool) {
    xml.val("c:crossAx", other);
    xml.val("c:crosses", if secondary { "max" } else { "autoZero" });
}
