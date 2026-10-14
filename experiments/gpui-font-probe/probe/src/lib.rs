//! W02 独立 GPUI 字体/字形绘制探针（ADR-010 线索验证，非正式升级）。
//!
//! 链路：系统 CJK 字体 → 按 SHA-256 改写 name 表为唯一家族名 →
//! 用改写后的字节（同一份）整形（joyword-layout/HarfRust）→ 页面场景
//! → 屏幕（vendored gpui 0.2.2 + `Window::paint_glyph`）与 PDF
//! （joyword-pdf/krilla）。屏幕消费场景中已有的字形 id/位置，绝不把
//! 原文交给系统文字重新整形后宣称同一链路。
//!
//! 文本为原创内容；系统字体不提交，只在报告记录来源、散列与许可。

pub mod json;
pub mod name;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use joyword_foundation::{Emu, ScreenScale};
use joyword_layout::{FeatureSet, GlyphRun, LoadedFont, PageScene, shape};
use sha2::{Digest, Sha256};

pub const DEFAULT_FONT: &str = "/System/Library/Fonts/Supplemental/Songti.ttc";
/// Songti.ttc 的 8 个 face 里第 6 号是 Songti SC Regular（宋體-簡 標準體）。
pub const DEFAULT_FACE: u32 = 6;
/// 冲突实验的"顶替者"字体：不同字节，将被改写为与 Songti SC 完全同名。
pub const IMPOSTOR_FONT: &str = "/System/Library/Fonts/Hiragino Sans GB.ttc";
pub const IMPOSTOR_FACE: u32 = 0;

/// 探针原创验证文本（中英混排 + 纯拉丁行）。
pub const LINES: [&str; 2] = [
    "JoyOffice 屏幕与 PDF 同一链路：同一字体字节，同一字形，同一位置。",
    "External glyph probe Aa Bb 0123, kunquat 问卷 2026.",
];

pub const FONT_SIZE_PT: i64 = 12;
pub const ORIGIN_X_MM: i64 = 25;
pub const BASELINE_MM: [i64; 2] = [40, 55];

/// 已加载并改写好的探针字体。整形、屏幕、PDF 全部使用 `rewritten` 字节。
pub struct ProbeFont {
    pub source_path: PathBuf,
    pub face_index: u32,
    pub original: Arc<[u8]>,
    pub original_sha256: String,
    pub original_family: String,
    /// Macintosh 平台（platform 1）记录的家族名，冲突实验用它注册/解析。
    pub original_family_mac: String,
    /// 抽取出的单字体文件（未改名）的散列。
    pub extracted_sha256: String,
    pub rewritten: Arc<[u8]>,
    pub rewritten_sha256: String,
    pub names: name::NewNames,
    pub loaded: LoadedFont,
}

pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 按原始字节散列生成唯一家族名，使 resolve_font 只可能命中本探针注册
/// 的那一份字节（ADR-010 候选做法）。
pub fn unique_names(original_sha256: &str, face_index: u32) -> name::NewNames {
    let short = &original_sha256[..12];
    name::NewNames {
        family: format!("JoyProbe {short} {face_index}"),
        subfamily: "Regular".to_owned(),
        full: format!("JoyProbe {short} {face_index}"),
        postscript: format!("JoyProbe-{short}-{face_index}"),
    }
}

/// Windows 平台 zh-CN 名称优先，其次 Windows 英文，其次 Mac。
fn pick_family(records: &[(u16, u16, u16, String)]) -> String {
    let preference = [(3, 2_048), (3, 1_028), (3, 1_033), (1, 0)];
    for (platform, language) in preference {
        for (name_id, platform_id, language_id, text) in records {
            if *name_id == 1 && *platform_id == platform && *language_id == language {
                return text.clone();
            }
        }
    }
    records
        .iter()
        .find(|(name_id, ..)| *name_id == 1)
        .map(|(_, _, _, text)| text.clone())
        .unwrap_or_else(|| "<no family>".to_owned())
}

