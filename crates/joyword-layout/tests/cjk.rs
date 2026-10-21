use joyword_foundation::Emu;
use joyword_layout::{LoadedFont, cjk::*};
use joyword_model::ThemeFont;

fn pt(points: i64) -> Emu {
    Emu::new(points * Emu::PER_POINT)
}

fn half_points(value: i64) -> Emu {
    Emu::new(value * Emu::PER_POINT / 2)
}

fn win_span(thousandths: i32) -> VerticalSpan {
    VerticalSpan {
        units_per_em: 1000,
        ascent: thousandths,
        descent: 0,
    }
}

fn assert_band(length: Emu, low: f64, high: f64) {
    let points = length.points();
    assert!(
        (low..=high).contains(&points),
        "{points} pt outside {low}..={high}"
    );
}

#[test]
fn r4_uses_the_win_span_and_the_measured_grid() {
    let set = MetricSet {
        units_per_em: 1000,
        hhea: win_span(1000),
        typo: win_span(800),
        win: win_span(1042),
    };
    assert_eq!(word_line_span(&set), set.win);

    let twelve = pt(12);
    let ten_point_five = half_points(21);
    let grid = measured_line_grid(true).unwrap();
    assert_eq!(grid.pitch.raw(), 312 * Emu::PER_TWIP);
    assert_eq!(grid.pitch.points(), 15.6);

    let dengxian = natural_line_extent(twelve, win_span(1042)).unwrap();
    assert_eq!(dengxian.evidence, Evidence::Inferred("R4"));
    assert_band(dengxian.length, 16.20, 16.35);
    let snapped = snap_line_extent(dengxian.length, grid).unwrap();
    assert_eq!(snapped.length.points(), 31.2);

    let dengxian_small = natural_line_extent(ten_point_five, win_span(1042)).unwrap();
    assert_band(dengxian_small.length, 14.16, 14.28);
    assert_eq!(
        snap_line_extent(dengxian_small.length, grid)
            .unwrap()
            .length
            .points(),
        15.6
    );

    let yahei = natural_line_extent(twelve, win_span(1320)).unwrap();
    assert_band(yahei.length, 20.52, 20.64);
    assert_eq!(
        single_line_height(twelve, win_span(1320), grid)
            .unwrap()
            .length
            .points(),
        31.2
    );

    let simsun = single_line_height(twelve, win_span(1000), grid).unwrap();
    assert_eq!(simsun.length.points(), 15.6);
    let simsun_small = single_line_height(ten_point_five, win_span(1000), grid).unwrap();
    assert_eq!(simsun_small.length.points(), 15.6);

    let off = measured_line_grid(false).unwrap();
    assert_eq!(
        snap_line_extent(dengxian.length, off).unwrap().length,
        dengxian.length
    );
    assert_eq!(
        snap_line_extent(
            dengxian.length,
            LineGrid {
                pitch: Emu::new(0),
                snap: true
            }
        ),
        Err(CjkIssue::IllegalGrid)
    );
    assert_eq!(
        natural_line_extent(Emu::new(0), win_span(1000)),
        Err(CjkIssue::IllegalMeasure)
    );
}

