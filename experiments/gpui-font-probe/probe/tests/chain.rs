//! 无 GUI 的机器证据（W02 任务卡第 3、4 条）：
//! 1. name 表改写不改变整形结果（同一文本、同一字形 id/位置/宽度）；
//! 2. 改写后的文件可解析、名称唯一、同集合其他 face 不受影响；
//! 3. 同一场景同一字节出的 PDF 确定且可提取文本（与 T003 同判据）；
//! 4. 屏幕/文档坐标换算在缩放下线性。
//!
//! 字体来自系统路径（缺字体时跳过；`JOY_PROBE_FONT=path:index` 覆盖），
//! 不提交字体本体。

use std::path::Path;
use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{FeatureSet, LoadedFont, shape};
use joyword_pdf as pdf;

use gpui_font_probe::{
    DEFAULT_FACE, DEFAULT_FONT, LINES, ProbeFont, build_scene, emu_to_px, load_probe_font, name,
};

fn probe_font() -> Option<ProbeFont> {
    let spec = std::env::var("JOY_PROBE_FONT").ok();
    let candidates: Vec<(String, u32)> = match spec.as_deref().and_then(|s| s.rsplit_once(':')) {
        Some((path, index)) => vec![(path.to_owned(), index.parse().expect("face index"))],
        None => vec![(DEFAULT_FONT.to_owned(), DEFAULT_FACE)],
    };
    for (path, index) in candidates {
        if Path::new(&path).is_file() {
            return load_probe_font(Path::new(&path), index)
                .map(Some)
                .unwrap_or_else(|e| {
                    panic!("load {path} #{index}: {e}");
                });
        }
    }
    eprintln!("skipped: no CJK probe font (set JOY_PROBE_FONT=path:index)");
    None
}

#[test]
fn rewriting_names_preserves_shaping_exactly() {
    let Some(probe) = probe_font() else { return };
    let original =
        LoadedFont::from_bytes(probe.original.clone(), probe.face_index).expect("original loads");
    let size = Emu::new(12 * Emu::PER_POINT);
    for line in LINES {
        let a = shape(&original, line, size, FeatureSet::default());
        let b = shape(&probe.loaded, line, size, FeatureSet::default());
        assert_eq!(a.font, original.key());
        assert_eq!(b.font, probe.loaded.key());
        assert_ne!(a.font, b.font, "bytes changed, so the key must differ");
        assert_eq!(
            a.glyphs, b.glyphs,
            "glyph ids and positions must be identical"
        );
        assert_eq!(a.width, b.width);
        assert!(
            a.missing.is_empty(),
            "probe text must be fully covered: {:?}",
            a.missing
        );
    }
}

#[test]
fn rewritten_font_is_unique_and_source_collection_intact() {
    let Some(probe) = probe_font() else { return };
    // 改写产物是单字体文件，家族名全部唯一。
    let records = name::read_names(&probe.rewritten, 0).expect("readable");
    let families: Vec<&str> = records
        .iter()
        .filter(|(id, _, _, _)| *id == 1)
        .map(|(_, _, _, text)| text.as_str())
        .collect();
    assert!(!families.is_empty());
    assert!(
        families.iter().all(|family| *family == probe.names.family),
        "all family records renamed: {families:?}"
    );
    assert_ne!(families[0], probe.original_family);
    assert_eq!(name::face_count(&probe.rewritten).unwrap(), 1);
    // 除 name/head（改名/校验和重算）外的表在抽取与改名两步中逐字节
    // 不变——load_probe_font 内部已强制，这里再次显式抽查整形关键表。
    for tag in [b"cmap", b"glyf", b"hmtx", b"GSUB", b"GSUB"] {
        if let Ok(original_table) = name::table_bytes(&probe.original, probe.face_index, tag) {
            let rewritten_table =
                name::table_bytes(&probe.rewritten, 0, tag).expect("table present after rewrite");
            assert_eq!(
                original_table,
                rewritten_table,
                "table {} changed",
                String::from_utf8_lossy(tag)
            );
        }
    }
    // 原 TTC 文件完好：所有 face 仍可解析。
    let faces = name::face_count(&probe.original).expect("face count");
    for face in 0..faces {
        LoadedFont::from_bytes(probe.original.clone(), face)
            .unwrap_or_else(|error| panic!("source face {face} broken: {error:?}"));
    }
}

#[test]
fn pdf_from_rewritten_bytes_is_deterministic_and_extractable() {
    let Some(probe) = probe_font() else { return };
    let scene = build_scene(&probe.loaded);
    let fonts = [probe.loaded.clone()];
    let first = pdf::render(std::slice::from_ref(&scene), &fonts).expect("pdf renders");
    let second = pdf::render(std::slice::from_ref(&scene), &fonts).expect("pdf renders");
    assert_eq!(first, second, "same scene, same bytes");
    assert!(first.starts_with(b"%PDF-"));
    let contains = |needle: &[u8]| first.windows(needle.len()).any(|w| w == needle);
    assert!(contains(b"/ToUnicode"), "text stays extractable");
    assert!(
        contains(b"/FontFile2") || contains(b"/FontFile3"),
        "font embedded"
    );
    assert!(
        first.len() < probe.loaded.data().len() / 4,
        "subset, not the whole {}-byte font",
        probe.loaded.data().len()
    );
}

#[test]
fn screen_projection_is_linear_across_zooms() {
    let Some(probe) = probe_font() else { return };
    let scene = build_scene(&probe.loaded);
    let run = &scene.runs[0];
    for zoom in [0.5, 1.0, 1.5, 2.0] {
        let (x, y) = gpui_font_probe::glyph_origin_px(run, 0, zoom);
        let (base_x, base_y) = gpui_font_probe::glyph_origin_px(run, 0, 1.0);
        assert!((x - base_x * zoom).abs() < 1e-9, "x linear at {zoom}");
        assert!((y - base_y * zoom).abs() < 1e-9, "y linear at {zoom}");
    }
    // 12pt 在 100% 缩放下是 16 个逻辑像素（96dpi 约定）。
    assert_eq!(emu_to_px(Emu::new(12 * Emu::PER_POINT), 1.0), 16.0);
    let _: Option<Arc<[u8]>> = None;
}
