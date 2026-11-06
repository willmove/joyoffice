//! Preserve original rich text while editing its click actions.
use super::*;
use joydeck_model::text::{Action, Hyperlink, Paragraph, safe_url};
use std::ops::Range;

pub(super) struct Relationships<'a> {
    pub source: &'a opc::Rels,
    pub generated: &'a opc::Rels,
    pub generated_slides: &'a BTreeMap<String, SlideId>,
    pub occupied: &'a std::collections::BTreeSet<String>,
    pub part: &'a str,
    pub destinations: &'a BTreeMap<SlideId, (usize, String)>,
    pub added: &'a mut Vec<El>,
}

impl Relationships<'_> {
    fn add(&mut self, kind: &str, absolute: &str, external: bool) -> String {
        if let Some(existing) = self.source.items.iter().find(|r| {
            r.kind == kind
                && r.external == external
                && if external {
                    r.target == absolute
                } else {
                    opc::norm(&r.target) == opc::norm(absolute)
                }
        }) {
            return existing.id.clone();
        }
        let target = if external {
            absolute.to_owned()
        } else {
            opc::relative(self.part, absolute)
        };
        let ty = format!("{}{kind}", opc::RT_BASE);
        if let Some(existing) = self
            .added
            .iter()
            .find(|r| {
                r.attr("Type") == Some(&ty)
                    && r.attr("Target") == Some(&target)
                    && (r.attr("TargetMode") == Some("External")) == external
            })
            .and_then(|r| r.attr("Id"))
        {
            return existing.into();
        }
        let mut id = format!("rIdJoyDeckLink{}", self.added.len() + 1);
        while self.occupied.contains(&id) || self.added.iter().any(|r| r.attr("Id") == Some(&id)) {
            id.push('x');
        }
        let mut relationship = rel_element(&id, kind, &target);
        if external {
            relationship.set_attr("TargetMode", "External");
        }
        self.added.push(relationship);
        id
    }

    fn slide(&mut self, id: SlideId) -> Result<String, PptxError> {
        let (_, path) = self
            .destinations
            .get(&id)
            .ok_or_else(|| error("link target slide no longer exists"))?;
        Ok(self.add("slide", path, false))
    }

    /// A new shape comes from the generated document, so its link IDs must be imported.
    pub(super) fn rebind_generated(&mut self, element: &mut El) -> Result<(), PptxError> {
        if element.prefix() == "a" && matches!(element.local(), "hlinkClick" | "hlinkMouseOver") {
            if element.child("snd").is_some() {
                return Err(error("linked sounds require .joydeck Save As"));
            }
            if let Some(id) = element.attr("r:id").filter(|s| !s.is_empty()) {
                let rel = self
                    .generated
                    .get(id)
                    .ok_or_else(|| error("generated hyperlink relationship missing"))?;
                let rebound = match (rel.kind.as_str(), rel.external) {
                    ("hyperlink", true) if safe_url(&rel.target) => {
                        self.add("hyperlink", &rel.target, true)
                    }
                    ("slide", false) => self.slide(
                        *self
                            .generated_slides
                            .get(&opc::norm(&rel.target))
                            .ok_or_else(|| error("generated hyperlink slide missing"))?,
                    )?,
                    _ => {
                        return Err(error(
                            "unsupported hyperlink relationship; save .joydeck instead",
                        ));
                    }
                };
                element.set_attr("r:id", &rebound);
            }
        }
        if !element.is("extLst") {
            for child in element.elements_mut() {
                self.rebind_generated(child)?;
            }
        }
        Ok(())
    }

    fn set_click(
        &mut self,
        run: &mut El,
        old: Option<&Hyperlink>,
        new: Option<&Hyperlink>,
    ) -> Result<(), PptxError> {
        let props = match child_mut(run, "rPr") {
            Some(props) => props,
            None => {
                run.children.insert(
                    0,
                    Node::El(El {
                        name: "a:rPr".into(),
                        ..El::default()
                    }),
                );
                child_mut(run, "rPr").ok_or_else(|| error("missing link run properties"))?
            }
        };
        let at = props
            .children
            .iter()
            .position(|n| matches!(n, Node::El(e) if e.is("hlinkClick")));
        let original = at.and_then(|i| match &props.children[i] {
            Node::El(e) => Some(e),
            _ => None,
        });
        if original.is_some() && old.is_none() {
            return Err(error("unknown original link action; save .joydeck instead"));
        }
        if let Some(old_xml) = original
            && new.is_none()
            && (old_xml.attrs.iter().any(|(key, _)| {
                !matches!(
                    key.as_str(),
                    "r:id" | "action" | "tooltip" | "highlightClick" | "endSnd"
                )
            }) || old_xml.elements().any(|e| !e.is("snd")))
        {
            return Err(error("the link has opaque metadata; save .joydeck instead"));
        }
        let Some(new) = new else {
            if let Some(at) = at {
                props.children.remove(at);
            }
            return Ok(());
        };
        if new.sound != old.and_then(|link| link.sound) || new.zoom_return.is_some() {
            return Err(error(
                "edited sound or return-to-zoom requires .joydeck Save As",
            ));
        }
        let (id, action) = match &new.action {
            Action::Url { url } if safe_url(url) => (self.add("hyperlink", url, true), None),
            Action::Slide { slide } => (self.slide(*slide)?, Some("ppaction://hlinksldjump")),
            _ => {
                return Err(error(
                    "unsupported edited link action; save .joydeck instead",
                ));
            }
        };
        let mut click = original.cloned().unwrap_or_else(|| El {
            name: "a:hlinkClick".into(),
            ..El::default()
        });
        // Update the relationship attribute in place when the source uses an alias.
        if let Some((_, value)) = click
            .attrs
            .iter_mut()
            .find(|(name, _)| name.ends_with(":id") && !name.starts_with("xmlns"))
        {
            *value = id;
        } else {
            click.set_attr("r:id", &id);
        }
        for (name, value) in [
            ("action", action),
            (
                "tooltip",
                (!new.tooltip.is_empty()).then_some(new.tooltip.as_str()),
            ),
            ("highlightClick", new.highlight_click.then_some("1")),
            ("endSnd", new.stop_sound.then_some("1")),
        ] {
            if let Some(value) = value {
                click.set_attr(name, value);
            } else {
                click.attrs.retain(|(key, _)| key != name);
            }
        }
        if let Some(at) = at {
            props.children[at] = Node::El(click);
        } else {
            let at = props.children.iter().position(|n| matches!(n, Node::El(e) if matches!(e.local(), "hlinkMouseOver" | "rtl" | "extLst")))
                .unwrap_or(props.children.len());
            props.children.insert(at, Node::El(click));
        }
        Ok(())
    }
}

