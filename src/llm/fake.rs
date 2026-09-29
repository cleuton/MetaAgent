//! A scripted model for tests: no network, fully deterministic.

use super::{Llm, LlmReply, LlmRequest};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::Mutex;

pub struct FakeLlm {
    script: Mutex<VecDeque<LlmReply>>,
    repeat: Option<LlmReply>,
    seen: Mutex<Vec<LlmRequest>>,
}

impl FakeLlm {
    /// Answers with each reply in order, then with `fallback` forever.
    pub fn scripted(replies: Vec<LlmReply>, fallback: LlmReply) -> FakeLlm {
        FakeLlm {
            script: Mutex::new(replies.into()),
            repeat: Some(fallback),
            seen: Mutex::new(Vec::new()),
        }
    }

    /// Always answers with the same reply.
    pub fn always(reply: LlmReply) -> FakeLlm {
        FakeLlm::scripted(Vec::new(), reply)
    }

    pub fn requests(&self) -> Vec<LlmRequest> {
        self.seen.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl Llm for FakeLlm {
    async fn complete(&self, request: &LlmRequest) -> Result<LlmReply, String> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(request.clone());
        }
        let next = self.script.lock().ok().and_then(|mut s| s.pop_front());
        match next.or_else(|| self.repeat.clone()) {
            Some(reply) => Ok(reply),
            None => Err("the fake model has no more answers".to_string()),
        }
    }
}
