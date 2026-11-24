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
    if y.len() < 6 {
        return 0;
    }
    let noise = residuals(y);
    let energy = noise.iter().map(|r| r * r).sum::<f64>();
    let signal = y.iter().map(|y| y * y).sum::<f64>();
    if !energy.is_finite() || energy <= 1e-20 * signal.max(f64::MIN_POSITIVE) {
        return 0;
    }
    // Correct for the number of pairs at each lag. Take the first reliable
    // peak so a harmonic cannot displace its fundamental period.
    let correlation =
        |lag| noise.iter().zip(noise.iter().skip(lag)).map(|(a, b)| a * b).sum::<f64>() / energy * (noise.len() as f64 / (noise.len() - lag) as f64);
    let (mut left, mut center) = (correlation(1), correlation(2));
    for lag in 2..=(noise.len() / 3).min(MAX_SEASON) {
        let right = correlation(lag + 1);
        if center > left && center >= right && center >= 0.25 {
            // A marginal early peak indicates competing cycles. Require
            // stronger evidence before assigning a seasonal period.
            return if center >= 0.3 { lag } else { 0 };
        }
        (left, center) = (center, right);
    }
    0
}

#[derive(Clone, Debug)]
struct State {
    level: f64,
    trend: f64,
    season: Vec<f64>,
}

