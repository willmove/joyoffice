//! Build dependency edges at the cell where a formula is used. Workbook structural
//! edits rebuild these edges; volatile functions still run on every calculation.
use super::{
    named_refs::{Target, name_target, table_area},
    special::{is_special, resizes_sum_range},
    *,
};

pub(crate) fn cell_precedents(wb: &Workbook, sheet: usize, at: Option<CellRef>, expr: &Expr) -> (Vec<Area>, bool, bool) {
    let mut scanner = Scanner { wb, at, expansions: 0, dynamic: false, runtime: false };
    let mut areas = Vec::new();
    scanner.scan(sheet, expr, 0, &mut areas);
    (areas, scanner.dynamic, scanner.runtime)
}

struct Scanner<'a> {
    wb: &'a Workbook,
    at: Option<CellRef>,
    expansions: usize,
    dynamic: bool,
    runtime: bool,
}

impl Scanner<'_> {
    fn add_reference(&self, sheet: usize, reference: &Reference, range: RangeRef, out: &mut Vec<Area>) {
        let span = match &reference.sheet {
            SheetSel::Current => Some((sheet, sheet)),
            SheetSel::Named(name) => self.wb.sheet_index(name).map(|i| (i, i)),
            SheetSel::Span(first, last) => self.wb.sheet_index(first).zip(self.wb.sheet_index(last)),
        };
        if let Some((a, b)) = span {
            out.extend((a.min(b)..=a.max(b)).map(|sheet| Area { sheet, range }));
        }
    }

    fn expand(&mut self, depth: usize) -> bool {
        // A cyclic name, a branching alias chain or nested range endpoints can
        // otherwise re-expand a small input indefinitely. Unresolved formulas
        // keep the conservative schedule.
        self.expansions += 1;
        if depth >= 16 || self.expansions > 128 {
            self.dynamic = true;
            return false;
        }
        true
    }

    fn scan(&mut self, sheet: usize, expr: &Expr, depth: usize, out: &mut Vec<Area>) {
        expr.walk(&mut |node| match node {
            Expr::Ref(reference) => self.add_reference(sheet, reference, reference.range(), out),
            Expr::Struct(reference) => match self.at.and_then(|at| table_area(self.wb, sheet, at, reference).ok()) {
                Some(area) => out.push(area),
                None => self.dynamic = true,
            },
            Expr::Name(name) => {
                let Some(at) = self.at else {
                    self.dynamic = true;
                    return;
                };
                if self.expand(depth) {
                    match name_target(self.wb, sheet, at, name) {
                        Some(Target::Table(area)) => out.push(area),
                        Some(Target::Expression { sheet, expr }) => self.scan(sheet, &expr, depth + 1, out),
                        None => self.dynamic = true,
                    }
                }
            }
            Expr::Call(name, args) if resizes_sum_range(name, args.len()) => match (args.first(), args.get(2)) {
                (Some(Expr::Ref(criteria)), Some(Expr::Ref(sum))) => {
                    let range = criteria.range();
                    self.add_reference(sheet, sum, resized_sum_range(sum.range(), range.height(), range.width()), out);
                }
                (_, Some(Expr::Number(_) | Expr::Text(_) | Expr::Bool(_) | Expr::Error(_) | Expr::Array(_) | Expr::Missing)) => {}
                _ => self.dynamic = true,
            },
            Expr::Call(name, _)
                if matches!(name.as_str(), "INDIRECT" | "OFFSET" | "NOW" | "TODAY" | "RAND" | "RANDBETWEEN" | "RANDARRAY" | "CELL" | "INFO") =>
            {
                self.dynamic = true;
            }
            Expr::Call(name, _) if database_function(name) => self.runtime = true,
            Expr::Call(name, _) if joysheet_functions::lookup(name).is_none() && !is_special(name) => self.dynamic = true,
            // A lambda or LET can supply a reference through its lexical scope.
            Expr::Call(name, _) if self.at.is_some() && matches!(name.as_str(), "LET" | "LAMBDA") => self.dynamic = true,
            Expr::Invoke(_, _) if self.at.is_some() => self.dynamic = true,
            Expr::Binary(BinOp::Range, a, b) if !matches!((&**a, &**b), (Expr::Ref(_), Expr::Ref(_))) && self.expand(depth) => {
                let mut endpoints = Vec::new();
                self.scan(sheet, a, depth + 1, &mut endpoints);
                self.scan(sheet, b, depth + 1, &mut endpoints);
                let mut bounds: Vec<Area> = Vec::new();
                for area in endpoints {
                    if let Some(previous) = bounds.iter_mut().find(|item| item.sheet == area.sheet) {
                        previous.range = previous.range.union(&area.range);
                    } else {
                        bounds.push(area);
                    }
                }
                out.extend(bounds);
            }
            _ => {}
        });
    }
}

