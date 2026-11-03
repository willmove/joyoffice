//! Bounded R5 transactions on the existing model. Desktop and CLI share undo/recovery semantics.
use super::*;
use serde::{Deserialize, Serialize};
use std::{io::Cursor, result::Result};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ContentAction {
    Background {
        rgb: u32,
    },
    MasterBackground {
        rgb: u32,
    },
    MasterVisibility {
        visible: bool,
    },
    Layout {
        id: LayoutId,
    },
    ThemeAccent {
        slot: u8,
        rgb: u32,
    },
    ThemeFonts {
        major: String,
        minor: String,
    },
    SlideSize {
        width: f64,
        height: f64,
    },
    ShapeFill {
        rgb: u32,
    },
    ShapeGradient {
        start_rgb: u32,
        end_rgb: u32,
        angle: f64,
    },
    ShapePreset {
        name: String,
    },
    Picture {
        crop: [f64; 4],
        alpha: f64,
        brightness: f64,
        contrast: f64,
    },
    ChartValue {
        series: usize,
        index: usize,
        value: f64,
    },
    ChartTitle {
        title: String,
    },
    ChartType {
        chart_type: chart::ChartType,
    },
    Metadata {
        title: String,
        author: String,
    },
}
fn color(rgb: u32) -> Result<style::ColorRef, String> {
    if rgb > 0xffffff {
        return Err("color must be RGB24".into());
    }
    Ok(style::ColorRef::rgb(joydeck_model::Rgba::rgb(
        (rgb >> 16) as u8,
        (rgb >> 8) as u8,
        rgb as u8,
    )))
}
fn short(text: &str) -> Result<(), String> {
    if text.len() > 4096 {
        Err("content text exceeds 4 KiB".into())
    } else {
        Ok(())
    }
}
fn supported_chart(kind: chart::ChartType) -> bool {
    use chart::ChartType::*;
    matches!(
        kind,
        Column
            | StackedColumn
            | PercentColumn
            | Bar
            | StackedBar
            | PercentBar
            | Line
            | LineMarkers
            | StackedLine
            | Area
            | StackedArea
            | Pie
            | Doughnut
    )
}
fn category_chart(c: &Chart) -> Result<(), String> {
    if !supported_chart(c.kind)
        || c.series.len() > 64
        || c.categories.len() > 4096
        || c.series.len().saturating_mul(c.categories.len()) > 65536
    {
        return Err("editable category chart exceeds type/data budget".into());
    }
    Ok(())
}
fn selected(st: &EditorSnapshot) -> Result<ShapeId, String> {
    if let [id] = st.selection.shapes.as_slice() {
        Ok(*id)
    } else {
        Err("select one object".into())
    }
}
pub fn apply(st: &mut EditorSnapshot, cmd: &Command) -> Option<Result<(), String>> {
    match cmd {
        Command::Content { action } => Some(edit(st, action)),
        Command::InsertPicture {
            png,
            x,
            y,
            width,
            height,
            replace,
        } => Some(picture(st, png, [*x, *y, *width, *height], *replace)),
        _ => None,
    }
}
fn edit(st: &mut EditorSnapshot, action: &ContentAction) -> Result<(), String> {
    let page = st.selection.page;
    let master = st
        .model
        .master_for(&st.model.slides[page])
        .map(|(m, _)| m.id)
        .ok_or("master missing")?;
    let id = st.selection.shapes.first().copied();
    let p = Arc::make_mut(&mut st.model);
    match action {
        ContentAction::Background { rgb } => {
            Arc::make_mut(&mut p.slides[page]).background = Some(Background::Fill {
                fill: Fill::solid(color(*rgb)?),
            })
        }
        ContentAction::MasterBackground { rgb } => {
            let m = p
                .masters
                .iter_mut()
                .find(|m| m.id == master)
                .ok_or("master missing")?;
            Arc::make_mut(m).background = Some(Background::Fill {
                fill: Fill::solid(color(*rgb)?),
            });
        }
        ContentAction::MasterVisibility { visible } => {
            Arc::make_mut(&mut p.slides[page]).show_master_shapes = *visible
        }
        ContentAction::Layout { id } => {
            if p.layout(*id).is_none() {
                return Err("layout missing".into());
            }
            Arc::make_mut(&mut p.slides[page]).layout = *id;
        }
        ContentAction::ThemeAccent { slot, rgb } => {
            let slot = match slot {
                1 => joydeck_model::SchemeSlot::Accent1,
                2 => joydeck_model::SchemeSlot::Accent2,
                3 => joydeck_model::SchemeSlot::Accent3,
                4 => joydeck_model::SchemeSlot::Accent4,
                5 => joydeck_model::SchemeSlot::Accent5,
                6 => joydeck_model::SchemeSlot::Accent6,
                _ => return Err("accent slot must be 1..6".into()),
            };
            let c = color(*rgb)?.resolve(&p.masters[0].theme.colors, None);
            let m = p
                .masters
                .iter_mut()
                .find(|m| m.id == master)
                .ok_or("master missing")?;
            Arc::make_mut(m).theme.colors.set(slot, c);
        }
        ContentAction::ThemeFonts { major, minor } => {
            short(major)?;
            short(minor)?;
            if major.trim().is_empty() || minor.trim().is_empty() {
                return Err("theme font must not be empty".into());
            }
            let m = p
                .masters
                .iter_mut()
                .find(|m| m.id == master)
                .ok_or("master missing")?;
            let fonts = &mut Arc::make_mut(m).theme.fonts;
            fonts.major.latin = major.clone();
            fonts.minor.latin = minor.clone();
        }
        ContentAction::SlideSize { width, height } => {
            if !width.is_finite()
                || !height.is_finite()
                || !(72.0..=4096.0).contains(width)
                || !(72.0..=4096.0).contains(height)
            {
                return Err("slide size must be finite 72..4096 points".into());
            }
            p.slide_size = joydeck_model::Size::new(*width, *height);
        }
        ContentAction::Metadata { title, author } => {
            short(title)?;
            short(author)?;
            p.props.title = title.clone();
            p.props.author = author.clone();
        }
        _ => {
            if st.selection.shapes.len() != 1 {
                return Err("select one object".into());
            }
            let s = Arc::make_mut(&mut p.slides[page])
                .shape_mut(id.ok_or("select one object")?)
                .ok_or("object missing")?;
            if s.locked {
                return Err("object is locked".into());
            }
            match action {
                ContentAction::ShapeFill { rgb } => {
                    if !matches!(s.kind, ShapeKind::Shape) {
                        return Err("select a basic shape for fill editing".into());
                    }
                    s.fill = Some(Fill::solid(color(*rgb)?));
                }
                ContentAction::ShapeGradient {
                    start_rgb,
                    end_rgb,
                    angle,
                } => {
                    if !matches!(s.kind, ShapeKind::Shape)
                        || !angle.is_finite()
                        || angle.abs() > 360.
                    {
                        return Err(
                            "select a basic shape and finite gradient angle within 360 degrees"
                                .into(),
                        );
                    }
                    s.fill = Some(Fill::Gradient(style::Gradient {
                        stops: vec![
                            style::GradientStop {
                                pos: 0.,
                                color: color(*start_rgb)?,
                            },
                            style::GradientStop {
                                pos: 1.,
                                color: color(*end_rgb)?,
                            },
                        ],
                        shape: style::GradientShape::Linear {
                            angle: *angle,
                            scaled: false,
                        },
                        flip: None,
                        rotate_with_shape: true,
                        tile: [0.; 4],
                    }));
                }
                ContentAction::ShapePreset { name } => {
                    if !matches!(s.kind, ShapeKind::Shape)
                        || ![
                            "rect",
                            "roundRect",
                            "ellipse",
                            "triangle",
                            "chevron",
                            "hexagon",
                            "star5",
                            "rightArrow",
                        ]
                        .contains(&name.as_str())
                    {
                        return Err("choose a supported basic shape".into());
                    }
                    s.geom = Geom::Preset {
                        name: name.clone(),
                        adj: vec![],
                    };
                }
                ContentAction::Picture {
                    crop,
                    alpha,
                    brightness,
                    contrast,
                } => {
                    if crop
                        .iter()
                        .any(|v| !v.is_finite() || !(0.0..=0.95).contains(v))
                        || crop[0] + crop[2] >= 0.98
                        || crop[1] + crop[3] >= 0.98
                        || !alpha.is_finite()
                        || !(0.0..=1.0).contains(alpha)
                        || !brightness.is_finite()
                        || !(-1.0..=1.0).contains(brightness)
                        || !contrast.is_finite()
                        || !(-1.0..=1.0).contains(contrast)
                    {
                        return Err("picture crop/adjust outside finite budget".into());
                    }
                    let ShapeKind::Picture { fill } = &mut s.kind else {
                        return Err("select a picture".into());
                    };
                    fill.crop = *crop;
                    fill.alpha = Some(*alpha);
                    fill.adjust.brightness = *brightness;
                    fill.adjust.contrast = *contrast;
                }
                ContentAction::ChartValue {
                    series,
                    index,
                    value,
                } => {
                    if !value.is_finite() || value.abs() > 1e12 {
                        return Err("chart value outside finite budget".into());
                    }
                    let ShapeKind::Chart(chart) = &mut s.kind else {
                        return Err("select a chart".into());
                    };
                    category_chart(chart)?;
                    let cell = chart
                        .series
                        .get_mut(*series)
                        .and_then(|s| s.values.get_mut(*index))
                        .ok_or("chart cell outside data grid")?;
                    *cell = Some(*value);
                }
                ContentAction::ChartTitle { title } => {
                    short(title)?;
                    let ShapeKind::Chart(chart) = &mut s.kind else {
                        return Err("select a chart".into());
                    };
                    category_chart(chart)?;
                    chart.title = if title.is_empty() {
                        None
                    } else {
                        Some(title.clone())
                    };
                }
                ContentAction::ChartType { chart_type } => {
                    if !supported_chart(*chart_type) {
                        return Err("advanced chart conversion needs a later slice".into());
                    }
                    let ShapeKind::Chart(chart) = &mut s.kind else {
                        return Err("select a chart".into());
                    };
                    category_chart(chart)?;
                    chart.kind = *chart_type;
                }
                _ => return Err("unsupported content edit".into()),
            }
        }
    }
    Ok(())
}
fn picture(
    st: &mut EditorSnapshot,
    png: &[u8],
    box_: [f64; 4],
    replace: bool,
) -> Result<(), String> {
    use image::ImageDecoder;
    if png.len() > 16 << 20 {
        return Err("picture exceeds 16 MiB".into());
    }
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 << 20);
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    let decoder = image::codecs::png::PngDecoder::with_limits(Cursor::new(png), limits.clone())
        .map_err(|e| e.to_string())?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 || w > 8192 || h > 8192 || u64::from(w) * u64::from(h) > 16_000_000 {
        return Err("picture exceeds 8192 side / 16 MP".into());
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(png), image::ImageFormat::Png);
    reader.limits(limits);
    let _ = reader.decode().map_err(|e| e.to_string())?;
    let [x, y, w, h] = box_;
    for v in box_ {
        checked(v)?;
    }
    if w < 1. || h < 1. {
        return Err("picture size must be positive".into());
    }
    let target = if replace { Some(selected(st)?) } else { None };
    let page = st.selection.page;
    let p = Arc::make_mut(&mut st.model);
    if p.media
        .iter()
        .map(|m| m.data.len())
        .sum::<usize>()
        .saturating_add(png.len())
        > 64 << 20
    {
        return Err("document media exceeds 64 MiB".into());
    }
    let media = if let Some(m) = p.media.iter().find(|m| m.data.as_slice() == png) {
        m.id
    } else {
        let id = MediaId(next_id(p)?);
        p.media.push(MediaItem {
            id,
            name: format!("joydeck-{}.png", id.0),
            content_type: "image/png".into(),
            data: Arc::new(png.to_vec()),
            link: None,
        });
        id
    };
    if let Some(id) = target {
        let s = Arc::make_mut(&mut p.slides[page])
            .shape_mut(id)
            .ok_or("picture missing")?;
        if s.locked {
            return Err("picture locked".into());
        }
        let ShapeKind::Picture { fill } = &mut s.kind else {
            return Err("select a picture".into());
        };
        fill.media = media;
        fill.artistic = None;
    } else {
        let id = ShapeId(next_id(p)?);
        Arc::make_mut(&mut p.slides[page]).shapes.push(Shape {
            id,
            name: "Picture".into(),
            xfrm: Some(Xfrm::new(x, y, w, h)),
            kind: ShapeKind::Picture {
                fill: style::PictureFill {
                    media,
                    ..Default::default()
                },
            },
            fill: Some(Fill::None),
            line: Some(Line::none()),
            ..Default::default()
        });
        st.selection.shapes = vec![id];
    }
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test assertions"
)]
mod tests {
    use super::*;
    fn session() -> Session {
        let p = Presentation::default();
        Session::new(p.clone(), original_bindings(&p), true)
    }
    #[test]
    fn global_edits_undo_restore_saved_state_and_keep_unedited_slides_shared() {
        let mut s = session();
        let before = s.snapshot();
        assert!(
            s.execute(Command::Content {
                action: ContentAction::ThemeAccent {
                    slot: 2,
                    rgb: 0x123456
                }
            })
            .unwrap()
        );
        assert!(Arc::ptr_eq(&before.model.slides[0], &s.model().slides[0]));
        assert!(s.dirty());
        assert!(s.execute(Command::Undo).unwrap());
        assert!(!s.dirty());
        assert_eq!(*s.model(), *before.model);
        assert!(s.execute(Command::Redo).unwrap());
        assert!(s.dirty());
        let changed = s.snapshot();
        assert!(
            s.execute(Command::Content {
                action: ContentAction::SlideSize {
                    width: f64::NAN,
                    height: 540.
                }
            })
            .is_err()
        );
        assert!(Arc::ptr_eq(&changed.model, &s.snapshot().model));
    }
    #[test]
    fn gradient_and_chart_cells_are_bounded_atomic_transactions() {
        let mut s = session();
        s.execute(Command::InsertRect {
            x: 10.,
            y: 10.,
            width: 100.,
            height: 80.,
        })
        .unwrap();
        s.execute(Command::Content {
            action: ContentAction::ShapeGradient {
                start_rgb: 0x123456,
                end_rgb: 0xaabbcc,
                angle: 45.,
            },
        })
        .unwrap();
        let before = s.snapshot();
        let revision = s.revision();
        assert!(
            s.execute(Command::Content {
                action: ContentAction::ShapeGradient {
                    start_rgb: 0,
                    end_rgb: 0,
                    angle: f64::INFINITY
                }
            })
            .is_err()
        );
        assert!(Arc::ptr_eq(&before.model, &s.snapshot().model));
        assert_eq!(revision, s.revision());
        assert!(
            s.execute(Command::Content {
                action: ContentAction::ChartValue {
                    series: 0,
                    index: 0,
                    value: 5.
                }
            })
            .is_err()
        );
        assert!(Arc::ptr_eq(&before.model, &s.snapshot().model));
    }
    #[test]
    fn malformed_pngs_cannot_add_media_or_history() {
        let mut s = session();
        let before = s.snapshot();
        let png = vec![137, 80, 78, 71, 13, 10, 26, 10];
        assert!(
            s.execute(Command::InsertPicture {
                png,
                x: 0.,
                y: 0.,
                width: 100.,
                height: 100.,
                replace: false
            })
            .is_err()
        );
        assert!(Arc::ptr_eq(&before.model, &s.snapshot().model));
        assert!(!s.can_undo());
        assert!(!s.dirty());
    }
}
