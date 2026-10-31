use super::*;

fn lines(spec: &[(u32, u8, bool)]) -> BTreeMap<u32, LineInfo> {
    spec.iter().map(|&(i, outline, hidden)| (i, LineInfo { outline, hidden, ..Default::default() })).collect()
}

#[test]
fn no_outline_has_no_groups() {
    let l = lines(&[(3, 0, true)]);
    assert_eq!(max_level(&l), 0);
    assert!(groups(&l).is_empty());
}

#[test]
fn nested_and_separate_groups() {
    // Rows 5..8 at level 1 with 6..7 at level 2; rows 12..13 at level 1 after a gap.
    let l = lines(&[(4, 1, false), (5, 2, false), (6, 2, false), (7, 1, false), (11, 1, true), (12, 1, true)]);
    assert_eq!(max_level(&l), 2);
    let g = groups(&l);
    assert_eq!(
        g,
        vec![
            OutlineGroup { level: 2, start: 5, end: 6, collapsed: false, summary_before: false },
            OutlineGroup { level: 1, start: 4, end: 7, collapsed: false, summary_before: false },
            OutlineGroup { level: 1, start: 11, end: 12, collapsed: true, summary_before: false },
        ]
    );
    assert_eq!(g[1].summary(), 8);
    assert_eq!(group_at(&l, 6, 1).map(|g| g.start), Some(4));
    assert_eq!(group_at(&l, 4, 2), None);
}

#[test]
fn adjacent_groups_at_one_level_split_by_a_shallower_line() {
    // Two level-2 groups inside one level-1 group, separated by a level-1 line.
    let l = lines(&[(0, 2, true), (1, 2, true), (2, 1, false), (3, 2, false)]);
    let g = groups(&l);
    assert_eq!(g.iter().filter(|g| g.level == 2).count(), 2);
    assert_eq!(g.iter().find(|g| g.level == 2 && g.start == 0).map(|g| g.collapsed), Some(true));
    assert_eq!(g.iter().find(|g| g.level == 1).map(|g| (g.start, g.end, g.collapsed)), Some((0, 3, false)));
}

#[test]
fn deepest_level_and_last_line_do_not_overflow() {
    let l = lines(&[(u32::MAX - 1, 9, false), (u32::MAX, 7, false)]);
    let g = groups(&l);
    assert_eq!(g.len(), 7);
    assert!(g.iter().all(|g| g.start == u32::MAX - 1 && g.end == u32::MAX));
    assert_eq!(g[0].summary(), u32::MAX);
    assert!(!g[0].has_summary());
}

#[test]
fn summary_before_the_detail() {
    // Rows 5..8 at level 1 with 6..7 at level 2: the summaries are rows 4 and 5.
    let l = lines(&[(4, 1, false), (5, 2, false), (6, 2, false), (7, 1, false), (0, 1, false)]);
    let g = groups_with(&l, true);
    let at = |level: u8, start: u32| g.iter().find(|g| g.level == level && g.start == start).copied();
    assert_eq!(at(1, 4).map(|g| (g.summary(), g.has_summary())), Some((3, true)));
    assert_eq!(at(2, 5).map(|g| (g.summary(), g.has_summary())), Some((4, true)));
    // A group on the first line has no line before it.
    assert_eq!(at(1, 0).map(|g| (g.summary(), g.has_summary())), Some((0, false)));
    let mut sh = Sheet::new("S");
    assert!(!summary_before(&sh, true) && !summary_before(&sh, false));
    sh.summary_below = false;
    assert!(summary_before(&sh, true) && !summary_before(&sh, false));
}
