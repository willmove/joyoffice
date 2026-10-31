//! Dependency graph and recalculation.
//!
//! The graph maps every formula cell to its static precedents (resolved areas). A reverse index
//! answers "which formulas read this cell?": single-cell precedents in a hash map, ranges as row
//! intervals per column (wide ranges per sheet), each an interval index so a lookup costs
//! O(log n + matches) however many ranges share the column. After an edit, the dirty set is the
//! transitive closure of dependents; it is evaluated in topological order, with on-demand
//! evaluation as a safety net for dynamic references. Cycles produce `#CIRC!` unless iterative
//! calculation is on: then a formula read while it is being evaluated gives its previous value,
//! and the formulas that read one are evaluated again until the loop settles
//! (`PassHost::iterate_loops`).

use std::collections::{BTreeSet, VecDeque};
use std::hash::{BuildHasherDefault, Hasher};

use joysheet_core::{Array, CellError, CellRef, RangeRef, Value};
use joysheet_formula::Expr;
use joysheet_model::{CalcMode, Workbook};

use crate::eval::{Area, Evaluator, Host, precedents, stack_position};

mod graph;
mod pass_host;

#[cfg(test)]
use graph::Intervals;
pub use graph::{FxHasher, Graph, Key};
use graph::{HashMap, HashSet, Node};
use pass_host::{PassHost, spills_as_before};

/// Calculation state kept beside a workbook.
#[derive(Clone, Debug)]
pub struct Calc {
    pub graph: Graph,
    rng: u64,
    /// Fixed "now" for deterministic tests; `None` = system clock.
    pub fixed_now: Option<f64>,
    /// Cells whose formulas have errors from cycles.
    pub circular: Vec<Key>,
    pub last_recalc_cells: usize,
}

impl Default for Calc {
    fn default() -> Self {
        Calc { graph: Graph::default(), rng: 0x9E37_79B9_7F4A_7C15, fixed_now: None, circular: vec![], last_recalc_cells: 0 }
    }
}

/// Pops one finished component off Tarjan's stack, labelling its nodes with `comp_index`. The
/// component is everything up to and including `root`.
fn pop_component(stack: &mut Vec<usize>, on_stack: &mut [bool], comp: &mut [usize], comp_index: usize, root: usize) {
    while let Some(w) = stack.pop() {
        on_stack[w] = false;
        comp[w] = comp_index;
        if w == root {
            break;
        }
    }
}

/// Strongly connected components of a graph given as adjacency lists (Tarjan, iterative): each
/// node's component index, and the number of nodes in each component.
fn components(edges: &[Vec<usize>]) -> (Vec<usize>, Vec<usize>) {
    let n = edges.len();
    let (mut low, mut num) = (vec![0usize; n], vec![usize::MAX; n]);
    let (mut on_stack, mut stack, mut comp) = (vec![false; n], Vec::new(), vec![usize::MAX; n]);
    let (mut next, mut comps) = (0usize, 0usize);
    for root in 0..n {
        if num[root] != usize::MAX {
            continue;
        }
        let mut call: Vec<(usize, usize)> = vec![(root, 0)];
        num[root] = next;
        low[root] = next;
        next += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(top) = call.last_mut() {
            let v = top.0;
            if let Some(&w) = edges.get(v).and_then(|e| e.get(top.1)) {
                top.1 += 1;
                if num[w] == usize::MAX {
                    num[w] = next;
                    low[w] = next;
                    next += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    call.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(num[w]);
                }
                continue;
            }
            call.pop();
            if let Some(&(u, _)) = call.last() {
                low[u] = low[u].min(low[v]);
            }
            if low[v] == num[v] {
                pop_component(&mut stack, &mut on_stack, &mut comp, comps, v);
                comps += 1;
            }
        }
    }
    let mut size = vec![0usize; comps];
    for &c in &comp {
        if let Some(s) = size.get_mut(c) {
            *s += 1;
        }
    }
    (comp, size)
}

impl Calc {
    pub fn new() -> Self {
        Self::default()
    }

    fn now(&self) -> f64 {
        if let Some(n) = self.fixed_now {
            return n;
        }
        now_serial()
    }

    /// Rebuilds the graph from scratch.
    pub fn rebuild(&mut self, wb: &Workbook) {
        let _profile = crate::profile::scope(crate::profile::Phase::Dependency);
        self.graph = Graph::default();
        let mut circ = Vec::new();
        for (si, sheet) in wb.sheets.iter().enumerate() {
            for (c, cell) in sheet.cells.formulas() {
                if let Some(f) = &cell.formula
                    && let Some(e) = f.expr()
                {
                    let (areas, dynamic) = precedents(wb, si, &e);
                    self.graph.insert((si, c), Node { areas, dynamic });
                    if cell.value == Value::Error(CellError::Circ) {
                        circ.push((si, c));
                    }
                }
            }
        }
        self.graph.settle();
        self.circular = if wb.calc.iterative { Vec::new() } else { self.loops_among(&circ) };
    }

