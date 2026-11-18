//! A text box's text (Shape Format › Text): which way it runs (Text Direction), where it sits
//! between the box's edges (Align Text), and linked boxes, whose text continues from one box in
//! the next (Create Link).
//!
//! The text is laid out in the box's own frame (lines `len` long, stacking `across` deep), cut to
//! what fits, aligned, then turned onto the page as a table cell's turned text is. A chain of
//! linked boxes lays out the first box's story and gives each box the next stretch of it: the
//! sizes come from the boxes' shapes, so a box shows the same text wherever it is laid out.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use joyword_geom::Rect;
use joyword_model::props::{TextDirection, TextVert, VAlign};
use joyword_model::{Block, Blocks, Path, StoryRef, TextBody};

use crate::{BoxInput, Ctx, Placed, layout_box};
mod frame;
use frame::FrameLayout;
pub(crate) use frame::fit_box;

/// Linked text boxes for one layout, worked out when a linked box is first laid out.
#[derive(Default)]
pub(crate) struct Chains {
    built: bool,
    /// Box → (chain, place in it).
    member: HashMap<u32, (usize, usize)>,
    chains: Vec<Vec<u32>>,
    /// Each chain's text split over its boxes: per box, items in its unturned frame.
    flows: HashMap<usize, Vec<Vec<Placed>>>,
}

impl Chains {
    fn build(&mut self, doc: &joyword_model::Document) {
        if self.built {
            return;
        }
        self.built = true;
        if !doc.parts.values().any(|p| p.body.next.is_some()) {
            return;
        }
        self.chains = doc.text_box_chains();
        for (ci, c) in self.chains.iter().enumerate() {
            for (k, id) in c.iter().enumerate() {
                self.member.insert(*id, (ci, k));
            }
        }
    }
}

/// The story text box `id` shows: the first box's, for a box linked from another.
pub(crate) fn shown_story(ctx: &mut Ctx, id: u32) -> u32 {
    ctx.chains.build(ctx.doc);
    match ctx.chains.member.get(&id) {
        Some((ci, _)) => ctx
            .chains
            .chains
            .get(*ci)
            .and_then(|c| c.first())
            .copied()
            .unwrap_or(id),
        None => id,
    }
}

/// Line length and the depth lines stack in, for a `w` × `h` box whose text runs as `body`
/// says, inside its internal margins.
fn frame(body: &TextBody, w: f32, h: f32) -> (f32, f32) {
    let (w, h) = (joyword_geom::finite(w), joyword_geom::finite(h));
    let [l, t, r, b] = body.insets_pt();
    let (along, across) = match body.vert.turn() {
        TextDirection::Horizontal => (w - l - r, h - t - b),
        _ => (h - t - b, w - l - r),
    };
    (along.max(12.0), across)
}

/// The width stacked text is laid out in (a paragraph's narrowest).
const STACK_WIDTH: f32 = 12.0;

/// Lay `blocks` of story `id` out in lines `len` long; stacked text one letter to a line.
fn lay(
    ctx: &mut Ctx,
    id: u32,
    blocks: &Blocks,
    len: f32,
    vert: TextVert,
    depth: usize,
) -> Vec<Placed> {
    let story = StoryRef::Part(id);
    if vert == TextVert::Stacked {
        // One letter to a line: words may break anywhere, and each letter takes the narrowest
        // line a paragraph gets (the spacing after it doesn't show).
        let stacked: Blocks = blocks
            .iter()
            .map(|b| match &**b {
                Block::Para(p) => {
                    let mut p = p.clone();
                    p.props.word_wrap = Some(false);
                    for r in &mut p.runs {
                        r.props.spacing = Some(STACK_WIDTH);
                    }
                    p.touch();
                    Arc::new(Block::Para(p))
                }
                _ => b.clone(),
            })
            .collect();
        return layout_box(
            ctx,
            BoxInput::other(story, &stacked, STACK_WIDTH, depth + 1, None),
        )
        .items;
    }
    layout_box(ctx, BoxInput::other(story, blocks, len, depth + 1, None)).items
}

