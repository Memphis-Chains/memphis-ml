//! P2P protocol message types

use serde::{Deserialize, Serialize};

/// All P2P messages
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

/// Handshake sent during connection establishment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handshake {
    pub node_id: String,
    pub address: String,
    pub timestamp_ms: u64,
}

impl Handshake {
    pub fn new(node_id: String, address: String) -> Self {
        Self {
            node_id,
            address,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        }
    }
}

/// An ML expression evaluated on a peer node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MlExpr {
    pub id: String,
    pub source: String,
    pub args: Vec<String>,
    /// Optional context/variables
    pub env: serde_json::Value,
}

impl MlExpr {
    pub fn new(id: &str, source: &str, args: Vec<String>) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            args,
            env: serde_json::Value::Object(serde_json::Map::new()),
        }
    }
}

/// A query sent to a peer node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Query {
    /// List running programs on the peer.
    ListPrograms,
    /// Get the source of a specific program.
    GetSource { program_id: String },
    /// Ping
    Ping,
}

impl Default for Query {
    fn default() -> Self {
        Query::Ping
    }
}

/// A response to a query.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    Programs { ids: Vec<String> },
    Source { program_id: String, source: String },
    Pong,
    Error { message: String },
}

impl Response {
    pub fn error(message: &str) -> Self {
        Response::Error {
            message: message.into(),
        }
    }
}
