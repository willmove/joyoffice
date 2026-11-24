//! Annuities are a sequence of equal cash flows plus two endpoint balances.
use super::*;

// ---------------------------------------------------------------------------------------------
// Time value of money
// ---------------------------------------------------------------------------------------------

/// The payment per period. `(1 + rate)^nper` comes from `nper * ln_1p(rate)` and `p - 1` from
/// `exp_m1`: rounding `1 + rate` first throws away the low bits of a small rate, which left PMT up to
/// about 1000 ULPs from Excel (observed over 400 synthetic probes; this form is within 4). A loan
/// schedule's last `IF(p - SUM(principal) > 0, ...)` turns on those bits. Excel gives #NUM! for a
/// rate of -1 or less, and -pv*rate (no divide) once `(1 + rate)^nper` overflows.
fn pmt_calc(rate: f64, nper: f64, pv: f64, fv: f64, typ: f64) -> f64 {
    if rate == 0.0 {
        return -(pv + fv) / nper;
    }
    if rate <= -1.0 {
        return f64::NAN;
    }
    let l = nper * rate.ln_1p();
    let m = l.exp_m1();
    let k = 1.0 + rate * typ;
    if m > 0.0 {
        // (fv + pv*p) / (p - 1) = pv + (pv + fv) / (p - 1), without the cancellation in p - 1.
        // rate / m (about 1 / nper for a tiny rate) is taken first so (pv + fv) / m can't overflow.
        -(pv * rate + (pv + fv) * (rate / m)) / k
    } else if m > -0.5 {
        // p near 1 from below: fv + pv*p = (pv + fv) + pv*m keeps the small terms that rounding p would drop.
        -(rate * (pv + fv + pv * m)) / (k * m)
    } else {
        // p well below 1: m nears -1 and pv*m would swallow pv + fv, so use p itself.
        -(rate * (fv + pv * l.exp())) / (k * m)
    }
}

/// Sum of `periods` consecutive powers of `1 + rate`, starting at power zero.
pub(super) fn annuity(rate: f64, periods: f64) -> f64 {
    if rate == 0.0 {
        return periods;
    }
    if rate <= -1.0 {
        // Keep the integer-power behavior at and beyond a zero growth factor.
        return ((1.0 + rate).powf(periods) - 1.0) / rate;
    }
    (periods * rate.ln_1p()).exp_m1() / rate
}

pub(super) fn pmt(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (rate, nper, pv) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
    let (fv, typ) = (opt_num(a, 3, 0.0)?, type_arg(a, 4)?);
    if nper == 0.0 {
        return Err(CellError::Num);
    }
    num_val(pmt_calc(rate, nper, pv, fv, typ))
}

/// FV and PV round the growth factor before taking its difference from one.
fn worksheet_growth(rate: f64, periods: f64) -> (f64, f64) {
    let growth = (1.0 + rate).powf(periods);
    let deposits = if rate == 0.0 { periods } else { (growth - 1.0) / rate };
    (growth, deposits)
}

pub(super) fn fv(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (rate, periods, payment) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
    let (present, due) = (opt_num(a, 3, 0.0)?, type_arg(a, 4)?);
    let (growth, deposits) = worksheet_growth(rate, periods);
    num_val(-present * growth - payment * (1.0 + due * rate) * deposits)
}

pub(super) fn pv(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (rate, periods, payment) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
    let (future, due) = (opt_num(a, 3, 0.0)?, type_arg(a, 4)?);
    let (growth, deposits) = worksheet_growth(rate, periods);
    if growth == 0.0 {
        return Err(CellError::Div0);
    }
    let cash = future + payment * (1.0 + due * rate) * deposits;
    num_val(-cash / growth)
}

pub(super) fn nper(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (rate, pmt, pv) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
    let (fv, typ) = (opt_num(a, 3, 0.0)?, type_arg(a, 4)?);
    if rate == 0.0 {
        if pmt == 0.0 {
            return Err(CellError::Div0);
        }
        return num_val(-(pv + fv) / pmt);
    }
    let net_payment = pv * rate + pmt * (1.0 + rate * typ);
    if rate <= -1.0 || net_payment == 0.0 {
        return Err(CellError::Num);
    }
    if 1.0 + rate == 1.0 {
        return Err(CellError::Div0);
    }
    // The difference of the two endpoints remains representable near zero rate.
    let change = -rate * (pv + fv) / net_payment;
    let log_ratio = if change.abs() < 0.5 { change.ln_1p() } else { ((pmt * (1.0 + rate * typ) - fv * rate) / net_payment).ln() };
    num_val(log_ratio / rate.ln_1p())
}

