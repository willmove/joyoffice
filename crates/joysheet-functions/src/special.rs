//! Shared real-valued special functions.
//!
//! Gamma recurrences, reflection and Stirling correction follow DLMF 5.5 and 5.11.
//! Incomplete gamma and beta use DLMF 8.7.1, 8.9.2 and 8.17.22, evaluated with
//! scaled convergents (3.10). The normal kernel uses the error function (7.2 and
//! 7.6) and a bracketed Newton solve in the smaller tail.

use std::f64::consts::{LN_2, PI, SQRT_2};

const LOG_SQRT_TWO_PI: f64 = 0.918_938_533_204_672_7;
const MAX_TERMS: usize = 10_000;

fn stirling_correction(z: f64) -> f64 {
    // B_(2k)/(2k (2k-1)) for k=1..8.
    const BERNOULLI: [f64; 8] =
        [1.0 / 12.0, -1.0 / 360.0, 1.0 / 1260.0, -1.0 / 1680.0, 1.0 / 1188.0, -691.0 / 360_360.0, 1.0 / 156.0, -3617.0 / 122_400.0];
    let inverse = z.recip();
    BERNOULLI.iter().rev().fold(0.0, |acc, coefficient| coefficient + inverse.powi(2) * acc) * inverse
}

fn positive_log_gamma(mut x: f64) -> f64 {
    if x <= 15.0 && x.fract() == 0.0 {
        return factorial_before(x).ln();
    }
    let mut shift = 0.0;
    while x < 8.0 {
        shift += x.ln();
        x += 1.0;
    }
    (x - 0.5).mul_add(x.ln(), -x) + (LOG_SQRT_TWO_PI + stirling_correction(x) - shift)
}

fn factorial_before(integer: f64) -> f64 {
    (2..integer as u32).map(f64::from).product()
}

/// ln |Γ(x)|. NaN at non-positive integers and nonfinite inputs.
pub(crate) fn ln_gamma(x: f64) -> f64 {
    if !x.is_finite() || (x <= 0.0 && x.fract() == 0.0) {
        return f64::NAN;
    }
    if x > 0.0 {
        return positive_log_gamma(x);
    }
    let sine = (PI * (x - x.round())).sin().abs();
    PI.ln() - sine.ln() - positive_log_gamma(1.0 - x)
}

pub(crate) fn gamma(x: f64) -> f64 {
    if x > 0.0 && x <= 171.0 && x.fract() == 0.0 {
        return factorial_before(x);
    }
    let value = ln_gamma(x).exp();
    if x < 0.0 && x.floor().rem_euclid(2.0) == 1.0 { -value } else { value }
}

fn log_sum_ratio(other: f64, base: f64) -> f64 {
    let ratio = other / base;
    if ratio.is_infinite() { other.ln() - base.ln() } else { ratio.ln_1p() }
}

/// ln B(a, b), with the large terms cancelled symbolically before evaluation.
pub(crate) fn ln_beta(mut a: f64, mut b: f64) -> f64 {
    if !(a > 0.0 && b > 0.0 && a.is_finite() && b.is_finite()) {
        return f64::NAN;
    }
    let mut shift = 0.0;
    while a < 8.0 {
        shift += log_sum_ratio(b, a);
        a += 1.0;
    }
    while b < 8.0 {
        shift += log_sum_ratio(a, b);
        b += 1.0;
    }
    let (small, large) = if a < b { (a, b) } else { (b, a) };
    let log_ratio = (small / large).ln_1p();
    let scale = -large * log_ratio + small * ((small / large).ln() - log_ratio);
    scale + LOG_SQRT_TWO_PI + 0.5 * (log_ratio - small.ln()) + stirling_correction(a) + stirling_correction(b) - stirling_correction(a + b) + shift
}

/// Reciprocal of a continued fraction. Rescale each pair of adjacent convergents
/// together so the recurrence cannot overflow from increasing denominator size.
fn reciprocal_fraction(initial: f64, terms: impl Iterator<Item = (f64, f64)>) -> f64 {
    let scale = initial.abs().max(1.0).recip();
    let (mut older, mut current) = ((scale, 0.0), (initial * scale, scale));
    let mut previous = current.1 / current.0;
    let mut stable = false;
    for (numerator, denominator) in terms.take(MAX_TERMS) {
        let a = numerator.mul_add(older.0, denominator * current.0);
        let b = numerator.mul_add(older.1, denominator * current.1);
        let value = b / a;
        let close = value.is_finite() && (value - previous).abs() <= 4.0 * f64::EPSILON * value.abs();
        if close && stable {
            return value;
        }
        stable = close;
        previous = value;
        let scale = a.abs().max(b.abs()).max(f64::MIN_POSITIVE).recip();
        older = (current.0 * scale, current.1 * scale);
        current = (a * scale, b * scale);
    }
    previous
}

