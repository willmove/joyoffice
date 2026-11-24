//! Worksheet probability distributions and tests.
//!
//! Distribution formulas follow the NIST Engineering Statistics Handbook
//! (https://www.itl.nist.gov/div898/handbook/eda/section3/eda366.htm) and
//! DLMF §§8.2, 8.4 and 8.17. Parameter and error contracts follow the worksheet
//! function registry and Microsoft's statistical function reference.

use super::descriptive::{arg_nums, arg_pairs, mean, var_s};
use super::*;
use crate::special::{beta_tail, erf, gamma, invert, ln_beta, ln_gamma, norm_cdf, norm_inv, norm_pdf, reg_gamma_p, reg_gamma_q, reg_inc_beta};
use std::f64::consts::PI;

const QUANTILE_LIMIT: f64 = 1e60;

fn open_probability(p: f64) -> bool {
    0.0 < p && p < 1.0
}

fn optional_flag(args: &[Arg], index: usize, default: bool) -> R<bool> {
    if args.len() <= index { Ok(default) } else { boolean(args, index) }
}

fn normal(x: f64, mean: f64, sd: f64, cumulative: bool) -> f64 {
    let z = (x - mean) / sd;
    if cumulative { norm_cdf(z) } else { norm_pdf(z) / sd }
}

fn normal_quantile(p: f64) -> f64 {
    if p > 0.5 { -norm_inv(1.0 - p) } else { norm_inv(p) }
}

pub(super) fn norm_dist(a: &[Arg], _: C) -> R<Value> {
    let (x, center, sd, cumulative) = (num(a, 0)?, num(a, 1)?, num(a, 2)?, boolean(a, 3)?);
    if sd <= 0.0 {
        return Err(CellError::Num);
    }
    num_val(normal(x, center, sd, cumulative))
}

pub(super) fn norm_s_dist(a: &[Arg], _: C) -> R<Value> {
    num_val(normal(num(a, 0)?, 0.0, 1.0, optional_flag(a, 1, true)?))
}

pub(super) fn norm_inv_fn(a: &[Arg], _: C) -> R<Value> {
    let (p, center, sd) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
    if !open_probability(p) || sd <= 0.0 {
        return Err(CellError::Num);
    }
    num_val(center + sd * normal_quantile(p))
}

pub(super) fn norm_s_inv(a: &[Arg], _: C) -> R<Value> {
    let p = num(a, 0)?;
    if !open_probability(p) {
        return Err(CellError::Num);
    }
    num_val(normal_quantile(p))
}

pub(super) fn lognorm_dist(a: &[Arg], _: C) -> R<Value> {
    let (x, center, sd) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
    let cumulative = optional_flag(a, 3, true)?;
    if x <= 0.0 || sd <= 0.0 {
        return Err(CellError::Num);
    }
    let result = normal(x.ln(), center, sd, cumulative);
    num_val(if cumulative { result } else { result / x })
}

pub(super) fn lognorm_inv(a: &[Arg], _: C) -> R<Value> {
    let (p, center, sd) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
    if !open_probability(p) || sd <= 0.0 {
        return Err(CellError::Num);
    }
    num_val((center + sd * normal_quantile(p)).exp())
}

// The small coordinate of the beta transform survives rounding when x is near
// zero or very large. The complement supplies the Student density's log term.
fn beta_coordinates(odds: f64) -> (f64, f64) {
    let tail = 1.0 / (1.0 + odds);
    let head = if odds < 1.0 { odds * tail } else { 1.0 - tail };
    (head, tail)
}

fn student_right(x: f64, df: f64) -> f64 {
    let (head, tail) = beta_coordinates((x / df.sqrt()).powi(2));
    let outside = 0.5 * beta_tail(head, tail, 0.5, df * 0.5, true);
    if x < 0.0 { 1.0 - outside } else { outside }
}

fn student_density(x: f64, df: f64) -> f64 {
    let half = df / 2.0;
    let slope = (x / df.sqrt()).powi(2).ln_1p();
    (-ln_beta(0.5, half) - 0.5 * df.ln() - (half + 0.5) * slope).exp()
}

