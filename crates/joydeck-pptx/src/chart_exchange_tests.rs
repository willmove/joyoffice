use std::sync::Arc;

use joydeck_model::chart::{Chart, ChartType, Series};
use joydeck_model::{Presentation, Shape, ShapeId, ShapeKind, Xfrm, defaults};

use crate::chart_style_parts::{PALETTE_REL, STYLE_REL};
use crate::opc::{Package, Rel};
use crate::xml::{Doc, El};

fn chart_document(chart: Chart) -> Presentation {
    let mut document = defaults::blank_presentation(defaults::WIDE, Default::default(), true);
    let id = document.alloc_id().expect("shape id");
    Arc::make_mut(&mut document.slides[0]).shapes = vec![Shape {
        id: ShapeId(id),
        kind: ShapeKind::Chart(Box::new(chart)),
        xfrm: Some(Xfrm::new(24.0, 24.0, 640.0, 360.0)),
        ..Default::default()
    }];
    document
}

fn only_chart(document: &mut Presentation) -> &mut Chart {
    let ShapeKind::Chart(chart) = &mut Arc::make_mut(&mut document.slides[0]).shapes[0].kind else {
        panic!("chart missing");
    };
    chart
}

fn resolve(archive: &Package, owner: &str, kind: &str) -> Rel {
    archive
        .rels(owner)
        .items
        .into_iter()
        .find(|edge| edge.ty == kind || edge.kind == kind)
        .expect("relationship target")
}

fn sheet(archive: &Package, owner: &str) -> Doc {
    let workbook = resolve(archive, owner, "package");
    let bytes = archive.get(&workbook.target).expect("embedded workbook");
    let zip = joyoffice_opc::ZipArchive::new(bytes, &Default::default()).expect("workbook ZIP");
    let index = zip
        .entries()
        .iter()
        .position(|file| file.name == "xl/worksheets/sheet1.xml")
        .expect("worksheet entry");
    crate::xml::parse(&zip.read(index).expect("worksheet bytes")).expect("worksheet XML")
}

#[test]
fn preserving_extended_value_keeps_chart_sidecars_and_workbook_ranges()
-> Result<(), crate::PptxError> {
    let mut chart = Chart::sample(ChartType::Treemap);
    chart.series[0].values[0] = Some(7.0);
    let source = crate::export(&chart_document(chart))?;
    let base = crate::import(&source)?;
    let slide = &base.slides[0];
    let bindings = [crate::SlideBinding {
        slide: slide.id,
        source: slide.id,
        shapes: slide
            .shapes
            .iter()
            .map(|shape| (shape.id, shape.id))
            .collect(),
    }];
    let mut changed = base.clone();
    only_chart(&mut changed).series[0].values[0] = Some(31.0);
    let output = crate::export_preserving(&source, &base, &changed, &bindings)?;
    let archive = Package::open(&output)?;
    let original = Package::open(&source)?;
    let owner = "ppt/slides/slide1.xml";
    let original_chart = resolve(&original, owner, "chartEx");
    let rewritten = archive
        .rels(owner)
        .all_of("chartEx")
        .map(|edge| edge.target.clone())
        .find(|target| *target != original_chart.target);
    assert!(rewritten.is_some());
    if let Some(rewritten) = rewritten {
        assert_eq!(
            archive.get(&original_chart.target),
            original.get(&original_chart.target)
        );
        assert_eq!(
            resolve(&archive, &rewritten, STYLE_REL).target,
            resolve(&original, &original_chart.target, STYLE_REL).target
        );
        let cell = archive.xml(&rewritten).and_then(|doc| {
            let formula = doc.root.find("numDim")?.child("f")?.text();
            let (_, range) = formula.rsplit_once('!')?;
            Some(range.split(':').next()?.replace('$', ""))
        });
        assert!(cell.is_some());
        let workbook = sheet(&archive, &rewritten);
        assert_eq!(
            workbook.root.find("sheetData").and_then(|data| {
                let mut cells = Vec::new();
                data.find_all("c", &mut cells);
                cells
                    .into_iter()
                    .find(|node| node.attr("r") == cell.as_deref())
                    .and_then(|cell| cell.child("v"))
                    .map(El::text)
            }),
            Some("31".into())
        );
    }
    let mut reopened = crate::import(&output)?;
    assert_eq!(only_chart(&mut reopened).series[0].values[0], Some(31.0));
    Ok(())
}

#[test]
fn rebuilt_classic_combo_preserves_series_order_and_secondary_axis() {
    let series = |name: &str, kind, secondary_axis| Series {
        name: name.into(),
        kind: Some(kind),
        secondary_axis,
        values: vec![Some(1.0), None, Some(3.0)],
        ..Series::default()
    };
    let chart = Chart {
        kind: ChartType::Combo,
        categories: vec!["A".into(), "B".into(), "C".into()],
        series: vec![
            series("Primary A", ChartType::Column, false),
            series("Secondary", ChartType::Line, true),
            series("Primary B", ChartType::Column, false),
        ],
        y_min: Some(0.0),
        y_major: Some(1.0),
        y2_min: Some(0.0),
        y2_max: Some(30.0),
        y2_major: Some(5.0),
        y2_format: Some("0.0".into()),
        y2_hidden: true,
        ..Chart::default()
    };
    let written = crate::export(&chart_document(chart.clone())).expect("write PPTX");
    let mut reopened = crate::import(&written).expect("reopen PPTX");
    let result = only_chart(&mut reopened);
    assert_eq!(result.kind, ChartType::Combo);
    assert_eq!(
        result
            .series
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        ["Primary A", "Secondary", "Primary B"]
    );
    assert_eq!(
        result
            .series
            .iter()
            .map(|item| item.secondary_axis)
            .collect::<Vec<_>>(),
        [false, true, false]
    );
    assert_eq!(result.y_major, chart.y_major);
    assert_eq!(result.y2_min, chart.y2_min);
    assert_eq!(result.y2_max, chart.y2_max);
    assert_eq!(result.y2_major, chart.y2_major);
    assert_eq!(result.y2_format, chart.y2_format);
    assert!(result.y2_hidden);
    assert_eq!(result.series[1].values, chart.series[1].values);
}

