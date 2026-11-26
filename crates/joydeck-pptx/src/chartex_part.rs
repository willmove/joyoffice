//! Read the dimension and series tables of an Office extended chart (MS-ODRAWXML §2.24).

use std::collections::HashMap;

use joydeck_model::chart::{Chart, ChartType, MAX_LEVELS, Series};

use crate::{chart_cache::MAX_CHART_POINTS, read, xml::El};

const MAX_LEVEL_POINTS: usize = 100_000;
const MAX_DATASETS: usize = 64;
const MAX_DIMENSIONS: usize = 64;
const MAX_SERIES: usize = 1000;

#[derive(Default)]
struct Dataset<'a> {
    leaves: Vec<String>,
    parents: Vec<Vec<String>>,
    numeric: Vec<Dimension<'a>>,
}

struct Dimension<'a> {
    role: &'a str,
    format: Option<&'a str>,
    values: Vec<Option<f64>>,
}

impl<'a> Dataset<'a> {
    fn read(dataset: &'a El, available: &mut usize) -> Self {
        let mut data = Self::default();
        for node in dataset.elements().take(MAX_DIMENSIONS) {
            if *available == 0 {
                break;
            }
            let mut levels = node.children_named("lvl");
            let Some(first) = levels.next() else {
                continue;
            };
            let role = node.attr("type").unwrap_or_default();
            if node.is("strDim") || (node.is("numDim") && role == "cat") {
                if role == "cat" || data.leaves.is_empty() {
                    data.leaves = read_level(first, available, El::text);
                    data.parents = levels
                        .take(MAX_LEVELS)
                        .map(|level| read_level(level, available, El::text))
                        .collect();
                }
            } else if node.is("numDim") {
                let dimension = Dimension {
                    role,
                    format: first.attr("formatCode"),
                    values: read_level(first, available, |point| {
                        point
                            .text()
                            .trim()
                            .parse::<f64>()
                            .ok()
                            .filter(|value| value.is_finite())
                    }),
                };
                if let Some(existing) = data.numeric.iter_mut().find(|item| item.role == role) {
                    *existing = dimension;
                } else {
                    data.numeric.push(dimension);
                }
            }
        }
        data
    }

    fn values(&self) -> Option<&Dimension<'_>> {
        ["val", "size", "colorVal"]
            .iter()
            .find_map(|role| {
                self.numeric
                    .iter()
                    .find(|dimension| dimension.role == *role)
            })
            .or_else(|| self.numeric.first())
    }
}

fn read_level<T: Default>(level: &El, available: &mut usize, decode: impl Fn(&El) -> T) -> Vec<T> {
    let limit = (*available).min(MAX_LEVEL_POINTS);
    let declared = level.u32("ptCount").unwrap_or_default() as usize;
    let mut values = Vec::new();
    values.resize_with(declared.min(limit), T::default);
    for point in level.children_named("pt").take(limit) {
        let index = point.u32("idx").unwrap_or_default() as usize;
        if index < limit {
            if values.len() <= index {
                values.resize_with(index + 1, T::default);
            }
            values[index] = decode(point);
        }
    }
    *available -= values.len();
    values
}

pub(crate) fn read_chartex_xml(bytes: &[u8]) -> Option<Chart> {
    let mut doc = crate::xml::parse(bytes).ok()?;
    read::resolve_mc(&mut doc.root, 0);
    let mut chart = chartex_from(&doc.root)?;
    chart.raw = Some(String::from_utf8_lossy(bytes).into_owned());
    Some(chart)
}

