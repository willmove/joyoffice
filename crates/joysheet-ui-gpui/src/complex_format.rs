//! Cell border and hatch primitives, clipped before expanding dash patterns.
use gpui_kit::*;
use joysheet_engine::model::{BorderStyle, PatternType};
pub fn checkbox(checked: bool, readonly: bool, rect: Bounds<Pixels>, scale: f32, window: &mut Window) {
    let side = (14. * scale).min(f32::from(rect.size.width) - 4.).min(f32::from(rect.size.height) - 2.).max(1.);
    let b = Bounds::new(
        rect.origin + point(px((f32::from(rect.size.width) - side) / 2.), px((f32::from(rect.size.height) - side) / 2.)),
        size(px(side), px(side)),
    );
    let ink = if readonly { 0x87938b } else { 0x147d59 };
    let mut q = gpui::fill(b, rgb(if checked { ink } else { 0xffffff }));
    q.border_color = rgb(ink).into();
    q.border_widths = Edges::all(px(1.));
    q.corner_radii = Corners::all(px(2.));
    window.paint_quad(q);
    if checked {
        crate::charts::path(
            &[[side * 0.2, side * 0.5], [side * 0.43, side * 0.75], [side * 0.82, side * 0.22]],
            false,
            None,
            Some(([255, 255, 255, 255], (side / 8.).max(1.))),
            b.origin,
            1.,
            window,
        );
    }
}

