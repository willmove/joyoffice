//! One structured-object scene for the desktop canvas and PDF output.
use joysheet_chart::{HAlign, Prim, VAlign};
use joysheet_model::{DrawingObject, MathExpr, Shape, SmartArtLayout, TimelineLevel, Workbook};
const BLUE: [u8; 4] = [68, 114, 196, 255];
const BLACK: [u8; 4] = [32, 32, 32, 255];
fn text(x: f32, y: f32, t: impl Into<String>, s: f32, c: [u8; 4]) -> Prim {
    Prim::Text { x, y, text: t.into(), size: s, color: c, bold: false, align: HAlign::Center, valign: VAlign::Middle, rotation: 0. }
}
fn left_text(x: f32, y: f32, t: impl Into<String>, s: f32, c: [u8; 4]) -> Prim {
    Prim::Text { x, y, text: t.into(), size: s, color: c, bold: false, align: HAlign::Left, valign: VAlign::Middle, rotation: 0. }
}
fn line(a: [f32; 2], b: [f32; 2], color: [u8; 4], width: f32) -> Prim {
    Prim::Line { pts: vec![a, b], color, width, dash: false }
}
fn rect(x: f32, y: f32, w: f32, h: f32, fill: [u8; 4]) -> Prim {
    Prim::Rect { x, y, w, h, fill: Some(fill), stroke: None, radius: 2. }
}

