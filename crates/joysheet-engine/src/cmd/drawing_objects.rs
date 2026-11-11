//! Editable Insert objects. Mutations, including timeline/report changes, are atomic undo steps.
use super::*;
use joysheet_model::*;
use serde_json::json;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "insert.smartArt",
            "SmartArt",
            ["Insert", "Illustrations"],
            None,
            "{layout?: process|hierarchy|cycle|list, text?: newline-separated nodes, at?, width?, height?}",
            has_doc,
            |s, p| insert(s, p, "smartArt")
        ),
        cmd!(
            "insert.wordArt",
            "WordArt",
            ["Insert", "Text"],
            None,
            "{text?, warp?: textNoShape|textArchUp|textWave1, shadow?: bool, color?, at?, width?, height?}",
            has_doc,
            |s, p| insert(s, p, "wordArt")
        ),
        cmd!("insert.equation", "Equation", ["Insert", "Symbols"], None, "{text?: linear equation, at?, width?, height?}", has_doc, |s, p| insert(
            s, p, "equation"
        )),
        cmd!(
            "insert.object",
            "Object",
            ["Insert", "Text"],
            None,
            "{path? | base64?, name?, mime?, at?, width?, height?} embeds original bytes as an icon; no activation",
            has_doc,
            |s, p| insert(s, p, "embedded")
        ),
        cmd!(
            "insert.timeline",
            "Timeline",
            ["Insert", "Filters"],
            None,
            "{pivot?, column?: date field, level?: years|quarters|months|days, at?, width?, height?}",
            has_doc,
            |s, p| insert(s, p, "timeline")
        ),
        cmd!("object.edit", "Edit Object", [], None, "{id, text?, layout?, warp?, shadow?, color?}", has_doc, edit_object),
        cmd!(query "object.extract", "Save Embedded File", [], None, "{id, path} saves the original embedded file without launching it",has_doc,extract),
        cmd!(
            "timeline.filter",
            "Filter Timeline",
            [],
            None,
            "{id, start?: Excel serial day, end?: Excel serial day, clear?: bool}",
            has_doc,
            filter_timeline
        ),
        cmd!("timeline.level", "Timeline Level", [], None, "{id, level: years|quarters|months|days}", has_doc, level_timeline),
        cmd!("timeline.scroll", "Scroll Timeline", [], None, "{id, delta: periods}", has_doc, scroll_timeline),
    ]
}

