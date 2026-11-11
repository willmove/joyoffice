use gpui_kit::{prelude::*, *};
use joysheet_engine::model::{Anchor, Color, DrawingObject, MathExpr, Shape, ShapeFormat, ShapeKind, SmartArtLayout};

impl crate::desktop::Desktop {
    pub(crate) fn object_preview(&self, id: &str, cx: &App) -> Option<AnyElement> {
        let view = self.view()?;
        let p = self.form_params(cx).ok()?;
        let t = p["text"].as_str().unwrap_or("");
        let old =
            self.selected_object.as_ref().and_then(|o| view.sheet()?.shapes.iter().find(|s| s.id == o.id)).and_then(|s| s.format.object.as_ref());
        let kind = match id {
            "insert.smartArt" => "smartArt",
            "insert.wordArt" => "wordArt",
            "insert.equation" => "equation",
            "object.edit" => match old? {
                DrawingObject::SmartArt { .. } => "smartArt",
                DrawingObject::WordArt { .. } => "wordArt",
                DrawingObject::Equation { .. } => "equation",
                _ => return None,
            },
            _ => return None,
        };
        let object = match kind {
            "smartArt" => DrawingObject::SmartArt {
                layout: match p["layout"].as_str() {
                    Some("hierarchy") => SmartArtLayout::Hierarchy,
                    Some("cycle") => SmartArtLayout::Cycle,
                    Some("list") => SmartArtLayout::List,
                    _ => SmartArtLayout::Process,
                },
                nodes: t.lines().take(64).map(|s| s.chars().take(128).collect()).collect(),
            },
            "wordArt" => {
                DrawingObject::WordArt { warp: p["warp"].as_str().unwrap_or("textNoShape").into(), shadow: p["shadow"].as_bool().unwrap_or(true) }
            }
            _ => DrawingObject::Equation { expression: MathExpr::parse(t).ok()? },
        };
        let shape = Shape {
            id: 0,
            kind: ShapeKind::TextBox,
            anchor: Anchor { width: 400., height: 140., ..Default::default() },
            fill: Color::Auto,
            line: Color::Auto,
            text: t.chars().take(1024).collect(),
            format: ShapeFormat {
                object: Some(object),
                font_size: Some(if kind == "wordArt" { 36. } else { 22. }),
                text_color: p["color"].as_str().and_then(Color::from_hex),
                ..Default::default()
            },
        };
        let prims = joysheet_engine::drawing_scene::render(&view.workbook, view.sheet, &shape)?;
        Some(
            div()
                .w_full()
                .h(px(144.))
                .border(px(0.5))
                .border_color(rgb(0xd0d0d0))
                .bg(rgb(0xffffff))
                .child(
                    canvas(
                        |_, _, _| {},
                        move |b, _, w, cx| {
                            let k = (f32::from(b.size.width) / 400.).min(f32::from(b.size.height) / 140.);
                            for prim in &prims {
                                crate::charts::paint_primitive(prim, b.origin, k, w, cx);
                            }
                        },
                    )
                    .w_full()
                    .h_full(),
                )
                .into_any_element(),
        )
    }
}
