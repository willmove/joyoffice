//! Reproducible minimal format cases for the corpus registry (T004).
//!
//! Each case is an original, hand-written WordprocessingML package aimed at
//! one behavior JoyWord must get right. Packages are generated
//! deterministically (fixed item order and timestamps), so the committed
//! files under `tests/fixtures/docx` can be checked byte for byte against a
//! fresh generation. They are inputs, not reference results: until a fixed
//! Word and WPS environment has produced reference output, their registry
//! status stays `planned`.

use joyoffice_opc::{Method, write_archive};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const REL_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const REL_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
const REL_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
const REL_CUSTOM_XML: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml";
const REL_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
const REL_TEMPLATE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/attachedTemplate";
const CT_MAIN: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const CT_STYLES: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
const CT_SETTINGS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml";
const XML_DECL: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;

/// A registered minimal case.
pub struct FormatCase {
    pub id: &'static str,
    /// File name under `tests/fixtures/docx`.
    pub file: &'static str,
    pub requirements: &'static [&'static str],
    pub expected: &'static [&'static str],
    parts: fn() -> Vec<(&'static str, String)>,
}

impl FormatCase {
    /// ZIP items in package order.
    pub fn items(&self) -> Vec<(&'static str, String)> {
        (self.parts)()
    }

    /// Deterministic package bytes.
    pub fn package(&self) -> Vec<u8> {
        let items = self.items();
        let entries: Vec<(&str, &[u8], Method)> = items
            .iter()
            .map(|(name, data)| (*name, data.as_bytes(), Method::Deflated))
            .collect();
        write_archive(&entries).expect("small in-memory archive")
    }
}

fn content_types(overrides: &[(&str, &str)]) -> String {
    let mut xml = format!(
        r#"{XML_DECL}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>"#
    );
    for (part, content_type) in overrides {
        xml.push_str(&format!(
            r#"<Override PartName="{part}" ContentType="{content_type}"/>"#
        ));
    }
    xml.push_str("</Types>");
    xml
}

fn relationships(entries: &[(&str, &str, &str, bool)]) -> String {
    let mut xml = format!(
        r#"{XML_DECL}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#
    );
    for (id, kind, target, external) in entries {
        let mode = if *external {
            r#" TargetMode="External""#
        } else {
            ""
        };
        xml.push_str(&format!(
            r#"<Relationship Id="{id}" Type="{kind}" Target="{target}"{mode}/>"#
        ));
    }
    xml.push_str("</Relationships>");
    xml
}

fn package_rels() -> String {
    relationships(&[("rId1", REL_OFFICE_DOCUMENT, "word/document.xml", false)])
}

/// A4 portrait with JoyWord's zh-CN new-document margins.
const SECTION: &str = r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="851" w:footer="992" w:gutter="0"/></w:sectPr>"#;

fn document(extra_namespaces: &str, body: &str) -> String {
    format!(
        r#"{XML_DECL}<w:document xmlns:w="{W}" xmlns:r="{R}"{extra_namespaces}><w:body>{body}{SECTION}</w:body></w:document>"#
    )
}

fn paragraph(text: &str) -> String {
    format!("<w:p><w:r><w:t xml:space=\"preserve\">{text}</w:t></w:r></w:p>")
}

