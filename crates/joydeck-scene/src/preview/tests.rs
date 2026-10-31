use super::*;
#[test]
fn mixed_bold_cjk_emoji_and_accents_use_real_glyphs() {
    let options = usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };
    let mut o = joydeck_model::identity::object("text");
    o.text = "中文😀 e\u{301}".into();
    let mut style = joydeck_model::text::Style::for_object(&o);
    style.bold = true;
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="100"><text x="0" y="40" font-size="24">{}</text></svg>"#,
        span_markup(&o.text, &style)
    );
    let tree = usvg::Tree::from_str(&svg, &options).unwrap();
    fn collect(group: &usvg::Group, tree: &usvg::Tree, text: &mut String) {
        for node in group.children() {
            match node {
                usvg::Node::Group(g) => collect(g, tree, text),
                usvg::Node::Text(t) => {
                    for span in t.layouted() {
                        for glyph in &span.positioned_glyphs {
                            assert_ne!(glyph.id.0, 0, "missing glyph {:?}", glyph.text);
                            let face = tree.fontdb().face(glyph.font).unwrap();
                            assert!(
                                face.families
                                    .iter()
                                    .all(|(name, _)| !name.contains("LastResort")),
                                "placeholder font used for {:?}",
                                glyph.text
                            );
                            text.push_str(&glyph.text);
                        }
                    }
                }
                _ => (),
            }
        }
    }
    let mut text = String::new();
    collect(tree.root(), &tree, &mut text);
    for part in ["中", "文", "😀", "e\u{301}"] {
        assert!(text.contains(part), "lost {part:?}: {text:?}");
    }
}
