//! Retain the preceding edit states within the editor's history limits.
use crate::EditorSnapshot;
use joydeck_model::{Presentation, Shape, walk};
use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};

const MAX_UNDO_STEPS: usize = 128;
const UNDO_BYTES: usize = 64 << 20;

#[derive(Clone)]
struct RetainedEdit {
    snapshot: EditorSnapshot,
    bytes: usize,
}

#[derive(Clone, Default)]
pub(super) struct History {
    undo: VecDeque<RetainedEdit>,
    redo: Vec<EditorSnapshot>,
}

impl History {
    pub(super) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(super) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub(super) fn record(&mut self, live: &mut EditorSnapshot, next: EditorSnapshot) {
        self.push_undo(live, next);
        self.redo.clear();
        let mut total = self
            .undo
            .iter()
            .map(|entry| entry.bytes as u128)
            .sum::<u128>();
        while self.undo.len() > 1
            && (self.undo.len() > MAX_UNDO_STEPS || total > UNDO_BYTES as u128)
        {
            if let Some(oldest) = self.undo.pop_front() {
                total -= oldest.bytes as u128;
            }
        }
    }

    pub(super) fn undo(&mut self, live: &mut EditorSnapshot) -> bool {
        let Some(previous) = self.undo.pop_back() else {
            return false;
        };
        self.redo.push(std::mem::replace(live, previous.snapshot));
        true
    }

    pub(super) fn redo(&mut self, live: &mut EditorSnapshot) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.push_undo(live, next);
        true
    }

    fn push_undo(&mut self, live: &mut EditorSnapshot, next: EditorSnapshot) {
        let bytes = retained_bytes(&live.model, &next.model);
        self.undo.push_back(RetainedEdit {
            snapshot: std::mem::replace(live, next),
            bytes,
        });
    }
}

/// Keep the historical charges so the same edits reach the eviction limits.
/// Reordering an Arc preserves its allocation, irrespective of its index or ID.
fn retained_bytes(previous: &Presentation, current: &Presentation) -> usize {
    let slides = unshared(previous.slides.iter(), current.slides.iter(), |slide| {
        512usize.saturating_add(shape_bytes(&slide.shapes))
    });
    let masters = unshared(previous.masters.iter(), current.masters.iter(), |master| {
        master.layouts.iter().fold(
            4096usize.saturating_add(shape_bytes(&master.shapes)),
            |bytes, layout| {
                bytes
                    .saturating_add(512)
                    .saturating_add(shape_bytes(&layout.shapes))
            },
        )
    });
    let media = unshared(
        previous.media.iter().map(|item| &item.data),
        current.media.iter().map(|item| &item.data),
        Vec::len,
    );
    previous
        .slides
        .len()
        .saturating_mul(64)
        .saturating_add(slides)
        .saturating_add(masters)
        .saturating_add(media)
}

fn unshared<'a, T: 'a>(
    previous: impl Iterator<Item = &'a Arc<T>>,
    current: impl Iterator<Item = &'a Arc<T>>,
    bytes: impl Fn(&T) -> usize,
) -> usize {
    let current: HashSet<_> = current.map(Arc::as_ptr).collect();
    previous
        .filter(|value| !current.contains(&Arc::as_ptr(value)))
        .fold(0usize, |sum, value| sum.saturating_add(bytes(value)))
}

fn shape_bytes(shapes: &[Shape]) -> usize {
    let mut bytes = 0usize;
    walk(shapes, &mut |shape, _| {
        bytes = bytes.saturating_add(768);
        if let Some(xml) = &shape.raw_ext {
            bytes = bytes.saturating_add(xml.len());
        }
        if let Some(text) = &shape.text {
            for paragraph in &text.paragraphs {
                bytes = bytes.saturating_add(96);
                for run in &paragraph.runs {
                    bytes = bytes.saturating_add(128).saturating_add(run.text.len());
                }
            }
        }
    });
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use joydeck_model::{Layout, Master, MediaItem, ShapeKind, Slide};

    #[test]
    fn only_unshared_allocations_are_charged() {
        let child = Shape {
            raw_ext: Some("<x/>".into()),
            ..Default::default()
        };
        let slide = Arc::new(Slide {
            shapes: vec![Shape {
                kind: ShapeKind::Group {
                    children: vec![child],
                    child: Default::default(),
                },
                ..Default::default()
            }],
            ..Default::default()
        });
        let previous = Presentation {
            slides: vec![slide, Arc::new(Slide::default())],
            masters: vec![Arc::new(Master {
                layouts: vec![Layout::default()],
                ..Default::default()
            })],
            media: vec![MediaItem {
                data: Arc::new(vec![0; 8]),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut next = previous.clone();
        next.slides.swap(0, 1);
        assert_eq!(retained_bytes(&previous, &next), 128);
        next.slides[1] = Arc::new((*next.slides[1]).clone());
        next.masters.clear();
        next.media[0].data = Arc::new(vec![0; 8]);
        assert_eq!(
            retained_bytes(&previous, &next),
            128 + 512 + 2 * 768 + 4 + 4096 + 512 + 8
        );
    }

    #[test]
    fn oldest_steps_expire_and_a_new_edit_clears_redo() {
        let mut live = Session::new(Presentation::default(), Default::default(), true).snapshot();
        let mut history = History::default();
        for id in 1..=130 {
            let mut next = live.clone();
            next.content_id = id;
            history.record(&mut live, next);
        }
        for expected in (2..130).rev() {
            assert!(history.undo(&mut live));
            assert_eq!(live.content_id, expected);
        }
        assert!(!history.undo(&mut live));
        assert!(history.redo(&mut live));
        assert_eq!(live.content_id, 3);
        let mut branch = live.clone();
        branch.content_id = 131;
        history.record(&mut live, branch);
        assert!(!history.redo(&mut live));
        assert!(history.undo(&mut live));
        assert_eq!(live.content_id, 3);
    }

    #[test]
    fn byte_budget_keeps_the_most_recent_oversized_step() {
        let mut live = Session::new(Presentation::default(), Default::default(), true).snapshot();
        let mut history = History::default();
        let mut next = live.clone();
        next.content_id = 1;
        Arc::make_mut(&mut next.model).media.push(MediaItem {
            data: Arc::new(vec![0; UNDO_BYTES + 1]),
            ..Default::default()
        });
        history.record(&mut live, next);
        let mut next = live.clone();
        next.content_id = 2;
        Arc::make_mut(&mut next.model).media.clear();
        history.record(&mut live, next);
        assert!(history.undo(&mut live));
        assert_eq!(live.content_id, 1);
        assert_eq!(live.model.media.len(), 1);
        assert!(!history.undo(&mut live));
        assert!(history.redo(&mut live));
        assert_eq!(live.content_id, 2);
        assert!(live.model.media.is_empty());
    }
}
