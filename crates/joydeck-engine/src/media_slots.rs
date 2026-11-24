//! Media slots in the presentation model. The same field inventory drives
//! discovery and rebinding, including fields that live on inherited styles.
use joydeck_model::{
    Background, Diagram, DiagramNode, EmbeddedFont, Layout, Master, MediaClip, MediaId,
    Presentation, Shape, ShapeKind, Slide,
    anim::{Animation, Transition},
    chart::{Chart, Series},
    style::{Artistic, Effects, Fill, FillOverlay, Line, PictureFill},
    table::{Cell, Row, Table, TableBorder, TablePart, TablePartStyle, TableStyleDef},
    text::{
        Bullet, Hyperlink, LevelStyle, ListStyle, ParaProps, Paragraph, Run, RunProps, TextBody,
    },
    theme::{FormatScheme, Theme},
};
use std::collections::HashSet;

type Read<'a> = dyn FnMut(&MediaId) + 'a;
type Write<'a> = dyn FnMut(&mut MediaId) + 'a;

trait Slots {
    fn read(&self, depth: usize, visit: &mut Read);
    fn write(&mut self, depth: usize, visit: &mut Write);
}

impl Slots for MediaId {
    fn read(&self, _: usize, visit: &mut Read) {
        visit(self);
    }
    fn write(&mut self, _: usize, visit: &mut Write) {
        visit(self);
    }
}

impl<T: Slots> Slots for Option<T> {
    fn read(&self, depth: usize, visit: &mut Read) {
        if let Some(value) = self {
            value.read(depth, visit);
        }
    }
    fn write(&mut self, depth: usize, visit: &mut Write) {
        if let Some(value) = self {
            value.write(depth, visit);
        }
    }
}

impl<T: Slots> Slots for [T] {
    fn read(&self, depth: usize, visit: &mut Read) {
        for value in self {
            value.read(depth, visit);
        }
    }
    fn write(&mut self, depth: usize, visit: &mut Write) {
        for value in self {
            value.write(depth, visit);
        }
    }
}

macro_rules! list_slots {
    ($container:ty $(, $extra:tt)*) => {
        impl<T: Slots $(, const $extra: usize)*> Slots for $container {
            fn read(&self, depth: usize, visit: &mut Read) {
                self.as_slice().read(depth, visit);
            }
            fn write(&mut self, depth: usize, visit: &mut Write) {
                self.as_mut_slice().write(depth, visit);
            }
        }
    };
}
list_slots!(Vec<T>);
list_slots!([T; N], N);

macro_rules! fields {
    ($($model:ty => $($field:tt),+;)*) => {$(
        impl Slots for $model {
            fn read(&self, depth: usize, visit: &mut Read) {
                $(self.$field.read(depth, visit);)+
            }
            fn write(&mut self, depth: usize, visit: &mut Write) {
                $(self.$field.write(depth, visit);)+
            }
        }
    )*};
}

fields! {
    Artistic => original;
    PictureFill => media, artistic;
    Line => fill;
    FillOverlay => fill;
    Effects => fill_overlay;
    Hyperlink => sound;
    RunProps => fill, outline, link, hover;
    ParaProps => bullet;
    LevelStyle => para, run;
    ListStyle => levels;
    Run => props;
    Paragraph => props, def_props, end_props, runs;
    TextBody => list_style, paragraphs;
    Cell => text, fill, borders, diag_down, diag_up;
    Row => cells;
    Table => background, rows;
    TablePartStyle => fill, borders;
    (TablePart, TablePartStyle) => 1;
    TableStyleDef => parts, background;
    (usize, Fill) => 1;
    Series => fill, line, point_fills;
    Chart => series, plot_fill, chart_fill;
    MediaClip => media, poster;
    (String, String, MediaId) => 2;
    DiagramNode => picture;
    Diagram => nodes;
    Shape => kind, fill, line, text, click, hover, diagram, effects;
    Transition => sound;
    Animation => sound;
    Slide => shapes, background, transition, animations, notes;
    Layout => shapes, background;
    FormatScheme => fills, bg_fills, lines, effects;
    Theme => format;
    Master => theme, shapes, background, title_style, body_style, other_style, layouts;
    EmbeddedFont => regular, bold, italic, bold_italic;
}

macro_rules! variants {
    ($model:ty { $($kind:pat => $($field:ident),* $(@ $limit:literal)?;)* }) => {
        impl Slots for $model {
            fn read(&self, depth: usize, _visit: &mut Read) {
                match self {
                    $($kind => variants!(@visit read, depth, _visit; $($field),* $(@ $limit)?),)*
                }
            }
            fn write(&mut self, depth: usize, _visit: &mut Write) {
                match self {
                    $($kind => variants!(@visit write, depth, _visit; $($field),* $(@ $limit)?),)*
                }
            }
        }
    };
    (@visit $method:ident, $depth:ident, $visit:ident; $($field:ident),* @ $limit:literal) => {{
        if $depth < $limit {
            $($field.$method($depth + 1, $visit);)*
        }
    }};
    (@visit $method:ident, $depth:ident, $visit:ident; $($field:ident),*) => {{
        $($field.$method($depth, $visit);)*
    }};
}

