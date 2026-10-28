//! Position mapping through edits (ARCHITECTURE §2.1, REVIEW-006).
//!
//! A [`PositionMap`] is the ordered list of primitive steps a transaction
//! performed. Mapping a position replays the steps; composing two maps
//! concatenates them, so composed and step-by-step mapping agree by
//! construction. A position never jumps into an unrelated paragraph: it is
//! kept, collapsed onto the boundary of deleted content, or reported lost.

use joyword_model::{Bias, NodeId, Position, ScalarOffset, StoryId};

/// One primitive change, in scalar offsets of the paragraph at that moment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapStep {
    /// `len` positions inserted at `at`.
    Insert {
        paragraph: NodeId,
        at: usize,
        len: usize,
    },
    /// `start..end` removed inside one paragraph.
    Delete {
        paragraph: NodeId,
        start: usize,
        end: usize,
    },
    /// Deletion from `first` at `start` through `last` before `end`,
    /// including the paragraph marks between; `removed` paragraphs vanish and
    /// what remains of `last` is appended to `first`.
    DeleteAcross {
        story: StoryId,
        first: NodeId,
        start: usize,
        last: NodeId,
        end: usize,
        removed: Vec<NodeId>,
    },
    /// Content from `at` moves into the new paragraph `new`.
    Split {
        paragraph: NodeId,
        at: usize,
        new: NodeId,
    },
    /// A rejected inserted mark joins `head` in front of `tail`: the head's
    /// content keeps its offsets at the front of the tail and the tail's
    /// content shifts by `head_len`. The tail keeps its identity.
    MergeBefore {
        story: StoryId,
        head: NodeId,
        head_len: usize,
        tail: NodeId,
    },
    /// The paragraph was removed with its block; positions collapse to
    /// `fallback` when there is one.
    Remove {
        paragraph: NodeId,
        fallback: Option<(StoryId, NodeId, usize)>,
    },
}

/// Result of mapping one position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mapped {
    /// The position survived; it may have shifted.
    Kept(Position),
    /// The content around the position was deleted; it now sits on the
    /// deletion boundary.
    Collapsed(Position),
    /// The position's paragraph is gone and no boundary exists.
    Lost,
}

impl Mapped {
    pub fn position(self) -> Option<Position> {
        match self {
            Mapped::Kept(position) | Mapped::Collapsed(position) => Some(position),
            Mapped::Lost => None,
        }
    }
}

fn moved(position: Position, story: StoryId, paragraph: NodeId, offset: usize) -> Position {
    Position {
        story,
        paragraph,
        offset: ScalarOffset(offset),
        bias: position.bias,
    }
}

fn map_step(step: &MapStep, position: Position) -> Mapped {
    let offset = position.offset.0;
    match *step {
        MapStep::Insert { paragraph, at, len } if position.paragraph == paragraph => {
            if offset > at || (offset == at && position.bias == Bias::After) {
                Mapped::Kept(Position {
                    offset: ScalarOffset(offset + len),
                    ..position
                })
            } else {
                Mapped::Kept(position)
            }
        }
        MapStep::Delete {
            paragraph,
            start,
            end,
        } if position.paragraph == paragraph => {
            if offset <= start {
                Mapped::Kept(position)
            } else if offset >= end {
                Mapped::Kept(Position {
                    offset: ScalarOffset(offset - (end - start)),
                    ..position
                })
            } else {
                Mapped::Collapsed(Position {
                    offset: ScalarOffset(start),
                    ..position
                })
            }
        }
        MapStep::DeleteAcross {
            story,
            first,
            start,
            last,
            end,
            ref removed,
        } => {
            let boundary = moved(position, story, first, start);
            if position.paragraph == first {
                if offset <= start {
                    Mapped::Kept(position)
                } else {
                    Mapped::Collapsed(boundary)
                }
            } else if position.paragraph == last {
                if offset >= end {
                    Mapped::Kept(moved(position, story, first, start + offset - end))
                } else {
                    Mapped::Collapsed(boundary)
                }
            } else if removed.contains(&position.paragraph) {
                Mapped::Collapsed(boundary)
            } else {
                Mapped::Kept(position)
            }
        }
        MapStep::MergeBefore {
            story,
            head,
            head_len,
            tail,
        } if position.paragraph == head || position.paragraph == tail => {
            if position.paragraph == head {
                Mapped::Kept(Position {
                    story,
                    paragraph: tail,
                    offset: ScalarOffset(offset),
                    bias: position.bias,
                })
            } else {
                Mapped::Kept(Position {
                    story,
                    paragraph: tail,
                    offset: ScalarOffset(offset + head_len),
                    bias: position.bias,
                })
            }
        }
        MapStep::Split { paragraph, at, new } if position.paragraph == paragraph => {
            if offset > at || (offset == at && position.bias == Bias::After) {
                Mapped::Kept(Position {
                    paragraph: new,
                    offset: ScalarOffset(offset - at),
                    ..position
                })
            } else {
                Mapped::Kept(position)
            }
        }
        MapStep::Remove {
            paragraph,
            fallback,
        } if position.paragraph == paragraph => match fallback {
            Some((story, paragraph, offset)) => {
                Mapped::Collapsed(moved(position, story, paragraph, offset))
            }
            None => Mapped::Lost,
        },
        _ => Mapped::Kept(position),
    }
}

