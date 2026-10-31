use crate::{
    design::{self, Design, Organization},
    document::{Deck, Result},
    text,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone)]
pub enum Action {
    SetDesign(Design),
    ApplyLayout(usize, String, bool),
    DeleteLayout(String, Option<String>),
    Notes(usize, String),
    Properties(String, Organization, String),
    Section(Vec<usize>, String, String),
    DeleteSection(String),
    ReplaceAll(String, String),
    ReplaceFont(String, String),
    PastePages(usize, PageTransfer, Option<String>),
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageTransfer {
    pub version: u32,
    pub deck: Box<Deck>,
}
impl PageTransfer {
    pub fn capture(deck: &Deck, pages: &[usize]) -> Result<Self> {
        let mut d = deck.clone();
        let mut seen = std::collections::BTreeSet::new();
        d.slides = pages
            .iter()
            .map(|i| {
                if !seen.insert(*i) {
                    return Err("重复页索引".into());
                }
                deck.slides.get(*i).cloned().ok_or("页不存在".into())
            })
            .collect::<Result<_>>()?;
        crate::motion::reconcile(&mut d);
        crate::media::collect_unused(&mut d);
        d.validate()?;
        Ok(Self {
            version: 1,
            deck: Box::new(d),
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub page: usize,
    pub object: String,
    pub range: std::ops::Range<usize>,
}
pub fn find(deck: &Deck, needle: &str) -> Result<Vec<Hit>> {
    if needle.is_empty() || needle.len() > 100000 {
        return Err("查找内容为空或超限".into());
    }
    let mut hits = vec![];
    for (page, s) in deck.slides.iter().enumerate() {
        for o in &s.objects {
            if o.field.is_some() {
                continue;
            }
            for (at, _) in o.text.match_indices(needle) {
                hits.push(Hit {
                    page,
                    object: o.id.clone(),
                    range: at..at + needle.len(),
                });
                if hits.len() > 10000 {
                    return Err("匹配超过10000项，请缩小查找范围".into());
                }
            }
        }
    }
    Ok(hits)
}
fn fresh(prefix: &str, next: &mut u64) -> Result<String> {
    *next = next.checked_add(1).ok_or("ID耗尽")?;
    Ok(format!("{prefix}-{next}"))
}
pub(crate) fn apply(d: &mut Deck, a: Action, next: &mut u64) -> Result<()> {
    match a {
        Action::SetDesign(design) => d.design = design,
        Action::ApplyLayout(page, id, reset) => {
            let old = design::resolve_slide(d, page)?;
            design::apply_layout(d, page, &id, reset)?;
            for o in &mut d.slides[page].objects {
                if o.binding.is_none()
                    && let Some(resolved) = old.objects.iter().find(|old| old.id == o.id)
                {
                    *o = resolved.clone();
                    o.binding = None;
                    o.field = None;
                }
            }
        }
        Action::DeleteLayout(id, replacement) => {
            if !d.design.layouts.contains_key(&id) {
                return Err("版式不存在".into());
            }
            if replacement.as_ref() == Some(&id) {
                return Err("替代版式不能是自身".into());
            }
            let pages: Vec<_> = d
                .slides
                .iter()
                .enumerate()
                .filter(|(_, s)| s.layout.as_ref() == Some(&id))
                .map(|(i, _)| i)
                .collect();
            if !pages.is_empty() && replacement.is_none() {
                return Err("版式正被使用，请先提供替代版式".into());
            }
            for p in pages {
                apply(
                    d,
                    Action::ApplyLayout(p, replacement.clone().ok_or("替代版式不存在")?, false),
                    next,
                )?;
            }
            d.design.layouts.remove(&id);
        }
        Action::Notes(page, notes) => d.slides.get_mut(page).ok_or("页面不存在")?.notes = notes,
        Action::Properties(title, org, date) => {
            d.title = title;
            d.organization = org;
            d.field_date = date;
        }
        Action::Section(pages, id, name) => {
            d.organization.sections.insert(id.clone(), name);
            for p in pages {
                d.slides.get_mut(p).ok_or("页面不存在")?.section = Some(id.clone());
            }
        }
        Action::DeleteSection(id) => {
            if d.organization.sections.remove(&id).is_none() {
                return Err("节不存在".into());
            }
            for s in &mut d.slides {
                if s.section.as_ref() == Some(&id) {
                    s.section = None;
                }
            }
        }
        Action::ReplaceAll(needle, value) => {
            let hits = find(d, &needle)?;
            if value.len() > 100000 {
                return Err("替换文字超限".into());
            }
            for hit in hits.into_iter().rev() {
                let o = d.slides[hit.page]
                    .objects
                    .iter_mut()
                    .find(|o| o.id == hit.object)
                    .ok_or("查找目标失效")?;
                if o.runs.is_empty() {
                    o.text.replace_range(hit.range, &value);
                    continue;
                }
                let style = text::runs(o)
                    .into_iter()
                    .find(|r| r.start <= hit.range.start && r.end > hit.range.start)
                    .map_or_else(|| text::Style::for_object(o), |r| r.style);
                text::replace(o, hit.range, &value, &style)?;
            }
        }
        Action::ReplaceFont(from, to) => {
            if from.is_empty() || to.trim().is_empty() || to.len() > 256 {
                return Err("字体名称无效".into());
            }
            for t in d.design.themes.values_mut() {
                for family in t.fonts.values_mut() {
                    if family == &from {
                        family.clone_from(&to);
                    }
                }
            }
            for layer in d
                .design
                .masters
                .values_mut()
                .flat_map(|m| &mut m.placeholders)
                .chain(
                    d.design
                        .layouts
                        .values_mut()
                        .flat_map(|l| &mut l.placeholders),
                )
            {
                if layer.style.family.as_ref() == Some(&from) {
                    layer.style.family = Some(to.clone());
                }
            }
            for o in d.slides.iter_mut().flat_map(|s| &mut s.objects) {
                if o.runs.is_empty() && !o.text.is_empty() && from == "sans-serif" {
                    o.runs = text::runs(o);
                }
                for r in &mut o.runs {
                    if r.style.family == from {
                        r.style.family.clone_from(&to);
                    }
                }
                if let Some(b) = &mut o.binding
                    && b.direct.family.as_ref() == Some(&from)
                {
                    b.direct.family = Some(to.clone());
                }
            }
        }
        Action::PastePages(at, transfer, target_theme) => {
            if transfer.version != 1 || at > d.slides.len() {
                return Err("页剪贴板版本或插入位置无效".into());
            }
            transfer.deck.validate()?;
            let mut source = *transfer.deck;
            if target_theme
                .as_ref()
                .is_some_and(|id| !d.design.themes.contains_key(id))
            {
                return Err("目标主题不存在".into());
            }
            let mut themes = BTreeMap::new();
            let mut masters = BTreeMap::new();
            let mut layouts = BTreeMap::new();
            let mut sections = BTreeMap::new();
            for (id, t) in source.design.themes {
                let mut new = fresh("theme", next)?;
                while d.design.themes.contains_key(&new) {
                    new = fresh("theme", next)?;
                }
                d.design.themes.insert(new.clone(), t);
                themes.insert(id, new);
            }
            let sx = d.width / source.width;
            let sy = d.height / source.height;
            let scale = |l: &mut design::Layer| {
                if let Some([x, y, w, h]) = l.frame {
                    l.frame = Some([x * sx, y * sy, w * sx, h * sy]);
                }
                if let Some(v) = l.size {
                    l.size = Some(v * sx.min(sy));
                }
            };
            for (id, mut m) in source.design.masters {
                let mut new = fresh("master", next)?;
                while d.design.masters.contains_key(&new) {
                    new = fresh("master", next)?;
                }
                m.theme = target_theme
                    .clone()
                    .unwrap_or_else(|| themes[&m.theme].clone());
                for p in &mut m.placeholders {
                    scale(&mut p.style);
                }
                d.design.masters.insert(new.clone(), m);
                masters.insert(id, new);
            }
            for (id, mut l) in source.design.layouts {
                let mut new = fresh("layout", next)?;
                while d.design.layouts.contains_key(&new) {
                    new = fresh("layout", next)?;
                }
                l.master = masters[&l.master].clone();
                for p in &mut l.placeholders {
                    scale(&mut p.style);
                }
                d.design.layouts.insert(new.clone(), l);
                layouts.insert(id, new);
            }
            for (id, name) in source.organization.sections {
                let mut new = fresh("section", next)?;
                while d.organization.sections.contains_key(&new) {
                    new = fresh("section", next)?;
                }
                d.organization.sections.insert(new.clone(), name);
                sections.insert(id, new);
            }
            let mut motion_map = BTreeMap::new();
            let mut object_map = BTreeMap::new();
            let mut page_map = BTreeMap::new();
            let mut used_objects = d
                .slides
                .iter()
                .chain(&source.slides)
                .flat_map(|s| &s.objects)
                .map(|o| o.id.clone())
                .collect::<std::collections::BTreeSet<_>>();
            let mut used_pages = d
                .slides
                .iter()
                .chain(&source.slides)
                .map(|s| s.id.clone())
                .collect::<std::collections::BTreeSet<_>>();
            for s in &mut source.slides {
                let mut id = fresh("page", next)?;
                while !used_pages.insert(id.clone()) {
                    id = fresh("page", next)?;
                }
                let mut timing = source.presentation.slides.remove(&s.id);
                page_map.insert(s.id.clone(), id.clone());
                s.id = id;
                s.layout = s.layout.as_ref().map(|id| layouts[id].clone());
                s.section = s.section.as_ref().map(|id| sections[id].clone());
                for o in &mut s.objects {
                    let old = o.id.clone();
                    o.id = fresh("object", next)?;
                    while !used_objects.insert(o.id.clone()) {
                        o.id = fresh("object", next)?;
                    }
                    object_map.insert(old.clone(), o.id.clone());
                    if let Some(t) = &mut timing {
                        for e in &mut t.effects {
                            if e.target == old {
                                e.target = o.id.clone();
                                e.dx *= sx;
                                e.dy *= sy;
                            }
                        }
                    }
                    o.x *= sx;
                    o.y *= sy;
                    o.width *= sx;
                    o.height *= sy;
                    crate::table::resize(o);
                    crate::table::scale_content(o, sx.min(sy));
                    o.font_size *= sx.min(sy);
                    for r in &mut o.runs {
                        r.style.size *= sx.min(sy);
                    }
                    if let Some(b) = &mut o.binding {
                        scale(&mut b.direct);
                    }
                }
                if let Some(t) = timing {
                    motion_map.insert(s.id.clone(), t);
                }
            }
            d.presentation.slides.extend(motion_map);
            for (object, page) in source.presentation.links {
                if let (Some(object), Some(page)) = (object_map.get(&object), page_map.get(&page)) {
                    d.presentation.links.insert(object.clone(), page.clone());
                }
            }
            for (id, r) in source.resources {
                d.resources.entry(id).or_insert_with(|| Arc::clone(&r));
            }
            d.slides.splice(at..at, source.slides);
        }
    }
    Ok(())
}
/// Convert changes to effective geometry/format into explicit slide overrides.
pub(crate) fn prepare_update(
    deck: &Deck,
    page: usize,
    o: &mut crate::document::Object,
) -> Result<()> {
    let Some(binding) = o.binding.as_mut() else {
        return Ok(());
    };
    let old = design::resolve_slide(deck, page)?
        .objects
        .into_iter()
        .find(|old| old.id == o.id)
        .ok_or("占位符已失效")?;
    if [o.x, o.y, o.width, o.height] != [old.x, old.y, old.width, old.height] {
        binding.direct.frame = Some([o.x, o.y, o.width, o.height]);
    }
    if o.fill != old.fill {
        binding.direct.fill = Some(o.fill.clone());
    }
    if o.font_size != old.font_size {
        binding.direct.size = Some(o.font_size);
    }
    Ok(())
}
