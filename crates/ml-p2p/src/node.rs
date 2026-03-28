//! P2P node implementation

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bytes::{Buf, BufMut, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::time::interval;

use crate::error::P2PError;
use crate::protocol::{Handshake, Message, MlExpr, Query, Response};

/// Max bytes for a single frame
const MAX_FRAME_SIZE: usize = 64 * 1024;

/// Peer connection info
#[derive(Debug, Clone)]
struct PeerInfo {
    address: SocketAddr,
    last_seen: Instant,
    programs: Vec<String>,
}

/// A running ML program on this node
#[allow(dead_code)]
struct RunningProgram {
    id: String,
    source: String,
    args: Vec<String>,
    created_at: Instant,
}

/// P2P node that can dial peers, send/receive ML expressions, and gossip programs.
pub struct Node {
    node_id: String,
    address: SocketAddr,
    peers: HashMap<String, PeerInfo>,
    programs: HashMap<String, RunningProgram>,
    listener: TcpListener,
    /// Shutdown trigger
    shutdown_tx: broadcast::Sender<()>,
}

impl Node {
    /// Bind to an address and start a new node.
    pub async fn bind(addr: &str) -> Result<Self, P2PError> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| P2PError::Connection(e.to_string()))?;
        let address = listener
            .local_addr()
            .map_err(|e| P2PError::Connection(e.to_string()))?;
        let (shutdown_tx, _) = broadcast::channel(1);
        Ok(Node {
            node_id: uuid::Uuid::new_v4().to_string(),
            address,
            peers: HashMap::new(),
            programs: HashMap::new(),
            listener,
            shutdown_tx,
        })
    }

    /// Run the node. This loops until shutdown.
    pub async fn run(&mut self) -> Result<(), P2PError> {
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                result = self.listener.accept() => {
                    let (socket, addr) = result.map_err(|e| P2PError::Connection(e.to_string()))?;
                    let node_id = self.node_id.clone();
                    let addr_str = self.address.to_string();
                    tokio::spawn(async move {
                        if let Err(e) = handle_inbound(socket, addr, &node_id, &addr_str).await {
                            tracing::warn!("inbound connection error from {}: {}", addr, e);
                        }
                    });
                }
                _ = Self::heartbeat_loop() => {
                    // runs every 30s, sends heartbeats to peers
                    self.send_heartbeats().await;
                }
                _ = shutdown_rx.recv() => {
                    tracing::info!("node {} shutting down", self.node_id);
                    break;
                }
            }
        }
        Ok(())
    }

    /// Signal the node to stop.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// Node's unique ID.
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Local address we're bound to.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Dial a peer and perform a handshake.
    pub async fn dial(&mut self, addr: &str) -> Result<(), P2PError> {
        let mut socket = TcpStream::connect(addr)
            .await
            .map_err(|e| P2PError::Connection(e.to_string()))?;
        let local_addr = socket
            .local_addr()
            .map_err(|e| P2PError::Connection(e.to_string()))?;

        // Send Handshake
        let hs = Handshake::new(self.node_id.clone(), local_addr.to_string());
        let hs_bytes = serde_json::to_vec(&hs).map_err(|e| P2PError::Protocol(e.to_string()))?;
        let frame = encode_frame(&hs_bytes);
        socket
            .write_all(&frame)
            .await
            .map_err(|e| P2PError::Connection(e.to_string()))?;

        // Read Handshake response
        let mut len_buf = [0u8; 4];
        socket
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| P2PError::Connection(e.to_string()))?;
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > MAX_FRAME_SIZE {
            return Err(P2PError::Protocol("frame too large".into()));
        }
        let mut data = vec![0u8; len];
        socket
            .read_exact(&mut data)
            .await
            .map_err(|e| P2PError::Connection(e.to_string()))?;

        let resp_hs: Handshake =
            serde_json::from_slice(&data).map_err(|e| P2PError::Protocol(e.to_string()))?;

        // Store peer
        let peer_socket_addr: SocketAddr = resp_hs
            .address
            .parse()
            .map_err(|e| P2PError::Connection(format!("invalid peer address: {}", e)))?;
        self.peers.insert(
            resp_hs.node_id.clone(),
            PeerInfo {
                address: peer_socket_addr,
                last_seen: Instant::now(),
                programs: Vec::new(),
            },
        );

        tracing::info!(
            "dialed peer {} at {}",
            resp_hs.node_id,
            peer_socket_addr
        );

        Ok(())
    }

    /// Send an ML expression to a peer by node_id or address string.
    pub async fn send_ml_expr(&self, to: &str, expr: MlExpr) -> Result<(), P2PError> {
        let addr: SocketAddr = if let Some(peer) = self.peers.get(to) {
            peer.address
        } else {
            // try parsing as SocketAddr directly
            to.parse()
                .map_err(|_| P2PError::PeerNotFound(to.into()))?
        };

        let payload = serde_json::to_vec(&expr).map_err(|e| P2PError::Protocol(e.to_string()))?;
        let frame = encode_frame(&payload);

        let mut socket = TcpStream::connect(addr)
            .await
            .map_err(|e| P2PError::Connection(e.to_string()))?;
        socket
            .write_all(&frame)
            .await
            .map_err(|e| P2PError::Connection(e.to_string()))?;

        Ok(())
    }

    /// Query a peer and wait for a response.
    pub async fn query_peer(
        &self,
        to: &str,
        q: Query,
        timeout: Duration,
    ) -> Result<Response, P2PError> {
        let addr: SocketAddr = if let Some(peer) = self.peers.get(to) {
            peer.address
        } else {
            to.parse()
                .map_err(|_| P2PError::PeerNotFound(to.into()))?
        };

        let payload = serde_json::to_vec(&q).map_err(|e| P2PError::Protocol(e.to_string()))?;
        let frame = encode_frame(&payload);

        let mut socket = TcpStream::connect(addr)
            .await
            .map_err(|e| P2PError::Connection(e.to_string()))?;
        socket
            .write_all(&frame)
            .await
            .map_err(|e| P2PError::Connection(e.to_string()))?;

        let mut buf = vec![0u8; MAX_FRAME_SIZE];
        let read_result = tokio::time::timeout(timeout, socket.read(&mut buf)).await;
        let n = match read_result {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(P2PError::Connection(e.to_string())),
            Err(_) => return Err(P2PError::Timeout),
        };
        if n == 0 {
            return Err(P2PError::Connection("peer closed".into()));
        }

        let frame_data = decode_frame(&buf[..n])?;
        let resp: Response =
            serde_json::from_slice(&frame_data).map_err(|e| P2PError::Protocol(e.to_string()))?;

        Ok(resp)
    }

    /// Gossip a program to all known peers.
    pub async fn gossip_program(&self, program_id: &str, program: &str, args: Vec<String>) {
        let msg = Message::spawn(program_id, program, args);
        if let Ok(data) = serde_json::to_vec(&msg) {
            let frame = encode_frame(&data);
            for peer in self.peers.values() {
                if let Ok(mut s) = TcpStream::connect(peer.address).await {
                    let _ = s.write_all(&frame).await;
                }
            }
        }
    }

    /// Get list of known peer IDs.
    #[allow(dead_code)]
    pub fn peer_ids(&self) -> Vec<String> {
        self.peers.keys().cloned().collect()
    }

    // -- private helpers --

    async fn heartbeat_loop() {
        let mut ticker = interval(Duration::from_secs(30));
        ticker.tick().await; // skip first instant tick
        ticker.tick().await; // wait for first interval
    }

    async fn send_heartbeats(&self) {
        let msg = Message::Heartbeat {
            node_id: self.node_id.clone(),
            program_ids: self.programs.keys().cloned().collect(),
        };
        if let Ok(data) = serde_json::to_vec(&msg) {
            let frame = encode_frame(&data);
            for peer in self.peers.values() {
                if let Ok(mut s) = TcpStream::connect(peer.address).await {
                    let _ = s.write_all(&frame).await;
                }
            }
        }
    }
}

