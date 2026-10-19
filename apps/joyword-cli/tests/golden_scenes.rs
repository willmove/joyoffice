//! Headless golden comparisons for the merged page scene, PDF and CPU raster.
//!
//! Each scenario is an original DOCX package imported with `joyword_docx` and
//! laid out with the committed Latin subset font. Regenerate the files under
//! `tests/golden/` with `JOYWORD_BLESS_GOLDEN=1`. Blessing is ignored when
//! `CI` is set. These checks are automated evidence only: they do not compare
//! with Word or WPS and they do not exercise the GUI.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use joyoffice_opc::{Budget, Method, write_archive};
use joyword_docx::import;
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, GlyphRun, LayoutCancellation, LayoutDiagnostic, LayoutLimits, LayoutNote,
    LayoutSnapshot, LineDiagnostic, LoadedFont, OverflowKind, PageScene, PaintedUnderline,
    SceneRect, SingleFontProfile, layout_document, layout_document_viewed, to_points,
};
use joyword_model::{
    Block, BlockSeq, Document, RevisionView, StyleId, TableCondition, sample_png_1x1,
};
use joyword_pdf::{PdfExport, PdfReport, inspect_pdf, links_from_document, render_export};
use joyword_render::{Projection, RasterCancellation, RasterLimits, RasterPage, rasterize};
use serde_json::{Value, json};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const PIC: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const OFFICE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const HEADER_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
const SETTINGS_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
const NUMBERING_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
const STYLES_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
const IMAGE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const HYPERLINK_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";

const TILE: u32 = 32;
/// Per-tile ink tolerance, in basis points of the tile (10000 = full).
/// Outline coverage is not proven identical on every architecture, so the
/// raster golden allows this slack. The scene digest stays exact EMU.
const TILE_TOLERANCE: i64 = 250;

fn test_font() -> LoadedFont {
    LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .expect("committed test face")
}

fn profile() -> SingleFontProfile {
    SingleFontProfile {
        size: Emu::new(12 * Emu::PER_POINT),
        leading: Emu::new(18 * Emu::PER_POINT),
        baseline: Emu::new(14 * Emu::PER_POINT),
        features: FeatureSet {
            kerning: false,
            standard_ligatures: false,
            contextual_alternates: false,
        },
    }
}

fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];
    let mut hash = [
        0x6a09_e667_u32,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let bit_len = u64::try_from(data.len()).expect("hash length") * 8;
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in message.as_chunks::<64>().0 {
        let mut words = [0_u32; 64];
        for (index, bytes) in chunk.as_chunks::<4>().0.iter().take(16).enumerate() {
            words[index] = u32::from_be_bytes(*bytes);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let mut a = hash[0];
        let mut b = hash[1];
        let mut c = hash[2];
        let mut d = hash[3];
        let mut e = hash[4];
        let mut f = hash[5];
        let mut g = hash[6];
        let mut h = hash[7];
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, add) in hash.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(add);
        }
    }
    let mut out = [0_u8; 32];
    for (index, word) in hash.iter().enumerate() {
        out[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn archive(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let stored: Vec<(&str, &[u8], Method)> = parts
        .iter()
        .map(|(name, data)| (*name, *data, Method::Stored))
        .collect();
    write_archive(&stored).expect("package")
}

fn content_types(overrides: &str, png: bool) -> String {
    let png = if png {
        r#"<Default Extension="png" ContentType="image/png"/>"#
    } else {
        ""
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>{png}<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>{overrides}</Types>"#
    )
}

fn package_rels() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"><Relationship Id="rId1" Type="{OFFICE}" Target="word/document.xml"/></Relationships>"#
    )
}

fn document_rels(inner: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}">{inner}</Relationships>"#
    )
}

fn document_xml(extra_ns: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"{extra_ns}><w:body>{body}</w:body></w:document>"#
    )
}

fn pack(
    body: &str,
    extra_ns: &str,
    rels: &str,
    extras: &[(&str, String)],
    png: Option<&[u8]>,
) -> Vec<u8> {
    let mut overrides = String::new();
    for (name, _) in extras {
        let kind = match *name {
            "word/header1.xml" | "word/header2.xml" | "word/header3.xml" => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"
            }
            "word/settings.xml" => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"
            }
            "word/numbering.xml" => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"
            }
            "word/styles.xml" => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"
            }
            other => panic!("unknown part {other}"),
        };
        overrides.push_str(&format!(
            r#"<Override PartName="/{name}" ContentType="{kind}"/>"#
        ));
    }
    let types = content_types(&overrides, png.is_some());
    let rels_xml = package_rels();
    let document = document_xml(extra_ns, body);
    let doc_rels = document_rels(rels);
    let mut names = vec![
        "[Content_Types].xml".to_string(),
        "_rels/.rels".to_string(),
        "word/document.xml".to_string(),
        "word/_rels/document.xml.rels".to_string(),
    ];
    let mut owned = vec![
        types.into_bytes(),
        rels_xml.into_bytes(),
        document.into_bytes(),
        doc_rels.into_bytes(),
    ];
    for (name, data) in extras {
        names.push((*name).to_string());
        owned.push(data.as_bytes().to_vec());
    }
    if let Some(png) = png {
        names.push("word/media/pixel.png".to_string());
        owned.push(png.to_vec());
    }
    let view: Vec<(&str, &[u8])> = names
        .iter()
        .zip(owned.iter())
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();
    archive(&view)
}

/// `w` and `h` are twips. Margins leave a body that the 18 pt profile can fill.
fn sect(width: i64, height: i64, inner: &str) -> String {
    format!(
        r#"<w:sectPr><w:pgSz w:w="{width}" w:h="{height}"/><w:pgMar w:top="400" w:right="360" w:bottom="400" w:left="360" w:header="200" w:footer="200" w:gutter="0"/>{inner}</w:sectPr>"#
    )
}