fn period_interest(rate: f64, per: f64, nper: f64, payment: f64, future: f64, typ: f64) -> f64 {
    if typ == 1.0 && per == 1.0 {
        return 0.0;
    }
    // Discount the remaining installments back to the interest date. This
    // avoids subtracting two growing balances near the end of a long loan.
    let exponent = (per - 1.0 - nper) * rate.ln_1p();
    -payment * exponent.exp_m1() + future * (rate / (1.0 + rate * typ)) * exponent.exp()
}

pub(super) fn ipmt(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (rate, per, np, pv) = (num(a, 0)?, num(a, 1)?, num(a, 2)?, num(a, 3)?);
    let (fv, typ) = (opt_num(a, 4, 0.0)?, type_arg(a, 5)?);
    if per < 1.0 || per > np || np == 0.0 {
        return Err(CellError::Num);
    }
    num_val(period_interest(rate, per, np, pmt_calc(rate, np, pv, fv, typ), fv, typ))
}

pub(super) fn ppmt(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (rate, per, np, pv) = (num(a, 0)?, num(a, 1)?, num(a, 2)?, num(a, 3)?);
    let (fv, typ) = (opt_num(a, 4, 0.0)?, type_arg(a, 5)?);
    if per < 1.0 || per > np || np == 0.0 {
        return Err(CellError::Num);
    }
    let payment = pmt_calc(rate, np, pv, fv, typ);
    num_val(payment - period_interest(rate, per, np, payment, fv, typ))
}

fn cumulative(a: &[Arg], principal: bool) -> R<Value> {
    let (rate, np, pv) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
    let (start, end, typ) = (num(a, 3)?, num(a, 4)?, num(a, 5)?);
    if rate <= 0.0 || np <= 0.0 || pv <= 0.0 || start < 1.0 || end < start || end > np || (typ != 0.0 && typ != 1.0) {
        return Err(CellError::Num);
    }
    if end - start > 1e7 || np > 1e15 {
        return Err(CellError::Num);
    }
    let payment = pmt_calc(rate, np, pv, 0.0, typ);
    let mut sum = 0.0;
    let log = rate.ln_1p();
    for period in start.ceil() as u64..=end.floor() as u64 {
        let weight = (period as f64 - 1.0 - np) * log;
        sum += if typ == 1.0 && period == 1 {
            if principal { payment } else { 0.0 }
        } else if principal {
            payment * weight.exp()
        } else {
            -payment * weight.exp_m1()
        };
    }
    num_val(sum)
}

pub(super) fn cumipmt(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    cumulative(a, false)
}

pub(super) fn cumprinc(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    cumulative(a, true)
}

/// The sum and derivative of the geometric series in log-growth coordinates.
fn annuity_slope(log: f64, periods: f64) -> (f64, f64) {
    if (log * periods.max(1.0)).abs() < 1e-5 {
        let linear = (periods - 1.0) / 2.0;
        let quadratic = (periods - 1.0) * (2.0 * periods - 1.0) / 12.0;
        return (periods * (1.0 + log * (linear + log * quadratic)), periods * (linear + 2.0 * log * quadratic));
    }
    let rate = log.exp_m1();
    let end = (log * periods).exp();
    let sum = (log * periods).exp_m1() / rate;
    (sum, (periods * end - sum * log.exp()) / rate)
}

