use joydeck_model::{
    document::{Deck, Object, Result},
    interop::escape,
};
fn text(x: f32, y: f32, size: f32, value: &str, anchor: &str) -> String {
    format!(
        r##"<text x="{x}" y="{y}" font-family="sans-serif" font-size="{size}" text-anchor="{anchor}" fill="#3b4453" stroke="none">{}</text>"##,
        escape(value)
    )
}
pub fn svg(deck: &Deck, page: usize, o: &Object) -> Result<String> {
    let c = o.chart.as_ref().ok_or("缺少图表数据")?;
    c.validate()?;
    let mut s = format!(
        r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#ffffff" stroke="none"/>"##,
        o.x, o.y, o.width, o.height
    );
    let sc = (o.width / 700.).min(o.height / 360.).clamp(0.1, 4.);
    let sz = 12. * sc;
    let (x, y, w, h) = (
        o.x + 64. * sc,
        o.y + 52. * sc,
        (o.width - 94. * sc).max(1.),
        (o.height - 110. * sc).max(1.),
    );
    let palette: Vec<_> = c
        .colors
        .iter()
        .map(|v| crate::table::theme_value(deck, page, v, false))
        .collect();
    s += &text(
        o.x + o.width / 2.,
        o.y + 26. * sc,
        20. * sc,
        &c.title,
        "middle",
    );
    if matches!(c.kind.as_str(), "pie" | "doughnut") {
        let total = c.series[0].values.iter().flatten().sum::<f64>();
        let r = (h.min(w) * 0.48).max(1.);
        let (cx, cy) = (x + w / 2., y + h / 2.);
        let mut a = -std::f64::consts::FRAC_PI_2;
        for (i, v) in c.series[0].values.iter().enumerate() {
            let v = v.unwrap_or(0.);
            if v <= 0. {
                continue;
            }
            let b = a + v / total * std::f64::consts::TAU;
            let (sx, sy) = (cx + r * a.cos() as f32, cy + r * a.sin() as f32);
            let (ex, ey) = (cx + r * b.cos() as f32, cy + r * b.sin() as f32);
            if c.kind == "doughnut" {
                let inner = r * 0.5;
                let (ix, iy) = (cx + inner * b.cos() as f32, cy + inner * b.sin() as f32);
                let (jx, jy) = (cx + inner * a.cos() as f32, cy + inner * a.sin() as f32);
                let path = if v >= total {
                    format!(
                        "M{} {cy} A{r} {r} 0 1 1 {} {cy} A{r} {r} 0 1 1 {} {cy} Z M{} {cy} A{inner} {inner} 0 1 0 {} {cy} A{inner} {inner} 0 1 0 {} {cy} Z",
                        cx - r,
                        cx + r,
                        cx - r,
                        cx - inner,
                        cx + inner,
                        cx - inner
                    )
                } else {
                    let large = u8::from(b - a > std::f64::consts::PI);
                    format!(
                        "M{sx} {sy} A{r} {r} 0 {large} 1 {ex} {ey} L{ix} {iy} A{inner} {inner} 0 {large} 0 {jx} {jy} Z"
                    )
                };
                s += &format!(
                    r#"<path d="{path}" fill="{}" fill-rule="evenodd" stroke="white"/>"#,
                    palette[i % palette.len()]
                );
            } else if v >= total {
                s += &format!(
                    r#"<circle cx="{cx}" cy="{cy}" r="{r}" fill="{}" stroke="white"/>"#,
                    palette[i % palette.len()]
                );
            } else {
                s += &format!(
                    r#"<path d="M{cx} {cy} L{sx} {sy} A{r} {r} 0 {} 1 {ex} {ey} Z" fill="{}" stroke="white"/>"#,
                    u8::from(b - a > std::f64::consts::PI),
                    palette[i % palette.len()]
                );
            }
            if c.labels {
                let m = (a + b) / 2.;
                s += &text(
                    cx + r * 0.7 * m.cos() as f32,
                    cy + r * 0.7 * m.sin() as f32,
                    sz,
                    &format!("{:.0}%", v / total * 100.),
                    "middle",
                );
            }
            a = b;
        }
    } else {
        let (lo, hi) = c.domain();
        let py = |v: f64| y + h - ((v - lo) / (hi - lo)) as f32 * h;
        let px = |v: f64| x + ((v - lo) / (hi - lo)) as f32 * w;
        let zero = 0f64.clamp(lo, hi);
        for k in 0..=4 {
            let v = lo + (hi - lo) * k as f64 / 4.;
            if c.grid {
                s += &format!(
                    r##"<path d="M{x} {} H{}" fill="none" stroke="#e5e7eb" stroke-width="0.8"/>"##,
                    y + h - h * k as f32 / 4.,
                    x + w
                );
            }
            if c.kind != "bar" {
                s += &text(
                    x - 8. * sc,
                    py(v) + 4. * sc,
                    sz,
                    &format!("{v:.0}{}", c.unit),
                    "end",
                );
            }
        }
        let n = c.categories.len();
        let xs: Vec<_> = c
            .series
            .iter()
            .flat_map(|s| {
                (0..n).filter_map(move |i| {
                    s.values[i]?;
                    if s.x.is_empty() {
                        Some((i + 1) as f64)
                    } else {
                        s.x[i]
                    }
                })
            })
            .collect();
        let (xmin, xmax) = if xs.is_empty() {
            (0., 1.)
        } else {
            xs.iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                    (a.min(*v), b.max(*v))
                })
        };
        let scatter_x = |v: f64| x + ((v - xmin) / (xmax - xmin).max(1.)) as f32 * w;
        let clipped = c
            .series
            .iter()
            .flat_map(|s| &s.values)
            .flatten()
            .any(|v| *v < lo || *v > hi);
        let clip = format!("chart-clip-{}", escape(&o.id));
        if clipped {
            s += &format!(
                r#"<defs><clipPath id="{clip}"><rect x="{x}" y="{y}" width="{w}" height="{h}"/></clipPath></defs><g clip-path="url(#{clip})">"#
            );
        }
        let m = c.series.len();
        for (j, ser) in c.series.iter().enumerate() {
            let col = &palette[j % palette.len()];
            let mut path = String::new();
            let mut gap = true;
            let mut last_x = x;
            for (i, v) in ser.values.iter().enumerate() {
                let Some(v) = v else {
                    if c.kind == "area" && !gap {
                        path += &format!("L{last_x} {} Z ", py(zero));
                    }
                    gap = true;
                    continue;
                };
                let cx = if c.kind == "scatter" {
                    let xv = if ser.x.is_empty() {
                        Some((i + 1) as f64)
                    } else {
                        ser.x[i]
                    };
                    let Some(xv) = xv else { continue };
                    scatter_x(xv)
                } else {
                    x + w * (i as f32 + 0.5) / n as f32
                };
                let cy = py(*v);
                match c.kind.as_str() {
                    "column" => {
                        let bw = w / n as f32 * 0.7 / m as f32;
                        let bx = x + w * (i as f32 + 0.15) / n as f32 + bw * j as f32;
                        s += &format!(
                            r#"<rect x="{bx}" y="{}" width="{bw}" height="{}" fill="{col}" stroke="none"/>"#,
                            cy.min(py(zero)),
                            (cy - py(zero)).abs()
                        );
                    }
                    "bar" => {
                        let bh = h / n as f32 * 0.7 / m as f32;
                        let by = y + h * (i as f32 + 0.15) / n as f32 + bh * j as f32;
                        s += &format!(
                            r#"<rect x="{}" y="{by}" width="{}" height="{bh}" fill="{col}" stroke="none"/>"#,
                            px(*v).min(px(zero)),
                            (px(*v) - px(zero)).abs()
                        );
                    }
                    "line" => {
                        path += &format!("{} {cx} {cy} ", if gap { "M" } else { "L" });
                        gap = false;
                        s += &format!(
                            r#"<circle cx="{cx}" cy="{cy}" r="{}" fill="{col}" stroke="none"/>"#,
                            2.5 * sc
                        );
                    }
                    "area" => {
                        if gap {
                            path += &format!("M{cx} {} L{cx} {cy} ", py(zero));
                        } else {
                            path += &format!("L{cx} {cy} ");
                        }
                        gap = false;
                        last_x = cx;
                    }
                    _ => {
                        let sx = cx;
                        s += &format!(
                            r#"<circle cx="{sx}" cy="{cy}" r="{}" fill="{col}" stroke="none"/>"#,
                            4. * sc
                        );
                    }
                }
                if c.labels && c.kind != "bar" {
                    s += &text(
                        cx,
                        (cy - 7. * sc).max(y + sz),
                        sz,
                        &format!("{v}{}", c.unit),
                        "middle",
                    );
                }
            }
            if c.kind == "area" && !gap {
                path += &format!("L{last_x} {} Z", py(zero));
            }
            if !path.is_empty() {
                s += &format!(
                    r#"<path d="{path}" fill="{}" stroke="{col}" stroke-width="{}"/>"#,
                    if c.kind == "area" {
                        col.as_str()
                    } else {
                        "none"
                    },
                    2. * sc
                );
            }
        }
        if clipped {
            s += "</g>";
        }
        for (i, cat) in c.categories.iter().enumerate().step_by((n / 8).max(1)) {
            if c.kind == "bar" {
                s += &text(
                    x - 8. * sc,
                    y + h * (i as f32 + 0.5) / n as f32,
                    sz,
                    cat,
                    "end",
                );
            } else if c.kind == "scatter" {
                if i < 5 {
                    let v = xmin + (xmax - xmin) * i as f64 / 4.;
                    s += &text(
                        scatter_x(v),
                        y + h + 20. * sc,
                        sz,
                        &format!("{v:.1}"),
                        "middle",
                    );
                }
            } else {
                s += &text(
                    x + w * (i as f32 + 0.5) / n as f32,
                    y + h + 20. * sc,
                    sz,
                    cat,
                    "middle",
                );
            }
        }
    }
    s += &text(
        x + w / 2.,
        o.y + o.height - 9. * sc,
        sz,
        &c.x_title,
        "middle",
    );
    s += &text(o.x + 4. * sc, y - 8. * sc, sz, &c.y_title, "start");
    if c.legend {
        let names: Vec<_> = if matches!(c.kind.as_str(), "pie" | "doughnut") {
            c.categories.iter().collect()
        } else {
            c.series.iter().map(|s| &s.name).collect()
        };
        for (i, name) in names.iter().take(8).enumerate() {
            let lx = x + i as f32 * w / names.len().min(8) as f32;
            s += &format!(
                r#"<rect x="{lx}" y="{}" width="{}" height="{}" fill="{}" stroke="none"/>"#,
                y + h + 35. * sc,
                10. * sc,
                10. * sc,
                palette[i % palette.len()]
            );
            s += &text(lx + 15. * sc, y + h + 44. * sc, sz, name, "start");
        }
    }
    Ok(s)
}
