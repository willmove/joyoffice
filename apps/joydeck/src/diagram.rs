//! Authored list/process/hierarchy diagrams, independent of proprietary SmartArt layouts.
use crate::document::{Object, Result};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub text: String,
    pub parent: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagram {
    pub layout: String,
    pub nodes: Vec<Node>,
    pub color: String,
}
impl Default for Diagram {
    fn default() -> Self {
        Self {
            layout: "process".into(),
            nodes: ["制定计划", "组织实施", "评估优化"]
                .iter()
                .enumerate()
                .map(|(i, s)| Node {
                    id: format!("node-{i}"),
                    text: (*s).into(),
                    parent: None,
                })
                .collect(),
            color: "#4472c4".into(),
        }
    }
}
impl Diagram {
    pub fn validate(&self) -> Result<()> {
        if !["list", "process", "hierarchy"].contains(&self.layout.as_str())
            || self.nodes.is_empty()
            || self.nodes.len() > 64
            || !crate::document::color(&self.color)
        {
            return Err("图示类型、数量或颜色无效".into());
        }
        let ids: std::collections::BTreeSet<_> = self.nodes.iter().map(|n| &n.id).collect();
        if ids.len() != self.nodes.len() {
            return Err("图示节点ID重复".into());
        }
        for n in &self.nodes {
            if n.id.is_empty()
                || n.id.len() > 256
                || n.text.len() > 4096
                || n.text
                    .chars()
                    .any(|c| c < ' ' && !matches!(c, '\n' | '\r' | '\t'))
            {
                return Err("图示节点超限".into());
            }
            let mut p = n.parent.as_ref();
            let mut seen = std::collections::BTreeSet::from([&n.id]);
            while let Some(id) = p {
                if !seen.insert(id) {
                    return Err("图示层级存在环".into());
                }
                let node = self
                    .nodes
                    .iter()
                    .find(|n| &n.id == id)
                    .ok_or("图示父节点不存在")?;
                p = node.parent.as_ref();
            }
        }
        Ok(())
    }
    fn depth(&self, node: &Node) -> usize {
        let mut depth = 0;
        let mut parent = node.parent.as_ref();
        while let Some(id) = parent {
            depth += 1;
            if depth > 64 {
                break;
            }
            parent = self
                .nodes
                .iter()
                .find(|n| &n.id == id)
                .and_then(|n| n.parent.as_ref());
        }
        depth
    }
    pub fn text(&self) -> String {
        fn walk(d: &Diagram, parent: Option<&str>, depth: usize, out: &mut Vec<String>) {
            for n in d.nodes.iter().filter(|n| n.parent.as_deref() == parent) {
                out.push(format!(
                    "{}{}",
                    "\t".repeat(depth),
                    n.text.replace('\n', " ")
                ));
                if depth < 64 {
                    walk(d, Some(&n.id), depth + 1, out);
                }
            }
        }
        let mut lines = vec![];
        walk(self, None, 0, &mut lines);
        lines.join("\n")
    }
    pub fn set_text(&mut self, text: &str) -> Result<()> {
        let mut nodes = vec![];
        let mut stack: Vec<String> = vec![];
        for (i, line) in text.lines().enumerate() {
            let depth = line.bytes().take_while(|b| *b == b'\t').count();
            if depth > 8 || depth > stack.len() {
                return Err("图示缩进不能跳级，最多8层".into());
            }
            let id = format!("node-{i}");
            nodes.push(Node {
                id: id.clone(),
                text: line.trim_start_matches('\t').into(),
                parent: if depth == 0 {
                    None
                } else {
                    Some(stack[depth - 1].clone())
                },
            });
            stack.truncate(depth);
            stack.push(id);
        }
        let mut d = self.clone();
        d.nodes = nodes;
        d.validate()?;
        *self = d;
        Ok(())
    }
}
pub fn object(layout: &str) -> Object {
    let mut o = crate::edit::object("diagram");
    o.diagram = Some(Diagram {
        layout: layout.into(),
        ..Default::default()
    });
    o.x = 100.;
    o.y = 160.;
    o.width = 720.;
    o.height = 260.;
    o
}
pub fn expand(o: &Object) -> Result<Vec<Object>> {
    let d = o.diagram.as_ref().ok_or("图示模型缺失")?;
    d.validate()?;
    let mut out = vec![];
    let n = d.nodes.len();
    for (i, node) in d.nodes.iter().enumerate() {
        let (x, y, w, h) = match d.layout.as_str() {
            "list" => (
                o.x,
                o.y + o.height * i as f32 / n as f32,
                o.width,
                o.height / n as f32 * 0.8,
            ),
            "hierarchy" => {
                let depth = d.depth(node);
                let levels = d.nodes.iter().map(|n| d.depth(n)).max().unwrap_or(0) + 1;
                let peers: Vec<_> = d
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| d.depth(n) == depth)
                    .collect();
                let at = peers.iter().position(|(j, _)| *j == i).unwrap();
                let w = o.width / peers.len() as f32;
                (
                    o.x + w * at as f32 + 4.,
                    o.y + o.height * depth as f32 / levels as f32,
                    (w - 8.).max(1.),
                    o.height / levels as f32 * 0.68,
                )
            }
            _ => (
                o.x + o.width * i as f32 / n as f32,
                o.y + o.height * 0.25,
                o.width / n as f32 * 0.85,
                o.height * 0.5,
            ),
        };
        let mut bg = crate::edit::object("roundRect");
        bg.id = format!("{}:node:{i}", o.id);
        bg.x = x;
        bg.y = y;
        bg.width = w.max(1.);
        bg.height = h.max(1.);
        bg.fill = d.color.clone();
        bg.opacity = o.opacity;
        let mut text = bg.clone();
        text.id = format!("{}:text:{i}", o.id);
        text.kind = "text".into();
        text.text = node.text.clone();
        text.font_size = (o.height / 12.).clamp(8., 24.);
        text.text_align = "center".into();
        text.fill = "#ffffff".into();
        text.x += 6.;
        text.y += h * 0.25;
        text.width = (w - 12.).max(1.);
        text.height = (h * 0.7).max(1.);
        out.extend([bg, text]);
        if d.layout == "process" && i + 1 < n {
            let mut arrow = crate::edit::object("arrow");
            arrow.id = format!("{}:arrow:{i}", o.id);
            arrow.x = x + w + 2.;
            arrow.y = y + h * 0.4;
            arrow.width = (o.width / n as f32 - w - 4.).max(1.);
            arrow.height = h * 0.2;
            arrow.fill = d.color.clone();
            arrow.opacity = o.opacity;
            out.push(arrow);
        }
    }
    if d.layout == "hierarchy" {
        let mut lines = vec![];
        for (i, node) in d.nodes.iter().enumerate() {
            if let Some(parent) = node
                .parent
                .as_ref()
                .and_then(|id| d.nodes.iter().position(|n| &n.id == id))
            {
                let a = out
                    .iter()
                    .find(|v| v.id == format!("{}:node:{parent}", o.id))
                    .unwrap();
                let b = out
                    .iter()
                    .find(|v| v.id == format!("{}:node:{i}", o.id))
                    .unwrap();
                let (ax, ay, bx, by) =
                    (a.x + a.width / 2., a.y + a.height, b.x + b.width / 2., b.y);
                let mut line = crate::edit::object("line");
                line.id = format!("{}:connector:{i}", o.id);
                let len = (bx - ax).hypot(by - ay).max(1.);
                line.x = (ax + bx) / 2. - len / 2.;
                line.y = (ay + by) / 2. - 0.5;
                line.width = len;
                line.height = 1.;
                line.rotation =
                    (by - ay).atan2(bx - ax).to_degrees() - (1. / len).atan().to_degrees();
                line.fill = d.color.clone();
                line.stroke = d.color.clone();
                line.stroke_width = 2.;
                line.opacity = o.opacity;
                lines.push(line);
            }
        }
        lines.extend(out);
        out = lines;
    }
    // Rotation is applied around the diagram, not each child independently.
    for item in &mut out {
        let dx = item.x + item.width / 2. - o.x - o.width / 2.;
        let dy = item.y + item.height / 2. - o.y - o.height / 2.;
        let (s, c) = o.rotation.to_radians().sin_cos();
        item.x = o.x + o.width / 2. + dx * c - dy * s - item.width / 2.;
        item.y = o.y + o.height / 2. + dx * s + dy * c - item.height / 2.;
        item.rotation += o.rotation;
    }
    Ok(out)
}