fn without_links(para: &Paragraph) -> Paragraph {
    let mut result = para.clone();
    for run in &mut result.runs {
        run.props.link = None;
    }
    result.normalize();
    result
}

fn spans(para: &Paragraph) -> Vec<(Range<usize>, Option<&Hyperlink>)> {
    let mut spans: Vec<(Range<usize>, Option<&Hyperlink>)> = vec![];
    let mut offset = 0;
    for run in &para.runs {
        let length = run.content().len();
        if length == 0 {
            continue;
        }
        let link = run.props.link.as_ref();
        if let Some((range, prev)) = spans.last_mut()
            && *prev == link
        {
            range.end += length;
        } else {
            spans.push((offset..offset + length, link));
        }
        offset += length;
    }
    spans
}

fn patch_paragraph(
    para: &mut El,
    before: &Paragraph,
    after: &Paragraph,
    links: &mut Relationships<'_>,
) -> Result<(), PptxError> {
    let expected = before.text();
    let old = spans(before);
    let new = spans(after);
    let prefix = para.prefix().to_owned();
    let (mut i, mut j, mut offset) = (0, 0, 0);
    let mut patched = Vec::with_capacity(para.children.len());
    for node in std::mem::take(&mut para.children) {
        let Node::El(run) = &node else {
            patched.push(node);
            continue;
        };
        if run.prefix() != prefix && run.prefix() != "a" {
            patched.push(node);
            continue;
        }
        let text = match run.local() {
            "r" | "fld" => run.child("t").map(El::text).unwrap_or_default(),
            "br" => "\u{b}".into(),
            _ => {
                patched.push(node);
                continue;
            }
        };
        let end = offset + text.len();
        if expected.get(offset..end) != Some(text.as_str()) {
            return Err(error(
                "cannot map the original linked text; save .joydeck instead",
            ));
        }
        let mut fragments = vec![];
        while offset < end {
            while old.get(i).is_some_and(|(span, _)| span.end <= offset) {
                i += 1;
            }
            while new.get(j).is_some_and(|(span, _)| span.end <= offset) {
                j += 1;
            }
            let (old_span, old_link) = old
                .get(i)
                .ok_or_else(|| error("missing original link span"))?;
            let (new_span, new_link) = new
                .get(j)
                .ok_or_else(|| error("missing edited link span"))?;
            let next = end.min(old_span.end).min(new_span.end);
            fragments.push((offset..next, *old_link, *new_link));
            offset = next;
        }
        if fragments.iter().all(|(_, before, after)| before == after) {
            patched.push(node);
            continue;
        }
        if !run.is("r")
            || (fragments.len() > 1
                && (run.find("extLst").is_some()
                    || run.elements().any(|e| !matches!(e.local(), "rPr" | "t"))))
        {
            return Err(error(
                "the edited run has opaque or atomic content; save .joydeck instead",
            ));
        }
        let start = end - text.len();
        let split = fragments.len() > 1;
        for (span, before, after) in fragments {
            let mut piece = run.clone();
            if split {
                let t =
                    child_mut(&mut piece, "t").ok_or_else(|| error("missing original run text"))?;
                if t.elements().next().is_some() {
                    return Err(error("run text has extensions; save .joydeck instead"));
                }
                let s = text
                    .get(span.start - start..span.end - start)
                    .ok_or_else(|| error("invalid link split"))?;
                t.children = vec![Node::Text(s.into())];
            }
            if before != after {
                links.set_click(&mut piece, before, after)?;
            }
            patched.push(Node::El(piece));
        }
    }
    if offset != expected.len() {
        return Err(error("cannot map all original text; save .joydeck instead"));
    }
    para.children = patched;
    Ok(())
}