/// Ordered primitive steps of one or more transactions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PositionMap {
    steps: Vec<MapStep>,
}

impl PositionMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn push(&mut self, step: MapStep) {
        self.steps.push(step);
    }

    pub fn steps(&self) -> &[MapStep] {
        &self.steps
    }

    pub fn is_identity(&self) -> bool {
        self.steps.is_empty()
    }

    /// This map followed by `next`.
    pub fn then(mut self, next: &PositionMap) -> PositionMap {
        self.steps.extend(next.steps.iter().cloned());
        self
    }

    pub fn map(&self, position: Position) -> Mapped {
        let mut collapsed = false;
        let mut current = position;
        for step in &self.steps {
            match map_step(step, current) {
                Mapped::Kept(next) => current = next,
                Mapped::Collapsed(next) => {
                    collapsed = true;
                    current = next;
                }
                Mapped::Lost => return Mapped::Lost,
            }
        }
        if collapsed {
            Mapped::Collapsed(current)
        } else {
            Mapped::Kept(current)
        }
    }
}

/// A selection; `anchor == head` is a caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Position,
    pub head: Position,
}

impl Selection {
    pub fn caret(position: Position) -> Self {
        Self {
            anchor: position,
            head: position,
        }
    }

    /// `None` when either end's paragraph was lost without a boundary.
    pub fn map(&self, map: &PositionMap) -> Option<Selection> {
        Some(Selection {
            anchor: map.map(self.anchor).position()?,
            head: map.map(self.head).position()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use joyword_model::IdAllocator;

    struct Fixture {
        story: StoryId,
        a: NodeId,
        b: NodeId,
        c: NodeId,
    }

    fn fixture() -> Fixture {
        let mut ids = IdAllocator::default();
        Fixture {
            story: ids.story(),
            a: ids.node(),
            b: ids.node(),
            c: ids.node(),
        }
    }

    fn at(f: &Fixture, paragraph: NodeId, offset: usize, bias: Bias) -> Position {
        Position {
            story: f.story,
            paragraph,
            offset: ScalarOffset(offset),
            bias,
        }
    }

    #[test]
    fn insertion_at_a_position_respects_bias() {
        let f = fixture();
        let mut map = PositionMap::new();
        map.push(MapStep::Insert {
            paragraph: f.a,
            at: 3,
            len: 2,
        });
        assert_eq!(
            map.map(at(&f, f.a, 3, Bias::Before)),
            Mapped::Kept(at(&f, f.a, 3, Bias::Before))
        );
        assert_eq!(
            map.map(at(&f, f.a, 3, Bias::After)),
            Mapped::Kept(at(&f, f.a, 5, Bias::After))
        );
        assert_eq!(
            map.map(at(&f, f.b, 3, Bias::After)),
            Mapped::Kept(at(&f, f.b, 3, Bias::After))
        );
    }

    #[test]
    fn deletion_collapses_interior_positions_only() {
        let f = fixture();
        let mut map = PositionMap::new();
        map.push(MapStep::Delete {
            paragraph: f.a,
            start: 2,
            end: 5,
        });
        assert_eq!(
            map.map(at(&f, f.a, 2, Bias::After)),
            Mapped::Kept(at(&f, f.a, 2, Bias::After))
        );
        assert_eq!(
            map.map(at(&f, f.a, 4, Bias::After)),
            Mapped::Collapsed(at(&f, f.a, 2, Bias::After))
        );
        assert_eq!(
            map.map(at(&f, f.a, 5, Bias::Before)),
            Mapped::Kept(at(&f, f.a, 2, Bias::Before))
        );
        assert_eq!(
            map.map(at(&f, f.a, 9, Bias::Before)),
            Mapped::Kept(at(&f, f.a, 6, Bias::Before))
        );
    }

    #[test]
    fn cross_paragraph_deletion_merges_and_collapses() {
        let f = fixture();
        let mut map = PositionMap::new();
        map.push(MapStep::DeleteAcross {
            story: f.story,
            first: f.a,
            start: 2,
            last: f.c,
            end: 3,
            removed: vec![f.b],
        });
        let kept = |p, o| map.map(at(&f, p, o, Bias::After));
        assert_eq!(kept(f.a, 2), Mapped::Kept(at(&f, f.a, 2, Bias::After)));
        assert_eq!(kept(f.a, 7), Mapped::Collapsed(at(&f, f.a, 2, Bias::After)));
        assert_eq!(kept(f.b, 0), Mapped::Collapsed(at(&f, f.a, 2, Bias::After)));
        assert_eq!(kept(f.c, 0), Mapped::Collapsed(at(&f, f.a, 2, Bias::After)));
        assert_eq!(kept(f.c, 3), Mapped::Kept(at(&f, f.a, 2, Bias::After)));
        assert_eq!(kept(f.c, 8), Mapped::Kept(at(&f, f.a, 7, Bias::After)));
    }

    #[test]
    fn split_moves_tail_positions_and_removal_without_fallback_is_lost() {
        let f = fixture();
        let mut map = PositionMap::new();
        map.push(MapStep::Split {
            paragraph: f.a,
            at: 4,
            new: f.b,
        });
        assert_eq!(
            map.map(at(&f, f.a, 4, Bias::Before)),
            Mapped::Kept(at(&f, f.a, 4, Bias::Before))
        );
        assert_eq!(
            map.map(at(&f, f.a, 4, Bias::After)),
            Mapped::Kept(at(&f, f.b, 0, Bias::After))
        );
        assert_eq!(
            map.map(at(&f, f.a, 6, Bias::Before)),
            Mapped::Kept(at(&f, f.b, 2, Bias::Before))
        );
        map.push(MapStep::Remove {
            paragraph: f.b,
            fallback: None,
        });
        assert_eq!(map.map(at(&f, f.a, 6, Bias::Before)), Mapped::Lost);
        assert_eq!(
            map.map(at(&f, f.a, 1, Bias::Before)),
            Mapped::Kept(at(&f, f.a, 1, Bias::Before))
        );
    }

    #[test]
    fn composition_equals_sequential_mapping() {
        let f = fixture();
        let mut first = PositionMap::new();
        first.push(MapStep::Insert {
            paragraph: f.a,
            at: 1,
            len: 3,
        });
        first.push(MapStep::Split {
            paragraph: f.a,
            at: 5,
            new: f.c,
        });
        let mut second = PositionMap::new();
        second.push(MapStep::Delete {
            paragraph: f.c,
            start: 0,
            end: 2,
        });
        let composed = first.clone().then(&second);
        for paragraph in [f.a, f.b, f.c] {
            for offset in 0..10 {
                for bias in [Bias::Before, Bias::After] {
                    let position = at(&f, paragraph, offset, bias);
                    let sequential = match first.map(position) {
                        Mapped::Kept(p) => second.map(p),
                        Mapped::Collapsed(p) => match second.map(p) {
                            Mapped::Kept(q) | Mapped::Collapsed(q) => Mapped::Collapsed(q),
                            Mapped::Lost => Mapped::Lost,
                        },
                        Mapped::Lost => Mapped::Lost,
                    };
                    assert_eq!(composed.map(position), sequential);
                }
            }
        }
    }
}
