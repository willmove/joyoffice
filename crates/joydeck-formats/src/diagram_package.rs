//! Import bounded SmartArt text topology as an explicitly simplified authored diagram.
use crate::{
    diagram::{Diagram, Node},
    document::{Object, Result},
};
use roxmltree::Node as Xml;
use std::collections::BTreeMap;
const D: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub fn read(
    parts: &BTreeMap<String, Vec<u8>>,
    slide: &str,
    frame: Xml<'_, '_>,
    page: usize,
) -> Result<Object> {
    let rel = frame
        .descendants()
        .find(|n| n.has_tag_name((D, "relIds")))
        .ok_or("图示关系缺失")?;
    let rid = rel.attribute((R, "dm")).ok_or("图示dataModel关系缺失")?;
    let path = crate::pptx::related(parts, slide, rid)?;
    let xml = crate::pptx::xml(crate::document::part(parts, &path)?)?;
    let mut nodes = vec![];
    for n in xml.descendants().filter(|n| {
        n.has_tag_name((D, "pt"))
            && matches!(n.attribute("type"), None | Some("node") | Some("asst"))
    }) {
        if nodes.len() >= 64 {
            return Err("SmartArt文本节点超过64预算".into());
        }
        let text = n
            .descendants()
            .filter(|n| n.has_tag_name((A, "p")))
            .map(|n| {
                n.descendants()
                    .filter(|n| n.has_tag_name((A, "t")))
                    .filter_map(|n| n.text())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        nodes.push(Node {
            id: n.attribute("modelId").ok_or("SmartArt节点缺少ID")?.into(),
            text,
            parent: None,
        });
    }
    for n in xml
        .descendants()
        .filter(|n| n.has_tag_name((D, "cxn")) && n.attribute("type") == Some("parOf"))
    {
        let source = n.attribute("srcId").ok_or("SmartArt连接源缺失")?;
        let dest = n.attribute("destId").ok_or("SmartArt连接目标缺失")?;
        if nodes.iter().any(|n| n.id == source)
            && let Some(child) = nodes.iter_mut().find(|n| n.id == dest)
        {
            if child.parent.is_some() {
                return Err("SmartArt多父节点不能转换为基础层级".into());
            }
            child.parent = Some(source.into());
        }
    }
    let layout = if nodes.iter().any(|n| n.parent.is_some()) {
        "hierarchy"
    } else {
        "list"
    };
    let d = Diagram {
        layout: layout.into(),
        nodes,
        color: "#4472c4".into(),
    };
    d.validate()?;
    let mut o = crate::diagram::object(layout);
    o.id = format!("pptx-diagram-{page}-{rid}");
    o.diagram = Some(d);
    let xfrm = frame
        .descendants()
        .find(|n| n.tag_name().name() == "xfrm")
        .ok_or("图示几何缺失")?;
    let num = |tag: &str, key: &str| -> Result<f32> {
        xfrm.descendants()
            .find(|n| n.has_tag_name((A, tag)))
            .and_then(|n| n.attribute(key))
            .and_then(|s| s.parse::<f32>().ok())
            .map(|v| v / 12700.)
            .ok_or("图示几何无效".into())
    };
    o.x = num("off", "x")?;
    o.y = num("off", "y")?;
    o.width = num("ext", "cx")?;
    o.height = num("ext", "cy")?;
    o.rotation = xfrm
        .attribute("rot")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.)
        / 60000.;
    Ok(o)
}
