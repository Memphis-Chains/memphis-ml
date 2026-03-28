//! P2P error types

#[derive(Debug, thiserror::Error)]
pub enum P2PError {
    #[error("connection failed: {0}")]
    Connection(String),

    #[error("peer not found: {0}")]
    PeerNotFound(String),

    #[error("timeout")]
    Timeout,

    #[error("protocol error: {0}")]
    Protocol(String),
}
