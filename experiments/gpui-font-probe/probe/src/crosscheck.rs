//! 字形交叉核对（W02 任务卡第 3 条的补充机器证据，无窗口）。
//!
//! 链路本身只用 HarfRust 的字形 id/位置；本模块把同一份（改写后的）
//! 字体字节按唯一家族名注册进 gpui，用 gpui 公开的 `layout_line`
//! （macOS 下由 CoreText 整形）对同一文本整形，比较两侧的字形 id
//! 序列与总宽度。两侧整形器不同，但读取的是同一份字节的 cmap/GSUB：
//! 序列一致 ⇒ 解析到的 FontId 确实指向我们注册的字节。
//!
//! 声明：这是"交叉核对"，不参与、不替代链路；链路不经过 layout_line。

use std::path::Path;

use gpui::{App, Application, TextRun, div, prelude::*, px};
use joyword_foundation::Emu;
use joyword_layout::{FeatureSet, shape};

use gpui_font_probe::{LINES, ProbeFont, build_scene, emu_to_px, json, load_probe_font};

const FONT_SIZE_PX: f32 = 16.0;

pub fn run(out: &Path) {
    let font = load_probe_font(
        std::path::Path::new(gpui_font_probe::DEFAULT_FONT),
        gpui_font_probe::DEFAULT_FACE,
    )
    .unwrap_or_else(|error| panic!("probe font: {error}"));
    std::fs::create_dir_all(out).expect("create evidence dir");
    let out = out.to_owned();
    Application::new().run(move |cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![std::borrow::Cow::Owned(font.rewritten.to_vec())])
            .expect("add_fonts");
        let font_id = cx
            .text_system()
            .resolve_font(&by_family(&font.names.family));

        // layout_line 在 0.2.2 属于 WindowTextSystem，需要一个窗口。
        // 开一个最小窗口完成度量后立即退出；窗口只存在一帧以内。
        struct Unit;
        impl gpui::Render for Unit {
            fn render(
                &mut self,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> impl gpui::IntoElement {
                div().size_full()
            }
        }
        let window = cx
            .open_window(gpui::WindowOptions::default(), |_, cx| cx.new(|_| Unit))
            .expect("open measure window");
        let out2 = out.clone();
        let result = window.update(cx, |_, window, _| {
            measure_and_write(window, &font, font_id, &out2);
        });
        if let Err(error) = result {
            eprintln!("measure window update failed: {error}");
        }
        cx.quit();
    });
}

