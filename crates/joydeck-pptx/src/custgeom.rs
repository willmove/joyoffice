//! DrawingML custom geometry (`a:custGeom`): guide formula evaluation and conversion of
//! `a:pathLst` to our `M L C Q Z` path syntax, and back.
//!
//! Formulas follow the ECMA-376 shape guide semantics (§20.1.9.11): angles are in 1/60 000 degree,
//! and every operand is a guide name, a built-in variable or an integer literal.

use joyoffice_drawingml::guide::{Guides, finite};
use std::fmt::Write as _;

use joydeck_geom::preset::FillMode;
use joydeck_model::CustomPath;

use crate::xml::{A, El, W};

/// Rule order is significant. A later guide can depend on or replace an earlier value.
fn read_guides(values: &mut Guides, list: Option<&El>) {
    if let Some(list) = list {
        for gd in list.children_named("gd").take(5000) {
            if let (Some(name), Some(formula)) = (gd.attr("name"), gd.attr("fmla")) {
                values.set(name, values.eval(formula));
            }
        }
    }
}

fn num(s: &mut String, v: f64) {
    let v = finite(v);
    let r = (v * 1000.0).round() / 1000.0;
    if r == r.trunc() {
        let _ = write!(s, "{}", r as i64);
    } else {
        let _ = write!(s, "{r}");
    }
}

fn points<const N: usize>(guides: &Guides, element: &El) -> Option<[(f64, f64); N]> {
    let mut points = element.children_named("pt");
    let mut result = [(0.0, 0.0); N];
    for point in &mut result {
        let node = points.next()?;
        *point = (
            guides.get(node.attr("x").unwrap_or("0")),
            guides.get(node.attr("y").unwrap_or("0")),
        );
    }
    points.next().is_none().then_some(result)
}

fn path_command(path: &mut String, command: char, points: &[(f64, f64)]) {
    path.push(command);
    path.push(' ');
    for &(x, y) in points {
        num(path, x);
        path.push(' ');
        num(path, y);
        path.push(' ');
    }
}

/// Convert `a:custGeom` to custom paths. `w`/`h` is the shape size in EMU (used for guides and for
/// paths that don't give their own coordinate space).
pub fn read_cust_geom(cg: &El, w_emu: f64, h_emu: f64) -> Vec<CustomPath> {
    let mut out = vec![];
    let Some(pl) = cg.child("pathLst") else {
        return out;
    };
    let mut previous = None;
    for p in pl.children_named("path").take(1000) {
        let pw = p.f64("w").unwrap_or(0.0);
        let ph = p.f64("h").unwrap_or(0.0);
        // Guides are evaluated in the path's own space when it has one.
        let (gw, gh) = if pw > 0.0 || ph > 0.0 {
            (pw, ph)
        } else {
            (w_emu, h_emu)
        };
        let g = match &mut previous {
            Some(((w, h), guides)) if *w == gw && *h == gh => guides,
            _ => {
                let mut guides = Guides::new(gw, gh);
                read_guides(&mut guides, cg.child("avLst"));
                read_guides(&mut guides, cg.child("gdLst"));
                &mut previous.insert(((gw, gh), guides)).1
            }
        };
        let mut d = String::new();
        let mut cur = (0.0, 0.0);
        let mut start = (0.0, 0.0);
        for c in p.elements().take(100_000) {
            match c.local() {
                "moveTo" if let Some([point]) = points(g, c) => {
                    cur = point;
                    start = point;
                    path_command(&mut d, 'M', &[point]);
                }
                "lnTo" if let Some([point]) = points(g, c) => {
                    cur = point;
                    path_command(&mut d, 'L', &[point]);
                }
                "cubicBezTo" if let Some([first, second, end]) = points(g, c) => {
                    cur = end;
                    path_command(&mut d, 'C', &[first, second, end]);
                }
                "quadBezTo" if let Some([control, end]) = points(g, c) => {
                    cur = end;
                    path_command(&mut d, 'Q', &[control, end]);
                }
                "arcTo" => {
                    let wr = g.get(c.attr("wR").unwrap_or("0"));
                    let hr = g.get(c.attr("hR").unwrap_or("0"));
                    let st = g.get(c.attr("stAng").unwrap_or("0"));
                    let sw = g.get(c.attr("swAng").unwrap_or("0"));
                    cur = arc_to(&mut d, cur, wr, hr, st, sw);
                }
                "close" => {
                    d.push_str("Z ");
                    cur = start;
                }
                _ => {}
            }
        }
        let fill = match p.attr("fill").unwrap_or("norm") {
            "none" => FillMode::None,
            "lighten" => FillMode::Lighten,
            "lightenLess" => FillMode::LightenLess,
            "darken" => FillMode::Darken,
            "darkenLess" => FillMode::DarkenLess,
            _ => FillMode::Norm,
        };
        let (ow, oh) = if pw > 0.0 || ph > 0.0 {
            (pw, ph)
        } else {
            (w_emu.max(1.0), h_emu.max(1.0))
        };
        out.push(CustomPath {
            w: ow,
            h: oh,
            d: d.trim_end().to_string(),
            fill,
            stroke: p.bool("stroke").unwrap_or(true),
        });
    }
    out
}

