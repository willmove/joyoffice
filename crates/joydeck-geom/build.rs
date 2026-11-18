//! Compile the unmodified ECMA-376 preset definitions into bounded, read-only
//! instruction tables. A name is resolved at its point of use because several
//! standard shapes assign a guide more than once.
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fmt::Write;
use std::fs;
use std::path::PathBuf;

use joyoffice_drawingml::guide::{Builtin, Operation};
use roxmltree::Node;

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const SOURCE: &str = "data/presetShapeDefinitions.xml";
type Names<'a> = HashMap<&'a str, usize>;

fn children<'a, 'd>(node: Node<'a, 'd>, name: &'a str) -> impl Iterator<Item = Node<'a, 'd>> {
    node.children()
        .filter(move |n| n.is_element() && n.tag_name().name() == name)
}

fn list<'a, 'd>(
    node: Node<'a, 'd>,
    list: &'a str,
    tag: &'a str,
) -> impl Iterator<Item = Node<'a, 'd>> {
    children(node, list).flat_map(move |n| children(n, tag))
}

fn attr<'a>(n: Node<'a, '_>, key: &str) -> Result<&'a str> {
    n.attribute(key)
        .ok_or_else(|| format!("{} is missing {key}", n.tag_name().name()).into())
}

fn value(token: &str, defs: &Names) -> Result<String> {
    if let Some(i) = defs.get(token) {
        return Ok(format!("V::Slot({i})"));
    }
    if let Ok(n) = token.parse::<i64>() {
        return Ok(format!("V::Number({n}_f64)"));
    }
    let builtin =
        Builtin::parse(token).ok_or_else(|| format!("Unbound DrawingML guide {token}"))?;
    let v = format!("V::Builtin(Builtin::{builtin:?})");
    Ok(v)
}

fn adjustment(name: &str) -> Option<usize> {
    if name == "adj" {
        return Some(0);
    }
    name.strip_prefix("adj")?
        .parse::<usize>()
        .ok()?
        .checked_sub(1)
}

fn operand_array(n: Node<'_, '_>, keys: &[&str], defs: &Names) -> Result<String> {
    let parts = keys
        .iter()
        .map(|key| value(attr(n, key)?, defs))
        .collect::<Result<Vec<_>>>()?;
    Ok(format!("[{}]", parts.join(",")))
}

