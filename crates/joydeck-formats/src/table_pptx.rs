//! Public DrawingML a:tbl adapter. No private snapshot is needed for table structure/text/style import.
use crate::interop::escape;
use crate::{
    document::{Deck, Object, Result},
    table::{self, Cell, Table, TableStyle},
    text::{Run, Style},
};
use roxmltree::Node;
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
fn emu(n: f32) -> i64 {
    (n as f64 * 12700.).round() as i64
}
fn fill(color: &str, opacity: f32) -> String {
    format!(
        r#"<a:solidFill><a:srgbClr val="{}"><a:alpha val="{}"/></a:srgbClr></a:solidFill>"#,
        &color[1..],
        (opacity * 100000.).round() as i64
    )
}
pub fn write(deck: &Deck, page: usize, o: &Object, id: usize) -> Result<String> {
    let t = o.table.as_ref().ok_or("表格数据缺失")?;
    let mut grid = String::new();
    for w in &t.columns {
        grid.push_str(&format!(r#"<a:gridCol w="{}"/>"#, emu(*w)));
    }
    let mut rows = String::new();
    for (r, h) in t.rows.iter().enumerate() {
        rows.push_str(&format!(r#"<a:tr h="{}">"#, emu(*h)));
        for c in 0..t.columns.len() {
            let (ar, ac) = t.owner(r, c)?;
            let anchor = &t.cells[t.index(ar, ac)?];
            let actual = &t.cells[t.index(r, c)?];
            let style = table::resolve(deck, page, t, ar, ac);
            let covered = actual.span == [0, 0];
            let mut attrs = String::new();
            if r == ar && anchor.span[0] > 1 {
                attrs.push_str(&format!(r#" rowSpan="{}""#, anchor.span[0]));
            }
            if c == ac && anchor.span[1] > 1 {
                attrs.push_str(&format!(r#" gridSpan="{}""#, anchor.span[1]));
            }
            if c != ac {
                attrs.push_str(" hMerge=\"1\"");
            }
            if r != ar {
                attrs.push_str(" vMerge=\"1\"");
            }
            let mut text = String::new();
            let mut start = 0;
            let runs = if actual.runs.is_empty() && !actual.text.is_empty() {
                vec![Run {
                    start: 0,
                    end: actual.text.len(),
                    style: style.text.clone(),
                }]
            } else {
                actual.runs.clone()
            };
            for line in actual.text.split('\n') {
                let end = start + line.len();
                let mut pieces = String::new();
                for run in &runs {
                    let a = run.start.max(start);
                    let b = run.end.min(end);
                    if a < b {
                        let s = &run.style;
                        pieces.push_str(&format!(r#"<a:r><a:rPr sz="{}" b="{}" i="{}">{}<a:latin typeface="{}"/><a:ea typeface="{}"/></a:rPr><a:t xml:space="preserve">{}</a:t></a:r>"#,(s.size*100.).round() as i64,u8::from(s.bold),u8::from(s.italic),fill(&s.color,o.opacity),escape(&s.family),escape(&s.family),escape(&actual.text[a..b])));
                    }
                }
                text.push_str(&format!(
                    r#"<a:p><a:pPr algn="{}"/><a:endParaRPr sz="{}"/>{pieces}</a:p>"#,
                    match style.align.as_str() {
                        "center" => "ctr",
                        "right" => "r",
                        _ => "l",
                    },
                    (style.text.size * 100.) as i64
                ));
                start = end + 1;
            }
            // endParaRPr follows runs in schema order.
            text = text.replace(
                &format!(
                    r#"<a:endParaRPr sz="{}"/>"#,
                    (style.text.size * 100.) as i64
                ),
                "",
            );
            let mut borders = String::new();
            for edge in ["L", "R", "T", "B"] {
                let hidden = covered
                    && match edge {
                        "L" => c > ac,
                        "R" => c + 1 < ac + anchor.span[1],
                        "T" => r > ar,
                        _ => r + 1 < ar + anchor.span[0],
                    };
                if hidden {
                    borders.push_str(&format!("<a:ln{edge}><a:noFill/></a:ln{edge}>"));
                } else {
                    borders.push_str(&format!(
                        r#"<a:ln{edge} w="{}">{}</a:ln{edge}>"#,
                        emu(style.border_width),
                        fill(&style.border, o.opacity)
                    ));
                }
            }
            rows.push_str(&format!(r#"<a:tc{attrs}><a:txBody><a:bodyPr wrap="none"/><a:lstStyle/>{text}</a:txBody><a:tcPr marL="{padding}" marR="{padding}" marT="{padding}" marB="{padding}" anchor="t">{borders}{}</a:tcPr></a:tc>"#,fill(&style.fill,o.opacity),padding=emu(style.padding)));
        }
        rows.push_str("</a:tr>");
    }
    Ok(format!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="{}"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm rot="{}"><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tblPr/><a:tblGrid>{grid}</a:tblGrid>{rows}</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
        escape(&o.id),
        (o.rotation * 60000.).round() as i64,
        emu(o.x),
        emu(o.y),
        emu(o.width),
        emu(o.height)
    ))
}
fn child<'a, 'b>(n: Node<'a, 'b>, ns: &str, name: &str) -> Option<Node<'a, 'b>> {
    n.children().find(|n| n.has_tag_name((ns, name)))
}
fn number(n: Node<'_, '_>, key: &str) -> Result<f32> {
    let n = n
        .attribute(key)
        .ok_or("缺少表格几何")?
        .parse::<f32>()
        .map_err(|_| "表格数值无效")?
        / 12700.;
    if !n.is_finite() || n.abs() > 10000. {
        return Err("表格数值超限".into());
    }
    Ok(n)
}
fn color(n: Node<'_, '_>) -> Option<String> {
    child(n, A, "solidFill")
        .and_then(|n| child(n, A, "srgbClr"))
        .and_then(|n| n.attribute("val"))
        .map(|s| format!("#{s}"))
}
pub fn read(n: Node<'_, '_>, page: usize) -> Result<Option<Object>> {
    let Some(tbl) = n.descendants().find(|n| n.has_tag_name((A, "tbl"))) else {
        return Ok(None);
    };
    let xf = child(n, P, "xfrm").ok_or("表格缺少变换")?;
    let off = child(xf, A, "off").ok_or("表格缺少位置")?;
    let ext = child(xf, A, "ext").ok_or("表格缺少尺寸")?;
    let grid = child(tbl, A, "tblGrid").ok_or("表格缺少网格")?;
    let columns = grid
        .children()
        .filter(|n| n.has_tag_name((A, "gridCol")))
        .map(|n| number(n, "w"))
        .collect::<Result<Vec<_>>>()?;
    let nodes: Vec<_> = tbl
        .children()
        .filter(|n| n.has_tag_name((A, "tr")))
        .collect();
    let mut t = Table::new(nodes.len(), columns.len())?;
    t.columns = columns;
    t.style = TableStyle::default();
    t.style.header = false;
    t.style.banded = false;
    for (r, row) in nodes.iter().enumerate() {
        t.rows[r] = number(*row, "h")?;
        let cells: Vec<_> = row
            .children()
            .filter(|n| n.has_tag_name((A, "tc")))
            .collect();
        if cells.len() != t.columns.len() {
            return Err("公开PPTX表格单元格数量与网格不一致".into());
        }
        for (c, node) in cells.iter().enumerate() {
            let continuation = matches!(node.attribute("hMerge"), Some("1" | "true"))
                || matches!(node.attribute("vMerge"), Some("1" | "true"));
            let mut cell = Cell {
                span: if continuation {
                    [0, 0]
                } else {
                    [
                        node.attribute("rowSpan")
                            .unwrap_or("1")
                            .parse()
                            .map_err(|_| "rowSpan无效")?,
                        node.attribute("gridSpan")
                            .unwrap_or("1")
                            .parse()
                            .map_err(|_| "gridSpan无效")?,
                    ]
                },
                ..Cell::default()
            };
            let props = child(*node, A, "tcPr");
            let body = child(*node, A, "txBody");
            let mut first = true;
            if let Some(body) = body {
                for paragraph in body.children().filter(|n| n.has_tag_name((A, "p"))) {
                    if !first {
                        cell.text.push('\n');
                    }
                    first = false;
                    let paragraph_start = cell.text.len();
                    if let Some(p) = child(paragraph, A, "pPr") {
                        cell.style.align = Some(
                            match p.attribute("algn") {
                                Some("ctr") => "center",
                                Some("r") => "right",
                                _ => "left",
                            }
                            .into(),
                        );
                    }
                    for run in paragraph
                        .children()
                        .filter(|n| n.has_tag_name((A, "r")) || n.has_tag_name((A, "br")))
                    {
                        if run.has_tag_name((A, "br")) {
                            cell.text.push('\n');
                            continue;
                        }
                        let text = child(run, A, "t").and_then(|n| n.text()).unwrap_or("");
                        let start = cell.text.len();
                        cell.text.push_str(text);
                        let pr = child(run, A, "rPr");
                        let style = Style {
                            family: pr
                                .and_then(|p| child(p, A, "latin"))
                                .and_then(|n| n.attribute("typeface"))
                                .unwrap_or("sans-serif")
                                .into(),
                            size: pr
                                .and_then(|p| p.attribute("sz"))
                                .and_then(|s| s.parse::<f32>().ok())
                                .unwrap_or(1800.)
                                / 100.,
                            bold: pr
                                .is_some_and(|p| matches!(p.attribute("b"), Some("1" | "true"))),
                            italic: pr
                                .is_some_and(|p| matches!(p.attribute("i"), Some("1" | "true"))),
                            color: pr.and_then(color).unwrap_or_else(|| "#243247".into()),
                        };
                        if start < cell.text.len() {
                            cell.runs.push(Run {
                                start,
                                end: cell.text.len(),
                                style,
                            });
                        }
                    }
                    if paragraph_start > 0
                        && let Some(run) = cell.runs.iter_mut().find(|r| r.start == paragraph_start)
                    {
                        run.start = paragraph_start - 1;
                    }
                }
            }
            // Cover paragraph separators and empty lines without inventing text.
            if !cell.text.is_empty() {
                let mut runs = vec![];
                let mut at = 0;
                for mut run in cell.runs {
                    if run.start > at {
                        run.start = at;
                    }
                    at = run.end;
                    runs.push(run);
                }
                if let Some(last) = runs.last_mut() {
                    last.end = cell.text.len();
                }
                cell.runs = runs;
            }
            if let Some(pr) = props {
                cell.style.fill = color(pr);
                cell.style.padding = pr
                    .attribute("marL")
                    .and_then(|s| s.parse::<f32>().ok())
                    .map(|v| v / 12700.);
                if let Some(line) = child(pr, A, "lnL") {
                    cell.style.border = color(line);
                    cell.style.border_width = line
                        .attribute("w")
                        .and_then(|s| s.parse::<f32>().ok())
                        .map(|v| v / 12700.);
                }
            }
            if continuation {
                cell.text.clear();
                cell.runs.clear();
            }
            let i = t.index(r, c)?;
            t.cells[i] = cell;
        }
    }
    let mut o = joydeck_model::identity::object("table");
    o.id = format!(
        "pptx-table-{page}-{}",
        n.descendants()
            .find(|n| n.has_tag_name((P, "cNvPr")))
            .and_then(|n| n.attribute("id"))
            .unwrap_or("0")
    );
    o.x = number(off, "x")?;
    o.y = number(off, "y")?;
    o.width = number(ext, "cx")?;
    o.height = number(ext, "cy")?;
    o.rotation = xf
        .attribute("rot")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.)
        / 60000.;
    o.table = Some(t);
    table::resize(&mut o);
    table::validate(&o)?;
    Ok(Some(o))
}
