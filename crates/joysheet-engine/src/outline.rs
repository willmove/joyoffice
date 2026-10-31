//! Outline groups (Data ▸ Group) of a sheet's rows or columns: the runs of lines at or above
//! each outline level, as the grid draws them (brackets, − / + buttons, level buttons) and the
//! Show / Hide Detail commands act on them. Summary rows sit below their detail and summary
//! columns right of it, or above / left of it when the sheet's outline settings say so
//! ([`Sheet::summary_below`], [`Sheet::summary_right`]).

use std::collections::BTreeMap;

use joysheet_model::{LineInfo, Sheet};

/// The deepest outline level XLSX allows.
pub const MAX_LEVEL: u8 = 7;

/// One group: lines `start..=end` at outline level `level` or deeper, with its summary line
/// just after `end`, or just before `start` when `summary_before` is set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutlineGroup {
    pub level: u8,
    pub start: u32,
    pub end: u32,
    /// Every detail line is hidden: the button shows +.
    pub collapsed: bool,
    /// The summary line is above (rows) or left of (columns) the detail.
    pub summary_before: bool,
}

impl OutlineGroup {
    /// The summary line, where the − / + button sits.
    pub fn summary(&self) -> u32 {
        if self.summary_before { self.start.saturating_sub(1) } else { self.end.saturating_add(1) }
    }
    /// Whether the summary line exists: a group that starts at the first line (summary before)
    /// or ends at the last one (summary after) has none.
    pub fn has_summary(&self) -> bool {
        !self.contains(self.summary())
    }
    pub fn contains(&self, line: u32) -> bool {
        line >= self.start && line <= self.end
    }
}

/// Whether the sheet's summary rows (`rows`) or summary columns sit before their detail.
pub fn summary_before(sh: &Sheet, rows: bool) -> bool {
    if rows { !sh.summary_below } else { !sh.summary_right }
}

/// The deepest outline level used by these lines (0 = no outline).
pub fn max_level(lines: &BTreeMap<u32, LineInfo>) -> u8 {
    lines.values().map(|l| l.outline.min(MAX_LEVEL)).max().unwrap_or(0)
}

/// Every group, with its summary line after the detail; see [`groups_with`].
pub fn groups(lines: &BTreeMap<u32, LineInfo>) -> Vec<OutlineGroup> {
    groups_with(lines, false)
}

/// Every group, ordered by where it ends (inner groups before the group around them), with its
/// summary line before the detail when `summary_before` is set.
pub fn groups_with(lines: &BTreeMap<u32, LineInfo>, summary_before: bool) -> Vec<OutlineGroup> {
    let mut out = Vec::new();
    // Open group per level (index level - 1): start line and whether all its lines are hidden.
    let mut open: [Option<(u32, bool)>; MAX_LEVEL as usize] = [None; MAX_LEVEL as usize];
    let mut prev: Option<u32> = None;
    let close_from = |open: &mut [Option<(u32, bool)>; MAX_LEVEL as usize], from: u8, end: u32, out: &mut Vec<OutlineGroup>| {
        for level in (from..=MAX_LEVEL).rev() {
            if let Some(slot) = open.get_mut(level as usize - 1)
                && let Some((start, hidden)) = slot.take()
            {
                out.push(OutlineGroup { level, start, end, collapsed: hidden, summary_before });
            }
        }
    };
    for (&i, info) in lines.iter().filter(|(_, l)| l.outline > 0) {
        let level = info.outline.min(MAX_LEVEL);
        match prev {
            // A gap ends every group.
            Some(p) if p.checked_add(1) != Some(i) => close_from(&mut open, 1, p, &mut out),
            Some(p) if level < MAX_LEVEL => close_from(&mut open, level + 1, p, &mut out),
            _ => {}
        }
        for slot in open.iter_mut().take(level as usize) {
            match slot {
                Some((_, hidden)) => *hidden &= info.hidden,
                None => *slot = Some((i, info.hidden)),
            }
        }
        prev = Some(i);
    }
    if let Some(p) = prev {
        close_from(&mut open, 1, p, &mut out);
    }
    out
}

/// The group at `level` that holds `line`.
pub fn group_at(lines: &BTreeMap<u32, LineInfo>, line: u32, level: u8) -> Option<OutlineGroup> {
    groups(lines).into_iter().find(|g| g.level == level && g.contains(line))
}

#[cfg(test)]
#[path = "tests/outline_tests.rs"]
mod tests;
