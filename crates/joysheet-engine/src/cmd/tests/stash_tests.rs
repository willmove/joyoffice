use super::*;

fn name(n: &str, formula: &str) -> DefinedName {
    DefinedName { name: n.into(), scope: None, formula: formula.into(), comment: String::new(), hidden: true }
}

fn excel_limits_hold(wb: &Workbook) {
    for n in &wb.names {
        assert!(n.formula.encode_utf16().count() <= FORMULA_UNITS, "{} is {} long", n.name, n.formula.len());
        // Every literal (quotes undoubled) is at most 255 characters.
        for lit in n.formula.split("\"&\"") {
            let inner = lit.trim_start_matches('"').trim_end_matches('"').replace("\"\"", "\"");
            assert!(inner.encode_utf16().count() <= LITERAL_UNITS, "{}", inner.len());
        }
    }
}

#[test]
fn long_values_split_over_names_and_come_back() {
    let mut wb = Workbook::new();
    // Quotes double when written; characters outside the BMP count twice.
    let text: String = (0..30_000).map(|i| ['a', '"', 'é', '\u{1D11E}', '&', '\n'][i % 6]).collect();
    set(&mut wb, "big", &text).unwrap();
    let names: Vec<&str> = wb.names.iter().map(|n| n.name.as_str()).collect();
    assert!(names.len() > 5, "{names:?}");
    assert_eq!(names[0], "_gc_big_0");
    assert!(wb.names.iter().all(|n| n.hidden && n.scope.is_none()));
    excel_limits_hold(&wb);
    assert_eq!(get(&wb, "big").as_deref(), Some(text.as_str()));
    assert_eq!(get(&wb, "BIG").as_deref(), Some(text.as_str()), "names are case-insensitive");
    // Shorter again: the extra chunks go.
    set(&mut wb, "big", "short").unwrap();
    assert_eq!(wb.names.len(), 1);
    assert_eq!(wb.names[0].formula, "\"short\"");
    set(&mut wb, "big", "").unwrap();
    assert!(wb.names.is_empty());
    assert_eq!(get(&wb, "big"), None);
}

#[test]
fn chunks_and_literals_respect_the_boundaries() {
    for len in [0, 1, 254, 255, 256, 8_000, 8_192, 8_193, 20_000] {
        let text = "q".repeat(len);
        let fs = chunk_formulas(&text);
        assert_eq!(fs.iter().map(|f| string_literals(f)).collect::<String>(), text, "{len}");
        assert!(fs.iter().all(|f| f.len() <= FORMULA_UNITS), "{len}");
    }
    // All quotes: each literal doubles in the formula.
    let text = "\"".repeat(9_000);
    let fs = chunk_formulas(&text);
    assert_eq!(fs.iter().map(|f| string_literals(f)).collect::<String>(), text);
    assert!(fs.iter().all(|f| f.len() <= FORMULA_UNITS));
}

#[test]
fn the_old_single_name_still_reads_and_is_replaced_on_write() {
    let mut wb = Workbook::new();
    wb.names.push(name("_sc_scripts", "\"{\"\"A\"\":[1]}\""));
    wb.names.push(name("_sc_scenarios", "\"[1,\"&\"2]\""));
    assert_eq!(get_json(&wb, "scripts"), Some(serde_json::json!({"A": [1]})));
    assert_eq!(get_json(&wb, "scenarios"), Some(serde_json::json!([1, 2])));
    set_json(&mut wb, "scripts", &serde_json::json!({"B": []})).unwrap();
    let names: Vec<&str> = wb.names.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, ["_sc_scenarios", "_gc_scripts_0"]);
    assert_eq!(get_json(&wb, "scripts"), Some(serde_json::json!({"B": []})));
}

#[test]
fn hostile_names_read_safely() {
    let mut wb = Workbook::new();
    // A gap: chunk 2 without chunk 1.
    wb.names.push(name("_gc_gap_0", "\"a\""));
    wb.names.push(name("_gc_gap_2", "\"c\""));
    assert_eq!(get(&wb, "gap").as_deref(), Some("a"));
    // Only a later chunk: no item, not listed, and the legacy name is still read.
    wb.names.push(name("_gc_orphan_2", "\"c\""));
    assert_eq!(get(&wb, "orphan"), None);
    assert_eq!(keys(&wb, "orphan"), Vec::<String>::new());
    wb.names.push(name("_sc_orphan", "\"old\""));
    assert_eq!(get(&wb, "orphan").as_deref(), Some("old"));
    assert_eq!(keys(&wb, "orphan"), ["orphan"]);
    // Chunks out of order, a duplicate, junk around the literals, an unterminated literal.
    wb.names.push(name("_gc_mix_1", "\"b\"&A1&"));
    wb.names.push(name("_gc_mix_0", "SUM(\"a\")"));
    wb.names.push(name("_gc_mix_0", "\"ignored\""));
    wb.names.push(name("_gc_mix_2", "\"c"));
    assert_eq!(get(&wb, "mix").as_deref(), Some("abc"));
    // Names that only look like chunks.
    for odd in ["_gc_", "_gc__0", "_gc_x_", "_gc_x_-1", "_gc_x_99999999999999999999", "_sc_", "_gcx_0"] {
        wb.names.push(name(odd, "\"zzz\""));
    }
    assert_eq!(get(&wb, "x"), None);
    assert_eq!(get(&wb, ""), None);
    // Not JSON, or JSON nested deeper than the parser allows.
    wb.names.push(name("_gc_notjson_0", "\"{\""));
    assert_eq!(get_json(&wb, "notjson"), None);
    let deep = format!("\"{}{}\"", "[".repeat(10_000), "]".repeat(10_000));
    wb.names.push(name("_gc_deep_0", &deep));
    assert_eq!(get_json(&wb, "deep"), None);
    // Sheet-scoped names are someone else's.
    wb.names.push(DefinedName { scope: Some(0), ..name("_gc_scoped_0", "\"s\"") });
    assert_eq!(get(&wb, "scoped"), None);
    // Bad keys are refused.
    for bad in ["", "has space", "quote\"", "a/b", &"k".repeat(201)] {
        assert!(set(&mut wb, bad, "v").is_err(), "{bad}");
    }
    assert!(set(&mut wb, "big", &"x".repeat(MAX_ITEM_CHARS + 1)).is_err());
}