variants!(Fill {
    Fill::Picture(picture) => picture;
    Fill::None | Fill::Solid { .. } | Fill::Gradient(_) | Fill::Pattern(_) | Fill::Group | Fill::Background => ;
});
variants!(Bullet {
    Bullet::Picture { media } => media;
    Bullet::None | Bullet::Char { .. } | Bullet::AutoNum { .. } => ;
});
variants!(TableBorder {
    TableBorder::Line { line } => line;
    TableBorder::Ref { .. } => ;
});
variants!(Background {
    Background::Fill { fill } => fill;
    Background::Ref { .. } => ;
});
variants!(ShapeKind {
    ShapeKind::Picture { fill } => fill;
    ShapeKind::Group { children, child: _ } => children @ 64;
    ShapeKind::Table(table) => table;
    ShapeKind::Chart(chart) => chart;
    ShapeKind::Media(clip) => clip;
    ShapeKind::Model3d { media, poster } => media, poster;
    ShapeKind::Opaque { preview, parts, xml: _, label: _ } => preview, parts;
    ShapeKind::Shape | ShapeKind::Connector { .. } | ShapeKind::Ink { .. } => ;
});

// Preserve the existing visitor entry points for the clipboard and text editor.
macro_rules! visitors {
    ($operation:ident, $callback:ident $(, $mutable:tt)?) => {
        use super::*;
        pub type F<'a> = $callback<'a>;

        visitors!(@one $operation, picture, PictureFill $(, $mutable)?);
        visitors!(@one $operation, fill, Fill $(, $mutable)?);
        visitors!(@one $operation, line, Line $(, $mutable)?);
        visitors!(@one $operation, list, ListStyle $(, $mutable)?);
        visitors!(@one $operation, text, TextBody $(, $mutable)?);
        visitors!(@one $operation, table_style, TableStyleDef $(, $mutable)?);
        visitors!(@one $operation, shape, Shape $(, $mutable)?);
        visitors!(@one $operation, shapes, [Shape] $(, $mutable)?);
        visitors!(@one $operation, slide, Slide $(, $mutable)?);
        visitors!(@one $operation, layout, Layout $(, $mutable)?);
        visitors!(@one $operation, theme, Theme $(, $mutable)?);
        visitors!(@one $operation, master, Master $(, $mutable)?);
        visitors!(@one $operation, font, EmbeddedFont $(, $mutable)?);
    };
    (@one $operation:ident, $name:ident, $model:ty $(, $mutable:tt)?) => {
        pub fn $name(model: & $($mutable)? $model, visit: &mut F) {
            model.$operation(0, visit);
        }
    };
}

/// Read the media IDs under a part of the model.
pub mod visit {
    visitors!(read, Read);
}

/// Rebind the media IDs under a part of the model.
pub mod visit_mut {
    visitors!(write, Write, mut);
}

/// Follow presentation references, including masters not used by any slide.
pub fn visit_presentation(p: &Presentation, visit: &mut visit::F) {
    for slide in &p.slides {
        slide.read(0, visit);
    }
    for master in p
        .masters
        .iter()
        .chain(&p.notes_master)
        .chain(&p.handout_master)
    {
        master.read(0, visit);
    }
    p.default_text_style.read(0, visit);
    p.table_styles.read(0, visit);
    p.embedded_fonts.read(0, visit);
}

/// Unique presentation media IDs, in encounter order.
pub fn presentation_media(p: &Presentation) -> Vec<MediaId> {
    unique(|visit| visit_presentation(p, visit))
}

/// Unique slide media IDs, in encounter order.
pub fn slide_media(slide: &Slide) -> Vec<MediaId> {
    unique(|visit| visit::slide(slide, visit))
}

/// Unique object media IDs, in encounter order.
pub fn shapes_media(shapes: &[Shape]) -> Vec<MediaId> {
    unique(|visit| visit::shapes(shapes, visit))
}

