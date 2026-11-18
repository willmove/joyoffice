//! Paired worksheet statistics. Both moments passes read the actual array cells in row-major order.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Statistic {
    Correlation,
    SquaredCorrelation,
    Slope,
    Intercept,
    StandardError,
    Forecast,
    PopulationCovariance,
    SampleCovariance,
}

fn cells(arg: &Arg) -> &[Value] {
    match &arg.value {
        Value::Array(values) => &values.data,
        scalar => std::slice::from_ref(scalar),
    }
}

pub(super) fn calculate(args: &[Arg], statistic: Statistic) -> R<Value> {
    use Statistic::*;
    let (first, prediction) = if statistic == Forecast { (1, num(args, 0)?) } else { (0, 0.) };
    let y = cells(arg(args, first)?);
    let x = cells(arg(args, first + 1)?);
    if y.len() != x.len() {
        return Err(CellError::NA);
    }
    let mut count = 0usize;
    let (mut sum_x, mut sum_y) = (0., 0.);
    for (a, b) in y.iter().zip(x) {
        match (a, b) {
            (Value::Error(e), _) | (_, Value::Error(e)) => return Err(*e),
            (Value::Number(yy), Value::Number(xx)) => {
                count += 1;
                sum_x += xx;
                sum_y += yy;
            }
            _ => {}
        }
    }
    let minimum = match statistic {
        StandardError => 3,
        PopulationCovariance => 1,
        _ => 2,
    };
    if count < minimum {
        return Err(CellError::Div0);
    }
    let n = count as f64;
    let (mean_x, mean_y) = (sum_x / n, sum_y / n);
    let (mut xx, mut xy, mut yy) = (0., 0., 0.);
    for (a, b) in y.iter().zip(x) {
        if let (Value::Number(a), Value::Number(b)) = (a, b) {
            let (dx, dy) = (b - mean_x, a - mean_y);
            xx += dx * dx;
            xy += dx * dy;
            yy += dy * dy;
        }
    }
    let result = match statistic {
        PopulationCovariance => xy / n,
        SampleCovariance => xy / (n - 1.),
        _ if !xx.is_finite() || !xy.is_finite() || (matches!(statistic, Correlation | SquaredCorrelation | StandardError) && !yy.is_finite()) => {
            return Err(CellError::Num);
        }
        _ if xx == 0. => return Err(CellError::Div0),
        Correlation | SquaredCorrelation => {
            if yy == 0. {
                return Err(CellError::Div0);
            }
            // Multiplying squared deviations first loses valid correlations at both extremes.
            let r = xy / (xx.sqrt() * yy.sqrt());
            if statistic == SquaredCorrelation { r * r } else { r }
        }
        Slope => xy / xx,
        Intercept => mean_y - (xy / xx) * mean_x,
        Forecast => {
            let slope = xy / xx;
            (mean_y - slope * mean_x) + slope * prediction
        }
        StandardError => {
            let residual = yy - xy * xy / xx;
            if !residual.is_finite() {
                return Err(CellError::Num);
            }
            (residual.max(0.) / (n - 2.)).sqrt()
        }
    };
    num_val(result)
}
