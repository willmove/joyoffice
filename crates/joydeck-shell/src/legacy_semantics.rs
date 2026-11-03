//! One-time transfer of validated v1 semantics into the sole v2 document model.
use joydeck_model::{
    Comment, CustomShow, MediaClip, MediaId, MediaItem, Presentation, Shape, ShapeId, ShapeKind,
    SlideId, Xfrm,
    text::{Action, Hyperlink},
};
use std::{collections::BTreeMap, sync::Arc};

pub(crate) fn apply(
    p: &mut Presentation,
    converted: &joydeck_legacy::Conversion,
) -> Result<(), String> {
    let state = &converted.supplement;
    let mut slides = BTreeMap::<String, SlideId>::new();
    let mut shapes = BTreeMap::<String, (usize, ShapeId)>::new();
    if p.slides.len() != converted.source_ids.len() {
        return Err("legacy slide mapping is incomplete".into());
    }
    for ((_, old_slide, objects), (page, slide)) in
        converted.source_ids.iter().zip(p.slides.iter().enumerate())
    {
        if objects.len() != slide.shapes.len() {
            return Err("legacy object mapping is incomplete".into());
        }
        slides.insert(old_slide.clone(), slide.id);
        for ((old, _), shape) in objects.iter().zip(&slide.shapes) {
            shapes.insert(old.clone(), (page, shape.id));
        }
    }
    for (page, (_, old_slide, objects)) in converted.source_ids.iter().enumerate() {
        let slide = Arc::make_mut(&mut p.slides[page]);
        slide
            .tags
            .push(("JoyOffice.LegacyId".into(), old_slide.clone()));
        for ((old, _), shape) in objects.iter().zip(&mut slide.shapes) {
            shape.tags.push(("JoyOffice.LegacyId".into(), old.clone()));
        }
    }
    for (id, access) in &state.accessibility {
        let &(page, shape) = shapes
            .get(id)
            .ok_or("legacy accessibility anchor missing")?;
        let shape = Arc::make_mut(&mut p.slides[page])
            .shape_mut(shape)
            .ok_or("mapped shape missing")?;
        shape.descr = access.alt.clone();
        shape.decorative = access.decorative;
    }
    for (old, order) in &state.reading_order {
        let slide = *slides
            .get(old)
            .ok_or("legacy reading order slide missing")?;
        for (rank, old) in order.iter().enumerate() {
            let &(page, id) = shapes
                .get(old)
                .ok_or("legacy reading order shape missing")?;
            if p.slides[page].id != slide {
                return Err("legacy reading order crosses slides".into());
            }
            Arc::make_mut(&mut p.slides[page])
                .shape_mut(id)
                .ok_or("mapped shape missing")?
                .tags
                .push(("JoyOffice.ReadingOrder".into(), rank.to_string()));
        }
    }
    for thread in &state.threads {
        let slide_id = *slides
            .get(&thread.slide)
            .ok_or("legacy comment slide missing")?;
        let page = p.slide_index(slide_id).ok_or("mapped slide missing")?;
        let shape = thread
            .object
            .as_ref()
            .map(|id| {
                shapes
                    .get(id)
                    .map(|x| x.1)
                    .ok_or("legacy comment shape missing")
            })
            .transpose()?;
        let mut messages = thread.messages.iter();
        let first = messages.next().ok_or("legacy comment has no messages")?;
        let message = |author: &str, text: &str| Comment {
            author: author.into(),
            text: text.into(),
            shape,
            ..Default::default()
        };
        let mut comment = message(&first.author, &first.text);
        comment.id = thread.id.clone();
        comment.resolved = thread.resolved;
        comment.replies = messages.map(|m| message(&m.author, &m.text)).collect();
        Arc::make_mut(&mut p.slides[page]).comments.push(comment);
    }
    for (old, destination) in &state.links {
        let &(page, id) = shapes.get(old).ok_or("legacy link source missing")?;
        let slide = *slides
            .get(destination)
            .ok_or("legacy link target missing")?;
        Arc::make_mut(&mut p.slides[page])
            .shape_mut(id)
            .ok_or("mapped shape missing")?
            .click = Some(Hyperlink {
            action: Action::Slide { slide },
            tooltip: String::new(),
            highlight_click: false,
            sound: None,
            stop_sound: false,
            zoom_return: None,
        });
    }
    p.show.loop_until_esc = state.loop_show;
    if !state.order.is_empty() {
        let order = state
            .order
            .iter()
            .map(|id| slides.get(id).copied().ok_or("legacy show page missing"))
            .collect::<Result<Vec<_>, _>>()?;
        p.custom_shows.push(CustomShow {
            name: "Legacy custom order".into(),
            slides: order,
        });
        p.show.custom_show = Some("Legacy custom order".into());
    }
    let mut media = BTreeMap::new();
    for (hash, asset) in &state.assets {
        let id = MediaId(p.alloc_id().ok_or("media id budget exhausted")?);
        p.media.push(MediaItem {
            id,
            name: format!("{hash}.{}", asset.extension),
            content_type: match asset.extension.as_str() {
                "wav" => "audio/wav",
                "mp3" => "audio/mpeg",
                "mp4" => "video/mp4",
                _ => return Err("unsupported legacy media type".into()),
            }
            .into(),
            data: Arc::new(asset.bytes.clone()),
            link: None,
        });
        media.insert(hash, id);
    }
    for clip in &state.clips {
        let asset = state
            .assets
            .get(&clip.asset)
            .ok_or("legacy media asset missing")?;
        let page = p
            .slide_index(
                *slides
                    .get(&clip.slide)
                    .ok_or("legacy media slide missing")?,
            )
            .ok_or("mapped slide missing")?;
        let id = ShapeId(p.alloc_id().ok_or("shape id budget exhausted")?);
        let video = asset.info.width > 0;
        let width = if video {
            p.slide_size.width.min(320.)
        } else {
            48.
        };
        let height = if video {
            width * f64::from(asset.info.height) / f64::from(asset.info.width)
        } else {
            48.
        };
        Arc::make_mut(&mut p.slides[page]).shapes.push(Shape {
            id,
            name: clip.id.clone(),
            xfrm: Some(Xfrm {
                x: 24.,
                y: 24.,
                w: width,
                h: height,
                ..Default::default()
            }),
            kind: ShapeKind::Media(MediaClip {
                media: *media.get(&clip.asset).ok_or("mapped media missing")?,
                video,
                trim_start_ms: (clip.start * 1000.).round() as u32,
                trim_end_ms: (clip.end * 1000.).round() as u32,
                duration_ms: (asset.info.duration * 1000.).round() as u32,
                width: asset.info.width,
                height: asset.info.height,
                volume: f64::from(clip.volume),
                autoplay: clip.autoplay,
                loop_play: clip.looping,
                play_across_slides: clip.cross_slide,
                ..Default::default()
            }),
            ..Default::default()
        });
    }
    Ok(())
}
