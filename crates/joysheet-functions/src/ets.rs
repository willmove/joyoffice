//! Additive exponential smoothing on a regular numeric or calendar timeline.
//!
//! The model follows Hyndman and Athanasopoulos, *Forecasting: Principles and
//! Practice*, 3rd ed., chapters 8.3–8.7. Each observation corrects the
//! projected level, slope and seasonal phase by a share of its innovation.
//! Excel documents the AAA model and its argument contract. Its parameter
//! estimation procedure and starting values are implementation-specific.

use joysheet_core::date::{datetime_from_serial, days_in_month_in, serial_from_ymd};
use joysheet_core::{CellError, DateSystem, Value};

use crate::special::norm_inv;
use crate::util::{R, arg, as_array, has, num, num_val};
use crate::{Arg, Ctx, FnSpec};

type C<'a> = &'a mut dyn Ctx;

/// Longest timeline (after completing missing points) the functions accept.
pub(crate) const MAX_POINTS: usize = 200_000;
/// Longest seasonality Excel supports (hours in a year).
pub(crate) const MAX_SEASON: usize = 8_760;
/// Share of the timeline's points that may be missing.
const MAX_MISSING: f64 = 0.3;
/// Seasonally adjusted values the initial level and trend are fitted to.
const INIT_POINTS: usize = 10;
const NM_ITERATIONS: usize = 400;

// ---------------------------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Season {
    Detect,
    None,
    Fixed(usize),
}

/// Duplicate time stamps are combined with this function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Aggregate {
    Average,
    Count,
    CountA,
    Max,
    Median,
    Min,
    Sum,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Options {
    pub seasonality: Season,
    /// `true`: missing points are interpolated from their neighbours; `false`: they are zero.
    pub interpolate: bool,
    pub aggregate: Aggregate,
}

impl Default for Options {
    fn default() -> Self {
        Options { seasonality: Season::Detect, interpolate: true, aggregate: Aggregate::Average }
    }
}

fn whole(a: &[Arg], i: usize) -> R<Option<f64>> {
    if !has(a, i) {
        return Ok(None);
    }
    let v = num(a, i)?;
    if !v.is_finite() {
        return Err(CellError::Num);
    }
    Ok(Some(v.trunc()))
}

/// `[seasonality]`: 1 (default) detects, 0 means none, 2–8,760 is the season length.
pub(crate) fn season_arg(a: &[Arg], i: usize) -> R<Season> {
    match whole(a, i)? {
        None => Ok(Season::Detect),
        Some(1.0) => Ok(Season::Detect),
        Some(0.0) => Ok(Season::None),
        Some(v) if v >= 2.0 && v <= MAX_SEASON as f64 => Ok(Season::Fixed(v as usize)),
        Some(_) => Err(CellError::Num),
    }
}

/// `[data_completion]`: 1 (default) interpolates, 0 fills zeros.
fn completion_arg(a: &[Arg], i: usize) -> R<bool> {
    match whole(a, i)? {
        None => Ok(true),
        Some(1.0) => Ok(true),
        Some(0.0) => Ok(false),
        Some(_) => Err(CellError::Num),
    }
}

/// `[aggregation]`: 0 or 1 AVERAGE (default), 2 COUNT, 3 COUNTA, 4 MAX, 5 MEDIAN, 6 MIN, 7 SUM.
fn aggregate_arg(a: &[Arg], i: usize) -> R<Aggregate> {
    Ok(match whole(a, i)? {
        None => Aggregate::Average,
        Some(v) => match v as i64 {
            0 | 1 => Aggregate::Average,
            2 => Aggregate::Count,
            3 => Aggregate::CountA,
            4 => Aggregate::Max,
            5 => Aggregate::Median,
            6 => Aggregate::Min,
            7 => Aggregate::Sum,
            _ => return Err(CellError::Num),
        },
    })
}

// ---------------------------------------------------------------------------------------------
// Timeline
// ---------------------------------------------------------------------------------------------

/// How timeline values map to point positions (0 = first point).
#[derive(Clone, Copy, Debug)]
enum Axis {
    /// A constant numeric step.
    Linear { t0: f64, step: f64 },
    /// Whole-day dates a constant number of calendar months apart, all on the same day of the
    /// month (`day = Some(d)`) or all on the last day of their month (`day = None`).
    Months { sys: DateSystem, month0: i64, step: i64, day: Option<u32> },
}