/// Write the shared elliptical arc in the editable model's path syntax.
fn arc_to(
    d: &mut String,
    start: (f64, f64),
    wr: f64,
    hr: f64,
    angle: f64,
    sweep: f64,
) -> (f64, f64) {
    let end =
        joyoffice_drawingml::arc::cubics([start.0, start.1], [wr, hr], [angle, sweep], |segment| {
            d.push_str("C ");
            for point in segment {
                for coordinate in point {
                    num(d, coordinate);
                    d.push(' ');
                }
            }
        });
    (end[0], end[1])
}

/// Parse our path syntax into commands.
pub fn parse_path(d: &str) -> Vec<(char, Vec<f64>)> {
    let mut out = vec![];
    let mut cmd: Option<char> = None;
    let mut nums: Vec<f64> = vec![];
    let flush = |cmd: Option<char>, nums: &mut Vec<f64>, out: &mut Vec<(char, Vec<f64>)>| {
        if let Some(c) = cmd {
            let n = match c {
                'M' | 'L' => 2,
                'C' => 6,
                'Q' => 4,
                _ => 0,
            };
            if n == 0 {
                out.push((c, vec![]));
            } else {
                for (i, chunk) in nums.chunks(n).enumerate() {
                    if chunk.len() == n && out.len() < 1_000_000 {
                        // Repeated coordinates after M continue as L.
                        let c2 = if c == 'M' && i > 0 { 'L' } else { c };
                        out.push((c2, chunk.to_vec()));
                    }
                }
            }
        }
        nums.clear();
    };
    let mut token = String::new();
    let push_tok = |token: &mut String, nums: &mut Vec<f64>| {
        if !token.is_empty() {
            if let Ok(v) = token.parse::<f64>()
                && v.is_finite()
            {
                nums.push(v);
            }
            token.clear();
        }
    };
    let cs: Vec<char> = d.chars().collect();
    for (i, &ch) in cs.iter().enumerate() {
        match ch {
            // An 'E' inside a number followed by a digit (or a sign and a digit) is an exponent,
            // not the end command.
            'E' if !token.is_empty() && {
                let n1 = cs.get(i + 1).copied().unwrap_or(' ');
                let n2 = cs.get(i + 2).copied().unwrap_or(' ');
                n1.is_ascii_digit() || ((n1 == '-' || n1 == '+') && n2.is_ascii_digit())
            } =>
            {
                token.push(ch)
            }
            'M' | 'L' | 'C' | 'Q' | 'Z' | 'm' | 'l' | 'c' | 'q' | 'z' | 'E' => {
                push_tok(&mut token, &mut nums);
                flush(cmd, &mut nums, &mut out);
                cmd = Some(ch.to_ascii_uppercase());
            }
            ' ' | ',' | '\t' | '\n' => push_tok(&mut token, &mut nums),
            '-' if !token.is_empty() && !token.ends_with(['e', 'E']) => {
                push_tok(&mut token, &mut nums);
                token.push(ch);
            }
            c => token.push(c),
        }
    }
    push_tok(&mut token, &mut nums);
    flush(cmd, &mut nums, &mut out);
    out
}

