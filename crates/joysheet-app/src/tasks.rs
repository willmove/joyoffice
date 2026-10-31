//! Cancellation flags never grant access to Session; only the engine worker mutates documents.
use joysheet_engine::ScriptControl;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
#[derive(Clone, Serialize)]
pub struct TaskInfo {
    pub id: u64,
    pub command: String,
    pub window: Option<u64>,
    pub doc: Option<u64>,
    pub state: String,
    pub completed: u64,
    pub cancellable: bool,
    pub message: String,
}
pub struct Task {
    pub info: TaskInfo,
    pub control: ScriptControl,
}
pub type Tasks = Arc<Mutex<BTreeMap<u64, Task>>>;
pub fn snapshot(tasks: &Tasks) -> Vec<TaskInfo> {
    let tasks = tasks.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    tasks
        .values()
        .rev()
        .take(64)
        .map(|t| {
            let mut info = t.info.clone();
            info.completed = t.control.completed.load(std::sync::atomic::Ordering::Acquire);
            info
        })
        .collect()
}
pub fn state(tasks: &Tasks, id: u64, state: &str, message: String) {
    if let Some(t) = tasks.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get_mut(&id) {
        t.info.state = state.into();
        t.info.message = message;
    }
}