fn student_quantile_upper(p: f64, df: f64) -> f64 {
    if df == 1.0 {
        // The Cauchy inverse has simple forms on either side of the quarter point.
        return if p < 0.25 { (PI * p).tan().recip() } else { (PI * (0.5 - p)).tan() };
    }
    if p == 0.5 { 0.0 } else { invert(|x| student_right(x, df), p, 0.0, 1.0, QUANTILE_LIMIT) }
}

fn degrees(a: &[Arg], index: usize) -> R<f64> {
    let df = num(a, index)?.trunc();
    if df >= 1.0 { Ok(df) } else { Err(CellError::Num) }
}

pub(super) fn t_dist(a: &[Arg], _: C) -> R<Value> {
    let (x, df) = (num(a, 0)?, degrees(a, 1)?);
    num_val(if boolean(a, 2)? { student_right(-x, df) } else { student_density(x, df) })
}

pub(super) fn t_dist_rt(a: &[Arg], _: C) -> R<Value> {
    num_val(student_right(num(a, 0)?, degrees(a, 1)?))
}

pub(super) fn t_dist_2t(a: &[Arg], _: C) -> R<Value> {
    let (x, df) = (num(a, 0)?, degrees(a, 1)?);
    if x < 0.0 {
        return Err(CellError::Num);
    }
    num_val(2.0 * student_right(x, df))
}

fn tail_count(a: &[Arg], index: usize) -> R<f64> {
    let count = num(a, index)?.trunc();
    if count == 1.0 || count == 2.0 { Ok(count) } else { Err(CellError::Num) }
}

pub(super) fn tdist_legacy(a: &[Arg], _: C) -> R<Value> {
    let (x, df, tails) = (num(a, 0)?, degrees(a, 1)?, tail_count(a, 2)?);
    if x < 0.0 {
        return Err(CellError::Num);
    }
    num_val(tails * student_right(x, df))
}

pub(super) fn t_inv(a: &[Arg], _: C) -> R<Value> {
    let (p, df) = (num(a, 0)?, degrees(a, 1)?);
    if !open_probability(p) {
        return Err(CellError::Num);
    }
    let magnitude = student_quantile_upper(p.min(1.0 - p), df);
    num_val(magnitude.copysign(p - 0.5))
}

pub(super) fn t_inv_2t(a: &[Arg], _: C) -> R<Value> {
    let (p, df) = (num(a, 0)?, degrees(a, 1)?);
    if !(p > 0.0 && p <= 1.0) {
        return Err(CellError::Num);
    }
    num_val(student_quantile_upper(p / 2.0, df))
}

fn gamma_density(x: f64, shape: f64, scale: f64) -> f64 {
    if x == 0.0 {
        return if shape == 1.0 {
            scale.recip()
        } else if shape > 1.0 {
            0.0
        } else {
            f64::INFINITY
        };
    }
    let log_z = x.ln() - scale.ln();
    ((shape - 1.0) * log_z - x / scale - ln_gamma(shape) - scale.ln()).exp()
}

fn gamma_quantile(p: f64, shape: f64, scale: f64, upper: bool) -> f64 {
    let anchor = (shape * scale).clamp(1e-300, QUANTILE_LIMIT);
    invert(
        |x| {
            if upper { reg_gamma_q(shape, x / scale) } else { reg_gamma_p(shape, x / scale) }
        },
        p,
        0.0,
        anchor,
        QUANTILE_LIMIT,
    )
}

pub(super) fn gamma_dist(a: &[Arg], _: C) -> R<Value> {
    let (x, shape, scale, cumulative) = (num(a, 0)?, num(a, 1)?, num(a, 2)?, boolean(a, 3)?);
    if x < 0.0 || shape <= 0.0 || scale <= 0.0 {
        return Err(CellError::Num);
    }
    num_val(if cumulative { reg_gamma_p(shape, x / scale) } else { gamma_density(x, shape, scale) })
}

pub(super) fn gamma_inv(a: &[Arg], _: C) -> R<Value> {
    let (p, shape, scale) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
    if !(0.0..1.0).contains(&p) || shape <= 0.0 || scale <= 0.0 {
        return Err(CellError::Num);
    }
    num_val(if p > 0.5 { gamma_quantile(1.0 - p, shape, scale, true) } else { gamma_quantile(p, shape, scale, false) })
}

