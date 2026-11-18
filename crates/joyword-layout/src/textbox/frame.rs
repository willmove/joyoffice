//! Select a visible frame from a previously shaped story. Paragraph positions
//! identify continuations when the following frame has a different width.
use super::{Cut, Path, Placed, StoryRef};
use crate::{Rect, TextDirection};
use std::collections::HashMap;

pub(super) struct FrameLayout {
    items: Vec<Placed>,
    paragraphs: HashMap<StoryRef, HashMap<Path, usize>>,
    /// Bottom of each nonoverlapping paragraph, in display order. Decorations
    /// and table cells overlap, so those layouts use the original paint order.
    ordered_bottoms: Option<Vec<f32>>,
}

impl FrameLayout {
    pub(super) fn new(items: Vec<Placed>) -> Self {
        let mut paragraphs: HashMap<_, HashMap<_, _>> = HashMap::new();
        let mut ordered_bottoms = Some(Vec::new());
        for (index, item) in items.iter().enumerate() {
            let Placed::Lines {
                story,
                path,
                para,
                l0,
                l1,
                y,
                turn,
                ..
            } = item
            else {
                ordered_bottoms = None;
                continue;
            };
            paragraphs
                .entry(*story)
                .or_default()
                .entry(path.clone())
                .or_insert(index);
            if let Some(bottoms) = &mut ordered_bottoms {
                let edges = para
                    .lines
                    .get(*l0..*l1)
                    .and_then(|lines| Some((lines.first()?, lines.last()?)));
                if *turn == TextDirection::Horizontal
                    && let Some((first, last)) = edges
                {
                    let bottom = y + (last.top - first.top) + last.height;
                    if y.is_finite()
                        && bottom.is_finite()
                        && bottom >= *y
                        && bottoms.last().is_none_or(|b| *b <= *y)
                    {
                        bottoms.push(bottom);
                        continue;
                    }
                }
                ordered_bottoms = None;
            }
        }
        Self {
            items,
            paragraphs,
            ordered_bottoms,
        }
    }

    fn line_top(&self, cut: &Cut) -> Option<(usize, f32)> {
        let item = self
            .items
            .get(*self.paragraphs.get(&cut.story)?.get(&cut.path)?)?;
        let Placed::Lines {
            para, l0, l1, y, ..
        } = item
        else {
            return None;
        };
        let lines = para.lines.get(*l0..*l1)?;
        let first = lines.first()?;
        let line = lines
            .get(lines.partition_point(|l| l.stop <= cut.byte))
            .or_else(|| lines.last())?;
        Some((line.start, y + line.top - first.top))
    }

    pub(super) fn needs_break(&self, cut: &Cut) -> bool {
        self.line_top(cut).is_some_and(|(byte, _)| byte != cut.byte)
    }

    pub(super) fn share(
        &self,
        cut: Option<&Cut>,
        height: f32,
    ) -> Option<(Vec<Placed>, Option<Cut>)> {
        let top = match cut {
            Some(cut) => Some(self.line_top(cut)?.1),
            None => None,
        };
        let start = self
            .ordered_bottoms
            .as_ref()
            .zip(top)
            .map_or(0, |(bottoms, top)| {
                bottoms.partition_point(|&b| b < top - 0.5)
            });
        Some(slice(
            &self.items[start..],
            top,
            height,
            self.ordered_bottoms.is_some(),
        ))
    }
}

pub(crate) fn fit_box(items: &[Placed], height: f32) -> Vec<Placed> {
    slice(items, None, height, false).0
}

fn slice(
    items: &[Placed],
    top: Option<f32>,
    height: f32,
    ordered: bool,
) -> (Vec<Placed>, Option<Cut>) {
    let lower = top.map_or(f32::NEG_INFINITY, |t| t - 0.5);
    let offset = top.unwrap_or(0.);
    let upper = height + 0.5;
    let mut visible = Vec::new();
    let mut has_line = false;
    let mut next: Option<(f32, Cut)> = None;
    for item in items {
        if let Some(rect) = item.turned_bounds() {
            if rect.y >= lower && rect.y - offset < upper {
                let mut copy = item.clone();
                copy.translate(0., -offset);
                visible.push(copy);
            }
            continue;
        }
        match item {
            Placed::Lines {
                story,
                path,
                para,
                l0,
                l1,
                x,
                y,
                turn,
            } => {
                let Some(lines) = para.lines.get(*l0..*l1) else {
                    continue;
                };
                let Some(first) = lines.first() else { continue };
                let start = lines.partition_point(|l| y + l.top - first.top < lower);
                let Some(line) = lines.get(start) else {
                    continue;
                };
                let local_y = if top.is_some() {
                    y + line.top - first.top - offset
                } else {
                    *y
                };
                let mut end = start;
                for l in &lines[start..] {
                    let local_top = local_y + (l.top - line.top);
                    if has_line && local_top + l.height > upper {
                        if next.as_ref().is_none_or(|(best, _)| local_top < *best) {
                            next = Some((
                                local_top,
                                Cut {
                                    story: *story,
                                    path: path.clone(),
                                    byte: l.start,
                                },
                            ));
                        }
                        break;
                    }
                    has_line = true;
                    end += 1;
                }
                if end > start {
                    visible.push(Placed::Lines {
                        story: *story,
                        path: path.clone(),
                        para: para.clone(),
                        l0: l0 + start,
                        l1: l0 + end,
                        x: *x,
                        y: local_y,
                        turn: *turn,
                    });
                }
                if ordered && next.is_some() {
                    break;
                }
            }
            Placed::Fill { rect, color } if rect.bottom() > lower => {
                let clipped = top.map_or(rect.y, |t| rect.y.max(t));
                let y = clipped - offset;
                if y < upper {
                    let h = if top.is_some() {
                        rect.bottom() - clipped
                    } else {
                        rect.h
                    };
                    visible.push(Placed::Fill {
                        rect: Rect::new(rect.x, y, rect.w, h.min(upper - y)),
                        color: *color,
                    });
                }
            }
            Placed::Rule {
                x0,
                y0,
                x1,
                y1,
                border,
            } if y0.max(*y1) >= lower && y0.min(*y1) - offset < upper => {
                visible.push(Placed::Rule {
                    x0: *x0,
                    y0: (y0 - offset).min(upper),
                    x1: *x1,
                    y1: (y1 - offset).min(upper),
                    border: *border,
                });
            }
            Placed::Image { rect, .. }
            | Placed::Shape { rect, .. }
            | Placed::Graphic { rect, .. }
            | Placed::Clip { rect, .. }
            | Placed::Cell { rect, .. }
            | Placed::Object { rect, .. }
                if rect.y >= lower && rect.y - offset < upper =>
            {
                let mut copy = item.clone();
                copy.translate(0., -offset);
                visible.push(copy);
            }
            _ => {}
        }
    }
    (visible, next.map(|(_, cut)| cut))
}
