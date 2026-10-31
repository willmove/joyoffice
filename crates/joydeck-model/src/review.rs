//! Offline review state with stable anchors. Playback and selection never enter this model.
use crate::document::{Deck, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub threads: Vec<Thread>,
    pub accessibility: BTreeMap<String, Access>,
    pub reading_order: BTreeMap<String, Vec<String>>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thread {
    pub id: String,
    pub slide: String,
    pub object: Option<String>,
    pub resolved: bool,
    pub messages: Vec<Message>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub author: String,
    pub text: String,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Access {
    pub alt: String,
    pub decorative: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Action {
    Add {
        id: String,
        slide: String,
        object: Option<String>,
        author: String,
        text: String,
    },
    Reply {
        id: String,
        author: String,
        text: String,
    },
    Resolve {
        id: String,
        resolved: bool,
    },
    Accessibility {
        object: String,
        alt: String,
        decorative: bool,
    },
    Order {
        slide: String,
        objects: Vec<String>,
    },
}
#[derive(Clone, Debug, Serialize)]
pub struct Finding {
    pub code: &'static str,
    pub slide: String,
    pub object: Option<String>,
    pub message: String,
}
fn message(author: String, text: String) -> Result<Message> {
    if author.trim().is_empty() || author.len() > 256 || text.trim().is_empty() || text.len() > 8192
    {
        return Err("作者或评论为空/超限".into());
    }
    Ok(Message { author, text })
}
impl Review {
    pub fn validate(&self, deck: &Deck) -> Result<()> {
        if self.threads.len() > 1024
            || self.accessibility.len() > 50000
            || self.reading_order.len() > 500
        {
            return Err("审阅预算超限".into());
        }
        let mut ids = BTreeSet::new();
        let mut bytes = 0;
        for t in &self.threads {
            if t.id.is_empty()
                || t.id.len() > 128
                || !ids.insert(&t.id)
                || t.messages.is_empty()
                || t.messages.len() > 128
            {
                return Err("评论线程无效".into());
            }
            let s = deck
                .slides
                .iter()
                .find(|s| s.id == t.slide)
                .ok_or("评论页不存在")?;
            if t.object
                .as_ref()
                .is_some_and(|id| !s.objects.iter().any(|o| &o.id == id))
            {
                return Err("评论对象不存在".into());
            }
            for m in &t.messages {
                message(m.author.clone(), m.text.clone())?;
                bytes += m.author.len() + m.text.len();
            }
        }
        for (id, a) in &self.accessibility {
            if a.alt.len() > 8192
                || !deck
                    .slides
                    .iter()
                    .any(|s| s.objects.iter().any(|o| &o.id == id))
            {
                return Err("替代文字锚点或长度无效".into());
            }
            bytes += a.alt.len();
        }
        for (id, order) in &self.reading_order {
            let s = deck
                .slides
                .iter()
                .find(|s| &s.id == id)
                .ok_or("阅读顺序页不存在")?;
            if order.len() != s.objects.len()
                || order.iter().collect::<BTreeSet<_>>().len() != order.len()
                || order
                    .iter()
                    .any(|id| !s.objects.iter().any(|o| &o.id == id))
            {
                return Err("阅读顺序必须完整且不重复".into());
            }
        }
        if bytes > 1024 * 1024 {
            return Err("审阅正文超过1MiB".into());
        }
        Ok(())
    }
}
pub fn apply(d: &mut Deck, a: Action) -> Result<()> {
    match a {
        Action::Add {
            id,
            slide,
            object,
            author,
            text,
        } => {
            if d.review.threads.iter().any(|t| t.id == id) {
                return Err("评论ID已存在".into());
            }
            d.review.threads.push(Thread {
                id,
                slide,
                object,
                resolved: false,
                messages: vec![message(author, text)?],
            });
        }
        Action::Reply { id, author, text } => d
            .review
            .threads
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("评论不存在")?
            .messages
            .push(message(author, text)?),
        Action::Resolve { id, resolved } => {
            d.review
                .threads
                .iter_mut()
                .find(|t| t.id == id)
                .ok_or("评论不存在")?
                .resolved = resolved
        }
        Action::Accessibility {
            object,
            alt,
            decorative,
        } => {
            d.review
                .accessibility
                .insert(object, Access { alt, decorative });
        }
        Action::Order { slide, objects } => {
            d.review.reading_order.insert(slide, objects);
        }
    }
    d.review.validate(d)
}
/// Called inside the same transaction as deletion. Undo restores both anchors and objects.
pub fn reconcile(d: &mut Deck) {
    d.review.threads.retain(|t| {
        d.slides.iter().any(|s| {
            s.id == t.slide
                && t.object
                    .as_ref()
                    .is_none_or(|id| s.objects.iter().any(|o| &o.id == id))
        })
    });
    d.review.accessibility.retain(|id, _| {
        d.slides
            .iter()
            .any(|s| s.objects.iter().any(|o| &o.id == id))
    });
    d.review
        .reading_order
        .retain(|id, _| d.slides.iter().any(|s| &s.id == id));
    for s in &d.slides {
        if let Some(order) = d.review.reading_order.get_mut(&s.id) {
            order.retain(|id| s.objects.iter().any(|o| &o.id == id));
            for o in &s.objects {
                if !order.contains(&o.id) {
                    order.push(o.id.clone());
                }
            }
        }
    }
}
pub fn check(d: &Deck) -> Vec<Finding> {
    let mut out = vec![];
    for s in &d.slides {
        if !s.objects.iter().any(|o| {
            matches!(o.kind.as_str(), "text" | "placeholder")
                && !o.text.trim().is_empty()
                && o.y < d.height * 0.3
        }) {
            out.push(Finding {
                code: "missing-title",
                slide: s.id.clone(),
                object: None,
                message: "页面缺少顶部可见标题".into(),
            });
        }
        for o in &s.objects {
            let a = d.review.accessibility.get(&o.id);
            if matches!(o.kind.as_str(), "image" | "chart" | "diagram")
                && a.is_none_or(|a| !a.decorative && a.alt.trim().is_empty())
            {
                out.push(Finding {
                    code: "missing-alt",
                    slide: s.id.clone(),
                    object: Some(o.id.clone()),
                    message: "对象缺少替代文字".into(),
                });
            }
            if !o.text.is_empty() && o.opacity < 0.4 {
                out.push(Finding {
                    code: "low-opacity",
                    slide: s.id.clone(),
                    object: Some(o.id.clone()),
                    message: "文字透明度较低，请核查对比度".into(),
                });
            }
        }
    }
    out
}
