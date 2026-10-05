//! MCP: external tools (client) and exposing agents as tools (server).

pub mod client;
pub mod server;
pub mod tool;

use client::McpConnection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// One connection per MCP server command, shared by everything in the process.
#[derive(Default)]
pub struct McpPool {
    connections: Mutex<HashMap<String, Arc<McpConnection>>>,
}

impl McpPool {
    pub fn get(&self, command: &str) -> Arc<McpConnection> {
        match self.connections.lock() {
            Ok(mut map) => map
                .entry(command.to_string())
                .or_insert_with(|| Arc::new(McpConnection::new(command.to_string())))
                .clone(),
            Err(_) => Arc::new(McpConnection::new(command.to_string())),
        }
    }
}