fn settings(children: &str) -> String {
    format!(r#"{XML_DECL}<w:settings xmlns:w="{W}" xmlns:r="{R}">{children}</w:settings>"#)
}

const COMPAT_15: &str = r#"<w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat>"#;

fn minimal() -> Vec<(&'static str, String)> {
    vec![
        (
            "[Content_Types].xml",
            content_types(&[("/word/document.xml", CT_MAIN)]),
        ),
        ("_rels/.rels", package_rels()),
        (
            "word/document.xml",
            document("", &paragraph("合作协议：甲方与乙方于 2026 年签署。")),
        ),
    ]
}

fn chinese_style_ids() -> Vec<(&'static str, String)> {
    let styles = format!(
        r#"{XML_DECL}<w:styles xmlns:w="{W}"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Times New Roman" w:eastAsia="宋体" w:hAnsi="Times New Roman" w:cs="Times New Roman"/><w:sz w:val="21"/><w:lang w:val="en-US" w:eastAsia="zh-CN"/></w:rPr></w:rPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="a"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="paragraph" w:styleId="1"><w:name w:val="heading 1"/><w:basedOn w:val="a"/><w:next w:val="a"/><w:qFormat/><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style><w:style w:type="character" w:default="1" w:styleId="a0"><w:name w:val="Default Paragraph Font"/></w:style></w:styles>"#
    );
    let body = format!(
        r#"<w:p><w:pPr><w:pStyle w:val="1"/></w:pPr><w:r><w:t>第一条 合作内容</w:t></w:r></w:p>{}"#,
        paragraph("甲方委托乙方完成设计服务。")
    );
    vec![
        (
            "[Content_Types].xml",
            content_types(&[
                ("/word/document.xml", CT_MAIN),
                ("/word/styles.xml", CT_STYLES),
                ("/word/settings.xml", CT_SETTINGS),
            ]),
        ),
        ("_rels/.rels", package_rels()),
        ("word/document.xml", document("", &body)),
        (
            "word/_rels/document.xml.rels",
            relationships(&[
                ("rId1", REL_STYLES, "styles.xml", false),
                ("rId2", REL_SETTINGS, "settings.xml", false),
            ]),
        ),
        ("word/styles.xml", styles),
        ("word/settings.xml", settings(COMPAT_15)),
    ]
}

fn unknown_content() -> Vec<(&'static str, String)> {
    let namespaces =
        format!(r#" xmlns:mc="{MC}" xmlns:jw="urn:joyoffice:test:unknown" mc:Ignorable="jw""#);
    let body = format!(
        r#"{}<jw:annotation jw:kind="原创测试">未知命名空间中的内容</jw:annotation>{}"#,
        paragraph("第一段：未知内容之前。"),
        paragraph("第二段：未知内容之后。")
    );
    vec![
        (
            "[Content_Types].xml",
            content_types(&[("/word/document.xml", CT_MAIN)]),
        ),
        ("_rels/.rels", package_rels()),
        ("word/document.xml", document(&namespaces, &body)),
        (
            "word/_rels/document.xml.rels",
            relationships(&[("rId1", REL_CUSTOM_XML, "../customXml/item1.xml", false)]),
        ),
        (
            "customXml/item1.xml",
            format!(
                r#"{XML_DECL}<contract xmlns="urn:joyoffice:test:custom"><party>甲方</party></contract>"#
            ),
        ),
    ]
}

fn external_references() -> Vec<(&'static str, String)> {
    let body = format!(
        r#"<w:p><w:hyperlink r:id="rId2"><w:r><w:t>项目说明</w:t></w:r></w:hyperlink></w:p>{}"#,
        paragraph("模板与链接都指向本包之外。")
    );
    vec![
        (
            "[Content_Types].xml",
            content_types(&[
                ("/word/document.xml", CT_MAIN),
                ("/word/settings.xml", CT_SETTINGS),
            ]),
        ),
        ("_rels/.rels", package_rels()),
        ("word/document.xml", document("", &body)),
        (
            "word/_rels/document.xml.rels",
            relationships(&[
                ("rId1", REL_SETTINGS, "settings.xml", false),
                (
                    "rId2",
                    REL_HYPERLINK,
                    "https://example.invalid/joyword",
                    true,
                ),
            ]),
        ),
        (
            "word/settings.xml",
            settings(&format!(r#"<w:attachedTemplate r:id="rId1"/>{COMPAT_15}"#)),
        ),
        (
            "word/_rels/settings.xml.rels",
            relationships(&[(
                "rId1",
                REL_TEMPLATE,
                "file://fileserver.invalid/templates/contract.dotm",
                true,
            )]),
        ),
    ]
}

fn protected_tracked() -> Vec<(&'static str, String)> {
    vec![
        (
            "[Content_Types].xml",
            content_types(&[
                ("/word/document.xml", CT_MAIN),
                ("/word/settings.xml", CT_SETTINGS),
            ]),
        ),
        ("_rels/.rels", package_rels()),
        (
            "word/document.xml",
            document("", &paragraph("本文档要求修订跟踪并限制编辑。")),
        ),
        (
            "word/_rels/document.xml.rels",
            relationships(&[("rId1", REL_SETTINGS, "settings.xml", false)]),
        ),
        (
            "word/settings.xml",
            settings(&format!(
                r#"<w:trackRevisions/><w:documentProtection w:edit="readOnly" w:enforcement="1"/>{COMPAT_15}"#
            )),
        ),
    ]
}

/// All registered minimal cases, in registry order.
pub fn cases() -> Vec<FormatCase> {
    vec![
        FormatCase {
            id: "DOCX-MIN-001",
            file: "DOCX-MIN-001.docx",
            requirements: &["FORMAT-001", "FORMAT-002"],
            expected: &[
                "正文段落文字逐码点读入",
                "缺少 settings 时兼容模式按 12 解释，无编辑保存不新增部件",
                "无编辑保存保留原包的部件与关系",
            ],
            parts: minimal,
        },
        FormatCase {
            id: "DOCX-MIN-002",
            file: "DOCX-MIN-002.docx",
            requirements: &["STYLE-001", "FORMAT-001"],
            expected: &[
                "样式 ID a／1 按 w:name 识别为 Normal 与 heading 1",
                "docDefaults 的四槽字体与字号读入且未设置与显式值可区分",
                "兼容模式 15 原样保留",
            ],
            parts: chinese_style_ids,
        },
        FormatCase {
            id: "DOCX-MIN-003",
            file: "DOCX-MIN-003.docx",
            requirements: &["FORMAT-002", "FORMAT-003"],
            expected: &[
                "mc:Ignorable 命名空间中的未知元素与 customXml 部件在保存后保留",
                "编辑相邻段落不丢弃未知内容；无法保证时阻止或提示另存",
            ],
            parts: unknown_content,
        },
        FormatCase {
            id: "DOCX-MIN-004",
            file: "DOCX-MIN-004.docx",
            requirements: &["QUALITY-005", "FORMAT-002"],
            expected: &[
                "打开时不访问网页、file:// 主机路径与附加模板",
                "外部目标在诊断中按网页与网络共享分类可见",
            ],
            parts: external_references,
        },
        FormatCase {
            id: "DOCX-MIN-005",
            file: "DOCX-MIN-005.docx",
            requirements: &["FORMAT-002", "REVIEW-001"],
            expected: &["限制编辑与强制修订在对应能力实现前以只读打开或明确提示，不得绕过"],
            parts: protected_tracked,
        },
    ]
}
