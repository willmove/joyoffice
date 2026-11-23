//! First-party Excel complex-number arithmetic, parsing, and formatting.
//!
//! Covers `COMPLEX`, `IMABS`, `IMAGINARY`, `IMREAL`, `IMARGUMENT`, `IMCONJUGATE`,
//! `IMDIV`, `IMSUB`, `IMPOWER`, `IMSUM`, `IMPRODUCT`, and the algebraic kernel
//! (`Cx`) shared by `IMEXP`, `IMLN`, `IMLOG10`, `IMLOG2`, `IMSQRT`, and complex
//! trigonometric/hyperbolic functions.
//!
//! Uses Smith's overflow/underflow-safe complex division, cancellation-free
//! principal square root, binary exponentiation for exact small integer powers,
//! and zero-allocation borrowed array folding for `IMSUM`/`IMPRODUCT`.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Cx {
    pub(super) re: f64,
    pub(super) im: f64,
}

pub(super) const ONE: Cx = Cx { re: 1.0, im: 0.0 };

impl Cx {
    pub(super) const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    const fn is_zero(self) -> bool {
        self.re == 0.0 && self.im == 0.0
    }

    fn add(self, rhs: Self) -> Self {
        Self::new(self.re + rhs.re, self.im + rhs.im)
    }

    fn sub(self, rhs: Self) -> Self {
        Self::new(self.re - rhs.re, self.im - rhs.im)
    }

    fn mul(self, rhs: Self) -> Self {
        Self::new(self.re * rhs.re - self.im * rhs.im, self.re * rhs.im + self.im * rhs.re)
    }

    /// Smith's scaling division avoids intermediate overflow (`|rhs| ~ 1e200`)
    /// and underflow (`|rhs| ~ 1e-200`) in `rhs.re^2 + rhs.im^2`.
    pub(super) fn div(self, rhs: Self) -> R<Self> {
        if rhs.is_zero() {
            return Err(CellError::Num);
        }
        let out = if rhs.re.abs() >= rhs.im.abs() {
            let ratio = rhs.im / rhs.re;
            let denom = rhs.re + rhs.im * ratio;
            Self::new((self.re + self.im * ratio) / denom, (self.im - self.re * ratio) / denom)
        } else {
            let ratio = rhs.re / rhs.im;
            let denom = rhs.im + rhs.re * ratio;
            Self::new((self.re * ratio + self.im) / denom, (self.im * ratio - self.re) / denom)
        };
        if out.re.is_finite() && out.im.is_finite() { Ok(out) } else { Err(CellError::Num) }
    }

    pub(super) fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }

    fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }

    /// Cancellation- and overflow-safe principal square root.
    pub(super) fn sqrt(self) -> Self {
        if self.is_zero() {
            return Self::new(0.0, 0.0);
        }
        let w = (0.5 * self.abs() + 0.5 * self.re.abs()).sqrt();
        if self.re >= 0.0 { Self::new(w, self.im / (2.0 * w)) } else { Self::new(self.im.abs() / (2.0 * w), if self.im < 0.0 { -w } else { w }) }
    }

    pub(super) fn exp(self) -> Self {
        let scale = self.re.exp();
        Self::new(scale * self.im.cos(), scale * self.im.sin())
    }

    pub(super) fn ln(self) -> R<Self> {
        if self.is_zero() {
            return Err(CellError::Num);
        }
        Ok(Self::new(self.abs().ln(), self.arg()))
    }

    pub(super) fn sin(self) -> Self {
        Self::new(self.re.sin() * self.im.cosh(), self.re.cos() * self.im.sinh())
    }

    pub(super) fn cos(self) -> Self {
        Self::new(self.re.cos() * self.im.cosh(), -self.re.sin() * self.im.sinh())
    }

    pub(super) fn sinh(self) -> Self {
        Self::new(self.re.sinh() * self.im.cos(), self.re.cosh() * self.im.sin())
    }

    pub(super) fn cosh(self) -> Self {
        Self::new(self.re.cosh() * self.im.cos(), self.re.sinh() * self.im.sin())
    }

    fn pow_int(self, mut exp: u32) -> Self {
        let mut base = self;
        let mut acc = ONE;
        while exp > 0 {
            if exp & 1 == 1 {
                acc = acc.mul(base);
            }
            exp >>= 1;
            if exp > 0 {
                base = base.mul(base);
            }
        }
        acc
    }

    fn powf(self, exponent: f64) -> R<Self> {
        if self.is_zero() {
            return if exponent > 0.0 { Ok(Self::new(0.0, 0.0)) } else { Err(CellError::Num) };
        }
        let radius = self.abs().powf(exponent);
        let angle = self.arg() * exponent;
        Ok(Self::new(radius * angle.cos(), radius * angle.sin()))
    }
}