/// Handle an inbound TCP connection.
async fn handle_inbound(
    mut socket: TcpStream,
    addr: SocketAddr,
    node_id: &str,
    address: &str,
) -> Result<(), P2PError> {
    // Read length prefix
    let mut len_buf = [0u8; 4];
    socket
        .read_exact(&mut len_buf)
        .await
        .map_err(|e| P2PError::Connection(e.to_string()))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_SIZE {
        return Err(P2PError::Protocol("frame too large".into()));
    }
    let mut data = vec![0u8; len];
    socket
        .read_exact(&mut data)
        .await
        .map_err(|e| P2PError::Connection(e.to_string()))?;

    // Try to parse as Handshake first
    if let Ok(hs) = serde_json::from_slice::<Handshake>(&data) {
        tracing::info!("received handshake from {}: node_id={}", addr, hs.node_id);

        // Respond with our handshake
        let resp = Handshake::new(node_id.into(), address.into());
        let resp_bytes =
            serde_json::to_vec(&resp).map_err(|e| P2PError::Protocol(e.to_string()))?;
        let frame = encode_frame(&resp_bytes);
        socket
            .write_all(&frame)
            .await
            .map_err(|e| P2PError::Connection(e.to_string()))?;
        return Ok(());
    }

    // Otherwise treat as ML expression / message
    if let Ok(expr) = serde_json::from_slice::<MlExpr>(&data) {
        tracing::debug!("received ML expr from {}: {:?}", addr, expr);
    }

    Ok(())
}

