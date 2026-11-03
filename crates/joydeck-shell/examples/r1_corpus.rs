//! Generate original, reproducible interoperability fixtures; no vendor artwork/fonts.
use joydeck_model::{text::RunProps, *};
use std::{path::Path, sync::Arc};
fn text(id: u32, value: &str, x: f64, y: f64, size: f64) -> Shape {
    let mut body = TextBody::from_text(value);
    for para in &mut body.paragraphs {
        para.def_props = RunProps {
            size: Some(size),
            font: Some("Microsoft YaHei".into()),
            ..Default::default()
        };
    }
    Shape {
        id: ShapeId(id),
        name: value.into(),
        xfrm: Some(Xfrm::new(x, y, 800., 80.)),
        text: Some(body),
        text_box: true,
        ..Default::default()
    }
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args().nth(1).ok_or("output directory required")?;
    let path = Path::new(&destination);
    std::fs::create_dir_all(path)?;
    let mut p = Presentation::default();
    p.slides.clear();
    for (i, (title, subtitle)) in [
        (
            "JoyDeck · 真实文稿预览",
            "Rust + GPUI / Shared CPU renderer",
        ),
        (
            "Shapes, theme & Unicode",
            "中文、English、العربية · 0123456789",
        ),
        (
            "Read once. Render everywhere.",
            "PPTX / .deckcraft → model → canvas + thumbnails + CLI",
        ),
    ]
    .iter()
    .enumerate()
    {
        let mut heading = text(100 + i as u32 * 10, title, 60., 70., 38.);
        heading.ph = Some(Placeholder {
            kind: PhType::Title,
            ..Default::default()
        });
        let slide = Slide {
            id: SlideId(i as u32 + 1),
            name: title.to_string(),
            notes: TextBody::from_text(&format!("Speaker notes / 演讲备注 {}", i + 1)),
            shapes: vec![
                Shape {
                    id: ShapeId(500 + i as u32),
                    xfrm: Some(Xfrm::new(0., 0., 16., 540.)),
                    fill: Some(Fill::solid(ColorRef::rgb(Rgba {
                        r: 34,
                        g: 111,
                        b: 76,
                        a: 255,
                    }))),
                    ..Default::default()
                },
                heading,
                text(200 + i as u32, subtitle, 64., 180., 22.),
                Shape {
                    id: ShapeId(600 + i as u32),
                    xfrm: Some(Xfrm::new(64., 310., 280. + 80. * i as f64, 90.)),
                    fill: Some(Fill::solid(ColorRef::scheme(SchemeSlot::Accent1))),
                    ..Default::default()
                },
                text(300 + i as u32, "JOYOFFICE / R1", 64., 440., 18.),
            ],
            ..Default::default()
        };
        p.slides.push(Arc::new(slide));
    }
    std::fs::write(path.join("r1-preview.deckcraft"), joydeck_format::save(&p)?)?;
    std::fs::write(path.join("r1-preview.pptx"), joydeck_pptx::export(&p)?)?;
    Ok(())
}
