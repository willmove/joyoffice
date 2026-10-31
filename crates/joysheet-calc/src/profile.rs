//! Optional per-thread wall-clock diagnostics. No workbook values or formula text are retained.
use std::{cell::Cell, time::Instant};

#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    pub recalc_ms: f64,
    pub dependency_ms: f64,
    pub evaluation_ms: f64,
    pub preparation_ms: f64,
    pub spill_seed_ms: f64,
    pub writeback_ms: f64,
    pub range_read_ms: f64,
    pub used_range_ms: f64,
    pub row_fit_ms: f64,
    pub formulas: usize,
    pub ranges: usize,
    pub sum_cache_hits: usize,
}
thread_local! { static ACTIVE: Cell<Option<Sample>> = const { Cell::new(None) }; }

pub fn start() {
    #[cfg(not(target_arch = "wasm32"))]
    ACTIVE.with(|s| s.set(Some(Sample::default())));
}
pub fn finish() -> Sample {
    ACTIVE.with(|s| s.take().unwrap_or_default())
}
#[derive(Clone, Copy)]
pub enum Phase {
    Recalc,
    Dependency,
    Evaluation,
    Preparation,
    SpillSeed,
    Writeback,
    RangeRead,
    UsedRange,
    RowFit,
}
pub struct Scope {
    phase: Phase,
    start: Option<Instant>,
}
pub fn scope(phase: Phase) -> Scope {
    #[cfg(not(target_arch = "wasm32"))]
    let start = ACTIVE.with(|s| s.get().map(|_| Instant::now()));
    #[cfg(target_arch = "wasm32")]
    let start = None;
    Scope { phase, start }
}
pub fn formulas(count: usize) {
    ACTIVE.with(|s| {
        if let Some(mut v) = s.get() {
            v.formulas = v.formulas.saturating_add(count);
            s.set(Some(v));
        }
    });
}
pub fn sum_hit() {
    ACTIVE.with(|s| {
        if let Some(mut v) = s.get() {
            v.sum_cache_hits = v.sum_cache_hits.saturating_add(1);
            s.set(Some(v));
        }
    });
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            let elapsed = start.elapsed().as_secs_f64() * 1000.;
            ACTIVE.with(|s| {
                if let Some(mut v) = s.get() {
                    match self.phase {
                        Phase::Recalc => v.recalc_ms += elapsed,
                        Phase::Dependency => v.dependency_ms += elapsed,
                        Phase::Evaluation => v.evaluation_ms += elapsed,
                        Phase::Preparation => v.preparation_ms += elapsed,
                        Phase::SpillSeed => v.spill_seed_ms += elapsed,
                        Phase::Writeback => v.writeback_ms += elapsed,
                        Phase::RangeRead => {
                            v.range_read_ms += elapsed;
                            v.ranges = v.ranges.saturating_add(1);
                        }
                        Phase::UsedRange => v.used_range_ms += elapsed,
                        Phase::RowFit => v.row_fit_ms += elapsed,
                    }
                    s.set(Some(v));
                }
            });
        }
    }
}

#[cfg(test)]
#[path = "tests/profile_tests.rs"]
mod tests;