/// 读取、抽取并改写一个 face。过程：
/// 1. 读取原文件（可为 TTC），记录整文件散列与原名；
/// 2. `extract_face` 把目标 face 打包成独立单字体文件（表字节不变；
///    gpui 0.2.2 的 macOS add_fonts 走 CGFont，TTC 只会装 face 0，
///    必须先拆）；
/// 3. `rename_face` 改写唯一家族名（只动 name 表）；
/// 4. 验证可解析、名称唯一后，`loaded` 作为后续整形/屏幕/PDF 的
///    唯一字节来源。
pub fn load_probe_font(path: &Path, face_index: u32) -> Result<ProbeFont, String> {
    let original: Arc<[u8]> = std::fs::read(path)
        .map_err(|error| format!("read {}: {error}", path.display()))?
        .into();
    let original_sha256 = sha256_hex(&original);
    let records = name::read_names(&original, face_index).map_err(|e| e.to_string())?;
    let original_family = pick_family(&records);
    let original_family_mac = records
        .iter()
        .find(|(name_id, platform, _, _)| *name_id == 1 && *platform == 1)
        .map(|(_, _, _, text)| text.clone())
        .unwrap_or_else(|| original_family.clone());
    let names = unique_names(&original_sha256, face_index);

    let extracted = name::extract_face(&original, face_index)
        .map_err(|error| format!("extract face: {error}"))?;
    let extracted_sha256 = sha256_hex(&extracted);
    // 抽取只允许改变 name 之外的布局；先证明除 name 外所有表与原
    // TTC 中该 face 的表逐字节相同（head 例外：checkSumAdjustment
    // 按规范重算）。
    verify_tables_unchanged(&original, face_index, &extracted, 0, "extract")?;

    let rewritten_bytes =
        name::rename_face(&extracted, 0, &names).map_err(|error| format!("rename: {error}"))?;
    let rewritten_sha256 = sha256_hex(&rewritten_bytes);
    verify_tables_unchanged(&extracted, 0, &rewritten_bytes, 0, "rename")?;

    // 改写后的解析验证：名称全部换新且不再含旧家族名。
    let rewritten_records = name::read_names(&rewritten_bytes, 0)
        .map_err(|error| format!("rewritten file unreadable: {error}"))?;
    for (name_id, platform_id, _, text) in &rewritten_records {
        if *name_id == 1 {
            if *text == original_family {
                return Err(format!(
                    "rewritten face still carries family {original_family}"
                ));
            }
            if (*platform_id == 1 || *platform_id == 3) && text != &names.family {
                return Err(format!("family not rewritten: {text}"));
            }
        }
    }
    if !rewritten_records
        .iter()
        .any(|(name_id, _, _, text)| *name_id == 6 && text == &names.postscript)
    {
        return Err("postscript name missing".to_owned());
    }
    if name::face_count(&rewritten_bytes).map_err(|e| e.to_string())? != 1 {
        return Err("rewritten file must be a single font".to_owned());
    }

    let rewritten: Arc<[u8]> = rewritten_bytes.into();
    let loaded = LoadedFont::from_bytes(rewritten.clone(), 0)
        .map_err(|error| format!("rewritten font unusable: {error:?}"))?;
    Ok(ProbeFont {
        source_path: path.to_owned(),
        face_index,
        original,
        original_sha256,
        original_family,
        original_family_mac,
        extracted_sha256,
        rewritten,
        rewritten_sha256,
        names,
        loaded,
    })
}

/// 逐表对比两个文件指定 face：除 `name`（重命名目标）与 `head`
/// （checkSumAdjustment 重算）外，所有表必须逐字节相同。
fn verify_tables_unchanged(
    before: &[u8],
    before_face: u32,
    after: &[u8],
    after_face: u32,
    stage: &str,
) -> Result<(), String> {
    let before_entries = name::table_tags(before, before_face).map_err(|e| e.to_string())?;
    for tag in before_entries {
        if &tag == b"name" || &tag == b"head" {
            continue;
        }
        let a = name::table_bytes(before, before_face, &tag).map_err(|e| e.to_string())?;
        let b = name::table_bytes(after, after_face, &tag).map_err(|e| e.to_string())?;
        if a != b {
            return Err(format!(
                "{stage} changed table {}",
                String::from_utf8_lossy(&tag)
            ));
        }
    }
    Ok(())
}

/// 与 T003 统一测试同型的场景：A4、12pt、两行原创文本。
pub fn build_scene(font: &LoadedFont) -> PageScene {
    let size = Emu::new(FONT_SIZE_PT * Emu::PER_POINT);
    let runs = LINES
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let run = shape(font, line, size, FeatureSet::default());
            GlyphRun::place(
                &run,
                Emu::new(ORIGIN_X_MM * Emu::PER_MM),
                Emu::new(BASELINE_MM[index] * Emu::PER_MM),
            )
        })
        .collect();
    PageScene {
        width: Emu::new(210 * Emu::PER_MM),
        height: Emu::new(297 * Emu::PER_MM),
        runs,
    }
}

/// 场景坐标 → 屏幕/输出换算（与 joyword-layout::scene 的公开函数同式）。
pub fn emu_to_px(length: Emu, zoom: f64) -> f64 {
    ScreenScale::new(zoom)
        .expect("zoom validated by caller")
        .logical_pixels(length)
}

/// 屏幕上第 `index` 个字形的绘制原点（逻辑像素，y 向下；字形 y 偏移按
/// PDF 侧同式取负，保持两出口一致）。
pub fn glyph_origin_px(run: &GlyphRun, index: usize, zoom: f64) -> (f64, f64) {
    let glyph = &run.glyphs[index];
    let x = Emu::new(run.origin_x.raw() + glyph.x.raw() + glyph.x_offset.raw());
    let y = Emu::new(run.baseline_y.raw() - glyph.y_offset.raw());
    (emu_to_px(x, zoom), emu_to_px(y, zoom))
}
