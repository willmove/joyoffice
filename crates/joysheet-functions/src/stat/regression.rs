use super::*;

// ---------------------------------------------------------------------------------------------
// Regression (LINEST, LOGEST, TREND, GROWTH)
// ---------------------------------------------------------------------------------------------

struct Xy {
    y: Vec<f64>,
    /// Each predictor is contiguous in observation order.
    columns: Vec<Vec<f64>>,
    y_is_row: bool,
    y_rows: usize,
    y_cols: usize,
}

fn strict_num(v: &Value) -> R<f64> {
    match v {
        Value::Number(n) => Ok(*n),
        Value::Error(e) => Err(*e),
        _ => Err(CellError::Value),
    }
}

/// Most predictor columns LINEST, LOGEST, TREND and GROWTH accept. The QR solver and coefficient statistics both need bounded work.
pub(super) const MAX_REGRESSORS: usize = 255;

/// Most multiply-adds (rows × k²) a regression may spend fitting its predictors.
const MAX_REGRESSION_WORK: usize = 1_000_000_000;

/// Rejects regressions too large to fit: more than [`MAX_REGRESSORS`] predictors is `#VALUE!`,
/// more than [`MAX_REGRESSION_WORK`] multiply-adds is `#NUM!`.
pub(super) fn check_regression_size(n: usize, k: usize) -> R<()> {
    if k > MAX_REGRESSORS {
        return Err(CellError::Value);
    }
    match n.checked_mul(k).and_then(|nk| nk.checked_mul(k)) {
        Some(work) if work <= MAX_REGRESSION_WORK => Ok(()),
        _ => Err(CellError::Num),
    }
}

fn parse_xy(y: &Value, x: Option<&Value>) -> R<Xy> {
    let ya = as_array(y);
    let n = ya.data.len();
    let y_is_row = ya.rows == 1 && ya.cols > 1;
    let yv: Vec<f64> = ya.data.iter().map(strict_num).collect::<R<_>>()?;
    let columns = match x {
        None => vec![(1..=n).map(|i| i as f64).collect()],
        Some(xv) => {
            let xa = as_array(xv);
            if xa.rows == ya.rows && xa.cols == ya.cols {
                vec![xa.data.iter().map(strict_num).collect::<R<_>>()?]
            } else {
                let (observations, predictors) = if y_is_row { (xa.cols, xa.rows) } else { (xa.rows, xa.cols) };
                if observations != n {
                    return Err(CellError::Ref);
                }
                check_regression_size(n, predictors)?;
                let mut columns: Vec<Vec<f64>> = (0..predictors).map(|_| Vec::with_capacity(n)).collect();
                for observation in 0..n {
                    for (predictor, column) in columns.iter_mut().enumerate() {
                        let (r, c) = if y_is_row { (predictor, observation) } else { (observation, predictor) };
                        column.push(strict_num(xa.get(r, c).unwrap_or(&Value::Empty))?);
                    }
                }
                columns
            }
        }
    };
    Ok(Xy { y: yv, columns, y_is_row, y_rows: ya.rows, y_cols: ya.cols })
}

fn const_arg(a: &[Arg], i: usize) -> R<bool> {
    if has(a, i) { boolean(a, i) } else { Ok(true) }
}

fn opt_value(a: &[Arg], i: usize) -> Option<&Value> {
    if has(a, i) { a.get(i).map(|x| &x.value) } else { None }
}

pub(super) fn linest_impl(a: &[Arg], log: bool) -> R<Value> {
    let mut xy = parse_xy(&arg(a, 0)?.value, opt_value(a, 1))?;
    if log {
        if xy.y.iter().any(|&v| v <= 0.0) {
            return Err(CellError::Num);
        }
        xy.y.iter_mut().for_each(|v| *v = v.ln());
    }
    let constant = const_arg(a, 2)?;
    let stats = if has(a, 3) { boolean(a, 3)? } else { false };
    let fit = super::least_squares::fit(xy.y, xy.columns, constant, stats)?;
    let tr = |v: f64| if log { v.exp() } else { v };
    let cols = fit.coef.len() + 1;
    let mut data: Vec<Value> = Vec::new();
    for c in fit.coef.iter().rev() {
        data.push(Value::number(tr(*c)));
    }
    data.push(Value::number(tr(fit.b)));
    if let Some(stats) = fit.stats {
        let rv = |r: R<f64>| r.map(Value::number).unwrap_or_else(Value::Error);
        let na = Value::Error(CellError::NA);
        for s in stats.se.iter().rev() {
            data.push(Value::number(*s));
        }
        data.push(stats.se_b.map(Value::number).unwrap_or(na.clone()));
        let rows: [[Value; 2]; 3] =
            [[rv(stats.r2), rv(stats.sey)], [rv(stats.f), Value::number(stats.df)], [Value::number(stats.ssreg), Value::number(stats.ssresid)]];
        for [p, q] in rows {
            data.push(p);
            if cols > 1 {
                data.push(q);
            }
            for _ in 2..cols {
                data.push(na.clone());
            }
        }
        return array_val(5, cols, data);
    }
    array_val(1, cols, data)
}

pub(super) fn trend_impl(a: &[Arg], log: bool) -> R<Value> {
    let mut xy = parse_xy(&arg(a, 0)?.value, opt_value(a, 1))?;
    if log {
        if xy.y.iter().any(|&v| v <= 0.0) {
            return Err(CellError::Num);
        }
        xy.y.iter_mut().for_each(|v| *v = v.ln());
    }
    let constant = const_arg(a, 3)?;
    let fit = super::least_squares::fit(xy.y, xy.columns, constant, false)?;
    if log
        && !fit.coef.iter().chain([&fit.b]).all(|v| {
            let factor = v.exp();
            factor.is_finite() && factor > 0.
        })
    {
        return Err(CellError::Num);
    }
    let predict = |row: &[f64]| -> Value {
        let linear = fit.predict(row);
        Value::number(if log { linear.exp() } else { linear })
    };
    match opt_value(a, 2).or_else(|| opt_value(a, 1)) {
        None => {
            let data = (1..=xy.y_rows * xy.y_cols).map(|x| predict(&[x as f64])).collect();
            array_val(xy.y_rows, xy.y_cols, data)
        }
        Some(nv) => {
            let na = as_array(nv);
            if fit.coef.len() == 1 {
                let data = na.data.iter().map(|v| strict_num(v).map(|x| predict(&[x]))).collect::<R<Vec<_>>>()?;
                array_val(na.rows, na.cols, data)
            } else if !xy.y_is_row && na.cols == fit.coef.len() {
                let mut data = Vec::with_capacity(na.rows);
                for r in 0..na.rows {
                    let row = (0..na.cols).map(|c| strict_num(na.get(r, c).unwrap_or(&Value::Empty))).collect::<R<Vec<_>>>()?;
                    data.push(predict(&row));
                }
                array_val(na.rows, 1, data)
            } else if xy.y_is_row && na.rows == fit.coef.len() {
                let mut data = Vec::with_capacity(na.cols);
                for c in 0..na.cols {
                    let row = (0..na.rows).map(|r| strict_num(na.get(r, c).unwrap_or(&Value::Empty))).collect::<R<Vec<_>>>()?;
                    data.push(predict(&row));
                }
                array_val(1, na.cols, data)
            } else {
                Err(CellError::Ref)
            }
        }
    }
}
