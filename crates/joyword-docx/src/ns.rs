//! Namespaces, relationship types and content types (ECMA-376 Part 1
//! Transitional and Part 3; Strict URIs only to recognise and refuse them).

/// WordprocessingML main namespace, Transitional.
pub const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
/// WordprocessingML main namespace, Strict (refused in this slice).
pub const W_STRICT: &str = "http://purl.oclc.org/ooxml/wordprocessingml/main";
pub const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
/// Markup Compatibility and Extensibility (ECMA-376 Part 3).
pub const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";

pub const REL_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
pub const REL_OFFICE_DOCUMENT_STRICT: &str =
    "http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument";
pub const REL_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
pub const REL_THEME: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
/// DrawingML main namespace (theme part).
pub const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const REL_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
pub const REL_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
pub const REL_VBA_PROJECT: &str =
    "http://schemas.microsoft.com/office/2006/relationships/vbaProject";

/// Relationship types whose parts hold text stories this slice does not
/// import yet; reported as warnings because their content is not visible.
pub(crate) const STORY_RELATIONSHIPS: [&str; 5] = [
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header",
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer",
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes",
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes",
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments",
];

/// Content types of a WordprocessingML main document part: document,
/// template, and their macro-enabled variants.
pub(crate) const MAIN_CONTENT_TYPES: [&str; 4] = [
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml",
    "application/vnd.ms-word.document.macroEnabled.main+xml",
    "application/vnd.ms-word.template.macroEnabledTemplate.main+xml",
];

/// `w:compatSetting` URI for Word's settings, including compatibilityMode.
pub(crate) const WORD_COMPAT_URI: &str = "http://schemas.microsoft.com/office/word";

/// Namespaces this importer understands for `mc:MustUnderstand`.
pub(crate) const UNDERSTOOD: [&str; 3] = [W, R, MC];

/// Office namespaces that are known to appear in WordprocessingML bodies but
/// are not mapped in this slice (math, DrawingML, VML, Word 2010+
/// extensions). Their elements are preserved and lock the containing block;
/// an element in any other non-ignorable namespace makes the document
/// read-only.
pub(crate) const KNOWN_OFFICE: [&str; 21] = [
    "http://schemas.openxmlformats.org/officeDocument/2006/math",
    "http://schemas.openxmlformats.org/drawingml/2006/main",
    "http://schemas.openxmlformats.org/drawingml/2006/picture",
    "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing",
    "urn:schemas-microsoft-com:vml",
    "urn:schemas-microsoft-com:office:office",
    "urn:schemas-microsoft-com:office:word",
    "http://schemas.microsoft.com/office/word/2006/wordml",
    "http://schemas.microsoft.com/office/word/2010/wordml",
    "http://schemas.microsoft.com/office/word/2012/wordml",
    "http://schemas.microsoft.com/office/word/2015/wordml/symex",
    "http://schemas.microsoft.com/office/word/2016/wordml/cid",
    "http://schemas.microsoft.com/office/word/2018/wordml",
    "http://schemas.microsoft.com/office/word/2018/wordml/cex",
    "http://schemas.microsoft.com/office/word/2010/wordprocessingShape",
    "http://schemas.microsoft.com/office/word/2010/wordprocessingGroup",
    "http://schemas.microsoft.com/office/word/2010/wordprocessingInk",
    "http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas",
    "http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing",
    "http://schemas.microsoft.com/office/drawing/2014/chartex",
    "http://schemas.openxmlformats.org/drawingml/2006/chart",
];

/// Tracked-change elements (ECMA-376 Part 1 §17.13.5). Any of them makes
/// the document read-only until review lands.
pub(crate) fn is_revision(local: &str) -> bool {
    matches!(
        local,
        "ins"
            | "del"
            | "moveFrom"
            | "moveTo"
            | "moveFromRangeStart"
            | "moveFromRangeEnd"
            | "moveToRangeStart"
            | "moveToRangeEnd"
            | "delText"
            | "delInstrText"
            | "rPrChange"
            | "pPrChange"
            | "sectPrChange"
            | "tblPrChange"
            | "tblPrExChange"
            | "tblGridChange"
            | "trPrChange"
            | "tcPrChange"
            | "numberingChange"
            | "cellIns"
            | "cellDel"
            | "cellMerge"
    ) || local.starts_with("customXmlIns")
        || local.starts_with("customXmlDel")
        || local.starts_with("customXmlMove")
}