fn open_document(bytes: &[u8]) -> Document {
    match import(bytes, &Budget::default()) {
        Ok(result) => {
            let issues = result.document.validate();
            assert!(!issues.has_errors(), "import produced {issues:?}");
            result.document
        }
        Err(error) => panic!("import failed: {error}"),
    }
}

fn layout_of(document: &Document, font: &LoadedFont, view: Option<RevisionView>) -> LayoutSnapshot {
    let limits = LayoutLimits::default();
    let cancel = LayoutCancellation::default();
    let faces = FaceSet::uniform(font);
    let result = match view {
        None => layout_document(&document.snapshot(), faces, profile(), limits, &cancel),
        Some(view) => layout_document_viewed(
            &document.snapshot(),
            faces,
            profile(),
            limits,
            &cancel,
            view,
        ),
    };
    result.unwrap_or_else(|error| panic!("layout failed: {error:?}"))
}

fn scene_text(snapshot: &LayoutSnapshot) -> String {
    snapshot
        .pages()
        .iter()
        .enumerate()
        .map(|(index, page)| {
            let text: String = page.runs.iter().map(|run| run.text.as_str()).collect();
            format!("p{index}:{text}")
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn color_hex(red: u8, green: u8, blue: u8) -> String {
    format!("{red:02x}{green:02x}{blue:02x}")
}

fn rect_kind(rect: &SceneRect, content_left: i64) -> &'static str {
    if (rect.red, rect.green, rect.blue) == (0x05, 0x63, 0xc1) {
        return "hyperlink";
    }
    if rect.height.raw() > rect.width.raw()
        && rect.x.raw().saturating_add(rect.width.raw()) <= content_left
    {
        return "margin-bar";
    }
    if rect.width.raw() > 0 && rect.height.raw().saturating_mul(4) < rect.width.raw() {
        return "stroke";
    }
    "fill"
}

fn underline_name(kind: PaintedUnderline) -> &'static str {
    match kind {
        PaintedUnderline::Single => "single",
        PaintedUnderline::Double => "double",
        PaintedUnderline::Dotted => "dotted",
    }
}

fn run_json(run: &GlyphRun) -> Value {
    let glyphs: Vec<Value> = run
        .glyphs
        .iter()
        .map(|glyph| {
            json!({
                "advance_emu": glyph.advance.raw(),
                "id": glyph.glyph_id,
                "x_emu": glyph.x.raw(),
                "x_offset_emu": glyph.x_offset.raw(),
                "y_offset_emu": glyph.y_offset.raw(),
            })
        })
        .collect();
    json!({
        "advance_emu": run.advance(),
        "baseline_y_emu": run.baseline_y.raw(),
        "bold": run.bold,
        "color": color_hex(run.color[0], run.color[1], run.color[2]),
        "glyphs": glyphs,
        "italic": run.italic,
        "origin_x_emu": run.origin_x.raw(),
        "size_emu": run.size.raw(),
        "underline": run.underline.map(|paint| underline_name(paint.kind)),
    })
}

fn note_json(note: &LayoutNote) -> Value {
    match note {
        LayoutNote::KeepTallerThanPage { paragraph } => {
            json!({"kind": "keep-taller-than-page", "paragraph": paragraph.to_string()})
        }
        LayoutNote::StoryContentSkipped { paragraph } => {
            json!({"kind": "story-content-skipped", "paragraph": paragraph.to_string()})
        }
        LayoutNote::StoryExceedsMargin { story } => {
            json!({"kind": "story-exceeds-margin", "story": story.to_string()})
        }
        LayoutNote::RowTallerThanPage { row } => {
            json!({"kind": "row-taller-than-page", "row": row.to_string()})
        }
        LayoutNote::HeaderTallerThanPage { table } => {
            json!({"kind": "header-taller-than-page", "table": table.to_string()})
        }
        LayoutNote::ContinuousPageSplit { section } => {
            json!({"kind": "continuous-page-split", "section": section.to_string()})
        }
        LayoutNote::PageNumberParity { section } => {
            json!({"kind": "page-number-parity", "section": section.to_string()})
        }
        LayoutNote::PageFieldCached { paragraph } => {
            json!({"kind": "page-field-cached", "paragraph": paragraph.to_string()})
        }
    }
}

fn range_text(range: &std::ops::Range<usize>) -> String {
    format!("{}..{}", range.start, range.end)
}

fn diagnostic_json(item: &LayoutDiagnostic) -> Value {
    let (kind, extra) = match &item.diagnostic {
        LineDiagnostic::Overflow {
            source,
            width,
            available,
            kind,
        } => (
            "overflow",
            json!({
                "available_emu": available.raw(),
                "bytes": range_text(&source.bytes),
                "overflow": match kind {
                    OverflowKind::SingleCluster => "single-cluster",
                    OverflowKind::UnbreakableSpan => "unbreakable-span",
                },
                "scalars": range_text(&source.scalars),
                "width_emu": width.raw(),
            }),
        ),
        LineDiagnostic::MissingGlyph { source } => (
            "missing-glyph",
            json!({
                "bytes": range_text(&source.bytes),
                "scalars": range_text(&source.scalars),
            }),
        ),
        LineDiagnostic::Numbering { detail } => ("numbering", json!({"detail": detail})),
    };
    json!({
        "diagnostic": extra,
        "kind": kind,
        "paragraph": item.paragraph.to_string(),
    })
}

fn scene_digest(snapshot: &LayoutSnapshot) -> Value {
    let pages: Vec<Value> = snapshot
        .pages()
        .iter()
        .enumerate()
        .map(|(index, page)| {
            let geometry = snapshot.page_geometry()[index];
            let rects: Vec<Value> = page
                .rects
                .iter()
                .map(|rect| {
                    json!({
                        "color": color_hex(rect.red, rect.green, rect.blue),
                        "height_emu": rect.height.raw(),
                        "kind": rect_kind(rect, geometry.left.raw()),
                        "width_emu": rect.width.raw(),
                        "x_emu": rect.x.raw(),
                        "y_emu": rect.y.raw(),
                    })
                })
                .collect();
            let images: Vec<Value> = page
                .images
                .iter()
                .map(|image| {
                    json!({
                        "height_emu": image.height.raw(),
                        "sha256": hex_bytes(&sha256(&image.bytes)),
                        "width_emu": image.width.raw(),
                        "x_emu": image.x.raw(),
                        "y_emu": image.y.raw(),
                    })
                })
                .collect();
            let runs: Vec<Value> = page.runs.iter().map(run_json).collect();
            json!({
                "footer_emu": geometry.footer.raw(),
                "header_emu": geometry.header.raw(),
                "height_emu": page.height.raw(),
                "images": images,
                "margin_bottom_emu": geometry.bottom.raw(),
                "margin_left_emu": geometry.left.raw(),
                "margin_right_emu": geometry.right.raw(),
                "margin_top_emu": geometry.top.raw(),
                "rects": rects,
                "runs": runs,
                "width_emu": page.width.raw(),
            })
        })
        .collect();
    let notes: Vec<Value> = snapshot.notes().iter().map(note_json).collect();
    let diagnostics: Vec<Value> = snapshot.diagnostics().iter().map(diagnostic_json).collect();
    json!({
        "diagnostics": diagnostics,
        "notes": notes,
        "omitted_blocks": snapshot.omitted_blocks(),
        "omitted_diagnostics": snapshot.omitted_diagnostics(),
        "pages": pages,
    })
}

fn hundredths(points: f64) -> i64 {
    (points * 100.0).round() as i64
}

fn pdf_digest(report: &PdfReport) -> Value {
    let pages: Vec<Value> = report
        .pages
        .iter()
        .map(|(width, height)| {
            json!([
                hundredths(f64::from(*width)),
                hundredths(f64::from(*height))
            ])
        })
        .collect();
    json!({
        "fonts": report.fonts,
        "fonts_with_to_unicode": report.fonts_with_to_unicode,
        "has_creation_date": report.has_creation_date,
        "has_mod_date": report.has_mod_date,
        "image_xobjects": report.image_xobjects,
        "link_uris": report.link_uris,
        "pages_hundredths_pt": pages,
        "producer": report.producer,
    })
}

fn assert_pdf_matches_scene(snapshot: &LayoutSnapshot, report: &PdfReport) {
    assert_eq!(report.pages.len(), snapshot.pages().len(), "pdf page count");
    assert!(!report.has_creation_date);
    assert!(!report.has_mod_date);
    assert_eq!(report.producer.as_deref(), Some("JoyWord"));
    for (index, page) in snapshot.pages().iter().enumerate() {
        let (width, height) = report.pages[index];
        let dx = (f64::from(width) - to_points(page.width)).abs();
        let dy = (f64::from(height) - to_points(page.height)).abs();
        assert!(
            dx <= 0.01 && dy <= 0.01,
            "page {index} box {width} {height} vs scene {} {}",
            to_points(page.width),
            to_points(page.height)
        );
    }
}

fn page_tiles(page: &RasterPage) -> Value {
    let width = page.width();
    let height = page.height();
    let rgba = page.rgba();
    let mut tiles = Vec::new();
    let mut y = 0_u32;
    while y < height {
        let mut x = 0_u32;
        while x < width {
            let x1 = (x + TILE).min(width);
            let y1 = (y + TILE).min(height);
            let mut ink = 0_u64;
            let mut total = 0_u64;
            for py in y..y1 {
                for px in x..x1 {
                    let index = ((py * width + px) * 4) as usize;
                    let sum = u32::from(rgba[index])
                        + u32::from(rgba[index + 1])
                        + u32::from(rgba[index + 2]);
                    total += 1;
                    if sum < 750 {
                        ink += 1;
                    }
                }
            }
            if ink > 0 {
                tiles.push(json!({
                    "basis_points": (ink * 10_000) / total,
                    "x": x,
                    "y": y,
                }));
            }
            x += TILE;
        }
        y += TILE;
    }
    json!({
        "height": height,
        "ink_tiles": tiles,
        "width": width,
    })
}

fn raster_one(page: &PageScene, font: &LoadedFont) -> Value {
    let projection = Projection::new(72.0, 1.0).expect("projection");
    let limits = RasterLimits::default();
    let cancel = RasterCancellation::default();
    let fonts = std::slice::from_ref(font);
    let first = rasterize(page, fonts, projection, limits, &cancel);
    let second = rasterize(page, fonts, projection, limits, &cancel);
    match (first, second) {
        (Ok(first), Ok(second)) => {
            assert_eq!(first.rgba(), second.rgba(), "raster is not repeatable");
            assert_eq!(first.width(), second.width());
            assert_eq!(first.height(), second.height());
            let mut tiles = page_tiles(&first);
            if let Value::Object(map) = &mut tiles {
                map.insert("status".to_string(), json!("ok"));
            }
            tiles
        }
        (Err(left), Err(right)) => {
            assert_eq!(left, right, "raster error is not repeatable");
            json!({
                "error": format!("{left:?}"),
                "status": "refused",
            })
        }
        (Ok(_), Err(error)) | (Err(error), Ok(_)) => {
            panic!("raster repeatability broke: {error:?}")
        }
    }
}

fn raster_digest(pages: &[PageScene], font: &LoadedFont) -> Value {
    json!({
        "dpi": 72,
        "pages": pages.iter().map(|page| raster_one(page, font)).collect::<Vec<_>>(),
        "tile_px": TILE,
        "tolerance_basis_points": TILE_TOLERANCE,
        "zoom": 1,
    })
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn bless_requested() -> bool {
    std::env::var("JOYWORD_BLESS_GOLDEN").ok().as_deref() == Some("1")
        && std::env::var_os("CI").is_none()
}

fn write_json(path: &std::path::Path, value: &Value) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("golden dir");
    }
    let mut text = serde_json::to_string_pretty(value).expect("json");
    text.push('\n');
    std::fs::write(path, text).expect("write golden");
}

