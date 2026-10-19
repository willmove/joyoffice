//! Ribbon, ruler and status-bar behavior for the first T205 slice.
//!
//! These tests live outside `src/` so the panic gate does not treat test
//! assertions as production panic sites.

use std::collections::BTreeMap;

use joyword_model::{Indent, Measure, OBJECT_MARKER, PageSetup, TabAlign, TabLeader, TabStop};
use joyword_shell::chrome::{
    DEFAULT_TAB_TWIPS, Evidence, FEATURE_FLOOR, LETTER_HEIGHT_TWIPS, LETTER_WIDTH_TWIPS,
    MAX_RULER_TABS, NARROW_MARGIN_TWIPS, RIBBON, RibbonArgs, RibbonControl, RibbonInput, RibbonTab,
    SURFACES, StatusSnapshot, TextCensus, census, controls_on, default_tab_interval, display_page,
    icon_art, icon_cells, icon_ids, ruler_model, status_line, visible_controls,
};
use joyword_shell::command::{
    ArgValue, Availability, CommandHost, CommandId, EDIT_BOLD, ExecuteError, InvokeError, Registry,
    VIEW_FIELD_CODES, VIEW_FORMATTING_MARKS, VIEW_HIDDEN_TEXT, ZOOM_IN,
};
use joyword_shell::host::Office;
use joyword_shell::l10n::{Catalog, Locale};

struct LogHost {
    log: Vec<(CommandId, BTreeMap<&'static str, ArgValue>)>,
    enabled: bool,
}

impl CommandHost for LogHost {
    fn availability(&self, _id: CommandId) -> Availability {
        if self.enabled {
            Availability::Enabled
        } else {
            Availability::Disabled("zoom.max")
        }
    }

