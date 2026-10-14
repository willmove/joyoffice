//! 系统同名字体冲突实验（W02 任务卡第 4 条，无窗口、CPU 度量）。
//!
//! 问题：gpui 的 `TextSystem::add_fonts` 不返回 FontId，`resolve_font`
//! 只按家族名/字重解析。若文档字体的家族名与系统已装字体相同，解析
//! 结果可能指向系统字节而不是注册的字节。
//!
//! 实验：把"冬青黑体"（不同字节）的 name 表改写成与系统
//! "Songti SC Regular" 完全同名后 add_fonts，度量同名解析在注册前后的
//! FontId 与单字形包围盒是否漂移：
//!
//! - 漂移 ⇒ 名称解析不稳定，同名冲突真实存在；
//! - 不漂移 ⇒ 注册字体在同名竞争中被忽略（或被去重），同样说明按名
//!   解析无法证明指向哪份字节。
//!
//! 正控制：同一份 Songti 字节改写为唯一家族名后注册、按唯一名解析，
//! 其包围盒应与系统解析一致（同一字节的直接证据）。

use std::path::Path;

use gpui::{App, Application, Font, FontFeatures, FontStyle, FontWeight, Pixels, px};
use joyword_foundation::Emu;

use gpui_font_probe::{
    DEFAULT_FACE, DEFAULT_FONT, IMPOSTOR_FACE, IMPOSTOR_FONT, json, load_probe_font, name,
    sha256_hex,
};

const MEASURE_FONT_SIZE: f32 = 32.0;
/// Songti 与冬青黑体都有映射的汉字（两侧字形度量必然不同设计的字符）。
const PROBE_CHAR: char = '永';

fn by_family(family: &str) -> Font {
    Font {
        family: gpui::SharedString::from(family.to_owned()),
        features: FontFeatures::default(),
        fallbacks: None,
        weight: FontWeight::default(),
        style: FontStyle::default(),
    }
}

/// 从记录里取 platform 1（Macintosh，ASCII）的某个 nameID。
fn mac_string(records: &[(u16, u16, u16, String)], name_id: u16) -> Option<String> {
    records
        .iter()
        .find(|(id, platform, _, _)| *id == name_id && *platform == 1)
        .map(|(_, _, _, text)| text.clone())
}

fn bounds_json(bounds: &gpui::Bounds<Pixels>, indent: usize) -> String {
    let one = |value: Pixels| format!("{:.3}", f32::from(value));
    [
        format!(
            "{}\"origin_x\": {}",
            "  ".repeat(indent),
            one(bounds.origin.x)
        ),
        format!(
            "{}\"origin_y\": {}",
            "  ".repeat(indent),
            one(bounds.origin.y)
        ),
        format!(
            "{}\"width\": {}",
            "  ".repeat(indent),
            one(bounds.size.width)
        ),
        format!(
            "{}\"height\": {}",
            "  ".repeat(indent),
            one(bounds.size.height)
        ),
    ]
    .join(",\n")
}