pub fn render(wb: &Workbook, si: usize, shape: &Shape) -> Option<Vec<Prim>> {
    let object = shape.format.object.as_ref()?;
    let (w, h) = (shape.anchor.width, shape.anchor.height);
    let accent = joysheet_model::Color::Theme(4, 0).resolve(&wb.theme).map(|[r, g, b]| [r, g, b, 255]).unwrap_or(BLUE);
    let mut out = Vec::new();
    match object {
        DrawingObject::SmartArt { layout, nodes } => {
            let n = nodes.len().max(1);
            let nf = n as f32;
            let positions: Vec<_> = (0..n)
                .map(|i| {
                    let i = i as f32;
                    match layout {
                        SmartArtLayout::Process => (i * w / nf + 4., h * 0.25, (w / nf - 16.).max(8.), h * 0.5),
                        SmartArtLayout::List => (8., i * h / nf + 4., w - 16., (h / nf - 8.).max(8.)),
                        SmartArtLayout::Hierarchy if i == 0. => (w * 0.35, 4., w * 0.3, h * 0.32),
                        SmartArtLayout::Hierarchy => ((i - 1.) * w / (nf - 1.) + 4., h * 0.62, (w / (nf - 1.) - 8.).max(8.), h * 0.32),
                        SmartArtLayout::Cycle => {
                            let a = i / nf * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                            (w * 0.5 + w * 0.32 * a.cos() - w * 0.14, h * 0.5 + h * 0.32 * a.sin() - h * 0.12, w * 0.28, h * 0.24)
                        }
                    }
                })
                .collect();
            for i in 1..n {
                let (a, b) = if *layout == SmartArtLayout::Hierarchy { (positions[0], positions[i]) } else { (positions[i - 1], positions[i]) };
                out.push(line([a.0 + a.2 / 2., a.1 + a.3 / 2.], [b.0 + b.2 / 2., b.1 + b.3 / 2.], accent, 1.5));
            }
            if *layout == SmartArtLayout::Cycle && n > 1 {
                let a = positions[n - 1];
                let b = positions[0];
                out.push(line([a.0 + a.2 / 2., a.1 + a.3 / 2.], [b.0 + b.2 / 2., b.1 + b.3 / 2.], accent, 1.5));
            }
            for (i, (x, y, bw, bh)) in positions.into_iter().enumerate() {
                out.push(rect(x, y, bw, bh, accent));
                if let Some(label) = nodes.get(i) {
                    out.push(text(x + bw / 2., y + bh / 2., label, 14f32.min(bh * 0.5), [255; 4]));
                }
            }
        }
        DrawingObject::Equation { expression } => {
            let font = shape.format.font_size.unwrap_or(22.).clamp(8., 96.);
            let (mw, mh) = math_size(expression, font);
            let k = ((w - 12.) / mw.max(1.)).min((h - 8.) / mh.max(1.)).min(1.);
            math_paint(expression, font * k, (w - mw * k) / 2., (h - mh * k) / 2., &mut out);
        }
        DrawingObject::WordArt { warp, shadow } => {
            let color = shape.format.text_color.and_then(|c| c.resolve(&wb.theme)).map(|[r, g, b]| [r, g, b, 255]).unwrap_or(accent);
            let count = shape.text.chars().count().max(1) as f32;
            let font = shape.format.font_size.unwrap_or(36.).min(w / (count * 0.62)).min(h * 0.7);
            let advance = font * 0.62;
            let left = (w - count * advance) / 2.;
            for (i, ch) in shape.text.chars().enumerate() {
                let p = (i as f32 + 0.5) / count;
                let x = left + (i as f32 + 0.5) * advance;
                let (y, angle) = match warp.as_str() {
                    "textArchUp" => (h * 0.65 - h * 0.3 * (std::f32::consts::PI * p).sin(), (p - 0.5) * 0.6),
                    "textWave1" => (h * 0.5 + h * 0.16 * (p * std::f32::consts::TAU).sin(), 0.),
                    _ => (h / 2., 0.),
                };
                if *shadow {
                    out.push(text(x + 2., y + 3., ch.to_string(), font, [185, 185, 185, 255]));
                }
                let mut prim = text(x, y, ch.to_string(), font, color);
                if let Prim::Text { rotation, bold, .. } = &mut prim {
                    *rotation = angle;
                    *bold = true;
                }
                out.push(prim);
            }
        }
        DrawingObject::Embedded { name, .. } => {
            out.push(Prim::Rect {
                x: w / 2. - 18.,
                y: 6.,
                w: 36.,
                h: (h - 52.).clamp(18., 44.),
                fill: Some([247, 249, 252, 255]),
                stroke: Some((BLUE, 1.)),
                radius: 1.,
            });
            out.push(line([w / 2. - 10., 18.], [w / 2. + 10., 18.], BLUE, 1.));
            out.push(line([w / 2. - 10., 25.], [w / 2. + 10., 25.], BLUE, 1.));
            out.push(text(w / 2., h - 13., name, 12., BLACK));
        }
        DrawingObject::Timeline { column, level, selection, .. } => {
            out.push(Prim::Rect { x: 0., y: 0., w, h, fill: Some([255; 4]), stroke: Some(([180, 188, 198, 255], 0.75)), radius: 1. });
            out.push(left_text(9., 16., column, 14., BLACK));
            out.push(text(w - 16., 16., "×", 12., BLACK));
            out.push(line([8., 28.], [w - 8., 28.], accent, 0.75));
            let selected_label = selection
                .and_then(|(a, b)| {
                    Some((
                        joysheet_core::date::datetime_from_serial(wb.date_system, a as f64)?,
                        joysheet_core::date::datetime_from_serial(wb.date_system, b as f64)?,
                    ))
                })
                .map(|(a, b)| format!("{}/{}/{} — {}/{}/{}", a.year, a.month, a.day, b.year, b.month, b.day))
                .unwrap_or_else(|| "所有时间".into());
            out.push(left_text(9., 42., selected_label, 12., BLACK));
            let periods = timeline_periods(wb, si, object);
            let n = periods.len().max(1) as f32;
            out.push(text(
                w - 36.,
                42.,
                match level {
                    TimelineLevel::Years => "年",
                    TimelineLevel::Quarters => "季度",
                    TimelineLevel::Months => "月",
                    TimelineLevel::Days => "日",
                },
                11.,
                BLACK,
            ));
            out.push(text(w - 12., 42., "▾", 10., BLACK));
            if let Some((a, _, _)) = periods.first()
                && let Some(d) = joysheet_core::date::datetime_from_serial(wb.date_system, *a as f64)
            {
                out.push(left_text(10., h - 83., d.year.to_string(), 11., [100, 100, 100, 255]));
            }
            for (i, (start, end, label)) in periods.iter().enumerate() {
                let x = 8. + i as f32 * (w - 16.) / n;
                let selected = selection.is_none_or(|(a, b)| *end >= a && *start <= b);
                out.push(rect(x, h - 52., (w - 16.) / n - 1., 18., if selected { accent } else { [221, 230, 244, 255] }));
                let caption = match level {
                    TimelineLevel::Months => format!("{}月", label.rsplit('/').next().unwrap_or(label)),
                    _ => label.clone(),
                };
                out.push(text(x + (w - 16.) / n / 2., h - 64., caption, 10., BLACK));
            }
            out.push(text(18., h - 12., "◀", 11., BLACK));
            out.push(text(w - 18., h - 12., "▶", 11., BLACK));
            out.push(rect(34., h - 15., w - 68., 5., [220, 225, 230, 255]));
        }
    }
    Some(out)
}

