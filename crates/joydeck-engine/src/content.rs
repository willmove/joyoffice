//! Bounded R5 transactions on the existing model. Desktop and CLI share undo/recovery semantics.
use super::*;
use serde::{Deserialize, Serialize};
use std::{io::Cursor, result::Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AlignEdge {
    Left,
    CenterH,
    Right,
    Top,
    MiddleV,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DistributeAxis {
    Horizontal,
    Vertical,
}

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
    ApplyTheme {
        name: String,
    },
    ApplyThemeVariant {
        index: usize,
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
    ShapeLine {
        rgb: Option<u32>,
        width: Option<f64>,
        dash: Option<String>,
    },
    ShapePreset {
        name: String,
    },
    AlignObjects {
        edge: AlignEdge,
    },
    DistributeObjects {
        axis: DistributeAxis,
    },
    GroupObjects,
    UngroupObjects,
    SlideTransition {
        effect: String,
        option: String,
        duration_ms: u32,
        advance_on_click: bool,
        advance_after_ms: Option<u32>,
    },
    ApplyTransitionAll,
    CalloutPoint {
        vertex: u8,
        x: i32,
        y: i32,
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
fn rgb24(rgb: u32) -> Result<joydeck_model::Rgba, String> {
    if rgb > 0xffffff {
        return Err("color must be RGB24".into());
    }
    Ok(joydeck_model::Rgba::rgb(
        (rgb >> 16) as u8,
        (rgb >> 8) as u8,
        rgb as u8,
    ))
}
fn color(rgb: u32) -> Result<style::ColorRef, String> {
    rgb24(rgb).map(style::ColorRef::rgb)
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
    match action {
        ContentAction::AlignObjects { edge } => return align_objects(st, *edge),
        ContentAction::DistributeObjects { axis } => return distribute_objects(st, *axis),
        ContentAction::GroupObjects => return group_objects(st),
        ContentAction::UngroupObjects => return ungroup_objects(st),
        _ => {}
    }
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
            let c = rgb24(*rgb)?;
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
        ContentAction::ApplyTheme { name } => {
            short(name)?;
            let preset = joydeck_model::theme::builtin_themes()
                .into_iter()
                .find(|t| t.name == *name)
                .ok_or("unknown built-in theme")?;
            let old_theme = p
                .masters
                .iter()
                .find(|m| m.id == master)
                .map(|m| m.theme.clone())
                .ok_or("master missing")?;
            for m in &mut p.masters {
                if m.id == master || m.theme == old_theme {
                    let m_mut = Arc::make_mut(m);
                    let theme = &mut m_mut.theme;
                    theme.name = preset.name.clone();
                    theme.colors = preset.colors.clone();
                    theme.fonts.name = preset.fonts.name.clone();
                    theme.fonts.major.latin = preset.fonts.major.latin.clone();
                    theme.fonts.minor.latin = preset.fonts.minor.latin.clone();
                    if matches!(
                        m_mut.background,
                        Some(Background::Fill {
                            fill: Fill::Solid { .. }
                        })
                    ) {
                        m_mut.background = None;
                    }
                }
            }
        }
        ContentAction::ApplyThemeVariant { index } => {
            let old_theme = p
                .masters
                .iter()
                .find(|m| m.id == master)
                .map(|m| m.theme.clone())
                .ok_or("master missing")?;
            let variants = joydeck_model::theme::theme_variants(&old_theme);
            let chosen = variants
                .get(*index)
                .cloned()
                .ok_or("theme variant index out of range")?;
            for m in &mut p.masters {
                if m.id == master || m.theme == old_theme {
                    Arc::make_mut(m).theme.colors = chosen.clone();
                }
            }
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
        ContentAction::SlideTransition {
            effect,
            option,
            duration_ms,
            advance_on_click,
            advance_after_ms,
        } => {
            let Some(&(_, _, _, _, options)) = joydeck_model::anim::TRANSITIONS
                .iter()
                .find(|(id, _, _, _, _)| *id == effect)
            else {
                return Err("unknown slide transition effect".into());
            };
            if !option.is_empty() && !options.contains(&option.as_str()) {
                return Err("unsupported option for this slide transition".into());
            }
            if *duration_ms > 60_000 {
                return Err("transition duration exceeds 60 seconds".into());
            }
            if advance_after_ms.is_some_and(|ms| ms > 86_400_000) {
                return Err("auto-advance interval exceeds 24 hours".into());
            }
            let slide = Arc::make_mut(&mut p.slides[page]);
            let prev = slide.transition.as_ref();
            let sound = prev.and_then(|t| t.sound);
            let sound_loop = prev.is_some_and(|t| t.sound_loop);
            let stop_sound = prev.is_some_and(|t| t.stop_sound);
            if effect == "none"
                && *advance_on_click
                && advance_after_ms.is_none()
                && sound.is_none()
                && !stop_sound
            {
                slide.transition = None;
            } else {
                slide.transition = Some(joydeck_model::Transition {
                    kind: effect.clone(),
                    option: option.clone(),
                    duration_ms: *duration_ms,
                    advance_on_click: *advance_on_click,
                    advance_after_ms: *advance_after_ms,
                    sound,
                    sound_loop,
                    stop_sound,
                    raw: None,
                });
            }
        }
        ContentAction::ApplyTransitionAll => {
            let current = p.slides[page].transition.clone();
            for slide in &mut p.slides {
                Arc::make_mut(slide).transition = current.clone();
            }
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
                ContentAction::ShapeLine { rgb, width, dash } => {
                    if !matches!(s.kind, ShapeKind::Shape | ShapeKind::Connector { .. }) {
                        return Err("select a shape or connector for outline editing".into());
                    }
                    if let Some(w) = *width
                        && (!w.is_finite() || !(0.0..=144.0).contains(&w))
                    {
                        return Err("outline width must be finite 0..144 points".into());
                    }
                    if let Some(name) = dash.as_deref()
                        && !style::Dash::PRESETS.contains(&name)
                    {
                        return Err("unsupported outline dash preset".into());
                    }
                    match *rgb {
                        None => {
                            s.line = Some(style::Line::none());
                        }
                        Some(rgb) => {
                            let line = s.line.get_or_insert_with(style::Line::default);
                            line.fill = Some(Fill::solid(color(rgb)?));
                            line.width = (*width).or(line.width).or(Some(1.0));
                            if let Some(name) = dash {
                                line.dash = Some(style::Dash::Preset { name: name.clone() });
                            }
                        }
                    }
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
                    if !s.set_basic_preset(name) {
                        return Err("choose a supported basic shape".into());
                    }
                }
                ContentAction::CalloutPoint { vertex, x, y } => {
                    if !s.set_callout_vertex(*vertex, *x, *y) {
                        return Err("invalid callout vertex or coordinates".into());
                    }
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

fn align_objects(st: &mut EditorSnapshot, edge: AlignEdge) -> Result<(), String> {
    let page = st.selection.page;
    let before = st.model.clone();
    let source = before.slides.get(page).ok_or("missing slide")?;
    let ctx = resolve::Ctx::for_slide(&before, source).ok_or("missing layout")?;
    let ids: BTreeSet<_> = st.selection.shapes.iter().copied().collect();
    if ids.is_empty() {
        return Err("select at least one object to align".into());
    }
    let mut items = Vec::with_capacity(ids.len());
    for (index, shape) in source
        .shapes
        .iter()
        .enumerate()
        .filter(|(_, s)| ids.contains(&s.id))
    {
        if shape.locked {
            return Err("selected object is locked".into());
        }
        let frame = resolve::xfrm(&ctx, shape);
        items.push((index, frame, frame.bounds()));
    }
    if items.len() != ids.len() {
        return Err("missing shape".into());
    }
    let target = if items.len() == 1 {
        joydeck_model::Rect::new(0.0, 0.0, before.slide_size.width, before.slide_size.height)
    } else {
        items
            .iter()
            .skip(1)
            .fold(items[0].2, |acc, (_, _, b)| acc.union(*b))
    };
    let mut updated = Vec::with_capacity(items.len());
    for (index, mut frame, bounds) in items {
        let (dx, dy) = match edge {
            AlignEdge::Left => (target.x0 - bounds.x0, 0.0),
            AlignEdge::CenterH => (target.center().x - bounds.center().x, 0.0),
            AlignEdge::Right => (target.x1 - bounds.x1, 0.0),
            AlignEdge::Top => (0.0, target.y0 - bounds.y0),
            AlignEdge::MiddleV => (0.0, target.center().y - bounds.center().y),
            AlignEdge::Bottom => (0.0, target.y1 - bounds.y1),
        };
        frame.x += dx;
        frame.y += dy;
        checked(frame.x)?;
        checked(frame.y)?;
        updated.push((index, frame));
    }
    let p = Arc::make_mut(&mut st.model);
    let draft = Arc::make_mut(&mut p.slides[page]);
    for (index, frame) in updated {
        draft.shapes[index].xfrm = Some(frame);
    }
    coordinates::retarget_connectors(&ctx, &source.shapes, &mut draft.shapes);
    Ok(())
}

fn distribute_objects(st: &mut EditorSnapshot, axis: DistributeAxis) -> Result<(), String> {
    let page = st.selection.page;
    let before = st.model.clone();
    let source = before.slides.get(page).ok_or("missing slide")?;
    let ctx = resolve::Ctx::for_slide(&before, source).ok_or("missing layout")?;
    let ids: BTreeSet<_> = st.selection.shapes.iter().copied().collect();
    if ids.len() < 3 {
        return Err("select at least three objects to distribute".into());
    }
    let mut items = Vec::with_capacity(ids.len());
    for (index, shape) in source
        .shapes
        .iter()
        .enumerate()
        .filter(|(_, s)| ids.contains(&s.id))
    {
        if shape.locked {
            return Err("selected object is locked".into());
        }
        let frame = resolve::xfrm(&ctx, shape);
        items.push((index, frame, frame.bounds()));
    }
    if items.len() != ids.len() {
        return Err("missing shape".into());
    }
    let count = items.len();
    let mut updated = Vec::with_capacity(count);
    match axis {
        DistributeAxis::Horizontal => {
            items.sort_by(|a, b| a.2.x0.total_cmp(&b.2.x0).then_with(|| a.0.cmp(&b.0)));
            let first = items[0].2;
            let last = items[count - 1].2;
            let middle_widths: f64 = items[1..count - 1].iter().map(|(_, _, b)| b.width()).sum();
            let span = last.x0 - first.x1;
            let slots = (count - 1) as f64;
            if span >= middle_widths {
                let gap = (span - middle_widths) / slots;
                let mut cursor = first.x1 + gap;
                for (index, mut frame, bounds) in items.into_iter().skip(1).take(count - 2) {
                    frame.x += cursor - bounds.x0;
                    checked(frame.x)?;
                    cursor += bounds.width() + gap;
                    updated.push((index, frame));
                }
            } else {
                let c0 = first.center().x;
                let step = (last.center().x - c0) / slots;
                for (pos, (index, mut frame, bounds)) in
                    items.into_iter().enumerate().skip(1).take(count - 2)
                {
                    frame.x += (c0 + step * pos as f64) - bounds.center().x;
                    checked(frame.x)?;
                    updated.push((index, frame));
                }
            }
        }
        DistributeAxis::Vertical => {
            items.sort_by(|a, b| a.2.y0.total_cmp(&b.2.y0).then_with(|| a.0.cmp(&b.0)));
            let first = items[0].2;
            let last = items[count - 1].2;
            let middle_heights: f64 = items[1..count - 1].iter().map(|(_, _, b)| b.height()).sum();
            let span = last.y0 - first.y1;
            let slots = (count - 1) as f64;
            if span >= middle_heights {
                let gap = (span - middle_heights) / slots;
                let mut cursor = first.y1 + gap;
                for (index, mut frame, bounds) in items.into_iter().skip(1).take(count - 2) {
                    frame.y += cursor - bounds.y0;
                    checked(frame.y)?;
                    cursor += bounds.height() + gap;
                    updated.push((index, frame));
                }
            } else {
                let c0 = first.center().y;
                let step = (last.center().y - c0) / slots;
                for (pos, (index, mut frame, bounds)) in
                    items.into_iter().enumerate().skip(1).take(count - 2)
                {
                    frame.y += (c0 + step * pos as f64) - bounds.center().y;
                    checked(frame.y)?;
                    updated.push((index, frame));
                }
            }
        }
    }
    let p = Arc::make_mut(&mut st.model);
    let draft = Arc::make_mut(&mut p.slides[page]);
    for (index, frame) in updated {
        draft.shapes[index].xfrm = Some(frame);
    }
    coordinates::retarget_connectors(&ctx, &source.shapes, &mut draft.shapes);
    Ok(())
}

fn group_objects(st: &mut EditorSnapshot) -> Result<(), String> {
    let page = st.selection.page;
    let source = st.model.slides.get(page).ok_or("missing slide")?.clone();
    let ctx = resolve::Ctx::for_slide(&st.model, &source).ok_or("missing layout")?;
    let ids: BTreeSet<_> = st.selection.shapes.iter().copied().collect();
    if ids.len() < 2 {
        return Err("select at least two objects to group".into());
    }
    let mut indices = Vec::with_capacity(ids.len());
    let mut union: Option<joydeck_model::Rect> = None;
    let mut children = Vec::with_capacity(ids.len());
    for (index, shape) in source.shapes.iter().enumerate() {
        if !ids.contains(&shape.id) {
            continue;
        }
        if shape.locked {
            return Err("selected object is locked".into());
        }
        let frame = resolve::xfrm(&ctx, shape);
        let b = frame.bounds();
        union = Some(match union {
            Some(acc) => acc.union(b),
            None => b,
        });
        let mut child = shape.clone();
        child.xfrm = Some(frame);
        children.push(child);
        indices.push(index);
    }
    if indices.len() != ids.len() {
        return Err("missing shape".into());
    }
    let rect = union.ok_or("select at least two objects to group")?;
    let group_frame = Xfrm::new(
        rect.x0,
        rect.y0,
        rect.width().max(1.0),
        rect.height().max(1.0),
    );
    let insert_at = indices
        .last()
        .copied()
        .unwrap_or(0)
        .saturating_sub(indices.len().saturating_sub(1));
    let p = Arc::make_mut(&mut st.model);
    let group_id = ShapeId(next_id(p)?);
    let group = Shape {
        id: group_id,
        name: format!("Group {}", group_id.0),
        xfrm: Some(group_frame),
        kind: ShapeKind::Group {
            children,
            child: group_frame,
        },
        ..Shape::default()
    };
    let slide = Arc::make_mut(&mut p.slides[page]);
    slide.shapes.retain(|s| !ids.contains(&s.id));
    let at = insert_at.min(slide.shapes.len());
    slide.shapes.insert(at, group);
    st.selection.shapes = vec![group_id];
    Ok(())
}

fn decompose_world_xfrm(coeffs: [f64; 6], local_w: f64, local_h: f64) -> Xfrm {
    let [a, b, c, d, tx, ty] = coeffs;
    let cx = a * (local_w * 0.5) + c * (local_h * 0.5) + tx;
    let cy = b * (local_w * 0.5) + d * (local_h * 0.5) + ty;
    let ux_x = a * (local_w * 0.5);
    let ux_y = b * (local_w * 0.5);
    let uy_x = c * (local_h * 0.5);
    let uy_y = d * (local_h * 0.5);
    let w = (2.0 * ux_x.hypot(ux_y)).max(1.0);
    let h = (2.0 * uy_x.hypot(uy_y)).max(1.0);
    let det = ux_x * uy_y - ux_y * uy_x;
    let (flip_h, rot_rad) = if det < 0.0 {
        (true, (-ux_y).atan2(-ux_x))
    } else {
        (false, ux_y.atan2(ux_x))
    };
    let mut rot = rot_rad.to_degrees().rem_euclid(360.0);
    if rot.abs() < 1e-6 || (360.0 - rot).abs() < 1e-6 {
        rot = 0.0;
    }
    Xfrm {
        x: cx - w * 0.5,
        y: cy - h * 0.5,
        w,
        h,
        rot,
        flip_h,
        flip_v: false,
    }
}

fn ungroup_objects(st: &mut EditorSnapshot) -> Result<(), String> {
    let page = st.selection.page;
    let group_id = selected(st)?;
    let before = st.model.clone();
    let source = before.slides.get(page).ok_or("missing slide")?;
    let slide_id = source.id;
    let ctx = resolve::Ctx::for_slide(&before, source).ok_or("missing layout")?;
    let pos = source
        .shapes
        .iter()
        .position(|s| s.id == group_id)
        .ok_or("object missing")?;
    let shape = &source.shapes[pos];
    if shape.locked {
        return Err("object is locked".into());
    }
    if shape.diagram.is_some() {
        return Err("SmartArt diagrams cannot be ungrouped".into());
    }
    let ShapeKind::Group { children, child } = &shape.kind else {
        return Err("select a group to ungroup".into());
    };
    if children.is_empty() {
        return Err("group has no child objects".into());
    }
    let group_xfrm = resolve::xfrm(&ctx, shape);
    let to_slide = group_xfrm.child_affine(child);
    let mut unpacked = Vec::with_capacity(children.len());
    let mut unpacked_ids = Vec::with_capacity(children.len());
    for item in children {
        let mut next = item.clone();
        let c_xfrm = resolve::xfrm(&ctx, item);
        let mapped =
            decompose_world_xfrm((to_slide * c_xfrm.affine()).as_coeffs(), c_xfrm.w, c_xfrm.h);
        checked(mapped.x)?;
        checked(mapped.y)?;
        checked(mapped.w)?;
        checked(mapped.h)?;
        next.xfrm = Some(mapped);
        unpacked_ids.push(next.id);
        unpacked.push(next);
    }
    let p = Arc::make_mut(&mut st.model);
    let slide = Arc::make_mut(&mut p.slides[page]);
    slide.shapes.remove(pos);
    for (offset, item) in unpacked.into_iter().enumerate() {
        slide.shapes.insert(pos + offset, item);
    }
    if let Some(origin) = st.origins.get_mut(&slide_id) {
        for cid in &unpacked_ids {
            origin.shapes.remove(cid);
        }
    }
    st.selection.shapes = unpacked_ids;
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
    #[test]
    fn shape_outline_align_distribute_group_and_transition_round_trip() {
        let mut s = session();
        let mut insert = |x: f64, y: f64, w: f64, h: f64| {
            let ok = s.execute(Command::InsertRect {
                x,
                y,
                width: w,
                height: h,
            });
            assert!(ok.is_ok());
            s.selection().shapes[0]
        };
        let a = insert(20., 40., 60., 40.);
        let b = insert(140., 100., 60., 40.);
        let c = insert(340., 160., 60., 40.);

        // Shape outline set + clear
        assert!(
            s.execute(Command::Content {
                action: ContentAction::ShapeLine {
                    rgb: Some(0xd04020),
                    width: Some(2.5),
                    dash: Some("dash".into()),
                }
            })
            .is_ok()
        );
        let line = s.model().slides[0].shape(c).and_then(|sh| sh.line.as_ref());
        assert!(line.is_some_and(|l| l.width == Some(2.5)));
        assert!(
            s.execute(Command::Content {
                action: ContentAction::ShapeLine {
                    rgb: None,
                    width: None,
                    dash: None,
                }
            })
            .is_ok()
        );
        let cleared = s.model().slides[0].shape(c).and_then(|sh| sh.line.as_ref());
        assert!(cleared.is_some_and(|l| l.fill == Some(Fill::None)));

        // Align three objects to Top (y0 = 40)
        assert!(s.select(0, vec![a, b, c]).is_ok());
        assert!(
            s.execute(Command::Content {
                action: ContentAction::AlignObjects {
                    edge: AlignEdge::Top
                }
            })
            .is_ok()
        );
        for id in [a, b, c] {
            let y = s.model().slides[0]
                .shape(id)
                .and_then(|sh| sh.xfrm)
                .map(|x| x.y);
            assert_eq!(y, Some(40.0));
        }

        // Distribute horizontally: a=[20..80], c=[340..400], b width 60 -> gap=(340-80-60)/2=100 -> b.x=180
        assert!(
            s.execute(Command::Content {
                action: ContentAction::DistributeObjects {
                    axis: DistributeAxis::Horizontal
                }
            })
            .is_ok()
        );
        let bx = s.model().slides[0]
            .shape(b)
            .and_then(|sh| sh.xfrm)
            .map(|x| x.x);
        assert!(bx.is_some_and(|x| (x - 180.0).abs() < 1e-6));

        // Group a & b, move group by (+30, +10), then ungroup and verify children moved
        assert!(s.select(0, vec![a, b]).is_ok());
        assert!(
            s.execute(Command::Content {
                action: ContentAction::GroupObjects
            })
            .is_ok()
        );
        assert_eq!(s.selection().shapes.len(), 1);
        let group_id = s.selection().shapes[0];
        assert!(
            s.execute(Command::Transform {
                dx: 30.,
                dy: 10.,
                dw: 0.,
                dh: 0.,
                rotation: 0.
            })
            .is_ok()
        );
        assert!(
            s.execute(Command::Content {
                action: ContentAction::UngroupObjects
            })
            .is_ok()
        );
        assert!(s.model().slides[0].shape(group_id).is_none());
        assert_eq!(s.selection().shapes, vec![a, b]);
        let ax = s.model().slides[0].shape(a).and_then(|sh| sh.xfrm);
        assert!(ax.is_some_and(|x| (x.x - 50.0).abs() < 1e-6 && (x.y - 50.0).abs() < 1e-6));

        // Slide transition + apply to all
        assert!(s.execute(Command::NewSlide).is_ok());
        assert!(s.select(0, vec![]).is_ok());
        assert!(
            s.execute(Command::Content {
                action: ContentAction::SlideTransition {
                    effect: "push".into(),
                    option: "l".into(),
                    duration_ms: 800,
                    advance_on_click: true,
                    advance_after_ms: Some(3000),
                }
            })
            .is_ok()
        );
        assert!(
            s.execute(Command::Content {
                action: ContentAction::ApplyTransitionAll
            })
            .is_ok()
        );
        for slide in &s.model().slides {
            let t = slide.transition.as_ref();
            assert!(t.is_some_and(|tr| tr.kind == "push"
                && tr.option == "l"
                && tr.duration_ms == 800
                && tr.advance_after_ms == Some(3000)));
        }
        assert!(
            s.execute(Command::Content {
                action: ContentAction::SlideTransition {
                    effect: "nonexistent".into(),
                    option: "".into(),
                    duration_ms: 500,
                    advance_on_click: true,
                    advance_after_ms: None,
                }
            })
            .is_err()
        );
    }
}