#[test]
fn regenerated_bubble_caches_reference_distinct_workbook_columns() {
    let series = |name: &str, x, y, size| Series {
        name: name.into(),
        values: vec![Some(y), Some(y + 1.0)],
        x_values: vec![Some(x), Some(x + 1.0)],
        sizes: vec![Some(size), None],
        ..Series::default()
    };
    let chart = Chart {
        kind: ChartType::Bubble,
        series: vec![
            series("First", 21.0, 10.0, 4.0),
            series("Second", 41.0, 20.0, 8.0),
        ],
        ..Chart::default()
    };
    let pptx = crate::export(&chart_document(chart)).expect("write PPTX with XY data");
    let package = Package::open(&pptx).expect("open ZIP");
    let name = "ppt/charts/chart1.xml";
    let chart = package.xml(name).expect("chart XML");
    let data = sheet(&package, name);
    let cells = data.root.child("sheetData").expect("sheet data");
    let value_at = |address: &str| {
        cells
            .children_named("row")
            .flat_map(|row| row.children_named("c"))
            .find(|cell| cell.attr("r") == Some(address))
            .and_then(|cell| cell.child("v"))
            .map(El::text)
    };
    let plot = chart
        .root
        .path(&["chart", "plotArea", "bubbleChart"])
        .expect("bubble plot");
    let range = |series: &El, field: &str| {
        series
            .path(&[field, "numRef", "f"])
            .expect("workbook reference")
            .text()
    };
    for (series, (columns, values)) in plot.children_named("ser").zip([
        (["C", "B", "D"], ["21", "10", "4"]),
        (["F", "E", "G"], ["41", "20", "8"]),
    ]) {
        for ((field, column), value) in ["xVal", "yVal", "bubbleSize"]
            .into_iter()
            .zip(columns)
            .zip(values)
        {
            assert_eq!(
                range(series, field),
                format!("Sheet1!${column}$2:${column}$3")
            );
            assert_eq!(value_at(&format!("{column}2")).as_deref(), Some(value));
        }
    }
    assert!(value_at("D3").is_none());
    assert!(value_at("G3").is_none());
}

#[test]
fn treemap_style_closure_and_workbook_are_written_together() {
    let mut chart = Chart::sample(ChartType::Treemap);
    chart.series[0].values[1] = None;
    let original = crate::export(&chart_document(chart)).expect("new tree");
    let archive = Package::open(&original).expect("original zip");
    let name = "ppt/charts/chartEx1.xml";
    let style = |package: &Package, ty| {
        let rel = resolve(package, name, ty);
        package.get(&rel.target).expect("style bytes").to_vec()
    };
    let mut model = crate::import(&original).expect("open tree");
    let chart = only_chart(&mut model);
    assert_eq!(chart.series[0].values[1], None);
    chart.series[0].values[0] = Some(95.0);
    let revised = crate::export(&model).expect("edit tree");
    let reloaded = Package::open(&revised).expect("rebuilt package");
    for ty in [STYLE_REL, PALETTE_REL] {
        assert_eq!(style(&archive, ty), style(&reloaded, ty));
    }
    let sheet = sheet(&reloaded, name);
    assert!(sheet.root.find("sheetData").is_some());
    let chart_part = reloaded.xml(name).expect("rebuilt chart");
    let workbook = resolve(&reloaded, name, "package");
    assert_eq!(
        chart_part
            .root
            .path(&["chartData", "externalData"])
            .and_then(|node| node.attr("r:id")),
        Some(workbook.id.as_str())
    );
    let mut reopened = crate::import(&revised).expect("reopen tree");
    assert_eq!(only_chart(&mut reopened).series[0].values[0], Some(95.0));
}

#[test]
fn office_binning_and_pareto_axis_use_value_attributes() {
    let mut model = Chart::sample(ChartType::Histogram);
    model.bin_count = Some(4);
    let name = "ppt/charts/chartEx1.xml";
    let bytes = crate::export(&chart_document(model.clone())).expect("histogram");
    let package = Package::open(&bytes).expect("histogram package");
    let doc = package.xml(name).expect("histogram chart");
    assert_eq!(
        doc.root.find("binCount").and_then(|node| node.u32("val")),
        Some(4)
    );

    model.kind = ChartType::Pareto;
    model.bin_count = None;
    model.bin_size = Some(8.0);
    let bytes = crate::export(&chart_document(model)).expect("Pareto");
    let package = Package::open(&bytes).expect("Pareto package");
    let doc = package.xml(name).expect("Pareto chart");
    assert_eq!(
        doc.root.find("binSize").and_then(|node| node.f64("val")),
        Some(8.0)
    );
    assert_eq!(
        doc.root.find("axisId").and_then(|node| node.u32("val")),
        Some(2)
    );
    let mut reopened = crate::import(&bytes).expect("reopen Pareto");
    assert_eq!(only_chart(&mut reopened).bin_size, Some(8.0));
}