#[test]
fn r7_punctuation_widths_follow_the_measured_pairs() {
    let em = pt(12);
    let quote = punctuation_advance('\u{201C}', Some('成'), em).unwrap();
    assert_eq!(quote.evidence, Evidence::Measured("R7"));
    assert_eq!(quote.length.raw(), 431 * Emu::PER_POINT / 100);
    assert_eq!(
        punctuation_advance('\u{201D}', Some('，'), em)
            .unwrap()
            .length,
        quote.length
    );

    let run: Vec<char> = "：：；。，、以".chars().collect();
    for (index, ch) in run.iter().enumerate().take(5) {
        let width = punctuation_advance(*ch, Some(run[index + 1]), em).unwrap();
        assert_eq!(width.length, pt(6), "{ch}");
    }
    assert_eq!(
        punctuation_advance('、', Some('以'), em).unwrap().length,
        em
    );
    assert_eq!(
        punctuation_advance('，', Some('其'), em).unwrap().length,
        em
    );
    assert_eq!(
        punctuation_advance('（', Some('括'), em).unwrap().length,
        em
    );
    assert_eq!(
        punctuation_advance('）', Some('：'), em).unwrap().length,
        pt(6)
    );
    assert_eq!(
        punctuation_advance('：', Some('甲'), em).unwrap().length,
        em
    );
    assert_eq!(
        punctuation_advance('。', Some('（'), em).unwrap().length,
        pt(6)
    );
    assert_eq!(
        punctuation_advance('（', Some('括'), em).unwrap().length,
        em
    );
    assert_eq!(
        punctuation_advance('。', Some(' '), em),
        Err(CjkIssue::UnverifiedPunctuation)
    );
    assert_eq!(
        punctuation_advance('！', Some('再'), em).unwrap().length,
        em
    );
    assert_eq!(
        punctuation_advance('？', Some('答'), em).unwrap().length,
        em
    );

    let doubled = punctuation_advance('\u{201C}', Some('成'), pt(24)).unwrap();
    assert_eq!(doubled.length.raw(), quote.length.raw() * 2);
    assert!(matches!(
        doubled.evidence,
        Evidence::Scaled { rule: "R7", .. }
    ));

    assert_eq!(
        punctuation_advance('「', Some('字'), em),
        Err(CjkIssue::UnverifiedPunctuation)
    );
    assert_eq!(
        punctuation_advance('（', Some('）'), em),
        Err(CjkIssue::UnverifiedPunctuation)
    );
    assert_eq!(
        punctuation_advance('！', Some('。'), em),
        Err(CjkIssue::UnverifiedPunctuation)
    );
    assert_eq!(
        punctuation_advance(',', Some('字'), em),
        Err(CjkIssue::UnverifiedPunctuation)
    );
    assert_eq!(
        punctuation_advance('甲', None, em),
        Err(CjkIssue::NotPunctuation)
    );
    assert_eq!(
        punctuation_advance('。', None, Emu::new(0)),
        Err(CjkIssue::IllegalMeasure)
    );
}

fn glyphs(text: &str, advance: Emu) -> Vec<JustifyGlyph> {
    text.char_indices()
        .map(|(cluster, ch)| JustifyGlyph {
            ch,
            cluster,
            advance,
        })
        .collect()
}

#[test]
fn justify_line_matches_r3_on_pure_chinese_and_spares_latin_words() {
    let chinese = glyphs("甲乙丙", pt(12));
    let r3 = justify_intercharacter(&[pt(12); 3], pt(48), false).unwrap();
    let mut expected = r3.gaps.clone();
    expected.push(Emu::new(0));
    assert_eq!(justify_line(&chinese, pt(48), false).unwrap(), expected);

    // Gaps: after 甲 (before A), after the space, between 乙 and 1, and
    // nowhere inside "Ab" or "12".
    let mixed = glyphs("甲Ab 乙12", pt(10));
    let gaps = justify_line(&mixed, pt(100), false).unwrap();
    assert_eq!(
        gaps,
        vec![pt(10), pt(0), pt(0), pt(10), pt(10), pt(0), pt(0)]
    );

    // A trailing space hangs: it takes no share and its width is not counted.
    let trailing = glyphs("甲乙 ", pt(10));
    assert_eq!(
        justify_line(&trailing, pt(30), false).unwrap(),
        vec![pt(10), pt(0), pt(0)]
    );

    // Multi-glyph clusters stretch only between clusters.
    let mut composed = glyphs("甲乙", pt(10));
    composed.insert(
        1,
        JustifyGlyph {
            ch: '\u{0301}',
            cluster: 0,
            advance: Emu::new(0),
        },
    );
    assert_eq!(
        justify_line(&composed, pt(30), false).unwrap(),
        vec![pt(0), pt(10), pt(0)]
    );

    assert_eq!(
        justify_line(&glyphs("summary", pt(10)), pt(100), false),
        Err(CjkIssue::UnverifiedJustify)
    );
    assert_eq!(
        justify_line(&glyphs("甲 b", pt(10)), pt(100), true).unwrap(),
        vec![pt(0); 3]
    );
    assert_eq!(
        justify_line(&glyphs("甲乙", pt(10)), pt(10), false),
        Err(CjkIssue::DoesNotFit)
    );
    assert_eq!(
        justify_line(&glyphs("  ", pt(10)), pt(100), false).unwrap(),
        vec![pt(0); 2]
    );
}