fn database_function(name: &str) -> bool {
    name.starts_with('D') && joysheet_functions::lookup(name).is_some_and(|spec| spec.category == joysheet_functions::Category::Database)
}

/// Only functions drawing their cell data from evaluated reference arguments
/// can use the selective spill follow-up. Metadata queries, externally supplied
/// handlers and synthesized criteria ranges keep the general schedule.
pub(super) fn can_trace(node: &Expr) -> bool {
    match node {
        Expr::Call(name, _) => {
            database_function(name)
                || matches!(
                    name.as_str(),
                    "INDIRECT"
                        | "OFFSET"
                        | "INDEX"
                        | "IF"
                        | "IFERROR"
                        | "IFNA"
                        | "SUM"
                        | "AVERAGE"
                        | "MIN"
                        | "MAX"
                        | "COUNT"
                        | "COUNTA"
                        | "ROWS"
                        | "COLUMNS"
                        | "ROW"
                        | "COLUMN"
                        | "AREAS"
                        | "ISREF"
                        | "RAND"
                        | "NOW"
                        | "TODAY"
                )
        }
        Expr::Invoke(_, _) => false,
        _ => true,
    }
}

/// Retain precise rectangular reads. Adjacent records of a calculated database
/// criterion normally touch vertically and therefore share a single interval.
pub(super) fn observe(tracked: &mut Option<Vec<Area>>, incoming: &[Area]) {
    let Some(areas) = tracked else { return };
    for &area in incoming {
        if let Some(existing) = areas.iter_mut().find(|existing| {
            if existing.sheet != area.sheet {
                return false;
            }
            let (a, b) = (existing.range, area.range);
            (a.contains(b.start) && a.contains(b.end))
                || (b.contains(a.start) && b.contains(a.end))
                || (a.start.col == b.start.col
                    && a.end.col == b.end.col
                    && a.start.row <= b.end.row.saturating_add(1)
                    && b.start.row <= a.end.row.saturating_add(1))
                || (a.start.row == b.start.row
                    && a.end.row == b.end.row
                    && a.start.col <= b.end.col.saturating_add(1)
                    && b.start.col <= a.end.col.saturating_add(1))
        }) {
            existing.range = existing.range.union(&area.range);
        } else if areas.len() < 32 {
            areas.push(area);
        } else {
            *tracked = None;
            return;
        }
    }
}

pub(super) fn criterion_reference(wb: &Workbook, current: usize, database: Area, reference: &Reference) -> bool {
    let in_record = |a: &joysheet_formula::Anchor| {
        a.row_abs
            || (a.row > database.range.start.row
                && database.range.contains(a.cell())
                && match &reference.sheet {
                    SheetSel::Current => current == database.sheet,
                    SheetSel::Named(name) => wb.sheet_index(name) == Some(database.sheet),
                    SheetSel::Span(_, _) => false,
                })
    };
    match &reference.kind {
        RefKind::Cell(a) => in_record(a),
        RefKind::Range(a, b) => in_record(a) && in_record(b),
        RefKind::Rows(..) | RefKind::Cols(..) => true,
    }
}
