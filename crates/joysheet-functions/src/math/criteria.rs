//! Single-pass conditional worksheet aggregates (`COUNTIF`, `COUNTIFS`, `SUMIF`, `SUMIFS`,
//! `AVERAGEIF`, `AVERAGEIFS`, `MAXIFS`, `MINIFS`) using pre-compiled criteria and constant-space
//! accumulators.

use joysheet_core::{CellError, DateSystem, Value};

use crate::criteria::Criterion;
use crate::util::{R, arg, has, num_val};
use crate::{Arg, Ctx};

#[derive(Clone, Copy)]
struct GridView<'a> {
    rows: usize,
    cols: usize,
    cells: &'a [Value],
}

impl<'a> GridView<'a> {
    fn from_value(v: &'a Value) -> Self {
        match v {
            Value::Array(arr) => Self { rows: arr.rows, cols: arr.cols, cells: arr.data.as_slice() },
            other => Self { rows: 1, cols: 1, cells: std::slice::from_ref(other) },
        }
    }

    #[inline]
    fn len(self) -> usize {
        self.rows.saturating_mul(self.cols)
    }

    #[inline]
    fn get_rc(self, r: usize, c: usize) -> Option<&'a Value> {
        if r < self.rows && c < self.cols { self.cells.get(r * self.cols + c) } else { None }
    }
}

#[derive(Clone, Copy)]
enum Aggregate {
    Sum,
    Average,
    Max,
    Min,
}

struct Fold {
    kind: Aggregate,
    sum: f64,
    count: usize,
    extreme: Option<f64>,
}

impl Fold {
    fn new(kind: Aggregate) -> Self {
        Self { kind, sum: 0.0, count: 0, extreme: None }
    }

    #[inline]
    fn push(&mut self, value: &Value) -> R<()> {
        match value {
            Value::Number(x) => match self.kind {
                Aggregate::Sum => self.sum += *x,
                Aggregate::Average => {
                    self.sum += *x;
                    self.count += 1;
                }
                Aggregate::Max => {
                    self.extreme = Some(match self.extreme {
                        Some(best) => best.max(*x),
                        None => *x,
                    });
                }
                Aggregate::Min => {
                    self.extreme = Some(match self.extreme {
                        Some(best) => best.min(*x),
                        None => *x,
                    });
                }
            },
            Value::Error(e) => return Err(*e),
            _ => {}
        }
        Ok(())
    }

    fn finish(self) -> R<Value> {
        match self.kind {
            Aggregate::Sum => num_val(self.sum),
            Aggregate::Average => {
                if self.count == 0 {
                    Err(CellError::Div0)
                } else {
                    num_val(self.sum / self.count as f64)
                }
            }
            Aggregate::Max | Aggregate::Min => num_val(self.extreme.unwrap_or(0.0)),
        }
    }
}

fn compile_pairs<'a>(a: &'a [Arg], first_pair: usize, rows: usize, cols: usize, date_system: DateSystem) -> R<Vec<(GridView<'a>, Criterion)>> {
    let rest = a.get(first_pair..).unwrap_or(&[]);
    if rest.is_empty() || rest.len() % 2 != 0 {
        return Err(CellError::Value);
    }
    let single_pair = rest.len() == 2;
    let mut pairs = Vec::with_capacity(rest.len() / 2);
    for chunk in rest.chunks(2) {
        let (Some(range_arg), Some(crit_arg)) = (chunk.first(), chunk.get(1)) else {
            return Err(CellError::Value);
        };
        let grid = GridView::from_value(&range_arg.value);
        if grid.rows != rows || grid.cols != cols {
            return Err(CellError::Value);
        }
        let crit_val = match &crit_arg.value {
            Value::Array(arr) => arr.data.first().unwrap_or(&Value::Empty),
            other => other,
        };
        let criterion =
            if single_pair { Criterion::parse_arg_in(crit_val, date_system)? } else { Criterion::parse_arg_truncated_in(crit_val, date_system) };
        pairs.push((grid, criterion));
    }
    Ok(pairs)
}

