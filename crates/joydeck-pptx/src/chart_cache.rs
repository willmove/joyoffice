//! Spreadsheet-style sparse caches in DrawingML chart parts.

use crate::xml::{A, El, W};

pub(super) const MAX_CHART_POINTS: usize = 1_000_000;
const MAX_CACHE_POINTS: usize = 100_000;

pub(super) fn strings(source: &El, available: &mut usize) -> Vec<String> {
    values(source, available, El::text)
}

pub(super) fn numbers(source: &El, available: &mut usize) -> Vec<Option<f64>> {
    values(source, available, |node| {
        node.text()
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
    })
}

fn values<T: Default>(source: &El, available: &mut usize, convert: impl Fn(&El) -> T) -> Vec<T> {
    let limit = (*available).min(MAX_CACHE_POINTS);
    let cache = ["strCache", "numCache", "strLit", "numLit"]
        .into_iter()
        .find_map(|kind| source.find(kind));
    let values = if let Some(cache) = cache {
        let size = cache
            .child("ptCount")
            .and_then(|n| n.u32("val"))
            .unwrap_or_default() as usize;
        let mut values = Vec::new();
        values.resize_with(size.min(limit), T::default);
        for point in cache.children_named("pt").take(MAX_CACHE_POINTS) {
            let index = point.u32("idx").unwrap_or_default() as usize;
            if index < limit {
                if index >= values.len() {
                    values.resize_with(index + 1, T::default);
                }
                values[index] = point.child("v").map(&convert).unwrap_or_default();
            }
        }
        values
    } else if let Some(level) = source
        .find("multiLvlStrCache")
        .and_then(|cache| cache.child("lvl"))
    {
        level
            .children_named("pt")
            .take(limit)
            .map(|point| point.child("v").map(&convert).unwrap_or_default())
            .collect()
    } else if limit > 0 {
        source.child("v").map(convert).into_iter().collect()
    } else {
        Vec::new()
    };
    *available -= values.len();
    values
}

pub(super) fn write_strings(xml: &mut W, formula: Option<&str>, values: &[String]) {
    let tag = if let Some(formula) = formula {
        xml.open0("c:strRef");
        xml.elt("c:f", formula);
        "c:strCache"
    } else {
        "c:strLit"
    };
    xml.open0(tag);
    xml.val("c:ptCount", values.len());
    for (index, value) in values.iter().enumerate() {
        point(xml, index, value);
    }
    xml.close(tag);
    if formula.is_some() {
        xml.close("c:strRef");
    }
}

pub(super) fn write_numbers(
    xml: &mut W,
    formula: Option<&str>,
    values: &[Option<f64>],
    format: Option<&str>,
) {
    let tag = if let Some(formula) = formula {
        xml.open0("c:numRef");
        xml.elt("c:f", formula);
        "c:numCache"
    } else {
        "c:numLit"
    };
    xml.open0(tag);
    if let Some(format) = format.or_else(|| formula.map(|_| "General")) {
        xml.elt("c:formatCode", format);
    }
    numeric_points(xml, values);
    xml.close(tag);
    if formula.is_some() {
        xml.close("c:numRef");
    }
}

pub(super) fn num_lit(xml: &mut W, values: &[Option<f64>]) {
    xml.open0("c:numLit");
    numeric_points(xml, values);
    xml.close("c:numLit");
}

fn numeric_points(xml: &mut W, values: &[Option<f64>]) {
    xml.val("c:ptCount", values.len());
    for (index, value) in values.iter().enumerate() {
        if let Some(value) = value.filter(|value| value.is_finite()) {
            point(xml, index, &format_number(value));
        }
    }
}

fn point(xml: &mut W, index: usize, value: &str) {
    xml.open("c:pt", A::new().a("idx", index));
    xml.elt("c:v", value);
    xml.close("c:pt");
}

pub(super) fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        (value as i64).to_string()
    } else {
        value.to_string()
    }
}
