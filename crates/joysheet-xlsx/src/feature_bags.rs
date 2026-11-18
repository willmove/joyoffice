//! Feature property bags ([MS-XLSX] 2.1.22, 2.3.9): how Excel stores cell checkboxes.
//!
//! A checkbox is a cell format (`xf`) whose `extLst` carries an `xfComplement i="n"` element
//! ([MS-XLSX] 2.4.99, styles extension `{C7286773-470A-42A8-94C5-96B5CB345126}`). `n` indexes the
//! `MappedFeaturePropertyBags` array of the `XFComplements` bag; that entry is an `XFComplement`
//! bag whose `XFControls` key names an `XFControls` bag, whose `CellControl` key names a
//! `Checkbox` bag. Bag references are zero-based indexes into the part's list of `bag` elements.
//!
//! JoySheet keeps a checkbox as the `checkbox` number format in its model, so reading maps each
//! `xfComplement` that resolves to a `Checkbox` bag onto that format, and writing emits one chain
//! that every checkbox `xf` points at.

use crate::xml::El;

/// Namespace of the part and of the `xfComplement` element.
pub const NS: &str = "http://schemas.microsoft.com/office/spreadsheetml/2022/featurepropertybag";
/// Relationship type from the workbook part.
pub const REL: &str = "http://schemas.microsoft.com/office/2022/11/relationships/FeaturePropertyBag";
/// Content type of the part.
pub const CONTENT_TYPE: &str = "application/vnd.ms-excel.featurepropertybag+xml";
/// Part name we write.
pub const PART: &str = "xl/featurePropertyBag/featurePropertyBag.xml";
/// `ext` URI of the `xfComplement` extension on an `xf`.
pub const XF_EXT_URI: &str = "{C7286773-470A-42A8-94C5-96B5CB345126}";

// Excel 16.115 requires the mapper's public name when opening a generated workbook.
const XF_MAPPER: &str = "XFComplementsMapperExtRef";

/// The bags for one checkbox cell control, referenced as `xfComplement i="0"`: Checkbox (0),
/// XFControls → 0 (1), XFComplement → 1 (2), XFComplements mapping index 0 → 2 (3). Every
/// reference points to an earlier bag, as [MS-XLSX] 2.6.230 requires.
pub fn checkbox_part() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<FeaturePropertyBags xmlns=\"{NS}\" count=\"4\"><bag type=\"Checkbox\"/><bag type=\"XFControls\"><bagId k=\"CellControl\">0</bagId></bag><bag type=\"XFComplement\"><bagId k=\"XFControls\">1</bagId></bag><bag type=\"XFComplements\" extRef=\"{XF_MAPPER}\"><a k=\"MappedFeaturePropertyBags\"><bagId>2</bagId></a></bag></FeaturePropertyBags>"
    )
}

/// The `extLst` that marks an `xf` as a checkbox (complement index 0 of [`checkbox_part`]).
pub fn checkbox_xf_ext() -> String {
    format!("<extLst><ext uri=\"{XF_EXT_URI}\" xmlns:xfpb=\"{NS}\"><xfpb:xfComplement i=\"0\"/></ext></extLst>")
}

/// For each `xfComplement` index, whether it resolves to a Checkbox cell control. Unknown,
/// malformed or out-of-range references simply resolve to `false`.
pub fn checkbox_complements(root: &El) -> Vec<bool> {
    let bags: Vec<&El> = root.kids("bag").collect();
    // A bagId may only refer to an earlier sibling. Follow three typed
    // edges, preserving the map index even when one entry is malformed.
    let earlier = |owner: usize, index: Option<usize>, kind: &str| {
        let index = index?;
        let bag = bags.get(..owner)?.get(index)?;
        (bag.attr("type") == Some(kind)).then_some((index, *bag))
    };
    let key = |bag: &El, name: &str| bag.kids("bagId").find(|e| e.attr("k") == Some(name))?.text.trim().parse::<usize>().ok();
    let mappers = || bags.iter().enumerate().filter(|(_, b)| b.attr("type") == Some("XFComplements"));
    // Older JoySheet writers omitted extRef. The schema also allows other
    // names. A single mapping is unambiguous in either case.
    let mapper = mappers().find(|(_, b)| b.attr("extRef") == Some(XF_MAPPER)).or_else(|| {
        let mut candidates = mappers();
        let first = candidates.next()?;
        candidates.next().is_none().then_some(first)
    });
    let Some((owner, mapping)) = mapper else { return Vec::new() };
    let Some(entries) = mapping.kids("a").find(|a| a.attr("k") == Some("MappedFeaturePropertyBags")) else { return Vec::new() };
    entries
        .kids("bagId")
        .take(100_000)
        .map(|entry| {
            let complement = earlier(owner, entry.text.trim().parse().ok(), "XFComplement");
            let controls = complement.and_then(|(index, bag)| earlier(index, key(bag, "XFControls"), "XFControls"));
            controls.and_then(|(index, bag)| earlier(index, key(bag, "CellControl"), "Checkbox")).is_some()
        })
        .collect()
}

/// Whether an `xf` element carries an `xfComplement` that resolves to a checkbox.
pub fn xf_is_checkbox(xf: &El, complements: &[bool]) -> bool {
    let Some(ext) = xf.child("extLst") else { return false };
    ext.kids("ext")
        .filter(|e| e.attr("uri").is_some_and(|u| u.eq_ignore_ascii_case(XF_EXT_URI)))
        .filter_map(|e| e.child("xfComplement"))
        .filter_map(|c| c.attr_u32("i"))
        .any(|i| complements.get(i as usize).copied().unwrap_or(false))
}

#[cfg(test)]
#[path = "tests/feature_bags_tests.rs"]
mod tests;