/// Write custom paths as `a:custGeom`. Coordinates are integers in the file, so small path spaces
/// are scaled up.
pub fn write_cust_geom(w: &mut W, paths: &[CustomPath]) {
    w.open0("a:custGeom");
    w.empty0("a:avLst");
    w.empty0("a:gdLst");
    w.empty0("a:ahLst");
    w.empty0("a:cxnLst");
    w.empty(
        "a:rect",
        A::new().a("l", "l").a("t", "t").a("r", "r").a("b", "b"),
    );
    w.open0("a:pathLst");
    for p in paths {
        let (pw, ph) = (finite(p.w).abs(), finite(p.h).abs());
        let m = pw.max(ph);
        let k = if m > 0.0 && m < 20_000.0 {
            100_000.0 / m
        } else {
            1.0
        };
        let r = |v: f64| (finite(v) * k).round().clamp(-2.0e9, 2.0e9) as i64;
        let fill = match p.fill {
            FillMode::Norm => None,
            FillMode::None => Some("none"),
            FillMode::Lighten => Some("lighten"),
            FillMode::LightenLess => Some("lightenLess"),
            FillMode::Darken => Some("darken"),
            FillMode::DarkenLess => Some("darkenLess"),
        };
        let a = A::new()
            .a("w", r(pw).max(0))
            .a("h", r(ph).max(0))
            .o("fill", fill)
            .o("stroke", (!p.stroke).then_some("0"));
        w.open("a:path", a);
        for (c, v) in parse_path(&p.d) {
            let pts: Vec<(i64, i64)> = v
                .chunks(2)
                .filter_map(|c| match c {
                    [x, y] => Some((r(*x), r(*y))),
                    _ => None,
                })
                .collect();
            let tag = match c {
                'M' => "a:moveTo",
                'L' => "a:lnTo",
                'C' => "a:cubicBezTo",
                'Q' => "a:quadBezTo",
                'Z' => {
                    w.empty0("a:close");
                    continue;
                }
                _ => continue,
            };
            w.open0(tag);
            for (x, y) in pts {
                w.empty("a:pt", A::new().a("x", x).a("y", y));
            }
            w.close(tag);
        }
        w.close("a:path");
    }
    w.close("a:pathLst");
    w.close("a:custGeom");
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test assertions"
)]
mod tests {
    use super::*;
    use crate::xml::parse;

    #[test]
    fn formulas() {
        let mut g = Guides::new(1000.0, 500.0);
        assert_eq!(g.eval("*/ w 1 2"), 500.0);
        assert_eq!(g.eval("+- w h 100"), 1400.0);
        assert_eq!(g.eval("+/ w h 3"), 500.0);
        assert_eq!(g.eval("?: -1 5 7"), 7.0);
        assert_eq!(g.eval("?: 1 5 7"), 5.0);
        assert_eq!(g.eval("abs -42"), 42.0);
        assert_eq!(g.eval("max 3 9"), 9.0);
        assert_eq!(g.eval("min 3 9"), 3.0);
        assert_eq!(g.eval("mod 3 4 0"), 5.0);
        assert_eq!(g.eval("pin 0 150 100"), 100.0);
        assert_eq!(g.eval("pin 0 -5 100"), 0.0);
        assert_eq!(g.eval("sqrt 16"), 4.0);
        assert_eq!(g.eval("val ss"), 500.0);
        assert!((g.eval("at2 1 1") - 2_700_000.0).abs() < 1e-6);
        assert!((g.eval("cos 100 5400000")).abs() < 1e-9);
        assert!((g.eval("sin 100 5400000") - 100.0).abs() < 1e-9);
        assert!((g.eval("tan 100 2700000") - 100.0).abs() < 1e-6);
        assert!((g.eval("cat2 10 1 0") - 10.0).abs() < 1e-9);
        assert!((g.eval("sat2 10 0 1") - 10.0).abs() < 1e-9);
        assert_eq!(g.eval("*/ 1 1 0"), 0.0, "division by zero is 0");
        assert_eq!(g.eval("bogus 1 2"), 0.0);
        assert_eq!(g.eval(""), 0.0);
        g.set("x1", 7.0);
        assert_eq!(g.eval("+- x1 0 2"), 5.0);
        assert_eq!(g.get("hc"), 500.0);
        assert_eq!(g.get("cd4"), 5_400_000.0);
    }

