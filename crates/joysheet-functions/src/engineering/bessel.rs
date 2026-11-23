//! First-party integer-order Bessel functions (`BESSELJ`, `BESSELY`, `BESSELI`, `BESSELK`).
//!
//! Implemented from standard DLMF §10 / Abramowitz & Stegun formulations:
//! - `J_n(x)`: Hankel asymptotic expansion for `|x| > 35 + 2n`, Miller's backward
//!   recurrence for `|x|, n <= 100`, and half-period even-symmetry trapezoid quadrature
//!   for intermediate large `(x, n)`.
//! - `Y_n(x)`: Neumann series over Miller's `J_k(x)` for `x <= 28`, Hankel asymptotic
//!   expansion for `x > 28`, followed by stable upward recurrence.
//! - `I_n(x)`: Log-scaled positive power series with periodic rescaling to prevent
//!   spurious intermediate overflow at large `(x, n)`.
//! - `K_n(x)`: Exponentially scaled `e^x K_0(x), e^x K_1(x)` via asymptotic series
//!   for `x > 35` and `2 x sinh^2(t/2)` trapezoid quadrature for `0 < x <= 35`,
//!   then upward recurrence before unscaling via `(ln(scaled) - x).exp()`.

use super::*;

const MAX_BESSEL_ORDER: f64 = 100_000.0;
const MAX_QUADRATURE_POINTS: f64 = 2_000_000.0;
const EULER_GAMMA: f64 = std::f64::consts::EULER_GAMMA;
const RESCALE_LIMIT: f64 = 1e200;
const RESCALE_INV: f64 = 1e-200;
const RESCALE_LN: f64 = 200.0 * std::f64::consts::LN_10;

/// Hankel asymptotic expansion for `(J_n(x), Y_n(x))` at large positive `x`.
fn hankel_jy(n: u32, x: f64) -> Option<(f64, f64)> {
    let mu = 4.0 * (n as f64) * (n as f64);
    let mut p = 0.0;
    let mut q = 0.0;
    let mut term = 1.0f64;
    let mut prev_mag = f64::INFINITY;
    for k in 0..200u32 {
        let mag = term.abs();
        if mag > prev_mag {
            break;
        }
        let signed = if k % 4 < 2 { term } else { -term };
        if k.is_multiple_of(2) {
            p += signed;
        } else {
            q += signed;
        }
        prev_mag = mag;
        if mag < 1e-17 {
            break;
        }
        let step = f64::from(k + 1);
        let odd = 2.0 * step - 1.0;
        term *= (mu - odd * odd) / (step * 8.0 * x);
    }
    let phase = x - (f64::from(n) * 0.5 + 0.25) * PI;
    let amp = (2.0 / (PI * x)).sqrt();
    let j = amp * (p * phase.cos() - q * phase.sin());
    let y = amp * (p * phase.sin() + q * phase.cos());
    (j.is_finite() && y.is_finite()).then_some((j, y))
}

/// Miller's backward recurrence for `J_0(x)..J_m(x)` (`x > 0`), normalized by
/// `J_0(x) + 2 Σ_{k>=1} J_{2k}(x) = 1`.
fn miller_j_table(x: f64, min_order: usize) -> Vec<f64> {
    let base = (x.ceil() as usize).max(min_order);
    let extra = 35 + (40.0 * x.max(1.0)).sqrt() as usize;
    let top = base + extra;
    let m = top + (top & 1);
    let mut j = vec![0.0f64; m + 2];
    if let Some(slot) = j.get_mut(m) {
        *slot = 1e-30;
    }
    for k in (1..=m).rev() {
        let jk = j.get(k).copied().unwrap_or(0.0);
        let jk1 = j.get(k + 1).copied().unwrap_or(0.0);
        let mut prev = (2.0 * k as f64 / x) * jk - jk1;
        if prev.abs() > 1e250 {
            for val in &mut j {
                *val *= 1e-250;
            }
            prev *= 1e-250;
        }
        if let Some(slot) = j.get_mut(k - 1) {
            *slot = prev;
        }
    }
    let norm = j.first().copied().unwrap_or(0.0) + 2.0 * j.iter().skip(2).step_by(2).sum::<f64>();
    if norm != 0.0 && norm.is_finite() {
        for val in &mut j {
            *val /= norm;
        }
    }
    j
}

