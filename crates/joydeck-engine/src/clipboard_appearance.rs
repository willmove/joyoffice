//! Resolve slide/layout/master inheritance where the copied objects live.
//! Materialize only typed format properties, leaving text and metadata intact.
use joydeck_model::{
    Rgba, SchemeSlot, Shape, ShapeKind, resolve,
    style::{ColorBase, ColorRef, Effects, Fill, Line, PictureFill},
    text::{ParaProps, RunProps, TextBody},
};

pub(super) fn materialize(source: &resolve::Ctx, shape: &mut Shape) {
    let appearance = Appearance { source };
    let (mut fill, fill_color) = resolve::fill(source, shape);
    let (mut line, line_color) = resolve::line(source, shape);
    let (mut effects, effect_color) = resolve::effects(source, shape);
    appearance.fill(fill.as_mut(), fill_color);
    appearance.fill(line.fill.as_mut(), line_color);
    if let Some(effects) = &mut effects {
        appearance.effects(effects, effect_color);
    }
    let body = shape.text.as_ref().map(|original| {
        let mut materialized = original.clone();
        materialized.body = resolve::body(source, shape);
        for (before, after) in original.paragraphs.iter().zip(&mut materialized.paragraphs) {
            after.props = resolve::para(source, shape, before);
            after.def_props = resolve::run(source, shape, before, &before.def_props);
            after.end_props = resolve::run(source, shape, before, &before.end_props);
            for (run, copied) in before.runs.iter().zip(&mut after.runs) {
                copied.props = resolve::run(source, shape, before, &run.props);
            }
            // Text without an explicit colour draws with the source theme's Tx1.
            for run in [&mut after.def_props, &mut after.end_props]
                .into_iter()
                .chain(after.runs.iter_mut().map(|run| &mut run.props))
            {
                run.fill
                    .get_or_insert_with(|| Fill::solid(ColorRef::scheme(SchemeSlot::Tx1)));
            }
        }
        appearance.text(&mut materialized);
        materialized
    });
    shape.xfrm = Some(resolve::xfrm(source, shape));
    shape.fill = fill;
    shape.line = Some(line);
    shape.effects = effects;
    shape.text = body;
    shape.ph = None;
    shape.style = None;
    match &mut shape.kind {
        ShapeKind::Group { children, .. } => {
            for child in children {
                materialize(source, child);
            }
        }
        ShapeKind::Picture { fill } => appearance.picture(fill),
        ShapeKind::Table(table) => {
            appearance.fill(table.background.as_mut(), None);
            for cell in table.rows.iter_mut().flat_map(|row| &mut row.cells) {
                appearance.text(&mut cell.text);
                appearance.fill(cell.fill.as_mut(), None);
                for line in cell
                    .borders
                    .iter_mut()
                    .chain([&mut cell.diag_down, &mut cell.diag_up])
                    .flatten()
                {
                    appearance.line(line);
                }
            }
        }
        _ => {}
    }
}

struct Appearance<'a, 'p> {
    source: &'a resolve::Ctx<'p>,
}

impl Appearance<'_, '_> {
    fn color(&self, color: &mut ColorRef, placeholder: Option<Rgba>) {
        if matches!(color.base, ColorBase::Scheme { .. }) {
            *color = ColorRef::rgb(self.source.color(color, placeholder));
        }
    }

    fn font(&self, family: &mut Option<String>) {
        if let Some(name) = family
            && matches!(
                name.as_str(),
                "+mj-lt" | "+mn-lt" | "+mj-ea" | "+mn-ea" | "+mj-cs" | "+mn-cs"
            )
        {
            let theme = &self.source.master.theme;
            let concrete = theme.font(name);
            *name = if concrete.is_empty() {
                theme.fonts.minor.latin.clone()
            } else {
                concrete
            };
        }
    }

    fn picture(&self, picture: &mut PictureFill) {
        if let Some((dark, light)) = &mut picture.adjust.duotone {
            self.color(dark, None);
            self.color(light, None);
        }
    }

    fn fill(&self, fill: Option<&mut Fill>, placeholder: Option<Rgba>) {
        match fill {
            Some(Fill::Solid { color }) => self.color(color, placeholder),
            Some(Fill::Gradient(gradient)) => {
                for stop in &mut gradient.stops {
                    self.color(&mut stop.color, placeholder);
                }
            }
            Some(Fill::Picture(picture)) => self.picture(picture),
            Some(Fill::Pattern(pattern)) => {
                self.color(&mut pattern.fg, placeholder);
                self.color(&mut pattern.bg, placeholder);
            }
            _ => {}
        }
    }

    fn line(&self, line: &mut Line) {
        self.fill(line.fill.as_mut(), None);
    }

    fn effects(&self, effects: &mut Effects, placeholder: Option<Rgba>) {
        for shadow in [&mut effects.inner_shadow, &mut effects.outer_shadow]
            .into_iter()
            .flatten()
        {
            self.color(&mut shadow.color, placeholder);
        }
        if let Some(glow) = &mut effects.glow {
            self.color(&mut glow.color, placeholder);
        }
        if let Some(overlay) = &mut effects.fill_overlay {
            self.fill(Some(&mut overlay.fill), placeholder);
        }
    }

    fn paragraph(&self, paragraph: &mut ParaProps) {
        self.font(&mut paragraph.bullet_font);
        if let Some(color) = &mut paragraph.bullet_color {
            self.color(color, None);
        }
    }

    fn run(&self, props: &mut RunProps) {
        for name in [
            &mut props.font,
            &mut props.font_ea,
            &mut props.font_cs,
            &mut props.font_sym,
        ] {
            self.font(name);
        }
        for color in [&mut props.underline_color, &mut props.highlight]
            .into_iter()
            .flatten()
        {
            self.color(color, None);
        }
        self.fill(props.fill.as_mut(), None);
        if let Some(line) = &mut props.outline {
            self.line(line);
        }
        for shadow in [&mut props.shadow, &mut props.inner_shadow]
            .into_iter()
            .flatten()
        {
            self.color(&mut shadow.color, None);
        }
        if let Some(glow) = &mut props.glow {
            self.color(&mut glow.color, None);
        }
    }

    fn text(&self, body: &mut TextBody) {
        for level in body.list_style.levels.iter_mut().flatten() {
            self.paragraph(&mut level.para);
            self.run(&mut level.run);
        }
        for paragraph in &mut body.paragraphs {
            self.paragraph(&mut paragraph.props);
            self.run(&mut paragraph.def_props);
            self.run(&mut paragraph.end_props);
            for run in &mut paragraph.runs {
                self.run(&mut run.props);
            }
        }
    }
}