fn month_parts(sys: DateSystem, t: f64) -> Option<(i64, f64)> {
    let d = datetime_from_serial(sys, t.floor())?;
    if d.day == 0 {
        return None;
    }
    Some((i64::from(d.year) * 12 + i64::from(d.month) - 1, f64::from(d.day)))
}

impl Axis {
    fn position(&self, t: f64) -> Option<f64> {
        match *self {
            Axis::Linear { t0, step } => Some((t - t0) / step),
            Axis::Months { sys, month0, step, day } => {
                let d = datetime_from_serial(sys, t.floor())?;
                if d.day == 0 {
                    return None;
                }
                let month = i64::from(d.year) * 12 + i64::from(d.month) - 1;
                let dim = f64::from(days_in_month_in(sys, d.year, d.month));
                if dim <= 0.0 {
                    return None;
                }
                let anchor = day.map_or(dim, |a| f64::from(a).min(dim));
                let offset = (f64::from(d.day) - anchor + (t - t.floor())) / dim;
                Some(((month - month0) as f64 + offset) / step as f64)
            }
        }
    }
}

fn gcd(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a.abs()
}

/// The axis of sorted, distinct time stamps: a constant step (missing points allowed) or, for
/// dates, a constant number of calendar months.
fn detect_axis(times: &[f64], sys: DateSystem) -> R<Axis> {
    let (Some(&t0), Some(&t1)) = (times.first(), times.get(1)) else { return Err(CellError::Num) };
    let step = times.windows(2).map(|w| w.get(1).copied().unwrap_or(t1) - w.first().copied().unwrap_or(t0)).fold(f64::INFINITY, f64::min);
    if step.is_finite() && step > 0.0 {
        let even = times.windows(2).all(|w| match (w.first(), w.get(1)) {
            (Some(a), Some(b)) => {
                let r = (b - a) / step;
                (r - r.round()).abs() <= 1e-6
            }
            _ => true,
        });
        if even {
            return Ok(Axis::Linear { t0, step });
        }
    }
    // Calendar months: whole days on one day of the month (or all month ends).
    let mut months = Vec::with_capacity(times.len());
    let mut same_day: Option<u32> = None;
    let mut all_same = true;
    let mut all_end = true;
    for &t in times {
        if t.fract() != 0.0 {
            return Err(CellError::Num);
        }
        let d = datetime_from_serial(sys, t).ok_or(CellError::Num)?;
        if d.day == 0 {
            return Err(CellError::Num);
        }
        months.push(i64::from(d.year) * 12 + i64::from(d.month) - 1);
        match same_day {
            None => same_day = Some(d.day),
            Some(x) if x != d.day => all_same = false,
            _ => {}
        }
        if d.day != days_in_month_in(sys, d.year, d.month) {
            all_end = false;
        }
    }
    let day = if all_same {
        same_day
    } else if all_end {
        None
    } else {
        return Err(CellError::Num);
    };
    let mut g = 0i64;
    for w in months.windows(2) {
        if let (Some(a), Some(b)) = (w.first(), w.get(1)) {
            let d = b - a;
            if d <= 0 {
                return Err(CellError::Num);
            }
            g = gcd(g, d);
        }
    }
    let month0 = month_parts(sys, t0).ok_or(CellError::Num)?.0;
    if g <= 0 {
        return Err(CellError::Num);
    }
    Ok(Axis::Months { sys, month0, step: g, day })
}

/// An evenly spaced series built from a values/timeline pair.
#[derive(Clone, Debug)]
pub(crate) struct Series {
    pub y: Vec<f64>,
    axis: Axis,
    /// The timeline's step in timeline units (the average step for calendar months).
    pub step: f64,
}

impl Series {
    fn position(&self, target: f64) -> R<f64> {
        let point = self.axis.position(target).ok_or(CellError::Num)?;
        if !point.is_finite() {
            return Err(CellError::Num);
        }
        Ok(point)
    }

    /// Position of a time stamp relative to the last point (0 = last point, 1 = next step).
    pub fn horizon(&self, t: f64) -> R<f64> {
        Ok(self.position(t)? - (self.y.len() as f64 - 1.0))
    }
}