#[test]
fn oversized_items_are_not_read() {
    let mut wb = Workbook::new();
    let chunk = format!("\"{}\"", "x".repeat(250));
    let big = vec![chunk.as_str(); 32].join("&");
    for i in 0..(MAX_ITEM_CHARS / 8_000 + 2) {
        wb.names.push(name(&format!("_gc_huge_{i}"), &big));
    }
    assert_eq!(get(&wb, "huge"), None);
}

#[test]
fn a_visible_name_of_the_item_shape_is_not_an_item() {
    // Someone's own visible `_gc_watch_0` is not storage: it is not read or listed, and
    // writing or removing the item leaves it alone.
    let mut wb = Workbook::new();
    let own = DefinedName { hidden: false, ..name("_gc_watch_0", "\"mine\"") };
    wb.names.push(own.clone());
    assert_eq!(get(&wb, "watch"), None);
    assert!(keys(&wb, "").is_empty());
    // Writing the item would put a hidden name of the same name beside it, and an xlsx file
    // keeps only one of them: refused, with the names as they were.
    let err = set(&mut wb, "watch", "theirs").unwrap_err().to_string();
    assert!(err.contains("_gc_watch_0"), "{err}");
    assert_eq!(wb.names, vec![own.clone()]);
    assert_eq!(get(&wb, "watch"), None);
    set(&mut wb, "other", "theirs").unwrap();
    assert_eq!(get(&wb, "other").as_deref(), Some("theirs"));
    remove(&mut wb, "watch");
    remove(&mut wb, "other");
    assert_eq!(wb.names, vec![own]);
}

#[test]
fn a_chunk_number_with_leading_zeros_is_another_name() {
    // `_gc_watch_00` is not chunk 0 under another spelling: a hidden one is not read, and a
    // visible one is not in the way of writing `_gc_watch_0`.
    let mut wb = Workbook::new();
    wb.names.push(name("_gc_watch_00", "\"theirs\""));
    assert_eq!(get(&wb, "watch"), None);
    assert!(keys(&wb, "").is_empty());
    let own = DefinedName { hidden: false, ..name("_gc_watch_00", "\"mine\"") };
    wb.names.push(own.clone());
    set(&mut wb, "watch", "ours").unwrap();
    assert_eq!(get(&wb, "watch").as_deref(), Some("ours"));
    assert_eq!(wb.names.len(), 3, "both other names stay");
    remove(&mut wb, "watch");
    assert_eq!(wb.names, vec![name("_gc_watch_00", "\"theirs\""), own]);
}

#[test]
fn keys_list_items_by_prefix() {
    let mut wb = Workbook::new();
    set(&mut wb, "macro_2", "b").unwrap();
    set(&mut wb, "macro_1", &"a".repeat(9_000)).unwrap();
    set(&mut wb, "scripts", "{}").unwrap();
    assert_eq!(keys(&wb, "macro_"), ["macro_1", "macro_2"]);
    assert_eq!(item_key("_gc_macro_1_3").as_deref(), Some("macro_1"));
    assert_eq!(item_key("_SC_Scripts").as_deref(), Some("scripts"));
    assert_eq!(item_key("Total"), None);
}

