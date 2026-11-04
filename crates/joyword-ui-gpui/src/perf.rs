//! Opt-in diagnostics for editor callback and GPUI paint submission time.
use std::{sync::OnceLock, time::Instant};

pub(super) fn start() -> Option<Instant> {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    ENABLED
        .get_or_init(|| std::env::var_os("JOYWORD_PERF").is_some_and(|v| v == "1"))
        .then(Instant::now)
}

pub(super) fn sample(stage: &str, start: Option<Instant>, page: usize) {
    if let Some(start) = start {
        elapsed(stage, start.elapsed().as_secs_f64() * 1000.0, page);
    }
}

pub(super) fn elapsed(stage: &str, ms: f64, page: usize) {
    eprintln!(
        "joyword-perf {{\"stage\":\"{stage}\",\"ms\":{ms:.3},\"page\":{}}}",
        page + 1
    );
}
