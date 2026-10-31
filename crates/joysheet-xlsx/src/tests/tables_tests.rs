use super::*;

#[test]
fn widths() {
    assert_eq!(col_width_to_px(9.140625), 64.0);
    assert_eq!(px_to_col_width(64.0), 9.140625);
    for px in 0..400 {
        assert_eq!(col_width_to_px(px_to_col_width(px as f32)), px as f32, "px {px}");
    }
    assert_eq!(pt_to_px(15.0), 20.0);
    assert_eq!(px_to_pt(20.0), 15.0);
}

#[test]
fn functions() {
    assert_eq!(file_function_name("xlookup").as_deref(), Some("_xlfn.XLOOKUP"));
    assert_eq!(file_function_name("forecast.ets.confint").as_deref(), Some("_xlfn.FORECAST.ETS.CONFINT"));
    assert_eq!(file_function_name("FILTER").as_deref(), Some("_xlfn._xlws.FILTER"));
    assert_eq!(file_function_name("PERCENTOF").as_deref(), Some("_xlfn.PERCENTOF"));
    assert_eq!(file_function_name("TRIMRANGE").as_deref(), Some("_xlfn.TRIMRANGE"));
    assert_eq!(file_function_name("SUM"), None);
    assert_eq!(builtin_format(14), Some("m/d/yyyy"));
    assert_eq!(builtin_id("0.00%"), Some(10));
    assert_eq!(builtin_id(builtin_format(5).unwrap()), None);
    assert_eq!(paper_name(9), Some("A4"));
    assert_eq!(paper_id("a4"), Some(9));
    // 12/13 are the JIS B4/B5 Japanese Excel uses; 42 is ISO B4.
    assert_eq!(paper_name(12), Some("B4"));
    assert_eq!(paper_id("b5"), Some(13));
    assert_eq!(paper_id("B5 (JIS)"), Some(13));
    assert_eq!(paper_id("B4 (ISO)"), Some(42));
    // The Japanese papers page setup names.
    assert_eq!(paper_name(43), Some("Japanese Postcard"));
    assert_eq!(paper_name(88), Some("B6"));
    for (name, id) in [
        ("Japanese Postcard", 43),
        ("japanese double postcard", 69),
        ("A6", 70),
        ("Japanese Envelope Kaku #2", 71),
        ("kaku #3", 72),
        ("Japanese Envelope #3", 73),
        ("chou #4", 74),
        ("B6 (JIS)", 88),
    ] {
        assert_eq!(paper_id(name), Some(id), "{name}");
    }
    assert_eq!(paper_id("Napkin"), None);
}