fn program(shape: Node<'_, '_>, output: &mut String) -> Result<()> {
    let mut defs = Names::new();
    let name = shape.tag_name().name();
    writeln!(
        output,
        "{name:?} => Some(&Program {{ name: {name:?}, rules: &["
    )?;
    for (slot, (is_adjust, guide)) in list(shape, "avLst", "gd")
        .map(|g| (true, g))
        .chain(list(shape, "gdLst", "gd").map(|g| (false, g)))
        .enumerate()
    {
        let name = attr(guide, "name")?;
        let mut words = attr(guide, "fmla")?.split_ascii_whitespace();
        let op = words
            .next()
            .and_then(Operation::parse)
            .ok_or("Unknown DrawingML guide operation")?;
        // Eight add/subtract guides in the published circular-arrow definitions carry
        // a fourth literal zero. The operator consumes exactly three operands.
        let mut raw = words.collect::<Vec<_>>();
        if op == Operation::Sum && raw.len() == 4 && raw[3] == "0" {
            raw.pop();
        }
        let operands = raw
            .into_iter()
            .map(|t| value(t, &defs))
            .collect::<Result<Vec<_>>>()?;
        if operands.len() > 3 {
            return Err("Too many guide operands".into());
        }
        let mut operands = operands;
        operands.resize(3, "V::Number(0.)".into());
        let index = if is_adjust { adjustment(name) } else { None };
        if index.is_some_and(|i| i >= 8) {
            return Err("Too many preset adjustments".into());
        }
        writeln!(
            output,
            "Rule(Op::{op:?},[{}],{index:?}),",
            operands.join(",")
        )?;
        // Each assignment always appends a new slot, including a rebinding.
        defs.insert(name, slot);
        if slot >= 512 {
            return Err("Preset guide program exceeds its evaluation budget".into());
        }
    }
    writeln!(output, "], paths: &[")?;
    for path in list(shape, "pathLst", "path") {
        let dimensions = |name| -> Result<i64> { Ok(path.attribute(name).unwrap_or("0").parse()?) };
        let fill = match path.attribute("fill").unwrap_or("norm") {
            "norm" => "Norm",
            "none" => "None",
            "lighten" => "Lighten",
            "lightenLess" => "LightenLess",
            "darken" => "Darken",
            "darkenLess" => "DarkenLess",
            v => return Err(format!("Unsupported path fill {v}").into()),
        };
        let stroke = !matches!(path.attribute("stroke"), Some("0" | "false"));
        writeln!(
            output,
            "Path {{ size:[{}_f64,{}_f64],fill:FillMode::{fill},stroke:{stroke},steps:&[",
            dimensions("w")?,
            dimensions("h")?
        )?;
        for command in path.children().filter(Node::is_element) {
            let mut points = children(command, "pt")
                .map(|p| operand_array(p, &["x", "y"], &defs))
                .collect::<Result<Vec<_>>>()?
                .into_iter();
            let mut next = || points.next().ok_or("Missing DrawingML path point");
            let step = match command.tag_name().name() {
                "moveTo" => format!("Move({})", next()?),
                "lnTo" => format!("Line({})", next()?),
                "quadBezTo" => format!("Quad({},{})", next()?, next()?),
                "cubicBezTo" => format!("Cubic({},{},{})", next()?, next()?, next()?),
                "arcTo" => format!(
                    "Arc({})",
                    operand_array(command, &["wR", "hR", "stAng", "swAng"], &defs)?
                ),
                "close" => "Close".into(),
                tag => return Err(format!("Unknown path command {tag}").into()),
            };
            if points.next().is_some() {
                return Err("Excess path points".into());
            }
            writeln!(output, "Step::{step},")?;
        }
        writeln!(output, "]}},")?;
    }
    let rect = children(shape, "rect")
        .next()
        .map(|r| operand_array(r, &["l", "t", "r", "b"], &defs))
        .transpose()?
        .unwrap_or("[V::Number(0.),V::Number(0.),V::Builtin(Builtin::Width(1)),V::Builtin(Builtin::Height(1))]".into());
    writeln!(output, "],rect:{rect},sites:&[")?;
    for s in list(shape, "cxnLst", "cxn") {
        let pos = children(s, "pos").next().ok_or("Missing site position")?;
        writeln!(output, "{},", operand_array(pos, &["x", "y"], &defs)?)?;
    }
    writeln!(output, "], handles:&[")?;
    for handle in children(shape, "ahLst").flat_map(|l| l.children().filter(Node::is_element)) {
        let pos = children(handle, "pos")
            .next()
            .ok_or("Missing handle position")?;
        let pos = operand_array(pos, &["x", "y"], &defs)?;
        for (axis, reference, lo, hi) in [
            ("X", "gdRefX", "minX", "maxX"),
            ("Y", "gdRefY", "minY", "maxY"),
            ("Radius", "gdRefR", "minR", "maxR"),
            ("Angle", "gdRefAng", "minAng", "maxAng"),
        ] {
            if let Some(name) = handle.attribute(reference) {
                let i = adjustment(name).ok_or("Unknown adjustment index")?;
                writeln!(
                    output,
                    "Control {{ index:{i},kind:Axis::{axis},pos:{pos},bounds:{} }},",
                    operand_array(handle, &[lo, hi], &defs)?
                )?;
            }
        }
    }
    writeln!(output, "] }}),")?;
    Ok(())
}

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed={SOURCE}");
    let source = fs::read_to_string(SOURCE)?;
    let doc = roxmltree::Document::parse(source.trim_start_matches('\u{feff}'))?;
    let mut out =
        String::from("fn program(name: &str) -> Option<&'static Program> { match name {\n");
    let mut seen = std::collections::HashSet::new();
    for shape in doc.root_element().children().filter(Node::is_element) {
        let name = shape.tag_name().name();
        if !seen.insert(name) {
            // The annex labels its single up arrow and its opposed up/down arrow
            // with the same element name. Both use the dedicated host generators.
            if name == "upDownArrow" {
                continue;
            }
            return Err(format!("Duplicate geometry {name}").into());
        }

        program(shape, &mut out).map_err(|e| format!("{}: {e}", shape.tag_name().name()))?;
    }
    out.push_str("_ => None, }}\n");
    let destination = PathBuf::from(env::var_os("OUT_DIR").ok_or("Cargo OUT_DIR missing")?)
        .join("standard_presets.rs");
    fs::write(destination, out)?;
    Ok(())
}
