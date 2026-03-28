//! P2P node implementation

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::sleep;

use crate::error::P2PError;
use crate::protocol::Message;

struct PeerInfo {
    address: SocketAddr,
    last_seen: Instant,
    programs: Vec<String>,
}

struct RunningProgram {
    id: String,
    source: String,
    args: Vec<String>,
    created_at: Instant,
}

pub struct Node {
    node_id: String,
    address: SocketAddr,
    peers: HashMap<String, PeerInfo>,
    programs: HashMap<String, RunningProgram>,
    listener: TcpListener,
}

impl Node {
    pub async fn bind(addr: &str) -> Result<Self, P2PError> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| P2PError::Connection(e.to_string()))?;
        let address = listener.local_addr().map_err(|e| P2PError::Connection(e.to_string()))?;
        Ok(Node {
            node_id: uuid::Uuid::new_v4().to_string(),
            address,
            peers: HashMap::new(),
            programs: HashMap::new(),
            listener,
        })
    }

    pub async fn run(&mut self) -> Result<(), P2PError> {
        loop {
            tokio::select! {
                result = self.listener.accept() => {
                    let (socket, addr) = result.map_err(|e: tokio::io::Error| P2PError::Connection(e.to_string()))?;
                    self.handle_connection(socket, addr).await;
                }
                _ = Self::heartbeat_tick() => {
                    self.send_heartbeats().await;
                }
            }
        }
    }

    async fn heartbeat_tick() {
        sleep(Duration::from_secs(30)).await;
    }

    async fn handle_connection(&mut self, mut socket: TcpStream, addr: SocketAddr) {
        let mut buf = Vec::new();
        match socket.read_buf(&mut buf).await {
            Ok(0) => return, // connection closed
            Ok(_n) => {}
            Err(e) => {
                tracing::warn!("failed to read from {}: {}", addr, e);
                return;
            }
        }

        match serde_json::from_slice::<Message>(&buf) {
            Ok(msg) => {
                self.handle_message(msg, addr).await;
            }
            Err(e) => {
                tracing::warn!("failed to parse message from {}: {}", addr, e);
            }
        }
    }

    async fn handle_message(&mut self, msg: Message, from: SocketAddr) {
        match msg {
            Message::Ping { node_id: _ } => {
                let response = Message::pong(&self.node_id, &self.address.to_string());
                self.send_to(&from, response).await;
            }
            Message::Pong { .. } => {
                // Handle pong if needed
            }
            Message::Spawn {
                program_id,
                program,
                args,
            } => {
                self.programs.insert(
                    program_id.clone(),
                    RunningProgram {
                        id: program_id.clone(),
                        source: program,
                        args,
                        created_at: Instant::now(),
                    },
                );
                let ack = Message::SpawnAck {
                    program_id,
                    node_id: self.node_id.clone(),
                };
                self.send_to(&from, ack).await;
            }
            Message::SpawnAck { .. } => {
                // Handle spawn ack if needed
            }
            Message::Stop { program_id } => {
                self.programs.remove(&program_id);
                let ack = Message::StopAck { program_id };
                self.send_to(&from, ack).await;
            }
            Message::StopAck { .. } => {
                // Handle stop ack if needed
            }
            Message::Send { to, payload } => {
                if let Some(peer) = self.peers.get(&to) {
                    let msg = Message::send(&self.node_id, payload);
                    self.send_to(&peer.address, msg).await;
                }
            }
            Message::Receive { .. } => {
                // Handle receive if needed
            }
            Message::StateRequest { program_id: _ } => {
                // Handle state request if needed
            }
            Message::StateResponse { .. } => {
                // Handle state response if needed
            }
            Message::Heartbeat {
                node_id,
                program_ids,
            } => {
                self.peers.insert(
                    node_id.clone(),
                    PeerInfo {
                        address: from,
                        last_seen: Instant::now(),
                        programs: program_ids,
                    },
                );
            }
        }
    }

    async fn send_to(&self, addr: &SocketAddr, msg: Message) {
        if let Ok(mut socket) = TcpStream::connect(addr).await {
            let data = serde_json::to_vec(&msg).unwrap_or_default();
            let _ = socket.write_all(&data).await;
        }
    }

    async fn send_heartbeats(&self) {
        let msg = Message::Heartbeat {
            node_id: self.node_id.clone(),
            program_ids: self.programs.keys().cloned().collect(),
        };
        for peer in self.peers.values() {
            self.send_to(&peer.address, msg.clone()).await;
        }
    }
}