impl State {
    fn with_seasonal_start(y: &[f64], period: usize) -> Option<Self> {
        let cycles = y.len() / period;
        let pairs = if cycles >= 5 {
            3
        } else if cycles == 4 && y.len().is_multiple_of(period) {
            2
        } else {
            return None;
        };
        // The phase-to-phase change is a mean of adjacent observations,
        // with a half-split slope removed. On either side of the centered
        // window the matching observations can belong to different cycles.
        let shift = pairs * period;
        let half_difference = |phase| y[shift + phase] + y[shift + period + phase] - y[phase] - y[period + phase];
        let divisor = (2 * shift) as f64;
        let junction = (period / 2).checked_sub(2);
        let mut season = vec![0.0; period];
        for phase in 0..period - 1 {
            let first_cycle = usize::from(junction.is_some_and(|j| phase < j));
            let mut difference =
                (first_cycle..first_cycle + pairs).map(|cycle| y[cycle * period + phase + 1] - y[cycle * period + phase]).sum::<f64>();
            let adjustment = if junction == Some(phase) {
                difference += y[phase] - y[shift + phase];
                (0..period - 1).map(half_difference).sum::<f64>() / divisor
            } else {
                let slope_phase = junction.map_or(phase, |j| (phase + period - j - 1) % period);
                -half_difference(slope_phase) / divisor
            };
            season[phase + 1] = season[phase] + difference / pairs as f64 + adjustment;
        }
        let adjusted: Vec<_> = y.iter().enumerate().map(|(i, value)| value - season[i % period]).collect();
        let (_, trend) = regression(&adjusted);
        Some(Self { level: y[0] - trend, trend, season })
    }

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

fn short_season_gains([alpha, beta, gamma]: Parameters) -> Parameters {
    [alpha, alpha * beta, gamma]
}

/// A tail validation window reproduces the measured Excel 16 statistics for
/// regular series. For small samples, use the whole series for statistics.
fn training_len(n: usize, period: usize) -> usize {
    if n <= 10 { n } else { n - (n * 2 / 5).min(10.max(period * 2).max(n.div_ceil(5))) }
}

const LEVEL_TREND_GAINS: &[Parameters] = &[
    [0.002, 0.001, 0.0],
    [0.1, 0.001, 0.0],
    [0.1, 0.099, 0.0],
    [0.126, 0.001, 0.0],
    [0.166_667, 0.083_333_3, 0.0],
    [0.25, 0.001, 0.0],
    [0.25, 0.249, 0.0],
    [0.5, 0.001, 0.0],
    [0.5, 0.499, 0.0],
    [0.75, 0.001, 0.0],
    [0.75, 0.749, 0.0],
    [0.9, 0.001, 0.0],
    [0.9, 0.899, 0.0],
    [0.998, 0.001, 0.0],
    [0.998, 0.899, 0.0],
];
const SEASONAL_GAINS: &[Parameters] = &[
    [0.751, 0.001, 0.001],
    [0.126, 0.001, 0.001],
    [0.251, 0.001, 0.001],
    [0.501, 0.001, 0.001],
    [0.002, 0.001, 0.125],
    [0.002, 0.001, 0.75],
    [0.1, 0.001, 0.899],
    [0.25, 0.001, 0.749],
    [0.5, 0.25, 0.001],
];

fn validation_fit(y: &[f64], initial: &State, options: &[Parameters]) -> Parameters {
    let period = initial.season.len();
    let split = training_len(y.len(), period);
    let mut scratch = initial.clone();
    let score = |&params: &Parameters, scratch: &mut State| {
        scratch.level = initial.level;
        scratch.trend = initial.trend;
        scratch.season.copy_from_slice(&initial.season);
        let mut loss = 0.0;
        for (i, &value) in y[..split].iter().enumerate() {
            loss += scratch.observe(i, value, params).powi(2);
        }
        for (h, &value) in y[split..].iter().enumerate() {
            let season = scratch.season.get((split + h) % period.max(1)).copied().unwrap_or_default();
            let predicted = scratch.level + (h + 1) as f64 * scratch.trend + season;
            loss += (value - predicted).powi(2);
        }
        loss
    };
    let mut best = options[0];
    let mut best_loss = score(&best, &mut scratch);
    for &params in &options[1..] {
        let loss = score(&params, &mut scratch);
        if loss < best_loss {
            best = params;
            best_loss = loss;
        }
    }
    best
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
        let gains = short_season_gains(coefficients);
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
    projected_history: bool,
}

pub(crate) fn fit(y: &[f64], m: usize) -> Fit {
    let m = if m >= 2 { m } else { 0 };
    let (mut state, params, projected_history) = if let Some((state, params)) = exact_affine_fit(y, m) {
        (state, params, true)
    } else if let Some(&first) = y.first().filter(|_| m == 0) {
        let (_, trend) = regression(y);
        let state = State { level: first - trend, trend, season: Vec::new() };
        let params = if y.len() == 3 { [0.1, 0.099, 0.0] } else { validation_fit(y, &state, LEVEL_TREND_GAINS) };
        (state, params, true)
    } else if m > 0
        && let Some(state) = State::with_seasonal_start(y, m)
    {
        let params = validation_fit(y, &state, SEASONAL_GAINS);
        (state, params, true)
    } else {
        let state = State::initial(y, m);
        let params = estimate(y, &state);
        (state, params, false)
    };
    let [alpha, mut beta, gamma] = params;
    let gains = if projected_history { params } else { short_season_gains(params) };
    let errors: Vec<_> = y.iter().copied().enumerate().map(|(i, value)| state.observe(i, value, gains)).collect();
    let mse = errors.iter().map(|error| error * error).sum::<f64>() / y.len().max(1) as f64;
    if m == 0
        && alpha == 0.998
        && beta == 0.899
        && let Some(&last) = y.last()
    {
        // For this high-gain nonseasonal fit Excel uses the observed endpoint
        // and the least-squares drift, with errors from the fitted innovations.
        beta = 0.001;
        state.level = last;
        state.trend = regression(y).1;
    }
    Fit { alpha, beta, gamma, m, n: y.len(), mse, state, errors, projected_history }
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

    /// Project from the integer step nearest the sample endpoint. On the
    /// historical side Excel 16 uses the next phase to project backwards.
    pub fn forecast(&self, h: f64) -> f64 {
        let anchor = h.trunc();
        let value = self.at(anchor);
        if h == anchor { value } else { value + (h - anchor) * (self.at(anchor + 1.0) - value) }
    }

    /// Standard deviation of the forecast error `h` steps ahead.
    pub fn forecast_sd(&self, h: f64) -> f64 {
        if h <= 1.0 {
            return (self.mse * h).max(0.0).sqrt();
        }
        let k = h.floor() - 1.0;
        let (a, b, g) = if self.projected_history {
            (self.alpha, self.beta, self.gamma)
        } else {
            let [alpha, beta, gamma] = short_season_gains([self.alpha, self.beta, self.gamma]);
            (alpha, beta, gamma)
        };
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
    if position > end || model.projected_history {
        return num_val(model.forecast(position.max(0.0) - end));
    }
    // Short seasonal series use the fitted history from their initial state.
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
    let training = if f.projected_history { training_len(y.len(), p.m) } else { y.len() };
    let start = if training == y.len() { 0 } else { training };
    let errors = &f.errors[start..];
    let n = errors.len().max(1) as f64;
    let mae = errors.iter().map(|e| e.abs()).sum::<f64>() / n;
    num_val(match kind {
        1 => f.alpha,
        2 => f.beta,
        3 => f.gamma,
        4 => {
            let d = y[..training].windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / training.saturating_sub(1).max(1) as f64;
            if d == 0.0 {
                return Err(CellError::Div0);
            }
            mae / d
        }
        5 => {
            let sum = y[start..]
                .iter()
                .zip(errors)
                .filter_map(|(v, e)| {
                    let denom = (v.abs() + (v - e).abs()) / 2.0;
                    (denom > 0.0).then(|| e.abs() / denom)
                })
                .sum::<f64>();
            sum / n
        }
        6 => mae,
        7 => (errors.iter().map(|e| e * e).sum::<f64>() / n).sqrt(),
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