pub(super) fn gamma_fn(a: &[Arg], _: C) -> R<Value> {
    num_val(gamma(num(a, 0)?))
}

pub(super) fn gammaln(a: &[Arg], _: C) -> R<Value> {
    let x = num(a, 0)?;
    if x <= 0.0 {
        return Err(CellError::Num);
    }
    num_val(ln_gamma(x))
}

pub(super) fn chisq_dist(a: &[Arg], _: C) -> R<Value> {
    let (x, df, cumulative) = (num(a, 0)?, degrees(a, 1)?, boolean(a, 2)?);
    if x < 0.0 || df > 1e10 {
        return Err(CellError::Num);
    }
    num_val(if cumulative { reg_gamma_p(df / 2.0, x / 2.0) } else { gamma_density(x, df / 2.0, 2.0) })
}

pub(super) fn chisq_dist_rt(a: &[Arg], _: C) -> R<Value> {
    let (x, df) = (num(a, 0)?, degrees(a, 1)?);
    if x < 0.0 || df > 1e10 {
        return Err(CellError::Num);
    }
    num_val(reg_gamma_q(df / 2.0, x / 2.0))
}

fn chisq_quantile(a: &[Arg], upper: bool) -> R<Value> {
    let (p, df) = (num(a, 0)?, degrees(a, 1)?);
    if df > 1e10 || !(0.0..=1.0).contains(&p) || p == if upper { 0.0 } else { 1.0 } {
        return Err(CellError::Num);
    }
    let (p, upper) = if p > 0.5 { (1.0 - p, !upper) } else { (p, upper) };
    num_val(gamma_quantile(p, df / 2.0, 2.0, upper))
}

pub(super) fn chisq_inv(a: &[Arg], _: C) -> R<Value> {
    chisq_quantile(a, false)
}

pub(super) fn chisq_inv_rt(a: &[Arg], _: C) -> R<Value> {
    chisq_quantile(a, true)
}

fn f_parameters(a: &[Arg]) -> R<(f64, f64, f64)> {
    let (value, d1, d2) = (num(a, 0)?, num(a, 1)?.trunc(), num(a, 2)?.trunc());
    if !(1.0..1e10).contains(&d1) || !(1.0..1e10).contains(&d2) {
        return Err(CellError::Num);
    }
    Ok((value, d1 * 0.5, d2 * 0.5))
}

fn fisher_probability(x: f64, first: f64, second: f64, upper: bool) -> f64 {
    let (head, tail) = beta_coordinates(x * (first / second));
    beta_tail(head, tail, first, second, upper)
}

fn fisher_density(x: f64, first: f64, second: f64) -> f64 {
    if x == 0.0 {
        return if first == 1.0 {
            1.0
        } else if first > 1.0 {
            0.0
        } else {
            f64::INFINITY
        };
    }
    let odds = x * (first / second);
    if odds.is_infinite() {
        return 0.0;
    }
    let log_tail = -odds.ln_1p();
    let log_head = if odds <= 1.0 { odds.ln() + log_tail } else { -(1.0 / odds).ln_1p() };
    (first * log_head + second * log_tail - ln_beta(first, second) - x.ln()).exp()
}

pub(super) fn f_dist(a: &[Arg], _: C) -> R<Value> {
    let (x, first, second) = f_parameters(a)?;
    let cumulative = boolean(a, 3)?;
    if x < 0.0 {
        return Err(CellError::Num);
    }
    num_val(if cumulative { fisher_probability(x, first, second, false) } else { fisher_density(x, first, second) })
}

pub(super) fn f_dist_rt(a: &[Arg], _: C) -> R<Value> {
    let (x, first, second) = f_parameters(a)?;
    if x < 0.0 {
        return Err(CellError::Num);
    }
    num_val(fisher_probability(x, first, second, true))
}

fn fisher_quantile(a: &[Arg], upper: bool) -> R<Value> {
    let (p, first, second) = f_parameters(a)?;
    if !(0.0..=1.0).contains(&p) || p == if upper { 0.0 } else { 1.0 } {
        return Err(CellError::Num);
    }
    let (p, upper) = if p > 0.5 { (1.0 - p, !upper) } else { (p, upper) };
    num_val(invert(|x| fisher_probability(x, first, second, upper), p, 0.0, 1.0, QUANTILE_LIMIT))
}

