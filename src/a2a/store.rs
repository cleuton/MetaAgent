//! Finished tasks are kept in memory for a while so `GetTask` can return them.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const RETENTION: Duration = Duration::from_secs(10 * 60);

struct Stored {
    task: Value,
    finished_at: Option<Instant>,
}

#[derive(Default)]
pub struct TaskStore {
    tasks: Mutex<HashMap<String, Stored>>,
    retention: Option<Duration>,
}

impl TaskStore {
    pub fn with_retention(retention: Duration) -> TaskStore {
        TaskStore {
            tasks: Mutex::new(HashMap::new()),
            retention: Some(retention),
        }
    }

    fn retention(&self) -> Duration {
        self.retention.unwrap_or(RETENTION)
    }

    /// Stores or replaces a task. `terminal` marks the moment the retention clock starts.
    pub fn put(&self, id: &str, task: Value, terminal: bool) {
        if let Ok(mut tasks) = self.tasks.lock() {
            let keep = self.retention();
            tasks.retain(|_, s| s.finished_at.map(|t| t.elapsed() < keep).unwrap_or(true));
            let finished_at = if terminal { Some(Instant::now()) } else { None };
            tasks.insert(id.to_string(), Stored { task, finished_at });
        }
    }

    pub fn get(&self, id: &str) -> Option<Value> {
        let tasks = self.tasks.lock().ok()?;
        let stored = tasks.get(id)?;
        if let Some(t) = stored.finished_at {
            if t.elapsed() >= self.retention() {
                return None;
            }
        }
        Some(stored.task.clone())
    }
}