fn aggregate(kind: Aggregate, xs: &mut [f64]) -> Option<f64> {
    let n = xs.len() as f64;
    match kind {
        Aggregate::Count | Aggregate::CountA => Some(n),
        _ if xs.is_empty() => None,
        Aggregate::Average => Some(xs.iter().sum::<f64>() / n),
        Aggregate::Sum => Some(xs.iter().sum()),
        Aggregate::Max => xs.iter().copied().reduce(f64::max),
        Aggregate::Min => xs.iter().copied().reduce(f64::min),
        Aggregate::Median => {
            xs.sort_by(f64::total_cmp);
            let k = xs.len() / 2;
            let hi = xs.get(k).copied()?;
            if xs.len() % 2 == 1 { Some(hi) } else { Some((xs.get(k.checked_sub(1)?).copied()? + hi) / 2.0) }
        }
    }
}

/// Builds the series: pairs values with time stamps, sorts, aggregates duplicates, checks the
/// step and completes missing points.
pub(crate) fn series(values: &Value, timeline: &Value, opts: &Options, sys: DateSystem) -> R<Series> {
    let va = as_array(values);
    let ta = as_array(timeline);
    if va.data.len() != ta.data.len() {
        return Err(CellError::NA);
    }
    if va.data.len() > MAX_POINTS {
        return Err(CellError::Num);
    }
    let mut points: Vec<(f64, Option<f64>)> = Vec::with_capacity(va.data.len());
    for (t, v) in ta.data.iter().zip(va.data.iter()) {
        if let Value::Error(e) = t {
            return Err(*e);
        }
        if let Value::Error(e) = v {
            return Err(*e);
        }
        let value = match v {
            Value::Number(x) => Some(*x),
            Value::Empty => None,
            _ => return Err(CellError::Value),
        };
        let time = match t {
            Value::Number(x) if x.is_finite() => *x,
            // A blank row (no time stamp, no value) is ignored.
            Value::Empty if value.is_none() => continue,
            _ => return Err(CellError::Value),
        };
        points.push((time, value));
    }
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut times: Vec<f64> = points.iter().map(|p| p.0).collect();
    times.dedup();
    if times.len() < 2 {
        return Err(CellError::Num);
    }
    let axis = detect_axis(&times, sys)?;
    let (Some(&first), Some(&last)) = (times.first(), times.last()) else { return Err(CellError::Num) };
    let span = axis.position(last).ok_or(CellError::Num)?.round();
    if !(span >= 1.0 && span < MAX_POINTS as f64) {
        return Err(CellError::Num);
    }
    let len = span as usize + 1;
    // Group the (sorted) points by position.
    let mut slots: Vec<Option<f64>> = vec![None; len];
    let mut group: Vec<f64> = Vec::new();
    let mut group_at: Option<usize> = None;
    let flush = |at: Option<usize>, group: &mut Vec<f64>, slots: &mut Vec<Option<f64>>| {
        if let Some(i) = at
            && let Some(s) = slots.get_mut(i)
        {
            *s = aggregate(opts.aggregate, group);
        }
        group.clear();
    };
    for &(t, v) in &points {
        let p = axis.position(t).ok_or(CellError::Num)?.round();
        if !(p >= 0.0 && p < len as f64) {
            return Err(CellError::Num);
        }
        let p = p as usize;
        if group_at != Some(p) {
            flush(group_at, &mut group, &mut slots);
            group_at = Some(p);
        }
        if let Some(x) = v {
            group.push(x);
        }
    }
    flush(group_at, &mut group, &mut slots);
    let known = slots.iter().filter(|s| s.is_some()).count();
    if known == 0 || (len - known) as f64 > MAX_MISSING * len as f64 {
        return Err(CellError::Num);
    }
    let y = if opts.interpolate { interpolate(&slots) } else { slots.iter().map(|s| s.unwrap_or(0.0)).collect() };
    let step = match axis {
        Axis::Linear { step, .. } => step,
        Axis::Months { .. } => (last - first) / span,
    };
    Ok(Series { y, axis, step })
}

/// Missing points become the straight line between their known neighbours (the nearest known
/// value at either end).
fn interpolate(slots: &[Option<f64>]) -> Vec<f64> {
    let mut out = Vec::with_capacity(slots.len());
    // Keep the next known point until it is reached, scanning each slot only once for lookahead.
    let mut known = slots.iter().enumerate().filter_map(|(j, s)| {
        #[cfg(test)]
        tests::INTERPOLATION_VISITS.with(|visits| visits.set(visits.get() + 1));
        s.map(|v| (j, v))
    });
    let mut next = known.next();
    let mut prev: Option<(usize, f64)> = None;
    for (i, s) in slots.iter().enumerate() {
        if let Some(v) = s {
            out.push(*v);
            prev = Some((i, *v));
            next = known.next();
            continue;
        }
        out.push(match (prev, next) {
            (Some((a, va)), Some((b, vb))) => va + (vb - va) * (i - a) as f64 / (b - a) as f64,
            (Some((_, v)), None) | (None, Some((_, v))) => v,
            (None, None) => 0.0,
        });
    }
    out
}

