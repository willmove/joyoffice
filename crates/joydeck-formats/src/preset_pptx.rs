//! Strict literal adjust-value subset; source XML is retained by the shell.
use joydeck_model::{
    document::Result,
    preset::{self, Preset},
};
use roxmltree::Node;
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

pub fn read(props: Node<'_, '_>) -> Result<Option<Preset>> {
    let Some(geom) = props.children().find(|n| n.has_tag_name((A, "prstGeom"))) else {
        return Ok(None);
    };
    let name = geom.attribute("prst").ok_or("缺少形状预设名称")?;
    let defaults = preset::defaults(name).ok_or("不支持的形状预设名称")?;
    if geom
        .attributes()
        .any(|a| a.name() != "prst" || a.namespace().is_some())
    {
        return Err("预设包含未知几何属性".into());
    }
    if geom
        .children()
        .filter(|n| n.is_element())
        .any(|n| !n.has_tag_name((A, "avLst")))
    {
        return Err("预设包含未知几何子节点".into());
    }
    let mut adjustments = vec![];
    let mut seen = std::collections::BTreeSet::new();
    if geom
        .children()
        .filter(|n| n.has_tag_name((A, "avLst")))
        .count()
        > 1
    {
        return Err("重复调整列表".into());
    }
    for list in geom.children().filter(|n| n.has_tag_name((A, "avLst"))) {
        if list.attributes().len() != 0 {
            return Err("调整列表含未知属性".into());
        }
        for gd in list.children().filter(|n| n.is_element()) {
            if !gd.has_tag_name((A, "gd")) {
                return Err("预设包含未知调整节点".into());
            }
            if gd.children().any(|n| n.is_element())
                || gd
                    .attributes()
                    .any(|a| !["name", "fmla"].contains(&a.name()) || a.namespace().is_some())
            {
                return Err("调整参数含未知属性或子节点".into());
            }
            let key = gd.attribute("name").ok_or("调整参数缺少名称")?;
            let index = if key == "adj" && defaults.len() == 1 {
                0
            } else {
                key.strip_prefix("adj")
                    .and_then(|v| v.parse::<usize>().ok())
                    .and_then(|n| n.checked_sub(1))
                    .ok_or("调整参数名称不受支持")?
            };
            if index >= defaults.len() || !seen.insert(index) {
                return Err("未知或重复的调整参数".into());
            }
            let mut formula = gd
                .attribute("fmla")
                .ok_or("调整参数缺少公式")?
                .split_whitespace();
            if formula.next() != Some("val") {
                return Err("调整参数公式仅支持字面 val".into());
            }
            let value = formula
                .next()
                .ok_or("缺少调整值")?
                .parse::<i64>()
                .map_err(|_| "无效调整值")? as f64;
            if formula.next().is_some() {
                return Err("调整公式有多余操作数".into());
            }
            if adjustments.is_empty() {
                adjustments = defaults.to_vec();
            }
            adjustments[index] = value;
        }
    }
    if adjustments.is_empty()
        && ["rect", "ellipse", "roundRect", "rightArrow", "line"].contains(&name)
    {
        return Ok(None);
    }
    let preset = Preset {
        name: name.into(),
        adjustments,
        no_fill: props.children().any(|n| n.has_tag_name((A, "noFill"))),
    };
    preset.validate()?;
    Ok(Some(preset))
}

pub fn geometry_xml(p: &Preset) -> Result<String> {
    p.validate()?;
    let count = preset::defaults(&p.name).ok_or("未知预设")?.len();
    let mut guides = String::new();
    for (i, value) in p.adjustments.iter().enumerate() {
        let key = if count == 1 {
            "adj".to_owned()
        } else {
            format!("adj{}", i + 1)
        };
        guides.push_str(&format!(
            r#"<a:gd name="{key}" fmla="val {}"/>"#,
            value.round() as i64
        ));
    }
    // textBox is a library alias; public DrawingML uses the rect preset.
    let name = if p.name == "textBox" { "rect" } else { &p.name };
    Ok(format!(
        r#"<a:prstGeom prst="{}"><a:avLst>{guides}</a:avLst></a:prstGeom>"#,
        name
    ))
}

#[cfg(test)]
#[path = "preset_pptx/tests.rs"]
mod tests;