/// Half-period Bessel integral `J_n(x) = (1/π) ∫_0^π cos(n t - x sin t) dt`.
fn bessel_j_quadrature(n: u32, x: f64) -> Option<f64> {
    let steps = (x + f64::from(n) + 32.0).ceil();
    if steps > MAX_QUADRATURE_POINTS {
        return None;
    }
    let m = steps as usize;
    let h = PI / m as f64;
    let nf = f64::from(n);
    // Endpoints at t = 0 (cos 0 = 1) and t = pi (cos(n*pi) = (-1)^n).
    let mut sum = 0.5 * (1.0 + if n.is_multiple_of(2) { 1.0 } else { -1.0 });
    for k in 1..m {
        let t = k as f64 * h;
        sum += (nf * t - x * t.sin()).cos();
    }
    let val = sum / m as f64;
    val.is_finite().then_some(val)
}

fn bessel_j(n: u32, x: f64) -> Option<f64> {
    if x == 0.0 {
        return Some(if n == 0 { 1.0 } else { 0.0 });
    }
    let ax = x.abs();
    let mag = if ax > 35.0 + 2.0 * f64::from(n) {
        hankel_jy(n, ax).map(|(j, _)| j)
    } else if ax <= 100.0 && n <= 100 {
        miller_j_table(ax, n as usize).get(n as usize).copied().filter(|v| v.is_finite())
    } else {
        bessel_j_quadrature(n, ax).or_else(|| (ax > 1e6).then(|| hankel_jy(n, ax).map(|(j, _)| j)).flatten())
    }?;
    Some(if x < 0.0 && !n.is_multiple_of(2) { -mag } else { mag })
}

fn bessel_y01(x: f64) -> Option<(f64, f64)> {
    if x > 28.0 {
        let (_, y0) = hankel_jy(0, x)?;
        let (_, y1) = hankel_jy(1, x)?;
        return Some((y0, y1));
    }
    let j = miller_j_table(x, 1);
    let at = |idx: usize| j.get(idx).copied().unwrap_or(0.0);
    let log_half_x = (0.5 * x).ln() + EULER_GAMMA;
    let mut s0 = 0.0;
    let mut s1 = 0.0;
    let mut k = 1usize;
    while 2 * k + 1 < j.len() {
        let sign = if k.is_multiple_of(2) { 1.0 } else { -1.0 };
        let inv_k = sign / k as f64;
        s0 += inv_k * at(2 * k);
        s1 += inv_k * (at(2 * k - 1) - at(2 * k + 1));
        k += 1;
    }
    let two_over_pi = 2.0 / PI;
    let y0 = two_over_pi * (log_half_x * at(0) - 2.0 * s0);
    let y1 = two_over_pi * (log_half_x * at(1) - at(0) / x + s1);
    (y0.is_finite() && y1.is_finite()).then_some((y0, y1))
}

fn bessel_y(n: u32, x: f64) -> Option<f64> {
    let (y0, y1) = bessel_y01(x)?;
    if n == 0 {
        return Some(y0);
    }
    let (mut prev, mut curr) = (y0, y1);
    for k in 1..n {
        let next = (2.0 * f64::from(k) / x) * curr - prev;
        if !next.is_finite() {
            return None;
        }
        prev = curr;
        curr = next;
    }
    Some(curr)
}

fn bessel_i(n: u32, x: f64) -> Option<f64> {
    if x == 0.0 {
        return Some(if n == 0 { 1.0 } else { 0.0 });
    }
    let ax = x.abs();
    let nf = f64::from(n);
    let ln_t0 = nf * (0.5 * ax).ln() - crate::special::ln_gamma(nf + 1.0);
    let q = 0.25 * ax * ax;
    let mut term = 1.0f64;
    let mut sum = 1.0f64;
    let mut scale_ln = 0.0f64;
    for k in 1..=1_000_000u32 {
        let kf = f64::from(k);
        term *= q / (kf * (kf + nf));
        sum += term;
        if sum > RESCALE_LIMIT {
            sum *= RESCALE_INV;
            term *= RESCALE_INV;
            scale_ln += RESCALE_LN;
        }
        if !sum.is_finite() {
            return None;
        }
        if term < 1e-17 * sum {
            break;
        }
    }
    let mag = (ln_t0 + scale_ln + sum.ln()).exp();
    let val = if x < 0.0 && !n.is_multiple_of(2) { -mag } else { mag };
    val.is_finite().then_some(val)
}

