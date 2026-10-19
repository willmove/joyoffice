//! Writes one PDF per merged page feature, shaped with a caller-supplied font.
//!
//! Usage: feature_scenes <font-file> <face-index> <out-dir>
//!
//! `scripts/verify_feature_pdf.py` reads those PDFs with `pdftotext -enc UTF-8`.
//! The phrases are original and fictional. This is not a Word, WPS or GUI check.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use joyoffice_opc::{Budget, Method, write_archive};
use joyword_docx::import;
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LayoutSnapshot, LoadedFont,
    SingleFontProfile, layout_document, layout_document_viewed,
};
use joyword_model::{Document, RevisionView, sample_png_1x1};
use joyword_pdf::{PdfExport, links_from_document, render_export};
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

struct Scenario {
    file: &'static str,
    pages: usize,
    view: Option<RevisionView>,
    must_contain: &'static [&'static str],
    must_absent: &'static [&'static str],
    bytes: Vec<u8>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [font_path, index_text, out_dir] = args.as_slice() else {
        return Err("usage: feature_scenes <font-file> <face-index> <out-dir>".into());
    };
    let index: u32 = index_text
        .parse()
        .map_err(|_| "face index must be a number".to_string())?;
    let bytes =
        std::fs::read(font_path).map_err(|error| format!("cannot read {font_path}: {error}"))?;
    let font = LoadedFont::from_bytes(Arc::<[u8]>::from(bytes), index)
        .map_err(|error| format!("unusable font: {error:?}"))?;
    let out = PathBuf::from(out_dir);
    std::fs::create_dir_all(&out)
        .map_err(|error| format!("cannot create {}: {error}", out.display()))?;

    let mut reports = Vec::new();
    for scenario in scenarios()? {
        let document = open_document(&scenario.bytes)?;
        let snapshot = layout_of(&document, &font, scenario.view)?;
        if snapshot.pages().len() != scenario.pages {
            return Err(format!(
                "{} laid out {} pages, expected {}",
                scenario.file,
                snapshot.pages().len(),
                scenario.pages
            ));
        }
        let text = scene_text(&snapshot);
        for phrase in scenario.must_contain {
            if !text.contains(phrase) {
                return Err(format!(
                    "{} scene is missing {phrase}: {text}",
                    scenario.file
                ));
            }
        }
        for phrase in scenario.must_absent {
            if text.contains(phrase) {
                return Err(format!(
                    "{} scene still contains {phrase}: {text}",
                    scenario.file
                ));
            }
        }
        let pdf = paint(&document, &snapshot, &font)?;
        std::fs::write(out.join(scenario.file), &pdf)
            .map_err(|error| format!("cannot write {}: {error}", scenario.file))?;
        println!(
            "{}: {} bytes, {} pages",
            scenario.file,
            pdf.len(),
            scenario.pages
        );
        reports.push(json!({
            "must_absent": scenario.must_absent,
            "must_contain": scenario.must_contain,
            "pages": scenario.pages,
            "pdf": scenario.file,
        }));
    }
    let expected = json!({ "scenarios": reports });
    write_expected(&out.join("expected.json"), &expected)?;
    println!("wrote {}", out.join("expected.json").display());
    Ok(())
}

fn write_expected(path: &std::path::Path, value: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|error| format!("json: {error}"))?;
    std::fs::write(path, format!("{text}\n"))
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
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

fn archive(parts: &[(&str, &[u8])]) -> Result<Vec<u8>, String> {
    let stored: Vec<(&str, &[u8], Method)> = parts
        .iter()
        .map(|(name, data)| (*name, *data, Method::Stored))
        .collect();
    write_archive(&stored).map_err(|error| format!("package: {error}"))
}

fn content_type(name: &str) -> Result<&'static str, String> {
    match name {
        "word/header1.xml" | "word/header2.xml" | "word/header3.xml" => {
            Ok("application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml")
        }
        "word/settings.xml" => {
            Ok("application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml")
        }
        "word/numbering.xml" => {
            Ok("application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml")
        }
        "word/styles.xml" => {
            Ok("application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml")
        }
        other => Err(format!("unknown part {other}")),
    }
}

fn pack(
    body: &str,
    extra_ns: &str,
    rels: &str,
    extras: &[(&str, String)],
    png: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    let mut overrides = String::new();
    for (name, _) in extras {
        let kind = content_type(name)?;
        overrides.push_str(&format!(
            r#"<Override PartName="/{name}" ContentType="{kind}"/>"#
        ));
    }
    let png_default = if png.is_some() {
        r#"<Default Extension="png" ContentType="image/png"/>"#
    } else {
        ""
    };
    let types = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>{png_default}<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>{overrides}</Types>"#
    );
    let package_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"><Relationship Id="rId1" Type="{OFFICE}" Target="word/document.xml"/></Relationships>"#
    );
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"{extra_ns}><w:body>{body}</w:body></w:document>"#
    );
    let doc_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}">{rels}</Relationships>"#
    );
    let mut names = vec![
        "[Content_Types].xml".to_string(),
        "_rels/.rels".to_string(),
        "word/document.xml".to_string(),
        "word/_rels/document.xml.rels".to_string(),
    ];
    let mut owned = vec![
        types.into_bytes(),
        package_rels.into_bytes(),
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
        .map(|(name, data)| (name.as_str(), data.as_slice()))
        .collect();
    archive(&view)
}