fn math_size(e: &MathExpr, s: f32) -> (f32, f32) {
    match e {
        MathExpr::Text(t) => (t.chars().count() as f32 * s * 0.6, s * 1.2),
        MathExpr::Row(v) => v.iter().map(|e| math_size(e, s)).fold((0., s * 1.2), |(w, h), (ew, eh)| (w + ew, h.max(eh))),
        MathExpr::Fraction(a, b) => {
            let aw = math_size(a, s * 0.85);
            let bw = math_size(b, s * 0.85);
            (aw.0.max(bw.0) + 8., aw.1 + bw.1 + 6.)
        }
        MathExpr::Radical(a) => {
            let (w, h) = math_size(a, s);
            (w + s * 0.7, h + 4.)
        }
        MathExpr::Superscript(a, b) | MathExpr::Subscript(a, b) => {
            let a = math_size(a, s);
            let b = math_size(b, s * 0.65);
            (a.0 + b.0, a.1 + b.1 * 0.55)
        }
    }
}
fn math_paint(e: &MathExpr, s: f32, x: f32, y: f32, out: &mut Vec<Prim>) {
    let (w, h) = math_size(e, s);
    match e {
        MathExpr::Text(t) => out.push(text(x + w / 2., y + h / 2., t, s, BLACK)),
        MathExpr::Row(v) => {
            let mut x = x;
            for e in v {
                let (ew, eh) = math_size(e, s);
                math_paint(e, s, x, y + (h - eh) / 2., out);
                x += ew;
            }
        }
        MathExpr::Fraction(a, b) => {
            let aw = math_size(a, s * 0.85);
            let bw = math_size(b, s * 0.85);
            math_paint(a, s * 0.85, x + (w - aw.0) / 2., y, out);
            out.push(line([x, y + aw.1 + 3.], [x + w, y + aw.1 + 3.], BLACK, 1.));
            math_paint(b, s * 0.85, x + (w - bw.0) / 2., y + aw.1 + 6., out);
        }
        MathExpr::Radical(a) => {
            out.push(text(x + s * 0.3, y + h / 2., "√", s * 1.1, BLACK));
            out.push(line([x + s * 0.65, y + 1.], [x + w, y + 1.], BLACK, 0.8));
            math_paint(a, s, x + s * 0.7, y + 4., out);
        }
        MathExpr::Superscript(a, b) => {
            let aw = math_size(a, s);
            let bh = math_size(b, s * 0.65).1;
            math_paint(a, s, x, y + bh * 0.55, out);
            math_paint(b, s * 0.65, x + aw.0, y, out);
        }
        MathExpr::Subscript(a, b) => {
            let aw = math_size(a, s);
            math_paint(a, s, x, y, out);
            math_paint(b, s * 0.65, x + aw.0, y + aw.1 * 0.65, out);
        }
    }
}

/// Visible period tiles, shared by paint and pointer hit testing. Includes both endpoint days.
pub fn timeline_periods(wb: &Workbook, si: usize, object: &DrawingObject) -> Vec<(i32, i32, String)> {
    use joysheet_core::date::{datetime_from_serial, serial_from_ymd};
    let DrawingObject::Timeline { pivot, column, level, scroll_start, .. } = object else { return vec![] };
    let Some(pt) = wb.sheets.iter().flat_map(|s| s.pivots.iter()).find(|p| p.name == *pivot) else { return vec![] };
    let Ok(src) = crate::pivot::read_source(wb, &pt.source, si) else { return vec![] };
    let Some(ci) = src.col(column) else { return vec![] };
    let bounds = src
        .rows
        .iter()
        .filter_map(|r| r.get(ci)?.as_f64())
        .filter(|v| v.is_finite())
        .map(|v| v.floor() as i32)
        .fold(None, |bounds: Option<(i32, i32)>, day| Some(bounds.map_or((day, day), |(lo, hi)| (lo.min(day), hi.max(day)))));
    let Some((min, max)) = bounds else { return vec![] };
    // Excel may persist a viewport outside the populated date range. Keep our
    // bounded local-data viewport usable so its arrows can scroll back to data.
    let mut day = scroll_start.unwrap_or(min).clamp(min, max);
    let mut out = Vec::new();
    while day <= max && out.len() < 12 {
        let Some(d) = datetime_from_serial(wb.date_system, day as f64) else { break };
        let (year, month, step, label) = match level {
            TimelineLevel::Years => (d.year, 1, 12, d.year.to_string()),
            TimelineLevel::Quarters => (d.year, (d.month - 1) / 3 * 3 + 1, 3, format!("{} Q{}", d.year, (d.month - 1) / 3 + 1)),
            TimelineLevel::Months => (d.year, d.month, 1, format!("{}/{}", d.year, d.month)),
            TimelineLevel::Days => {
                out.push((day, day, format!("{}/{}", d.month, d.day)));
                day += 1;
                continue;
            }
        };
        let start = serial_from_ymd(wb.date_system, year as i64, month as i64, 1).unwrap_or(day as f64) as i32;
        let next = serial_from_ymd(wb.date_system, year as i64, (month + step) as i64, 1).unwrap_or((day + 366) as f64) as i32;
        if next <= day {
            break;
        }
        out.push((start, next - 1, label));
        day = next;
    }
    out
}
