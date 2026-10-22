//! Settings children that cannot change pagination, and the theme-language
//! lookup that names an empty theme font slot.
//!
//! Sources: ECMA-376 Part 1 5th edition §17.15.1 (document settings),
//! §17.15.3 (`w:compatSetting`), §17.15.1.88 (`w:themeFontLang`) and
//! §20.1.4.1.16 (`a:font` supplemental fonts); [MS-DOCX] §2.6 for the `w14`
//! and `w15` settings extensions.

use std::sync::Arc;

use joyword_model::ThemeFont;

use crate::ns::{A, W, W14, W15, WORD_COMPAT_URI};
use crate::props::{on_off, val};
use crate::xml::Element;

const MATH: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";
const SCHEMA_LIBRARY: &str = "http://schemas.openxmlformats.org/schemaLibrary/2006/main";

/// `w:` settings that hold view or proofing state, editing aids, save and
/// print options, or identifiers. None of them moves a line or a page.
const VIEW_AND_METADATA: &[&str] = &[
    "zoom",
    "removePersonalInformation",
    "removeDateAndTime",
    "doNotDisplayPageBoundaries",
    "displayBackgroundShape",
    "printPostScriptOverText",
    "printFractionalCharacterWidth",
    "printFormsData",
    "embedTrueTypeFonts",
    "embedSystemFonts",
    "saveSubsetFonts",
    "saveFormsData",
    // Only page borders are affected, and layout draws none.
    "bordersDoNotSurroundHeader",
    "bordersDoNotSurroundFooter",
    "hideSpellingErrors",
    "hideGrammaticalErrors",
    "activeWritingStyle",
    "proofState",
    "formsDesign",
    "linkStyles",
    "stylePaneFormatFilter",
    "stylePaneSortMethod",
    "documentType",
    "mailMerge",
    "revisionView",
    "doNotTrackMoves",
    "doNotTrackFormatting",
    "autoFormatOverride",
    "styleLockTheme",
    "styleLockQFSet",
    // Hyphenation tuning applies only while `w:autoHyphenation` is on.
    "consecutiveHyphenLimit",
    "hyphenationZone",
    "doNotHyphenateCaps",
    "showEnvelope",
    "summaryLength",
    "clickAndTypeStyle",
    "defaultTableStyle",
    "drawingGridHorizontalSpacing",
    "drawingGridVerticalSpacing",
    "displayHorizontalDrawingGridEvery",
    "displayVerticalDrawingGridEvery",
    "doNotUseMarginsForDrawingGridOrigin",
    "drawingGridHorizontalOrigin",
    "drawingGridVerticalOrigin",
    "doNotShadeFormData",
    "savePreviewPicture",
    "doNotValidateAgainstSchema",
    "saveInvalidXml",
    "ignoreMixedContent",
    "alwaysShowPlaceholderText",
    "doNotDemarcateInvalidXml",
    "saveXmlDataOnly",
    "useXSLTWhenSaving",
    "saveThroughXslt",
    "showXMLTags",
    "alwaysMergeEmptyNamespace",
    "updateFields",
    "hdrShapeDefaults",
    "docVars",
    "rsids",
    "attachedSchema",
    // Applied to the theme fonts at import.
    "themeFontLang",
    "doNotIncludeSubdocsInStats",
    "doNotAutoCompressPictures",
    "forceUpgrade",
    "captions",
    "readModeInkLockDown",
    "smartTagType",
    "shapeDefaults",
    "doNotEmbedSmartTags",
    "decimalSymbol",
    "listSeparator",
];

/// `w:compatSetting` names Word writes into new documents. Each one only
/// changes hyphenation, floating tables, mirrored indents, OpenType features
/// or table-style overrides that this layout does not branch on.
const NEUTRAL_COMPAT_SETTINGS: &[&str] = &[
    "overrideTableStyleFontSizeAndJustification",
    "enableOpenTypeFeatures",
    "doNotFlipMirrorIndents",
    "differentiateMultirowTableHeaders",
    "useWord2013TrackBottomHyphenation",
    "allowHyphenationAtTrackBottom",
    "allowTextAfterFloatingTableBreak",
];

