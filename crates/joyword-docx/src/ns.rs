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
/// WordprocessingML drawing (`wp:inline`, `wp:extent`).
pub const WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
/// DrawingML picture (`pic:pic`).
/// PictureML. Blips are read from `a:blip`; this name is the graphic-data URI.
#[allow(dead_code)]
pub(crate) const PIC: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
/// VML, used by legacy `w:pict`.
pub const VML: &str = "urn:schemas-microsoft-com:vml";
pub const REL_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
/// ECMA-376 Part 1 §15.2.3 / §17.16.22. External targets are not fetched.
pub const REL_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
pub const REL_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
pub const REL_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
pub const REL_HEADER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
pub const REL_FOOTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
pub const REL_VBA_PROJECT: &str =
    "http://schemas.microsoft.com/office/2006/relationships/vbaProject";
/// ECMA-376 Part 1 §17.13.4 comments part.
pub(crate) const REL_COMMENTS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
/// [MS-DOCX] commentsExtended (`w15:commentEx`).
pub(crate) const REL_COMMENTS_EXTENDED: &str =
    "http://schemas.microsoft.com/office/2011/relationships/commentsExtended";
/// [MS-DOCX] commentsIds (`w16cid:commentId`).
pub(crate) const REL_COMMENTS_IDS: &str =
    "http://schemas.microsoft.com/office/2016/09/relationships/commentsIds";
/// [MS-DOCX] commentsExtensible (`w16cex:commentExtensible`).
pub(crate) const REL_COMMENTS_EXTENSIBLE: &str =
    "http://schemas.microsoft.com/office/2018/08/relationships/commentsExtensible";
/// Word 2010 wordml (`w14:paraId`).
pub(crate) const W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
/// Word 2012 wordml (`w15`).
pub(crate) const W15: &str = "http://schemas.microsoft.com/office/word/2012/wordml";
/// Word 2016 comment id (`w16cid`).
pub(crate) const W16CID: &str = "http://schemas.microsoft.com/office/word/2016/wordml/cid";
/// Word 2018 comment extensibility (`w16cex`).
pub(crate) const W16CEX: &str = "http://schemas.microsoft.com/office/word/2018/wordml/cex";

/// Relationship types whose parts hold text stories. Header and footer parts
/// are imported when a section references them; an unreferenced one is still
/// warned. Notes stay unimported. Comments are imported separately.
pub(crate) const STORY_RELATIONSHIPS: [&str; 4] = [
    REL_HEADER,
    REL_FOOTER,
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes",
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes",
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

/// Tracked-change elements (ECMA-376 Part 1 §17.13.5).
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

/// Text and move revisions this slice maps (`w:ins`, `w:del`, `w:moveFrom`,
/// `w:moveTo`, their range markers, and deleted-text leaves). Format
/// revisions are typed separately (see [`is_format_change`]); table
/// structure revisions stay outside the model.
pub(crate) fn is_text_revision(local: &str) -> bool {
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
    )
}

/// Format revisions this slice types (`w:rPrChange` §17.13.5.30,
/// `w:pPrChange` §17.13.5.29) when they occur in a paragraph that is not
/// inside a table. Everywhere else they keep the document read-only.
pub(crate) fn is_format_change(local: &str) -> bool {
    matches!(local, "rPrChange" | "pPrChange")
}

/// Table property revisions (`w:tblPrChange`, `w:trPrChange`,
/// `w:tcPrChange`): typed when the whole containing table is inside the
/// represented subset. An occurrence outside that subset refuses the table,
/// which stays preserved and unedited, so the markup cannot be dropped; it
/// does not make the document read-only.
pub(crate) fn is_table_props_change(local: &str) -> bool {
    matches!(local, "tblPrChange" | "trPrChange" | "tcPrChange")
}

/// Revision markup that still makes the whole document read-only. Text
/// revisions are always typed. Format revisions (`w:rPrChange`,
/// `w:pPrChange`) are typed outside tables (`in_table` false) and block
/// inside tables. Table property revisions (`w:tblPrChange`,
/// `w:trPrChange`, `w:tcPrChange`) never block: either the table typed them
/// or the table stays preserved and unedited. All other non-text revisions
/// (structural table changes, section, numbering, custom XML) always block.
pub(crate) fn blocks_for_review(local: &str, in_table: bool) -> bool {
    if !is_revision(local) || is_text_revision(local) {
        return false;
    }
    if is_format_change(local) {
        return in_table;
    }
    !is_table_props_change(local)
}
