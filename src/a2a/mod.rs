//! A2A 1.0 (JSON-RPC binding): agents talking to agents over the network.
//! Supported: Agent Card, `SendMessage`, `GetTask`. Not in the MVP: streaming, push
//! notifications, `ListTasks`, `CancelTask`, authentication.

pub mod card;
pub mod client;
pub mod message;
pub mod server;
pub mod store;

/// The A2A protocol version this implementation speaks.
pub const PROTOCOL_VERSION: &str = "1.0";
pub const MEDIA_TYPE: &str = "application/a2a+json";

pub fn now_text() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// A unique id such as `msg-18f2a3c4d5-7`.
pub fn new_id(prefix: &str) -> String {
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{:x}-{}", prefix, nanos, n)
}
