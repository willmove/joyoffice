//! Excel database functions compile the criteria once and fold selected fields
//! in worksheet order. Only variance needs a second visit to the numeric values.
use crate::{
    Arg, Ctx,
    criteria::Criterion,
    util::{R, as_array, has, num_val},
};
use joysheet_core::{Array, CellError, Value, compare_text, number_to_text};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Operation {
    Sum,
    Average,
    Count,
    CountA,
    Get,
    Max,
    Min,
    Product,
    Var,
    VarP,
    StdDev,
    StdDevP,
}

fn column(db: &Array, header: &str) -> Option<usize> {
    let name = header.trim();
    (0..db.cols).find(|&column| match db.get(0, column) {
        Some(Value::Text(value)) => compare_text(value.trim(), name).is_eq(),
        Some(Value::Number(value)) => number_to_text(*value) == name,
        _ => false,
    })
}

fn field(args: &[Arg], db: &Array, op: Operation) -> R<Option<usize>> {
    if !has(args, 1) {
        return if matches!(op, Operation::Count | Operation::CountA) { Ok(None) } else { Err(CellError::Value) };
    }
    let value = args[1].value.scalar();
    match value {
        Value::Number(n) if n.is_finite() && n.trunc() >= 1. && n.trunc() <= db.cols as f64 => Ok(Some(n as usize - 1)),
        Value::Text(text) => column(db, &text).map(Some).ok_or(CellError::Value),
        Value::Error(error) => Err(error),
        Value::Empty if matches!(op, Operation::Count | Operation::CountA) => Ok(None),
        _ => Err(CellError::Value),
    }
}

enum Predicate {
    Field(usize, Criterion),
    Formula(usize, usize),
}

fn criteria(db: &Array, criteria: &Array) -> Vec<Vec<Predicate>> {
    let fields: Vec<_> = (0..criteria.cols)
        .map(|c| match criteria.get(0, c) {
            Some(Value::Text(t)) => column(db, t),
            Some(Value::Number(n)) => column(db, &number_to_text(*n)),
            _ => None,
        })
        .collect();
    (1..criteria.rows)
        .map(|row| {
            fields
                .iter()
                .enumerate()
                .filter_map(|(c, field)| {
                    let value = criteria.get(row, c)?;
                    if value.is_empty() {
                        return None;
                    }
                    let Some(field) = field else { return Some(Predicate::Formula(row, c)) };
                    // A text label without an explicit comparison is a prefix match.
                    let pattern = match value {
                        Value::Text(s)
                            if !s.starts_with(['=', '<', '>'])
                                && !s.ends_with('*')
                                && joysheet_core::parse::parse_number_text(s).is_none()
                                && !s.eq_ignore_ascii_case("TRUE")
                                && !s.eq_ignore_ascii_case("FALSE")
                                && CellError::parse(s).is_none() =>
                        {
                            Criterion::parse(&Value::from(format!("{s}*")))
                        }
                        other => Criterion::parse(other),
                    };
                    Some(Predicate::Field(*field, pattern))
                })
                .collect()
        })
        .collect()
}

pub(super) fn evaluate(op: Operation, args: &[Arg], ctx: &mut dyn Ctx) -> R<Value> {
    use Operation::*;
    let argument = |i: usize| -> R<_> {
        let value = &args.get(i).ok_or(CellError::Value)?.value;
        if let Value::Error(error) = value {
            return Err(*error);
        }
        Ok(as_array(value))
    };
    let database = argument(0)?;
    let filter = argument(2)?;
    let field = field(args, &database, op)?;
    let conditions = criteria(&database, &filter);
    let mut n = 0usize;
    let mut sum = 0.;
    let mut product = 1.;
    let mut extreme = None::<f64>;
    let mut singleton = None;
    let mut variance_values = Vec::new();
    for row in 1..database.rows {
        let selected = conditions.is_empty()
            || conditions.iter().any(|clause| {
                clause.iter().all(|p| match p {
                    Predicate::Field(col, test) => test.matches(database.get(row, *col).unwrap_or(&Value::Empty)),
                    Predicate::Formula(r, c) => ctx.computed_criterion(*r, *c, &database, row).unwrap_or(false),
                })
            });
        if !selected {
            continue;
        }
        let Some(column) = field else {
            n += 1;
            continue;
        };
        let value = database.get(row, column).unwrap_or(&Value::Empty);
        match op {
            Count => {
                n += usize::from(matches!(value, Value::Number(_)));
                continue;
            }
            CountA => {
                n += usize::from(!value.is_empty());
                continue;
            }
            Get => {
                if !value.is_empty() && (!matches!(value, Value::Text(text) if text.is_empty()) || ctx.database_formula_cell(row, column)) {
                    if singleton.is_some() {
                        return Err(CellError::Num);
                    }
                    singleton = Some(value.clone());
                }
                continue;
            }
            _ => {}
        }
        let number = match value {
            Value::Error(error) => return Err(*error),
            Value::Number(number) => *number,
            _ => continue,
        };
        n += 1;
        match op {
            Sum | Average => sum += number,
            Product => product *= number,
            Min => extreme = Some(extreme.map_or(number, |old| old.min(number))),
            Max => extreme = Some(extreme.map_or(number, |old| old.max(number))),
            Var | VarP | StdDev | StdDevP => {
                sum += number;
                variance_values.push(number);
            }
            _ => {}
        }
    }
    let number = match op {
        Sum => sum,
        Average if n > 0 => sum / n as f64,
        Average => return Err(CellError::Div0),
        Count | CountA => n as f64,
        Get => return singleton.ok_or(CellError::Value),
        Max | Min => extreme.unwrap_or(0.),
        Product => {
            if n == 0 {
                0.
            } else {
                product
            }
        }
        Var | VarP | StdDev | StdDevP => {
            let sample = usize::from(matches!(op, Var | StdDev));
            if n <= sample {
                return Err(CellError::Div0);
            }
            let mean = sum / n as f64;
            let residuals = variance_values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>();
            let variance = residuals / (n - sample) as f64;
            if matches!(op, StdDev | StdDevP) { variance.sqrt() } else { variance }
        }
    };
    num_val(number)
}