/// The identity `w:clrSchemeMapping` (§17.15.1.20) that Word writes by
/// default. Another mapping would change every theme colour.
const IDENTITY_COLOR_MAPPING: &[(&str, &str)] = &[
    ("bg1", "light1"),
    ("t1", "dark1"),
    ("bg2", "light2"),
    ("t2", "dark2"),
    ("accent1", "accent1"),
    ("accent2", "accent2"),
    ("accent3", "accent3"),
    ("accent4", "accent4"),
    ("accent5", "accent5"),
    ("accent6", "accent6"),
    ("hyperlink", "hyperlink"),
    ("followedHyperlink", "followedHyperlink"),
];

/// `true` for a settings child with no model field that still cannot move
/// a line or a page. Anything not recognised here counts as layout-relevant.
pub(crate) fn layout_neutral(child: &Element) -> bool {
    match child.namespace.as_deref() {
        Some(W) => w_layout_neutral(child),
        Some(W14) => matches!(&*child.local, "docId" | "defaultImageDpi"),
        Some(W15) => matches!(&*child.local, "docId" | "chartTrackingRefBased"),
        Some(MATH) => &*child.local == "mathPr",
        Some(SCHEMA_LIBRARY) => &*child.local == "schemaLibrary",
        _ => false,
    }
}

fn w_layout_neutral(child: &Element) -> bool {
    match &*child.local {
        "compat" => child.elements().all(compat_neutral),
        // Layout applies the R7 punctuation widths measured on Word 2024
        // whether or not punctuation compression is requested; kana
        // compression is not implemented.
        "characterSpacingControl" => {
            matches!(val(child), Some("doNotCompress" | "compressPunctuation"))
        }
        "autoHyphenation" => on_off(val(child)) == Some(false),
        "clrSchemeMapping" => IDENTITY_COLOR_MAPPING.iter().all(|(attribute, value)| {
            child
                .attribute(Some(W), attribute)
                .is_none_or(|found| found == *value)
        }),
        local => VIEW_AND_METADATA.contains(&local),
    }
}

fn compat_neutral(setting: &Element) -> bool {
    if setting.is(W, "useFELayout") {
        // Word's East Asian line layout, which is what layout implements.
        return true;
    }
    if !setting.is(W, "compatSetting") || setting.attribute(Some(W), "uri") != Some(WORD_COMPAT_URI)
    {
        return false;
    }
    match setting.attribute(Some(W), "name") {
        Some("compatibilityMode") => val(setting).is_some_and(|mode| mode.parse::<u32>().is_ok()),
        Some(name) => NEUTRAL_COMPAT_SETTINGS.contains(&name),
        None => false,
    }
}

/// Script tag of the theme `a:font` that serves a language (§17.15.1.88).
/// Only the scripts Office themes list for East Asian and complex text.
pub(crate) fn theme_script(language: &str) -> Option<&'static str> {
    let lower = language.to_ascii_lowercase();
    let mut parts = lower.split(['-', '_']);
    let primary = parts.next()?;
    let rest: Vec<&str> = parts.collect();
    Some(match primary {
        "zh" if rest
            .iter()
            .any(|part| matches!(*part, "hant" | "tw" | "hk" | "mo")) =>
        {
            "Hant"
        }
        "zh" => "Hans",
        "ja" => "Jpan",
        "ko" => "Hang",
        "ar" | "fa" | "ur" => "Arab",
        "he" => "Hebr",
        "th" => "Thai",
        _ => return None,
    })
}

/// `a:font` supplemental faces and the slots whose main typeface is empty,
/// kept until the settings part supplies the theme languages.
#[derive(Debug, Default)]
pub(crate) struct ThemeScripts {
    major: Vec<(Box<str>, Arc<str>)>,
    minor: Vec<(Box<str>, Arc<str>)>,
    empty: Vec<ThemeFont>,
}

