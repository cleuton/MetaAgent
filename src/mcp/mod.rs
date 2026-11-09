//! MCP: external tools (client) and exposing agents as tools (server).

pub mod client;
pub mod gate; // 0.1.4: the door in front of /mcp for protected messages
pub mod server;
pub mod tool;

use client::McpConnection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// One connection per MCP server command, shared by everything in the process.
#[derive(Default)]
pub struct McpPool {
    connections: Mutex<HashMap<String, Arc<McpConnection>>>,
    /// 0.1.4: one session per (address, token), for tools listed in `[auth] forward`; the least recently used go first
    with_tokens: Mutex<
        Vec<(
            (String, crate::runtime::auth::Fingerprint),
            Arc<McpConnection>,
        )>,
    >,
}

/// 0.1.4: how many per-token sessions are kept.
const TOKEN_SESSIONS: usize = 64;

impl McpPool {
    pub fn get(
        &self,
        command: &str,
        net: &Arc<crate::runtime::net::ClientSet>,
    ) -> Arc<McpConnection> {
        match self.connections.lock() {
            Ok(mut map) => map
                .entry(command.to_string())
                .or_insert_with(|| Arc::new(McpConnection::new(command.to_string(), net.clone())))
                .clone(),
            Err(_) => Arc::new(McpConnection::new(command.to_string(), net.clone())),
        }
    }

    /// 0.1.4: the session that belongs to this token (D12): its own connection, never shared with another token.
    pub fn get_for_token(
        &self,
        command: &str,
        net: &Arc<crate::runtime::net::ClientSet>,
        token: &crate::runtime::auth::Token,
    ) -> Arc<McpConnection> {
        let key = (
            command.to_string(),
            crate::runtime::auth::Fingerprint::of(token),
        );
        let Ok(mut list) = self.with_tokens.lock() else {
            return Arc::new(McpConnection::with_token(
                command.to_string(),
                net.clone(),
                token.clone(),
            ));
        };
        if let Some(i) = list.iter().position(|(k, _)| *k == key) {
            let entry = list.remove(i);
            let found = entry.1.clone();
            list.push(entry);
            return found;
        }
        let connection = Arc::new(McpConnection::with_token(
            command.to_string(),
            net.clone(),
            token.clone(),
        ));
        list.push((key, connection.clone()));
        while list.len() > TOKEN_SESSIONS {
            list.remove(0);
        }
        connection
    }

    /// 0.1.4: drop the session of a token the server did not accept.
    pub fn forget_token(&self, command: &str, token: &crate::runtime::auth::Token) {
        let key = (
            command.to_string(),
            crate::runtime::auth::Fingerprint::of(token),
        );
        if let Ok(mut list) = self.with_tokens.lock() {
            list.retain(|(k, _)| *k != key);
        }
    }

    /// 0.1.3: forget every connection, so the next call connects again with the current `[network]` settings.
    pub fn forget_all(&self) {
        if let Ok(mut map) = self.connections.lock() {
            map.clear();
        }
        if let Ok(mut list) = self.with_tokens.lock() {
            list.clear(); // 0.1.4
        }
    }
}