pub(super) fn f_inv(a: &[Arg], _: C) -> R<Value> {
    fisher_quantile(a, false)
}

pub(super) fn f_inv_rt(a: &[Arg], _: C) -> R<Value> {
    fisher_quantile(a, true)
}

fn beta_bounds(a: &[Arg], bounds_at: usize) -> R<(f64, f64)> {
    let lo = if has(a, bounds_at) { num(a, bounds_at)? } else { 0.0 };
    let hi = if has(a, bounds_at + 1) { num(a, bounds_at + 1)? } else { 1.0 };
    if lo >= hi {
        return Err(CellError::Num);
    }
    Ok((lo, hi))
}

fn beta_distribution(x: f64, shape: f64, other: f64, (lo, hi): (f64, f64), cumulative: bool) -> R<Value> {
    if shape <= 0.0 || other <= 0.0 || !(lo..=hi).contains(&x) {
        return Err(CellError::Num);
    }
    let width = hi - lo;
    let (z, complement) = ((x - lo) / width, (hi - x) / width);
    if cumulative {
        return num_val(beta_tail(z, complement, shape, other, false));
    }
    if z == 0.0 || complement == 0.0 {
        let (edge, opposite) = if z == 0.0 { (shape, other) } else { (other, shape) };
        return if edge < 1.0 { Err(CellError::Num) } else { num_val(if edge == 1.0 { opposite / width } else { 0.0 }) };
    }
    num_val(((shape - 1.0) * z.ln() + (other - 1.0) * complement.ln() - ln_beta(shape, other) - width.ln()).exp())
}

pub(super) fn beta_dist(a: &[Arg], _: C) -> R<Value> {
    let (x, shape, other, cumulative) = (num(a, 0)?, num(a, 1)?, num(a, 2)?, boolean(a, 3)?);
    beta_distribution(x, shape, other, beta_bounds(a, 4)?, cumulative)
}

pub(super) fn betadist_legacy(a: &[Arg], _: C) -> R<Value> {
    let (x, shape, other) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
    beta_distribution(x, shape, other, beta_bounds(a, 3)?, true)
}

pub(super) fn beta_inv(a: &[Arg], _: C) -> R<Value> {
    let (p, shape, other) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
    let (lo, hi) = beta_bounds(a, 3)?;
    if shape <= 0.0 || other <= 0.0 || !(p > 0.0 && p <= 1.0) {
        return Err(CellError::Num);
    }
    let result = if p <= 0.5 {
        lo + (hi - lo) * invert(|z| reg_inc_beta(z, shape, other), p, 0.0, 1.0, 1.0)
    } else {
        hi - (hi - lo) * invert(|z| reg_inc_beta(z, other, shape), 1.0 - p, 0.0, 1.0, 1.0)
    };
    num_val(result)
}

pub(super) fn expon_dist(a: &[Arg], _: C) -> R<Value> {
    let (x, rate, cumulative) = (num(a, 0)?, num(a, 1)?, boolean(a, 2)?);
    if x < 0.0 || rate <= 0.0 {
        return Err(CellError::Num);
    }
    let log_survival = -(x * rate);
    num_val(if cumulative { -log_survival.exp_m1() } else { rate * log_survival.exp() })
}

pub(super) fn weibull_dist(a: &[Arg], _: C) -> R<Value> {
    let (x, shape, scale, cumulative) = (num(a, 0)?, num(a, 1)?, num(a, 2)?, boolean(a, 3)?);
    if x < 0.0 || shape <= 0.0 || scale <= 0.0 {
        return Err(CellError::Num);
    }
    let log_z = x.ln() - scale.ln();
    let power = (shape * log_z).exp();
    if cumulative {
        return num_val(-(-power).exp_m1());
    }
    if x == 0.0 {
        return num_val(gamma_density(x, shape, scale));
    }
    if power.is_infinite() {
        let factor = (shape / scale.powf(shape)) * x.powf(shape - 1.0);
        return num_val(factor * 0.0);
    }
    num_val((shape.ln() - scale.ln() + (shape - 1.0) * log_z - power).exp())
}

