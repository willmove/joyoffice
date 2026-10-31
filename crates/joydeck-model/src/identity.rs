use crate::document::{Deck, Object, Slide};
use std::sync::atomic::{AtomicU64, Ordering};
pub static IDS: AtomicU64 = AtomicU64::new(1);
pub fn normalize_ids(d: &mut Deck) {
    let mut ids = std::collections::BTreeSet::new();
    for s in &d.slides {
        for o in &s.objects {
            if !o.id.is_empty() {
                ids.insert(o.id.clone());
            }
        }
    }
    for s in &mut d.slides {
        for o in &mut s.objects {
            if o.id.is_empty() {
                loop {
                    let id = format!("object-{}", IDS.fetch_add(1, Ordering::Relaxed));
                    if ids.insert(id.clone()) {
                        o.id = id;
                        break;
                    }
                }
            }
        }
    }
}
pub fn object(kind: &str) -> Object {
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
        kind: kind.into(),
        x: 80.,
        y: 80.,
        width: 240.,
        height: 100.,
        fill: "#27b6a7".into(),
        text: if kind == "text" {
            "输入文字".into()
        } else {
            String::new()
        },
        font_size: 24.,
        runs: vec![],
    }
}

pub fn blank() -> Deck {
    Deck {
        playback: Default::default(),
        review: Default::default(),
        presentation: Default::default(),
        source: None,
        resources: Default::default(),
        design: Default::default(),
        organization: Default::default(),
        field_date: String::new(),
        title: "未命名文稿".into(),
        width: 960.,
        height: 540.,
        slides: vec![Slide {
            hidden: false,
            layout: None,
            inherit_background: false,
            notes: String::new(),
            section: None,
            id: "page-1".into(),
            title: "第1页".into(),
            background: "#ffffff".into(),
            objects: vec![],
        }],
        warnings: vec![],
    }
}
