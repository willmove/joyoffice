//! Descriptive statistics over the evaluated arguments. Each pass visits cells
//! in spreadsheet order, with constant scratch space even for large ranges.
use crate::Arg;
use crate::util::{NumberInput, R, num_val, visit_stat_numbers};
use joysheet_core::{CellError, Value};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Statistic {
    Mean,
    Minimum,
    Maximum,
    SumSquares,
    MeanDeviation,
    SampleVariance,
    PopulationVariance,
    SampleDeviation,
    PopulationDeviation,
    SampleSkew,
    PopulationSkew,
    ExcessKurtosis,
}

pub(super) fn calculate(args: &[Arg], stat: Statistic, input: NumberInput) -> R<Value> {
    use Statistic::*;
    let mut count = 0usize;
    let mut total = 0.;
    match stat {
        Minimum | Maximum => {
            let minimum = stat == Minimum;
            total = if minimum { f64::INFINITY } else { f64::NEG_INFINITY };
            visit_stat_numbers(args, input, |value| {
                total = if minimum { total.min(value) } else { total.max(value) };
                count += 1;
            })?;
        }
        _ => {
            visit_stat_numbers(args, input, |value| {
                total += value;
                count += 1;
            })?;
        }
    }
    let minimum = match stat {
        Minimum | Maximum => 0,
        SampleVariance | SampleDeviation => 2,
        SampleSkew | PopulationSkew => 3,
        ExcessKurtosis => 4,
        _ => 1,
    };
    if count < minimum {
        return Err(if matches!(stat, SumSquares | MeanDeviation) { CellError::Num } else { CellError::Div0 });
    }
    let n = count as f64;
    let mean = total / n;
    let sum_squares = || -> R<f64> {
        let mut sum = 0.;
        visit_stat_numbers(args, input, |v| {
            let difference = v - mean;
            sum += difference * difference;
        })?;
        Ok(sum)
    };
    let variance = || -> R<f64> {
        let sample = matches!(stat, SampleVariance | SampleDeviation | SampleSkew | ExcessKurtosis);
        Ok(sum_squares()? / if sample { n - 1. } else { n })
    };
    match stat {
        Minimum | Maximum => num_val(if count == 0 { 0. } else { total }),
        Mean => num_val(mean),
        MeanDeviation => {
            let mut absolute = 0.;
            visit_stat_numbers(args, input, |v| absolute += (v - mean).abs())?;
            num_val(absolute / n)
        }
        SumSquares => num_val(sum_squares()?),
        SampleVariance | PopulationVariance => num_val(variance()?),
        SampleDeviation | PopulationDeviation => num_val(variance()?.sqrt()),
        SampleSkew | PopulationSkew | ExcessKurtosis => {
            let deviation = variance()?.sqrt();
            if deviation == 0. {
                return Err(CellError::Div0);
            }
            // Normalize before raising to the third/fourth power. A very wide
            // spread can have finite skewness even when x^4 is unrepresentable.
            let mut standardized = 0.;
            visit_stat_numbers(args, input, |v| {
                let z = (v - mean) / deviation;
                standardized += if stat == ExcessKurtosis { z.powi(4) } else { z.powi(3) };
            })?;
            let result = match stat {
                PopulationSkew => standardized / n,
                SampleSkew => n / ((n - 1.) * (n - 2.)) * standardized,
                _ => n * (n + 1.) / ((n - 1.) * (n - 2.) * (n - 3.)) * standardized - 3. * (n - 1.).powi(2) / ((n - 2.) * (n - 3.)),
            };
            num_val(result)
        }
    }
}
