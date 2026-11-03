//! Resolve selection formatting through the same theme and table styles as painting.
use super::*;
use joydeck_engine::{
    TextTarget,
    formatting::{self, Summary},
};
use joydeck_model::{
    ShapeKind, resolve,
    text::{Align, Bullet, Spacing, Strike},
};

impl Viewer {
    pub(super) fn format_targets(&self) -> Vec<TextTarget> {
        if let Some(inline) = &self.inline {
            return vec![inline.target.clone()];
        }
        if let Some((shape, row, col)) = self.cell {
            return vec![TextTarget::Cell { shape, row, col }];
        }
        if let Some((shape, range)) = self.cell_range
            && self
                .editor
                .as_ref()
                .is_some_and(|e| e.session.selection().shapes == [shape])
        {
            return (range.row..=range.end_row)
                .flat_map(|row| {
                    (range.col..=range.end_col).map(move |col| TextTarget::Cell { shape, row, col })
                })
                .filter(|target| {
                    self.editor.as_ref().is_some_and(|e| {
                        joydeck_engine::text_body(e.session.model(), self.page, target).is_ok()
                    })
                })
                .take(257)
                .collect();
        }
        let Some(e) = &self.editor else { return vec![] };
        let Some(slide) = e.session.model().slides.get(self.page) else {
            return vec![];
        };
        let mut out = vec![];
        for id in &e.session.selection().shapes {
            let Some(shape) = slide.shape(*id) else {
                continue;
            };
            if let ShapeKind::Table(t) = &shape.kind {
                for (row, r) in t.rows.iter().enumerate() {
                    for (col, c) in r.cells.iter().enumerate() {
                        if !c.is_covered() {
                            out.push(TextTarget::Cell {
                                shape: *id,
                                row,
                                col,
                            });
                        }
                        if out.len() > 256 {
                            return vec![];
                        }
                    }
                }
            } else if matches!(shape.kind, ShapeKind::Shape | ShapeKind::Connector { .. }) {
                out.push(TextTarget::Shape { shape: *id });
            }
        }
        out
    }
    pub(super) fn formatting(&self) -> Summary {
        let Some(e) = &self.editor else {
            return Summary::default();
        };
        let pres = e.session.model();
        let Some(slide) = pres.slides.get(self.page) else {
            return Summary::default();
        };
        let Some(ctx) = resolve::Ctx::for_slide(pres, slide) else {
            return Summary::default();
        };
        let mut out = Summary::default();
        for target in self.format_targets() {
            let id = match &target {
                TextTarget::Shape { shape } | TextTarget::Cell { shape, .. } => *shape,
                TextTarget::Notes => continue,
            };
            let Some(shape) = slide.shape(id) else {
                continue;
            };
            let Ok(mut body) = joydeck_engine::text_body(pres, self.page, &target) else {
                continue;
            };
            let range = self
                .inline
                .as_ref()
                .filter(|i| i.target == target)
                .map(|i| {
                    body = i.draft.body.clone();
                    i.draft.selection.clone()
                });
            if let (TextTarget::Cell { row, col, .. }, ShapeKind::Table(t)) = (&target, &shape.kind)
                && let Some(cell) = t.cell(*row, *col)
            {
                let mut cell = cell.clone();
                cell.text = body;
                body = joydeck_render::table_cell_body(&ctx, t, *row, *col, &cell);
            }
            let result = formatting::summarize(
                &body,
                range,
                |p, props| {
                    let mut r = resolve::run(&ctx, shape, p, props);
                    r.font = Some(resolve::font_family(&ctx, &r));
                    r.bold = Some(r.bold.unwrap_or(false));
                    r.italic = Some(r.italic.unwrap_or(false));
                    r.underline = Some(r.underline.unwrap_or_else(|| "none".into()));
                    r.strike = Some(r.strike.unwrap_or(Strike::None));
                    r.baseline = Some(r.baseline.unwrap_or(0.));
                    r.spacing = Some(r.spacing.unwrap_or(0.));
                    r
                },
                |p| {
                    let mut para = resolve::para(&ctx, shape, p);
                    para.align = Some(para.align.unwrap_or(Align::Left));
                    para.margin_right = Some(para.margin_right.unwrap_or(0.));
                    para.line_spacing = Some(para.line_spacing.unwrap_or(Spacing::Pct(1.)));
                    para.space_before = Some(para.space_before.unwrap_or(Spacing::Pts(0.)));
                    para.space_after = Some(para.space_after.unwrap_or(Spacing::Pts(0.)));
                    para.bullet = Some(para.bullet.unwrap_or(Bullet::None));
                    para
                },
            );
            if let Ok(mut state) = result {
                if let Some(i) = &self.inline
                    && i.target == target
                    && i.draft.selection.is_empty()
                    && let Some(typing) = &i.draft.typing_props
                {
                    let mut explicit = typing.clone();
                    explicit.inherit(&state.run);
                    state.run = explicit;
                }
                out.add(state);
            }
        }
        out
    }
}
