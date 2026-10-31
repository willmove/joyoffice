//! Transitional DrawingML charts with a fresh embedded SpreadsheetML workbook.
use crate::{
    chart::{Chart, Series},
    document::{Object, Result},
    interop::escape,
};
use roxmltree::Node;
use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
};
const C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
fn col(mut n: usize) -> String {
    let mut out = String::new();
    loop {
        out.insert(0, (b'A' + (n % 26) as u8) as char);
        if n < 26 {
            return out;
        }
        n = n / 26 - 1;
    }
}
fn workbook(c: &Chart) -> Result<Vec<u8>> {
    let scatter = c.kind == "scatter";
    let mut data = Vec::<Vec<Option<String>>>::new();
    if scatter {
        data.push(
            c.series
                .iter()
                .flat_map(|s| [Some(format!("{} X", s.name)), Some(s.name.clone())])
                .collect(),
        );
        for i in 0..c.categories.len() {
            data.push(
                c.series
                    .iter()
                    .flat_map(|s| {
                        [
                            s.x.get(i)
                                .copied()
                                .unwrap_or(Some((i + 1) as f64))
                                .map(|n| n.to_string()),
                            s.values[i].map(|n| n.to_string()),
                        ]
                    })
                    .collect(),
            );
        }
    } else {
        data.push(
            std::iter::once(Some("类别".into()))
                .chain(c.series.iter().map(|s| Some(s.name.clone())))
                .collect(),
        );
        for i in 0..c.categories.len() {
            data.push(
                std::iter::once(Some(c.categories[i].clone()))
                    .chain(c.series.iter().map(|s| s.values[i].map(|n| n.to_string())))
                    .collect(),
            );
        }
    }
    let mut rows = String::new();
    for (r, row) in data.iter().enumerate() {
        rows += &format!("<row r=\"{}\">", r + 1);
        for (i, v) in row.iter().enumerate() {
            if let Some(v) = v {
                let address = format!("{}{}", col(i), r + 1);
                if r == 0 || (!scatter && i == 0) {
                    rows += &format!(
                        r#"<c r="{address}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
                        escape(v)
                    );
                } else {
                    rows += &format!(r#"<c r="{address}"><v>{}</v></c>"#, escape(v));
                }
            }
        }
        rows += "</row>";
    }
    let files=[("[Content_Types].xml",r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#.into()),("_rels/.rels",format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="{R}/officeDocument" Target="xl/workbook.xml"/></Relationships>"#)),("xl/workbook.xml",format!(r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="{R}"><sheets><sheet name="Sheet1" sheetId="1" r:id="sheet1"/></sheets></workbook>"#)),("xl/_rels/workbook.xml.rels",format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="sheet1" Type="{R}/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#)),("xl/worksheets/sheet1.xml",format!(r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{rows}</sheetData></worksheet>"#))];
    let mut z = zip::ZipWriter::new(Cursor::new(vec![]));
    for (name, xml) in files {
        z.start_file(
            name,
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .map_err(|e| e.to_string())?;
        z.write_all(xml.as_bytes()).map_err(|e| e.to_string())?;
    }
    Ok(z.finish().map_err(|e| e.to_string())?.into_inner())
}
fn rich(value: &str) -> String {
    format!(
        r#"<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="zh-CN"/><a:t>{}</a:t></a:r></a:p></c:rich></c:tx><c:layout/><c:overlay val="0"/></c:title>"#,
        escape(value)
    )
}
fn cache(values: &[Option<f64>], formula: &str) -> String {
    let points = values
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.map(|v| format!(r#"<c:pt idx="{i}"><c:v>{v}</c:v></c:pt>"#)))
        .collect::<String>();
    format!(
        r#"<c:numRef><c:f>{formula}</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="{}"/>{points}</c:numCache></c:numRef>"#,
        values.len()
    )
}
fn axis(c: &Chart, id: usize, cross: usize, category: bool, pos: &str, title: &str) -> String {
    let tag = if category { "catAx" } else { "valAx" };
    let mut scale = "<c:orientation val=\"minMax\"/>".to_owned();
    if !category && id == 200 {
        if let Some(v) = c.maximum {
            scale += &format!(r#"<c:max val="{v}"/>"#);
        }
        if let Some(v) = c.minimum {
            scale += &format!(r#"<c:min val="{v}"/>"#);
        }
    }
    let grid = if !category && c.grid {
        r#"<c:majorGridlines><c:spPr><a:ln w="9525"><a:solidFill><a:srgbClr val="e5e7eb"/></a:solidFill></a:ln></c:spPr></c:majorGridlines>"#
    } else {
        ""
    };
    format!(
        r#"<c:{tag}><c:axId val="{id}"/><c:scaling>{scale}</c:scaling><c:delete val="0"/><c:axPos val="{pos}"/>{grid}{}<c:numFmt formatCode="{}" sourceLinked="0"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:crossAx val="{cross}"/><c:crosses val="autoZero"/>{}<c:spPr><a:ln w="6350"><a:solidFill><a:srgbClr val="d8dadd"/></a:solidFill></a:ln></c:spPr></c:{tag}>"#,
        if title.is_empty() {
            String::new()
        } else {
            rich(title)
        },
        escape(&if c.unit.is_empty() {
            "General".into()
        } else {
            format!("0\"{}\"", c.unit.replace('"', ""))
        }),
        if category {
            "<c:auto val=\"1\"/><c:lblAlgn val=\"ctr\"/><c:lblOffset val=\"100\"/>"
        } else {
            "<c:crossBetween val=\"between\"/>"
        }
    )
}
pub fn write(c: &Chart, name: &str) -> Result<(String, Vec<u8>, String)> {
    c.validate()?;
    let tag = match c.kind.as_str() {
        "line" => "lineChart",
        "area" => "areaChart",
        "pie" => "pieChart",
        "doughnut" => "doughnutChart",
        "scatter" => "scatterChart",
        _ => "barChart",
    };
    let scatter = c.kind == "scatter";
    let mut series = String::new();
    for (j, s) in c.series.iter().enumerate() {
        let scol = if scatter { j * 2 + 1 } else { j + 1 };
        let color = &c.colors[j % c.colors.len()];
        let color = if color.starts_with('#') {
            color.clone()
        } else {
            "#4472c4".into()
        };
        let mut body = format!(
            r#"<c:idx val="{j}"/><c:order val="{j}"/><c:tx><c:strRef><c:f>Sheet1!${}$1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>{}</c:v></c:pt></c:strCache></c:strRef></c:tx><c:spPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:ln w="25400"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:ln></c:spPr>"#,
            col(scol),
            escape(&s.name),
            &color[1..],
            &color[1..]
        );
        if c.kind == "line" || scatter {
            body += r#"<c:marker><c:symbol val="circle"/><c:size val="5"/></c:marker>"#;
        }
        if matches!(c.kind.as_str(), "pie" | "doughnut") {
            for i in 0..c.categories.len() {
                let color = &c.colors[i % c.colors.len()];
                let color = color.strip_prefix('#').unwrap_or("4472c4");
                body += &format!(
                    r#"<c:dPt><c:idx val="{i}"/><c:spPr><a:solidFill><a:srgbClr val="{color}"/></a:solidFill></c:spPr></c:dPt>"#
                );
            }
        }
        let n = c.categories.len() + 1;
        if scatter {
            let x: Vec<_> = (0..c.categories.len())
                .map(|i| s.x.get(i).copied().unwrap_or(Some((i + 1) as f64)))
                .collect();
            body += &format!(
                "<c:xVal>{}</c:xVal><c:yVal>{}</c:yVal>",
                cache(&x, &format!("Sheet1!${}$2:${}${n}", col(j * 2), col(j * 2))),
                cache(
                    &s.values,
                    &format!("Sheet1!${}$2:${}${n}", col(scol), col(scol))
                )
            );
        } else {
            let cats = c
                .categories
                .iter()
                .enumerate()
                .map(|(i, v)| format!(r#"<c:pt idx="{i}"><c:v>{}</c:v></c:pt>"#, escape(v)))
                .collect::<String>();
            body += &format!(
                r#"<c:cat><c:strRef><c:f>Sheet1!$A$2:$A${n}</c:f><c:strCache><c:ptCount val="{}"/>{cats}</c:strCache></c:strRef></c:cat><c:val>{}</c:val>"#,
                c.categories.len(),
                cache(
                    &s.values,
                    &format!("Sheet1!${}$2:${}${n}", col(scol), col(scol))
                )
            );
        }
        if c.kind == "line" || scatter {
            body += "<c:smooth val=\"0\"/>";
        }
        series += &format!("<c:ser>{body}</c:ser>");
    }
    let opts = match c.kind.as_str() {
        "line" | "area" => "<c:grouping val=\"standard\"/>".into(),
        "pie" | "doughnut" => "<c:varyColors val=\"1\"/>".into(),
        "scatter" => "<c:scatterStyle val=\"marker\"/><c:varyColors val=\"0\"/>".into(),
        _ => format!(
            r#"<c:barDir val="{}"/><c:grouping val="clustered"/><c:varyColors val="0"/>"#,
            if c.kind == "bar" { "bar" } else { "col" }
        ),
    };
    if scatter {
        let mut out = String::new();
        let mut rest = series.as_str();
        while let Some(at) = rest.find("<a:ln w=\"25400\">") {
            out += &rest[..at];
            let tail = &rest[at..];
            let end = tail.find("</a:ln>").ok_or("散点线条序列化失败")? + 7;
            out += "<a:ln><a:noFill/></a:ln>";
            rest = &tail[end..];
        }
        out += rest;
        series = out;
    }
    let axes = if c.kind == "doughnut" {
        "<c:holeSize val=\"50\"/>".to_owned()
    } else if c.kind == "pie" {
        String::new()
    } else {
        "<c:axId val=\"100\"/><c:axId val=\"200\"/>".to_owned()
    };
    let axisxml = if matches!(c.kind.as_str(), "pie" | "doughnut") {
        String::new()
    } else {
        axis(
            c,
            100,
            200,
            !scatter,
            if c.kind == "bar" { "l" } else { "b" },
            &c.x_title,
        ) + &axis(
            c,
            200,
            100,
            false,
            if c.kind == "bar" { "b" } else { "l" },
            &c.y_title,
        )
    };
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><c:chartSpace xmlns:c="{C}" xmlns:a="{A}" xmlns:r="{R}"><c:date1904 val="0"/><c:lang val="zh-CN"/><c:chart>{}<c:autoTitleDeleted val="{}"/><c:plotArea><c:layout/><c:{tag}>{opts}{series}<c:dLbls><c:showLegendKey val="0"/><c:showVal val="{}"/><c:showCatName val="0"/><c:showSerName val="0"/><c:showPercent val="{}"/></c:dLbls>{axes}</c:{tag}>{axisxml}</c:plotArea>{}<c:plotVisOnly val="1"/><c:dispBlanksAs val="gap"/><c:showDLblsOverMax val="0"/></c:chart><c:externalData r:id="workbook"><c:autoUpdate val="0"/></c:externalData></c:chartSpace>"#,
        if c.title.is_empty() {
            String::new()
        } else {
            rich(&c.title)
        },
        u8::from(c.title.is_empty()),
        u8::from(c.labels && !matches!(c.kind.as_str(), "pie" | "doughnut")),
        u8::from(c.labels && matches!(c.kind.as_str(), "pie" | "doughnut")),
        if c.legend {
            r#"<c:legend><c:legendPos val="b"/><c:layout/><c:overlay val="0"/></c:legend>"#
        } else {
            ""
        }
    );
    let rel = format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="workbook" Type="{R}/package" Target="../embeddings/{name}.xlsx"/></Relationships>"#
    );
    Ok((xml, workbook(c)?, rel))
}
pub fn frame(o: &Object, id: usize, rid: &str) -> String {
    let emu = |n: f32| (n as f64 * 12700.).round() as i64;
    format!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="{}"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm rot="{}"><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></p:xfrm><a:graphic><a:graphicData uri="{C}"><c:chart xmlns:c="{C}" xmlns:r="{R}" r:id="{rid}"/></a:graphicData></a:graphic></p:graphicFrame>"#,
        escape(&o.id),
        (o.rotation * 60000.) as i64,
        emu(o.x),
        emu(o.y),
        emu(o.width),
        emu(o.height)
    )
}
fn find<'a, 'b>(n: Node<'a, 'b>, tag: &str) -> Option<Node<'a, 'b>> {
    n.descendants().find(|n| n.has_tag_name((C, tag)))
}
fn val(n: Node<'_, '_>, tag: &str) -> Option<f64> {
    find(n, tag)?.attribute("val")?.parse().ok()
}
fn nums(n: Node<'_, '_>, tag: &str) -> Result<Vec<Option<f64>>> {
    let parent = find(n, tag).ok_or("图表缺少数值缓存")?;
    let cache = find(parent, "numCache")
        .or_else(|| find(parent, "numLit"))
        .ok_or("图表不含受支持的数值缓存")?;
    let count = find(cache, "ptCount")
        .and_then(|n| n.attribute("val"))
        .and_then(|s| s.parse::<usize>().ok())
        .ok_or("图表点数必须是非负整数")?;
    if count > 256 {
        return Err("图表点数超限".into());
    }
    let mut values = vec![None; count];
    let mut seen = std::collections::BTreeSet::new();
    for pt in cache.children().filter(|n| n.has_tag_name((C, "pt"))) {
        let i = pt
            .attribute("idx")
            .and_then(|v| v.parse::<usize>().ok())
            .ok_or("图表点索引无效")?;
        if i >= count || !seen.insert(i) {
            return Err("图表点索引越界/重复".into());
        }
        values[i] = Some(
            find(pt, "v")
                .and_then(|v| v.text())
                .ok_or("图表数值缺失")?
                .parse()
                .map_err(|_| "图表数值无效")?,
        );
    }
    Ok(values)
}
pub fn read(
    parts: &BTreeMap<String, Vec<u8>>,
    slide: &str,
    n: Node<'_, '_>,
    page: usize,
) -> Result<Object> {
    let node = find(n, "chart").ok_or("图表引用缺失")?;
    let rid = node.attribute((R, "id")).ok_or("缺少图表关系")?;
    let target = crate::pptx::related(parts, slide, rid)?;
    let doc = crate::pptx::xml(crate::document::part(parts, &target)?)?;
    let root = doc.root_element();
    let plot = find(root, "plotArea").ok_or("图表绘图区缺失")?;
    let typ = plot
        .children()
        .find(|n| {
            n.is_element()
                && [
                    "barChart",
                    "lineChart",
                    "areaChart",
                    "pieChart",
                    "doughnutChart",
                    "scatterChart",
                ]
                .contains(&n.tag_name().name())
        })
        .ok_or("暂不支持的图表类型")?;
    if plot
        .children()
        .filter(|n| n.is_element() && n.tag_name().name().ends_with("Chart"))
        .count()
        != 1
        || typ.descendants().any(|n| {
            matches!(
                n.tag_name().name(),
                "trendline" | "errBars" | "pictureOptions" | "explosion"
            )
        })
    {
        return Err("组合图表/趋势线/误差线/图片或分离扇区不在基础图表范围；原包保留".into());
    }
    if find(typ, "grouping")
        .and_then(|n| n.attribute("val"))
        .is_some_and(|v| !matches!(v, "clustered" | "standard"))
    {
        return Err("堆积图表不受基础模型支持；原包保留".into());
    }
    if typ.has_tag_name((C, "doughnutChart"))
        && find(typ, "holeSize").and_then(|n| n.attribute("val")) != Some("50")
    {
        return Err("环形图当前仅支持50%内径；原包保留".into());
    }
    let kind = match typ.tag_name().name() {
        "barChart" => {
            if find(typ, "barDir").and_then(|n| n.attribute("val")) == Some("bar") {
                "bar"
            } else {
                "column"
            }
        }
        "lineChart" => "line",
        "areaChart" => "area",
        "pieChart" => "pie",
        "doughnutChart" => "doughnut",
        _ => "scatter",
    };
    let mut c = Chart {
        kind: kind.into(),
        categories: vec![],
        series: vec![],
        title: find(root, "title")
            .map(|n| {
                n.descendants()
                    .filter(|n| n.has_tag_name((A, "t")))
                    .filter_map(|n| n.text())
                    .collect()
            })
            .unwrap_or_default(),
        legend: find(root, "legend").is_some(),
        grid: find(root, "majorGridlines").is_some(),
        labels: val(typ, "showVal") == Some(1.) || val(typ, "showPercent") == Some(1.),
        ..Default::default()
    };
    for ser in typ.children().filter(|n| n.has_tag_name((C, "ser"))) {
        let values = nums(ser, if kind == "scatter" { "yVal" } else { "val" })?;
        let x = if kind == "scatter" {
            nums(ser, "xVal")?
        } else {
            vec![]
        };
        if c.categories.is_empty() {
            if kind == "scatter" {
                c.categories = (0..values.len()).map(|i| (i + 1).to_string()).collect();
            } else {
                let cat = find(ser, "cat").ok_or("图表类别缺失")?;
                let mut cats = vec![String::new(); values.len()];
                for p in cat.descendants().filter(|n| n.has_tag_name((C, "pt"))) {
                    let i = p
                        .attribute("idx")
                        .and_then(|i| i.parse::<usize>().ok())
                        .ok_or("图表类别索引无效")?;
                    if i >= cats.len() {
                        return Err("图表类别索引越界".into());
                    }
                    cats[i] = find(p, "v").and_then(|n| n.text()).unwrap_or("").into();
                }
                c.categories = cats;
            }
        }
        let name = find(ser, "tx")
            .and_then(|n| find(n, "v"))
            .and_then(|n| n.text())
            .unwrap_or("系列")
            .into();
        c.series.push(Series { name, values, x });
    }
    let axes: Vec<_> = plot
        .children()
        .filter(|n| n.has_tag_name((C, "valAx")) || n.has_tag_name((C, "catAx")))
        .collect();
    let axis_title = |n: Node<'_, '_>| {
        find(n, "title")
            .map(|n| {
                n.descendants()
                    .filter(|n| n.has_tag_name((A, "t")))
                    .filter_map(|n| n.text())
                    .collect::<String>()
            })
            .unwrap_or_default()
    };
    if let Some(a) = axes.first() {
        c.x_title = axis_title(*a);
    }
    if let Some(a) = axes.get(1) {
        c.y_title = axis_title(*a);
        c.unit = find(*a, "numFmt")
            .and_then(|n| n.attribute("formatCode"))
            .and_then(|s| s.strip_prefix("0\""))
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or("")
            .into();
        c.minimum = val(*a, "min");
        c.maximum = val(*a, "max");
    }
    let colors: Vec<_> = typ
        .children()
        .filter(|n| n.has_tag_name((C, "ser")))
        .filter_map(|n| n.children().find(|n| n.has_tag_name((C, "spPr"))))
        .filter_map(|n| {
            n.descendants()
                .find(|n| n.has_tag_name((A, "srgbClr")))
                .and_then(|n| n.attribute("val"))
        })
        .map(|s| format!("#{s}"))
        .collect();
    if !colors.is_empty() {
        c.colors = colors;
    }
    if matches!(kind, "pie" | "doughnut") {
        let colors: Vec<_> = typ
            .descendants()
            .filter(|n| n.has_tag_name((C, "dPt")))
            .filter_map(|n| {
                n.descendants()
                    .find(|n| n.has_tag_name((A, "srgbClr")))
                    .and_then(|n| n.attribute("val"))
            })
            .map(|s| format!("#{s}"))
            .collect();
        if !colors.is_empty() {
            c.colors = colors;
        }
    }
    verify_workbook(parts, &target, root)?;
    c.validate()?;
    let mut o = crate::chart::object(kind);
    o.id = format!("pptx-chart-{page}-{rid}");
    let xf = n
        .descendants()
        .find(|n| n.tag_name().name() == "xfrm")
        .ok_or("图表变换缺失")?;
    let off = xf
        .descendants()
        .find(|n| n.has_tag_name((A, "off")))
        .ok_or("图表位置缺失")?;
    let ext = xf
        .descendants()
        .find(|n| n.has_tag_name((A, "ext")))
        .ok_or("图表尺寸缺失")?;
    let num = |n: Node<'_, '_>, key: &str| -> Result<f32> {
        Ok(n.attribute(key)
            .ok_or("图表几何缺失")?
            .parse::<f32>()
            .map_err(|_| "图表几何无效")?
            / 12700.)
    };
    o.x = num(off, "x")?;
    o.y = num(off, "y")?;
    o.width = num(ext, "cx")?;
    o.height = num(ext, "cy")?;
    o.rotation = xf
        .attribute("rot")
        .and_then(|n| n.parse::<f32>().ok())
        .unwrap_or(0.)
        / 60000.;
    o.chart = Some(c);
    Ok(o)
}
fn verify_workbook(
    parts: &BTreeMap<String, Vec<u8>>,
    chart: &str,
    root: Node<'_, '_>,
) -> Result<()> {
    const S: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
    let Some(external) = find(root, "externalData") else {
        return Ok(());
    };
    let rid = external.attribute((R, "id")).ok_or("嵌入工作簿关系无效")?;
    let name = crate::pptx::related(parts, chart, rid)?;
    let bytes = parts.get(&name).ok_or("嵌入工作簿缺失")?;
    let book =
        crate::document::read_nested_package(bytes, &std::sync::atomic::AtomicBool::new(false))?;
    let workbook = crate::pptx::xml(crate::document::part(&book, "xl/workbook.xml")?)?;
    for reference in root.descendants().filter(|n| n.has_tag_name((C, "numRef"))) {
        let formula = find(reference, "f")
            .and_then(|n| n.text())
            .ok_or("数值数据范围缺失")?;
        let (sheet, range) = formula
            .rsplit_once('!')
            .ok_or("仅支持工作表单列数值范围；原包保留")?;
        let sheet = sheet.trim_matches('\'');
        let node = workbook
            .descendants()
            .find(|n| n.has_tag_name((S, "sheet")) && n.attribute("name") == Some(sheet))
            .ok_or("图表工作表不存在")?;
        let id = node.attribute((R, "id")).ok_or("工作表关系缺失")?;
        let path = crate::pptx::related(&book, "xl/workbook.xml", id)?;
        let xml = crate::pptx::xml(crate::document::part(&book, &path)?)?;
        let address = |s: &str| -> Result<(String, usize)> {
            let text = s.replace('$', "");
            let i = text
                .bytes()
                .position(|b| b.is_ascii_digit())
                .ok_or("单元格地址无效")?;
            let (col, row) = text.split_at(i);
            if col.is_empty() || !col.bytes().all(|b| b.is_ascii_uppercase()) {
                return Err("图表列地址无效".into());
            }
            Ok((col.into(), row.parse().map_err(|_| "图表行地址无效")?))
        };
        let (a, b) = range.split_once(':').unwrap_or((range, range));
        let (col, start) = address(a)?;
        let (other, end) = address(b)?;
        if col != other || end < start || end - start >= 256 {
            return Err("工作簿范围超出图表预算".into());
        }
        let cache = nums(reference, "numRef")?;
        if cache.len() != end - start + 1 {
            return Err("图表缓存与工作簿范围长度不一致，原包保留".into());
        }
        for (i, value) in cache.iter().enumerate() {
            let address = format!("{col}{}", start + i);
            let cell = xml
                .descendants()
                .find(|n| n.has_tag_name((S, "c")) && n.attribute("r") == Some(&address));
            let actual = cell
                .and_then(|n| n.children().find(|n| n.has_tag_name((S, "v"))))
                .and_then(|n| n.text())
                .map(|v| {
                    v.parse::<f64>()
                        .map_err(|_| "工作簿数值不是有限数字".to_owned())
                })
                .transpose()?;
            let equal = match (*value, actual) {
                (None, None) => true,
                (Some(a), Some(b)) => {
                    a.is_finite()
                        && b.is_finite()
                        && (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.)
                }
                _ => false,
            };
            if !equal {
                return Err(format!(
                    "图表缓存与嵌入工作簿{address}不一致；拒绝将旧缓存冒充最新数据，原包保留"
                ));
            }
        }
    }
    Ok(())
}