fn sect(width: i64, height: i64, inner: &str) -> String {
    format!(
        r#"<w:sectPr><w:pgSz w:w="{width}" w:h="{height}"/><w:pgMar w:top="400" w:right="360" w:bottom="400" w:left="360" w:header="200" w:footer="200" w:gutter="0"/>{inner}</w:sectPr>"#
    )
}

fn open_document(bytes: &[u8]) -> Result<Document, String> {
    match import(bytes, &Budget::default()) {
        Ok(result) => {
            let issues = result.document.validate();
            if issues.has_errors() {
                return Err(format!("import produced {issues:?}"));
            }
            Ok(result.document)
        }
        Err(error) => Err(format!("import failed: {error}")),
    }
}

fn layout_of(
    document: &Document,
    font: &LoadedFont,
    view: Option<RevisionView>,
) -> Result<LayoutSnapshot, String> {
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
    result.map_err(|error| format!("layout failed: {error:?}"))
}

fn scene_text(snapshot: &LayoutSnapshot) -> String {
    let mut text = String::new();
    for page in snapshot.pages() {
        for run in &page.runs {
            text.push_str(&run.text);
        }
    }
    text
}

fn paint(
    document: &Document,
    snapshot: &LayoutSnapshot,
    font: &LoadedFont,
) -> Result<Vec<u8>, String> {
    let export = PdfExport {
        links: links_from_document(document, snapshot),
        ..PdfExport::default()
    };
    render_export(snapshot.pages(), std::slice::from_ref(font), &export)
        .map_err(|error| format!("pdf: {error:?}"))
}

fn scenarios() -> Result<Vec<Scenario>, String> {
    Ok(vec![
        equal_columns()?,
        unequal_columns()?,
        sections()?,
        headers()?,
        wrap("wrap-none.pdf", "<wp:wrapNone/>", &["叠字", "下移"], &[])?,
        wrap(
            "wrap-square.pdf",
            r#"<wp:wrapSquare wrapText="bothSides"/>"#,
            &["绕开", "下移"],
            &[],
        )?,
        table()?,
        revisions(
            "revisions-markup.pdf",
            RevisionView::FinalWithMarkup,
            &[
                "留下", "新增", "删去", "移走", "移来", "页眉", "新眉", "旧眉",
            ],
            &[],
        )?,
        revisions(
            "revisions-final.pdf",
            RevisionView::Final,
            &["留下", "新增", "移来", "页眉", "新眉"],
            &["删去", "移走", "旧眉"],
        )?,
        revisions(
            "revisions-original.pdf",
            RevisionView::Original,
            &["留下", "删去", "移走", "页眉", "旧眉"],
            &["新增", "移来", "新眉"],
        )?,
        paper_link()?,
        numbering()?,
    ])
}