/// A timeline's step, for continuing it past its last point (the Forecast Sheet's future rows).
#[derive(Clone, Copy, Debug)]
pub struct Timeline {
    axis: Axis,
    /// Position of the last time stamp.
    last: i64,
}

impl Timeline {
    /// The step of a set of time stamps (any order, duplicates allowed): a constant numeric step
    /// (missing points allowed) or, for whole-day dates on one day of the month, calendar months.
    /// `None` without at least two distinct, finite time stamps or without a constant step.
    pub fn detect(times: &[f64], sys: DateSystem) -> Option<Timeline> {
        if times.iter().any(|t| !t.is_finite()) {
            return None;
        }
        let mut sorted = times.to_vec();
        sorted.sort_by(f64::total_cmp);
        sorted.dedup();
        let axis = detect_axis(&sorted, sys).ok()?;
        let last = axis.position(*sorted.last()?)?.round();
        // Like the functions, at most 30% of the points may be missing.
        if !(0.0..MAX_POINTS as f64).contains(&last) || (last + 1.0 - sorted.len() as f64) > MAX_MISSING * (last + 1.0) {
            return None;
        }
        Some(Timeline { axis, last: last as i64 })
    }

    /// Steps from the last time stamp to `t` (fractional between steps, negative before it).
    pub fn steps_after_last(&self, t: f64) -> Option<f64> {
        let p = self.axis.position(t)? - self.last as f64;
        p.is_finite().then_some(p)
    }

    /// The time stamp `k` steps after the last one.
    pub fn after_last(&self, k: u32) -> Option<f64> {
        let pos = self.last.checked_add(i64::from(k))?;
        match self.axis {
            Axis::Linear { t0, step } => {
                let t = t0 + pos as f64 * step;
                t.is_finite().then_some(t)
            }
            Axis::Months { sys, month0, step, day } => {
                let month = month0.checked_add(pos.checked_mul(step)?)?;
                let (y, m) = (month.div_euclid(12), month.rem_euclid(12) + 1);
                let dim = i64::from(days_in_month_in(sys, i32::try_from(y).ok()?, u32::try_from(m).ok()?));
                let d = day.map_or(dim, |d| i64::from(d).min(dim));
                if y < 1900 {
                    return None;
                }
                serial_from_ymd(sys, y, m, d)
            }
        }
    }
}

/// The fitted line at the first observation and its slope. The observations
/// are on a unit-spaced axis so its centered sum of squares is closed form.
fn regression(values: &[f64]) -> (f64, f64) {
    let count = values.len() as f64;
    if count < 2.0 {
        return (values.first().copied().unwrap_or(0.0), 0.0);
    }
    let center = (count - 1.0) / 2.0;
    let mean = values.iter().sum::<f64>() / count;
    let covariance: f64 = values.iter().enumerate().map(|(i, y)| (i as f64 - center) * (y - mean)).sum();
    let slope = covariance / (count * (count - 1.0) * (count + 1.0) / 12.0);
    (mean - center * slope, slope)
}

fn residuals(values: &[f64]) -> Vec<f64> {
    let (base, slope) = regression(values);
    values.iter().enumerate().map(|(i, y)| y - (base + slope * i as f64)).collect()
}

pub(crate) fn detect_season(y: &[f64]) -> usize {
    if y.len() < 4 {
        return 0;
    }
    let noise = residuals(y);
    let energy = noise.iter().map(|r| r * r).sum::<f64>();
    let signal = y.iter().map(|y| y * y).sum::<f64>();
    if !energy.is_finite() || energy <= 1e-20 * signal.max(f64::MIN_POSITIVE) {
        return 0;
    }
    let correlation = |lag| noise.iter().zip(noise.iter().skip(lag)).map(|(a, b)| a * b).sum::<f64>() / energy;
    let mut peak = (0, 2.0 / (noise.len() as f64).sqrt());
    let (mut left, mut center) = (correlation(1), correlation(2));
    for lag in 2..=(noise.len() / 2).min(MAX_SEASON) {
        let right = correlation(lag + 1);
        if center > left && center >= right && center > peak.1 {
            peak = (lag, center);
        }
        (left, center) = (center, right);
    }
    peak.0
}

