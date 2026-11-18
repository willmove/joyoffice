//! Select source row/column indices first. Only the result materializes cells.
use super::{check_output_size, int_list, truthy};
use crate::util::{R, arg, as_array, has, num, opt_bool};
use crate::{Arg, Ctx};
use joysheet_core::{Array, CellError, Value, sort_compare};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{HashMap, hash_map::Entry};

pub(super) fn input(args: &[Arg], index: usize) -> R<Cow<'_, Array>> {
    match &arg(args, index)?.value {
        Value::Error(error) => Err(*error),
        value => {
            let array = as_array(value);
            array.rows.checked_mul(array.cols).ok_or(CellError::Num)?;
            Ok(array)
        }
    }
}

struct Lines<'a> {
    array: &'a Array,
    by_column: bool,
}

impl Lines<'_> {
    fn len(&self) -> usize {
        if self.by_column { self.array.cols } else { self.array.rows }
    }

    fn width(&self) -> usize {
        if self.by_column { self.array.rows } else { self.array.cols }
    }

    fn cell(&self, line: usize, offset: usize) -> &Value {
        let (r, c) = if self.by_column { (offset, line) } else { (line, offset) };
        self.array.get(r, c).unwrap_or(&Value::Empty)
    }

    fn subset(&self, selected: &[usize]) -> R<Value> {
        check_output_size(selected.len(), self.width())?;
        self.gather(selected)
    }

    // Sorting preserves the dimensions of an already accepted input. FILTER
    // and UNIQUE also apply their existing output budget via subset().
    fn gather(&self, selected: &[usize]) -> R<Value> {
        let width = self.width();
        let count = selected.len().checked_mul(width).ok_or(CellError::Num)?;
        let mut data = Vec::new();
        data.try_reserve_exact(count).map_err(|_| CellError::Num)?;
        let (rows, cols) = if self.by_column {
            for r in 0..width {
                data.extend(selected.iter().map(|&index| self.cell(index, r).clone()));
            }
            (width, selected.len())
        } else {
            for &index in selected {
                let span = index.checked_mul(width).and_then(|start| start.checked_add(width).map(|end| start..end));
                // Arrays read from older JSON may lack trailing cells.
                if let Some(row) = span.and_then(|range| self.array.data.get(range)) {
                    data.extend_from_slice(row);
                } else {
                    data.extend((0..width).map(|c| self.cell(index, c).clone()));
                }
            }
            (selected.len(), width)
        };
        Array::new(rows, cols, data).map(Value::from).ok_or(CellError::Calc)
    }
}

fn columns_for(input: &Array, keys: &Array) -> R<bool> {
    if keys.rows == input.rows && keys.cols == 1 {
        Ok(false)
    } else if keys.cols == input.cols && keys.rows == 1 {
        Ok(true)
    } else {
        Err(CellError::Value)
    }
}

pub(super) fn filter(args: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    let a = input(args, 0)?;
    let mask = input(args, 1)?;
    let lines = Lines { array: &a, by_column: columns_for(&a, &mask)? };
    let mut selected = Vec::new();
    for (index, flag) in mask.data.iter().enumerate() {
        if truthy(flag)? && index < lines.len() {
            selected.push(index);
        }
    }
    if selected.is_empty() || lines.width() == 0 {
        return if has(args, 2) { Ok(arg(args, 2)?.value.clone()) } else { Err(CellError::Calc) };
    }
    lines.subset(&selected)
}

pub(super) fn ordered(x: &Value, y: &Value, order: i64) -> Ordering {
    if order < 0 && !matches!(x, Value::Empty) && !matches!(y, Value::Empty) { sort_compare(y, x) } else { sort_compare(x, y) }
}