pub(super) fn rate(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (periods, payment, present) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
    let (future, due, guess) = (opt_num(a, 3, 0.0)?, type_arg(a, 4)?, opt_num(a, 5, 0.1)?);
    if periods <= 0.0 {
        return Err(CellError::Num);
    }
    // Use the endpoint at which every discount is at most one. The set of
    // roots is the same at both endpoints, even near the boundary rate of -1.
    let scale = present.abs().max(future.abs()).max(payment.abs()).max(f64::MIN_POSITIVE);
    let (present, future, payment) = (present / scale, future / scale, payment / scale);
    let balance = |log: f64| {
        let (x, first, last, shift, terminal_period, sign) = if log > 0.0 {
            (-log, present, future, 1.0 - due, periods, -1.0)
        } else if future == 0.0 && due == 1.0 {
            // Rebase at the latest payment to eliminate the common factor
            // that goes to zero as the rate approaches -1.
            (log, 0.0, present, 0.0, periods - 1.0, 1.0)
        } else {
            (log, future, present, due, periods, 1.0)
        };
        let (sum, slope) = annuity_slope(x, periods);
        let installments = payment * (shift * x).exp();
        let terminal = last * (terminal_period * x).exp();
        (first + installments * sum + terminal, sign * (installments * (slope + shift * sum) + terminal_period * terminal))
    };
    for seed in [guess, 0.0, 0.1, -0.5, 0.5, 1.0, -0.9, 5.0, 10.0] {
        let mut x = seed.ln_1p();
        for _ in 0..100 {
            let (residual, slope) = balance(x);
            let step = residual / slope;
            if residual == 0.0 || (residual.abs() <= 4.0 * f64::EPSILON && step.abs() <= 1e-12) {
                return num_val(x.exp_m1());
            }
            if !step.is_finite() {
                break;
            }
            let next = x - step.clamp(-1.0, 1.0);
            if next == x || next < -36.0 || next > 709.0 {
                break;
            }
            x = next;
        }
    }
    Err(CellError::Num)
}

pub(super) fn fvschedule(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let mut p = num(a, 0)?;
    for v in flatten(a.get(1..2).unwrap_or(&[])) {
        let r = match v {
            Value::Number(n) => n,
            Value::Empty => 0.0,
            Value::Error(e) => return Err(e),
            _ => return Err(CellError::Value),
        };
        p *= 1.0 + r;
    }
    num_val(p)
}

pub(super) fn effect(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (nom, n) = (num(a, 0)?, num(a, 1)?.trunc());
    if nom <= 0.0 || n < 1.0 {
        return Err(CellError::Num);
    }
    num_val((n * (nom / n).ln_1p()).exp_m1())
}

pub(super) fn nominal(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (eff, n) = (num(a, 0)?, num(a, 1)?.trunc());
    if eff <= 0.0 || n < 1.0 {
        return Err(CellError::Num);
    }
    num_val(n * (eff.ln_1p() / n).exp_m1())
}

pub(super) fn ispmt(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (rate, per, np, pv) = (num(a, 0)?, num(a, 1)?, num(a, 2)?, num(a, 3)?);
    if np == 0.0 {
        return Err(CellError::Div0);
    }
    num_val(pv * rate * (per / np - 1.0))
}

pub(super) fn pduration(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (rate, pv, fv) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
    if rate <= 0.0 || pv <= 0.0 || fv <= 0.0 {
        return Err(CellError::Num);
    }
    let change = (fv - pv) / pv;
    let log = if change.abs() < 0.5 { change.ln_1p() } else { fv.ln() - pv.ln() };
    num_val(log / (1.0 + rate).ln())
}

pub(super) fn rri(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (np, pv, fv) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
    let ratio = fv / pv;
    if np <= 0.0 || pv == 0.0 || ratio < 0.0 {
        return Err(CellError::Num);
    }
    let change = (fv - pv) / pv;
    let log = if change.abs() < 0.5 { change.ln_1p() } else { ratio.ln() };
    num_val((log / np).exp_m1())
}

fn dollar_digits(fraction: f64) -> R<f64> {
    if fraction < 0.0 {
        return Err(CellError::Num);
    }
    if fraction < 1.0 {
        return Err(CellError::Div0);
    }
    Ok(10f64.powf(fraction.log10().ceil()))
}

pub(super) fn dollarde(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (x, f) = (num(a, 0)?, num(a, 1)?.trunc());
    let p = dollar_digits(f)?;
    let int = x.trunc();
    num_val(int + (x - int) * p / f)
}

pub(super) fn dollarfr(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (x, f) = (num(a, 0)?, num(a, 1)?.trunc());
    let p = dollar_digits(f)?;
    let int = x.trunc();
    num_val(int + (x - int) * f / p)
}