#[derive(Clone, Debug)]
struct State {
    level: f64,
    trend: f64,
    season: Vec<f64>,
}

impl State {
    /// Estimate seasonal offsets from centered moving means of the first
    /// cycles. For an even period the mean lies between observations.
    fn initial(y: &[f64], period: usize) -> Self {
        let mut offsets = vec![0.0; period];
        if period > 0 {
            let mut counts = vec![0usize; period];
            let mut add = |position: usize, residual: f64| {
                let phase = position % period;
                if let (Some(total), Some(count)) = (offsets.get_mut(phase), counts.get_mut(phase)) {
                    *total += residual;
                    *count += 1;
                }
            };
            if y.len() >= 2 * period {
                let prefix: Vec<f64> = std::iter::once(0.0)
                    .chain(y.iter().take(4 * period).scan(0.0, |sum, value| {
                        *sum += value;
                        Some(*sum)
                    }))
                    .collect();
                let mean = |i| -> Option<f64> { Some((prefix.get(i + period)? - prefix.get(i)?) / period as f64) };
                let half = period / 2;
                for (i, observed) in y.iter().take(prefix.len() - 1).skip(half).enumerate() {
                    let Some(mut trend) = mean(i) else { break };
                    if period.is_multiple_of(2) {
                        let Some(next) = mean(i + 1) else { break };
                        trend = trend.midpoint(next);
                    }
                    add(i + half, observed - trend);
                }
            } else {
                for (i, error) in residuals(y).into_iter().enumerate() {
                    add(i, error);
                }
            }
            for (phase, count) in offsets.iter_mut().zip(counts) {
                *phase /= count.max(1) as f64;
            }
            let mean = offsets.iter().sum::<f64>() / period as f64;
            for phase in &mut offsets {
                *phase -= mean;
            }
        }
        let adjusted: Vec<_> =
            y.iter().take(INIT_POINTS).enumerate().map(|(i, value)| value - offsets.get(i % period.max(1)).copied().unwrap_or_default()).collect();
        let (base, trend) = regression(&adjusted);
        Self { level: base - trend, trend, season: offsets }
    }

    fn observe(&mut self, i: usize, observed: f64, [level, slope, season]: [f64; 3]) -> f64 {
        let index = i % self.season.len().max(1);
        let phase = self.season.get_mut(index);
        let projected = self.level + self.trend;
        let error = observed - (projected + phase.as_deref().copied().unwrap_or_default());
        self.level = projected + level * error;
        self.trend += slope * error;
        if let Some(offset) = phase {
            *offset += season * error;
        }
        error
    }
}

type Parameters = [f64; 3];

fn gains([alpha, beta, gamma]: Parameters) -> Parameters {
    [alpha, alpha * beta, gamma]
}

#[derive(Clone, Copy)]
struct Trial {
    coefficients: Parameters,
    loss: f64,
}

fn project([a, b, c]: Parameters, [x, y, z]: Parameters, step: f64) -> Parameters {
    [a + step * (x - a), b + step * (y - b), c + step * (z - c)]
}

