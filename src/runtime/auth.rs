//! 0.1.4: `[auth]` in metagente.toml: which messages need a bearer token, the program that decides if a token is good,
//! a short memory of accepted tokens, and the names that receive the caller's token. Metagente never opens a token.

use super::validator::{self, Outcome};
use crate::lang::AgentDef;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const MAX_TOKEN_LEN: usize = 16384; // D7
pub const CACHE_LIMIT: usize = 1000; // D10

// ---------- the token and its fingerprint ----------

/// The text after `Bearer`. Printing it, by any means, shows nothing (FR-035).
#[derive(Clone)]
pub struct Token(String);

impl Token {
    /// Only the validator runner and the forwarding code call this.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(<hidden>)")
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<token hidden>")
    }
}

/// A hash of a token: it cannot be turned back into the token. Kept only in memory.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Fingerprint([u8; 32]);

impl std::fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Fingerprint({:02x}{:02x}{:02x}{:02x}..)",
            self.0[0], self.0[1], self.0[2], self.0[3]
        )
    }
}

impl Fingerprint {
    pub fn of(token: &Token) -> Fingerprint {
        let mut out = [0u8; 32];
        out.copy_from_slice(&Sha256::digest(token.0.as_bytes()));
        Fingerprint(out)
    }
}

/// The `Authorization` header was there but is not one bearer token Metagente accepts.
#[derive(Debug, PartialEq)]
pub struct Malformed;

/// D7: exactly one `Authorization` header, `Bearer <token>` (any letter case, one space); the token has only letters,
/// digits and `- . _ ~ + /`, with `=` allowed at the end, and at most 16384 characters.
pub fn parse_authorization(headers: &axum::http::HeaderMap) -> Result<Option<Token>, Malformed> {
    let mut all = headers.get_all(axum::http::header::AUTHORIZATION).iter();
    let Some(first) = all.next() else {
        return Ok(None);
    };
    if all.next().is_some() {
        return Err(Malformed);
    }
    let text = first.to_str().map_err(|_| Malformed)?;
    if text.len() < 7 || !text.is_char_boundary(7) {
        return Err(Malformed);
    }
    let (scheme, rest) = text.split_at(7);
    if !scheme.eq_ignore_ascii_case("bearer ") {
        return Err(Malformed);
    }
    parse_token(rest).map(Some)
}

pub fn parse_token(text: &str) -> Result<Token, Malformed> {
    if text.is_empty() || text.len() > MAX_TOKEN_LEN {
        return Err(Malformed);
    }
    let body = text.trim_end_matches('=');
    if body.is_empty() {
        return Err(Malformed);
    }
    let ok = body
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'+' | b'/'));
    if ok {
        Ok(Token(text.to_string()))
    } else {
        Err(Malformed)
    }
}

// ---------- the policy ----------

/// One entry of `protect`: `Agent.message`, or `Agent.*`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtectEntry {
    pub agent: String,
    /// `None` is `*`: every message of the agent.
    pub message: Option<String>,
    pub text: String,
    pub line: usize,
}

/// The accepted tokens, by fingerprint, with the moment each was accepted (D10).
#[derive(Default)]
pub struct AcceptedCache {
    map: Mutex<HashMap<Fingerprint, Instant>>,
}

impl std::fmt::Debug for AcceptedCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AcceptedCache")
    }
}

impl AcceptedCache {
    pub fn new() -> AcceptedCache {
        AcceptedCache::default()
    }

    /// True while the entry is younger than `lifetime`. Using an entry does not extend it.
    pub fn is_fresh(&self, fp: &Fingerprint, lifetime: Duration, now: Instant) -> bool {
        if lifetime.is_zero() {
            return false;
        }
        match self.map.lock() {
            Ok(map) => map
                .get(fp)
                .map(|at| now.saturating_duration_since(*at) < lifetime)
                .unwrap_or(false),
            Err(_) => false,
        }
    }

    /// Records an acceptance. Nothing is kept with a zero lifetime. At most 1000 entries; the oldest go first.
    pub fn record(&self, fp: Fingerprint, lifetime: Duration, now: Instant) {
        if lifetime.is_zero() {
            return;
        }
        if let Ok(mut map) = self.map.lock() {
            map.retain(|_, at| now.saturating_duration_since(*at) < lifetime);
            while map.len() >= CACHE_LIMIT {
                let oldest = map
                    .iter()
                    .min_by_key(|(_, at)| **at)
                    .map(|(k, _)| k.clone());
                match oldest {
                    Some(k) => {
                        map.remove(&k);
                    }
                    None => break,
                }
            }
            map.insert(fp, now);
        }
    }