/// The items showing text box `id` in its frame `rect` (page coordinates): its text, or its
/// share of a linked chain's, cut to what fits, aligned and turned as the box says.
pub(crate) fn box_items(ctx: &mut Ctx, id: u32, rect: Rect, depth: usize) -> Vec<Placed> {
    let body = ctx.doc.parts.get(&id).map(|p| p.body).unwrap_or_default();
    let (len, across) = frame(&body, rect.w, rect.h);
    let items = match chain_share(ctx, id, depth) {
        Some(items) => items,
        None => {
            if !ctx.boxes.enter(id) {
                return Vec::new();
            }
            let blocks = ctx
                .doc
                .parts
                .get(&id)
                .map(|p| p.blocks.clone())
                .unwrap_or_default();
            let inner = lay(ctx, id, &blocks, len, body.vert, depth);
            ctx.boxes.leave();
            // Text that doesn't fit inside the margins is hidden, as in Word; a box that resizes
            // to fit its text shows it all (#743).
            if body.auto_fit {
                inner
            } else {
                fit_box(&inner, across)
            }
        }
    };
    place(items, &body, rect, across)
}

/// The height of a `w` wide text box `id` that resizes to fit its text (`a:spAutoFit`): its text's
/// depth plus its top and bottom margins, as Word fits it when laying the document out. `None`
/// for a box that keeps its own height: one that doesn't resize, with turned text, or linked.
pub(crate) fn fitted_height(ctx: &mut Ctx, id: u32, w: f32) -> Option<f32> {
    let body = ctx.doc.parts.get(&id).map(|p| p.body)?;
    if !body.auto_fit || body.vert.turn() != TextDirection::Horizontal || body.next.is_some() {
        return None;
    }
    ctx.chains.build(ctx.doc);
    if ctx.chains.member.contains_key(&id) || !ctx.boxes.enter(id) {
        return None;
    }
    let blocks = ctx
        .doc
        .parts
        .get(&id)
        .map(|p| p.blocks.clone())
        .unwrap_or_default();
    let (len, _) = frame(&body, w, 0.0);
    // Measuring doesn't count the box's list numbers and equations: `box_items` lays it out again.
    let snap = (ctx.counters.clone(), ctx.eq_count);
    let items = lay(ctx, id, &blocks, len, body.vert, 0);
    (ctx.counters, ctx.eq_count) = snap;
    ctx.boxes.leave();
    let [_, t, _, b] = body.insets_pt();
    let h = extent(&items) + t + b;
    h.is_finite().then_some(h)
}

/// Move items laid out in a box's frame onto the page: aligned in the `across` room (top,
/// middle, bottom; for turned text, from the side its lines start on) and turned.
fn place(mut items: Vec<Placed>, body: &TextBody, rect: Rect, across: f32) -> Vec<Placed> {
    let used = extent(&items);
    let off = match body.anchor {
        VAlign::Top => 0.0,
        VAlign::Center => ((across - used) / 2.0).max(0.0),
        VAlign::Bottom => (across - used).max(0.0),
    };
    let turn = body.vert.turn();
    let [l, t, r, b] = body.insets_pt();
    let (ox, oy) = match turn {
        TextDirection::Horizontal => (rect.x + l, rect.y + t + off),
        // Top-to-bottom lines stack from the right edge, bottom-to-top ones from the left.
        TextDirection::Down => (rect.right() - r - off, rect.y + t),
        TextDirection::Up => (rect.x + l + off, rect.bottom() - b),
    };
    for it in &mut items {
        it.turn(turn, ox, oy);
    }
    items
}