fn log1p_minus_x(x: f64) -> f64 {
    if x.abs() >= 0.03125 {
        return x.ln_1p() - x;
    }
    let mut power = -x * x;
    let mut sum = power * 0.5;
    for n in 3..=14 {
        power *= -x;
        sum += power / f64::from(n);
    }
    sum
}

fn gamma_log_weight(a: f64, x: f64) -> f64 {
    let delta = (x - a) / a;
    if a >= 8.0 && delta.abs() < 0.5 {
        return a * log1p_minus_x(delta) + 0.5 * a.ln() - LOG_SQRT_TWO_PI - stirling_correction(a);
    }
    a.mul_add(x.ln(), -x) - ln_gamma(a)
}

fn gamma_lower_log(a: f64, x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    for n in 1..=MAX_TERMS {
        term *= x / (a + n as f64);
        sum += term;
        if term <= f64::EPSILON * sum * 0.5 {
            break;
        }
    }
    gamma_log_weight(a, x) - a.ln() + sum.ln()
}

fn gamma_upper_log(a: f64, x: f64) -> f64 {
    let first = (x - a) + 1.0;
    let ratio = reciprocal_fraction(
        first,
        (1..).map(|index| {
            let n = f64::from(index);
            (n * (a - n), first + 2.0 * n)
        }),
    );
    gamma_log_weight(a, x) + ratio.ln()
}

fn tail_value(log_p: f64, complement: bool) -> f64 {
    let value = if complement { -log_p.exp_m1() } else { log_p.exp() };
    value.clamp(0.0, 1.0)
}

fn gamma_tail(a: f64, x: f64, upper: bool) -> f64 {
    if !(a.is_finite() && a > 0.0 && x >= 0.0) {
        return f64::NAN;
    }
    if x == 0.0 || x.is_infinite() {
        return if (x == 0.0) == upper { 1.0 } else { 0.0 };
    }
    if a == 1.0 {
        return tail_value(-x, !upper);
    }
    if x < a + 1.0 { tail_value(gamma_lower_log(a, x), upper) } else { tail_value(gamma_upper_log(a, x), !upper) }
}

pub(crate) fn reg_gamma_p(a: f64, x: f64) -> f64 {
    gamma_tail(a, x, false)
}

pub(crate) fn reg_gamma_q(a: f64, x: f64) -> f64 {
    gamma_tail(a, x, true)
}

pub(crate) fn erf(x: f64) -> f64 {
    let magnitude = x.abs();
    if magnitude < 0.5 {
        let square = -magnitude * magnitude;
        let series = (1..=12).rev().fold(1.0, |acc, i| {
            let n = f64::from(i);
            1.0 + square * (2.0 * n - 1.0) / (n * (2.0 * n + 1.0)) * acc
        });
        return (2.0 / PI.sqrt() * series) * x;
    }
    reg_gamma_p(0.5, x * x).copysign(x)
}

pub(crate) fn erfc(x: f64) -> f64 {
    if x.abs() < 0.5 {
        1.0 - erf(x)
    } else if x < 0.0 {
        1.0 + reg_gamma_p(0.5, x * x)
    } else {
        reg_gamma_q(0.5, x * x)
    }
}

pub(crate) fn reg_inc_beta(x: f64, a: f64, b: f64) -> f64 {
    beta_tail(x, 1.0 - x, a, b, false)
}

fn beta_fraction(a: f64, b: f64, x: f64) -> f64 {
    let odd = |n: f64| -x * ((a + n) / (a + 2.0 * n)) * ((a + b + n) / (a + 2.0 * n + 1.0));
    let even = |n: f64| x * (n / (a + 2.0 * n)) * ((b - n) / (a + 2.0 * n - 1.0));
    reciprocal_fraction(
        1.0,
        (1..).map(|index| {
            let n = f64::from(index / 2);
            (if index % 2 == 0 { even(n) } else { odd(n) }, 1.0)
        }),
    )
}

