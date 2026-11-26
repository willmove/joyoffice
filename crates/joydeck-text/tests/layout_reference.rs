//! A deterministic probe for comparing line and glyph geometry across builds on the same host.

use joydeck_geom::{Point, Rect, Xfrm};
use joydeck_model::resolve::Ctx;
use joydeck_model::text::{Anchor, AutoFit, Bullet, TabStop, TextBody, TextDir};
use joydeck_model::{Presentation, Shape};
use joydeck_text::{Affinity, NoFields, Opts, Pos, layout};

#[test]
fn layout_reference() -> Result<(), &'static str> {
    let presentation = Presentation::default();
    let slide = presentation.slides.first().ok_or("missing slide")?;
    let ctx = Ctx::for_slide(&presentation, slide).ok_or("missing slide layout")?;
    let cases = [
        (
            "latin",
            "Office AV fi word/word and verylongunbreakableword",
            "Inter",
        ),
        ("mixed", "abc (123) سَلَام world 45 עברית.", "Arial"),
        ("rtl", "أبجد 12 (text) הַשָּׁלוֹם AB", "Arial"),
        (
            "japanese",
            "日本語の「文章。」あいうっかな，よこー文字",
            "Yu Gothic",
        ),
        ("korean", "한글문서 배치 테스트 가나다라", "Malgun Gothic"),
        (
            "cluster",
            "A\u{0301} e\u{0301}\u{0327} 👩🏽‍💻 🇨🇳 ✈️ 1️⃣",
            "Inter",
        ),
        ("tabs", "Label\t103.5\tItem\u{b}More\t12.7\tEnd", "Calibri"),
        (
            "vertical",
            "ABC縦書き。日本語と Latin\n次の段落",
            "Yu Gothic",
        ),
        (
            "columns",
            "First paragraph with words\nSecond paragraph\nThird paragraph\nFourth paragraph",
            "Inter",
        ),
        (
            "shrink",
            "A very long line that must fit when the font is too large\nAnother paragraph",
            "Inter",
        ),
    ];

    for (name, text, family) in cases {
        for width in [63.0, 121.0, 244.0] {
            for size in [16.0, 24.0] {
                let height = if name == "columns" || name == "shrink" {
                    51.0
                } else {
                    140.0
                };
                let mut body = TextBody::from_text(text);
                body.body.anchor = Some(Anchor::Top);
                if name == "vertical" {
                    body.body.vert = Some(TextDir::EaVertical);
                }
                if name == "columns" {
                    body.body.columns = Some(2);
                    body.body.rtl_col = Some(true);
                }
                if name == "shrink" {
                    body.body.autofit = Some(AutoFit::Shrink {
                        font_scale: Some(1.0),
                        line_reduction: 0.0,
                    });
                }
                for paragraph in &mut body.paragraphs {
                    if name == "rtl" {
                        paragraph.props.rtl = Some(true);
                    }
                    if name == "japanese" {
                        paragraph.props.hanging_punct = Some(true);
                    }
                    if name == "columns" {
                        paragraph.props.bullet = Some(Bullet::AutoNum {
                            scheme: "arabicPeriod".into(),
                            start_at: 3,
                        });
                    }
                    if name == "tabs" {
                        paragraph.props.tabs = Some(vec![TabStop {
                            pos: 75.0,
                            align: "dec".into(),
                        }]);
                    }
                    for run in &mut paragraph.runs {
                        run.props.font = Some(family.into());
                        run.props.size = Some(size);
                    }
                }
                let shape = Shape {
                    xfrm: Some(Xfrm::new(0.0, 0.0, width, height)),
                    text: Some(body),
                    text_box: true,
                    ..Default::default()
                };
                let body = shape.text.as_ref().ok_or("missing text")?;
                let laid = layout(
                    &ctx,
                    &shape,
                    body,
                    &Opts {
                        rect: Rect::new(0.0, 0.0, width, height),
                        fields: &NoFields,
                        prompt_color: None,
                        no_shrink: false,
                    },
                );
                assert!(!laid.lines.is_empty() && !laid.runs.is_empty(), "{name}");
                println!(
                    "REF case {name} {width} {size} {:.8} {:.8} {:.8} {}",
                    laid.font_scale, laid.content_width, laid.content_height, laid.overflow
                );
                for line in &laid.lines {
                    assert!(line.top.is_finite() && line.baseline.is_finite());
                    println!(
                        "REF line {} {} {} {} {:.8} {:.8} {:.8}",
                        line.para,
                        line.start,
                        line.end,
                        line.column,
                        line.top,
                        line.baseline,
                        line.bottom
                    );
                    let y = (line.top + line.bottom) / 2.0;
                    for (i, x) in line.caret_x.iter().copied().enumerate() {
                        let position = Pos {
                            para: line.para,
                            ch: line.start + i,
                        };
                        for affinity in [Affinity::Upstream, Affinity::Downstream] {
                            if let Some((cx, top, bot)) = laid.caret_at(position, affinity) {
                                println!(
                                    "REF caret {} {} {affinity:?} {cx:.8} {top:.8} {bot:.8}",
                                    position.para, position.ch
                                );
                            }
                        }
                        for offset in [-0.125, 0.125] {
                            let (hit, side) = laid.hit_affinity(Point::new(x + offset, y));
                            assert_eq!(
                                laid.boundaries.get(hit.para).and_then(|p| p.get(hit.ch)),
                                Some(&true),
                                "{name} {hit:?}"
                            );
                            println!(
                                "REF hit {:.8} {y:.8} {} {} {side:?}",
                                x + offset,
                                hit.para,
                                hit.ch
                            );
                        }
                    }
                }
                for run in &laid.runs {
                    println!(
                        "REF face {} {} {} {:.8}",
                        run.face.identity(),
                        run.para,
                        run.upright,
                        run.size
                    );
                    for (gid, x, y) in &run.glyphs {
                        println!("REF glyph {gid} {x:.8} {y:.8}");
                    }
                }
            }
        }
    }
    Ok(())
}