/// How deep laid-out items reach below their frame's top.
fn extent(items: &[Placed]) -> f32 {
    let mut bottom = 0.0f32;
    for it in items {
        let b = match it {
            _ if it.turned_bounds().is_some() => it.turned_bounds().map_or(0.0, |r| r.bottom()),
            Placed::Lines {
                para, l0, l1, y, ..
            } => {
                let (Some(first), Some(last)) = (
                    para.lines.get(*l0),
                    l1.checked_sub(1).and_then(|k| para.lines.get(k)),
                ) else {
                    continue;
                };
                y + last.top + last.height - first.top
            }
            Placed::Rule { y0, y1, .. } => y0.max(*y1),
            Placed::Fill { rect, .. }
            | Placed::Image { rect, .. }
            | Placed::Shape { rect, .. }
            | Placed::Graphic { rect, .. }
            | Placed::Clip { rect, .. }
            | Placed::Cell { rect, .. }
            | Placed::Object { rect, .. } => rect.bottom(),
        };
        if b.is_finite() {
            bottom = bottom.max(b);
        }
    }
    bottom
}

/// Box `id`'s share of its chain's text (items in its unturned frame), if it is linked.
fn chain_share(ctx: &mut Ctx, id: u32, depth: usize) -> Option<Vec<Placed>> {
    ctx.chains.build(ctx.doc);
    let (ci, k) = *ctx.chains.member.get(&id)?;
    if !ctx.chains.flows.contains_key(&ci) {
        let chain = ctx.chains.chains.get(ci).cloned().unwrap_or_default();
        let flow = flow(ctx, &chain, depth);
        ctx.chains.flows.insert(ci, flow);
    }
    Some(
        ctx.chains
            .flows
            .get(&ci)
            .and_then(|f| f.get(k))
            .cloned()
            .unwrap_or_default(),
    )
}

/// Where a box's text starts: the line holding byte `byte` of paragraph `path`.
struct Cut {
    story: StoryRef,
    path: Path,
    byte: usize,
}

/// Most line lengths (and text directions) a chain's story is laid out at. Each lays the whole
/// story out again and is kept while the chain is split, so a chain of boxes that each differ in
/// width would multiply its story's layout work and memory. Past 16, a box whose length isn't
/// among them takes the nearest narrower one laid out in its text direction, so its lines still
/// fit across it (they stop short of its far edge), else the narrowest, whose lines are longer
/// than the box. Far above a real chain's few box sizes.
pub(crate) const MAX_FLOW_WIDTHS: usize = 16;

/// Line lengths are told apart to a twentieth of a point (a twip): resized boxes a hair apart
/// share one layout. Stacked text is laid out at [`STACK_WIDTH`] whatever the box's size.
fn flow_key(len: f32, vert: TextVert) -> (i32, TextVert) {
    let len = if vert == TextVert::Stacked {
        STACK_WIDTH
    } else {
        len
    };
    ((len * 20.0).round() as i32, vert)
}

/// Bound repeated shaping when every frame starts in the middle of a line
/// at the new width. The charge counts input bytes and a minimum for each
/// block and table cell. A large chain uses its narrowest frame throughout.
const FRAME_REFLOW_BUDGET: usize = 256 * 1024;

fn fits_reflow_budget(blocks: &Blocks, remaining: &mut usize) -> bool {
    for block in blocks {
        let charge = match &**block {
            Block::Para(p) => p.text.len().saturating_add(32),
            Block::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        let Some(left) = remaining.checked_sub(32) else {
                            return false;
                        };
                        *remaining = left;
                        if !fits_reflow_budget(&cell.blocks, remaining) {
                            return false;
                        }
                    }
                }
                32
            }
        };
        let Some(left) = remaining.checked_sub(charge) else {
            return false;
        };
        *remaining = left;
    }
    true
}