    fn execute(
        &mut self,
        id: CommandId,
        args: &BTreeMap<&'static str, ArgValue>,
    ) -> Result<(), ExecuteError> {
        self.log.push((id, args.clone()));
        Ok(())
    }
}

fn control(id: &str) -> &'static RibbonControl {
    RIBBON
        .iter()
        .find(|control| control.id == id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

fn twips(value: i64) -> i64 {
    Measure::Twips(value)
        .absolute()
        .map(|length| length.raw())
        .unwrap_or_else(|| panic!("twips {value}"))
}

#[test]
fn census_counts_cjk_per_character_and_keeps_the_other_buckets_apart() {
    let counted = census("你好 world!");
    assert_eq!(counted.cjk, 2);
    assert_eq!(counted.latin_words, 1);
    assert_eq!(counted.punctuation, 1);
    assert_eq!(counted.spaces, 1);
    assert_eq!(counted.words(), 3);
    assert_eq!(counted.graphemes, 9);

    let chinese = census("你好，世界。");
    assert_eq!(chinese.cjk, 4);
    assert_eq!(chinese.punctuation, 2);
    assert_eq!(chinese.words(), 4);

    let contraction = census("don't");
    assert_eq!(contraction.latin_words, 1);
    assert_eq!(contraction.punctuation, 0);

    let number = census("12.5 ３");
    assert_eq!(number.numbers, 2);
    assert_eq!(number.spaces, 1);
    assert_eq!(number.words(), 2);

    let spaced = census("a  b");
    assert_eq!(spaced.latin_words, 2);
    assert_eq!(spaced.spaces, 2);
}

#[test]
fn census_counts_graphemes_and_skips_the_page_break_marker() {
    let mark = OBJECT_MARKER.to_string();
    let mixed = census(&format!("你{mark}a"));
    assert_eq!(mixed.cjk, 1);
    assert_eq!(mixed.latin_words, 1);
    assert_eq!(mixed.graphemes, 2);

    let accent = census("e\u{0301}");
    assert_eq!(accent.graphemes, 1);
    assert_eq!(accent.latin_words, 1);

    let family = census("👨\u{200D}👩\u{200D}👧\u{200D}👦");
    assert_eq!(family.graphemes, 1);
    assert_eq!(family.words(), 0);
    assert_eq!(family.other, 1);

    assert_eq!(census("こんにちは").cjk, 5);
    assert_eq!(census("한").cjk, 1);
}

#[test]
fn status_line_reports_page_and_the_separate_counts() {
    let census = census("你好 world!");
    let status = StatusSnapshot {
        page: Some(2),
        pages: Some(4),
        census,
    };
    let zh = status_line(Catalog::shipped(Locale::ZhCn), &status);
    assert!(zh.contains('2') && zh.contains('4'), "{zh}");
    assert!(zh.contains('3'), "{zh}");
    let en = status_line(Catalog::shipped(Locale::En), &status);
    assert!(en.contains("Page 2 of 4"), "{en}");
    assert!(en.contains("3 words"), "{en}");
    assert!(en.contains("9 graphemes"), "{en}");
    assert!(en.contains("CJK 2"), "{en}");
    assert!(en.contains("punctuation 1"), "{en}");
    assert!(en.contains("spaces 1"), "{en}");

    let unknown = status_line(
        Catalog::shipped(Locale::En),
        &StatusSnapshot {
            page: None,
            pages: None,
            census: TextCensus::default(),
        },
    );
    assert!(unknown.contains("Page unavailable"), "{unknown}");
}

#[test]
fn display_page_is_one_based_and_keeps_a_count_without_a_caret() {
    assert_eq!(display_page(Some(0), 3), (Some(1), Some(3)));
    assert_eq!(display_page(Some(2), 3), (Some(3), Some(3)));
    assert_eq!(display_page(Some(9), 3), (Some(3), Some(3)));
    assert_eq!(display_page(None, 3), (None, Some(3)));
    assert_eq!(display_page(Some(0), 0), (None, None));
}

#[test]
fn ruler_places_first_line_hanging_and_custom_tabs() {
    let width = twips(6 * 1_440);
    let interval = twips(DEFAULT_TAB_TWIPS);
    let first = Indent {
        start: Some(Measure::Twips(720)),
        first_line: Some(Measure::Twips(360)),
        ..Indent::default()
    };
    let tab = TabStop {
        position: Measure::Twips(1_440),
        align: TabAlign::Left,
        leader: TabLeader::None,
    };
    let model = ruler_model(&first, &[tab], interval, Some(width));
    assert_eq!(model.left, Some(twips(720)));
    assert_eq!(model.first_line, Some(twips(1_080)));
    assert_eq!(model.right, Some(width));
    assert!(!model.unresolved);
    assert!(model.tabs.iter().any(|stop| {
        stop.custom && stop.position == twips(1_440) && stop.align == TabAlign::Left
    }));
    assert!(model.tabs.iter().any(|stop| !stop.custom));
    assert!(model.tabs.len() <= MAX_RULER_TABS + 1);

    let hanging = Indent {
        start: Some(Measure::Twips(720)),
        hanging: Some(Measure::Twips(360)),
        ..Indent::default()
    };
    let hung = ruler_model(&hanging, &[], interval, Some(width));
    assert_eq!(hung.first_line, Some(twips(720)));
    assert_eq!(hung.left, Some(twips(1_080)));
}

#[test]
fn ruler_does_not_invent_positions_for_character_units() {
    let indent = Indent {
        start: Some(Measure::CharHundredths(100)),
        ..Indent::default()
    };
    let model = ruler_model(
        &indent,
        &[],
        twips(DEFAULT_TAB_TWIPS),
        Some(twips(6 * 1_440)),
    );
    assert!(model.unresolved);
    assert_eq!(model.left, None);
    assert_eq!(model.first_line, None);

    let tab = TabStop {
        position: Measure::CharHundredths(50),
        align: TabAlign::Center,
        leader: TabLeader::None,
    };
    let tabs = ruler_model(&Indent::default(), &[tab], twips(DEFAULT_TAB_TWIPS), None);
    assert!(tabs.unresolved);
    assert!(tabs.tabs.iter().all(|stop| !stop.custom));
    assert!(tabs.content_width.is_none());
}

#[test]
fn omitted_default_tab_stop_is_720_twips_and_a_relative_one_is_unresolved() {
    let (interval, unresolved) = default_tab_interval(None);
    assert_eq!(interval, twips(720));
    assert!(!unresolved);
    let (fallback, unresolved) = default_tab_interval(Some(Measure::CharHundredths(100)));
    assert_eq!(fallback, twips(720));
    assert!(unresolved);
}

#[test]
fn ribbon_hides_commands_the_registry_does_not_contain() {
    let builtin = Registry::builtin();
    let shown = visible_controls(&builtin, RIBBON);
    assert_eq!(shown.len(), RIBBON.len());
    assert!(shown.iter().any(|control| control.id == "start.bold"));
    let empty = Registry::new(Vec::new());
    assert!(visible_controls(&empty, RIBBON).is_empty());
    let zoom_only = Registry::new(
        builtin
            .commands()
            .iter()
            .filter(|spec| spec.id == ZOOM_IN)
            .cloned()
            .collect(),
    );
    let zoom = visible_controls(&zoom_only, RIBBON);
    assert!(zoom.iter().all(|control| control.command == ZOOM_IN));
    assert!(!zoom.iter().any(|control| control.id == "start.bold"));
}

#[test]
fn view_ribbon_commands_do_not_modify_the_document() {
    let registry = Registry::builtin();
    let view = controls_on(RibbonTab::View, visible_controls(&registry, RIBBON));
    assert!(!view.is_empty());
    for control in view {
        let spec = registry.get(control.command).unwrap();
        assert!(!spec.modifies_document, "{}", control.id);
    }
}

#[test]
fn word_count_matches_the_documented_edit_009_rule() {
    // Hand count of the EDIT-009 rule in `census`, not a Word dialog.
    // 你 好 are two CJK words; ， is punctuation.
    // Word, don't and count are three Latin words; the apostrophe stays inside don't.
    // 2024 and 12.5 are two numbers; the dot stays inside 12.5.
    // 的 字 数 are three CJK words; ： and 。 are punctuation.
    // Four spaces separate the Latin and numeric runs.
    let counted = census("你好，Word 2024 的字数：don't count 12.5。");
    assert_eq!(
        counted,
        TextCensus {
            cjk: 5,
            latin_words: 3,
            numbers: 2,
            punctuation: 3,
            spaces: 4,
            graphemes: 34,
            other: 0,
        }
    );
    assert_eq!(counted.words(), 10);
}

#[test]
fn view_toggles_flip_without_a_document() {
    let registry = Registry::builtin();
    let mut host = Office::new();
    for id in [VIEW_FORMATTING_MARKS, VIEW_HIDDEN_TEXT, VIEW_FIELD_CODES] {
        assert_eq!(
            CommandHost::availability(&host, id),
            Availability::Enabled,
            "{id}"
        );
        assert!(!registry.get(id).unwrap().modifies_document, "{id}");
        host.invoke(id, &[]).unwrap();
    }
    let toggles = host.view_toggles();
    assert!(toggles.formatting_marks);
    assert!(toggles.show_hidden);
    assert!(toggles.field_codes);
    host.invoke(VIEW_FORMATTING_MARKS, &[]).unwrap();
    assert!(!host.view_toggles().formatting_marks);
}

#[test]
fn ribbon_bindings_invoke_the_registered_command() {
    let registry = Registry::builtin();
    let input = RibbonInput {
        query: "甲",
        replacement: "乙",
        case_sensitive: true,
        whole_word: false,
        format_repeat: true,
    };
    let mut host = LogHost {
        log: Vec::new(),
        enabled: true,
    };
    for control in visible_controls(&registry, RIBBON) {
        let call = control.bind(input);
        if control.args == RibbonArgs::Dialog {
            let error = registry
                .invoke_with(&mut host, call.command, &call.args)
                .unwrap_err();
            assert!(
                matches!(error, InvokeError::PendingInput { .. }),
                "{}",
                control.id
            );
            continue;
        }
        registry
            .invoke_with(&mut host, call.command, &call.args)
            .unwrap_or_else(|error| panic!("{}: {error:?}", control.id));
        let (id, args) = host.log.last().unwrap();
        assert_eq!(*id, control.command, "{}", control.id);
        match control.args {
            RibbonArgs::Fixed(pairs) => {
                assert_eq!(args.len(), pairs.len(), "{}", control.id);
            }
            RibbonArgs::Find { replacement } => {
                assert!(matches!(args.get("query"), Some(ArgValue::Text(text)) if text == "甲"));
                if replacement {
                    assert!(
                        matches!(args.get("replacement"), Some(ArgValue::Text(text)) if text == "乙")
                    );
                }
            }
            RibbonArgs::FormatCopy => {
                assert_eq!(args.get("repeat"), Some(&ArgValue::Bool(true)));
            }
            RibbonArgs::None => assert!(args.is_empty(), "{}", control.id),
            RibbonArgs::Dialog => unreachable!("dialog controls return before this match"),
        }
    }
}

#[test]
fn an_empty_find_query_asks_for_input_instead_of_searching() {
    let registry = Registry::builtin();
    let call = control("start.find_next").bind(RibbonInput::default());
    let mut host = LogHost {
        log: Vec::new(),
        enabled: true,
    };
    let error = registry
        .invoke_with(&mut host, call.command, &call.args)
        .unwrap_err();
    assert!(matches!(
        error,
        InvokeError::PendingInput { missing } if missing.contains(&"query")
    ));
    assert!(host.log.is_empty());
}

#[test]
fn a_disabled_ribbon_command_does_not_run() {
    let registry = Registry::builtin();
    let mut host = LogHost {
        log: Vec::new(),
        enabled: false,
    };
    let error = registry.invoke(&mut host, EDIT_BOLD).unwrap_err();
    assert!(matches!(error, InvokeError::Disabled(_)));
    assert!(host.log.is_empty());
}

#[test]
fn page_presets_match_the_product_page_setup() {
    let page = PageSetup::a4_zh_cn();
    assert_eq!(page.width, Measure::Twips(11_906));
    assert_eq!(page.height, Measure::Twips(16_838));
    assert_eq!(page.top, Measure::Twips(1_440));
    assert_eq!(page.right, Measure::Twips(1_800));
    assert_eq!(page.bottom, Measure::Twips(1_440));
    assert_eq!(page.left, Measure::Twips(1_800));
    assert_eq!(preset_text("layout.a4", "width"), Some("11906"));
    assert_eq!(preset_text("layout.a4", "height"), Some("16838"));
    assert_eq!(LETTER_WIDTH_TWIPS, 12_240);
    assert_eq!(LETTER_HEIGHT_TWIPS, 15_840);
    assert_eq!(NARROW_MARGIN_TWIPS, 720);
    assert_eq!(preset_text("layout.letter", "width"), Some("12240"));
    assert_eq!(preset_text("layout.letter", "height"), Some("15840"));
    assert_eq!(preset_text("layout.margins", "left"), Some("1800"));
    assert_eq!(preset_text("layout.margins_narrow", "top"), Some("720"));
    assert_eq!(
        preset_text("layout.portrait", "orientation"),
        Some("portrait")
    );
    assert_eq!(
        preset_text("layout.landscape", "orientation"),
        Some("landscape")
    );
    assert_eq!(preset_text("start.list_level_1", "level"), Some("0"));
    assert_eq!(preset_text("start.list_level_2", "level"), Some("1"));
}

fn preset_text(id: &str, name: &str) -> Option<&'static str> {
    let RibbonArgs::Fixed(pairs) = control(id).args else {
        return None;
    };
    pairs.iter().find_map(|pair| match pair.value {
        joyword_shell::chrome::PresetValue::Text(text) if pair.name == name => Some(text),
        _ => None,
    })
}

#[test]
fn every_icon_is_a_16_by_16_drawing_and_every_control_uses_one() {
    let mut ids = BTreeMap::new();
    for id in icon_ids() {
        let cells = icon_cells(id);
        assert!(!cells.is_empty(), "{id}");
        assert!(cells.iter().all(|(x, y)| *x < 16 && *y < 16), "{id}");
        assert!(ids.insert(id, cells.len()).is_none(), "duplicate {id}");
    }
    for control in RIBBON {
        assert!(ids.contains_key(control.icon), "{}", control.id);
    }
}

#[test]
fn the_feature_catalog_matches_the_ribbon_and_does_not_drop_the_floor() {
    let text = include_str!("../../../docs/joyword/feature_catalog.json");
    let floor = json_number(text, "floor");
    assert_eq!(floor, FEATURE_FLOOR);
    const {
        assert!(FEATURE_FLOOR >= 34, "the catalog floor only moves up");
    }
    let entries = json_objects(text);
    let controls: Vec<_> = entries
        .iter()
        .filter(|entry| entry.contains_key("requirement"))
        .collect();
    let icons: Vec<_> = entries
        .iter()
        .filter(|entry| entry.get("origin").is_some())
        .collect();
    assert_eq!(controls.len(), FEATURE_FLOOR);
    let expected = catalog_rows();
    assert_eq!(controls.len(), expected.len());
    for (entry, row) in controls.iter().zip(expected.iter()) {
        assert_eq!(entry.get("id").map(String::as_str), Some(row.id));
        assert_eq!(entry.get("kind").map(String::as_str), Some(row.kind));
        assert_eq!(
            entry.get("requirement").map(String::as_str),
            Some(row.requirement)
        );
        assert_eq!(
            entry.get("evidence").map(String::as_str),
            Some(row.evidence)
        );
        assert_eq!(entry.get("command").map(String::as_str), row.command);
        assert_eq!(entry.get("icon").map(String::as_str), row.icon);
        assert_eq!(entry.get("tab").map(String::as_str), row.tab);
    }
    for icon in &icons {
        let id = icon.get("id").map(String::as_str).unwrap_or("");
        assert_eq!(
            icon.get("origin").map(String::as_str),
            Some("drawn-in-code")
        );
        assert_eq!(icon.get("license").map(String::as_str), Some("MIT"));
        assert!(icon_art(id).is_some(), "{id}");
    }
    for control in RIBBON {
        assert!(
            icons
                .iter()
                .any(|icon| icon.get("id").map(String::as_str) == Some(control.icon)),
            "{}",
            control.icon
        );
    }
}

struct CatalogRow {
    id: &'static str,
    kind: &'static str,
    tab: Option<&'static str>,
    command: Option<&'static str>,
    requirement: &'static str,
    evidence: &'static str,
    icon: Option<&'static str>,
}

fn catalog_rows() -> Vec<CatalogRow> {
    let mut rows = Vec::new();
    for control in RIBBON {
        rows.push(CatalogRow {
            id: control.id,
            kind: "command",
            tab: Some(control.tab.as_str()),
            command: Some(control.command.0),
            requirement: control.requirement,
            evidence: control.evidence.as_str(),
            icon: Some(control.icon),
        });
    }
    for surface in SURFACES {
        rows.push(CatalogRow {
            id: surface.id,
            kind: surface.kind,
            tab: None,
            command: None,
            requirement: surface.requirement,
            evidence: surface.evidence.as_str(),
            icon: None,
        });
    }
    rows
}

fn json_number(text: &str, key: &str) -> usize {
    for line in text.lines() {
        let line = line.trim().trim_end_matches(',');
        let Some(rest) = line.strip_prefix('"') else {
            continue;
        };
        let Some((found, tail)) = rest.split_once('"') else {
            continue;
        };
        if found == key {
            let digits: String = tail.chars().filter(|ch| ch.is_ascii_digit()).collect();
            return digits.parse().unwrap_or(0);
        }
    }
    0
}

fn json_objects(text: &str) -> Vec<BTreeMap<String, String>> {
    text.lines()
        .filter_map(|line| parse_flat_object(line.trim().trim_end_matches(',')))
        .collect()
}

fn parse_flat_object(line: &str) -> Option<BTreeMap<String, String>> {
    let mut chars = line.chars().peekable();
    if chars.next() != Some('{') {
        return None;
    }
    let mut map = BTreeMap::new();
    loop {
        skip_space(&mut chars);
        match chars.peek() {
            Some('}') => break,
            Some('"') => {}
            _ => return None,
        }
        let key = read_string(&mut chars)?;
        skip_space(&mut chars);
        if chars.next() != Some(':') {
            return None;
        }
        skip_space(&mut chars);
        let value = if chars.peek() == Some(&'"') {
            read_string(&mut chars)?
        } else {
            let mut number = String::new();
            while let Some(ch) = chars.peek() {
                if ch.is_ascii_digit() {
                    number.push(*ch);
                    chars.next();
                } else {
                    break;
                }
            }
            if number.is_empty() {
                return None;
            }
            number
        };
        map.insert(key, value);
        skip_space(&mut chars);
        match chars.next() {
            Some(',') => continue,
            Some('}') => break,
            _ => return None,
        }
    }
    Some(map)
}

fn skip_space(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while chars.peek().is_some_and(|ch| ch.is_whitespace()) {
        chars.next();
    }
}

fn read_string(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
    if chars.next() != Some('"') {
        return None;
    }
    let mut out = String::new();
    for ch in chars.by_ref() {
        if ch == '"' {
            return Some(out);
        }
        out.push(ch);
    }
    None
}

#[test]
fn evidence_labels_match_the_catalog_vocabulary() {
    assert_eq!(Evidence::Unit.as_str(), "unit");
    assert_eq!(Evidence::Exists.as_str(), "exists");
    assert_eq!(Evidence::Corpus.as_str(), "corpus");
    assert_eq!(Evidence::Reference.as_str(), "reference");
}