    /// The circular references of a rebuilt workbook (after undo, redo or a rollback, which put
    /// back cells without calculating them), from the formulas whose cached value is `#CIRC!`:
    /// the listed cells still on a loop, and one cell of each loop none of them is on.
    fn loops_among(&self, circ: &[Key]) -> Vec<Key> {
        if circ.is_empty() {
            return Vec::new();
        }
        let n = circ.len();
        let index: HashMap<Key, usize> = circ.iter().enumerate().map(|(i, k)| (*k, i)).collect();
        // Edges from a cell to the `#CIRC!` formulas that read it; loops are the same either way.
        let mut deps = Vec::new();
        let edges: Vec<Vec<usize>> = circ
            .iter()
            .map(|k| {
                deps.clear();
                self.graph.dependents(k.0, k.1, &mut deps);
                let mut e: Vec<usize> = deps.iter().filter_map(|d| index.get(d).copied()).collect();
                e.sort_unstable();
                e.dedup();
                e
            })
            .collect();
        // Loop members: nodes in a component of more than one, or with an edge to themselves.
        let (comp, size) = components(&edges);
        let on_loop = |i: usize| comp.get(i).and_then(|c| size.get(*c)).is_some_and(|s| *s > 1) || edges.get(i).is_some_and(|e| e.contains(&i));
        // A formula with dynamic references (INDIRECT, OFFSET) may close a loop the graph can't see.
        let mut out: Vec<Key> =
            self.circular.iter().copied().filter(|k| index.get(k).is_some_and(|&i| on_loop(i) || self.graph.dynamic.contains(k))).collect();
        let mut covered: HashSet<usize> = out.iter().filter_map(|k| index.get(k)).filter_map(|&i| comp.get(i).copied()).collect();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&i| circ.get(i).map(|(s, c)| (*s, c.row, c.col)));
        for i in order {
            if let (Some(&c), Some(&k)) = (comp.get(i), circ.get(i))
                && on_loop(i)
                && covered.insert(c)
            {
                out.push(k);
            }
        }
        out
    }

    /// Recalculates every formula (F9 / Ctrl+Alt+F9, and after loading).
    pub fn recalc_all(&mut self, wb: &mut Workbook) {
        let _profile = crate::profile::scope(crate::profile::Phase::Recalc);
        self.rebuild(wb);
        // Spills are laid out again from the formulas where they are now (a structural edit moves
        // or deletes the formulas, not the values they spilled).
        for i in 0..wb.sheets.len() {
            if wb.sheet(i).is_some_and(|s| !s.spill_ranges.is_empty() || !s.spill.is_empty())
                && let Some(sh) = wb.sheet_mut(i)
            {
                sh.spill.clear();
                sh.spill_ranges.clear();
            }
        }
        let all: Vec<Key> = self.graph.nodes.keys().copied().collect();
        self.run(wb, all, true);
    }

    /// Updates the graph for cells whose content changed and recalculates what depends on them.
    pub fn cells_changed(&mut self, wb: &mut Workbook, changed: &[Key]) {
        let _profile = crate::profile::scope(crate::profile::Phase::Recalc);
        let dependency_profile = crate::profile::scope(crate::profile::Phase::Dependency);
        let mut cleared: Vec<(usize, RangeRef)> = Vec::new();
        for &k in changed {
            let formula = wb.sheet(k.0).and_then(|s| s.cell(k.1)).and_then(|c| c.formula.clone());
            match formula.and_then(|f| f.expr()) {
                Some(e) => {
                    let (areas, dynamic) = precedents(wb, k.0, &e);
                    self.graph.insert(k, Node { areas, dynamic });
                }
                None => {
                    self.graph.remove(k);
                    // A formula replaced by a constant (or cleared) takes its spilled values with it.
                    if let Some(sh) = wb.sheet_mut(k.0)
                        && let Some(r) = sh.spill_ranges.remove(&k.1)
                    {
                        for c in r.iter() {
                            sh.spill.remove(&c);
                        }
                        cleared.push((k.0, r));
                    }
                    // A formula that doesn't parse evaluates to #NAME?.
                    if let Some(cell) = wb.sheet_mut(k.0).and_then(|s| s.cells.get_mut(k.1))
                        && cell.formula.is_some()
                    {
                        cell.value = Value::Error(CellError::Name);
                    }
                }
            }
        }
        self.graph.settle();
        drop(dependency_profile);
        if wb.calc.mode == CalcMode::Manual {
            // Only the edited formulas themselves are evaluated.
            let own: Vec<Key> = changed.iter().copied().filter(|k| self.graph.nodes.contains_key(k)).collect();
            self.run(wb, own, false);
            return;
        }
        let spill_seed_profile = crate::profile::scope(crate::profile::Phase::SpillSeed);
        let mut seeds: Vec<Key> = changed.to_vec();
        let mut emptied: Vec<Key> = Vec::new();
        // Typing into (or clearing) a cell a dynamic array spills over (or would spill over)
        // re-evaluates the anchor, which may now be blocked or unblocked.
        for &k in changed {
            if let Some(sh) = wb.sheet(k.0) {
                for (anchor, r) in &sh.spill_ranges {
                    if *anchor != k.1 && r.contains(k.1) {
                        seeds.push((k.0, *anchor));
                    }
                }
                for (c, cell) in sh.cells.iter_range(RangeRef::new(CellRef::new(k.1.row.saturating_sub(64), k.1.col.saturating_sub(64)), k.1)) {
                    if c != k.1 && cell.value == Value::Error(CellError::Spill) {
                        seeds.push((k.0, c));
                    }
                }
                if !sh.cells.has(k.1) {
                    match emptied.iter_mut().find(|x| x.0 == k.0) {
                        Some(x) => x.1 = CellRef::new(x.1.row.max(k.1.row), x.1.col.max(k.1.col)),
                        None => emptied.push(k),
                    }
                }
            }
        }
        // An emptied cell can unblock an anchor any distance above or to the left of it.
        for (si, end) in emptied {
            for &(s, c) in self.graph.nodes.keys() {
                if s == si && c.row <= end.row && c.col <= end.col && wb.sheet(s).is_some_and(|sh| sh.value(c) == Value::Error(CellError::Spill)) {
                    seeds.push((s, c));
                }
            }
        }
        // Spill areas of changed anchors also change.
        for &k in changed {
            if let Some(r) = wb.sheet(k.0).and_then(|s| s.spill_ranges.get(&k.1)) {
                self.seed_range(wb, k.0, *r, &mut seeds);
            }
        }
        for (si, r) in &cleared {
            self.seed_range(wb, *si, *r, &mut seeds);
        }
        drop(spill_seed_profile);
        let t0 = prof_now();
        let dirty = self.dirty_closure(wb, &seeds);
        let t1 = prof_now();
        let n = dirty.len();
        self.run(wb, dirty, false);
        if std::env::var_os("GRIDCRAFT_PROFILE").is_some() {
            eprintln!("recalc: {} seeds, closure {} cells {:.1} ms, run {:.1} ms", seeds.len(), n, t1 - t0, prof_now() - t1);
        }
    }

    /// Equivalent to seeding every coordinate of a spill, without allocating/walking its area.
    /// Include formulas inside it as well as readers (a newly typed formula may block the spill).
    fn seed_range(&self, wb: &Workbook, sheet: usize, range: RangeRef, out: &mut Vec<Key>) {
        self.graph.dependents_of_range(sheet, range, out);
        if let Some(sh) = wb.sheet(sheet) {
            out.extend(sh.cells.iter_formulas(range).map(|(c, _)| (sheet, c)));
        }
    }

    fn dirty_closure(&self, _wb: &Workbook, seeds: &[Key]) -> Vec<Key> {
        let _profile = crate::profile::scope(crate::profile::Phase::Dependency);
        let mut seen: HashSet<Key> = HashSet::default();
        let mut queue: VecDeque<Key> = VecDeque::new();
        for k in seeds {
            if self.graph.nodes.contains_key(k) && seen.insert(*k) {
                queue.push_back(*k);
            }
            let mut deps = Vec::new();
            self.graph.dependents(k.0, k.1, &mut deps);
            for d in deps {
                if seen.insert(d) {
                    queue.push_back(d);
                }
            }
        }
        for k in &self.graph.dynamic {
            if seen.insert(*k) {
                queue.push_back(*k);
            }
        }
        let mut deps = Vec::new();
        while let Some(k) = queue.pop_front() {
            deps.clear();
            self.graph.dependents(k.0, k.1, &mut deps);
            for d in deps.drain(..) {
                if seen.insert(d) {
                    queue.push_back(d);
                }
            }
        }
        seen.into_iter().collect()
    }

    /// Evaluates the dirty set and writes results (and spills) back.
    fn run(&mut self, wb: &mut Workbook, mut dirty: Vec<Key>, full: bool) {
        let mut dynamic_again = !self.graph.dynamic.is_empty();
        self.last_recalc_cells = 0;
        for _round in 0..8 {
            if dirty.is_empty() {
                break;
            }
            let preparation_profile = crate::profile::scope(crate::profile::Phase::Preparation);
            // Row-major order is a good topological guess; on-demand evaluation fixes the rest.
            dirty.sort_by_key(|(s, c)| (*s, c.row, c.col));
            let mut exprs: HashMap<Key, std::sync::Arc<Expr>> = HashMap::with_capacity_and_hasher(dirty.len(), Default::default());
            for k in &dirty {
                if let Some(e) = wb.sheet(k.0).and_then(|s| s.cell(k.1)).and_then(|c| c.formula.as_ref()).and_then(|f| f.expr_arc()) {
                    exprs.insert(*k, e);
                }
            }
            drop(preparation_profile);
            let now = self.now();
            let mut rng = self.rng;
            let tp = prof_now();
            let evaluation_profile = crate::profile::scope(crate::profile::Phase::Evaluation);
            let (mut results, spills, cycles) = {
                let mut host = PassHost::new(wb, &exprs, &mut rng, now);
                host.iterate = wb.calc.iterative;
                for k in &dirty {
                    host.calculate(*k);
                }
                if host.iterate && !host.members.is_empty() {
                    let passes = host.iterate_loops(&dirty, wb.calc.max_iterations, wb.calc.max_change);
                    if std::env::var_os("GRIDCRAFT_PROFILE").is_some() {
                        eprintln!("  iterative calculation: {} passes over {} formulas", passes, host.tainted.len());
                    }
                }
                (host.results, host.spills, host.cycles)
            };
            self.rng = rng;
            drop(evaluation_profile);
            crate::profile::formulas(results.len());
            if std::env::var_os("GRIDCRAFT_PROFILE").is_some() {
                eprintln!("  eval {} formulas {:.1} ms", results.len(), prof_now() - tp);
            }
            let tw = prof_now();
            let writeback_profile = crate::profile::scope(crate::profile::Phase::Writeback);
            self.last_recalc_cells += results.len();
            // The loops this round finds replace the ones listed for the formulas it evaluated.
            if !self.circular.is_empty() {
                let evaluated: HashSet<Key> = dirty.iter().copied().collect();
                self.circular.retain(|k| !evaluated.contains(k));
            }
            self.circular.extend(cycles);
            // Write back.
            let mut spill_changes: Vec<Key> = Vec::new();
            let mut freed: Vec<(usize, RangeRef)> = Vec::new();
            for group in dirty.chunk_by(|a, b| a.0 == b.0) {
                if let Some(first) = group.first()
                    && let Some(sheet) = wb.sheet_mut(first.0)
                {
                    sheet.cells.write_values(group.iter().filter_map(|k| results.remove(k).map(|v| (k.1, v))));
                }
            }
            for k in &dirty {
                let new = spills.get(k);
                let Some(sheet) = wb.sheet_mut(k.0) else { continue };
                let old = sheet.spill_ranges.get(&k.1).copied();
                if old.is_none() && new.is_none() {
                    continue;
                }
                // An array that spills exactly as before changes nothing its dependents read, so
                // they aren't queued again. Otherwise two arrays that read each other's spill areas
                // queue each other in every round, up to the round limit.
                if let (Some(r), Some(arr)) = (old, new)
                    && spills_as_before(sheet, k.1, r, arr)
                {
                    continue;
                }
                if let Some(r) = old {
                    for c in r.iter() {
                        sheet.spill.remove(&c);
                    }
                    sheet.spill_ranges.remove(&k.1);
                }
                let mut changed_range = old;
                if let Some(arr) = new {
                    let end = CellRef::new(k.1.row.saturating_add(arr.rows as u32 - 1), k.1.col.saturating_add(arr.cols as u32 - 1));
                    let range = RangeRef::new(k.1, end);
                    let blocked = end.row >= joysheet_core::MAX_ROWS
                        || end.col >= joysheet_core::MAX_COLS
                        || range.iter().any(|c| c != k.1 && (sheet.cells.has(c) || sheet.spill.contains_key(&c)))
                        || sheet.merges.iter().any(|m| m.intersects(&range))
                        || sheet.tables.iter().any(|t| t.range.contains(k.1));
                    if blocked {
                        if let Some(cell) = sheet.cells.get_mut(k.1) {
                            cell.value = Value::Error(CellError::Spill);
                        }
                        // Not blocked while the pass ran (it would have been #SPILL! there), so
                        // blocked by another array of this pass: the formulas that read this one,
                        // at its anchor or over the area it would have spilled into, read #SPILL!
                        // or empty cells now.
                        let mut deps = Vec::new();
                        self.graph.dependents_of_range(k.0, range, &mut deps);
                        spill_changes.extend(deps.into_iter().filter(|d| d != k));
                    } else {
                        for (i, c) in range.iter().enumerate() {
                            if c != k.1 {
                                sheet.spill.insert(c, arr.data.get(i).cloned().unwrap_or_default());
                            }
                        }
                        sheet.spill_ranges.insert(k.1, range);
                        changed_range = Some(changed_range.map_or(range, |o| o.union(&range)));
                    }
                }
                if let Some(r) = changed_range {
                    let mut deps = Vec::new();
                    self.graph.dependents_of_range(k.0, r, &mut deps);
                    // Not the anchor itself (that would read its own array), unless its old array
                    // is gone or partly freed: its new formula may have read the old values, still
                    // laid out while it was evaluated.
                    let freed = match (old, sheet.spill_ranges.get(&k.1)) {
                        (Some(o), Some(n)) => !(n.contains(o.start) && n.contains(o.end)),
                        (_, None) => true,
                        (None, Some(_)) => false,
                    };
                    spill_changes.extend(deps.into_iter().filter(|d| d != k || freed));
                }
                if let Some(o) = old
                    && !sheet.spill_ranges.get(&k.1).is_some_and(|n| n.contains(o.start) && n.contains(o.end))
                {
                    freed.push((k.0, o));
                }
            }
            // Formulas with references only known while evaluating (INDIRECT, OFFSET) may have read
            // a spill area before it was laid out: they are evaluated once more.
            if dynamic_again && !spills.is_empty() {
                dynamic_again = false;
                spill_changes.extend(self.graph.dynamic.iter().copied());
            }
            // An area a formula no longer spills over may unblock another formula's array.
            for (si, o) in freed {
                for &(s, c) in self.graph.nodes.keys() {
                    if s == si
                        && c.row <= o.end.row
                        && c.col <= o.end.col
                        && wb.sheet(s).is_some_and(|sh| sh.value(c) == Value::Error(CellError::Spill))
                    {
                        spill_changes.push((s, c));
                    }
                }
            }
            if full {
                // Everything was already evaluated once; spill dependents need another pass.
            }
            if std::env::var_os("GRIDCRAFT_PROFILE").is_some() {
                eprintln!("  write-back {:.1} ms", prof_now() - tw);
            }
            spill_changes.sort_by_key(|(s, c)| (*s, c.row, c.col));
            drop(writeback_profile);
            spill_changes.dedup();
            if spill_changes.is_empty() {
                break;
            }
            dirty = self.dirty_closure(wb, &spill_changes);
        }
        // Loops among formulas the run did not evaluate stay listed while they are still formulas.
        // Iterative calculation reports none.
        if wb.calc.iterative {
            self.circular.clear();
        } else {
            self.circular.retain(|k| self.graph.nodes.contains_key(k));
        }
    }
}