#[test]
fn listing_many_items_takes_one_pass_and_is_capped() {
    // 20,000 hidden items, as a file can carry: a `get` per key looked at every name for each
    // key (400 million visits); `items` looks at each name once, and lists at most MAX_ITEMS.
    let mut wb = Workbook::new();
    let total = 20_000;
    for i in 0..total {
        wb.names.push(name(&format!("_gc_macro_{i:05}_0"), &format!("\"m{i}\"")));
    }
    set(&mut wb, "macro_big", &"b".repeat(9_000)).unwrap();
    wb.names.push(name("_sc_macro_legacy", "\"old\""));
    wb.names.push(name("_gc_macro_orphan_1", "\"no chunk 0\""));
    wb.names.push(name("_gc_other_0", "\"not a macro\""));
    NAME_VISITS.with(|v| v.set(0));
    let all = items(&wb, "MACRO_");
    assert_eq!(NAME_VISITS.with(|v| v.get()), wb.names.len(), "one pass over the names");
    assert_eq!(all.len(), MAX_ITEMS);
    let first: Vec<&String> = all.keys().take(2).collect();
    assert_eq!(first, ["macro_00000", "macro_00001"]);
    assert_eq!(all.get("macro_09999").map(String::as_str), Some("m9999"));
    assert!(!all.contains_key("macro_10000"), "past the cap");
    NAME_VISITS.with(|v| v.set(0));
    let listed = keys(&wb, "macro_");
    assert_eq!(NAME_VISITS.with(|v| v.get()), wb.names.len());
    assert_eq!(listed, all.keys().cloned().collect::<Vec<_>>(), "keys and items agree");

    // Under the cap every item is there, read as `get` reads it.
    wb.names.retain(|n| !n.name.starts_with("_gc_macro_0") && !n.name.starts_with("_gc_macro_1"));
    let few = items(&wb, "macro_");
    let want: BTreeMap<String, String> =
        [("macro_big", "b".repeat(9_000)), ("macro_legacy", "old".into())].into_iter().map(|(k, t)| (k.to_string(), t)).collect();
    assert_eq!(few, want);
    for (k, text) in &few {
        assert_eq!(get(&wb, k).as_ref(), Some(text));
    }
    assert_eq!(keys(&wb, "macro_"), ["macro_big", "macro_legacy"]);
    assert_eq!(items(&wb, "").len(), 3, "the other item too");
}

#[test]
fn items_round_trip_through_xlsx() {
    let mut wb = Workbook::new();
    let text: String = (0..20_000).map(|i| ['x', '"', 'ü', '\u{1D11E}', ' '][i % 5]).collect();
    set(&mut wb, "blob", &text).unwrap();
    wb.names.push(name("_sc_watch", "\"[\"\"A1\"\"]\""));
    let bytes = joysheet_xlsx::write_xlsx(&wb).unwrap();
    let (back, warnings) = crate::io::open_bytes("data.xlsx", &bytes).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(get(&back, "blob").as_deref(), Some(text.as_str()));
    assert_eq!(get_json(&back, "watch"), Some(serde_json::json!(["A1"])));

    // Characters XML can't carry, and the escape marker itself, come back too, in text and
    // in JSON (which leaves U+FFFE and U+FFFF unescaped).
    let mut wb = Workbook::new();
    let odd: String = (0u32..0x20).filter_map(char::from_u32).chain(['\u{FFFE}', '\u{FFFF}', ESCAPE, 'a', '\u{7F}', '\u{FDD1}']).collect();
    let text: String = odd.chars().cycle().take(odd.chars().count() * 300).collect();
    set(&mut wb, "odd", &text).unwrap();
    excel_limits_hold(&wb);
    let json = serde_json::json!({"s": "x\u{FFFF}y\u{1}z\u{FDD0}0041"});
    set_json(&mut wb, "json", &json).unwrap();
    // All escapes: each literal holds whole escapes, within the limits.
    let controls = "\u{1}".repeat(9_000);
    set(&mut wb, "controls", &controls).unwrap();
    excel_limits_hold(&wb);
    let bytes = joysheet_xlsx::write_xlsx(&wb).unwrap();
    let (back, warnings) = crate::io::open_bytes("data.xlsx", &bytes).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(get(&back, "odd").as_deref(), Some(text.as_str()));
    assert_eq!(get_json(&back, "json"), Some(json));
    assert_eq!(get(&back, "controls").as_deref(), Some(controls.as_str()));
}

#[test]
fn escapes_count_against_the_limits_and_odd_markers_read_as_they_are() {
    // Excel's limits hold however many characters are escaped, and the longest item of
    // characters that all need escaping still reads.
    for len in [1, 50, 51, 52, 1_638, 1_639, 5_000] {
        let text = "\u{2}".repeat(len);
        let fs = chunk_formulas(&text);
        assert_eq!(unescape(&fs.iter().map(|f| string_literals(f)).collect::<String>()), text, "{len}");
        assert!(fs.iter().all(|f| f.encode_utf16().count() <= FORMULA_UNITS), "{len}");
    }
    let mut wb = Workbook::new();
    let longest = "\u{0}".repeat(MAX_ITEM_CHARS);
    set(&mut wb, "longest", &longest).unwrap();
    excel_limits_hold(&wb);
    assert_eq!(get(&wb, "longest").map(|t| t.len()), Some(MAX_ITEM_CHARS));
    // A marker in a hand-made name that is not an escape stays as it is.
    let mut wb = Workbook::new();
    wb.names.push(name("_gc_hand_0", "\"a\u{FDD0}zz\u{FDD0}D800\u{FDD0}004\u{FDD0}0041\u{FDD0}\""));
    assert_eq!(get(&wb, "hand").as_deref(), Some("a\u{FDD0}zz\u{FDD0}D800\u{FDD0}004A\u{FDD0}"));
}
