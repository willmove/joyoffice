//! First-party matrix functions (`MMULT`, `MDETERM`, `MINVERSE`, `MUNIT`).
//!
//! `MDETERM` and `MINVERSE` share an in-place LU factorization with row-scaled
//! partial pivoting (`lu_factor`), using contiguous row-slice operations for
//! cache locality and SIMD auto-vectorization without `unsafe`.

use super::*;

const MAX_MMULT_OPS: usize = 2_000_000_000;
const MAX_SQUARE_DIM: usize = 2000;

/// Extracts a row-major numeric matrix from `v` without cloning an existing array.
/// Every element must be a finite number; errors propagate and non-numbers give `#VALUE!`.
fn extract_matrix(v: &Value) -> R<(usize, usize, Vec<f64>)> {
    match v {
        Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.data.len());
            for cell in arr.iter() {
                match cell {
                    Value::Number(n) => out.push(*n),
                    Value::Error(e) => return Err(*e),
                    _ => return Err(CellError::Value),
                }
            }
            Ok((arr.rows, arr.cols, out))
        }
        Value::Number(n) => Ok((1, 1, vec![*n])),
        Value::Error(e) => Err(*e),
        _ => Err(CellError::Value),
    }
}

fn to_value_array(rows: usize, cols: usize, data: Vec<f64>) -> R<Value> {
    let vals = data.into_iter().map(|x| if x.is_finite() { Value::Number(x) } else { Value::Error(CellError::Num) }).collect();
    array_val(rows, cols, vals)
}

fn square_matrix(a: &[Arg]) -> R<(usize, Vec<f64>)> {
    let (r, c, d) = extract_matrix(&crate::util::arg(a, 0)?.value)?;
    if r != c {
        return Err(CellError::Value);
    }
    if r > MAX_SQUARE_DIM {
        return Err(CellError::Num);
    }
    Ok((r, d))
}

/// Factors square `n × n` matrix `lu` in place as `P · A = L · U` with implicit
/// row-equilibrated (scaled) partial pivoting.
///
/// Returns `(perm, sign, max_abs)` where row `i` of `P · A` is original row
/// `perm[i]`, `sign` is `±1.0` for the permutation parity, and `max_abs` is the
/// max entry magnitude. Returns `None` when a row is identically zero or a pivot
/// magnitude is `<= min_pivot_rel * max_abs`.
fn lu_factor(n: usize, lu: &mut [f64], min_pivot_rel: f64) -> Option<(Vec<usize>, f64, f64)> {
    let mut row_scale = Vec::with_capacity(n);
    let mut max_abs = 0.0_f64;
    for row in lu.chunks_exact(n) {
        let rmax = row.iter().fold(0.0_f64, |acc, &x| acc.max(x.abs()));
        if rmax == 0.0 || !rmax.is_finite() {
            return None;
        }
        max_abs = max_abs.max(rmax);
        row_scale.push(rmax);
    }
    let min_pivot = if min_pivot_rel > 0.0 { max_abs.max(1e-300) * min_pivot_rel } else { 0.0 };
    let mut perm: Vec<usize> = (0..n).collect();
    let mut sign = 1.0_f64;

    for col in 0..n {
        let mut piv = col;
        let mut best_scaled = -1.0_f64;
        let mut best_raw = 0.0_f64;
        for row in col..n {
            let raw = lu.get(row * n + col).copied().unwrap_or(0.0).abs();
            let scale = row_scale.get(row).copied().unwrap_or(1.0);
            let scaled = raw / scale;
            if scaled > best_scaled {
                best_scaled = scaled;
                best_raw = raw;
                piv = row;
            }
        }
        if best_raw <= min_pivot || !best_raw.is_finite() {
            return None;
        }
        if piv != col {
            let (head, tail) = lu.split_at_mut(piv * n);
            if let (Some(r1), Some(r2)) = (head.get_mut(col * n..(col + 1) * n), tail.get_mut(..n)) {
                r1.swap_with_slice(r2);
            }
            row_scale.swap(piv, col);
            perm.swap(piv, col);
            sign = -sign;
        }
        let p = lu.get(col * n + col).copied().unwrap_or(0.0);
        if p == 0.0 || !p.is_finite() {
            return None;
        }
        if col + 1 < n {
            let (upper, lower) = lu.split_at_mut((col + 1) * n);
            let piv_tail = upper.get(col * n + col + 1..(col + 1) * n).unwrap_or(&[]);
            for rem_row in lower.chunks_exact_mut(n) {
                let Some(slot) = rem_row.get_mut(col) else { continue };
                let factor = *slot / p;
                *slot = factor;
                if factor == 0.0 {
                    continue;
                }
                if let Some(tail_row) = rem_row.get_mut(col + 1..) {
                    for (dst, &src) in tail_row.iter_mut().zip(piv_tail) {
                        *dst -= factor * src;
                    }
                }
            }
        }
    }
    Some((perm, sign, max_abs))
}

