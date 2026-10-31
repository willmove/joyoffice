//! Decimal-string rounding and the General format.
//!
//! Excel works with 15 significant digits, so numbers are first written with 15 significant
//! digits and then rounded half away from zero on that decimal string. That makes 2.675 with
//! "0.00" show 2.68 and 1.005 show 1.01, as in Excel, even though neither is exact in binary.

/// Most decimals we ever compute; placeholders beyond this are padded with zeros.
pub(crate) const MAX_DECIMALS: usize = 400;

/// The 15 significant digits of a positive finite number and the power of ten of the first
/// digit: `a ≈ d0.d1d2… × 10^exp`.
///
/// The value is rounded to the nearest 15-digit decimal. An exact tie at the 16th digit (the exact
/// decimal expansion is 15 digits, a 5, then only zeros, as for 1234567890123455 or
/// 123456789012345.5) goes toward zero, as Excel does: `1234567890123455` shows `1234567890123450`,
/// not `…460`. Excel observed (Windows 16.0.20430, TEXT()).
pub(crate) fn decimal_digits(a: f64) -> ([u8; 15], i32) {
    let mut digits = [0u8; 15];
    if !a.is_finite() || a <= 0.0 {
        return (digits, 0);
    }
    // 16 digits, rounded. Unless the 16th is a 5 it decides the 15th exactly (4 or less rounds
    // down, 6 or more up), so one format serves almost every number.
    let (d16, exp) = significant_digits(a, 16);
    for (slot, d) in digits.iter_mut().zip(&d16) {
        *slot = *d;
    }
    match d16.get(15).copied().unwrap_or(0) {
        0..=4 => (digits, exp),
        5 if is_exact_tie(a) => (digits, exp),
        5 => {
            // Not a tie: round the exact value (the 16-digit rounding above may be a double rounding).
            let (d15, exp15) = significant_digits(a, 15);
            let mut out = [0u8; 15];
            for (slot, d) in out.iter_mut().zip(&d15) {
                *slot = *d;
            }
            (out, exp15)
        }
        _ => {
            let mut exp = exp;
            let mut carry = true;
            for d in digits.iter_mut().rev() {
                if *d == 9 {
                    *d = 0;
                } else {
                    *d += 1;
                    carry = false;
                    break;
                }
            }
            if carry {
                digits = [0; 15];
                digits[0] = 1;
                exp += 1;
            }
            (digits, exp)
        }
    }
}

/// `n` significant digits of `a` (rounded half to even on the exact binary value) and the power of
/// ten of the first.
fn significant_digits(a: f64, n: usize) -> (Vec<u8>, i32) {
    let s = format!("{:.*e}", n.saturating_sub(1), a);
    let (mant, exp) = s.split_once('e').unwrap_or((s.as_str(), "0"));
    (mant.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect(), exp.parse().unwrap_or(0))
}

/// True when `a`'s exact decimal expansion has a 5 as its 16th significant digit and only zeros after.
fn is_exact_tie(a: f64) -> bool {
    let is_tie = |ds: &[u8]| ds.get(15) == Some(&5) && ds.get(16..).is_some_and(|t| t.iter().all(|&d| d == 0));
    // A cheap screen first (30 digits), then the exact expansion: a double has at most 767
    // significant digits and Rust prints every one of them.
    is_tie(&significant_digits(a, 30).0) && is_tie(&significant_digits(a, 800).0)
}