fn log_combinations(n: f64, k: f64) -> f64 {
    let small = k.min(n - k);
    if small <= 64.0 {
        // Summing the short product avoids subtracting nearly equal log gammas.
        return (1..=small as u32)
            .map(|j| {
                let j = f64::from(j);
                ((n - small + j) / j).ln()
            })
            .sum();
    }
    -(n + 1.0).ln() - ln_beta(k + 1.0, n - k + 1.0)
}

fn binomial_mass(k: f64, trials: f64, p: f64) -> f64 {
    if p == 0.0 || p == 1.0 {
        return if k == p * trials { 1.0 } else { 0.0 };
    }
    (log_combinations(trials, k) + k * p.ln() + (trials - k) * (-p).ln_1p()).exp()
}

fn binomial_tail(k: f64, trials: f64, p: f64, upper: bool) -> f64 {
    if k < 0.0 || k >= trials {
        return if (k < 0.0) == upper { 1.0 } else { 0.0 };
    }
    // The survival probability is I_p(k+1, n-k), DLMF 8.17.5.
    beta_tail(p, 1.0 - p, k + 1.0, trials - k, !upper)
}

pub(super) fn binom_dist(a: &[Arg], _: C) -> R<Value> {
    let (k, n, p) = (num(a, 0)?.trunc(), num(a, 1)?.trunc(), num(a, 2)?);
    let cumulative = boolean(a, 3)?;
    if k < 0.0 || k > n || !(0.0..=1.0).contains(&p) {
        return Err(CellError::Num);
    }
    num_val(if cumulative { binomial_tail(k, n, p, false) } else { binomial_mass(k, n, p) })
}

pub(super) fn binom_dist_range(a: &[Arg], _: C) -> R<Value> {
    let (n, p, first) = (num(a, 0)?.trunc(), num(a, 1)?, num(a, 2)?.trunc());
    let last = if has(a, 3) { num(a, 3)?.trunc() } else { first };
    if !(0.0..=1e15).contains(&n) || !(0.0..=1.0).contains(&p) || first < 0.0 || first > last || last > n {
        return Err(CellError::Num);
    }
    if first == last {
        return num_val(binomial_mass(first, n, p));
    }
    let before = binomial_tail(first - 1.0, n, p, false);
    let range = if before <= 0.5 {
        binomial_tail(last, n, p, false) - before
    } else {
        binomial_tail(first - 1.0, n, p, true) - binomial_tail(last, n, p, true)
    };
    num_val(range.clamp(0.0, 1.0))
}

pub(super) fn binom_inv(a: &[Arg], _: C) -> R<Value> {
    let (n, p, alpha) = (num(a, 0)?.trunc(), num(a, 1)?, num(a, 2)?);
    if n < 0.0 || !open_probability(p) || !open_probability(alpha) {
        return Err(CellError::Num);
    }
    // At the median of the symmetric distribution an exact CDF tie can
    // round to either side of one half. Its lower median is floor(n / 2).
    if p == 0.5 && alpha == 0.5 {
        return num_val((n / 2.0).floor());
    }
    // Keep a known failing and a known passing integer. On large numbers use
    // progress in representable f64 values as well as the integer stopping rule.
    let mut lower = -1.0;
    let mut upper = n;
    while upper - lower > 1.0 {
        let mid = (lower + (upper - lower) * 0.5).floor();
        if mid <= lower || mid >= upper {
            break;
        }
        if binomial_tail(mid, n, p, false) >= alpha {
            upper = mid;
        } else {
            lower = mid;
        }
    }
    num_val(upper)
}

pub(super) fn poisson_dist(a: &[Arg], _: C) -> R<Value> {
    let (k, rate, cumulative) = (num(a, 0)?.trunc(), num(a, 1)?, boolean(a, 2)?);
    if k < 0.0 || rate < 0.0 {
        return Err(CellError::Num);
    }
    if cumulative {
        return num_val(reg_gamma_q(k + 1.0, rate));
    }
    if rate == 0.0 {
        return num_val(if k == 0.0 { 1.0 } else { 0.0 });
    }
    num_val((k * rate.ln() - rate - ln_gamma(k + 1.0)).exp())
}