/// Split the text of `chain`'s first box over its boxes.
fn flow(ctx: &mut Ctx, chain: &[u32], depth: usize) -> Vec<Vec<Placed>> {
    let Some(&head) = chain.first() else {
        return Vec::new();
    };
    if !ctx.boxes.enter(head) {
        return vec![Vec::new(); chain.len()];
    }
    let ids: BTreeSet<u32> = chain.iter().copied().collect();
    let sizes = ctx.doc.text_box_sizes(&ids);
    let blocks = ctx
        .doc
        .parts
        .get(&head)
        .map(|p| p.blocks.clone())
        .unwrap_or_default();
    // Each box lays the whole story out at its own line length (the same list numbers every
    // time), sharing a layout with the boxes before it of the same length.
    let snap = (ctx.counters.clone(), ctx.eq_count);
    let mut after = None;
    let frames: Vec<_> = chain
        .iter()
        .map(|id| {
            let body = ctx.doc.parts.get(id).map(|p| p.body).unwrap_or_default();
            let (w, h) = sizes.get(id).copied().unwrap_or_default();
            let (len, across) = frame(&body, w, h);
            (len, body.vert, across)
        })
        .collect();
    let (first, vert, _) = frames[0];
    let mut budget = FRAME_REFLOW_BUDGET / frames.len();
    let shared = (frames
        .iter()
        .any(|&(w, v, _)| flow_key(w, v) != flow_key(first, vert))
        && !fits_reflow_budget(&blocks, &mut budget))
    .then(|| {
        frames
            .iter()
            .map(|&(w, v, _)| (w, v))
            .min_by_key(|&(w, v)| (flow_key(w, v).0, v != TextVert::Stacked))
            .unwrap_or((first, vert))
    });
    let mut layouts: Vec<((i32, TextVert), f32, FrameLayout)> = Vec::new();
    let mut out = Vec::with_capacity(chain.len());
    // Where the next box's text starts: `Some(None)` at the beginning, `None` when all is shown.
    let mut start: Option<Option<Cut>> = Some(None);
    for (len, vert, across) in frames {
        let Some(cut) = start.take() else {
            out.push(Vec::new());
            continue;
        };
        let (len, vert) = shared.unwrap_or((len, vert));
        let key = flow_key(len, vert);
        // Past the cap the box takes the nearest layout in its direction: the widest line length
        // that fits it, else the narrowest. The first box in a direction is laid out (one more
        // layout for each of the few directions at most).
        let closest = || {
            layouts
                .iter()
                .enumerate()
                .filter(|(_, ((_, v), _, _))| *v == key.1)
                .min_by_key(|(_, ((k, _), _, _))| (*k > key.0, k.abs_diff(key.0)))
                .map(|(index, _)| index)
        };
        let cached = layouts
            .iter()
            .position(|(k, _, _)| *k == key)
            .or_else(|| (layouts.len() >= MAX_FLOW_WIDTHS).then(closest).flatten());
        let index = match cached {
            Some(i) => i,
            None => {
                (ctx.counters, ctx.eq_count) = (snap.0.clone(), snap.1);
                let items = lay(ctx, head, &blocks, len, vert, depth);
                after.get_or_insert_with(|| (ctx.counters.clone(), ctx.eq_count));
                let i = layouts.len();
                layouts.push((key, len, FrameLayout::new(items)));
                i
            }
        };
        let ((_, layout_vert), width, layout) = &mut layouts[index];
        if let Some(cut) = &cut
            && layout.needs_break(cut)
        {
            (ctx.counters, ctx.eq_count) = (snap.0.clone(), snap.1);
            let previous = ctx
                .frame_start
                .replace((cut.story, cut.path.clone(), cut.byte));
            *layout = FrameLayout::new(lay(ctx, head, &blocks, *width, *layout_vert, depth));
            ctx.frame_start = previous;
        }
        let Some((kept, next)) = layout.share(cut.as_ref(), across) else {
            out.push(Vec::new());
            continue;
        };
        start = next.map(Some);
        out.push(kept);
    }
    if let Some(a) = after {
        (ctx.counters, ctx.eq_count) = a;
    }
    ctx.boxes.leave();
    out
}