fn unique(visit: impl FnOnce(&mut visit::F)) -> Vec<MediaId> {
    let mut seen = HashSet::new();
    let mut ids = Vec::new();
    visit(&mut |&id| {
        if seen.insert(id) {
            ids.push(id);
        }
    });
    ids
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use joydeck_model::style::{Artistic, Fill, PictureFill};
    use joydeck_model::text::{Action, Bullet, Hyperlink};
    use joydeck_model::{Background, MediaClip, ShapeKind, defaults};

    use super::*;

    fn pf(media: u32) -> PictureFill {
        PictureFill {
            media: MediaId(media),
            ..Default::default()
        }
    }

    /// One reference of each kind, every one a different id.
    fn deck() -> Presentation {
        let mut p = defaults::new_presentation(None);
        let mut s = (*p.slides.first().cloned().unwrap_or_default()).clone();
        let pic = Shape {
            kind: ShapeKind::Picture {
                fill: PictureFill {
                    artistic: Some(Artistic {
                        effect: "artisticBlur".into(),
                        original: MediaId(9002),
                    }),
                    ..pf(9001)
                },
            },
            ..Default::default()
        };
        let clip = Shape {
            kind: ShapeKind::Media(MediaClip {
                media: MediaId(9003),
                poster: Some(MediaId(9004)),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut text = joydeck_model::text::TextBody::from_text("x");
        if let Some(para) = text.paragraphs.first_mut() {
            para.props.bullet = Some(Bullet::Picture {
                media: MediaId(9007),
            });
        }
        let filled = Shape {
            fill: Some(Fill::Picture(pf(9005))),
            click: Some(Hyperlink {
                action: Action::NextSlide,
                tooltip: String::new(),
                highlight_click: false,
                sound: Some(MediaId(9006)),
                stop_sound: false,
                zoom_return: None,
            }),
            text: Some(text),
            ..Default::default()
        };
        let group = Shape {
            kind: ShapeKind::Group {
                children: vec![pic, clip],
                child: Default::default(),
            },
            ..Default::default()
        };
        s.shapes = vec![group, filled];
        s.background = Some(Background::Fill {
            fill: Fill::Picture(pf(9008)),
        });
        p.slides = vec![Arc::new(s)];
        if let Some(m) = p.masters.first_mut() {
            Arc::make_mut(m)
                .theme
                .format
                .bg_fills
                .push(Fill::Picture(pf(9009)));
        }
        p.embedded_fonts.push(joydeck_model::EmbeddedFont {
            regular: Some(MediaId(9010)),
            ..Default::default()
        });
        p
    }

    #[test]
    fn every_kind_of_reference_is_found() {
        let p = deck();
        let mut got = presentation_media(&p);
        got.sort();
        assert_eq!(got, (9001..=9010).map(MediaId).collect::<Vec<_>>());
        let mut slide = p.slides.first().map(|s| slide_media(s)).unwrap_or_default();
        slide.sort();
        assert_eq!(slide, (9001..=9008).map(MediaId).collect::<Vec<_>>());
    }

    #[test]
    fn visit_mut_rewrites_every_reference() {
        let p = deck();
        let mut s = p
            .slides
            .first()
            .map(|slide| (**slide).clone())
            .unwrap_or_default();
        visit_mut::slide(&mut s, &mut |id: &mut MediaId| id.0 += 100);
        let mut got = slide_media(&s);
        got.sort();
        assert_eq!(got, (9101..=9108).map(MediaId).collect::<Vec<_>>());
    }

    /// SmartArt and OLE objects keep their parts as media items (#209); those count as used.
    #[test]
    fn kept_parts_of_unmodelled_objects_are_references() {
        let opaque = Shape {
            kind: ShapeKind::Opaque {
                xml: String::new(),
                preview: Some(MediaId(9101)),
                label: "object".into(),
                parts: vec![("rId2".into(), "oleObject".into(), MediaId(9102))],
            },
            ..Default::default()
        };
        assert_eq!(
            shapes_media(std::slice::from_ref(&opaque)),
            vec![MediaId(9101), MediaId(9102)]
        );
    }

    #[test]
    fn overlays_and_layout_images_follow_the_same_read_and_write_inventory() {
        let overlay = |id| Effects {
            fill_overlay: Some(FillOverlay {
                blend: "over".into(),
                fill: Fill::Picture(pf(id)),
            }),
            ..Default::default()
        };
        let mut master = Master::default();
        master.theme.format.effects.push(overlay(60));
        master.layouts.push(Layout {
            shapes: vec![Shape {
                effects: Some(overlay(61)),
                ..Default::default()
            }],
            background: Some(Background::Fill {
                fill: Fill::Picture(pf(62)),
            }),
            ..Default::default()
        });
        let read = |master: &Master| unique(|visit| visit::master(master, visit));
        assert_eq!(read(&master), [MediaId(60), MediaId(61), MediaId(62)]);
        visit_mut::master(&mut master, &mut |id| id.0 += 5);
        assert_eq!(read(&master), [MediaId(65), MediaId(66), MediaId(67)]);
    }
}
