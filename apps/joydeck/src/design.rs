//! Independent master/layout/theme cascade. Stored values never become resolved RGB.
use crate::document::{Deck, Object, Result, Slide, color};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Design {
    pub themes: BTreeMap<String, Theme>,
    pub masters: BTreeMap<String, Master>,
    pub layouts: BTreeMap<String, Layout>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub name: String,
    pub colors: BTreeMap<String, String>,
    pub fonts: BTreeMap<String, String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Master {
    pub name: String,
    pub theme: String,
    pub background: Option<String>,
    pub placeholders: Vec<Placeholder>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub name: String,
    pub master: String,
    pub background: Option<String>,
    pub placeholders: Vec<Placeholder>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placeholder {
    pub kind: String,
    pub index: u32,
    pub style: Layer,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub frame: Option<[f32; 4]>,
    pub fill: Option<String>,
    pub size: Option<f32>,
    pub family: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub kind: String,
    pub index: u32,
    pub direct: Layer,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Organization {
    pub author: String,
    pub subject: String,
    pub sections: BTreeMap<String, String>,
}
fn label(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
}
fn valid_paint(s: &str, theme: &Theme) -> bool {
    color(s)
        || s.strip_prefix('@')
            .is_some_and(|s| theme.colors.contains_key(s))
}
fn validate_layer(layer: &Layer, theme: &Theme) -> Result<()> {
    if layer.frame.is_some_and(|a| {
        !a.iter().all(|v| v.is_finite() && v.abs() <= 10000.) || a[2] <= 0. || a[3] <= 0.
    }) || layer.fill.as_ref().is_some_and(|s| !valid_paint(s, theme))
        || layer
            .size
            .is_some_and(|s| !s.is_finite() || !(1. ..=512.).contains(&s))
        || layer.family.as_ref().is_some_and(|s| {
            !label(s)
                || s.strip_prefix('@')
                    .is_some_and(|s| !theme.fonts.contains_key(s))
        })
    {
        return Err("无效继承样式/主题引用".into());
    }
    Ok(())
}
fn placeholders(items: &[Placeholder], theme: &Theme) -> Result<()> {
    let mut keys = BTreeSet::new();
    if items.len() > 128 {
        return Err("占位符超过128项".into());
    }
    for p in items {
        if !matches!(
            p.kind.as_str(),
            "title" | "body" | "footer" | "number" | "date"
        ) || p.index > 10000
            || !keys.insert((&p.kind, p.index))
        {
            return Err("占位符kind/idx非法或重复".into());
        }
        validate_layer(&p.style, theme)?;
    }
    Ok(())
}
pub fn validate(deck: &Deck) -> Result<()> {
    let d = &deck.design;
    if d.themes.len() > 128
        || d.masters.len() > 128
        || d.layouts.len() > 512
        || deck.organization.sections.len() > 500
    {
        return Err("设计/节数量超限".into());
    }
    for (id, t) in &d.themes {
        if !label(id)
            || !label(&t.name)
            || t.colors.len() > 64
            || t.fonts.len() > 16
            || t.colors.iter().any(|(k, v)| !label(k) || !color(v))
            || t.fonts.iter().any(|(k, v)| !label(k) || !label(v))
        {
            return Err("无效主题".into());
        }
    }
    for (id, m) in &d.masters {
        let t = d.themes.get(&m.theme).ok_or("母版主题不存在")?;
        if !label(id)
            || !label(&m.name)
            || m.background.as_ref().is_some_and(|s| !valid_paint(s, t))
        {
            return Err("无效母版".into());
        }
        placeholders(&m.placeholders, t)?;
    }
    for (id, l) in &d.layouts {
        let m = d.masters.get(&l.master).ok_or("版式母版不存在")?;
        let t = &d.themes[&m.theme];
        if !label(id)
            || !label(&l.name)
            || l.background.as_ref().is_some_and(|s| !valid_paint(s, t))
        {
            return Err("无效版式".into());
        }
        placeholders(&l.placeholders, t)?;
    }
    if deck.field_date.len() > 256
        || deck.field_date.chars().any(char::is_control)
        || deck.title.len() > 4096
        || deck.organization.author.len() > 4096
        || deck.organization.subject.len() > 16384
        || deck
            .organization
            .sections
            .iter()
            .any(|(id, n)| !label(id) || !label(n))
    {
        return Err("文档属性/节名称超限".into());
    }
    for s in &deck.slides {
        if s.notes.len() > 100000
            || s.section
                .as_ref()
                .is_some_and(|id| !deck.organization.sections.contains_key(id))
        {
            return Err("备注超限或节引用失效".into());
        }
        let layout = s
            .layout
            .as_ref()
            .map(|id| d.layouts.get(id).ok_or("页面版式不存在"))
            .transpose()?;
        let mut seen = BTreeSet::new();
        for o in &s.objects {
            if o.field.as_ref().is_some_and(|f| {
                o.kind != "text"
                    || !matches!(f.as_str(), "number" | "date")
                    || o.binding.as_ref().is_none_or(|b| b.kind != *f)
            }) {
                return Err("无效动态字段".into());
            }
            if let Some(b) = &o.binding {
                let l = layout.ok_or("占位符必须引用版式")?;
                if !l
                    .placeholders
                    .iter()
                    .chain(&d.masters[&l.master].placeholders)
                    .any(|p| p.kind == b.kind && p.index == b.index)
                    || !seen.insert((&b.kind, b.index))
                    || o.kind != "text"
                {
                    return Err("占位符匹配失效/重复/非文字".into());
                }
                validate_layer(&b.direct, &d.themes[&d.masters[&l.master].theme])?;
            }
        }
    }
    Ok(())
}
fn paint(value: &str, t: &Theme) -> String {
    value
        .strip_prefix('@')
        .and_then(|k| t.colors.get(k))
        .map_or_else(|| value.to_owned(), Clone::clone)
}
fn apply(o: &mut Object, layer: &Layer, t: &Theme) {
    if let Some([x, y, w, h]) = layer.frame {
        o.x = x;
        o.y = y;
        o.width = w;
        o.height = h;
    }
    if let Some(s) = &layer.fill {
        o.fill = paint(s, t);
    }
    if let Some(v) = layer.size {
        o.font_size = v;
    }
}
pub fn resolve_slide(deck: &Deck, page: usize) -> Result<Slide> {
    let mut s = deck.slides.get(page).ok_or("页面不存在")?.clone();
    let Some(id) = &s.layout else {
        return Ok(s);
    };
    let l = deck.design.layouts.get(id).ok_or("页面版式不存在")?;
    let m = deck.design.masters.get(&l.master).ok_or("母版不存在")?;
    let t = deck.design.themes.get(&m.theme).ok_or("主题不存在")?;
    if s.inherit_background {
        s.background = paint(
            l.background
                .as_ref()
                .or(m.background.as_ref())
                .map_or("#ffffff", String::as_str),
            t,
        );
    }
    for o in &mut s.objects {
        if let Some(field) = &o.field {
            o.text = match field.as_str() {
                "number" => (page + 1).to_string(),
                "date" => deck.field_date.clone(),
                _ => o.text.clone(),
            };
            o.runs.clear();
        }
        let Some(b) = o.binding.clone() else {
            continue;
        };
        let mut family = None;
        for layer in m
            .placeholders
            .iter()
            .chain(&l.placeholders)
            .filter(|p| p.kind == b.kind && p.index == b.index)
            .map(|p| &p.style)
            .chain(std::iter::once(&b.direct))
        {
            apply(o, layer, t);
            if let Some(f) = &layer.family {
                family = Some(
                    f.strip_prefix('@')
                        .and_then(|k| t.fonts.get(k))
                        .unwrap_or(f)
                        .clone(),
                );
            }
        }
        if o.runs.is_empty()
            && !o.text.is_empty()
            && let Some(family) = family
        {
            let mut style = crate::text::Style::for_object(o);
            style.family = family;
            o.runs = vec![crate::text::Run {
                start: 0,
                end: o.text.len(),
                style,
            }];
        }
    }
    Ok(s)
}
/// Apply/reapply a layout by stable kind/idx, preserving all existing text and IDs.
pub fn apply_layout(deck: &mut Deck, page: usize, id: &str, reset: bool) -> Result<()> {
    let l = deck.design.layouts.get(id).ok_or("版式不存在")?.clone();
    let m = deck
        .design
        .masters
        .get(&l.master)
        .ok_or("母版不存在")?
        .clone();
    let slide = deck.slides.get_mut(page).ok_or("页面不存在")?;
    slide.layout = Some(id.into());
    slide.inherit_background = true;
    let mut keys = BTreeSet::new();
    for p in m.placeholders.iter().chain(&l.placeholders) {
        if !keys.insert((p.kind.clone(), p.index)) {
            continue;
        }
        if let Some(o) = slide.objects.iter_mut().find(|o| {
            o.binding
                .as_ref()
                .is_some_and(|b| b.kind == p.kind && b.index == p.index)
        }) {
            if reset {
                o.binding.as_mut().unwrap().direct = Layer::default();
                o.runs.clear();
            }
        } else {
            let mut o = crate::edit::object("text");
            o.text.clear();
            o.binding = Some(Binding {
                kind: p.kind.clone(),
                index: p.index,
                direct: Layer::default(),
            });
            if matches!(p.kind.as_str(), "number" | "date") {
                o.field = Some(p.kind.clone());
            }
            slide.objects.push(o);
        }
    }
    // Unmatched objects become ordinary text; resolve with the previous layout before calling this for visual preservation.
    for o in &mut slide.objects {
        if o.binding
            .as_ref()
            .is_some_and(|b| !keys.contains(&(b.kind.clone(), b.index)))
        {
            o.binding = None;
            o.field = None;
        }
    }
    Ok(())
}
pub fn starter() -> Design {
    let theme = Theme {
        name: "JoyDeck 原创".into(),
        colors: BTreeMap::from([
            ("accent".into(), "#087f8c".into()),
            ("background".into(), "#ffffff".into()),
        ]),
        fonts: BTreeMap::from([("body".into(), "sans-serif".into())]),
    };
    let title = Placeholder {
        kind: "title".into(),
        index: 0,
        style: Layer {
            frame: Some([60., 45., 840., 90.]),
            fill: Some("@accent".into()),
            size: Some(40.),
            family: Some("@body".into()),
        },
    };
    let body = Placeholder {
        kind: "body".into(),
        index: 0,
        style: Layer {
            frame: Some([60., 160., 840., 280.]),
            fill: Some("#202b3a".into()),
            size: Some(24.),
            family: Some("@body".into()),
        },
    };
    Design {
        themes: BTreeMap::from([("theme-1".into(), theme)]),
        masters: BTreeMap::from([(
            "master-1".into(),
            Master {
                name: "默认母版".into(),
                theme: "theme-1".into(),
                background: Some("@background".into()),
                placeholders: vec![title],
            },
        )]),
        layouts: BTreeMap::from([(
            "layout-1".into(),
            Layout {
                name: "标题与正文".into(),
                master: "master-1".into(),
                background: None,
                placeholders: vec![body],
            },
        )]),
    }
}