fn equal_columns() -> Result<Scenario, String> {
    let body = format!(
        "<w:p><w:r><w:t>甲栏</w:t></w:r></w:p><w:p><w:r><w:t>乙栏</w:t></w:r></w:p><w:p><w:r><w:t>丙栏</w:t></w:r></w:p><w:p><w:r><w:t>丁栏</w:t></w:r></w:p>{}",
        sect(4_000, 1_520, r#"<w:cols w:num="2" w:space="120"/>"#)
    );
    Ok(Scenario {
        file: "equal-columns.pdf",
        pages: 1,
        view: None,
        must_contain: &["甲栏", "乙栏", "丙栏", "丁栏"],
        must_absent: &[],
        bytes: pack(&body, "", "", &[], None)?,
    })
}

fn unequal_columns() -> Result<Scenario, String> {
    let body = format!(
        r#"<w:p><w:r><w:t>窄栏</w:t></w:r><w:r><w:br w:type="column"/></w:r><w:r><w:t>宽栏</w:t></w:r></w:p>{}"#,
        sect(
            4_000,
            1_160,
            r#"<w:cols w:equalWidth="0" w:num="2"><w:col w:w="900" w:space="200"/><w:col w:w="1800"/></w:cols>"#
        )
    );
    Ok(Scenario {
        file: "unequal-columns.pdf",
        pages: 1,
        view: None,
        must_contain: &["窄栏", "宽栏"],
        must_absent: &[],
        bytes: pack(&body, "", "", &[], None)?,
    })
}

fn sections() -> Result<Scenario, String> {
    let continuous = sect(3_200, 1_520, r#"<w:type w:val="continuous"/>"#);
    let even = sect(3_200, 1_520, r#"<w:type w:val="evenPage"/>"#);
    let first = sect(3_200, 1_520, r#"<w:pgNumType w:start="2"/>"#);
    let body = format!(
        r#"<w:p><w:pPr>{first}</w:pPr><w:r><w:t>同页</w:t></w:r></w:p><w:p><w:pPr>{continuous}</w:pPr><w:r><w:t>续写</w:t></w:r></w:p><w:p><w:r><w:t>偶数</w:t></w:r></w:p>{even}"#
    );
    Ok(Scenario {
        file: "sections.pdf",
        pages: 3,
        view: None,
        must_contain: &["同页", "续写", "偶数"],
        must_absent: &[],
        bytes: pack(&body, "", "", &[], None)?,
    })
}

fn headers() -> Result<Scenario, String> {
    let references = r#"<w:titlePg/><w:headerReference w:type="first" r:id="rId2"/><w:headerReference w:type="even" r:id="rId3"/><w:headerReference w:type="default" r:id="rId4"/>"#;
    let body = format!(
        r#"<w:p><w:r><w:t>正文</w:t><w:br w:type="page"/><w:t>正文</w:t><w:br w:type="page"/><w:t>正文</w:t></w:r></w:p>{}"#,
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
    Ok(Scenario {
        file: "headers.pdf",
        pages: 3,
        view: None,
        must_contain: &["首页", "偶页", "常页", "正文"],
        must_absent: &[],
        bytes: pack(
            &body,
            "",
            &rels,
            &[
                ("word/settings.xml", settings),
                ("word/header1.xml", header("首页", "PAGE")),
                ("word/header2.xml", header("偶页", "NUMPAGES")),
                ("word/header3.xml", header("常页", "PAGE")),
            ],
            None,
        )?,
    })
}

fn wrap(
    file: &'static str,
    wrap_xml: &str,
    must_contain: &'static [&'static str],
    must_absent: &'static [&'static str],
) -> Result<Scenario, String> {
    let ns = format!(r#" xmlns:wp="{WP}" xmlns:a="{A}" xmlns:pic="{PIC}""#);
    let body = format!(
        r#"<w:p><w:r><w:drawing><wp:anchor behindDoc="0" distT="0" distB="0" distL="0" distR="0" simplePos="0"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:posOffset>228600</wp:posOffset></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>254000</wp:posOffset></wp:positionV><wp:extent cx="457200" cy="228600"/>{wrap_xml}<a:graphic><a:graphicData uri="{PIC}"><pic:pic><pic:blipFill><a:blip r:embed="rId1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r><w:r><w:t>{}</w:t></w:r></w:p><w:p><w:r><w:t>下移</w:t></w:r></w:p>{}"#,
        must_contain[0],
        sect(6_000, 2_240, "")
    );
    let rels = format!(r#"<Relationship Id="rId1" Type="{IMAGE_REL}" Target="media/pixel.png"/>"#);
    Ok(Scenario {
        file,
        pages: 1,
        view: None,
        must_contain,
        must_absent,
        bytes: pack(&body, &ns, &rels, &[], Some(sample_png_1x1().as_slice()))?,
    })
}

fn table() -> Result<Scenario, String> {
    let styles = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:styles xmlns:w="{W}"><w:style w:type="table" w:styleId="bands"><w:name w:val="Bands"/><w:tblStylePr w:type="band1Horz"><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="0000FF"/></w:tcPr></w:tblStylePr></w:style></w:styles>"#
    );
    let nested = r#"<w:tbl><w:tblPr><w:tblW w:w="800" w:type="dxa"/></w:tblPr><w:tblGrid><w:gridCol w:w="800"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>嵌套</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#;
    let body = format!(
        r#"<w:tbl><w:tblPr><w:tblStyle w:val="bands"/><w:tblW w:w="2000" w:type="dxa"/><w:tblLook w:firstRow="0" w:lastRow="0" w:firstColumn="0" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/></w:tblPr><w:tblGrid><w:gridCol w:w="1000"/><w:gridCol w:w="1000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>蓝格</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>蓝格</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>白格</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>白格</w:t></w:r></w:p>{nested}</w:tc></w:tr></w:tbl>{}"#,
        sect(4_400, 2_600, "")
    );
    let rels = format!(r#"<Relationship Id="rId1" Type="{STYLES_REL}" Target="styles.xml"/>"#);
    Ok(Scenario {
        file: "table.pdf",
        pages: 1,
        view: None,
        must_contain: &["蓝格", "嵌套"],
        must_absent: &[],
        bytes: pack(&body, "", &rels, &[("word/styles.xml", styles)], None)?,
    })
}

fn revisions(
    file: &'static str,
    view: RevisionView,
    must_contain: &'static [&'static str],
    must_absent: &'static [&'static str],
) -> Result<Scenario, String> {
    let body_para = concat!(
        r#"<w:p>"#,
        r#"<w:r><w:t>留下</w:t></w:r>"#,
        r#"<w:ins w:id="1" w:author="Ada" w:date="2026-01-02T03:04:05Z"><w:r><w:t>新增</w:t></w:r></w:ins>"#,
        r#"<w:del w:id="2" w:author="Ada" w:date="2026-01-02T03:04:05Z"><w:r><w:delText>删去</w:delText></w:r></w:del>"#,
        r#"<w:moveFromRangeStart w:id="3" w:author="Ada" w:name="pair" w:date="2026-01-02T03:04:05Z"/>"#,
        r#"<w:moveFrom w:id="4" w:author="Ada" w:date="2026-01-02T03:04:05Z"><w:r><w:delText>移走</w:delText></w:r></w:moveFrom>"#,
        r#"<w:moveFromRangeEnd w:id="5" w:author="Ada" w:name="pair" w:date="2026-01-02T03:04:05Z"/>"#,
        r#"<w:moveToRangeStart w:id="6" w:author="Ada" w:name="pair" w:date="2026-01-02T03:04:05Z"/>"#,
        r#"<w:moveTo w:id="7" w:author="Ada" w:date="2026-01-02T03:04:05Z"><w:r><w:t>移来</w:t></w:r></w:moveTo>"#,
        r#"<w:moveToRangeEnd w:id="8" w:author="Ada" w:name="pair" w:date="2026-01-02T03:04:05Z"/>"#,
        r#"</w:p>"#,
    );
    let header = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:hdr xmlns:w="{W}"><w:p><w:r><w:t>页眉</w:t></w:r><w:ins w:id="11" w:author="Ada" w:date="2026-01-02T03:04:05Z"><w:r><w:t>新眉</w:t></w:r></w:ins><w:del w:id="12" w:author="Ada" w:date="2026-01-02T03:04:05Z"><w:r><w:delText>旧眉</w:delText></w:r></w:del></w:p></w:hdr>"#
    );
    let body = format!(
        "{body_para}{}",
        sect(
            5_200,
            1_800,
            r#"<w:headerReference w:type="default" r:id="rId2"/>"#
        )
    );
    let rels = format!(r#"<Relationship Id="rId2" Type="{HEADER_REL}" Target="header1.xml"/>"#);
    Ok(Scenario {
        file,
        pages: 1,
        view: Some(view),
        must_contain,
        must_absent,
        bytes: pack(&body, "", &rels, &[("word/header1.xml", header)], None)?,
    })
}

fn paper_link() -> Result<Scenario, String> {
    let body = format!(
        r#"<w:p><w:pPr>{}</w:pPr><w:hyperlink r:id="rId2"><w:r><w:t>链接</w:t></w:r></w:hyperlink></w:p><w:p><w:r><w:t>另页</w:t></w:r></w:p>{}"#,
        sect(3_200, 1_400, ""),
        sect(4_800, 2_000, "")
    );
    let rels = format!(
        r#"<Relationship Id="rId2" Type="{HYPERLINK_REL}" Target="https://example.invalid/joyword" TargetMode="External"/>"#
    );
    Ok(Scenario {
        file: "paper-link.pdf",
        pages: 2,
        view: None,
        must_contain: &["链接", "另页"],
        must_absent: &[],
        bytes: pack(&body, "", &rels, &[], None)?,
    })
}

fn numbering() -> Result<Scenario, String> {
    let numbering = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:numbering xmlns:w="{W}"><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="upperLetter"/><w:lvlText w:val="%1"/><w:suff w:val="tab"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#
    );
    let settings = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="{W}"><w:defaultTabStop w:val="720"/></w:settings>"#
    );
    let body = format!(
        r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr><w:tabs><w:tab w:val="left" w:pos="1440"/></w:tabs></w:pPr><w:r><w:t>条</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>目</w:t></w:r></w:p>{}"#,
        sect(4_400, 1_160, "")
    );
    let rels = format!(
        r#"<Relationship Id="rId1" Type="{SETTINGS_REL}" Target="settings.xml"/><Relationship Id="rId2" Type="{NUMBERING_REL}" Target="numbering.xml"/>"#
    );
    Ok(Scenario {
        file: "numbering.pdf",
        pages: 1,
        view: None,
        must_contain: &["A", "条", "目"],
        must_absent: &[],
        bytes: pack(
            &body,
            "",
            &rels,
            &[
                ("word/settings.xml", settings),
                ("word/numbering.xml", numbering),
            ],
            None,
        )?,
    })
}