#[test]
fn r3_justify_fills_the_line_and_leaves_the_last_line() {
    let advances = [pt(12), pt(12), pt(12)];
    let filled = justify_intercharacter(&advances, pt(48), false).unwrap();
    assert_eq!(filled.evidence, Evidence::Inferred("R3"));
    assert_eq!(filled.gaps, vec![pt(6), pt(6)]);
    let total = advances.iter().map(|advance| advance.raw()).sum::<i64>()
        + filled.gaps.iter().map(|gap| gap.raw()).sum::<i64>();
    assert_eq!(total, pt(48).raw());

    let remainder = advances.iter().map(|advance| advance.raw()).sum::<i64>() + 5;
    let split = justify_intercharacter(&advances, Emu::new(remainder), false).unwrap();
    assert_eq!(split.gaps, vec![Emu::new(3), Emu::new(2)]);
    assert_eq!(split.gaps.iter().map(|gap| gap.raw()).sum::<i64>(), 5);

    let last = justify_intercharacter(&advances, pt(48), true).unwrap();
    assert_eq!(last.gaps, vec![Emu::new(0), Emu::new(0)]);
    assert_eq!(
        justify_intercharacter(&[pt(12)], pt(24), false),
        Err(CjkIssue::UnverifiedJustify)
    );
    assert_eq!(
        justify_intercharacter(&advances, pt(24), false),
        Err(CjkIssue::DoesNotFit)
    );
}

#[test]
fn unsupported_followers_and_extrapolated_evidence_are_explicit() {
    for ch in ['：', '。', '“', '”', '（', '！'] {
        for next in [None, Some('A'), Some(' '), Some('「'), Some('!')] {
            assert_eq!(
                punctuation_advance(ch, next, pt(12)),
                Err(CjkIssue::UnverifiedPunctuation)
            );
        }
    }
    assert_eq!(
        punctuation_advance('：', Some('）'), pt(12)),
        Err(CjkIssue::UnverifiedPunctuation)
    );
    assert_eq!(
        punctuation_advance('，', Some('汉'), pt(12))
            .unwrap()
            .evidence,
        Evidence::Inferred("R7")
    );
    let arbitrary = natural_line_extent(pt(37), win_span(1573)).unwrap();
    assert_eq!(arbitrary.evidence, Evidence::Inferred("R4"));
    assert_eq!(
        snap_line_extent(
            arbitrary.length,
            LineGrid {
                pitch: pt(19),
                snap: true
            }
        )
        .unwrap()
        .evidence,
        Evidence::Inferred("R4")
    );
    assert_eq!(
        natural_line_extent(pt(12), win_span(0)),
        Err(CjkIssue::IllegalMeasure)
    );
}

#[test]
fn arithmetic_and_allocation_fail_without_partial_results() {
    assert_eq!(
        natural_line_extent(Emu::new(i64::MAX), win_span(i32::MAX)),
        Err(CjkIssue::Overflow)
    );
    assert_eq!(
        snap_line_extent(
            Emu::new(i64::MAX),
            LineGrid {
                pitch: Emu::new(2),
                snap: true
            }
        ),
        Err(CjkIssue::Overflow)
    );
    assert_eq!(
        scale_hundredths(i64::MAX, Emu::new(i64::MAX)),
        Err(CjkIssue::Overflow)
    );
    assert_eq!(
        scale_hundredths(-50, Emu::new(3)).unwrap().length,
        Emu::new(-2)
    );
    assert_eq!(
        justify_intercharacter(
            &[Emu::new(i64::MAX), Emu::new(1)],
            Emu::new(i64::MAX),
            false
        ),
        Err(CjkIssue::Overflow)
    );
    assert_eq!(
        justify_intercharacter(&[Emu::new(-1)], pt(12), false),
        Err(CjkIssue::IllegalMeasure)
    );
    let too_many = vec![Emu::new(0); MAX_JUSTIFY_ADVANCES + 1];
    assert_eq!(
        justify_intercharacter(&too_many, pt(12), false),
        Err(CjkIssue::BudgetExceeded)
    );
    assert_eq!(
        justify_intercharacter(&too_many[..MAX_JUSTIFY_ADVANCES], Emu::new(0), true)
            .unwrap()
            .gaps
            .len(),
        MAX_JUSTIFY_ADVANCES - 1
    );
}