/// Parses an ASCII real token without surrounding whitespace.
fn parse_real_token(token: &str) -> Option<f64> {
    if token.is_empty() {
        return None;
    }
    if !token.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-')) {
        return None;
    }
    token.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Parses Excel `"a+bi"` / `"a+bj"` complex text into `(value, optional_suffix)`.
pub(super) fn parse_complex(text: &str) -> R<(Cx, Option<char>)> {
    if text.is_empty() {
        return Ok((Cx::new(0.0, 0.0), None));
    }
    let bytes = text.as_bytes();
    let suffix = match bytes.last().copied() {
        Some(b'i') => Some('i'),
        Some(b'j') => Some('j'),
        _ => None,
    };
    let Some(sfx) = suffix else {
        return parse_real_token(text).map(|re| (Cx::new(re, 0.0), None)).ok_or(CellError::Num);
    };
    let body = &text[..text.len() - 1];
    let body_bytes = body.as_bytes();
    let mut split_at = None;
    for idx in (1..body_bytes.len()).rev() {
        let cur = body_bytes.get(idx).copied().unwrap_or(0);
        let prev = body_bytes.get(idx - 1).copied().unwrap_or(0);
        if (cur == b'+' || cur == b'-') && prev != b'e' && prev != b'E' {
            split_at = Some(idx);
            break;
        }
    }
    let (re_part, im_part) = match split_at {
        Some(idx) => (&body[..idx], &body[idx..]),
        None => ("", body),
    };
    let re = if re_part.is_empty() { 0.0 } else { parse_real_token(re_part).ok_or(CellError::Num)? };
    let im = match im_part {
        "" | "+" => 1.0,
        "-" => -1.0,
        other => parse_real_token(other).ok_or(CellError::Num)?,
    };
    Ok((Cx::new(re, im), Some(sfx)))
}

fn value_to_complex(value: &Value) -> R<(Cx, Option<char>)> {
    match value {
        Value::Number(n) => Ok((Cx::new(*n, 0.0), None)),
        Value::Empty => Ok((Cx::new(0.0, 0.0), None)),
        Value::Bool(_) => Err(CellError::Value),
        Value::Error(err) => Err(*err),
        Value::Text(s) => parse_complex(s),
        Value::Array(arr) => value_to_complex(arr.data.first().unwrap_or(&Value::Empty)),
    }
}

fn complex_arg(args: &[Arg], idx: usize) -> R<(Cx, Option<char>)> {
    let val = args.get(idx).map(scalar).unwrap_or(Value::Empty);
    value_to_complex(&val)
}

fn combine_suffix(acc: Option<char>, next: Option<char>) -> R<Option<char>> {
    match (acc, next) {
        (Some(a), Some(b)) if a != b => Err(CellError::Value),
        (Some(a), _) => Ok(Some(a)),
        (None, b) => Ok(b),
    }
}

/// Rounds to 15 significant digits (Excel precision) and normalizes `-0.0` to `0.0`.
fn normalize_component(value: f64) -> f64 {
    let rounded = format!("{value:.14e}").parse::<f64>().unwrap_or(value);
    if rounded == 0.0 { 0.0 } else { rounded }
}