pub(super) fn mmult(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (ar, ac, ad) = extract_matrix(&crate::util::arg(a, 0)?.value)?;
    let (br, bc, bd) = extract_matrix(&crate::util::arg(a, 1)?.value)?;
    if ac != br {
        return Err(CellError::Value);
    }
    if ar.saturating_mul(bc) > MAX_CELLS || ar.saturating_mul(bc).saturating_mul(ac) > MAX_MMULT_OPS {
        return Err(CellError::Num);
    }
    let mut out = vec![0.0_f64; ar * bc];
    for (out_row, a_row) in out.chunks_exact_mut(bc).zip(ad.chunks_exact(ac)) {
        for (&x, b_row) in a_row.iter().zip(bd.chunks_exact(bc)) {
            if x == 0.0 {
                continue;
            }
            for (dst, &y) in out_row.iter_mut().zip(b_row) {
                *dst += x * y;
            }
        }
    }
    to_value_array(ar, bc, out)
}

pub(super) fn mdeterm(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (n, mut m) = square_matrix(a)?;
    let d = if n == 1 {
        m.first().copied().unwrap_or(0.0)
    } else if let Some((_, sign, _)) = lu_factor(n, &mut m, 0.0) {
        let mut prod = sign;
        for i in 0..n {
            prod *= m.get(i * n + i).copied().unwrap_or(0.0);
        }
        prod
    } else {
        0.0
    };
    if !d.is_finite() {
        return Err(CellError::Num);
    }
    // Tidy floating-point residuals near integers (e.g. 0.9999999999999998 -> 1).
    num_val(if d.abs() > 1e-300 { to_sig_digits(d, 15) } else { d })
}

pub(super) fn minverse(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (n, mut lu) = square_matrix(a)?;
    if n == 1 {
        let x = lu.first().copied().unwrap_or(0.0);
        if x == 0.0 || !x.is_finite() {
            return Err(CellError::Num);
        }
        return to_value_array(1, 1, vec![1.0 / x]);
    }
    let Some((perm, _, max_abs)) = lu_factor(n, &mut lu, 1e-15) else {
        return Err(CellError::Num);
    };
    // Initialize right-hand side Y = P · I so row i has 1.0 at column perm[i].
    let mut inv = vec![0.0_f64; n * n];
    for (i, &src_col) in perm.iter().enumerate() {
        if let Some(slot) = inv.get_mut(i * n + src_col) {
            *slot = 1.0;
        }
    }
    // Forward substitution: solve L · Y = P · I in row-major order.
    for i in 1..n {
        let (prev_rows, rest) = inv.split_at_mut(i * n);
        let Some(cur_row) = rest.get_mut(..n) else { continue };
        let l_row = lu.get(i * n..i * n + i).unwrap_or(&[]);
        for (k, &lik) in l_row.iter().enumerate() {
            if lik == 0.0 {
                continue;
            }
            if let Some(prev) = prev_rows.get(k * n..(k + 1) * n) {
                for (dst, &src) in cur_row.iter_mut().zip(prev) {
                    *dst -= lik * src;
                }
            }
        }
    }
    // Back substitution: solve U · X = Y in row-major order.
    for i in (0..n).rev() {
        let (head, tail) = inv.split_at_mut((i + 1) * n);
        let Some(cur_row) = head.get_mut(i * n..(i + 1) * n) else { continue };
        let u_row = lu.get(i * n + i + 1..(i + 1) * n).unwrap_or(&[]);
        for (offset, &uik) in u_row.iter().enumerate() {
            if uik == 0.0 {
                continue;
            }
            if let Some(below) = tail.get(offset * n..(offset + 1) * n) {
                for (dst, &src) in cur_row.iter_mut().zip(below) {
                    *dst -= uik * src;
                }
            }
        }
        let diag = lu.get(i * n + i).copied().unwrap_or(0.0);
        if diag == 0.0 || !diag.is_finite() {
            return Err(CellError::Num);
        }
        let inv_diag = 1.0 / diag;
        for dst in cur_row.iter_mut() {
            *dst *= inv_diag;
        }
    }
    // Rounding noise in the inverse scales with 1/max_abs of the input matrix.
    let scale = max_abs.max(1e-300);
    let noise = 1e-15 / scale;
    for x in &mut inv {
        if !x.is_finite() {
            return Err(CellError::Num);
        }
        if x.abs() < noise {
            *x = 0.0;
        }
    }
    to_value_array(n, n, inv)
}

pub(super) fn munit(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let n = num(a, 0)?.trunc();
    let (r, _) = check_size(n, n)?;
    let mut data = vec![Value::Number(0.0); r * r];
    for i in 0..r {
        if let Some(x) = data.get_mut(i * r + i) {
            *x = Value::Number(1.0);
        }
    }
    array_val(r, r, data)
}
