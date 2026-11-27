//! Compatibility cases from the original table-style adapter.
use super::*;

#[test]
fn style_parts_follow_schema_order() -> Result<(), crate::PptxError> {
    const ORDER: [&str; 14] = [
        "tblBg", "wholeTbl", "band1H", "band2H", "band1V", "band2V", "lastCol", "firstCol",
        "lastRow", "seCell", "swCell", "firstRow", "neCell", "nwCell",
    ];
    let used: Vec<String> = joydeck_model::table::table_styles()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let d = crate::xml::parse(&table_styles_xml(&used, &[]))?;
    let mut n = 0;
    for st in d.root.children_named("tblStyle") {
        n += 1;
        let pos: Vec<usize> = st
            .elements()
            .map(|e| {
                ORDER
                    .iter()
                    .position(|o| *o == e.local())
                    .unwrap_or(ORDER.len())
            })
            .collect();
        assert!(
            pos.iter().all(|i| *i < ORDER.len()) && pos.windows(2).all(|w| w[0] < w[1]),
            "{:?}: {pos:?}",
            st.attr("styleName")
        );
    }
    assert!(n > 30);
    Ok(())
}

#[test]
fn guids_round_trip() {
    for (id, _) in joydeck_model::table::table_styles() {
        let (g, _) = guid_for_style(&id);
        assert_eq!(style_from_guid(&g), id, "{id} via {g}");
        assert_eq!(g.len(), 38);
    }
    assert_eq!(
        style_from_guid("{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}"),
        "medium2-accent1"
    );
    assert_eq!(
        style_from_guid("{ABCDEF00-0000-0000-0000-000000000000}"),
        "{ABCDEF00-0000-0000-0000-000000000000}"
    );
}