/// Bound the simplex search to valid smoothing coefficients. Reuse the
/// same seasonal state allocation for all objective evaluations.
fn estimate(y: &[f64], initial: &State) -> Parameters {
    let dimensions = if initial.season.is_empty() { 2 } else { 3 };
    let mut scratch = initial.clone();
    let mut evaluate = |mut coefficients: Parameters| {
        for (i, value) in coefficients.iter_mut().enumerate() {
            *value = if i < dimensions { value.clamp(0.0, 1.0) } else { 0.0 };
        }
        scratch.level = initial.level;
        scratch.trend = initial.trend;
        scratch.season.copy_from_slice(&initial.season);
        let gains = gains(coefficients);
        let loss = y.iter().copied().enumerate().map(|(i, observed)| scratch.observe(i, observed, gains).powi(2)).sum::<f64>();
        Trial { coefficients, loss: if loss.is_finite() { loss } else { f64::INFINITY } }
    };
    const SEEDS: [f64; 5] = [0.05, 0.25, 0.5, 0.75, 0.95];
    let mut seed = Trial { coefficients: [0.5, 0.1, if dimensions == 2 { 0.0 } else { 0.1 }], loss: f64::INFINITY };
    for alpha in SEEDS {
        for beta in SEEDS {
            for &gamma in SEEDS.iter().take(if dimensions == 2 { 1 } else { SEEDS.len() }) {
                let trial = evaluate([alpha, beta, gamma]);
                if trial.loss < seed.loss {
                    seed = trial;
                }
            }
        }
    }
    let mut vertices = [seed; 4];
    for (i, vertex) in vertices.iter_mut().skip(1).take(dimensions).enumerate() {
        let mut coefficients = seed.coefficients;
        if let Some(value) = coefficients.get_mut(i) {
            *value += if *value > 0.5 { -0.1 } else { 0.1 };
        }
        *vertex = evaluate(coefficients);
    }
    let Some(simplex) = vertices.get_mut(..=dimensions) else { return seed.coefficients };
    for _ in 0..NM_ITERATIONS {
        simplex.sort_by(|a, b| a.loss.total_cmp(&b.loss));
        let Some((worst, face)) = simplex.split_last_mut() else { break };
        let (Some(&best), Some(second_worst)) = (face.first(), face.last()) else { break };
        let diameter = face
            .iter()
            .chain(std::iter::once(&*worst))
            .flat_map(|v| v.coefficients.into_iter().zip(best.coefficients))
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
        if worst.loss - best.loss <= 1e-12 * best.loss.abs() + 1e-300 || diameter < 1e-7 {
            break;
        }
        let center = std::array::from_fn(|i| face.iter().filter_map(|v| v.coefficients.get(i)).map(|x| x / dimensions as f64).sum());
        let reflection = evaluate(project(center, worst.coefficients, -1.0));
        if reflection.loss < best.loss {
            let expansion = evaluate(project(center, worst.coefficients, -2.0));
            *worst = if expansion.loss < reflection.loss { expansion } else { reflection };
        } else if reflection.loss < second_worst.loss {
            *worst = reflection;
        } else {
            let contraction = evaluate(project(center, worst.coefficients, 0.5));
            if contraction.loss < worst.loss {
                *worst = contraction;
            } else {
                for vertex in simplex.iter_mut().skip(1) {
                    *vertex = evaluate(project(best.coefficients, vertex.coefficients, 0.5));
                }
            }
        }
    }
    simplex.iter().min_by(|a, b| a.loss.total_cmp(&b.loss)).map_or(seed.coefficients, |trial| trial.coefficients)
}

/// A fitted model.
#[derive(Clone, Debug)]
pub(crate) struct Fit {
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
    pub m: usize,
    n: usize,
    mse: f64,
    state: State,
    pub errors: Vec<f64>,
}

pub(crate) fn fit(y: &[f64], m: usize) -> Fit {
    let m = if m >= 2 { m } else { 0 };
    let (mut state, [alpha, beta, gamma]) = exact_affine_fit(y, m).unwrap_or_else(|| {
        let state = State::initial(y, m);
        let parameters = estimate(y, &state);
        (state, parameters)
    });
    let gains = gains([alpha, beta, gamma]);
    let errors: Vec<_> = y.iter().copied().enumerate().map(|(i, value)| state.observe(i, value, gains)).collect();
    let mse = errors.iter().map(|error| error * error).sum::<f64>() / y.len().max(1) as f64;
    Fit { alpha, beta, gamma, m, n: y.len(), mse, state, errors }
}

/// On an exactly linear integer series the residuals do not identify the gains.
/// Excel 16 reports these defaults for zero, constant, rising and falling
/// integer series, with the seasonal defaults requiring two complete cycles.
fn exact_affine_fit(y: &[f64], period: usize) -> Option<(State, Parameters)> {
    let [first, second, _, _, ..] = *y else { return None };
    if period > y.len() / 2 || y.iter().any(|y| y.fract() != 0.0 || y.abs() >= 2.0_f64.powi(51)) {
        return None;
    }
    let trend = second - first;
    if !y.windows(2).all(|pair| pair[1] - pair[0] == trend) {
        return None;
    }
    let state = State { level: first - trend, trend, season: vec![0.0; period] };
    let parameters = if period == 0 { [0.9, 0.001, 0.0] } else { [0.751, 0.001, 0.001] };
    Some((state, parameters))
}