/// Encode a payload as a length-prefixed frame (4-byte big-endian length + data).
fn encode_frame(payload: &[u8]) -> BytesMut {
    let mut frame = BytesMut::with_capacity(4 + payload.len());
    frame.put_u32(payload.len() as u32);
    frame.put_slice(payload);
    frame
}

/// Decode a length-prefixed frame from a byte slice.
/// Returns the payload (without the length prefix).
fn decode_frame(data: &[u8]) -> Result<Vec<u8>, P2PError> {
    if data.len() < 4 {
        return Err(P2PError::Protocol("frame too short".into()));
    }
    let mut buf = std::io::Cursor::new(data);
    let len = buf.get_u32() as usize;
    if buf.remaining() < len {
        return Err(P2PError::Protocol("incomplete frame".into()));
    }
    let payload = buf.get_ref()[buf.position() as usize..][..len].to_vec();
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Message;

    #[test]
    fn test_message_serialization_roundtrip() {
        let msg = Message::ping("node-42");
        let bytes = serde_json::to_vec(&msg).unwrap();
        let roundtrip: Message = serde_json::from_slice(&bytes).unwrap();
        assert!(matches!(roundtrip, Message::Ping { node_id } if node_id == "node-42"));
    }

    #[test]
    fn test_message_spawn_roundtrip() {
        let msg = Message::spawn("prog-1", "(+ 1 2)", vec!["x".into(), "y".into()]);
        let bytes = serde_json::to_vec(&msg).unwrap();
        let roundtrip: Message = serde_json::from_slice(&bytes).unwrap();
        assert!(matches!(
            roundtrip,
            Message::Spawn { program_id, program, args }
            if program_id == "prog-1" && program == "(+ 1 2)" && args.len() == 2
        ));
    }

    #[test]
    fn test_encode_decode_frame() {
        let payload = b"hello world";
        let frame = encode_frame(payload);
        let decoded = decode_frame(&frame).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn test_handshake_new() {
        let hs = Handshake::new("n1".into(), "127.0.0.1:9001".into());
        assert_eq!(hs.node_id, "n1");
        assert_eq!(hs.address, "127.0.0.1:9001");
        assert!(hs.timestamp_ms > 0);
    }

    #[test]
    fn test_ml_expr_new() {
        let expr = MlExpr::new("e1", "(def x 10)", vec![]);
        assert_eq!(expr.id, "e1");
        assert_eq!(expr.source, "(def x 10)");
        assert!(expr.args.is_empty());
    }

    #[tokio::test]
    async fn test_node_bind() {
        let node = Node::bind("127.0.0.1:0").await.unwrap();
        assert!(!node.node_id().is_empty());
        assert!(node.address().port() > 0);
        node.shutdown();
    }

    #[tokio::test]
    async fn test_node_dial_local() {
        // Start a server that we'll dial
        let mut listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        // Spawn server task
        let addr_clone = addr.clone();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut len_buf = [0u8; 4];
            socket.read_exact(&mut len_buf).await.unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut data = vec![0u8; len];
            socket.read_exact(&mut data).await.unwrap();
            let _hs: Handshake = serde_json::from_slice(&data).unwrap();

            // Respond with handshake
            let resp = Handshake::new("server-1".into(), addr.clone());
            let resp_bytes = serde_json::to_vec(&resp).unwrap();
            let mut frame = BytesMut::with_capacity(4 + resp_bytes.len());
            frame.put_u32(resp_bytes.len() as u32);
            frame.put_slice(&resp_bytes);
            let mut socket = socket;
            socket.write_all(&frame).await.unwrap();
        });

        // Dial from a client node
        let mut node = Node::bind("127.0.0.1:0").await.unwrap();
        node.dial(&addr).await.unwrap();

        server.await.unwrap();
        assert!(node.peer_ids().contains(&"server-1".to_string()));
    }
}