#[inline]
fn all_match(pairs: &[(GridView<'_>, Criterion)], idx: usize) -> bool {
    for (grid, criterion) in pairs {
        let Some(cell) = grid.cells.get(idx) else { return false };
        if !criterion.matches(cell) {
            return false;
        }
    }
    true
}

fn fold_ifs(a: &[Arg], kind: Aggregate, date_system: DateSystem) -> R<Value> {
    let target = GridView::from_value(&arg(a, 0)?.value);
    let pairs = compile_pairs(a, 1, target.rows, target.cols, date_system)?;
    let mut fold = Fold::new(kind);
    let total = target.len().min(target.cells.len());
    if let [(grid, criterion)] = pairs.as_slice() {
        for (cell, value) in grid.cells.iter().zip(target.cells.iter()).take(total) {
            if criterion.matches(cell) {
                fold.push(value)?;
            }
        }
    } else {
        for (idx, value) in target.cells.iter().enumerate().take(total) {
            if all_match(&pairs, idx) {
                fold.push(value)?;
            }
        }
    }
    fold.finish()
}

fn fold_if(a: &[Arg], kind: Aggregate, date_system: DateSystem) -> R<Value> {
    let range = GridView::from_value(&arg(a, 0)?.value);
    let crit_val = match &arg(a, 1)?.value {
        Value::Array(arr) => arr.data.first().unwrap_or(&Value::Empty),
        other => other,
    };
    let criterion = Criterion::parse_arg_in(crit_val, date_system)?;
    let mut fold = Fold::new(kind);
    if !has(a, 2) {
        for cell in range.cells.iter().take(range.len()) {
            if criterion.matches(cell) {
                fold.push(cell)?;
            }
        }
        return fold.finish();
    }
    let target = GridView::from_value(&arg(a, 2)?.value);
    if target.rows == range.rows && target.cols == range.cols {
        for (cell, value) in range.cells.iter().zip(target.cells.iter()).take(range.len()) {
            if criterion.matches(cell) {
                fold.push(value)?;
            }
        }
    } else {
        for r in 0..range.rows {
            for col in 0..range.cols {
                let Some(cell) = range.get_rc(r, col) else { continue };
                if criterion.matches(cell)
                    && let Some(value) = target.get_rc(r, col)
                {
                    fold.push(value)?;
                }
            }
        }
    }
    fold.finish()
}

pub(super) fn countif(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let range = GridView::from_value(&arg(a, 0)?.value);
    let crit_val = match &arg(a, 1)?.value {
        Value::Array(arr) => arr.data.first().unwrap_or(&Value::Empty),
        other => other,
    };
    let criterion = Criterion::parse_arg_in(crit_val, c.date_system())?;
    let mut count = 0usize;
    for cell in range.cells.iter().take(range.len()) {
        if criterion.matches(cell) {
            count += 1;
        }
    }
    num_val(count as f64)
}

pub(super) fn countifs(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let first = GridView::from_value(&arg(a, 0)?.value);
    let pairs = compile_pairs(a, 0, first.rows, first.cols, c.date_system())?;
    let total = first.len().min(first.cells.len());
    let mut count = 0usize;
    if let [(grid, criterion)] = pairs.as_slice() {
        for cell in grid.cells.iter().take(total) {
            if criterion.matches(cell) {
                count += 1;
            }
        }
    } else {
        for idx in 0..total {
            if all_match(&pairs, idx) {
                count += 1;
            }
        }
    }
    num_val(count as f64)
}

pub(super) fn sumif(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    fold_if(a, Aggregate::Sum, c.date_system())
}

pub(super) fn averageif(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    fold_if(a, Aggregate::Average, c.date_system())
}

pub(super) fn sumifs(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    fold_ifs(a, Aggregate::Sum, c.date_system())
}

pub(super) fn averageifs(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    fold_ifs(a, Aggregate::Average, c.date_system())
}

pub(super) fn maxifs(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    fold_ifs(a, Aggregate::Max, c.date_system())
}

pub(super) fn minifs(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    fold_ifs(a, Aggregate::Min, c.date_system())
}
