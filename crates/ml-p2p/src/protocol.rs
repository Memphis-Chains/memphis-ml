//! P2P protocol message types

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    // Discovery
    Ping { node_id: String },
    Pong { node_id: String, address: String },

    // Program distribution
    Spawn {
        program_id: String,
        program: String,
        args: Vec<String>,
    },
    SpawnAck { program_id: String, node_id: String },

    // Execution control
    Stop { program_id: String },
    StopAck { program_id: String },

    // Data exchange
    Send { to: String, payload: serde_json::Value },
    Receive {
        from: String,
        timeout_ms: Option<u64>,
    },

    // State sync
    StateRequest { program_id: String },
    StateResponse {
        program_id: String,
        state: serde_json::Value,
    },

    // Heartbeat
    Heartbeat {
        node_id: String,
        program_ids: Vec<String>,
    },
}

impl Message {
    pub fn ping(node_id: &str) -> Self {
        Message::Ping {
            node_id: node_id.into(),
        }
    }

    pub fn pong(node_id: &str, address: &str) -> Self {
        Message::Pong {
            node_id: node_id.into(),
            address: address.into(),
        }
    }

    pub fn spawn(program_id: &str, program: &str, args: Vec<String>) -> Self {
        Message::Spawn {
            program_id: program_id.into(),
            program: program.into(),
            args,
        }
    }

    pub fn stop(program_id: &str) -> Self {
        Message::Stop {
            program_id: program_id.into(),
        }
    }

    pub fn send(to: &str, payload: serde_json::Value) -> Self {
        Message::Send {
            to: to.into(),
            payload,
        }
    }
}
