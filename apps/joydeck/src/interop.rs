//! Source-package preservation and deliberately narrow non-lossy PPTX editing.
use crate::document::{self, Deck, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePackage {
    pub parts: BTreeMap<String, Vec<u8>>,
    pub baseline: Vec<u8>,
    pub text: Vec<TextBinding>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextBinding {
    pub object: String,
    pub part: String,
    pub start: usize,
    pub end: usize,
}
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
pub fn hash(parts: &BTreeMap<String, Vec<u8>>) -> String {
    let mut h = Sha256::new();
    for (n, b) in parts {
        h.update((n.len() as u64).to_le_bytes());
        h.update(n);
        h.update((b.len() as u64).to_le_bytes());
        h.update(b);
    }
    format!("{:x}", h.finalize())
}
pub fn xml(s: &str) -> Result<roxmltree::Document<'_>> {
    roxmltree::Document::parse_with_options(
        s,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 100000,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())
}
pub fn target(owner: &str, value: &str) -> Result<String> {
    if value.contains(['\\', ':', '%', '?', '#']) {
        return Err("不支持的内部关系URI".into());
    }
    let mut p: Vec<_> = if value.starts_with('/') {
        vec![]
    } else {
        owner
            .rsplit_once('/')
            .map_or(vec![], |(dir, _)| dir.split('/').collect())
    };
    for s in value.trim_start_matches('/').split('/') {
        match s {
            "" | "." => {}
            ".." => {
                p.pop().ok_or("关系逃逸包根目录")?;
            }
            _ => p.push(s),
        }
    }
    if p.is_empty() {
        return Err("空关系目标".into());
    }
    Ok(p.join("/"))
}
pub fn validate_graph(parts: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    if parts.len() > 4096
        || parts.values().any(|b| b.len() > document::MAX_PART)
        || parts.values().map(Vec::len).sum::<usize>() > document::MAX_TOTAL
    {
        return Err("源包部件预算超限".into());
    }
    for (name, bytes) in parts {
        if name.starts_with('/')
            || name.contains(['\\', ':', '%'])
            || name.trim_end_matches('/').is_empty()
            || (name.ends_with('/') && !bytes.is_empty())
            || name
                .trim_end_matches('/')
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == "..")
        {
            return Err("源包路径非法".into());
        }
        if !name.ends_with(".rels") {
            continue;
        }
        let owner = if name == "_rels/.rels" {
            String::new()
        } else {
            let (dir, file) = name.rsplit_once("/_rels/").ok_or("关系部件位置非法")?;
            format!(
                "{dir}/{}",
                file.strip_suffix(".rels").ok_or("关系部件非法")?
            )
        };
        let doc = xml(std::str::from_utf8(bytes).map_err(|e| e.to_string())?)?;
        if !doc.root_element().has_tag_name((
            "http://schemas.openxmlformats.org/package/2006/relationships",
            "Relationships",
        )) {
            return Err("非法关系根".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for n in doc.root_element().children().filter(|n| n.is_element()) {
            if !n.has_tag_name((
                "http://schemas.openxmlformats.org/package/2006/relationships",
                "Relationship",
            )) || n.attribute("Type").is_none_or(|s| s.is_empty())
                || n.attribute("TargetMode")
                    .is_some_and(|s| s != "Internal" && s != "External")
                || !ids.insert(n.attribute("Id").ok_or("缺少关系ID")?)
            {
                return Err("关系ID缺失/重复".into());
            }
            if n.attribute("TargetMode") == Some("External") {
                continue;
            }
            let dest = target(&owner, n.attribute("Target").ok_or("缺少关系目标")?)?;
            if !parts.contains_key(&dest) {
                return Err(format!("悬空关系 {owner} → {dest}"));
            }
        }
    }
    Ok(())
}
pub fn import(path: &Path) -> Result<Deck> {
    let cancel = AtomicBool::new(false);
    let parts = document::read_package(path, &cancel)?;
    if parts.contains_key("mimetype") {
        return crate::edit::open_editable(path);
    }
    validate_graph(&parts)?;
    if let Some(d) = crate::pptx_write::read_snapshot(&parts)? {
        return Ok(d);
    }
    let mut d = crate::pptx::import(&parts, &cancel)?;
    d.validate()?;
    d = crate::edit::Session::from_deck(d, false)
        .deck
        .as_ref()
        .clone();
    let mut bindings = vec![];
    // Match unique single-run text against original XML. Ambiguous/multi-run text stays preserved but not editable in-place.
    let a = "http://schemas.openxmlformats.org/drawingml/2006/main";
    for s in &d.slides {
        for o in &s.objects {
            if o.kind != "text" {
                continue;
            }
            let mut matches = vec![];
            for (name, bytes) in &parts {
                if !name.ends_with(".xml") {
                    continue;
                }
                let Ok(text) = std::str::from_utf8(bytes) else {
                    continue;
                };
                let Ok(doc) = xml(text) else {
                    continue;
                };
                if !doc.root_element().has_tag_name((
                    "http://schemas.openxmlformats.org/presentationml/2006/main",
                    "sld",
                )) {
                    continue;
                }
                for body in doc
                    .descendants()
                    .filter(|n| n.tag_name().name() == "txBody")
                {
                    let texts: Vec<_> = body
                        .descendants()
                        .filter(|n| n.has_tag_name((a, "t")))
                        .collect();
                    if texts.len() != 1
                        || body
                            .descendants()
                            .filter(|n| n.has_tag_name((a, "p")))
                            .count()
                            != 1
                        || texts[0].text().unwrap_or("") != o.text
                    {
                        continue;
                    }
                    if let Some(node) = texts[0].children().find(|n| n.is_text()) {
                        let range = node.range();
                        matches.push(TextBinding {
                            object: o.id.clone(),
                            part: name.clone(),
                            start: range.start,
                            end: range.end,
                        });
                    }
                }
            }
            if matches.len() == 1 {
                bindings.push(matches.remove(0));
            }
        }
    }
    let baseline = serde_json::to_vec(&d).map_err(|e| e.to_string())?;
    d.source = Some(Arc::new(SourcePackage {
        parts,
        baseline,
        text: bindings,
    }));
    d.validate()?;
    Ok(d)
}
impl SourcePackage {
    pub fn validate(&self) -> Result<()> {
        validate_graph(&self.parts)?;
        if self.baseline.len() > document::MAX_PART || self.text.len() > 50000 {
            return Err("源包映射超限".into());
        }
        let base: Deck = serde_json::from_slice(&self.baseline).map_err(|e| e.to_string())?;
        if base.source.is_some() {
            return Err("递归源包模型不允许".into());
        }
        base.validate()?;
        let mut seen = std::collections::BTreeSet::new();
        let mut positions = std::collections::BTreeSet::new();
        for b in &self.text {
            let text = std::str::from_utf8(self.parts.get(&b.part).ok_or("源部件缺失")?)
                .map_err(|e| e.to_string())?;
            if !seen.insert(&b.object)
                || b.start > b.end
                || !text.is_char_boundary(b.start)
                || !text.is_char_boundary(b.end)
                || text.get(b.start..b.end).is_none()
            {
                return Err("源文字映射无效".into());
            }
            let doc = xml(text)?;
            let old = base
                .slides
                .iter()
                .flat_map(|s| &s.objects)
                .find(|o| o.id == b.object && o.kind == "text")
                .ok_or("源文字对象缺失")?;
            if !positions.insert((&b.part, b.start, b.end))
                || !doc.descendants().any(|n| {
                    n.has_tag_name(("http://schemas.openxmlformats.org/drawingml/2006/main", "t"))
                        && n.text() == Some(old.text.as_str())
                        && n.children()
                            .any(|n| n.is_text() && n.range() == (b.start..b.end))
                })
            {
                return Err("源文字映射不匹配原始XML文本节点".into());
            }
        }
        Ok(())
    }
    pub fn preserve(&self, deck: &Deck) -> Result<BTreeMap<String, Vec<u8>>> {
        self.validate()?;
        deck.validate()?;
        let base: Deck = serde_json::from_slice(&self.baseline).map_err(|e| e.to_string())?;
        let mut compare = deck.clone();
        compare.source = None;
        compare.warnings.clear();
        let mut patches: BTreeMap<String, Vec<(usize, usize, String)>> = BTreeMap::new();
        for old in base.slides.iter().flat_map(|s| &s.objects) {
            let Some(new) = compare
                .slides
                .iter_mut()
                .flat_map(|s| &mut s.objects)
                .find(|o| o.id == old.id)
            else {
                continue;
            };
            if new.text != old.text {
                let b=self.text.iter().find(|b|b.object==old.id).ok_or("此文字含多个run/段落或匹配不唯一，无法保留源包写回；请使用明确有损的新建PPTX导出")?;
                if new.text.chars().any(char::is_control) {
                    return Err("源包保留写回仅支持单行文字；请使用有损导出".into());
                }
                patches.entry(b.part.clone()).or_default().push((
                    b.start,
                    b.end,
                    escape(&new.text),
                ));
            }
            if old.runs.is_empty()
                && new
                    .runs
                    .iter()
                    .all(|r| r.style == crate::text::Style::for_object(old))
            {
                new.runs.clear();
            }
            new.text.clone_from(&old.text);
        }
        if compare != base {
            return Err(
                "修改超出源包保留写回的单行文字子集；拒绝静默丢失，请选择有损导出并查看损失声明"
                    .into(),
            );
        }
        let mut parts = self.parts.clone();
        for (name, mut edits) in patches {
            edits.sort_by_key(|p| std::cmp::Reverse(p.0));
            let mut bytes = String::from_utf8(parts[&name].clone()).map_err(|e| e.to_string())?;
            let mut last = bytes.len();
            for (a, b, value) in edits {
                if b > last {
                    return Err("源文字映射重叠".into());
                }
                bytes.replace_range(a..b, &value);
                last = a;
            }
            parts.insert(name, bytes.into_bytes());
        }
        validate_graph(&parts)?;
        Ok(parts)
    }
}