impl Fit {
    fn at(&self, k: f64) -> f64 {
        let s = if self.m > 0 {
            let idx = ((self.n as f64 - 1.0) + k).rem_euclid(self.m as f64);
            self.state.season.get(idx as usize).copied().unwrap_or(0.0)
        } else {
            0.0
        };
        self.state.level + k * self.state.trend + s
    }

    /// Point forecast `h` steps after the last point (fractional steps interpolate).
    pub fn forecast(&self, h: f64) -> f64 {
        let lo = h.floor();
        let a = self.at(lo);
        if h == lo { a } else { a + (h - lo) * (self.at(lo + 1.0) - a) }
    }

    /// Standard deviation of the forecast error `h` steps ahead.
    pub fn forecast_sd(&self, h: f64) -> f64 {
        if h <= 1.0 {
            return (self.mse * h).max(0.0).sqrt();
        }
        let k = h.floor() - 1.0;
        let (a, b, g) = (self.alpha, self.alpha * self.beta, self.gamma);
        // Σ_{j=1}^{k} (a + b·j)² + seasonal terms, in closed form.
        let mut sum = k * a * a + a * b * k * (k + 1.0) + b * b * k * (k + 1.0) * (2.0 * k + 1.0) / 6.0;
        if self.m > 0 {
            let m = self.m as f64;
            let q = (k / m).floor();
            sum += 2.0 * g * (q * a + b * m * q * (q + 1.0) / 2.0) + g * g * q;
        }
        // Each extra step contributes the square of one innovation weight.
        // Between sample points interpolate that contribution in variance space.
        let portion = h.fract();
        if portion > 0.0 {
            let j = k + 1.0;
            let seasonal = if self.m > 0 && j % self.m as f64 == 0.0 { g } else { 0.0 };
            sum += portion * (a + b * j + seasonal).powi(2);
        }
        (self.mse * (1.0 + sum)).max(0.0).sqrt()
    }
}

// ---------------------------------------------------------------------------------------------
// Worksheet functions
// ---------------------------------------------------------------------------------------------

struct Prepared {
    series: Series,
    m: usize,
}

fn prepare(a: &[Arg], c: C, values: usize, timeline: usize, opts: Options) -> R<Prepared> {
    let series = series(&arg(a, values)?.value, &arg(a, timeline)?.value, &opts, c.date_system())?;
    let m = match opts.seasonality {
        Season::Detect => detect_season(&series.y),
        Season::None => 0,
        Season::Fixed(m) => m,
    };
    Ok(Prepared { series, m })
}

fn options(a: &[Arg], season: Option<usize>, completion: usize, aggregation: usize) -> R<Options> {
    Ok(Options {
        seasonality: match season {
            Some(i) => season_arg(a, i)?,
            None => Season::Detect,
        },
        interpolate: completion_arg(a, completion)?,
        aggregate: aggregate_arg(a, aggregation)?,
    })
}

fn forecast_ets(a: &[Arg], c: C) -> R<Value> {
    let target = num(a, 0)?;
    let opts = options(a, Some(3), 4, 5)?;
    let p = prepare(a, c, 1, 2, opts)?;
    let position = p.series.position(target)?;
    if position < -1e-9 {
        return Err(CellError::Num);
    }
    let y = &p.series.y;
    let end = y.len().saturating_sub(1) as f64;
    let model = fit(y, p.m);
    if position > end {
        return num_val(model.forecast(position - end));
    }
    // Desktop Excel exposes the historical one-step fitted values when the
    // target is within the series, including interpolation between samples.
    let position = position.max(0.0);
    let lo = position.floor() as usize;
    let hi = position.ceil() as usize;
    let fitted = |index| -> R<f64> { Ok(y.get(index).ok_or(CellError::Num)? - model.errors.get(index).ok_or(CellError::Num)?) };
    let base = fitted(lo)?;
    num_val(base + position.fract() * (fitted(hi)? - base))
}