pub(super) fn sort(args: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    let a = input(args, 0)?;
    let lines = Lines { array: &a, by_column: opt_bool(args, 3, false)? };
    let key_indices = if has(args, 1) { int_list(&arg(args, 1)?.value)? } else { vec![1] };
    let orders = if has(args, 2) { int_list(&arg(args, 2)?.value)? } else { vec![1] };
    if key_indices.is_empty()
        || key_indices.iter().any(|&i| i < 1 || i as usize > lines.width())
        || orders.iter().any(|&o| !matches!(o, -1 | 1))
        || (orders.len() > 1 && orders.len() != key_indices.len())
    {
        return Err(CellError::Value);
    }
    let keys: Vec<_> =
        key_indices.into_iter().enumerate().map(|(n, key)| (key as usize - 1, orders.get(n).or(orders.first()).copied().unwrap_or(1))).collect();
    let mut indices: Vec<_> = (0..lines.len()).collect();
    indices.sort_by(|&a, &b| {
        keys.iter()
            .map(|&(key, order)| ordered(lines.cell(a, key), lines.cell(b, key), order))
            .find(|&o| o != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    });
    lines.gather(&indices)
}

pub(super) fn sortby(args: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    let a = input(args, 0)?;
    let mut keys = Vec::new();
    for index in (1..args.len()).step_by(2) {
        let key = input(args, index)?;
        let order = if has(args, index + 1) { num(args, index + 1)?.trunc() as i64 } else { 1 };
        if !matches!(order, 1 | -1) {
            return Err(CellError::Value);
        }
        keys.push((key, order));
    }
    let (first, _) = keys.first().ok_or(CellError::Value)?;
    let columns = columns_for(&a, first)?;
    if !keys.iter().all(|(k, _)| (k.rows, k.cols) == (first.rows, first.cols)) {
        return Err(CellError::Value);
    }
    let lines = Lines { array: &a, by_column: columns };
    let mut indices: Vec<_> = (0..lines.len()).collect();
    indices.sort_by(|&a, &b| {
        keys.iter()
            .map(|(key, order)| ordered(key.data.get(a).unwrap_or(&Value::Empty), key.data.get(b).unwrap_or(&Value::Empty), *order))
            .find(|&o| o != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    });
    lines.gather(&indices)
}

#[derive(Hash, Eq, PartialEq)]
enum CellKey<'a> {
    Empty,
    Number(u64),
    Text(Cow<'a, str>),
    Bool(bool),
    Error(CellError),
}

impl<'a> From<&'a Value> for CellKey<'a> {
    fn from(value: &'a Value) -> Self {
        match value {
            Value::Empty => Self::Empty,
            Value::Bool(v) => Self::Bool(*v),
            Value::Number(v) => Self::Number(if *v == 0. { 0 } else { v.to_bits() }),
            Value::Error(e) => Self::Error(*e),
            Value::Text(s) if s.is_ascii() && !s.bytes().any(|c| c.is_ascii_uppercase()) => Self::Text(Cow::Borrowed(s)),
            Value::Text(s) => Self::Text(Cow::Owned(s.to_lowercase())),
            Value::Array(a) => Self::from(a.data.first().unwrap_or(&Value::Empty)),
        }
    }
}

pub(super) fn unique(args: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    let a = input(args, 0)?;
    let lines = Lines { array: &a, by_column: opt_bool(args, 1, false)? };
    let once = opt_bool(args, 2, false)?;
    let mut seen = HashMap::new();
    let mut candidates = Vec::new();
    for index in 0..lines.len() {
        let key: Vec<_> = (0..lines.width()).map(|n| CellKey::from(lines.cell(index, n))).collect();
        match seen.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(candidates.len());
                candidates.push(Some(index));
            }
            Entry::Occupied(entry) if once => candidates[*entry.get()] = None,
            Entry::Occupied(_) => {}
        }
    }
    let selected: Vec<_> = candidates.into_iter().flatten().collect();
    lines.subset(&selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::testutil::{arr, av, ev, rf, rows_of};

    #[test]
    fn masks_select_whole_rows_and_columns_in_source_order() {
        let cells: Vec<Vec<Value>> = (0..4).map(|r| (0..4).map(|c| Value::number((10 * r + c) as f64)).collect()).collect();
        for bits in 0..16 {
            let indices: Vec<_> = (0..4).filter(|i| bits & (1 << i) != 0).collect();
            let flags: Vec<Value> = (0..4).map(|i| Value::Bool(bits & (1 << i) != 0)).collect();
            for by_column in [false, true] {
                let mask = if by_column { vec![flags.clone()] } else { flags.iter().cloned().map(|v| vec![v]).collect() };
                let output = ev("FILTER", vec![rf(arr(cells.clone())), av(arr(mask))]);
                if indices.is_empty() {
                    assert_eq!(output, Value::Error(CellError::Calc));
                    continue;
                }
                let expected = if by_column {
                    cells.iter().map(|r| indices.iter().map(|&i| r[i].clone()).collect()).collect()
                } else {
                    indices.iter().map(|&i| cells[i].clone()).collect::<Vec<_>>()
                };
                assert_eq!(rows_of(&output), expected);
            }
        }
    }

    #[test]
    fn distinct_columns_keep_the_first_spelling_and_distinguish_scalar_types() {
        let row = vec![
            Value::number(-0.),
            Value::number(0.),
            Value::from("0"),
            Value::from("Straße"),
            Value::from("straße"),
            Value::Bool(false),
            Value::Error(CellError::NA),
            Value::Empty,
            Value::Error(CellError::NA),
            Value::Error(CellError::Value),
        ];
        for (once, indices) in [(false, vec![0, 2, 3, 5, 6, 7, 9]), (true, vec![2, 5, 7, 9])] {
            let got = ev("UNIQUE", vec![av(arr(vec![row.clone()])), av(Value::Bool(true)), av(Value::Bool(once))]);
            assert_eq!(rows_of(&got), vec![indices.into_iter().map(|i| row[i].clone()).collect::<Vec<_>>()]);
        }
    }
}