pub(super) fn negbinom_dist(a: &[Arg], _: C) -> R<Value> {
    let (failures, successes, p) = (num(a, 0)?.trunc(), num(a, 1)?.trunc(), num(a, 2)?);
    let cumulative = optional_flag(a, 3, false)?;
    if failures < 0.0 || successes < 1.0 || !(0.0..=1.0).contains(&p) {
        return Err(CellError::Num);
    }
    if cumulative {
        return num_val(reg_inc_beta(p, successes, failures + 1.0));
    }
    if p == 0.0 || p == 1.0 {
        return num_val(if p == 1.0 && failures == 0.0 { 1.0 } else { 0.0 });
    }
    let arrangements = log_combinations(failures + (successes - 1.0), failures);
    num_val((arrangements + successes * p.ln() + failures * (-p).ln_1p()).exp())
}

pub(super) fn hypgeom_dist(a: &[Arg], _: C) -> R<Value> {
    let (hits, draws, marked, population) = (num(a, 0)?.trunc(), num(a, 1)?.trunc(), num(a, 2)?.trunc(), num(a, 3)?.trunc());
    let cumulative = optional_flag(a, 4, false)?;
    let misses = population - marked;
    let first = (draws - misses).max(0.0);
    let last = draws.min(marked);
    if draws <= 0.0 || marked <= 0.0 || draws > population || marked > population || !(first..=last).contains(&hits) {
        return Err(CellError::Num);
    }
    let mass = |k: f64| (log_combinations(marked, k) + log_combinations(misses, draws - k) - log_combinations(population, draws)).exp();
    if !cumulative {
        return num_val(mass(hits));
    }
    if hits - first > 1e6 || hits + 1.0 == hits {
        return Err(CellError::Num);
    }
    if hits == last {
        return num_val(1.0);
    }

    // Start at the largest term in the requested interval, then use adjacent
    // mass ratios in each direction so an underflow at an end cannot erase the sum.
    let mode = ((draws + 1.0) * ((marked + 1.0) / (population + 2.0))).floor();
    let pivot = mode.clamp(first, hits);
    let peak = mass(pivot);
    let mut total = peak;
    let mut term = peak;
    let mut k = pivot;
    while k > first {
        term *= (k / (marked - k + 1.0)) * ((misses - (draws - k)) / (draws - k + 1.0));
        total += term;
        k -= 1.0;
    }
    term = peak;
    k = pivot;
    while k < hits {
        term *= ((marked - k) / (k + 1.0)) * ((draws - k) / (misses - (draws - k) + 1.0));
        total += term;
        k += 1.0;
    }
    num_val(total.min(1.0))
}

fn confidence_parameters(a: &[Arg]) -> R<(f64, f64, f64)> {
    let (alpha, sd, n) = (num(a, 0)?, num(a, 1)?, num(a, 2)?.trunc());
    if !open_probability(alpha) || sd <= 0.0 || n < 1.0 {
        return Err(CellError::Num);
    }
    Ok((alpha * 0.5, sd / n.sqrt(), n))
}

pub(super) fn confidence_norm(a: &[Arg], _: C) -> R<Value> {
    let (tail, standard_error, _) = confidence_parameters(a)?;
    num_val(normal_quantile(1.0 - tail) * standard_error)
}

pub(super) fn confidence_t(a: &[Arg], _: C) -> R<Value> {
    let (tail, standard_error, n) = confidence_parameters(a)?;
    if n == 1.0 {
        return Err(CellError::Div0);
    }
    num_val(student_quantile_upper(tail, n - 1.0) * standard_error)
}

