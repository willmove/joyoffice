//! Shape agreement and borrowed row access for worksheet array formulas.
//! A singleton axis repeats. A short non-singleton axis supplies #N/A beyond its edge.
use crate::{Array, CellError, Value};

static MISSING: Value = Value::Error(CellError::NA);

/// Row and column count together with the checked number of output cells.
/// The caller supplies its own evaluator or function allocation limit.
pub fn dimensions(sizes: impl IntoIterator<Item = (usize, usize)>, limit: usize) -> Option<(usize, usize, usize)> {
    fn axis(a: usize, b: usize) -> usize {
        // Valid worksheet arrays are nonempty. Keep an empty axis empty when
        // combined with a scalar, as with the previous binary operators.
        if a == 1 {
            b
        } else if b == 1 {
            a
        } else {
            a.max(b)
        }
    }
    let (rows, cols) = sizes.into_iter().reduce(|(r, c), (h, w)| (axis(r, h), axis(c, w))).unwrap_or((1, 1));
    let count = rows.checked_mul(cols)?;
    (count <= limit).then_some((rows, cols, count))
}

/// A row of an operand at a particular output row. Scalars and singleton
/// columns can share one value across every output column.
#[derive(Clone, Copy)]
pub struct BroadcastRow<'a> {
    values: &'a [Value],
    repeat: bool,
}

impl<'a> BroadcastRow<'a> {
    pub fn cell(self, column: usize) -> &'a Value {
        self.values.get(if self.repeat { 0 } else { column }).unwrap_or(&MISSING)
    }
}

impl Value {
    pub fn array_dimensions(&self) -> (usize, usize) {
        match self {
            Self::Array(a) => (a.rows, a.cols),
            _ => (1, 1),
        }
    }

    pub fn broadcast_row(&self, row: usize) -> BroadcastRow<'_> {
        match self {
            Self::Array(a) => a.broadcast_row(row),
            value => BroadcastRow { values: std::slice::from_ref(value), repeat: true },
        }
    }
}

impl Array {
    pub fn broadcast_row(&self, row: usize) -> BroadcastRow<'_> {
        let index = if self.rows == 1 { 0 } else { row };
        let values = (index < self.rows)
            .then_some(index)
            .and_then(|i| i.checked_mul(self.cols))
            .and_then(|start| self.data.get(start..))
            .map_or(&[][..], |tail| &tail[..tail.len().min(self.cols)]);
        BroadcastRow { values, repeat: self.cols == 1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_small_rectangle_agrees_with_explicit_scalar_expansion() {
        for h in 1..=4 {
            for w in 1..=4 {
                let data = (0..h * w).map(|i| Value::number(i as f64)).collect();
                let a = Array { rows: h, cols: w, data };
                for other_h in 1..=4 {
                    for other_w in 1..=4 {
                        let (rows, cols) = (h.max(other_h), w.max(other_w));
                        assert_eq!(dimensions([(h, w), (other_h, other_w)], 16), Some((rows, cols, rows * cols)));
                        for r in 0..rows {
                            let row = a.broadcast_row(r);
                            for c in 0..cols {
                                let ri = if h == 1 { 0 } else { r };
                                let ci = if w == 1 { 0 } else { c };
                                let reference = if ri < h && ci < w { Value::number((ri * w + ci) as f64) } else { MISSING.clone() };
                                assert_eq!(row.cell(c), &reference);
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(dimensions([(usize::MAX, 1), (1, 2)], usize::MAX), None);
        assert_eq!(dimensions([(100, 1), (1, 100)], 9999), None);
        let missing = Array { rows: usize::MAX, cols: usize::MAX, data: vec![] };
        assert_eq!(missing.broadcast_row(2).cell(1), &MISSING);
        let text = Value::from("原文");
        assert!(std::ptr::eq(text.broadcast_row(200).cell(10), &text));
    }
}
