//! Bounded, identity-checked commit timelines. No cell contents are retained.
use joysheet_app::{Completion, profiling::CommandTiming};
use serde::Serialize;
use serde_json::{Value, json};
use std::{collections::VecDeque, time::Instant};
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameTiming {
    pub prepaint_ms: f64,
    pub geometry_ms: f64,
    pub extent_ms: f64,
    pub dimensions_ms: f64,
    pub layout_ms: f64,
    pub paint_ms: f64,
    pub cells_ms: f64,
    pub text_ms: f64,
    pub scrollbar_ms: f64,
}
struct Edit {
    request: u64,
    window: u64,
    doc: u64,
    sheet: usize,
    start: Instant,
    preflight_ms: f64,
    completion: Option<(CommandTiming, Instant, Instant, u64, u64)>,
    applied: Option<Instant>,
}
#[derive(Default)]
pub struct Performance {
    pending: VecDeque<Edit>,
    edits: VecDeque<Value>,
    frames: VecDeque<FrameTiming>,
    pub discarded: usize,
}
fn ms(a: Instant, b: Instant) -> f64 {
    b.saturating_duration_since(a).as_secs_f64() * 1000.
}
impl Performance {
    pub fn begin(&mut self, request: u64, window: u64, doc: u64, sheet: usize, start: Instant, preflight_ms: f64) {
        if self.pending.len() >= 64 {
            self.pending.pop_front();
            self.discarded += 1;
        }
        self.pending.push_back(Edit { request, window, doc, sheet, start, preflight_ms, completion: None, applied: None });
    }
    pub fn completed(&mut self, c: &Completion, observed: Instant) {
        if c.result.is_err() {
            self.reject(c.request);
            return;
        }
        if let Some(e) = self.pending.iter_mut().find(|e| e.request == c.request)
            && let Some(v) = c.snapshot.window(e.window).filter(|v| v.doc == e.doc && v.sheet == e.sheet)
        {
            e.completion = Some((c.timing.clone(), c.ready_at, observed, c.snapshot.sequence, v.revision));
        }
    }
    pub fn reject(&mut self, request: u64) {
        self.pending.retain(|e| e.request != request);
    }
    pub fn applied(&mut self, sequence: u64) {
        for e in &mut self.pending {
            if e.applied.is_none() && e.completion.as_ref().is_some_and(|c| c.3 <= sequence) {
                e.applied = Some(Instant::now());
            }
        }
    }
    pub fn paint(&mut self, window: u64, doc: u64, sheet: usize, sequence: u64, revision: u64, prepaint: Instant, frame: FrameTiming) {
        let end = Instant::now();
        let mut i = 0;
        while i < self.pending.len() {
            let e = &self.pending[i];
            if e.start.elapsed().as_secs() > 10 {
                self.pending.remove(i);
                self.discarded += 1;
                continue;
            }
            if (e.window, e.doc, e.sheet) != (window, doc, sheet) {
                i += 1;
                continue;
            }
            let Some((command, ready, observed, fence, rev)) = &e.completion else {
                i += 1;
                continue;
            };
            let Some(applied) = e.applied else {
                i += 1;
                continue;
            };
            if sequence < *fence || revision < *rev {
                i += 1;
                continue;
            }
            let mut sample = json!({"request":e.request,"window":window,"doc":doc,"sheet":sheet,"revision":revision,"sequence":sequence,"preflightMs":e.preflight_ms,
                "queueMs":command.queue_ms,"engineMs":command.execute_ms,"snapshotMs":command.snapshot_ms,"publishMs":command.publish_ms,
                "uiPollMs":ms(*ready,*observed),"uiSyncMs":ms(*observed,applied),"frameWaitMs":ms(applied,prepaint),"totalMs":ms(e.start,end)});
            if let Ok(Value::Object(fields)) = serde_json::to_value(&frame) {
                for (k, v) in fields {
                    sample[k] = v;
                }
            }
            if let Value::Object(fields) = &command.calculation {
                for (key, value) in fields {
                    sample[key] = value.clone();
                }
            }
            if self.edits.len() >= 512 {
                self.edits.pop_front();
            }
            self.edits.push_back(sample);
            self.pending.remove(i);
        }
        if self.frames.len() >= 512 {
            self.frames.pop_front();
        }
        self.frames.push_back(frame);
    }
    pub fn report(&self, raw: bool) -> Value {
        let mut phases = serde_json::Map::new();
        let mut frame_phases = serde_json::Map::new();
        for key in ["prepaintMs", "geometryMs", "extentMs", "dimensionsMs", "layoutMs", "paintMs", "cellsMs", "textMs", "scrollbarMs"] {
            frame_phases
                .insert(key.into(), distribution(self.frames.iter().filter_map(|f| serde_json::to_value(f).ok()?.get(key)?.as_f64()).collect()));
        }
        for key in [
            "totalMs",
            "preflightMs",
            "queueMs",
            "engineMs",
            "recalcMs",
            "dependencyMs",
            "evaluationMs",
            "preparationMs",
            "spillSeedMs",
            "writebackMs",
            "rangeReadMs",
            "usedRangeMs",
            "rowFitMs",
            "snapshotMs",
            "publishMs",
            "uiPollMs",
            "uiSyncMs",
            "frameWaitMs",
            "prepaintMs",
            "geometryMs",
            "extentMs",
            "dimensionsMs",
            "layoutMs",
            "paintMs",
            "cellsMs",
            "textMs",
            "scrollbarMs",
        ] {
            phases.insert(key.into(), distribution(self.edits.iter().filter_map(|s| s[key].as_f64()).collect()));
        }
        json!({"scope":"commit handler through first matching worksheet CPU paint; no GPU presentation","capacity":512,"pending":self.pending.len(),"discarded":self.discarded,"editCount":self.edits.len(),"phases":phases,"framePhases":frame_phases,"edits":if raw{json!(self.edits)}else{Value::Null},"frames":if raw{json!(self.frames)}else{Value::Null}})
    }
    pub fn reset(&mut self) {
        self.edits.clear();
        self.frames.clear();
        self.discarded = 0;
    }
}
fn distribution(mut v: Vec<f64>) -> Value {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    let q = |p: usize| v.get((n * p / 100).min(n.saturating_sub(1))).copied();
    json!({"count":n,"p50":q(50),"p95":q(95),"p99":q(99),"max":v.last()})
}
#[cfg(test)]
mod tests {
    #[test]
    fn completion_cannot_be_attributed_to_another_document_or_older_frame() {
        let engine = joysheet_app::EngineClient::start().unwrap();
        engine.execute("file.new", serde_json::json!({})).unwrap().result.unwrap();
        let before = engine.latest();
        let v = before.active().unwrap();
        let start = std::time::Instant::now();
        let ticket = engine.submit("cell.set", serde_json::json!({"input":"42"}), Default::default()).unwrap();
        let mut p = super::Performance::default();
        p.begin(ticket.id, v.window, v.doc, v.sheet, start, 0.);
        let c = ticket.wait(std::time::Duration::from_secs(5)).unwrap();
        p.completed(&c, std::time::Instant::now());
        p.applied(c.snapshot.sequence);
        let after = c.snapshot.active().unwrap();
        p.paint(after.window, after.doc + 1, after.sheet, c.snapshot.sequence, after.revision, std::time::Instant::now(), Default::default());
        p.paint(
            after.window,
            after.doc,
            after.sheet,
            c.snapshot.sequence.saturating_sub(1),
            after.revision,
            std::time::Instant::now(),
            Default::default(),
        );
        assert_eq!(p.report(true)["editCount"], 0);
        p.paint(after.window, after.doc, after.sheet, c.snapshot.sequence, after.revision, std::time::Instant::now(), Default::default());
        let report = p.report(true);
        assert_eq!(report["editCount"], 1);
        assert_eq!(report["edits"][0]["request"], c.request);
        for key in ["totalMs", "queueMs", "engineMs", "snapshotMs", "uiPollMs", "frameWaitMs"] {
            assert!(report["edits"][0][key].as_f64().unwrap() >= 0.);
        }
    }
}