fn level(p: &Json) -> Result<TimelineLevel> {
    Ok(match str_param(p, "level").unwrap_or("months") {
        "years" => TimelineLevel::Years,
        "quarters" => TimelineLevel::Quarters,
        "months" => TimelineLevel::Months,
        "days" => TimelineLevel::Days,
        _ => return Err(bad("timeline", "level must be years, quarters, months or days")),
    })
}
fn text(p: &Json, fallback: &str) -> Result<String> {
    let t = str_param(p, "text").unwrap_or(fallback);
    if t.chars().count() > 32767 {
        return Err(bad("object", "text exceeds 32767 characters"));
    }
    Ok(t.into())
}
fn content(s: &Session, p: &Json, kind: &str) -> Result<(DrawingObject, String)> {
    Ok(match kind {
        "smartArt" => {
            let layout = match str_param(p, "layout").unwrap_or("process") {
                "process" => SmartArtLayout::Process,
                "hierarchy" => SmartArtLayout::Hierarchy,
                "cycle" => SmartArtLayout::Cycle,
                "list" => SmartArtLayout::List,
                _ => return Err(bad("insert.smartArt", "choose process, hierarchy, cycle or list")),
            };
            let text = text(p, "步骤 1\n步骤 2\n步骤 3")?;
            let nodes: Vec<String> = text.lines().map(str::trim).filter(|x| !x.is_empty()).map(str::to_owned).collect();
            if nodes.is_empty() || nodes.len() > 64 || nodes.iter().any(|n| n.chars().count() > 1024) {
                return Err(bad("insert.smartArt", "provide 1–64 nodes, at most 1024 characters each"));
            }
            (DrawingObject::SmartArt { layout, nodes }, text)
        }
        "wordArt" => {
            let warp = str_param(p, "warp").unwrap_or("textNoShape");
            if !matches!(warp, "textNoShape" | "textArchUp" | "textWave1") {
                return Err(bad("insert.wordArt", "unsupported WordArt transform"));
            }
            (DrawingObject::WordArt { warp: warp.into(), shadow: bool_param(p, "shadow").unwrap_or(true) }, text(p, "艺术字")?)
        }
        "equation" => {
            let text = text(p, "x^{2}+y^{2}=z^{2}")?;
            let expression = MathExpr::parse(&text).map_err(|e| bad("insert.equation", e))?;
            (DrawingObject::Equation { expression }, text)
        }
        "embedded" => {
            let mime = str_param(p, "mime").unwrap_or("application/octet-stream");
            if mime.len() > 255 || !mime.is_ascii() || mime.chars().any(char::is_control) {
                return Err(bad("insert.object", "invalid media type"));
            }
            let data = if let Some(b) = str_param(p, "base64") {
                crate::io::base64_decode(b).ok_or_else(|| bad("insert.object", "invalid base64"))?
            } else if let Some(path) = str_param(p, "path").filter(|p| !p.is_empty()) {
                crate::io::read_file(path)?
            } else {
                return Err(bad("insert.object", "choose a file to embed"));
            };
            if data.len() > 32 * 1024 * 1024 {
                return Err(bad("insert.object", "embedded file exceeds 32 MiB"));
            }
            let name = str_param(p, "name").or_else(|| str_param(p, "path").and_then(|s| s.rsplit(['\\', '/']).next())).unwrap_or("Object.bin");
            if name.is_empty() || name.len() > 255 || name.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':')) {
                return Err(bad("insert.object", "invalid file name"));
            }
            (DrawingObject::Embedded { name: name.into(), mime: mime.into(), data: data.into() }, name.into())
        }
        "timeline" => {
            let wb = &s.doc()?.wb;
            let pivot = wb
                .sheets
                .iter()
                .flat_map(|sh| sh.pivots.iter())
                .find(|pt| str_param(p, "pivot").is_none_or(|n| n.is_empty() || pt.name.eq_ignore_ascii_case(n)))
                .ok_or_else(|| bad("insert.timeline", "select or create a PivotTable first"))?;
            let source = crate::pivot::read_source(wb, &pivot.source, wb.active_sheet()).map_err(|e| bad("insert.timeline", e))?;
            let ci =
                if let Some(c) = str_param(p, "column").filter(|c| !c.is_empty()) { source.col(c) } else { source.dates.iter().position(|d| *d) }
                    .ok_or_else(|| bad("insert.timeline", "choose a date field in the PivotTable source"))?;
            if !source.dates.get(ci).copied().unwrap_or(false) {
                return Err(bad("insert.timeline", "the selected column must contain dates"));
            }
            let column = source.headers[ci].clone();
            (
                DrawingObject::Timeline { pivot: pivot.name.clone(), column: column.clone(), level: level(p)?, selection: None, scroll_start: None },
                column,
            )
        }
        _ => return Err(bad("object", "unsupported object type")),
    })
}
fn insert(s: &mut Session, p: &Json, kind: &str) -> Result<Json> {
    let (object, text) = content(s, p, kind)?;
    let d = s.doc()?;
    let sheet = d.wb.active_sheet();
    if d.wb.active().is_some_and(|sh| sh.is_protected()) {
        return Err(EngineError::Other(PROTECTED.into()));
    }
    let id = d.wb.next_object_id();
    let at = cell_param(s, p, "at")?.unwrap_or(d.selection.active);
    let mut format = ShapeFormat { object: Some(object), no_line: true, ..Default::default() };
    if kind == "wordArt" {
        format.font_size = Some(36.);
        format.bold = true;
        format.text_color = Some(Color::Theme(4, 0));
        format.text_outline = Some((Color::Theme(4, -250), 0.75));
        format.align = Some(ShapeTextAlign::Center);
        format.anchor = Some(ShapeTextAnchor::Middle);
    }
    if let Some(c) = str_param(p, "color") {
        format.text_color = Some(Color::from_hex(c).ok_or_else(|| bad("object", "invalid color"))?);
    }
    let dim = |name, default| -> Result<f32> {
        let v = f64_param(p, name).unwrap_or(default);
        if !v.is_finite() || !(24. ..=10000.).contains(&v) {
            return Err(bad("object", "object dimensions must be 24–10000"));
        }
        Ok(v as f32)
    };
    let shape = Shape {
        id,
        kind: ShapeKind::TextBox,
        anchor: Anchor {
            cell: at,
            width: dim(
                "width",
                match kind {
                    "embedded" => 140.,
                    "timeline" => 350.,
                    _ => 420.,
                },
            )?,
            height: dim(
                "height",
                match kind {
                    "smartArt" => 180.,
                    "timeline" => 150.,
                    _ => 100.,
                },
            )?,
            mode: AnchorMode::MoveOnly,
            ..Default::default()
        },
        fill: Color::Auto,
        line: Color::Auto,
        text,
        format,
    };
    edit(s, |cx| {
        cx.sheet_mut(sheet)?.shapes.push(shape);
        Ok(json!({"shape":id,"type":kind}))
    })
}
pub(super) fn edit_object(s: &mut Session, p: &Json) -> Result<Json> {
    let id = u32_param(p, "id").ok_or_else(|| bad("object.edit", "missing id"))?;
    let d = s.doc()?;
    let si = d.wb.active_sheet();
    let sh = d.wb.active().ok_or(EngineError::NoDocument)?;
    if sh.is_protected() {
        return Err(EngineError::Other(PROTECTED.into()));
    }
    let old = sh.shapes.iter().find(|a| a.id == id).ok_or_else(|| bad("object.edit", "object not found"))?;
    if old.format.source.is_some() {
        return Err(EngineError::Disabled(
            "object.edit".into(),
            "This native layout contains unsupported properties; its original XML is retained read-only.".into(),
        ));
    }
    let mut p = p.clone();
    if p.get("text").is_none() {
        p["text"] = json!(old.text);
    }
    let kind = match old.format.object.as_ref() {
        Some(DrawingObject::SmartArt { layout, .. }) => {
            if p.get("layout").is_none() {
                p["layout"] = json!(match layout {
                    SmartArtLayout::Process => "process",
                    SmartArtLayout::Hierarchy => "hierarchy",
                    SmartArtLayout::Cycle => "cycle",
                    SmartArtLayout::List => "list",
                });
            }
            "smartArt"
        }
        Some(DrawingObject::WordArt { warp, shadow }) => {
            if p.get("warp").is_none() {
                p["warp"] = json!(warp);
            }
            if p.get("shadow").is_none() {
                p["shadow"] = json!(shadow);
            }
            "wordArt"
        }
        Some(DrawingObject::Equation { .. }) => "equation",
        _ => return Err(bad("object.edit", "use the object's dedicated properties or filter command")),
    };
    let (object, text) = content(s, &p, kind)?;
    let color = str_param(&p, "color").map(|c| Color::from_hex(c).ok_or_else(|| bad("object.edit", "invalid color"))).transpose()?;
    edit(s, |cx| {
        let shape = cx.sheet_mut(si)?.shapes.iter_mut().find(|a| a.id == id).ok_or_else(|| bad("object.edit", "object not found"))?;
        shape.text = text;
        shape.format.object = Some(object);
        if let Some(color) = color {
            shape.format.text_color = Some(color);
        }
        ok()
    })
}
fn extract(s: &mut Session, p: &Json) -> Result<Json> {
    let id = u32_param(p, "id").ok_or_else(|| bad("object.extract", "missing id"))?;
    let path = str_param(p, "path").filter(|p| !p.is_empty()).ok_or_else(|| bad("object.extract", "choose an output path"))?;
    let object = s.doc()?.wb.active().and_then(|sh| sh.shapes.iter().find(|sh| sh.id == id)).and_then(|sh| sh.format.object.as_ref());
    let Some(DrawingObject::Embedded { data, .. }) = object else {
        return Err(bad("object.extract", "select an embedded file"));
    };
    crate::io::write_file(path, data)?;
    Ok(json!({"path":path,"bytes":data.len()}))
}
fn level_timeline(s: &mut Session, p: &Json) -> Result<Json> {
    let level = level(p)?;
    let id = u32_param(p, "id").ok_or_else(|| bad("timeline.level", "missing id"))?;
    let si = s.doc()?.wb.active_sheet();
    if s.doc()?.wb.active().is_some_and(|s| s.is_protected()) {
        return Err(EngineError::Other(PROTECTED.into()));
    }
    edit(s, |cx| {
        let shape = cx.sheet_mut(si)?.shapes.iter_mut().find(|a| a.id == id).ok_or_else(|| bad("timeline.level", "timeline not found"))?;
        let Some(DrawingObject::Timeline { level: l, .. }) = &mut shape.format.object else {
            return Err(bad("timeline.level", "not a timeline"));
        };
        *l = level;
        ok()
    })
}
fn filter_timeline(s: &mut Session, p: &Json) -> Result<Json> {
    let id = u32_param(p, "id").ok_or_else(|| bad("timeline.filter", "missing id"))?;
    let d = s.doc()?;
    let si = d.wb.active_sheet();
    let object = d.wb.active().and_then(|sh| sh.shapes.iter().find(|a| a.id == id)).and_then(|a| a.format.object.as_ref());
    let Some(DrawingObject::Timeline { pivot, column, .. }) = object else {
        return Err(bad("timeline.filter", "timeline not found"));
    };
    let (ps, pi, pt) =
        d.wb.sheets
            .iter()
            .enumerate()
            .find_map(|(si, sh)| sh.pivots.iter().enumerate().find(|(_, pt)| pt.name == *pivot).map(|(i, pt)| (si, i, pt)))
            .ok_or_else(|| bad("timeline.filter", "PivotTable no longer exists"))?;
    if d.wb.sheet(si).is_some_and(|sh| sh.is_protected()) || d.wb.sheet(ps).is_some_and(|sh| sh.is_protected()) {
        return Err(EngineError::Other(PROTECTED.into()));
    }
    let source = crate::pivot::read_source(&d.wb, &pt.source, ps).map_err(|e| bad("timeline.filter", e))?;
    let ci = source.col(column).ok_or_else(|| bad("timeline.filter", "date field no longer exists"))?;
    let selection = if bool_param(p, "clear") == Some(true) {
        None
    } else {
        let start = date_parameter(&p["start"], d.wb.date_system)?;
        let end = date_parameter(&p["end"], d.wb.date_system)?;
        if start > end {
            return Err(bad("timeline.filter", "start must not follow end"));
        }
        Some((start, end))
    };
    let labels = selection.map(|(start, end)| {
        source
            .rows
            .iter()
            .filter_map(|r| r.get(ci))
            .filter(|v| v.as_f64().is_some_and(|n| n >= start as f64 && n < (end as f64 + 1.)))
            .map(|v| crate::pivot::item_label(&d.wb, v, PivotDateGroup::None, &source.formats[ci]))
            .collect::<Vec<_>>()
    });
    let column = column.clone();
    edit(s, |cx| {
        let pt = cx.sheet_mut(ps)?.pivots.get_mut(pi).ok_or_else(|| bad("timeline.filter", "PivotTable not found"))?;
        pt.filters.retain(|f| f.source_col != column);
        pt.filters.push(PivotFilter { source_col: column, selected: labels });
        cx.structural = true;
        crate::pivot::refresh(&mut cx.wb, ps, pi, false).map_err(|e| bad("timeline.filter", e))?;
        let shape = cx.sheet_mut(si)?.shapes.iter_mut().find(|a| a.id == id).ok_or_else(|| bad("timeline.filter", "timeline not found"))?;
        if let Some(DrawingObject::Timeline { selection: s, .. }) = &mut shape.format.object {
            *s = selection;
        }
        ok()
    })
}