fn measure_and_write(
    window: &mut gpui::Window,
    font: &ProbeFont,
    font_id: gpui::FontId,
    out: &Path,
) {
    {
        let ts = window.text_system();

        let mut lines_json = Vec::new();
        for (line_index, text) in LINES.iter().enumerate() {
            // 链路侧：HarfRust 同一份字节。
            let run = shape(
                &font.loaded,
                text,
                Emu::new(12 * Emu::PER_POINT),
                FeatureSet::default(),
            );
            let harfrust_ids: Vec<u32> = run.glyphs.iter().map(|g| u32::from(g.glyph_id)).collect();
            let harfrust_width_px = emu_to_px(run.width, 1.0);
            // 同一字节再按"连字/字距全开"整形：若 CoreText 序列与此一致，
            // 则两侧差异只来自特性策略，而不是字体字节不同。
            let liga_run = shape(
                &font.loaded,
                text,
                Emu::new(12 * Emu::PER_POINT),
                FeatureSet {
                    kerning: true,
                    standard_ligatures: true,
                    contextual_alternates: true,
                },
            );
            let harfrust_liga_ids: Vec<u32> = liga_run
                .glyphs
                .iter()
                .map(|g| u32::from(g.glyph_id))
                .collect();

            // 交叉核对侧：CoreText 经注册 FontId（不参与链路）。
            let text_run = TextRun {
                len: text.len(),
                font: by_family(&font.names.family),
                color: gpui::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let layout = ts.layout_line(text, px(FONT_SIZE_PX), &[text_run], None);
            let coretext: Vec<(u32, f32)> = layout
                .runs
                .iter()
                .flat_map(|run| run.glyphs.iter())
                .map(|glyph| (glyph.id.0, f32::from(glyph.position.x)))
                .collect();
            let coretext_ids: Vec<u32> = coretext.iter().map(|(id, _)| *id).collect();
            let coretext_width = f32::from(layout.width);

            let ids_equal = harfrust_ids == coretext_ids;
            let ids_equal_with_liga = harfrust_liga_ids == coretext_ids;
            let width_delta = (coretext_width as f64 - harfrust_width_px).abs();
            lines_json.push(format!(
                "{{\n{}\n{}}}",
                [
                    json::field("text", &json::string(text), 2),
                    json::field("glyph_count_harfrust", &harfrust_ids.len().to_string(), 2),
                    json::field("glyph_count_coretext", &coretext_ids.len().to_string(), 2),
                    json::field("ids_equal_default_features", &ids_equal.to_string(), 2),
                    json::field("ids_equal_with_liga", &ids_equal_with_liga.to_string(), 2),
                    json::field(
                        "harfrust_ids",
                        &format!(
                            "\"{}\"",
                            harfrust_ids
                                .iter()
                                .map(u32::to_string)
                                .collect::<Vec<_>>()
                                .join(",")
                        ),
                        2,
                    ),
                    json::field(
                        "coretext_ids",
                        &format!(
                            "\"{}\"",
                            coretext_ids
                                .iter()
                                .map(u32::to_string)
                                .collect::<Vec<_>>()
                                .join(",")
                        ),
                        2,
                    ),
                    json::field("harfrust_width_lpx", &format!("{harfrust_width_px:.3}"), 2,),
                    json::field("coretext_width_lpx", &format!("{coretext_width:.3}"), 2),
                    json::field("width_delta_lpx", &format!("{width_delta:.3}"), 2,),
                ]
                .join(",\n"),
                "  "
            ));
            let _ = line_index;
        }

        let scene = build_scene(&font.loaded);
        let scene_line = format!(
            "场景字形数（run0）: {}，全部来自 HarfRust 同一份字节",
            scene.runs[0].glyphs.len()
        );
        let document = [
            "{".to_owned(),
            format!(
                "{},",
                json::field(
                    "probe",
                    &json::string(
                        "字形交叉核对：HarfRust（链路） vs CoreText via 注册 FontId（非链路）"
                    ),
                    1
                )
            ),
            format!(
                "{},",
                json::field(
                    "font",
                    &format!(
                        "{{\n{}\n{}}}",
                        [
                            json::field("family", &json::string(&font.names.family), 2),
                            json::field("sha256_original", &json::string(&font.original_sha256), 2),
                            json::field(
                                "sha256_extracted",
                                &json::string(&font.extracted_sha256),
                                2
                            ),
                            json::field(
                                "sha256_rewritten",
                                &json::string(&font.rewritten_sha256),
                                2
                            ),
                            json::field("font_id", &font_id.0.to_string(), 2),
                        ]
                        .join(",\n"),
                        "  "
                    ),
                    1
                )
            ),
            format!(
                "{},",
                json::field("font_size_lpx", &FONT_SIZE_PX.to_string(), 1)
            ),
            format!(
                "{},",
                json::field(
                    "lines",
                    &format!("[\n{}\n{}]", lines_json.join(",\n"), "  "),
                    1,
                )
            ),
            json::field("scene_note", &json::string(&scene_line), 1),
            "}".to_owned(),
        ]
        .join("\n");
        let path = out.join("glyph-crosscheck.json");
        std::fs::write(&path, document + "\n")
            .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
        println!("glyph crosscheck written to {}", path.display());
    }
}

fn by_family(family: &str) -> gpui::Font {
    gpui::Font {
        family: gpui::SharedString::from(family.to_owned()),
        features: gpui::FontFeatures::default(),
        fallbacks: None,
        weight: gpui::FontWeight::default(),
        style: gpui::FontStyle::default(),
    }
}