/// Overflow-free `ln(cosh(u))`.
fn ln_cosh(u: f64) -> f64 {
    let a = u.abs();
    a + (-2.0 * a).exp().ln_1p() - std::f64::consts::LN_2
}

/// Asymptotic expansion for `e^x K_nu(x)` (`nu` in `{0, 1}`, `x > 35`).
fn scaled_k_asymptotic(nu: f64, x: f64) -> f64 {
    let mu = 4.0 * nu * nu;
    let mut sum = 1.0f64;
    let mut term = 1.0f64;
    let mut prev_mag = f64::INFINITY;
    for k in 1..120u32 {
        let kf = f64::from(k);
        let odd = 2.0 * kf - 1.0;
        let next = term * (mu - odd * odd) / (kf * 8.0 * x);
        let mag = next.abs();
        if mag > prev_mag {
            break;
        }
        sum += next;
        term = next;
        prev_mag = mag;
        if mag < 1e-17 * sum.abs() {
            break;
        }
    }
    (PI / (2.0 * x)).sqrt() * sum
}

/// Exponentially scaled `e^x K_nu(x)` for `0 < x <= 35` via
/// `∫_0^∞ exp(-2 x sinh^2(t/2)) cosh(nu t) dt`.
fn scaled_k_quadrature(nu: f64, x: f64) -> f64 {
    let h = 0.02f64;
    let mut sum = 0.5f64;
    let mut t = h;
    while t < 800.0 {
        let sh = (0.5 * t).sinh();
        let decay = 2.0 * x * sh * sh;
        let exponent = -decay + if nu == 0.0 { 0.0 } else { ln_cosh(nu * t) };
        let term = exponent.exp();
        sum += term;
        if decay > 50.0 && term < 1e-18 * sum {
            break;
        }
        t += h;
    }
    sum * h
}

fn bessel_k(n: u32, x: f64) -> Option<f64> {
    let (mut prev, mut curr) = if x > 35.0 {
        (scaled_k_asymptotic(0.0, x), scaled_k_asymptotic(1.0, x))
    } else {
        (scaled_k_quadrature(0.0, x), scaled_k_quadrature(1.0, x))
    };
    if n == 0 {
        let val = (prev.ln() - x).exp();
        return val.is_finite().then_some(val);
    }
    let mut scale_ln = 0.0f64;
    for k in 1..n {
        let next = prev + (2.0 * f64::from(k) / x) * curr;
        prev = curr;
        curr = next;
        if curr > RESCALE_LIMIT {
            prev *= RESCALE_INV;
            curr *= RESCALE_INV;
            scale_ln += RESCALE_LN;
        }
        if !curr.is_finite() {
            return None;
        }
    }
    let val = (curr.ln() + scale_ln - x).exp();
    val.is_finite().then_some(val)
}

fn bessel_args(args: &[Arg]) -> R<(f64, u32)> {
    let x = num(args, 0)?;
    let n = num(args, 1)?.trunc();
    if !(0.0..=MAX_BESSEL_ORDER).contains(&n) {
        return Err(CellError::Num);
    }
    Ok((x, n as u32))
}

pub(super) fn besselj(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    let (x, n) = bessel_args(args)?;
    num_val(bessel_j(n, x).ok_or(CellError::Num)?)
}

pub(super) fn bessely(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    let (x, n) = bessel_args(args)?;
    if x <= 0.0 {
        return Err(CellError::Num);
    }
    num_val(bessel_y(n, x).ok_or(CellError::Num)?)
}

pub(super) fn besseli(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    let (x, n) = bessel_args(args)?;
    num_val(bessel_i(n, x).ok_or(CellError::Num)?)
}

pub(super) fn besselk(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    let (x, n) = bessel_args(args)?;
    if x <= 0.0 {
        return Err(CellError::Num);
    }
    num_val(bessel_k(n, x).ok_or(CellError::Num)?)
}