fn read_json(path: &std::path::Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| {
        panic!(
            "missing {}; bless with JOYWORD_BLESS_GOLDEN=1",
            path.display()
        )
    });
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn push_diff(out: &mut Vec<String>, line: String) {
    if out.len() < 40 {
        out.push(line);
    }
}

fn diff_values(path: &str, expected: &Value, actual: &Value, out: &mut Vec<String>) {
    if out.len() >= 40 {
        return;
    }
    match (expected, actual) {
        (Value::Object(left), Value::Object(right)) => {
            for (key, value) in left {
                match right.get(key) {
                    Some(other) => diff_values(&format!("{path}.{key}"), value, other, out),
                    None => push_diff(out, format!("{path}.{key}: missing in actual")),
                }
            }
            for key in right.keys() {
                if !left.contains_key(key) {
                    push_diff(out, format!("{path}.{key}: unexpected in actual"));
                }
            }
        }
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                push_diff(
                    out,
                    format!("{path}: length {} != {}", left.len(), right.len()),
                );
            }
            for (index, (value, other)) in left.iter().zip(right.iter()).enumerate() {
                diff_values(&format!("{path}[{index}]"), value, other, out);
            }
        }
        (left, right) if left != right => {
            push_diff(out, format!("{path}: {left} != {right}"));
        }
        _ => {}
    }
}

