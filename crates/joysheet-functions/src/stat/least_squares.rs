//! Column-pivoted Householder least squares for LINEST, LOGEST, TREND and GROWTH.
//! Work directly on the design matrix. Squaring the matrix's condition number
//! loses coefficients when two predictors are close but still independent.
use super::regression::check_regression_size;
use super::{CellError, R};

pub(super) struct Fit {
    pub coef: Vec<f64>,
    pub b: f64,
    pub stats: Option<Statistics>,
    response_mean: f64,
    means: Vec<f64>,
}

impl Fit {
    // The ordinary intercept can be orders of magnitude larger than the
    // predicted value when offsets such as dates occur in several columns.
    pub fn predict(&self, row: &[f64]) -> f64 {
        self.response_mean + row.iter().zip(&self.means).zip(&self.coef).map(|((&x, &mean), &beta)| beta * (x - mean)).sum::<f64>()
    }
}

pub(super) struct Statistics {
    pub se: Vec<f64>,
    pub se_b: Option<f64>,
    pub r2: R<f64>,
    pub sey: R<f64>,
    pub f: R<f64>,
    pub df: f64,
    pub ssreg: f64,
    pub ssresid: f64,
}

fn squared(xs: &[f64]) -> f64 {
    xs.iter().map(|x| x * x).sum()
}

fn norm(xs: &[f64]) -> f64 {
    let sum = squared(xs);
    if sum.is_normal() {
        return sum.sqrt();
    }
    let scale = xs.iter().fold(0.0_f64, |s, x| s.max(x.abs()));
    if scale == 0. {
        return 0.;
    }
    scale * xs.iter().map(|x| (x / scale).powi(2)).sum::<f64>().sqrt()
}

fn centre(values: &mut [f64]) -> f64 {
    // Remove the common origin before summing, including for date-like values.
    let first = values[0];
    let offset = values.iter().map(|v| v - first).sum::<f64>() / values.len() as f64;
    for v in values {
        *v = (*v - first) - offset;
    }
    first + offset
}

// Construct a reflector in place. Its first entry is implicitly one. Return
// the diagonal of R and the multiplier for x -= tau v (vᵀx).
fn reflector(x: &mut [f64], norm: f64) -> (f64, f64) {
    if norm == 0. {
        return (0., 0.);
    }
    let diagonal = -norm.copysign(x[0]);
    let head = x[0] - diagonal;
    let tau = if head.is_finite() { -head / diagonal } else { 1. - x[0] / diagonal };
    let first = x[0];
    x[0] = 1.;
    for tail in &mut x[1..] {
        *tail = if head.is_finite() { *tail / head } else { (*tail / diagonal) / (first / diagonal - 1.) };
    }
    (diagonal, tau)
}

fn reflect(target: &mut [f64], vector: &[f64], tau: f64) {
    let projection = tau * vector.iter().zip(target.iter()).map(|(v, y)| v * y).sum::<f64>();
    for (y, v) in target.iter_mut().zip(vector) {
        *y -= v * projection;
    }
}

// Solve R x = b for the triangular leading block, stored by columns.
fn back_substitute(columns: &[Vec<f64>], start: usize, rhs: &mut [f64]) {
    for j in (0..rhs.len()).rev() {
        rhs[j] /= columns[j][start + j];
        for i in 0..j {
            rhs[i] -= columns[j][start + i] * rhs[j];
        }
    }
}

pub(super) fn fit(mut response: Vec<f64>, mut columns: Vec<Vec<f64>>, constant: bool, stats: bool) -> R<Fit> {
    let (n, k) = (response.len(), columns.len());
    if n == 0 || k == 0 {
        return Err(CellError::Value);
    }
    check_regression_size(n, k)?;
    let start = usize::from(constant);
    let mut means = vec![0.; k];
    let mut response_mean = 0.;
    if constant {
        response_mean = centre(&mut response);
        let mut ones = vec![1.; n];
        let (_, tau) = reflector(&mut ones, (n as f64).sqrt());
        reflect(&mut response, &ones, tau);
        for (column, mean) in columns.iter_mut().zip(&mut means) {
            *mean = centre(column);
            reflect(column, &ones, tau);
        }
    }
    let total = if stats { squared(&response[start..]) } else { 0. };
    let norms: Vec<_> = columns.iter().map(|col| norm(&col[start..])).collect();
    let mut order: Vec<usize> = (0..k).collect();
    let mut rank = 0;
    while rank < k && start + rank < n {
        let row = start + rank;
        let mut pivot = None;
        let mut largest = 0.;
        for j in rank..k {
            let length = if rank == 0 { norms[order[j]] } else { norm(&columns[j][row..]) };
            if (length > largest || (constant && length == largest)) && length > norms[order[j]] * f64::EPSILON.sqrt() {
                largest = length;
                pivot = Some(j);
            }
        }
        let Some(j) = pivot else { break };
        columns.swap(rank, j);
        order.swap(rank, j);
        let (head, pending) = columns.split_at_mut(rank + 1);
        let vector = &mut head[rank][row..];
        let (diagonal, tau) = reflector(vector, largest);
        for next in pending {
            reflect(&mut next[row..], vector, tau);
        }
        reflect(&mut response[row..], vector, tau);
        vector[0] = diagonal;
        rank += 1;
    }

    let mut solved = response[start..start + rank].to_vec();
    back_substitute(&columns, start, &mut solved);
    let mut coef = vec![0.; k];
    for (&variable, &estimate) in order.iter().zip(&solved) {
        coef[variable] = estimate;
    }
    let intercept = response_mean - means.iter().zip(&coef).map(|(mean, slope)| mean * slope).sum::<f64>();
    if !stats {
        return Ok(Fit { coef, b: intercept, response_mean, means, stats: None });
    }
    // An identically zero design column still consumes a residual coordinate in
    // Excel, including a constant predictor after fitting an intercept.
    let reserved = norms.iter().filter(|&&v| v == 0.).count();
    let used = (start + rank + reserved).min(n);
    let df = (n - used) as f64;
    let ssresid = squared(&response[used..]);
    let ssreg = (total - ssresid).max(0.);
    let variance = if df > 0. { ssresid / df } else { 0. };
    let r2 = if total > 0. {
        Ok(ssreg / total)
    } else if df == 0. {
        Ok(1.)
    } else {
        Err(CellError::Num)
    };
    let terms = (used - start) as f64;
    let f = if variance > 0. && terms > 0. { Ok(ssreg / terms / variance) } else { Err(CellError::Num) };
    let mut diagonal = vec![0.; k];
    let mut intercept_weight = 1. / n as f64;
    // Diagonal of R⁻¹R⁻ᵀ and the intercept leverage, without inverting XᵀX.
    for j in 0..rank {
        let mut column = vec![0.; j + 1];
        column[j] = 1.;
        back_substitute(&columns, start, &mut column);
        let mut projection = 0.;
        for (&estimate, &variable) in column.iter().zip(&order) {
            diagonal[variable] += estimate * estimate;
            projection += means[variable] * estimate;
        }
        intercept_weight += projection * projection;
    }
    let se = diagonal.into_iter().map(|weight| if weight == 0. { 0. } else { (weight * variance).sqrt() }).collect();
    let se_b = constant.then(|| (intercept_weight * variance).sqrt());
    let stats = Statistics { se, se_b, r2, sey: Ok(variance.sqrt()), f, df, ssreg, ssresid };
    Ok(Fit { coef, b: intercept, response_mean, means, stats: Some(stats) })
}