/// Patch links in the original run tree, retaining its formatting and unmodelled XML.
pub(super) fn patch(
    source: &mut El,
    before: Option<&TextBody>,
    after: Option<&TextBody>,
    links: &mut Relationships<'_>,
) -> Result<bool, PptxError> {
    let (Some(before), Some(after)) = (before, after) else {
        return Ok(false);
    };
    if before.list_style != after.list_style
        || before.paragraphs.len() != after.paragraphs.len()
        || before.paragraphs == after.paragraphs
        || before
            .paragraphs
            .iter()
            .zip(&after.paragraphs)
            .any(|(a, b)| a != b && without_links(a) != without_links(b))
    {
        return Ok(false);
    }
    let source =
        child_mut(source, "txBody").ok_or_else(|| error("missing original linked text body"))?;
    let paras = source
        .elements_mut()
        .filter(|e| e.is("p"))
        .collect::<Vec<_>>();
    if paras.len() != before.paragraphs.len() {
        return Err(error("cannot map linked paragraphs; save .joydeck instead"));
    }
    for ((para, before), after) in paras
        .into_iter()
        .zip(&before.paragraphs)
        .zip(&after.paragraphs)
    {
        if before != after {
            patch_paragraph(para, before, after, links)?;
        }
    }
    Ok(true)
}
