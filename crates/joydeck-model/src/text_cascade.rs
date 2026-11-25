//! DrawingML text defaults. Property precedence follows ECMA-376 Part 1 §21.1.2.4
//! and the PresentationML placeholder and master text styles in §19.3.1.

use joydeck_color::SchemeSlot;

use crate::resolve::{Ctx, master_list_style, parents};
use crate::style::{ColorRef, Fill};
use crate::text::{Align, LevelStyle, ParaProps, Paragraph, RunProps};
use crate::{Shape, ShapeKind};

pub fn level_style(context: &Ctx, shape: &Shape, level: u8) -> LevelStyle {
    let (layout, master) = if matches!(shape.kind, ShapeKind::Table(_)) {
        (None, None)
    } else {
        parents(context, shape)
    };
    let mut result = LevelStyle::default();
    let mut apply_level = |style: &LevelStyle| {
        result.para.inherit(&style.para);
        result.run.inherit(&style.run);
    };
    let local_layers = [Some(shape), layout, master]
        .into_iter()
        .flatten()
        .filter_map(|shape| shape.text.as_ref()?.list_style.level(level));
    let font_ref = shape
        .style
        .as_ref()
        .or_else(|| layout?.style.as_ref())
        .map(|style| {
            let theme_family = match style.font_ref.0.as_str() {
                "major" => Some("+mj-lt"),
                "minor" => Some("+mn-lt"),
                _ => None,
            };
            LevelStyle {
                run: RunProps {
                    font: theme_family.map(str::to_owned),
                    fill: style.font_ref.1.clone().map(Fill::solid),
                    ..RunProps::default()
                },
                ..LevelStyle::default()
            }
        });
    let global_layers = [
        master_list_style(context, shape),
        &context.pres.default_text_style,
    ]
    .into_iter()
    .filter_map(|list| list.level(level));
    for layer in local_layers.chain(font_ref.as_ref()).chain(global_layers) {
        apply_level(layer);
    }
    apply_level(&LevelStyle {
        para: ParaProps {
            align: Some(Align::Left),
            margin_left: Some(0.0),
            indent: Some(0.0),
            ..ParaProps::default()
        },
        run: RunProps {
            font: Some("+mn-lt".to_owned()),
            size: Some(18.0),
            bold: Some(false),
            italic: Some(false),
            ..RunProps::default()
        },
    });
    result
}

pub fn para(context: &Ctx, shape: &Shape, paragraph: &Paragraph) -> ParaProps {
    let mut result = paragraph.props.clone();
    result.inherit(&level_style(context, shape, paragraph.level).para);
    result
}

pub fn run(context: &Ctx, shape: &Shape, paragraph: &Paragraph, explicit: &RunProps) -> RunProps {
    let mut result = explicit.clone();
    result.inherit(&paragraph.def_props);
    result.inherit(&level_style(context, shape, paragraph.level).run);

    if result.link.is_some() {
        // A directly formatted link can override the hyperlink color and underline.
        let defaults = explicit.link.is_none().then_some(&paragraph.def_props);
        if explicit.fill.is_none() && defaults.is_none_or(|props| props.fill.is_none()) {
            result.fill = Some(Fill::solid(ColorRef::scheme(SchemeSlot::Hlink)));
        }
        if explicit.underline.is_none() && defaults.is_none_or(|props| props.underline.is_none()) {
            result.underline = Some("sng".into());
        }
    }
    result
}