/// Current date-time as a serial (1900 system), from the system clock (browser clock on wasm).
/// UTC; the local offset isn't available without platform calls. Falls back to a fixed date only
/// when no clock is available.
pub fn now_serial() -> f64 {
    joysheet_core::date::unix_now_ms().map_or(46302.5, joysheet_core::date::serial_from_unix_ms)
}

/// Evaluates a formula in the context of a cell without storing it (conditional formats, data
/// validation, Evaluate Formula, Name Manager previews, the CLI `eval`).
pub fn evaluate(wb: &Workbook, sheet: usize, at: CellRef, formula: &str) -> Value {
    let body = formula.strip_prefix('=').unwrap_or(formula);
    let Ok(expr) = joysheet_formula::parse(body) else { return Value::Error(CellError::Name) };
    evaluate_expr(wb, sheet, at, &expr)
}

pub fn evaluate_expr(wb: &Workbook, sheet: usize, at: CellRef, expr: &Expr) -> Value {
    let exprs = HashMap::default();
    let mut rng = 0x1234_5678_9ABC_DEF0u64;
    let mut host = PassHost::new(wb, &exprs, &mut rng, now_serial());
    let mut ev = Evaluator::new(&mut host, sheet, at);
    ev.value(expr)
}

#[allow(clippy::disallowed_methods)] // the clock is read only off wasm
fn prof_now() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        0.0
    }
}

#[cfg(test)]
#[path = "tests/recalc_tests.rs"]
pub(crate) mod tests;