/// Rounds the number `digits × 10^exp` (see [`decimal_digits`]) to `decimals` places, half away
/// from zero. Returns the integer digits ("" when the integer part is zero) and exactly
/// `decimals` fraction digits.
pub(crate) fn fixed_from_digits(digits: &[u8; 15], exp: i32, decimals: usize) -> (String, String) {
    let decimals_calc = decimals.min(MAX_DECIMALS);
    let keep = exp as i64 + decimals_calc as i64 + 1;
    let mut m: Vec<u8> = if keep <= 0 {
        if keep == 0 && digits.first().copied().unwrap_or(0) >= 5 { vec![1] } else { Vec::new() }
    } else if keep as usize >= digits.len() {
        let mut v = digits.to_vec();
        v.resize(keep as usize, 0);
        v
    } else {
        let keep = keep as usize;
        let mut v = digits.get(..keep).map(<[u8]>::to_vec).unwrap_or_default();
        if digits.get(keep).copied().unwrap_or(0) >= 5 {
            let mut carry = true;
            for d in v.iter_mut().rev() {
                if *d == 9 {
                    *d = 0;
                } else {
                    *d += 1;
                    carry = false;
                    break;
                }
            }
            if carry {
                v.insert(0, 1);
            }
        }
        v
    };
    let lead = m.iter().take_while(|&&d| d == 0).count();
    m.drain(..lead);
    let to_s = |ds: &[u8]| ds.iter().map(|d| char::from(b'0' + d)).collect::<String>();
    let (int, mut frac) = if m.len() <= decimals_calc {
        let mut f = "0".repeat(decimals_calc - m.len());
        f.push_str(&to_s(&m));
        (String::new(), f)
    } else {
        let split = m.len() - decimals_calc;
        (to_s(m.get(..split).unwrap_or(&[])), to_s(m.get(split..).unwrap_or(&[])))
    };
    if decimals > decimals_calc {
        frac.push_str(&"0".repeat(decimals - decimals_calc));
    }
    (int, frac)
}

/// Rounds a non-negative number to `decimals` places (see [`fixed_from_digits`]).
pub(crate) fn fixed(a: f64, decimals: usize) -> (String, String) {
    if !a.is_finite() || a <= 0.0 {
        return (String::new(), "0".repeat(decimals.min(MAX_DECIMALS * 4)));
    }
    let (d, e) = decimal_digits(a);
    fixed_from_digits(&d, e, decimals)
}

/// General format squeezed into `max_chars` characters. Excel shows fewer decimals, then
/// scientific notation, then "####" when a column is narrow; `None` means the caller paints "####".
pub fn format_general_fit(n: f64, max_chars: usize) -> Option<String> {
    if !n.is_finite() {
        return None;
    }
    if n == 0.0 {
        return (max_chars >= 1).then(|| "0".to_string());
    }
    let sign = if n < 0.0 { "-" } else { "" };
    let (digits, x) = decimal_digits(n.abs());
    let int_len = sign.len() + (x.max(0) as usize) + 1;
    // Fixed notation for magnitudes from 1E-4 up to what fits.
    if x >= -4 && int_len <= max_chars {
        let avail = max_chars - int_len;
        let mut decimals = if avail >= 2 { avail - 1 } else { 0 };
        loop {
            let (i, f) = fixed_from_digits(&digits, x, decimals);
            let f = f.trim_end_matches('0');
            if i.is_empty() && f.is_empty() {
                break; // rounded away to zero: use scientific
            }
            let mut s = String::from(sign);
            s.push_str(if i.is_empty() { "0" } else { &i });
            if !f.is_empty() {
                s.push('.');
                s.push_str(f);
            }
            if s.chars().count() <= max_chars {
                return Some(s);
            }
            if decimals == 0 {
                break;
            }
            decimals -= 1;
        }
    }
    for md in (0..=max_chars.min(14)).rev() {
        let mut e = x;
        let (mut mi, mut mf) = fixed_from_digits(&digits, 0, md);
        if mi.len() > 1 {
            e += 1;
            (mi, mf) = fixed_from_digits(&digits, -1, md);
        }
        let mf = mf.trim_end_matches('0');
        let mut s = String::from(sign);
        s.push_str(if mi.is_empty() { "0" } else { &mi });
        if !mf.is_empty() {
            s.push('.');
            s.push_str(mf);
        }
        s.push_str(&format!("E{}{:02}", if e < 0 { '-' } else { '+' }, e.unsigned_abs()));
        if s.chars().count() <= max_chars {
            return Some(s);
        }
    }
    None
}

#[cfg(test)]
#[path = "tests/decimal_tests.rs"]
mod tests;