fn date_parameter(v: &Json, system: joysheet_core::DateSystem) -> Result<i32> {
    if let Some(day) = v.as_i64().and_then(|n| i32::try_from(n).ok()).filter(|n| *n >= 0 && (*n as i64) <= joysheet_core::date::max_serial(system)) {
        return Ok(day);
    }
    if let Some(text) = v.as_str() {
        let p: Vec<_> = text.split('-').collect();
        if p.len() == 3
            && let (Ok(y), Ok(m), Ok(d)) = (p[0].parse::<i64>(), p[1].parse::<i64>(), p[2].parse::<i64>())
            && let Some(day) = joysheet_core::date::serial_from_ymd(system, y, m, d)
            && joysheet_core::date::datetime_from_serial(system, day)
                .is_some_and(|date| date.year as i64 == y && date.month as i64 == m && date.day as i64 == d)
        {
            return Ok(day as i32);
        }
    }
    Err(bad("timeline.filter", "enter a valid date as YYYY-MM-DD"))
}
fn scroll_timeline(s: &mut Session, p: &Json) -> Result<Json> {
    use joysheet_core::date::{datetime_from_serial, serial_from_ymd};
    let id = u32_param(p, "id").ok_or_else(|| bad("timeline.scroll", "missing id"))?;
    let d = s.doc()?;
    let si = d.wb.active_sheet();
    let sh = d.wb.active().ok_or(EngineError::NoDocument)?;
    if sh.is_protected() {
        return Err(EngineError::Other(PROTECTED.into()));
    }
    let mut object =
        sh.shapes.iter().find(|a| a.id == id).and_then(|s| s.format.object.clone()).ok_or_else(|| bad("timeline.scroll", "timeline not found"))?;
    let periods = crate::drawing_scene::timeline_periods(&d.wb, si, &object);
    let Some(&(first, _, _)) = periods.first() else { return ok() };
    let delta = p["delta"].as_i64().unwrap_or(6).clamp(-120, 120);
    let DrawingObject::Timeline { level, scroll_start, .. } = &mut object else {
        return Err(bad("timeline.scroll", "not a timeline"));
    };
    let date = datetime_from_serial(d.wb.date_system, first as f64).ok_or_else(|| bad("timeline.scroll", "invalid date"))?;
    let next = match level {
        TimelineLevel::Days => (first as i64 + delta).max(0) as i32,
        _ => {
            let months = match level {
                TimelineLevel::Years => delta * 12,
                TimelineLevel::Quarters => delta * 3,
                _ => delta,
            };
            serial_from_ymd(d.wb.date_system, date.year as i64, date.month as i64 + months, 1).unwrap_or(first as f64) as i32
        }
    };
    *scroll_start = Some(next);
    if crate::drawing_scene::timeline_periods(&d.wb, si, &object).is_empty() {
        return ok();
    }
    edit(s, |cx| {
        let shape = cx.sheet_mut(si)?.shapes.iter_mut().find(|s| s.id == id).ok_or_else(|| bad("timeline.scroll", "timeline not found"))?;
        shape.format.object = Some(object);
        ok()
    })
}