fn forecast_ets_confint(a: &[Arg], c: C) -> R<Value> {
    let target = num(a, 0)?;
    let level = if has(a, 3) { num(a, 3)? } else { 0.95 };
    if !(level > 0.0 && level < 1.0) {
        return Err(CellError::Num);
    }
    let opts = options(a, Some(4), 5, 6)?;
    let p = prepare(a, c, 1, 2, opts)?;
    let h = p.series.horizon(target)?;
    // Excel 16 reports zero strictly between the final two historical
    // observations, excluding a thousandth of a step at each boundary.
    const EDGE: f64 = 0.001;
    if (-1.0 + EDGE..=-EDGE).contains(&h) {
        return num_val(0.0);
    }
    if h < EDGE {
        return Err(CellError::Num);
    }
    let rounded = h.round();
    let h = if (h - rounded).abs() < EDGE { rounded } else { h };
    let z = -norm_inv((1.0 - level) / 2.0);
    num_val(z * fit(&p.series.y, p.m).forecast_sd(h))
}

fn forecast_ets_seasonality(a: &[Arg], c: C) -> R<Value> {
    let opts = options(a, None, 2, 3)?;
    let p = prepare(a, c, 0, 1, opts)?;
    num_val(p.m as f64)
}

fn forecast_ets_stat(a: &[Arg], c: C) -> R<Value> {
    let kind = num(a, 2)?;
    if !kind.is_finite() || !(1.0..9.0).contains(&kind.trunc()) {
        return Err(CellError::Num);
    }
    let kind = kind.trunc() as u8;
    let opts = options(a, Some(3), 4, 5)?;
    let p = prepare(a, c, 0, 1, opts)?;
    if kind == 8 {
        return num_val(p.series.step);
    }
    let y = &p.series.y;
    let f = fit(y, p.m);
    let n = f.errors.len().max(1) as f64;
    let mae = f.errors.iter().map(|e| e.abs()).sum::<f64>() / n;
    num_val(match kind {
        1 => f.alpha,
        2 => f.beta,
        3 => f.gamma,
        4 => {
            let naive: Vec<f64> = y.windows(2).filter_map(|w| Some((w.get(1)? - w.first()?).abs())).collect();
            let d = naive.iter().sum::<f64>() / naive.len().max(1) as f64;
            if d == 0.0 {
                return Err(CellError::Div0);
            }
            mae / d
        }
        5 => {
            let terms: Vec<f64> = y
                .iter()
                .zip(&f.errors)
                .filter_map(|(v, e)| {
                    let denom = (v.abs() + (v - e).abs()) / 2.0;
                    (denom > 0.0).then(|| e.abs() / denom)
                })
                .collect();
            terms.iter().sum::<f64>() / terms.len().max(1) as f64
        }
        6 => mae,
        7 => (f.errors.iter().map(|e| e * e).sum::<f64>() / n).sqrt(),
        _ => return Err(CellError::Num),
    })
}

const S_ETS: &[bool] = &[true, false, false, true, true, true, true];
const S_ETS_SEASON: &[bool] = &[false, false, true, true];
const S_ETS_STAT: &[bool] = &[false, false, true, true, true, true];

pub(crate) fn specs() -> Vec<FnSpec> {
    vec![
        f!(
            "FORECAST.ETS",
            3,
            6,
            Statistical,
            S_ETS,
            "FORECAST.ETS(target_date, values, timeline, [seasonality], [data_completion], [aggregation])",
            "Forecasts a value by additive (AAA) exponential smoothing.",
            forecast_ets
        ),
        f!(
            "FORECAST.ETS.CONFINT",
            3,
            7,
            Statistical,
            S_ETS,
            "FORECAST.ETS.CONFINT(target_date, values, timeline, [confidence_level], [seasonality], [data_completion], [aggregation])",
            "Half-width of the confidence interval around a FORECAST.ETS value.",
            forecast_ets_confint
        ),
        f!(
            "FORECAST.ETS.SEASONALITY",
            2,
            4,
            Statistical,
            S_ETS_SEASON,
            "FORECAST.ETS.SEASONALITY(values, timeline, [data_completion], [aggregation])",
            "Length of the repeating pattern detected in a time series.",
            forecast_ets_seasonality
        ),
        f!(
            "FORECAST.ETS.STAT",
            3,
            6,
            Statistical,
            S_ETS_STAT,
            "FORECAST.ETS.STAT(values, timeline, statistic_type, [seasonality], [data_completion], [aggregation])",
            "A statistic of the FORECAST.ETS model: 1 alpha, 2 beta, 3 gamma, 4 MASE, 5 SMAPE, 6 MAE, 7 RMSE, 8 step size.",
            forecast_ets_stat
        ),
    ]
}

#[cfg(test)]
#[path = "tests/ets_tests.rs"]
mod tests;