pub fn run(out: &Path) {
    // 1. 真身：Songti SC Regular（系统已装，macOS 路径）。
    let songti = load_probe_font(std::path::Path::new(DEFAULT_FONT), DEFAULT_FACE)
        .unwrap_or_else(|error| panic!("load songti: {error}"));
    // 2. 顶替者：冬青黑体 face 0，改写为与 Songti face 6 完全相同的名字。
    let impostor_path = std::path::Path::new(IMPOSTOR_FONT);
    let impostor_bytes = std::fs::read(impostor_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", impostor_path.display()));
    let impostor_sha = sha256_hex(&impostor_bytes);
    let songti_records = name::read_names(&songti.original, DEFAULT_FACE)
        .unwrap_or_else(|error| panic!("songti names: {error}"));
    let impostor_names = name::NewNames {
        family: mac_string(&songti_records, 1).expect("songti mac family"),
        subfamily: mac_string(&songti_records, 2).unwrap_or_else(|| "Regular".into()),
        full: mac_string(&songti_records, 4).unwrap_or_else(|| "Songti SC".into()),
        postscript: mac_string(&songti_records, 6).expect("songti mac postscript"),
    };
    // 与正控制同样的两步：先抽出单字体（CGFont 不认 TTC 的非 0 face），
    // 再改写为与系统 Songti SC 完全同名。
    let impostor_extracted = name::extract_face(&impostor_bytes, IMPOSTOR_FACE)
        .unwrap_or_else(|error| panic!("extract impostor: {error}"));
    let impostor_rewritten = name::rename_face(&impostor_extracted, 0, &impostor_names)
        .unwrap_or_else(|error| panic!("rename impostor: {error}"));
    let impostor_rewritten_sha = sha256_hex(&impostor_rewritten);

    std::fs::create_dir_all(out).expect("create evidence dir");
    let out = out.to_owned();
    Application::new().run(move |cx: &mut App| {
        let ts = cx.text_system();
        let size = px(MEASURE_FONT_SIZE);

        // 注册前基线：系统 "Songti SC" 解析。
        let fid_before = ts.resolve_font(&by_family(&songti.original_family_mac));
        let bounds_before = ts
            .typographic_bounds(fid_before, size, PROBE_CHAR)
            .expect("typographic bounds before");

        // 注册顶替者（不同字节、完全同名）。
        let add_impostor = ts
            .add_fonts(vec![std::borrow::Cow::Owned(impostor_rewritten.clone())])
            .is_ok();

        // 注册后同名解析。
        let fid_after = ts.resolve_font(&by_family(&songti.original_family_mac));
        let bounds_after = ts
            .typographic_bounds(fid_after, size, PROBE_CHAR)
            .expect("typographic bounds after");

        // 正控制：同一份 Songti 字节（唯一家族名）注册并按唯一名解析。
        let add_probe = ts.add_fonts(vec![std::borrow::Cow::Owned(songti.rewritten.to_vec())]).is_ok();
        let fid_probe = ts.resolve_font(&by_family(&songti.names.family));
        let bounds_probe = ts
            .typographic_bounds(fid_probe, size, PROBE_CHAR)
            .expect("typographic bounds probe");

        // 参照：系统冬青黑体按本名解析（确认两侧字体度量本就不同）。
        let fid_hiragino = ts.resolve_font(&by_family("Hiragino Sans GB"));
        let bounds_hiragino = ts
            .typographic_bounds(fid_hiragino, size, PROBE_CHAR)
            .expect("typographic bounds hiragino");

        let same_id = fid_before == fid_after;
        let same_bounds = bounds_json(&bounds_before, 0) == bounds_json(&bounds_after, 0);
        let probe_matches_system =
            bounds_json(&bounds_probe, 0) == bounds_json(&bounds_before, 0);
        let conclusion = if !same_id || !same_bounds {
            "同名解析在注册顶替字体后发生漂移：按名解析无法绑定字节，冲突真实存在"
        } else if !probe_matches_system {
            "同名解析未漂移，但唯一名注册字体的度量与系统不一致：需要人工复核字体版本"
        } else {
            "同名解析未漂移（注册字体在同名竞争中被忽略或去重）：按名解析仍无法证明指向哪份字节"
        };

        let document = [
            "{".to_owned(),
            format!(
                "{},",
                json::field(
                    "probe",
                    &json::string("同名冲突实验：不同字节顶替系统家族名"),
                    1
                )
            ),
            format!(
                "{},",
                json::field(
                    "measure",
                    &format!(
                        "{{\n{}\n{}}}",
                        [
                            json::field("char", &json::string(PROBE_CHAR.to_string().as_str()), 2),
                            json::field("font_size_px", &MEASURE_FONT_SIZE.to_string(), 2),
                        ]
                        .join(",\n"),
                        "  "
                    ),
                    1
                )
            ),
            format!(
                "{},",
                json::field(
                    "songti",
                    &format!(
                        "{{\n{}\n{}}}",
                        [
                            json::field("path", &json::string(DEFAULT_FONT), 2),
                            json::field("face", &DEFAULT_FACE.to_string(), 2),
                            json::field("family", &json::string(&songti.original_family), 2),
                            json::field("sha256", &json::string(&songti.original_sha256), 2),
                            json::field("rewritten_sha256", &json::string(&songti.rewritten_sha256), 2),
                            json::field("rewritten_family", &json::string(&songti.names.family), 2),
                        ]
                        .join(",\n"),
                        "  "
                    ),
                    1
                )
            ),
            format!(
                "{},",
                json::field(
                    "impostor",
                    &format!(
                        "{{\n{}\n{}}}",
                        [
                            json::field("path", &json::string(IMPOSTOR_FONT), 2),
                            json::field("face", &IMPOSTOR_FACE.to_string(), 2),
                            json::field("sha256", &json::string(&impostor_sha), 2),
                            json::field("rewritten_sha256", &json::string(&impostor_rewritten_sha), 2),
                            json::field("family", &json::string(&impostor_names.family), 2),
                            json::field("postscript", &json::string(&impostor_names.postscript), 2),
                            json::field("license_note", &json::string("macOS 系统内置字体（Apple 专有许可）。仅本机实验输入，不提交、不重分发。"), 2),
                        ]
                        .join(",\n"),
                        "  "
                    ),
                    1
                )
            ),
            format!(
                "{},",
                json::field(
                    "results",
                    &format!(
                        "{{\n{}\n{}}}",
                        [
                            json::field("add_impostor_ok", &add_impostor.to_string(), 2),
                            json::field("add_probe_ok", &add_probe.to_string(), 2),
                            json::field("font_id_before", &fid_before.0.to_string(), 2),
                            json::field("font_id_after", &fid_after.0.to_string(), 2),
                            json::field("font_id_probe", &fid_probe.0.to_string(), 2),
                            json::field("font_id_hiragino", &fid_hiragino.0.to_string(), 2),
                            json::field("same_font_id", &same_id.to_string(), 2),
                            json::field("same_bounds", &same_bounds.to_string(), 2),
                            json::field("probe_bounds_match_system", &probe_matches_system.to_string(), 2),
                            format!("{}\"bounds_before\": {{\n{}\n{}}}", "  ".repeat(2), bounds_json(&bounds_before, 3), "  ".repeat(2)),
                            format!("{}\"bounds_after\": {{\n{}\n{}}}", "  ".repeat(2), bounds_json(&bounds_after, 3), "  ".repeat(2)),
                            format!("{}\"bounds_probe\": {{\n{}\n{}}}", "  ".repeat(2), bounds_json(&bounds_probe, 3), "  ".repeat(2)),
                            format!("{}\"bounds_hiragino\": {{\n{}\n{}}}", "  ".repeat(2), bounds_json(&bounds_hiragino, 3), "  ".repeat(2)),
                        ]
                        .join(",\n"),
                        "  "
                    ),
                    1
                )
            ),
            format!("{},", json::field("conclusion", &json::string(conclusion), 1)),
            format!(
                "{},",
                json::field(
                    "note",
                    &json::string("bounds 为 CoreText 在 32px 下 '永' 的包围盒（gpui typographic_bounds，CPU 度量）"),
                    1
                )
            ),
            "}".to_owned(),
        ]
        .join("\n");
        let path = out.join("conflict.json");
        std::fs::write(&path, document + "\n")
            .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
        println!("conflict check written to {}", path.display());
        println!("conclusion: {conclusion}");
        let _ = Emu::PER_POINT;
        cx.quit();
    });
}