/// `w:themeFontLang` languages for East Asian and complex script text.
#[derive(Debug, Default)]
pub(crate) struct ThemeLanguages {
    pub east_asia: Option<Box<str>>,
    pub bidi: Option<Box<str>>,
}

impl ThemeLanguages {
    pub(crate) fn read(element: &Element) -> Self {
        let get = |name: &str| element.attribute(Some(W), name).map(Box::from);
        Self {
            east_asia: get("eastAsia"),
            bidi: get("bidi"),
        }
    }
}

impl ThemeScripts {
    /// Read `a:majorFont` or `a:minorFont`.
    pub(crate) fn read(&mut self, collection: &Element, major: bool) {
        let faces: Vec<(Box<str>, Arc<str>)> = collection
            .elements()
            .filter(|font| font.is(A, "font"))
            .filter_map(|font| {
                let script = font.attribute(None, "script")?;
                let face = font.attribute(None, "typeface")?;
                (!face.is_empty()).then(|| (Box::from(script), Arc::from(face)))
            })
            .collect();
        let slots: [(&str, &[ThemeFont]); 3] = if major {
            [
                ("latin", &[ThemeFont::MajorAscii, ThemeFont::MajorHAnsi]),
                ("ea", &[ThemeFont::MajorEastAsia]),
                ("cs", &[ThemeFont::MajorBidi]),
            ]
        } else {
            [
                ("latin", &[ThemeFont::MinorAscii, ThemeFont::MinorHAnsi]),
                ("ea", &[ThemeFont::MinorEastAsia]),
                ("cs", &[ThemeFont::MinorBidi]),
            ]
        };
        for (local, fonts) in slots {
            let empty = collection
                .child(A, local)
                .is_some_and(|font| font.attribute(None, "typeface").is_none_or(str::is_empty));
            if empty {
                self.empty.extend_from_slice(fonts);
            }
        }
        if major {
            self.major = faces;
        } else {
            self.minor = faces;
        }
    }

    /// Name each empty East Asian or complex slot from the supplemental font
    /// for its theme language. A slot with no match is listed as unnamed.
    pub(crate) fn apply(self, languages: &ThemeLanguages, fonts: &mut joyword_model::ThemeFonts) {
        let east_asia = languages.east_asia.as_deref().and_then(theme_script);
        let bidi = languages.bidi.as_deref().and_then(theme_script);
        for slot in self.empty {
            let (script, faces, field) = match slot {
                ThemeFont::MajorEastAsia => (east_asia, &self.major, &mut fonts.major_east_asia),
                ThemeFont::MinorEastAsia => (east_asia, &self.minor, &mut fonts.minor_east_asia),
                ThemeFont::MajorBidi => (bidi, &self.major, &mut fonts.major_complex),
                ThemeFont::MinorBidi => (bidi, &self.minor, &mut fonts.minor_complex),
                _ => {
                    fonts.unnamed.push(slot);
                    continue;
                }
            };
            let found = script.and_then(|script| {
                faces
                    .iter()
                    .find(|(tag, _)| &**tag == script)
                    .map(|(_, face)| face.clone())
            });
            match found {
                Some(face) => *field = Some(face),
                None => fonts.unnamed.push(slot),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::theme_script;

    #[test]
    fn theme_languages_map_to_office_script_tags() {
        assert_eq!(theme_script("zh-CN"), Some("Hans"));
        assert_eq!(theme_script("zh-TW"), Some("Hant"));
        assert_eq!(theme_script("zh-Hant-HK"), Some("Hant"));
        assert_eq!(theme_script("ja-JP"), Some("Jpan"));
        assert_eq!(theme_script("ko-KR"), Some("Hang"));
        assert_eq!(theme_script("ar-SA"), Some("Arab"));
        assert_eq!(theme_script("he-IL"), Some("Hebr"));
        assert_eq!(theme_script("en-US"), None);
    }
}
