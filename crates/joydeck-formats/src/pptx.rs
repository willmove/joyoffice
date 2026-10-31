//! Public OPC / Transitional PresentationML subset. No file-system extraction.
use crate::document::*;
use roxmltree::{Document, Node, ParsingOptions};
use std::{collections::BTreeMap, sync::atomic::AtomicBool};

const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PKG: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
pub(crate) fn xml(text: &str) -> Result<Document<'_>> {
    let doc = Document::parse_with_options(
        text,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: 100_000,
            ..Default::default()
        },
    )
    .map_err(|e| format!("XML 无效：{e}"))?;
    if doc
        .descendants()
        .any(|n| n.ancestors().take(130).count() > 128)
    {
        return Err("XML 深度超过 128".into());
    }
    Ok(doc)
}
fn child<'a, 'b>(n: Node<'a, 'b>, ns: &str, name: &str) -> Option<Node<'a, 'b>> {
    n.children().find(|n| n.has_tag_name((ns, name)))
}
fn find<'a, 'b>(n: Node<'a, 'b>, ns: &str, name: &str) -> Option<Node<'a, 'b>> {
    n.descendants().find(|n| n.has_tag_name((ns, name)))
}
fn number(n: Node<'_, '_>, attr: &str) -> Result<f32> {
    let v: f32 = n
        .attribute(attr)
        .ok_or_else(|| format!("缺少数值属性 {attr}"))?
        .parse()
        .map_err(|_| format!("数值属性无效 {attr}"))?;
    if !v.is_finite() || v.abs() > 127_000_000. {
        return Err(format!("几何数值超限 {attr}"));
    }
    Ok(v / 12700.)
}
fn resolve(owner: &str, target: &str) -> Result<String> {
    if target.contains(['\\', ':', '%', '?', '#']) {
        return Err("不支持的关系目标 URI".into());
    }
    let mut components: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        owner
            .rsplit_once('/')
            .map(|(dir, _)| dir.split('/').collect())
            .unwrap_or_default()
    };
    for c in target.trim_start_matches('/').split('/') {
        match c {
            "" | "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err("关系目标逃逸包根目录".into());
                }
            }
            _ => components.push(c),
        }
    }
    if components.is_empty() {
        return Err("空关系目标".into());
    }
    Ok(components.join("/"))
}
struct Rel {
    kind: String,
    target: String,
    external: bool,
}
fn rels(parts: &BTreeMap<String, Vec<u8>>, owner: &str) -> Result<BTreeMap<String, Rel>> {
    let path = if owner.is_empty() {
        "_rels/.rels".to_owned()
    } else {
        let (dir, file) = owner.rsplit_once('/').unwrap_or(("", owner));
        format!(
            "{}{}/{}.rels",
            if dir.is_empty() {
                String::new()
            } else {
                format!("{dir}/")
            },
            "_rels",
            file
        )
    };
    let doc = xml(part(parts, &path)?)?;
    if !doc.root_element().has_tag_name((PKG, "Relationships")) {
        return Err("关系 namespace 不支持".into());
    }
    let mut out = BTreeMap::new();
    for n in doc.root_element().children().filter(|n| n.is_element()) {
        if !n.has_tag_name((PKG, "Relationship")) {
            return Err("未知关系元素".into());
        }
        let id = n.attribute("Id").ok_or("关系缺少 Id")?;
        let kind = n.attribute("Type").ok_or("关系缺少 Type")?;
        let target = n.attribute("Target").ok_or("关系缺少 Target")?;
        let external = n.attribute("TargetMode") == Some("External");
        let target = if external {
            target.to_string()
        } else {
            resolve(owner, target)?
        };
        if out
            .insert(
                id.to_string(),
                Rel {
                    kind: kind.into(),
                    target,
                    external,
                },
            )
            .is_some()
        {
            return Err("重复关系 Id".into());
        }
    }
    Ok(out)
}
pub(crate) fn related(parts: &BTreeMap<String, Vec<u8>>, base: &str, id: &str) -> Result<String> {
    let relations = rels(parts, base)?;
    let r = relations.get(id).ok_or("关系不存在")?;
    if r.external {
        return Err("不读取外部图表链接".into());
    }
    Ok(r.target.clone())
}
pub fn import(parts: &BTreeMap<String, Vec<u8>>, cancel: &AtomicBool) -> Result<Deck> {
    if let Some(deck) = crate::pptx_write::read_snapshot(parts)? {
        return Ok(deck);
    }
    let root = rels(parts, "")?;
    let mains: Vec<_> = root
        .values()
        .filter(|r| r.kind == format!("{R}/officeDocument") && !r.external)
        .collect();
    if mains.len() != 1 {
        return Err("未找到唯一 Transitional PPTX 主文档；Strict/加密/宏格式未支持".into());
    }
    let main = &mains[0].target;
    let types = xml(part(parts, "[Content_Types].xml")?)?;
    let ct = "http://schemas.openxmlformats.org/package/2006/content-types";
    if !types.root_element().has_tag_name((ct,"Types")) || !types.descendants().any(|n| n.has_tag_name((ct,"Override")) && n.attribute("PartName") == Some(format!("/{main}").as_str()) && n.attribute("ContentType") == Some("application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml")) {
        return Err("仅支持普通 PPTX 主文档内容类型".into());
    }
    let presentation = xml(part(parts, main)?)?;
    let pr = presentation.root_element();
    if !pr.has_tag_name((P, "presentation")) {
        return Err("不支持的 PresentationML namespace".into());
    }
    let sz = child(pr, P, "sldSz").ok_or("缺少页面尺寸")?;
    let mut deck = Deck {
        playback: Default::default(),
        review: Default::default(),
        presentation: Default::default(),
        source: None,
        resources: Default::default(),
        design: Default::default(),
        organization: Default::default(),
        field_date: String::new(),
        title: "PPTX 文稿".into(),
        width: number(sz, "cx")?,
        height: number(sz, "cy")?,
        slides: Vec::new(),
        warnings: Vec::new(),
    };
    deck.warn(
        "preview-subset",
        None,
        main,
        "PPTX 简化只读预览：母版/版式/主题、富文本、旋转及效果未完整显示；不支持的对象有页级报告",
    );
    let relationships = rels(parts, main)?;
    let list = child(pr, P, "sldIdLst").ok_or("缺少页面列表")?;
    for id in list.children().filter(|n| n.is_element()) {
        check_cancel(cancel)?;
        if !id.has_tag_name((P, "sldId")) {
            return Err("页面列表包含未知元素".into());
        }
        if deck.slides.len() >= MAX_PAGES {
            return Err("页数超过 500".into());
        }
        let rel = id
            .attribute((R, "id"))
            .and_then(|id| relationships.get(id))
            .ok_or("页面关系缺失")?;
        if rel.external || rel.kind != format!("{R}/slide") {
            return Err("无效或外部页面关系".into());
        }
        let sd = xml(part(parts, &rel.target)?)?;
        if !sd.root_element().has_tag_name((P, "sld")) {
            return Err("不支持的页面 namespace".into());
        }
        let page = deck.slides.len();
        let bg = find(sd.root_element(), P, "bgPr")
            .and_then(solid)
            .unwrap_or_else(|| "#ffffff".into());
        let mut slide = Slide {
            hidden: sd.root_element().attribute("show") == Some("0"),
            layout: None,
            inherit_background: false,
            notes: String::new(),
            section: None,
            id: id.attribute("id").ok_or("页面 ID 缺失")?.into(),
            title: format!("第 {} 页", page + 1),
            background: bg,
            objects: Vec::new(),
        };
        let tree = find(sd.root_element(), P, "spTree").ok_or("页面缺少 spTree")?;
        let mut motion_ids = std::collections::BTreeMap::new();
        for n in tree.children().filter(|n| n.is_element()) {
            check_cancel(cancel)?;
            if slide.objects.len() >= 10000 {
                return Err("单页对象超过 10000".into());
            }
            let start = slide.objects.len();
            match n.tag_name().name() {
                "nvGrpSpPr" | "grpSpPr" => {}
                "graphicFrame"
                    if n.tag_name().namespace() == Some(P)
                        && n.descendants().any(|n| n.has_tag_name((A, "tbl"))) =>
                {
                    slide
                        .objects
                        .push(crate::table_pptx::read(n, page)?.ok_or("表格缺少数据")?);
                }
                "graphicFrame"
                    if n.descendants().any(|n| {
                        n.has_tag_name((
                            "http://schemas.openxmlformats.org/drawingml/2006/chart",
                            "chart",
                        ))
                    }) =>
                {
                    match crate::chart_package::read(parts, &rel.target, n, page) {
                        Ok(o) => slide.objects.push(o),
                        Err(e) => {
                            deck.warn("chart-subset", Some(page), &rel.target, &e);
                            slide.objects.push(placeholder(
                                slide.objects.len(),
                                "此图表暂不支持编辑；原包保留",
                            ));
                        }
                    }
                }
                "graphicFrame"
                    if n.descendants().any(|n| {
                        n.has_tag_name((
                            "http://schemas.openxmlformats.org/drawingml/2006/diagram",
                            "relIds",
                        ))
                    }) =>
                {
                    match crate::diagram_package::read(parts, &rel.target, n, page) {
                        Ok(o) => {
                            slide.objects.push(o);
                            deck.warn("smartart-simplified",Some(page),&rel.target,"SmartArt文本拓扑已转换为基础图示；布局、样式和绘制缓存不等同原图，原包继续保留");
                        }
                        Err(e) => {
                            deck.warn("smartart-subset", Some(page), &rel.target, &e);
                            slide
                                .objects
                                .push(placeholder(slide.objects.len(), "复杂SmartArt，原包保留"));
                        }
                    }
                }
                "grpSp" => parse_group(n, &mut slide, &mut deck, page, &rel.target, 0)?,
                "sp" if n.tag_name().namespace() == Some(P) => {
                    parse_shape(n, &mut slide, &mut deck, page, &rel.target)?
                }
                other => {
                    deck.warn(
                        "unsupported-object",
                        Some(page),
                        &rel.target,
                        &format!("未支持对象 {other}；以占位提示显示"),
                    );
                    let label = match other {
                        "pic" => "图片暂未显示",
                        "grpSp" => "组合对象暂未显示",
                        "graphicFrame" => "图表或表格暂未显示",
                        "cxnSp" => "连接线暂未显示",
                        _ => "此对象暂未支持预览",
                    };
                    slide.objects.push(placeholder(slide.objects.len(), label));
                }
            }
            if let Some(spid) = n
                .descendants()
                .find(|n| n.has_tag_name((P, "cNvPr")))
                .and_then(|n| n.attribute("id"))
            {
                for (j, o) in slide.objects[start..].iter_mut().enumerate() {
                    if o.id.is_empty() {
                        o.id = format!("pptx-{page}-{spid}-{j}");
                    }
                }
                if let Some(key) = n
                    .descendants()
                    .find(|n| n.has_tag_name((P, "cNvPr")))
                    .and_then(|n| n.attribute("name"))
                    .and_then(|s| s.strip_prefix("!!"))
                    && let Some(o) = slide.objects.last_mut()
                {
                    o.morph_key = Some(key.into());
                }
                if n.tag_name().name() != "grpSp"
                    && let Some(o) = slide.objects.last()
                {
                    motion_ids.insert(spid.to_owned(), o.id.clone());
                }
            }
        }
        match crate::motion_package::read(sd.root_element(), &motion_ids, deck.width, deck.height) {
            Ok((timing, unknown)) => {
                if timing != Default::default() {
                    deck.presentation.slides.insert(slide.id.clone(), timing);
                }
                if unknown {
                    deck.warn(
                        "motion-subset",
                        Some(page),
                        &rel.target,
                        "部分动画不受支持，原包保留；新建导出会丢失这些效果",
                    );
                }
            }
            Err(e) => deck.warn("motion-subset", Some(page), &rel.target, &e),
        }
        if let Some(o) = slide
            .objects
            .iter()
            .find(|o| o.kind == "text" && !o.text.is_empty())
        {
            slide.title = o
                .text
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(64)
                .collect();
        }
        deck.slides.push(slide);
    }
    Ok(deck)
}
fn parse_group(
    n: Node<'_, '_>,
    slide: &mut Slide,
    deck: &mut Deck,
    page: usize,
    path: &str,
    depth: usize,
) -> Result<()> {
    if depth > 8 {
        return Err("组合层级超过8".into());
    }
    let start = slide.objects.len();
    let xf = child(n, P, "grpSpPr")
        .and_then(|n| child(n, A, "xfrm"))
        .ok_or("组合缺少变换")?;
    let off = child(xf, A, "off").ok_or("组合位置缺失")?;
    let ext = child(xf, A, "ext").ok_or("组合尺寸缺失")?;
    let coff = child(xf, A, "chOff").ok_or("组合子坐标缺失")?;
    let cext = child(xf, A, "chExt").ok_or("组合子尺寸缺失")?;
    let (x, y, w, h, cx, cy, cw, ch) = (
        number(off, "x")?,
        number(off, "y")?,
        number(ext, "cx")?,
        number(ext, "cy")?,
        number(coff, "x")?,
        number(coff, "y")?,
        number(cext, "cx")?,
        number(cext, "cy")?,
    );
    if cw <= 0. || ch <= 0. {
        return Err("组合子坐标尺寸无效".into());
    }
    for child in n.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "sp" => parse_shape(child, slide, deck, page, path)?,
            "grpSp" => parse_group(child, slide, deck, page, path, depth + 1)?,
            "nvGrpSpPr" | "grpSpPr" => {}
            _ => {
                deck.warn(
                    "group-subset",
                    Some(page),
                    path,
                    "组合内包含不受支持对象；原包保留",
                );
            }
        }
    }
    let rotation = xf
        .attribute("rot")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.)
        / 60000.;
    let (sin, cos) = rotation.to_radians().sin_cos();
    for (i, o) in slide.objects[start..].iter_mut().enumerate() {
        o.x = x + (o.x - cx) * w / cw;
        o.y = y + (o.y - cy) * h / ch;
        o.width *= w / cw;
        o.height *= h / ch;
        let dx = o.x + o.width / 2. - x - w / 2.;
        let dy = o.y + o.height / 2. - y - h / 2.;
        o.x = x + w / 2. + dx * cos - dy * sin - o.width / 2.;
        o.y = y + h / 2. + dx * sin + dy * cos - o.height / 2.;
        o.rotation += rotation;
        o.id = format!(
            "pptx-group-{page}-{}-{i}",
            n.descendants()
                .find(|n| n.has_tag_name((P, "cNvPr")))
                .and_then(|n| n.attribute("id"))
                .unwrap_or("group")
        );
    }
    deck.warn(
        "group-flattened",
        Some(page),
        path,
        "组合已展开为独立可编辑形状；原始组合关系保存在源包",
    );
    Ok(())
}
fn solid(n: Node<'_, '_>) -> Option<String> {
    let c = child(n, A, "solidFill")
        .and_then(|n| child(n, A, "srgbClr"))?
        .attribute("val")?;
    let c = format!("#{c}");
    color(&c).then_some(c)
}
fn placeholder(index: usize, text: &str) -> Object {
    Object {
        text_align: "left".into(),
        morph_key: None,
        chart: None,
        diagram: None,
        table: None,
        image: None,
        binding: None,
        field: None,
        id: String::new(),
        rotation: 0.,
        opacity: 1.,
        stroke: "#202b3a".into(),
        stroke_width: 0.,
        kind: "placeholder".into(),
        x: 32.,
        y: 32. + (index % 8) as f32 * 38.,
        width: 380.,
        height: 32.,
        fill: "#a96313".into(),
        text: text.into(),
        font_size: 14.,
        runs: vec![],
    }
}
fn parse_shape(
    n: Node<'_, '_>,
    slide: &mut Slide,
    deck: &mut Deck,
    page: usize,
    path: &str,
) -> Result<()> {
    let Some(props) = child(n, P, "spPr") else {
        return Err("形状缺少 spPr".into());
    };
    let Some(xf) = child(props, A, "xfrm") else {
        deck.warn(
            "layout-position",
            Some(page),
            path,
            "形状依赖版式坐标，当前显示占位",
        );
        slide
            .objects
            .push(placeholder(slide.objects.len(), "版式定位对象"));
        return Ok(());
    };
    let off = child(xf, A, "off").ok_or("形状缺少 off")?;
    let ext = child(xf, A, "ext").ok_or("形状缺少 ext")?;
    let (x, y, w, h) = (
        number(off, "x")?,
        number(off, "y")?,
        number(ext, "cx")?,
        number(ext, "cy")?,
    );
    let geom = child(props, A, "prstGeom").and_then(|n| n.attribute("prst"));
    let kind = match geom {
        Some("rect") => "rect",
        Some("ellipse") => "ellipse",
        Some("roundRect") => "roundRect",
        Some("rightArrow") => "arrow",
        Some("line") => "line",
        _ => "placeholder",
    };
    if kind == "placeholder"
        || props.descendants().any(|n| {
            matches!(
                n.tag_name().name(),
                "gradFill" | "effectLst" | "custGeom" | "schemeClr"
            )
        })
        || xf.attribute("rot").is_some()
        || xf.attribute("flipH").is_some()
        || xf.attribute("flipV").is_some()
    {
        deck.warn(
            "shape-style",
            Some(page),
            path,
            "形状预设、主题色、旋转或效果未完整支持",
        );
    }
    let no_fill = child(props, A, "noFill").is_some();
    if !no_fill || kind == "line" || kind == "placeholder" {
        let fill = if kind == "line" {
            child(props, A, "ln").and_then(solid)
        } else {
            solid(props)
        }
        .unwrap_or_else(|| "#dce5ef".into());
        slide.objects.push(Object {
            text_align: "left".into(),
            morph_key: None,
            chart: None,
            diagram: None,
            table: None,
            image: None,
            binding: None,
            field: None,
            id: String::new(),
            rotation: 0.,
            opacity: 1.,
            stroke: "#202b3a".into(),
            stroke_width: 0.,
            kind: kind.into(),
            x,
            y,
            width: w,
            height: h,
            fill,
            text: if kind == "placeholder" {
                "未支持的形状".into()
            } else {
                String::new()
            },
            font_size: 14.,
            runs: vec![],
        });
    }
    if let Some(body) = child(n, P, "txBody") {
        let text = body
            .children()
            .filter(|n| n.has_tag_name((A, "p")))
            .map(|p| {
                let mut line = String::new();
                for c in p.descendants().filter(|c| c.is_element()) {
                    if c.has_tag_name((A, "t")) {
                        line.push_str(c.text().unwrap_or(""));
                    }
                    if c.has_tag_name((A, "br")) {
                        line.push('\n');
                    }
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n");
        let style = find(body, A, "rPr");
        let size = match style.and_then(|n| n.attribute("sz")) {
            Some(s) => s.parse::<f32>().map_err(|_| "无效 PPTX 字号")? / 100.,
            None => 24.,
        };
        let fill = style.and_then(solid).unwrap_or_else(|| "#202b3a".into());
        slide.objects.push(Object {
            text_align: "left".into(),
            morph_key: None,
            chart: None,
            diagram: None,
            table: None,
            image: None,
            binding: None,
            field: None,
            id: String::new(),
            rotation: 0.,
            opacity: 1.,
            stroke: "#202b3a".into(),
            stroke_width: 0.,
            kind: "text".into(),
            x,
            y,
            width: w,
            height: h,
            fill,
            text,
            font_size: size,
            runs: vec![],
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "pptx/tests.rs"]
mod tests;
