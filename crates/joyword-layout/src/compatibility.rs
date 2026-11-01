//! Explicit display approximation for inherited style gaps. This is a separate
//! snapshot with identical paragraph positions and visible placeholders for
//! preserved objects. It must never be exported.
use joyword_model::{Block, BlockSeq, DocumentSnapshot, InlineKind, RunProps};

fn run_gap(p: &RunProps) -> bool {
    (p.preserved.is_some() && !p.preserved_layout_neutral)
        || (p.fonts.ascii.is_some() && p.fonts.h_ansi.is_some() && p.fonts.ascii != p.fonts.h_ansi)
}

pub fn has_style_approximations(document: &DocumentSnapshot) -> bool {
    fn blocks_gap(blocks: &BlockSeq) -> bool {
        blocks.iter().any(|b| match b {
            Block::Paragraph(p) => {
                p.props.preserved.is_some()
                    || run_gap(&p.mark_props)
                    || p.runs().iter().any(|r| run_gap(&r.props))
                    || p.inlines()
                        .iter()
                        .any(|i| matches!(i.kind, InlineKind::Preserved(_)))
            }
            Block::Table(t) => t
                .rows
                .iter()
                .any(|r| r.cells.iter().any(|c| blocks_gap(&c.blocks))),
            Block::Sdt(s) => blocks_gap(&s.blocks),
            Block::Opaque(_) => false,
        })
    }
    document
        .styles()
        .iter()
        .any(|s| s.para.preserved.is_some() || run_gap(&s.run))
        || document.stories().any(|s| blocks_gap(&s.blocks))
}

/// Opt-in inherited-style display: use the represented properties and the
/// selected Latin face. The caller must disclose the approximation, retain
/// the original model for editing/saving, and not export this display scene.
/// Preserved inline objects occupy a visible square; their source paragraphs
/// remain locked. Image placement may be approximate. Content positions and
/// layout budgets are retained; the source model is never changed.
pub fn with_compatible_styles(document: &DocumentSnapshot) -> DocumentSnapshot {
    if !has_style_approximations(document) {
        return document.clone();
    }
    let mut display = std::ops::Deref::deref(document).clone();
    let styles: Vec<_> = document
        .styles()
        .iter()
        .map(|s| s.as_ref().clone())
        .collect();
    for mut style in styles {
        style.para.preserved = None;
        style.run.preserved = None;
        if style.run.fonts.ascii.is_some() {
            style.run.fonts.h_ansi = style.run.fonts.ascii.clone();
        }
        // IDs are unchanged and came from this same style store.
        let _ = display.styles_mut().replace(style);
    }
    let ids: Vec<_> = display.stories().map(|s| s.id).collect();
    for id in ids {
        if let Some(story) = display.story_mut(id) {
            approximate_blocks(&mut story.blocks);
        }
    }
    display.snapshot()
}

fn approximate_blocks(blocks: &mut joyword_model::BlockSeq) {
    use joyword_model::{Block, ScalarOffset};
    use std::sync::Arc;
    for index in 0..blocks.len() {
        blocks.update(index, |block| match block {
            Block::Paragraph(p) => {
                let p = Arc::make_mut(p);
                let mut offset = 0;
                let objects: Vec<_> = p
                    .segments()
                    .iter()
                    .filter_map(|s| match s {
                        joyword_model::Segment::Text(text) => {
                            offset += text.chars().count();
                            None
                        }
                        joyword_model::Segment::Inline(object) => {
                            let at = offset;
                            offset += 1;
                            matches!(object.kind, joyword_model::InlineKind::Preserved(_))
                                .then_some(at)
                        }
                    })
                    .collect();
                for at in objects.into_iter().rev() {
                    if p.remove(ScalarOffset(at)..ScalarOffset(at + 1)).is_ok() {
                        let _ = p.insert_text(ScalarOffset(at), "□");
                    }
                }
                let mut para = p.props.as_ref().clone();
                para.preserved = None;
                if p.inlines()
                    .iter()
                    .any(|i| matches!(i.kind, joyword_model::InlineKind::Image { .. }))
                {
                    p.style = None;
                    para.indent = joyword_model::Indent {
                        start: Some(joyword_model::Measure::Twips(0)),
                        end: Some(joyword_model::Measure::Twips(0)),
                        first_line: Some(joyword_model::Measure::Twips(0)),
                        hanging: Some(joyword_model::Measure::Twips(0)),
                    };
                    para.numbering = None;
                    para.tabs = None;
                }
                p.props = Arc::new(para);
                let _ = p.format_runs(ScalarOffset(0)..ScalarOffset(p.len()), |props| {
                    props.preserved = None;
                    if props.fonts.ascii.is_some() {
                        props.fonts.h_ansi = props.fonts.ascii.clone();
                    }
                });
                Arc::make_mut(&mut p.mark_props).preserved = None;
            }
            Block::Table(t) => {
                for row in &mut Arc::make_mut(t).rows {
                    for cell in &mut row.cells {
                        approximate_blocks(&mut cell.blocks);
                    }
                }
            }
            Block::Sdt(s) => approximate_blocks(&mut Arc::make_mut(s).blocks),
            Block::Opaque(_) => {}
        });
    }
}
