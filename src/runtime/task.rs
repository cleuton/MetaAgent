//! Calls in progress: the chain of agents involved, deadlines, and task states.

use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Running,
    Completed,
    Failed,
    TimedOut,
}

impl TaskState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Failed | TaskState::TimedOut
        )
    }

    /// One way transitions only.
    pub fn can_become(self, next: TaskState) -> bool {
        matches!(
            (self, next),
            (TaskState::Pending, TaskState::Running)
                | (TaskState::Running, TaskState::Completed)
                | (TaskState::Running, TaskState::Failed)
                | (TaskState::Running, TaskState::TimedOut)
                | (TaskState::Pending, TaskState::Failed)
        )
    }
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub target: String,
    pub state: TaskState,
}

impl Task {
    pub fn new(id: impl Into<String>, target: impl Into<String>) -> Task {
        Task {
            id: id.into(),
            target: target.into(),
            state: TaskState::Pending,
        }
    }

    pub fn advance(&mut self, next: TaskState) -> bool {
        if self.state.can_become(next) {
            self.state = next;
            true
        } else {
            false
        }
    }
}

/// Travels with every call so cycles can be detected and deadlines respected.
#[derive(Debug, Clone)]
pub struct CallContext {
    /// Agents currently executing in this chain, outermost first.
    pub chain: Vec<String>,
    pub task_id: String,
    pub deadline: Option<Instant>,
}

impl CallContext {
    pub fn new(task_id: impl Into<String>) -> CallContext {
        CallContext {
            chain: Vec::new(),
            task_id: task_id.into(),
            deadline: None,
        }
    }

    pub fn entering(&self, agent: &str) -> CallContext {
        let mut next = self.clone();
        next.chain.push(agent.to_string());
        next
    }
}

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// A fresh task id, unique within this process.
pub fn new_task_id() -> String {
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("task-{:x}-{}", nanos, n)
}