#[test]
fn relative_units_and_unmeasured_adjustments_stay_unverified() {
    assert_eq!(
        resolve_char_hundredths(200, None),
        Err(CjkIssue::UnverifiedCharUnit)
    );
    assert_eq!(
        resolve_line_hundredths(100, None),
        Err(CjkIssue::UnverifiedLineUnit)
    );
    let chars = resolve_char_hundredths(200, Some(pt(12))).unwrap();
    assert_eq!(chars.length, pt(24));
    assert_eq!(chars.evidence, Evidence::SuppliedBasis);
    assert_eq!(
        resolve_line_hundredths(50, Some(pt(15)))
            .unwrap()
            .length
            .points(),
        7.5
    );
    assert_eq!(
        scale_hundredths(1, Emu::new(0)),
        Err(CjkIssue::IllegalMeasure)
    );
    assert_eq!(auto_space_width(), Err(CjkIssue::UnverifiedAutoSpace));
    assert_eq!(
        adjust_right_indent(),
        Err(CjkIssue::UnverifiedAdjustRightInd)
    );
    assert!(needs_auto_space(script_run('察'), script_run('W')));
    assert!(!needs_auto_space(script_run('察'), script_run(' ')));
    assert!(!needs_auto_space(script_run('甲'), script_run('乙')));
    assert_eq!(diagnose_missing_glyphs(0), Ok(()));
    assert_eq!(diagnose_missing_glyphs(2), Err(CjkIssue::MissingGlyph));
}

#[test]
fn line_edge_table_matches_the_existing_break_predicate() {
    for ch in [
        '，', '。', '！', '？', '；', '：', '、', '）', '】', '》', '」', '』', ',', '.', '!', '?',
        ';', ':', ')',
    ] {
        assert!(prohibits_line_start(ch), "{ch}");
        assert!(
            joyword_layout::prohibited_line_edge(&ch.to_string()),
            "{ch}"
        );
        assert!(!prohibits_line_end(ch), "{ch}");
    }
    for ch in ['（', '【', '《', '「', '『', '(', '['] {
        assert!(prohibits_line_end(ch), "{ch}");
        assert!(!prohibits_line_start(ch), "{ch}");
        let text = format!("甲{ch}");
        assert!(joyword_layout::prohibited_line_edge(&text), "{ch}");
    }
    assert!(!prohibits_line_start('甲'));
    assert!(!prohibits_line_end('甲'));
}

#[test]
fn families_match_exactly_and_theme_slots_do_not_cross() {
    assert_eq!(default_east_asia_family(), CjkFamily::DengXian);
    assert_eq!(parse_cjk_family(" 宋体 "), Some(CjkFamily::SimSun));
    assert_eq!(parse_cjk_family("simsun"), Some(CjkFamily::SimSun));
    assert_eq!(parse_cjk_family("NSimSun"), None);
    assert_eq!(parse_cjk_family("Songti SC"), None);
    assert_eq!(
        parse_cjk_family("微软雅黑"),
        Some(CjkFamily::MicrosoftYaHei)
    );
    assert_eq!(
        parse_cjk_family("microsoft yahei"),
        Some(CjkFamily::MicrosoftYaHei)
    );
    assert_eq!(CjkFamily::KaiTi.canonical_name(), "楷体");

    let font = LoadedFont::from_bytes(
        std::sync::Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .unwrap();
    let metrics = metric_set(&font).unwrap();
    assert_eq!(metrics.units_per_em, font.units_per_em);
    assert_eq!(word_line_span(&metrics), metrics.win);
    assert!(metrics.win.ascent >= 0 && metrics.win.descent >= 0);

    let available = [AvailableFace {
        family: CjkFamily::DengXian,
        key: font.key(),
    }];
    assert_eq!(
        match_family(CjkFamily::DengXian, &available),
        FontMatch::Exact {
            family: CjkFamily::DengXian,
            key: font.key(),
        }
    );
    assert_eq!(
        match_family(CjkFamily::SimSun, &available),
        FontMatch::Missing {
            family: CjkFamily::SimSun
        }
    );

    let theme = EastAsiaTheme {
        major: Some(CjkFamily::SimHei),
        minor: Some(CjkFamily::DengXian),
    };
    assert_eq!(
        theme_east_asia(ThemeFont::MinorEastAsia, theme),
        Ok(CjkFamily::DengXian)
    );
    assert_eq!(
        theme_east_asia(ThemeFont::MajorEastAsia, theme),
        Ok(CjkFamily::SimHei)
    );
    assert_eq!(
        theme_east_asia(ThemeFont::MinorAscii, theme),
        Err(CjkIssue::NotEastAsiaSlot)
    );
    assert_eq!(
        theme_east_asia(ThemeFont::MinorEastAsia, EastAsiaTheme::default()),
        Err(CjkIssue::MissingFont)
    );
}