    #[test]
    fn custgeom_with_guides_and_arc() {
        let x = br#"<a:custGeom xmlns:a="x"><a:avLst/><a:gdLst><a:gd name="mid" fmla="*/ w 1 2"/></a:gdLst>
            <a:pathLst><a:path w="200" h="100"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="mid" y="h"/></a:lnTo>
            <a:arcTo wR="50" hR="50" stAng="0" swAng="5400000"/><a:close/></a:path>
            <a:path w="10" h="10" fill="none" stroke="0"><a:moveTo><a:pt x="1" y="1"/></a:moveTo><a:quadBezTo><a:pt x="2" y="2"/><a:pt x="3" y="1"/></a:quadBezTo></a:path></a:pathLst></a:custGeom>"#;
        let d = parse(x).unwrap();
        let paths = read_cust_geom(&d.root, 1000.0, 1000.0);
        assert_eq!(paths.len(), 2);
        assert!(
            paths[0].d.starts_with("M 0 0 L 100 100 C"),
            "{}",
            paths[0].d
        );
        assert!(paths[0].d.ends_with('Z'));
        // The quarter arc from (100,100) with centre (50,100) ends at (50,150).
        assert!(paths[0].d.contains("50 150"), "{}", paths[0].d);
        assert_eq!(paths[1].fill, FillMode::None);
        assert!(!paths[1].stroke);
        assert_eq!(paths[1].d, "M 1 1 Q 2 2 3 1");
    }

    #[test]
    fn write_then_read_path() {
        let p = CustomPath {
            w: 100.0,
            h: 50.0,
            d: "M 0 0 L 100 0 C 100 10 90 50 50 50 Q 0 50 0 0 Z".into(),
            fill: FillMode::Norm,
            stroke: true,
        };
        let mut w = W::frag();
        write_cust_geom(&mut w, std::slice::from_ref(&p));
        let d = parse(w.s.as_bytes()).unwrap();
        let back = read_cust_geom(&d.root, 1.0, 1.0);
        assert_eq!(back.len(), 1);
        // Scaled by 1000 (100 → 100000).
        assert_eq!(back[0].w, 100_000.0);
        assert_eq!(
            back[0].d,
            "M 0 0 L 100000 0 C 100000 10000 90000 50000 50000 50000 Q 0 50000 0 0 Z"
        );
    }

    #[test]
    fn parse_path_uppercase_exponent() {
        assert_eq!(
            parse_path("M 1E-5 2 L 3 4"),
            vec![('M', vec![1e-5, 2.0]), ('L', vec![3.0, 4.0])]
        );
        assert_eq!(parse_path("M 1.5E3 2"), vec![('M', vec![1500.0, 2.0])]);
        assert_eq!(parse_path("M 1e-5 2"), vec![('M', vec![1e-5, 2.0])]);
        // An 'E' right after a number with no exponent digits is the end command.
        assert_eq!(
            parse_path("M 0 0E"),
            vec![('M', vec![0.0, 0.0]), ('E', vec![])]
        );
    }

    #[test]
    fn parse_path_tokens() {
        let v = parse_path("M0,0 L10-5 Z");
        assert_eq!(v.len(), 3);
        assert_eq!(v[1], ('L', vec![10.0, -5.0]));
        assert!(parse_path("garbage ### C 1").is_empty() || parse_path("garbage").is_empty());
    }
}