/// Accept both coordinates to preserve the smaller one when the larger rounds to 1.
pub(crate) fn beta_tail(x: f64, y: f64, a: f64, b: f64, upper: bool) -> f64 {
    if !(a > 0.0 && b > 0.0 && a.is_finite() && b.is_finite() && (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)) {
        return f64::NAN;
    }
    if x == 0.0 || y == 0.0 {
        return if (x == 0.0) == upper { 1.0 } else { 0.0 };
    }
    let (log_x, log_y) = if x < y { (x.ln(), (-x).ln_1p()) } else { ((-y).ln_1p(), y.ln()) };
    if a == 1.0 {
        return tail_value(b * log_y, !upper);
    }
    if b == 1.0 {
        return tail_value(a * log_x, upper);
    }
    if a == b && x == y {
        return 0.5;
    }
    let lower = x < (a + 1.0) / (a + b + 2.0);
    let (shape, other, coordinate) = if lower { (a, b, x) } else { (b, a, y) };
    let weight = a * log_x + b * log_y - ln_beta(a, b);
    let log_probability = weight + beta_fraction(shape, other, coordinate).ln() - shape.ln();
    tail_value(log_probability, lower == upper)
}

fn normal_log_density(z: f64) -> f64 {
    -0.5 * z * z - LOG_SQRT_TWO_PI
}

pub(crate) fn norm_pdf(z: f64) -> f64 {
    normal_log_density(z).exp()
}

pub(crate) fn norm_cdf(z: f64) -> f64 {
    erfc(-z / SQRT_2) * 0.5
}

fn normal_log_right(z: f64) -> f64 {
    let x = z * z * 0.5;
    if x >= 1.5 { gamma_upper_log(0.5, x) - LN_2 } else { norm_cdf(-z).ln() }
}

/// Invert the smaller tail, with Newton steps kept inside an analytic bracket.
/// The centre uses the signed area from 0 to avoid coalescing nearby probabilities.
pub(crate) fn norm_inv(p: f64) -> f64 {
    if !(p > 0.0 && p < 1.0) {
        return f64::NAN;
    }
    let small = p.min(1.0 - p);
    if small == 0.5 {
        return 0.0;
    }
    let near_centre = small >= 0.25;
    let log_target = small.ln();
    let area = 0.5 - small;
    let (mut low, mut high) = (0.0, if near_centre { 1.0 } else { (-2.0 * (log_target + LN_2)).sqrt() });
    let mut z = if near_centre { area / norm_pdf(0.0) } else { high - (high.ln() + LOG_SQRT_TWO_PI - LN_2) / high };
    for _ in 0..64 {
        let (offset, derivative) = if near_centre {
            (0.5 * erf(z / SQRT_2) - area, norm_pdf(z))
        } else {
            let log_right = normal_log_right(z);
            (log_target - log_right, (normal_log_density(z) - log_right).exp())
        };
        let newton = z - offset / derivative;
        if (newton - z).abs() <= (2.0 * f64::EPSILON) * z {
            break;
        }
        if offset < 0.0 {
            low = z;
        } else {
            high = z;
        }
        z = if low < newton && newton < high { newton } else { (low + high) * 0.5 };
    }
    z.copysign(p - 0.5)
}

/// Finds `x` in `[lo, hi]` with `f(x) = target` for a monotone `f` by bisection (expanding `hi`
/// up to `hi_max` while the target is not bracketed). Returns NaN when no root is bracketed.
pub(crate) fn invert(f: impl Fn(f64) -> f64, target: f64, lo: f64, hi: f64, hi_max: f64) -> f64 {
    let start = f(lo);
    if start == target {
        return lo;
    }
    if start.is_nan() {
        return f64::NAN;
    }
    let before = |value: f64| (value < target) == (start < target);
    let mut right = hi.min(hi_max);
    let mut left = lo;
    let value = f(right);
    if value.is_nan() {
        return f64::NAN;
    }
    if before(value) && value != target {
        loop {
            if right >= hi_max {
                return f64::NAN;
            }
            let next = (2.0 * right).min(hi_max);
            if next <= right {
                return f64::NAN;
            }
            left = right;
            right = next;
            let value = f(right);
            if value.is_nan() {
                return f64::NAN;
            }
            if !before(value) || value == target {
                break;
            }
        }
    } else {
        // Bracket small quantiles on the exponent scale before linear bisection.
        loop {
            let next = lo + (right - lo) * 0.5;
            if next <= lo {
                break;
            }
            let value = f(next);
            if value.is_nan() {
                return f64::NAN;
            }
            if before(value) && value != target {
                left = next;
                break;
            }
            right = next;
        }
    }
    loop {
        let mid = left + (right - left) * 0.5;
        if mid <= left || mid >= right {
            return mid;
        }
        let value = f(mid);
        if value.is_nan() {
            return f64::NAN;
        }
        if value == target {
            return mid;
        }
        if before(value) {
            left = mid;
        } else {
            right = mid;
        }
    }
}

#[cfg(test)]
#[path = "tests/special_tests.rs"]
mod tests;