fn clipped(a: [f32; 2], b: [f32; 2], clip: Bounds<Pixels>) -> Option<(f32, f32)> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let mut lo = 0f32;
    let mut hi = 1f32;
    for (p, q) in [
        (-d[0], a[0] - f32::from(clip.left())),
        (d[0], f32::from(clip.right()) - a[0]),
        (-d[1], a[1] - f32::from(clip.top())),
        (d[1], f32::from(clip.bottom()) - a[1]),
    ] {
        if p.abs() < 0.0001 {
            if q < 0. {
                return None;
            }
        } else if p < 0. {
            lo = lo.max(q / p);
        } else {
            hi = hi.min(q / p);
        }
    }
    (lo <= hi).then_some((lo, hi))
}
pub fn dash_pattern(style: BorderStyle) -> &'static [f32] {
    match style {
        BorderStyle::Dotted => &[1., 2.],
        BorderStyle::Dashed | BorderStyle::MediumDashed => &[6., 3.],
        BorderStyle::DashDot | BorderStyle::MediumDashDot | BorderStyle::SlantDashDot => &[6., 3., 1., 3.],
        BorderStyle::DashDotDot | BorderStyle::MediumDashDotDot => &[6., 3., 1., 3., 1., 3.],
        _ => &[],
    }
}
pub fn border(a: [f32; 2], b: [f32; 2], style: BorderStyle, ink: u32, scale: f32, clip: Bounds<Pixels>, window: &mut Window) {
    if style == BorderStyle::None {
        return;
    }
    let Some((lo, hi)) = clipped(a, b, clip) else { return };
    let d = [b[0] - a[0], b[1] - a[1]];
    let len = d[0].hypot(d[1]);
    if len < 0.001 {
        return;
    }
    let color = [(ink >> 16) as u8, (ink >> 8) as u8, ink as u8, 255];
    let thickness = if style == BorderStyle::Hair { 0.5 } else { style.width() };
    let draw = |start: f32, end: f32, offset: f32, window: &mut Window| {
        let n = [-d[1] / len * offset, d[0] / len * offset];
        crate::charts::path(
            &[[a[0] + d[0] * start + n[0], a[1] + d[1] * start + n[1]], [a[0] + d[0] * end + n[0], a[1] + d[1] * end + n[1]]],
            false,
            None,
            Some((color, if style == BorderStyle::Double { scale } else { thickness * scale })),
            point(px(0.), px(0.)),
            1.,
            window,
        );
    };
    if style == BorderStyle::Double {
        draw(lo, hi, -scale, window);
        draw(lo, hi, scale, window);
        return;
    }
    let pattern = dash_pattern(style);
    if pattern.is_empty() {
        draw(lo, hi, 0., window);
        return;
    }
    let unit = scale.max(0.25);
    let cycle = pattern.iter().sum::<f32>() * unit;
    let first = (lo * len / cycle).floor() as i64;
    let last = (hi * len / cycle).ceil() as i64;
    for cycle_index in first..last.min(first + 8192) {
        let mut position = cycle_index as f32 * cycle;
        for (i, part) in pattern.iter().enumerate() {
            let end = position + part * unit;
            if i % 2 == 0 && end >= lo * len && position <= hi * len {
                draw((position / len).max(lo), (end / len).min(hi), 0., window);
            }
            position = end;
        }
    }
}
pub fn pattern(rect: Bounds<Pixels>, clip: Bounds<Pixels>, kind: PatternType, ink: u32, scale: f32, window: &mut Window) {
    if matches!(kind, PatternType::None | PatternType::Solid) {
        return;
    }
    let visible = rect.intersect(&clip);
    if visible.size.width <= px(0.) || visible.size.height <= px(0.) {
        return;
    }
    window.with_content_mask(Some(ContentMask { bounds: visible }), |window| {
        let x = f32::from(visible.left());
        let y = f32::from(visible.top());
        let w = f32::from(visible.size.width);
        let h = f32::from(visible.size.height);
        let gray = matches!(kind, PatternType::Gray50 | PatternType::Gray75 | PatternType::Gray25 | PatternType::Gray125 | PatternType::Gray0625);
        if gray {
            let step = match kind {
                PatternType::Gray75 | PatternType::Gray50 => 2.,
                PatternType::Gray25 => 3.,
                PatternType::Gray125 => 4.,
                _ => 6.,
            } * scale;
            let step = step.max(1.);
            let mut count = 0;
            for row in (y / step).floor() as i32..=((y + h) / step).ceil() as i32 {
                for col in (x / step).floor() as i32..=((x + w) / step).ceil() as i32 {
                    if count >= 32768 {
                        break;
                    }
                    if kind == PatternType::Gray75 || (row + col) % 2 == 0 {
                        window.paint_quad(gpui::fill(
                            Bounds::new(point(px(col as f32 * step), px(row as f32 * step)), size(px(scale.max(1.)), px(scale.max(1.)))),
                            rgb(ink),
                        ));
                    }
                    count += 1;
                }
            }
            return;
        }
        let dark = matches!(
            kind,
            PatternType::DarkHorizontal
                | PatternType::DarkVertical
                | PatternType::DarkDown
                | PatternType::DarkUp
                | PatternType::DarkGrid
                | PatternType::DarkTrellis
        );
        let step = (if dark { 5. } else { 9. } * scale).max(2.);
        let stroke = if dark { BorderStyle::Thin } else { BorderStyle::Hair };
        if matches!(kind, PatternType::DarkHorizontal | PatternType::LightHorizontal | PatternType::DarkGrid | PatternType::LightGrid) {
            for row in (y / step).floor() as i32..=((y + h) / step).ceil() as i32 {
                let yy = row as f32 * step;
                border([x, yy], [x + w, yy], stroke, ink, scale, visible, window);
            }
        }
        if matches!(kind, PatternType::DarkVertical | PatternType::LightVertical | PatternType::DarkGrid | PatternType::LightGrid) {
            for col in (x / step).floor() as i32..=((x + w) / step).ceil() as i32 {
                let xx = col as f32 * step;
                border([xx, y], [xx, y + h], stroke, ink, scale, visible, window);
            }
        }
        for up in [false, true] {
            if !matches!(kind, PatternType::DarkTrellis | PatternType::LightTrellis)
                && !(if up {
                    matches!(kind, PatternType::DarkUp | PatternType::LightUp)
                } else {
                    matches!(kind, PatternType::DarkDown | PatternType::LightDown)
                })
            {
                continue;
            }
            for n in -1..(((w + h) / step).ceil() as i32).min(8192) {
                let xx = x - h + n as f32 * step;
                border(if up { [xx, y + h] } else { [xx, y] }, if up { [xx + h, y] } else { [xx + h, y + h] }, stroke, ink, scale, visible, window);
            }
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn far_lines_are_clipped_before_expansion() {
        let clip = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.)));
        let (lo, hi) = clipped([-1_000_000., 50.], [1_000_000., 50.], clip).unwrap();
        assert!(lo >= 0.5 && hi - lo < 0.001);
        assert!(clipped([0., -10.], [100., -10.], clip).is_none());
        assert_eq!(dash_pattern(BorderStyle::DashDotDot), &[6., 3., 1., 3., 1., 3.]);
    }
}
