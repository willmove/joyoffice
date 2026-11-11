//! MS-XLSX §2.1.7–8, §2.6.99, §2.6.110–116 and §2.6.139.
use crate::{
    write::{Out, Rels, XML_DECL},
    xml::{El, esc_attr},
};
use joysheet_model::{DrawingObject, Shape, ShapeFormat, TimelineLevel, Workbook};
use std::fmt::Write as _;
const NS: &str = "http://schemas.microsoft.com/office/spreadsheetml/2010/11/main";
const DRAW: &str = "http://schemas.microsoft.com/office/drawing/2012/timeslicer";
fn name(si: usize, id: u32) -> String {
    format!("JoyTimeline_{si}_{id}")
}
fn cache(wb: &Workbook, si: usize, id: u32) -> String {
    let base = format!("JoyTimelineCache_{si}_{id}");
    let mut name = base.clone();
    let mut suffix = 2;
    while wb.names.iter().any(|n| n.name.eq_ignore_ascii_case(&name)) {
        name = format!("{base}_{suffix}");
        suffix += 1;
    }
    name
}

/// Excel registers each Timeline cache in the workbook's defined-name scope.
pub fn defined_names(wb: &Workbook) -> String {
    let mut xml = String::new();
    for (si, sheet) in wb.sheets.iter().enumerate() {
        for shape in &sheet.shapes {
            if matches!(shape.format.object, Some(DrawingObject::Timeline { .. })) {
                let _ = write!(xml, "<definedName name=\"{}\">#N/A</definedName>", cache(wb, si, shape.id));
            }
        }
    }
    xml
}
fn level(l: TimelineLevel) -> u32 {
    match l {
        TimelineLevel::Years => 0,
        TimelineLevel::Quarters => 1,
        TimelineLevel::Months => 2,
        TimelineLevel::Days => 3,
    }
}
pub fn graphic(shape: &Shape, si: usize, id: usize) -> Option<String> {
    if !matches!(shape.format.object, Some(DrawingObject::Timeline { .. })) {
        return None;
    }
    Some(format!(
        "<mc:AlternateContent xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\"><mc:Choice xmlns:tsle=\"{DRAW}\" Requires=\"tsle\"><xdr:graphicFrame macro=\"\"><xdr:nvGraphicFramePr><xdr:cNvPr id=\"{id}\" name=\"{}\">{}</xdr:cNvPr><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr>{}<a:graphic><a:graphicData uri=\"{DRAW}\"><tsle:timeslicer name=\"{}\"/></a:graphicData></a:graphic></xdr:graphicFrame></mc:Choice><mc:Fallback><xdr:sp><xdr:nvSpPr><xdr:cNvPr id=\"0\" name=\"\"/><xdr:cNvSpPr/></xdr:nvSpPr><xdr:spPr><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></xdr:spPr><xdr:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Timeline — requires Excel 2013 or later</a:t></a:r></a:p></xdr:txBody></xdr:sp></mc:Fallback></mc:AlternateContent>",
        name(si, shape.id),
        crate::drawing_objects::metadata(shape),
        crate::drawing::xfrm(&shape.anchor, "xdr:xfrm"),
        name(si, shape.id)
    ))
}
pub fn write_sheet(wb: &Workbook, si: usize, out: &mut Out, rels: &mut Rels) -> String {
    let Some(sheet) = wb.sheet(si) else { return String::new() };
    let mut xml = String::new();
    for shape in &sheet.shapes {
        if let Some(DrawingObject::Timeline { column, level: l, scroll_start, .. }) = &shape.format.object {
            let _ = write!(
                xml,
                "<timeline name=\"{}\" cache=\"{}\" caption=\"{}\" level=\"{}\" selectionLevel=\"{}\"{}/>",
                name(si, shape.id),
                cache(wb, si, shape.id),
                esc_attr(column),
                level(*l),
                level(*l),
                scroll_start.map(|d| format!(" scrollPosition=\"{}\"", date(wb, d))).unwrap_or_default()
            );
        }
    }
    if xml.is_empty() {
        return xml;
    }
    out.part(
        &format!("xl/timelines/timeline{}.xml", si + 1),
        Some("application/vnd.ms-excel.timeline+xml"),
        format!("{XML_DECL}<timelines xmlns=\"{NS}\">{xml}</timelines>").into_bytes(),
    );
    let rid = rels.add_uri("http://schemas.microsoft.com/office/2011/relationships/timeline", &format!("../timelines/timeline{}.xml", si + 1));
    format!(
        "<ext uri=\"{{7E03D99C-DC04-49d9-9315-930204A7B6E9}}\" xmlns:x15=\"{NS}\"><x15:timelineRefs><x15:timelineRef r:id=\"{rid}\"/></x15:timelineRefs></ext>"
    )
}
fn date(wb: &Workbook, day: i32) -> String {
    let d = joysheet_core::date::datetime_from_serial(wb.date_system, day as f64).unwrap_or_default();
    format!("{:04}-{:02}-{:02}T00:00:00", d.year, d.month, d.day)
}
pub fn write_workbook(
    wb: &Workbook,
    pivots: &crate::pivot::PivotWriter,
    sheet_ids: &[u32],
    out: &mut Out,
    rels: &mut Rels,
) -> Result<String, crate::IoError> {
    let mut refs = String::new();
    for (si, sheet) in wb.sheets.iter().enumerate() {
        for shape in &sheet.shapes {
            let Some(DrawingObject::Timeline { pivot, column, selection, .. }) = &shape.format.object else { continue };
            let (ps, cache_id, min, max) = pivots
                .timeline_info(wb, pivot, column)
                .ok_or_else(|| crate::IoError::Format(format!("timeline's PivotTable/date field is missing: {pivot}!{column}")))?;
            let sel = selection.map(|(a, b)| format!("<selection startDate=\"{}\" endDate=\"{}\"/>", date(wb, a), date(wb, b))).unwrap_or_default();
            let filter = if selection.is_some() { "dateBetween" } else { "unknown" };
            let cname = cache(wb, si, shape.id);
            let file = format!("timelineCache{si}_{}.xml", shape.id);
            // Excel requires complete calendar years as the display bounds, even
            // when the source contains only a few months of that year.
            let first_year = joysheet_core::date::datetime_from_serial(wb.date_system, min as f64).unwrap_or_default().year;
            let last_year = joysheet_core::date::datetime_from_serial(wb.date_system, max as f64).unwrap_or_default().year;
            let lower = format!("{first_year:04}-01-01T00:00:00");
            let upper = format!("{:04}-01-01T00:00:00", last_year + 1);
            let xml = format!(
                "{XML_DECL}<timelineCacheDefinition xmlns=\"{NS}\" name=\"{cname}\" sourceName=\"{}\"><pivotTables><pivotTable tabId=\"{}\" name=\"{}\"/></pivotTables><state minimalRefreshVersion=\"6\" lastRefreshVersion=\"6\" pivotCacheId=\"{cache_id}\" filterType=\"{filter}\">{sel}<bounds startDate=\"{lower}\" endDate=\"{upper}\"/></state></timelineCacheDefinition>",
                esc_attr(column),
                sheet_ids.get(ps).copied().unwrap_or(ps as u32 + 1),
                esc_attr(pivot)
            );
            out.part(&format!("xl/timelineCaches/{file}"), Some("application/vnd.ms-excel.timelineCache+xml"), xml.into_bytes());
            let rid = rels.add_uri("http://schemas.microsoft.com/office/2011/relationships/timelineCache", &format!("timelineCaches/{file}"));
            let _ = write!(refs, "<x15:timelineCacheRef r:id=\"{rid}\"/>");
        }
    }
    Ok(if refs.is_empty() {
        refs
    } else {
        format!(
            "<ext uri=\"{{D0CA8CA8-9F24-4464-BF8E-62219DCF47F9}}\" xmlns:x15=\"{NS}\"><x15:timelineCacheRefs>{refs}</x15:timelineCacheRefs></ext>"
        )
    })
}
pub fn read_graphic(cx: &mut crate::read::Ctx<'_>, obj: &El, anchor: joysheet_model::Anchor) -> Option<Shape> {
    let n = obj.path(&["graphic", "graphicData", "timeslicer"])?.attr("name")?;
    let object = crate::drawing_objects::read_metadata(obj).unwrap_or(DrawingObject::Timeline {
        pivot: String::new(),
        column: String::new(),
        level: TimelineLevel::Months,
        selection: None,
        scroll_start: None,
    });
    Some(Shape {
        id: cx.next_object_id(),
        kind: joysheet_model::ShapeKind::TextBox,
        anchor,
        fill: joysheet_model::Color::Auto,
        line: joysheet_model::Color::Auto,
        text: n.into(),
        format: ShapeFormat { no_line: true, object: Some(object), ..Default::default() },
    })
}
pub fn read_sheet(cx: &mut crate::read::Ctx<'_>, part: &str, sheet: &mut joysheet_model::Sheet) -> Result<(), crate::IoError> {
    for rel in cx.pkg.rels(part)?.into_iter().filter(|r| r.kind.eq_ignore_ascii_case("timeline") && !r.external) {
        if let Some(root) = cx.optional_xml(&rel.target)? {
            for t in root.kids("timeline") {
                let Some(n) = t.attr("name") else { continue };
                let Some(shape) = sheet.shapes.iter_mut().find(|s| s.text == n) else { continue };
                if let Some(DrawingObject::Timeline { pivot, column, level, selection, scroll_start }) = &mut shape.format.object {
                    // Cache name is resolved after all worksheets and PivotTables have been read.
                    *pivot = t.attr("cache").unwrap_or("").into();
                    *column = t.attr("caption").unwrap_or("").into();
                    *level = match t.attr_u32("level") {
                        Some(0) => TimelineLevel::Years,
                        Some(1) => TimelineLevel::Quarters,
                        Some(3) => TimelineLevel::Days,
                        _ => TimelineLevel::Months,
                    };
                    *selection = None;
                    *scroll_start = t.attr("scrollPosition").and_then(|s| parse_date(cx.date_system, s));
                }
            }
        }
    }
    Ok(())
}
pub fn read_workbook(cx: &mut crate::read::Ctx<'_>, rels: &[crate::package::Rel], wb: &mut Workbook) -> Result<(), crate::IoError> {
    for rel in rels.iter().filter(|r| r.kind.eq_ignore_ascii_case("timelineCache") && !r.external) {
        let Some(root) = cx.optional_xml(&rel.target)? else { continue };
        let Some(n) = root.attr("name") else { continue };
        let pivot = root.path(&["pivotTables", "pivotTable"]).and_then(|p| p.attr("name")).unwrap_or("");
        let column = root.attr("sourceName").unwrap_or("");
        let parse = |value: Option<&str>| {
            let value = value?;
            let mut parts = value.split('T').next()?.split('-');
            let y = parts.next()?.parse().ok()?;
            let m = parts.next()?.parse().ok()?;
            let d = parts.next()?.parse().ok()?;
            joysheet_core::date::serial_from_ymd(wb.date_system, y, m, d).map(|v| v as i32)
        };
        let range = root.path(&["state", "selection"]);
        let selection = range.and_then(|r| Some((parse(r.attr("startDate"))?, parse(r.attr("endDate"))?)));
        let mut resolved = false;
        for sheet in &mut wb.sheets {
            for shape in &mut std::sync::Arc::make_mut(sheet).shapes {
                if let Some(DrawingObject::Timeline { pivot: p, column: c, selection: s, .. }) = &mut shape.format.object
                    && p == n
                {
                    *p = pivot.into();
                    *c = column.into();
                    *s = selection;
                    shape.text = column.into();
                    resolved = true;
                }
            }
        }
        if resolved {
            wb.names.retain(|defined| !(defined.scope.is_none() && defined.name.eq_ignore_ascii_case(n) && defined.formula.trim() == "#N/A"));
        }
    }
    Ok(())
}

fn parse_date(system: joysheet_core::DateSystem, text: &str) -> Option<i32> {
    let mut p = text.split('T').next()?.split('-');
    joysheet_core::date::serial_from_ymd(system, p.next()?.parse().ok()?, p.next()?.parse().ok()?, p.next()?.parse().ok()?).map(|v| v as i32)
}