fn assert_same_json(path: &std::path::Path, actual: &Value) {
    let expected = read_json(path);
    let mut mismatches = Vec::new();
    diff_values("$", &expected, actual, &mut mismatches);
    assert!(
        mismatches.is_empty(),
        "{} mismatch:\n{}",
        path.display(),
        mismatches.join("\n")
    );
}

fn tile_map(page: &Value) -> BTreeMap<(i64, i64), i64> {
    let mut map = BTreeMap::new();
    let Some(tiles) = page.get("ink_tiles").and_then(Value::as_array) else {
        return map;
    };
    for tile in tiles {
        let x = tile.get("x").and_then(Value::as_i64).unwrap_or(0);
        let y = tile.get("y").and_then(Value::as_i64).unwrap_or(0);
        let ink = tile
            .get("basis_points")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        map.insert((x, y), ink);
    }
    map
}

fn assert_raster_close(path: &std::path::Path, actual: &Value) {
    let expected = read_json(path);
    assert_eq!(expected["dpi"], actual["dpi"]);
    assert_eq!(expected["tile_px"], actual["tile_px"]);
    assert_eq!(expected["zoom"], actual["zoom"]);
    let left = expected["pages"].as_array().expect("pages");
    let right = actual["pages"].as_array().expect("pages");
    assert_eq!(left.len(), right.len(), "raster page count");
    let mut mismatches = Vec::new();
    for (index, (exp_page, act_page)) in left.iter().zip(right.iter()).enumerate() {
        if exp_page["status"] != act_page["status"] || exp_page["status"] == "refused" {
            if exp_page != act_page {
                push_diff(
                    &mut mismatches,
                    format!("page {index} raster {exp_page} != {act_page}"),
                );
            }
            continue;
        }
        if exp_page["width"] != act_page["width"] || exp_page["height"] != act_page["height"] {
            push_diff(
                &mut mismatches,
                format!(
                    "page {index} size {}x{} != {}x{}",
                    exp_page["width"], exp_page["height"], act_page["width"], act_page["height"]
                ),
            );
        }
        let exp_tiles = tile_map(exp_page);
        let act_tiles = tile_map(act_page);
        let mut keys: Vec<(i64, i64)> = exp_tiles.keys().copied().collect();
        keys.extend(act_tiles.keys().copied());
        keys.sort_unstable();
        keys.dedup();
        for key in keys {
            let exp = exp_tiles.get(&key).copied().unwrap_or(0);
            let act = act_tiles.get(&key).copied().unwrap_or(0);
            if (exp - act).abs() > TILE_TOLERANCE {
                push_diff(
                    &mut mismatches,
                    format!(
                        "page {index} tile ({}, {}) ink {exp} != {act} (tolerance {TILE_TOLERANCE})",
                        key.0, key.1
                    ),
                );
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} raster mismatch:\n{}",
        path.display(),
        mismatches.join("\n")
    );
}

struct Evidence {
    scene: Value,
    pdf: Value,
    raster: Value,
}

fn evidence_for(document: &Document, snapshot: &LayoutSnapshot, font: &LoadedFont) -> Evidence {
    let scene = scene_digest(snapshot);
    let export = PdfExport {
        links: links_from_document(document, snapshot),
        ..PdfExport::default()
    };
    let fonts = std::slice::from_ref(font);
    let first = render_export(snapshot.pages(), fonts, &export).expect("pdf");
    let second = render_export(snapshot.pages(), fonts, &export).expect("pdf");
    assert_eq!(first, second, "pdf bytes differ across two runs");
    let report = inspect_pdf(&first).expect("inspect pdf");
    assert_pdf_matches_scene(snapshot, &report);
    if report.fonts > 0 {
        assert_eq!(
            report.fonts, report.fonts_with_to_unicode,
            "embedded fonts should carry ToUnicode"
        );
    }
    Evidence {
        scene,
        pdf: pdf_digest(&report),
        raster: raster_digest(snapshot.pages(), font),
    }
}

fn publish(name: &str, evidence: &Evidence) {
    let dir = golden_dir();
    let scene = dir.join(format!("{name}.json"));
    let pdf = dir.join(format!("{name}.pdf.json"));
    let raster = dir.join(format!("{name}.raster.json"));
    if bless_requested() {
        write_json(&scene, &evidence.scene);
        write_json(&pdf, &evidence.pdf);
        write_json(&raster, &evidence.raster);
        return;
    }
    assert_same_json(&scene, &evidence.scene);
    assert_same_json(&pdf, &evidence.pdf);
    assert_raster_close(&raster, &evidence.raster);
}

fn prepare(bytes: &[u8], view: Option<RevisionView>) -> (Document, LayoutSnapshot, LoadedFont) {
    let document = open_document(bytes);
    let font = test_font();
    let snapshot = layout_of(&document, &font, view);
    (document, snapshot, font)
}

fn count_tables(blocks: &BlockSeq) -> usize {
    let mut count = 0;
    for block in blocks.iter() {
        if let Block::Table(table) = block {
            count += 1;
            for row in &table.rows {
                for cell in &row.cells {
                    count += count_tables(&cell.blocks);
                }
            }
        }
    }
    count
}

fn run_origins(page: &PageScene) -> Vec<i64> {
    page.runs.iter().map(|run| run.origin_x.raw()).collect()
}

fn overlaps(run: &GlyphRun, image_x: i64, image_y: i64, image_w: i64, image_h: i64) -> bool {
    let left = run.origin_x.raw();
    let right = left.saturating_add(run.advance());
    let bottom = run.baseline_y.raw();
    let top = bottom.saturating_sub(run.size.raw());
    left < image_x.saturating_add(image_w)
        && right > image_x
        && top < image_y.saturating_add(image_h)
        && bottom > image_y
}

fn anchor_drawing(wrap: &str) -> Vec<u8> {
    let ns = format!(r#" xmlns:wp="{WP}" xmlns:a="{A}" xmlns:pic="{PIC}""#);
    let body = format!(
        r#"<w:p><w:r><w:drawing><wp:anchor behindDoc="0" distT="0" distB="0" distL="0" distR="0" simplePos="0"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:posOffset>228600</wp:posOffset></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>254000</wp:posOffset></wp:positionV><wp:extent cx="457200" cy="228600"/>{wrap}<a:graphic><a:graphicData uri="{PIC}"><pic:pic><pic:blipFill><a:blip r:embed="rId1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r><w:r><w:t>A</w:t></w:r></w:p><w:p><w:r><w:t>V</w:t></w:r></w:p>{}"#,
        sect(6_000, 2_240, "")
    );
    let rels = format!(r#"<Relationship Id="rId1" Type="{IMAGE_REL}" Target="media/pixel.png"/>"#);
    let png = sample_png_1x1();
    pack(&body, &ns, &rels, &[], Some(&png))
}

#[test]
fn sha256_matches_fips_180_4_vectors() {
    assert_eq!(
        hex_bytes(&sha256(b"")),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        hex_bytes(&sha256(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn equal_columns_match_the_golden_scene() {
    let body = format!(
        "<w:p><w:r><w:t>A</w:t></w:r></w:p><w:p><w:r><w:t>V</w:t></w:r></w:p><w:p><w:r><w:t>f</w:t></w:r></w:p><w:p><w:r><w:t>i</w:t></w:r></w:p>{}",
        sect(4_000, 1_520, r#"<w:cols w:num="2" w:space="120"/>"#)
    );
    let (document, snapshot, font) = prepare(&pack(&body, "", "", &[], None), None);
    assert_eq!(snapshot.pages().len(), 1, "{}", scene_text(&snapshot));
    let origins = run_origins(&snapshot.pages()[0]);
    let distinct = origins
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    assert!(
        distinct >= 2,
        "equal columns should use two x origins {origins:?} {}",
        scene_text(&snapshot)
    );
    publish(
        "t202-equal-columns",
        &evidence_for(&document, &snapshot, &font),
    );
}

#[test]
fn unequal_columns_and_a_column_break_match_the_golden_scene() {
    let body = format!(
        r#"<w:p><w:r><w:t>A</w:t></w:r><w:r><w:br w:type="column"/></w:r><w:r><w:t>V</w:t></w:r></w:p>{}"#,
        sect(
            4_000,
            1_160,
            r#"<w:cols w:equalWidth="0" w:num="2"><w:col w:w="900" w:space="200"/><w:col w:w="1800"/></w:cols>"#
        )
    );
    let (document, snapshot, font) = prepare(&pack(&body, "", "", &[], None), None);
    assert_eq!(snapshot.pages().len(), 1, "{}", scene_text(&snapshot));
    let origins = run_origins(&snapshot.pages()[0]);
    let max_x = *origins.iter().max().unwrap_or(&0);
    let min_x = *origins.iter().min().unwrap_or(&0);
    assert!(
        origins.len() >= 2 && max_x > min_x,
        "column break should separate A and V: {origins:?} {}",
        scene_text(&snapshot)
    );
    let gap = max_x - min_x;
    assert!(
        gap > Emu::from_twips(900).unwrap().raw(),
        "second column starts after the narrow column: {gap}"
    );
    publish(
        "t202-unequal-columns",
        &evidence_for(&document, &snapshot, &font),
    );
}

#[test]
fn continuous_and_even_page_sections_match_the_golden_scene() {
    let continuous = sect(3_200, 1_520, r#"<w:type w:val="continuous"/>"#);
    let even = sect(3_200, 1_520, r#"<w:type w:val="evenPage"/>"#);
    let first = sect(3_200, 1_520, r#"<w:pgNumType w:start="2"/>"#);
    let body = format!(
        r#"<w:p><w:pPr>{first}</w:pPr><w:r><w:t>A</w:t></w:r></w:p><w:p><w:pPr>{continuous}</w:pPr><w:r><w:t>V</w:t></w:r></w:p><w:p><w:r><w:t>f</w:t></w:r></w:p>{even}"#
    );
    let (document, snapshot, font) = prepare(&pack(&body, "", "", &[], None), None);
    assert!(
        snapshot.pages().len() >= 3,
        "even page should insert a blank page: {}",
        scene_text(&snapshot)
    );
    let page0 = &snapshot.pages()[0];
    let text0: String = page0.runs.iter().map(|run| run.text.as_str()).collect();
    assert!(
        text0.contains('A') && text0.contains('V'),
        "continuous section should share page 1: {text0}"
    );
    assert!(
        snapshot.pages().iter().any(|page| page.runs.is_empty()),
        "evenPage from an odd continuation should leave a blank page: {}",
        scene_text(&snapshot)
    );
    let last = snapshot.pages().last().unwrap();
    let last_text: String = last.runs.iter().map(|run| run.text.as_str()).collect();
    assert!(last_text.contains('f'), "even page body: {last_text}");
    publish(
        "t202-continuous-even-page",
        &evidence_for(&document, &snapshot, &font),
    );
}

#[test]
fn first_even_and_default_headers_match_the_golden_scene() {
    let references = r#"<w:titlePg/><w:headerReference w:type="first" r:id="rId2"/><w:headerReference w:type="even" r:id="rId3"/><w:headerReference w:type="default" r:id="rId4"/>"#;
    let body = format!(
        r#"<w:p><w:r><w:t>i</w:t><w:br w:type="page"/><w:t>i</w:t><w:br w:type="page"/><w:t>i</w:t></w:r></w:p>{}"#,
        sect(3_600, 1_160, references)
    );
    let rels = format!(
        r#"<Relationship Id="rId1" Type="{SETTINGS_REL}" Target="settings.xml"/><Relationship Id="rId2" Type="{HEADER_REL}" Target="header1.xml"/><Relationship Id="rId3" Type="{HEADER_REL}" Target="header2.xml"/><Relationship Id="rId4" Type="{HEADER_REL}" Target="header3.xml"/>"#
    );
    let settings = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="{W}"><w:evenAndOddHeaders/></w:settings>"#
    );
    let header = |mark: &str, field: &str| {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><w:hdr xmlns:w="{W}"><w:p><w:r><w:t>{mark}</w:t></w:r><w:fldSimple w:instr=" {field} "><w:r><w:t>9</w:t></w:r></w:fldSimple></w:p></w:hdr>"#
        )
    };
    let bytes = pack(
        &body,
        "",
        &rels,
        &[
            ("word/settings.xml", settings),
            ("word/header1.xml", header("A", "PAGE")),
            ("word/header2.xml", header("V", "NUMPAGES")),
            ("word/header3.xml", header("f", "PAGE")),
        ],
        None,
    );
    let (document, snapshot, font) = prepare(&bytes, None);
    assert_eq!(snapshot.pages().len(), 3, "{}", scene_text(&snapshot));
    let page = |index: usize| -> String {
        snapshot.pages()[index]
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect()
    };
    assert!(
        page(0).contains('A') && !page(0).contains('V') && !page(0).contains('f'),
        "{}",
        page(0)
    );
    assert!(
        page(1).contains('V') && !page(1).contains('A'),
        "{}",
        page(1)
    );
    assert!(
        page(2).contains('f') && !page(2).contains('A'),
        "{}",
        page(2)
    );
    publish(
        "t202-headers-page-fields",
        &evidence_for(&document, &snapshot, &font),
    );
}

#[test]
fn wrap_none_anchor_matches_the_golden_scene() {
    let (document, snapshot, font) = prepare(&anchor_drawing("<wp:wrapNone/>"), None);
    let page = &snapshot.pages()[0];
    assert_eq!(page.images.len(), 1, "{}", scene_text(&snapshot));
    let image = &page.images[0];
    let text = page
        .runs
        .iter()
        .find(|run| run.text.contains('A'))
        .expect("text");
    assert_eq!(text.origin_x, snapshot.page_geometry()[0].left);
    assert!(
        overlaps(
            text,
            image.x.raw(),
            image.y.raw(),
            image.width.raw(),
            image.height.raw()
        ),
        "wrapNone should leave text across the anchor"
    );
    publish("t203-wrap-none", &evidence_for(&document, &snapshot, &font));
}

#[test]
fn wrap_square_text_avoids_the_rectangle() {
    let (document, snapshot, font) = prepare(
        &anchor_drawing(r#"<wp:wrapSquare wrapText="bothSides"/>"#),
        None,
    );
    let page = &snapshot.pages()[0];
    assert_eq!(page.images.len(), 1, "{}", scene_text(&snapshot));
    let image = &page.images[0];
    assert!(
        page.runs.iter().all(|run| {
            !overlaps(
                run,
                image.x.raw(),
                image.y.raw(),
                image.width.raw(),
                image.height.raw(),
            )
        }),
        "wrapSquare text still crosses the image: {}",
        scene_text(&snapshot)
    );
    let shifted = page.runs.iter().any(|run| {
        run.origin_x.raw() >= image.x.raw().saturating_add(image.width.raw())
            || run.baseline_y.raw().saturating_sub(run.size.raw())
                >= image.y.raw().saturating_add(image.height.raw())
    });
    assert!(shifted, "text should move beside or below the square");
    publish(
        "t203-wrap-square",
        &evidence_for(&document, &snapshot, &font),
    );
}

fn banded_table_package() -> Vec<u8> {
    let styles = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:styles xmlns:w="{W}"><w:style w:type="table" w:styleId="bands"><w:name w:val="Bands"/><w:tblStylePr w:type="band1Horz"><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="0000FF"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="firstCol"><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="FF0000"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="lastCol"><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="FFFF00"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="band1Vert"><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="00FF00"/></w:tcPr></w:tblStylePr></w:style></w:styles>"#
    );
    let cell = |text: &str| {
        format!(
            r#"<w:tc><w:tcPr><w:tcW w:w="1000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:tc>"#
        )
    };
    let nested = r#"<w:tbl><w:tblPr><w:tblW w:w="800" w:type="dxa"/></w:tblPr><w:tblGrid><w:gridCol w:w="800"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>i</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#;
    let body = format!(
        r#"<w:tbl><w:tblPr><w:tblStyle w:val="bands"/><w:tblW w:w="3000" w:type="dxa"/><w:tblLook w:firstRow="0" w:lastRow="0" w:firstColumn="1" w:lastColumn="1" w:noHBand="0" w:noVBand="0"/></w:tblPr><w:tblGrid><w:gridCol w:w="1000"/><w:gridCol w:w="1000"/><w:gridCol w:w="1000"/></w:tblGrid><w:tr>{}{}{}</w:tr><w:tr>{}<w:tc><w:tcPr><w:tcW w:w="1000" w:type="dxa"/><w:tcBorders><w:top w:val="single" w:sz="4" w:color="112233"/><w:left w:val="single" w:sz="4" w:color="112233"/><w:bottom w:val="single" w:sz="4" w:color="112233"/><w:right w:val="single" w:sz="4" w:color="112233"/></w:tcBorders></w:tcPr><w:p><w:r><w:t>A</w:t></w:r></w:p>{nested}</w:tc>{}</w:tr></w:tbl>{}"#,
        cell("A"),
        cell("V"),
        cell("f"),
        cell("V"),
        cell("f"),
        sect(4_400, 2_600, "")
    );
    let rels = format!(r#"<Relationship Id="rId1" Type="{STYLES_REL}" Target="styles.xml"/>"#);
    pack(&body, "", &rels, &[("word/styles.xml", styles)], None)
}

#[test]
fn banded_table_with_a_nested_table_matches_the_golden_scene() {
    let (document, snapshot, font) = prepare(&banded_table_package(), None);
    assert_eq!(count_tables(&document.body().blocks), 2);
    let style = document
        .styles()
        .get(&StyleId::new("bands"))
        .expect("table style");
    let conditions: Vec<TableCondition> = style
        .table_conditionals
        .iter()
        .map(|item| item.condition)
        .collect();
    for condition in [
        TableCondition::Band1Horizontal,
        TableCondition::FirstColumn,
        TableCondition::LastColumn,
        TableCondition::Band1Vertical,
    ] {
        assert!(conditions.contains(&condition), "{condition:?}");
    }
    let page = &snapshot.pages()[0];
    let fills: Vec<String> = page
        .rects
        .iter()
        .map(|rect| color_hex(rect.red, rect.green, rect.blue))
        .collect();
    for (fill, region) in [
        ("0000ff", "horizontal band"),
        ("ff0000", "first column"),
        ("ffff00", "last column"),
        ("00ff00", "vertical band"),
    ] {
        assert!(
            fills.iter().any(|item| item == fill),
            "{region} shading should paint: {fills:?}"
        );
    }
    publish(
        "t201-banded-nested-table",
        &evidence_for(&document, &snapshot, &font),
    );
}

#[test]
fn vertical_column_bands_are_imported_from_table_style_pr() {
    let document = open_document(&banded_table_package());
    let style = document
        .styles()
        .get(&StyleId::new("bands"))
        .expect("style");
    let conditions: Vec<TableCondition> = style
        .table_conditionals
        .iter()
        .map(|item| item.condition)
        .collect();
    assert!(conditions.contains(&TableCondition::FirstColumn));
    assert!(conditions.contains(&TableCondition::LastColumn));
    assert!(conditions.contains(&TableCondition::Band1Vertical));
}

fn revision_package() -> Vec<u8> {
    let body_para = concat!(
        r#"<w:p>"#,
        r#"<w:r><w:t>A</w:t></w:r>"#,
        r#"<w:ins w:id="1" w:author="Ada" w:date="2026-01-02T03:04:05Z"><w:r><w:t>V</w:t></w:r></w:ins>"#,
        r#"<w:del w:id="2" w:author="Ada" w:date="2026-01-02T03:04:05Z"><w:r><w:delText>f</w:delText></w:r></w:del>"#,
        r#"<w:moveFromRangeStart w:id="3" w:author="Ada" w:name="pair" w:date="2026-01-02T03:04:05Z"/>"#,
        r#"<w:moveFrom w:id="4" w:author="Ada" w:date="2026-01-02T03:04:05Z"><w:r><w:delText>i</w:delText></w:r></w:moveFrom>"#,
        r#"<w:moveFromRangeEnd w:id="5" w:author="Ada" w:name="pair" w:date="2026-01-02T03:04:05Z"/>"#,
        r#"<w:moveToRangeStart w:id="6" w:author="Ada" w:name="pair" w:date="2026-01-02T03:04:05Z"/>"#,
        r#"<w:moveTo w:id="7" w:author="Ada" w:date="2026-01-02T03:04:05Z"><w:r><w:t>A</w:t></w:r></w:moveTo>"#,
        r#"<w:moveToRangeEnd w:id="8" w:author="Ada" w:name="pair" w:date="2026-01-02T03:04:05Z"/>"#,
        r#"</w:p>"#,
    );
    let header = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:hdr xmlns:w="{W}"><w:p><w:r><w:t>A</w:t></w:r><w:ins w:id="11" w:author="Ada" w:date="2026-01-02T03:04:05Z"><w:r><w:t>V</w:t></w:r></w:ins><w:del w:id="12" w:author="Ada" w:date="2026-01-02T03:04:05Z"><w:r><w:delText>f</w:delText></w:r></w:del></w:p></w:hdr>"#
    );
    let body = format!(
        "{body_para}{}",
        sect(
            4_000,
            1_600,
            r#"<w:headerReference w:type="default" r:id="rId2"/>"#
        )
    );
    let rels = format!(r#"<Relationship Id="rId2" Type="{HEADER_REL}" Target="header1.xml"/>"#);
    pack(&body, "", &rels, &[("word/header1.xml", header)], None)
}

fn page_letters(snapshot: &LayoutSnapshot) -> String {
    snapshot
        .pages()
        .iter()
        .flat_map(|page| page.runs.iter().map(|run| run.text.as_str()))
        .collect()
}

#[test]
fn revision_markup_final_and_original_match_the_golden_scene() {
    let bytes = revision_package();
    let cases = [
        ("t300-revisions-markup", RevisionView::FinalWithMarkup, true),
        ("t300-revisions-final", RevisionView::Final, false),
        ("t300-revisions-original", RevisionView::Original, false),
    ];
    for (name, view, markup) in cases {
        let (document, snapshot, font) = prepare(&bytes, Some(view));
        let letters: String = page_letters(&snapshot)
            .chars()
            .filter(|ch| matches!(ch, 'A' | 'V' | 'f' | 'i'))
            .collect();
        match view {
            RevisionView::FinalWithMarkup => {
                assert!(
                    letters.contains('V') && letters.contains('f') && letters.contains('i'),
                    "{name} {letters}"
                );
            }
            RevisionView::Final => {
                assert!(
                    letters.contains('V') && !letters.contains('f') && !letters.contains('i'),
                    "{name} {letters}"
                );
            }
            RevisionView::Original => {
                assert!(
                    letters.contains('f') && letters.contains('i') && !letters.contains('V'),
                    "{name} {letters}"
                );
            }
        }
        let colored = snapshot.pages()[0]
            .runs
            .iter()
            .any(|run| run.color != [0, 0, 0]);
        assert_eq!(colored, markup, "{name} author colour");
        if markup {
            assert!(
                snapshot.pages()[0].rects.iter().any(|rect| {
                    rect_kind(rect, snapshot.page_geometry()[0].left.raw()) == "margin-bar"
                }),
                "markup should paint a margin bar"
            );
        }
        publish(name, &evidence_for(&document, &snapshot, &font));
    }
}

#[test]
fn paper_sizes_and_an_https_link_match_the_golden_scene() {
    let body = format!(
        r#"<w:p><w:pPr>{}</w:pPr><w:hyperlink r:id="rId2"><w:r><w:t>AVi</w:t></w:r></w:hyperlink></w:p><w:p><w:r><w:t>f</w:t></w:r></w:p>{}"#,
        sect(3_200, 1_400, ""),
        sect(4_800, 2_000, "")
    );
    let rels = format!(
        r#"<Relationship Id="rId2" Type="{HYPERLINK_REL}" Target="https://example.invalid/joyword" TargetMode="External"/>"#
    );
    let (document, snapshot, font) = prepare(&pack(&body, "", &rels, &[], None), None);
    assert_eq!(snapshot.pages().len(), 2, "{}", scene_text(&snapshot));
    assert_ne!(snapshot.pages()[0].width, snapshot.pages()[1].width);
    assert_ne!(snapshot.pages()[0].height, snapshot.pages()[1].height);
    assert!(
        snapshot.pages()[0]
            .rects
            .iter()
            .any(|rect| { (rect.red, rect.green, rect.blue) == (0x05, 0x63, 0xc1) }),
        "https link should paint an underline"
    );
    let evidence = evidence_for(&document, &snapshot, &font);
    let uris = evidence.pdf["link_uris"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(uris, vec![json!("https://example.invalid/joyword")]);
    publish("t204-paper-and-link", &evidence);
}

#[test]
fn numbering_label_and_a_tab_stop_match_the_golden_scene() {
    let numbering = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:numbering xmlns:w="{W}"><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="upperLetter"/><w:lvlText w:val="%1"/><w:suff w:val="tab"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#
    );
    let settings = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="{W}"><w:defaultTabStop w:val="720"/></w:settings>"#
    );
    let body = format!(
        r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr><w:tabs><w:tab w:val="left" w:pos="1440"/></w:tabs></w:pPr><w:r><w:t>f</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>i</w:t></w:r></w:p>{}"#,
        sect(4_400, 1_160, "")
    );
    let rels = format!(
        r#"<Relationship Id="rId1" Type="{SETTINGS_REL}" Target="settings.xml"/><Relationship Id="rId2" Type="{NUMBERING_REL}" Target="numbering.xml"/>"#
    );
    let bytes = pack(
        &body,
        "",
        &rels,
        &[
            ("word/settings.xml", settings),
            ("word/numbering.xml", numbering),
        ],
        None,
    );
    let (document, snapshot, font) = prepare(&bytes, None);
    let text = scene_text(&snapshot);
    assert!(text.contains('A'), "number label missing: {text}");
    assert!(text.contains('f') && text.contains('i'), "{text}");
    assert!(
        snapshot
            .lines()
            .iter()
            .any(|line| !line.tab_gaps.is_empty()),
        "tab stop should leave a gap: {text}"
    );
    let origins = run_origins(&snapshot.pages()[0]);
    assert!(
        origins.iter().max().unwrap() > origins.iter().min().unwrap()
            || snapshot.pages()[0]
                .runs
                .iter()
                .any(|run| run.glyphs.len() > 2),
        "label and tab should spread glyphs: {origins:?} {text}"
    );
    publish(
        "t200-numbering-tab",
        &evidence_for(&document, &snapshot, &font),
    );
}