pub(crate) fn chartex_from(root: &El) -> Option<Chart> {
    if !root.is("chartSpace") {
        return None;
    }
    let mut decoding_budget = MAX_CHART_POINTS;
    let datasets: HashMap<_, _> = root
        .child("chartData")?
        .children_named("data")
        .take(MAX_DATASETS)
        .map(|node| {
            (
                node.attr("id").unwrap_or("0"),
                Dataset::read(node, &mut decoding_budget),
            )
        })
        .collect();
    let plot = root.child("chart")?;
    let region = plot.path(&["plotArea", "plotAreaRegion"])?;
    let mut chart = Chart {
        categories: Vec::new(),
        series: Vec::new(),
        title: plot.child("title").map(|node| {
            read::chart::rich_text(node)
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| "Chart Title".into())
        }),
        legend: plot
            .child("legend")
            .map(|node| node.attr("pos").unwrap_or("t").into()),
        ..Chart::default()
    };
    let mut kind = None;
    let mut remaining = MAX_CHART_POINTS;
    for series in region.children_named("series").take(MAX_SERIES) {
        let layout = series.attr("layoutId").unwrap_or_default();
        if layout == "paretoLine" {
            kind = Some(ChartType::Pareto);
            continue;
        }
        let layout_type = match layout {
            "waterfall" => ChartType::Waterfall,
            "funnel" => ChartType::Funnel,
            "treemap" => ChartType::Treemap,
            "sunburst" => ChartType::Sunburst,
            "clusteredColumn" => ChartType::Histogram,
            "boxWhisker" => ChartType::BoxWhisker,
            "regionMap" => ChartType::RegionMap,
            _ => return None,
        };
        if series.bool("hidden") == Some(true) {
            continue;
        }
        if kind != Some(ChartType::Pareto) {
            kind = Some(layout_type);
        }
        let id = series
            .child("dataId")
            .and_then(|node| node.attr("val"))
            .unwrap_or("0");
        let data = datasets.get(id)?;
        let (values, number_format) = if let Some(dimension) = data.values() {
            let count = dimension.values.len().min(remaining);
            remaining -= count;
            (
                dimension.values[..count].to_vec(),
                dimension.format.map(str::to_owned),
            )
        } else {
            (Vec::new(), None)
        };
        if chart.categories.is_empty() && chart.category_levels.is_empty() {
            chart.categories.clone_from(&data.leaves);
            chart.category_levels.clone_from(&data.parents);
            remaining = remaining.saturating_sub(
                data.leaves.len() + data.parents.iter().map(Vec::len).sum::<usize>(),
            );
        }
        if let Some(properties) = series.child("layoutPr") {
            read_layout(&mut chart, properties);
        }
        chart.series.push(Series {
            name: series
                .child("tx")
                .and_then(|node| node.find("v"))
                .map(El::text)
                .unwrap_or_default(),
            values,
            number_format,
            data_labels: series
                .child("dataLabels")
                .map(|node| node.bool("hidden") != Some(true)),
            ..Series::default()
        });
    }
    chart.kind = kind?;
    chart.data_labels = chart
        .series
        .iter()
        .any(|series| series.data_labels == Some(true));
    use ChartType::{BoxWhisker, Funnel, Histogram, Pareto, Sunburst, Treemap, Waterfall};
    chart.vary_colors = matches!(chart.kind, Funnel | Treemap | Sunburst | Waterfall);
    chart.gridlines = matches!(chart.kind, Waterfall | Histogram | Pareto | BoxWhisker);
    Some(chart)
}

fn read_layout(chart: &mut Chart, properties: &El) {
    if let Some(node) = properties.child("statistics") {
        chart.inclusive_quartiles = node.attr("quartileMethod") == Some("inclusive");
    }
    if let Some(node) = properties.child("binning") {
        chart.bin_count = node
            .child("binCount")
            .and_then(|count| {
                count
                    .u32("val")
                    .or_else(|| count.text().trim().parse().ok())
            })
            .filter(|count| *count > 0);
        chart.bin_size = node
            .child("binSize")
            .and_then(|size| size.f64("val").or_else(|| size.text().trim().parse().ok()))
            .filter(|value| value.is_finite() && *value > 0.0);
    }
    if let Some(node) = properties.child("subtotals") {
        chart.subtotals = node
            .children_named("idx")
            .filter_map(|point| {
                point
                    .u32("val")
                    .or_else(|| point.text().trim().parse().ok())
            })
            .take(MAX_LEVEL_POINTS)
            .map(|index| index as usize)
            .collect();
    }
}
