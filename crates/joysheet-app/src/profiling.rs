//! Bounded diagnostics contain command identifiers, never workbook inputs or paths.
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandTiming {
    pub calculation: Value,
    pub queue_ms: f64,
    /// Original Session command, including its mutation/recalculation and row fitting.
    pub execute_ms: f64,
    pub snapshot_ms: f64,
    pub publish_ms: f64,
}
pub fn calculation(s: joysheet_engine::calc::profile::Sample) -> Value {
    json!({"recalcMs":s.recalc_ms,"dependencyMs":s.dependency_ms,"evaluationMs":s.evaluation_ms,"writebackMs":s.writeback_ms,
        "preparationMs":s.preparation_ms,"spillSeedMs":s.spill_seed_ms,"rangeReadMs":s.range_read_ms,"usedRangeMs":s.used_range_ms,"rowFitMs":s.row_fit_ms,"formulasEvaluated":s.formulas,"rangesRead":s.ranges,"sumCacheHits":s.sum_cache_hits})
}
#[derive(Clone, Default)]
pub struct CommandHistory(Arc<Mutex<VecDeque<Value>>>);
impl CommandHistory {
    pub fn record(&self, request: u64, command: &str, sequence: u64, timing: CommandTiming) {
        let mut h = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if h.len() >= 512 {
            h.pop_front();
        }
        h.push_back(json!({"request":request,"command":command,"sequence":sequence,"timing":timing}));
    }
    pub fn inspect(&self) -> Value {
        let samples = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        json!({"capacity":512,"samples":samples})
    }
}