fn t_statistic(a: &[Arg], kind: f64) -> R<(f64, f64)> {
    if kind == 1.0 {
        let (left, right) = arg_pairs(a, 0, 1)?;
        let differences: Vec<f64> = left.into_iter().zip(right).map(|(x, y)| x - y).collect();
        let n = differences.len() as f64;
        let standard_error = (var_s(&differences)? / n).sqrt();
        if standard_error == 0.0 {
            return Err(CellError::Div0);
        }
        return Ok((mean(&differences)? / standard_error, n - 1.0));
    }
    let left = arg_nums(a, 0)?;
    let right = arg_nums(a, 1)?;
    let (v_left, v_right) = (var_s(&left)?, var_s(&right)?);
    let (n_left, n_right) = (left.len() as f64, right.len() as f64);
    let (df_left, df_right) = (n_left - 1.0, n_right - 1.0);
    let (variance, df) = if kind == 2.0 {
        let df = df_left + df_right;
        let pooled = (df_left * v_left + df_right * v_right) / df;
        (pooled / n_left + pooled / n_right, df)
    } else {
        let (s_left, s_right) = (v_left / n_left, v_right / n_right);
        let variance = s_left + s_right;
        let df = ((s_left / variance).powi(2) / df_left + (s_right / variance).powi(2) / df_right).recip();
        (variance, df)
    };
    if variance == 0.0 {
        return Err(CellError::Div0);
    }
    Ok(((mean(&left)? - mean(&right)?) / variance.sqrt(), df))
}

pub(super) fn t_test(a: &[Arg], _: C) -> R<Value> {
    let (tails, kind) = (num(a, 2)?.trunc(), num(a, 3)?.trunc());
    if !(tails == 1.0 || tails == 2.0) || !(1.0..=3.0).contains(&kind) {
        return Err(CellError::Num);
    }
    let (statistic, df) = t_statistic(a, kind)?;
    num_val(tails * student_right(statistic.abs(), df))
}

pub(super) fn f_test(a: &[Arg], _: C) -> R<Value> {
    let (left, right) = (arg_nums(a, 0)?, arg_nums(a, 1)?);
    let (v_left, v_right) = (var_s(&left)?, var_s(&right)?);
    if v_left == 0.0 || v_right == 0.0 {
        return Err(CellError::Div0);
    }
    let x = v_left / v_right;
    let (first, second) = ((left.len() - 1) as f64 * 0.5, (right.len() - 1) as f64 * 0.5);
    let lower = fisher_probability(x, first, second, false);
    let upper = fisher_probability(x, first, second, true);
    num_val((2.0 * lower.min(upper)).min(1.0))
}

pub(super) fn chisq_test(a: &[Arg], _: C) -> R<Value> {
    let actual = as_array(&arg(a, 0)?.value);
    let expected = as_array(&arg(a, 1)?.value);
    if (actual.rows, actual.cols) != (expected.rows, expected.cols) {
        return Err(CellError::NA);
    }
    let statistic = actual.data.iter().zip(&expected.data).try_fold(0.0, |sum, (actual, expected)| match (actual, expected) {
        (Value::Error(error), _) | (_, Value::Error(error)) => Err(*error),
        (Value::Number(_), Value::Number(expected)) if *expected == 0.0 => Err(CellError::Div0),
        (Value::Number(actual), Value::Number(expected)) => Ok(sum + (actual - expected).powi(2) / expected),
        _ => Ok(sum),
    })?;
    let df = if actual.rows == 1 || actual.cols == 1 {
        actual.data.len().saturating_sub(1)
    } else {
        actual.rows.saturating_sub(1) * actual.cols.saturating_sub(1)
    };
    if df == 0 {
        return Err(CellError::NA);
    }
    num_val(reg_gamma_q(df as f64 * 0.5, statistic * 0.5))
}

pub(super) fn z_test(a: &[Arg], _: C) -> R<Value> {
    let sample = arg_nums(a, 0)?;
    let center = num(a, 1)?;
    if sample.is_empty() {
        return Err(CellError::NA);
    }
    let supplied = has(a, 2);
    let sd = if supplied { num(a, 2)? } else { var_s(&sample)?.sqrt() };
    if sd <= 0.0 {
        return Err(if supplied { CellError::Num } else { CellError::Div0 });
    }
    let z = (mean(&sample)? - center) / (sd / (sample.len() as f64).sqrt());
    num_val(norm_cdf(-z))
}

pub(super) fn fisher(a: &[Arg], _: C) -> R<Value> {
    let correlation = num(a, 0)?;
    if correlation <= -1.0 || correlation >= 1.0 {
        return Err(CellError::Num);
    }
    num_val(((1.0 + correlation) / (1.0 - correlation)).ln() * 0.5)
}

pub(super) fn gauss(a: &[Arg], _: C) -> R<Value> {
    num_val(erf(num(a, 0)? / std::f64::consts::SQRT_2) * 0.5)
}