    pub fn len(&self) -> usize {
        self.map.lock().map(|m| m.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The parsed `[auth]` section. Frozen when a process starts (D16).
#[derive(Debug)]
pub struct AuthPolicy {
    pub validator: Option<String>,
    pub protect: Vec<ProtectEntry>,
    pub forward: Vec<String>,
    pub cache_seconds: u64,
    pub timeout_seconds: u64,
    /// The folder of metagente.toml: working directory of the validator and base of a relative program path.
    pub base_dir: PathBuf,
    /// The metagente.toml these settings came from.
    pub file: String,
    pub forward_line: usize,
    pub cache: AcceptedCache,
}

impl AuthPolicy {
    pub fn is_protected(&self, agent: &str, message: &str) -> bool {
        self.protect
            .iter()
            .any(|p| p.agent == agent && p.message.as_deref().is_none_or(|m| m == message))
    }

    pub fn forwards_to(&self, name: &str) -> bool {
        self.forward.iter().any(|f| f == name)
    }

    /// True when at least one message these agents accept is protected.
    pub fn protects_any(&self, agents: &[Arc<AgentDef>]) -> bool {
        agents.iter().any(|a| {
            a.accepts
                .iter()
                .any(|acc| self.is_protected(&a.name, &acc.message))
        })
    }

    /// The first protected `Agent.message` among these agents, for messages.
    pub fn first_protected(&self, agents: &[Arc<AgentDef>]) -> Option<String> {
        agents.iter().find_map(|a| {
            a.accepts
                .iter()
                .find(|acc| self.is_protected(&a.name, &acc.message))
                .map(|acc| format!("{}.{}", a.name, acc.message))
        })
    }

    fn lifetime(&self) -> Duration {
        Duration::from_secs(self.cache_seconds)
    }
}

// ---------- the request ----------

/// Where a call came from. A call with no `RequestAuth` at all is the 0.1.3 behavior.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Origin {
    /// Came over the network (A2A or MCP on http).
    Network,
}

/// What a request carried, bound to that one request (FR-028). It rides in `CallContext`.
#[derive(Debug)]
pub struct RequestAuth {
    pub origin: Origin,
    pub token: Option<Token>,
    /// True only after the validator (or the cache) said yes.
    pub accepted: bool,
    pub fingerprint: Option<Fingerprint>,
    /// The policy of the project that was started: `link`ed agents and forwarded calls follow it (FR-032).
    pub policy: Arc<AuthPolicy>,
}

impl RequestAuth {
    pub fn network(
        policy: Arc<AuthPolicy>,
        token: Option<Token>,
        accepted: bool,
    ) -> Arc<RequestAuth> {
        let fingerprint = token.as_ref().map(Fingerprint::of);
        Arc::new(RequestAuth {
            origin: Origin::Network,
            token,
            accepted,
            fingerprint,
            policy,
        })
    }
}

/// The one decision about passing a token on (D12): the name is listed, and the request had a token.
pub fn forward_token<'a>(
    auth: Option<&'a Arc<RequestAuth>>,
    destination: &str,
) -> Option<&'a Token> {
    let auth = auth?;
    if !auth.policy.forwards_to(destination) {
        return None;
    }
    auth.token.as_ref()
}

// ---------- judging ----------

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Accepted,
    NoToken,
    Malformed,
    NotAccepted(Option<i32>),
    Unavailable(String),
}

/// A refusal ready to send: the HTTP status, the plain message with its fix, and the line for the operator.
pub struct Refusal {
    pub status: u16,
    pub reason: &'static str,
    pub message: String,
    pub log: String,
}

impl Verdict {
    pub fn is_accepted(&self) -> bool {
        matches!(self, Verdict::Accepted)
    }

    /// `what` is `Planner.plan`.
    pub fn refusal(&self, what: &str, policy: &AuthPolicy) -> Option<Refusal> {
        let validator = policy.validator.clone().unwrap_or_default();
        let (status, reason, message, why) = match self {
            Verdict::Accepted => return None,
            Verdict::NoToken => (
                401,
                "UNAUTHENTICATED",
                format!(
                    "{} needs a token, and this request had none. Send the header `Authorization: Bearer <token>`.",
                    what
                ),
                "no token".to_string(),
            ),
            Verdict::Malformed => (
                401,
                "UNAUTHENTICATED",
                "The Authorization header of this request is not a bearer token I can read. Send `Authorization: Bearer <token>`; a token has only letters, digits and - . _ ~ + / =, at most 16384 characters.".to_string(),
                "malformed Authorization header".to_string(),
            ),
            Verdict::NotAccepted(code) => (
                401,
                "UNAUTHENTICATED",
                format!(
                    "The token was not accepted for {}. Get a new token from the system that issues them.",
                    what
                ),
                match code {
                    Some(c) => format!("token not accepted (the validator exited with code {})", c),
                    None => "token not accepted (the validator was stopped without an exit code)".to_string(),
                },
            ),
            Verdict::Unavailable(why) => (
                503,
                "VALIDATOR_UNAVAILABLE",
                format!(
                    "I could not ask the validator ({}): {}. Nothing was run. Check that the validator works, or raise timeout_seconds in [auth].",
                    validator, why
                ),
                format!("validator unavailable ({})", why),
            ),
        };
        Some(Refusal {
            status,
            reason,
            message,
            log: format!("Refused {}: {}.", what, why),
        })
    }
}

/// Reads the token, asks the cache or the validator, and says yes or no. The token comes back so the caller can keep it
/// for passing on; it is never stored here.
pub async fn judge(
    policy: &AuthPolicy,
    parsed: Result<Option<Token>, Malformed>,
) -> (Verdict, Option<Token>) {
    let token = match parsed {
        Err(Malformed) => return (Verdict::Malformed, None),
        Ok(None) => return (Verdict::NoToken, None),
        Ok(Some(t)) => t,
    };
    let fp = Fingerprint::of(&token);
    if policy
        .cache
        .is_fresh(&fp, policy.lifetime(), Instant::now())
    {
        return (Verdict::Accepted, Some(token));
    }
    let verdict = match validator::run(policy, &token).await {
        Outcome::Accepted => {
            policy.cache.record(fp, policy.lifetime(), Instant::now());
            Verdict::Accepted
        }
        Outcome::Refused(code) => Verdict::NotAccepted(code),
        Outcome::Unavailable(why) => Verdict::Unavailable(why),
    };
    (verdict, Some(token))
}

/// The JSON body of a refusal: the shape the A2A server uses for its errors.
pub fn refusal_json(id: &serde_json::Value, refusal: &Refusal) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": -32000,
            "message": refusal.message,
            "data": [{
                "@type": "type.googleapis.com/google.rpc.ErrorInfo",
                "reason": refusal.reason,
                "domain": "a2a-protocol.org",
                "metadata": {"timestamp": crate::a2a::now_text()}
            }]
        }
    })
}