fn format_complex(z: Cx, suffix: char) -> R<Value> {
    if !z.re.is_finite() || !z.im.is_finite() {
        return Err(CellError::Num);
    }
    let re = normalize_component(z.re);
    let im = normalize_component(z.im);
    let mut out = String::with_capacity(32);
    if im == 0.0 {
        out.push_str(&number_to_text(re));
        return text_val(out);
    }
    if re != 0.0 {
        out.push_str(&number_to_text(re));
        if im > 0.0 {
            out.push('+');
        }
    }
    if im == -1.0 {
        out.push('-');
    } else if im != 1.0 {
        out.push_str(&number_to_text(im));
    }
    out.push(suffix);
    text_val(out)
}

pub(super) fn complex(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    let re = num(args, 0)?;
    let im = num(args, 1)?;
    let suffix = if has(args, 2) { text(args, 2)? } else { "i".into() };
    let unit = match suffix.as_str() {
        "" | "i" => 'i',
        "j" => 'j',
        _ => return Err(CellError::Value),
    };
    format_complex(Cx::new(re, im), unit)
}

pub(super) fn im_unary(args: &[Arg], op: fn(Cx) -> R<Cx>) -> R<Value> {
    let (z, sfx) = complex_arg(args, 0)?;
    format_complex(op(z)?, sfx.unwrap_or('i'))
}

fn im_scalar(args: &[Arg], op: fn(Cx) -> R<f64>) -> R<Value> {
    let (z, _) = complex_arg(args, 0)?;
    num_val(op(z)?)
}

pub(super) fn imabs(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    im_scalar(args, |z| Ok(z.abs()))
}

pub(super) fn imreal(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    im_scalar(args, |z| Ok(z.re))
}

pub(super) fn imaginary(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    im_scalar(args, |z| Ok(z.im))
}

pub(super) fn imargument(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    im_scalar(args, |z| if z.is_zero() { Err(CellError::Div0) } else { Ok(z.arg()) })
}

fn im_binary(args: &[Arg], op: fn(Cx, Cx) -> R<Cx>) -> R<Value> {
    let (lhs, s1) = complex_arg(args, 0)?;
    let (rhs, s2) = complex_arg(args, 1)?;
    let sfx = combine_suffix(s1, s2)?;
    format_complex(op(lhs, rhs)?, sfx.unwrap_or('i'))
}

pub(super) fn imdiv(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    im_binary(args, Cx::div)
}

pub(super) fn imsub(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    im_binary(args, |x, y| Ok(x.sub(y)))
}

pub(super) fn impower(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    let (z, sfx) = complex_arg(args, 0)?;
    let exponent = num(args, 1)?;
    let res = if exponent == exponent.trunc() && exponent.abs() <= 64.0 && !z.is_zero() {
        let powered = z.pow_int(exponent.abs() as u32);
        if exponent < 0.0 { ONE.div(powered)? } else { powered }
    } else {
        z.powf(exponent)?
    };
    format_complex(res, sfx.unwrap_or('i'))
}

fn fold_complex_args(args: &[Arg], init: Cx, combine: fn(Cx, Cx) -> Cx) -> R<Value> {
    let mut acc = init;
    let mut sfx = None;
    for arg in args {
        match &arg.value {
            Value::Array(arr) => {
                for cell in &arr.data {
                    if matches!(cell, Value::Empty) {
                        continue;
                    }
                    let (z, s) = value_to_complex(cell)?;
                    sfx = combine_suffix(sfx, s)?;
                    acc = combine(acc, z);
                }
            }
            scalar_val => {
                let (z, s) = value_to_complex(scalar_val)?;
                sfx = combine_suffix(sfx, s)?;
                acc = combine(acc, z);
            }
        }
    }
    format_complex(acc, sfx.unwrap_or('i'))
}

pub(super) fn imsum(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    fold_complex_args(args, Cx::new(0.0, 0.0), Cx::add)
}

pub(super) fn improduct(args: &[Arg], _ctx: &mut dyn Ctx) -> R<Value> {
    fold_complex_args(args, ONE, Cx::mul)
}
