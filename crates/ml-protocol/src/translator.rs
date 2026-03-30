//! # ML ↔ MP Translator
//!
//! Translates between ML S-expression syntax and Memphis Protocol JSON messages.
//!
//! ## ML Expression Types
//!
//! ML expressions that translate to MP messages:
//! ```ml
//! (request :from marcin :to synjar :action search :query "docs")
//! (response :to synjar :correlation-id "msg-001" :payload {result: []})
//! (ack :to synjar :correlation-id "msg-001")
//! (error :to synjar :correlation-id "msg-001" :message "not found")
//! (hello :agent marcin :protocols [ml-v1 mp-v1] :capabilities [search recall])
//! (bye :agent marcin :reason "done")
//! ```
//!
//! ## Supported ML Actions
//!
//! | ML action | MP action | Notes |
//! |---|---|---|
//! | `request` | `request` | Main request message |
//! | `response` | `response` | Response with payload |
//! | `ack` | `ack` | Acknowledgment |
//! | `error` | `error` | Error response |
//! | `hello` | `hello` | Agent hello / handshake |
//! | `bye` | `bye` | Agent goodbye |
//! | `journal` | `journal` | Memphis journal entry |
//! | `decide` | `decide` | Decision chain entry |
//! | `recall` | `recall` | Memory recall request |
//! | `search` | `search` | Semantic search |
//! | `execute` | `execute` | Execute a tool/command |
//! | `spawn` | `spawn` | Spawn remote agent |

use crate::{
    AgentDid, AgentIdentityMap, MPMessage, MPMessageMeta, MPMessageType, ProtocolError,
    TrustLevel, MIN_DEADLINE_SECS, MP_VERSION,
};
use serde_json::Value;
use uuid::Uuid;
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// ML Expression AST

// TokenBuffer - wrapper to provide push_back on iterators
struct TokenBuffer<I> {
    inner: I,
    buffer: VecDeque<String>,
}

impl<I: Iterator<Item = String>> TokenBuffer<I> {
    fn new(inner: I) -> Self {
        TokenBuffer { inner, buffer: VecDeque::new() }
    }
    
    fn next(&mut self) -> Option<String> {
        self.buffer.pop_front().or_else(|| self.inner.next())
    }
    
    fn push_back(&mut self, item: String) {
        self.buffer.push_back(item);
    }
    
    fn peek(&mut self) -> Option<&String> {
        if self.buffer.is_empty() {
            self.inner.next().map(|item| {
                self.buffer.push_back(item);
                self.buffer.front().unwrap()
            })
        } else {
            self.buffer.front()
        }
    }
}

// ---------------------------------------------------------------------------

/// ML S-expression representation.
/// This is the intermediate representation used during translation.
#[derive(Debug, Clone)]
pub enum MLExpr {
    /// A top-level ML statement (list starting with a keyword)
    Top(Box<MLStmt>),
    /// A raw scalar value
    Value(MLValue),
}

/// ML statement types
#[derive(Debug, Clone)]
pub enum MLStmt {
    /// (request :from X :to Y :action Z :query W :deadline T :trust high)
    Request {
        from: String,
        to: String,
        action: String,
        payload: Value,
        deadline: Option<String>,
        trust_level: Option<TrustLevel>,
        confidential: bool,
        correlation_id: Option<String>,
    },

    /// (response :to X :correlation-id Y :payload Z)
    Response {
        to: String,
        correlation_id: String,
        payload: Value,
    },

    /// (ack :to X :correlation-id Y)
    Ack {
        to: String,
        correlation_id: String,
    },

    /// (error :to X :correlation-id Y :message Z)
    Error {
        to: String,
        correlation_id: Option<String>,
        message: String,
    },

    /// (hello :agent X :protocols [P] :capabilities [C] :trust T)
    Hello {
        agent: String,
        protocols: Vec<String>,
        capabilities: Vec<String>,
        trust_requirements: Vec<String>,
    },

    /// (bye :agent X :reason Y)
    Bye {
        agent: String,
        reason: Option<String>,
    },

    /// Generic action (spawn, journal, decide, recall, search, execute, etc.)
    Action {
        action: String,
        args: Vec<(String, Value)>,
    },
}

/// ML value types
#[derive(Debug, Clone, PartialEq)]
pub enum MLValue {
    Number(f64),
    String(String),
    Boolean(bool),
    Symbol(String),
    List(Vec<MLValue>),
    Record(serde_json::Map<String, Value>),
    Null,
}

impl MLValue {
    pub fn to_json(&self) -> Value {
        match self {
            MLValue::Number(n) => Value::from(*n),
            MLValue::String(s) => Value::from(s.clone()),
            MLValue::Boolean(b) => Value::Bool(*b),
            MLValue::Symbol(s) => Value::String(s.clone()),
            MLValue::List(items) => {
                Value::Array(items.iter().map(|v| v.to_json()).collect())
            }
            MLValue::Record(m) => Value::Object(m.clone()),
            MLValue::Null => Value::Null,
        }
    }
}

// ---------------------------------------------------------------------------
// ML → MP Translator
// ---------------------------------------------------------------------------

/// Translates ML expressions to MP messages.
#[derive(Debug, Clone, Default)]
pub struct MlToMpTranslator {
    identity_map: AgentIdentityMap,
    secret: Option<Vec<u8>>,
}

impl MlToMpTranslator {
    pub fn new(identity_map: AgentIdentityMap) -> Self {
        Self {
            identity_map,
            secret: None,
        }
    }

    /// Set the signing secret used for message signatures.
    pub fn with_secret(mut self, secret: &[u8]) -> Self {
        self.secret = Some(secret.to_vec());
        self
    }

    /// Translate an ML expression to an MPMessage.
    pub fn translate(&self, expr: &MLExpr) -> Result<MPMessage, ProtocolError> {
        match expr {
            MLExpr::Top(stmt) => self.translate_stmt(stmt),
            MLExpr::Value(_) => Err(ProtocolError::TranslationError(
                "bare value cannot be translated to MP message".into(),
            )),
        }
    }

    fn translate_stmt(&self, stmt: &MLStmt) -> Result<MPMessage, ProtocolError> {
        match stmt {
            MLStmt::Request {
                from,
                to,
                action,
                payload,
                deadline,
                trust_level,
                confidential,
                correlation_id,
            } => {
                let from_did = self.resolve_did(from)?;
                let to_did = self.resolve_did(to)?;
                let msg_id = Uuid::new_v4().to_string();
                let mut meta = MPMessageMeta {
                    deadline: deadline.clone(),
                    trust_level: trust_level.clone(),
                    confidential: *confidential,
                    correlation_id: correlation_id.clone(),
                    reply_to: None,
                    extras: serde_json::Map::new(),
                };

                let mut msg = MPMessage {
                    id: msg_id,
                    msg_type: MPMessageType::Request,
                    from: from_did,
                    to: to_did,
                    action: action.clone(),
                    payload: payload.clone(),
                    meta,
                    signature: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    version: MP_VERSION.to_string(),
                };

                if let Some(ref sec) = self.secret {
                    msg.sign(sec);
                }
                Ok(msg)
            }

            MLStmt::Response {
                to,
                correlation_id,
                payload,
            } => {
                let to_did = self.resolve_did(to)?;
                let msg_id = Uuid::new_v4().to_string();
                let mut meta = MPMessageMeta::default();
                meta.correlation_id = Some(correlation_id.clone());

                let mut msg = MPMessage {
                    id: msg_id,
                    msg_type: MPMessageType::Response,
                    from: String::new(), // filled by context
                    to: to_did,
                    action: "response".to_string(),
                    payload: payload.clone(),
                    meta,
                    signature: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    version: MP_VERSION.to_string(),
                };

                if let Some(ref sec) = self.secret {
                    msg.sign(sec);
                }
                Ok(msg)
            }

            MLStmt::Ack {
                to,
                correlation_id,
            } => {
                let to_did = self.resolve_did(to)?;
                let msg_id = Uuid::new_v4().to_string();
                let mut meta = MPMessageMeta::default();
                meta.correlation_id = Some(correlation_id.clone());

                let mut msg = MPMessage {
                    id: msg_id,
                    msg_type: MPMessageType::Ack,
                    from: String::new(),
                    to: to_did,
                    action: "ack".to_string(),
                    payload: Value::Null,
                    meta,
                    signature: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    version: MP_VERSION.to_string(),
                };

                if let Some(ref sec) = self.secret {
                    msg.sign(sec);
                }
                Ok(msg)
            }

            MLStmt::Error {
                to,
                correlation_id,
                message,
            } => {
                let to_did = self.resolve_did(to)?;
                let msg_id = Uuid::new_v4().to_string();
                let mut meta = MPMessageMeta::default();
                meta.correlation_id = correlation_id.clone();

                let payload = serde_json::json!({ "message": message });

                let mut msg = MPMessage {
                    id: msg_id,
                    msg_type: MPMessageType::Error,
                    from: String::new(),
                    to: to_did,
                    action: "error".to_string(),
                    payload,
                    meta,
                    signature: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    version: MP_VERSION.to_string(),
                };

                if let Some(ref sec) = self.secret {
                    msg.sign(sec);
                }
                Ok(msg)
            }

            MLStmt::Hello {
                agent,
                protocols,
                capabilities,
                trust_requirements,
            } => {
                let agent_did = self.resolve_did(agent)?;
                let msg_id = Uuid::new_v4().to_string();
                let payload = serde_json::json!({
                    "agent": agent.clone(),
                    "protocols": protocols.clone(),
                    "capabilities": capabilities.clone(),
                    "trust_requirements": trust_requirements.clone(),
                });

                let mut msg = MPMessage {
                    id: msg_id,
                    msg_type: MPMessageType::Hello,
                    from: agent_did,
                    to: "agent:broadcast".to_string(),
                    action: "hello".to_string(),
                    payload,
                    meta: MPMessageMeta::default(),
                    signature: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    version: MP_VERSION.to_string(),
                };

                if let Some(ref sec) = self.secret {
                    msg.sign(sec);
                }
                Ok(msg)
            }

            MLStmt::Bye { agent, reason } => {
                let agent_did = self.resolve_did(agent)?;
                let msg_id = Uuid::new_v4().to_string();
                let mut payload = serde_json::json!({ "agent": agent.clone() });
                if let Some(ref r) = reason {
                    payload["reason"] = Value::String(r.clone());
                }

                let mut msg = MPMessage {
                    id: msg_id,
                    msg_type: MPMessageType::Bye,
                    from: agent_did,
                    to: "agent:broadcast".to_string(),
                    action: "bye".to_string(),
                    payload,
                    meta: MPMessageMeta::default(),
                    signature: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    version: MP_VERSION.to_string(),
                };

                if let Some(ref sec) = self.secret {
                    msg.sign(sec);
                }
                Ok(msg)
            }

            MLStmt::Action { action, args } => {
                let msg_id = Uuid::new_v4().to_string();
                let payload = serde_json::json!({
                    "args": serde_json::Map::from_iter(
                        args.iter().map(|(k, v)| (k.clone(), v.clone()))
                    )
                });

                let mut msg = MPMessage {
                    id: msg_id,
                    msg_type: MPMessageType::Request,
                    from: String::new(),
                    to: String::new(),
                    action: action.clone(),
                    payload,
                    meta: MPMessageMeta::default(),
                    signature: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    version: MP_VERSION.to_string(),
                };

                if let Some(ref sec) = self.secret {
                    msg.sign(sec);
                }
                Ok(msg)
            }
        }
    }

    /// Resolve an ML agent name to an MP DID string.
    fn resolve_did(&self, name: &str) -> Result<String, ProtocolError> {
        // Check if it's already a full DID
        if name.starts_with("agent:") {
            return Ok(name.to_string());
        }
        // Try local registry
        if let Some(did) = self.identity_map.resolve_to_did(name) {
            return Ok(did.into());
        }
        // Default: treat as Memphis agent
        Ok(format!("{}/{}", crate::MEMPHIS_DID_PREFIX, name))
    }
}

// ---------------------------------------------------------------------------
// MP → ML Translator
// ---------------------------------------------------------------------------

/// Translates MP messages back to ML expressions.
#[derive(Debug, Clone, Default)]
pub struct MpToMlTranslator {
    identity_map: AgentIdentityMap,
}

impl MpToMlTranslator {
    pub fn new(identity_map: AgentIdentityMap) -> Self {
        Self { identity_map }
    }

    /// Translate an MPMessage to an ML S-expression string.
    pub fn translate(&self, msg: &MPMessage) -> Result<String, ProtocolError> {
        msg.validate()?;
        let expr = self.translate_to_expr(msg)?;
        Ok(self.expr_to_string(&expr, 0))
    }

    /// Translate an MPMessage to an MLExpr AST.
    pub fn translate_to_expr(&self, msg: &MPMessage) -> Result<MLExpr, ProtocolError> {
        // Validate first
        msg.validate()?;

        let stmt = match msg.msg_type {
            MPMessageType::Request => {
                let from_name = self.did_to_local_name(&msg.from);
                let to_name = self.did_to_local_name(&msg.to);
                MLStmt::Request {
                    from: from_name,
                    to: to_name,
                    action: msg.action.clone(),
                    payload: msg.payload.clone(),
                    deadline: msg.meta.deadline.clone(),
                    trust_level: msg.meta.trust_level.clone(),
                    confidential: msg.meta.confidential,
                    correlation_id: msg.meta.correlation_id.clone(),
                }
            }
            MPMessageType::Response => MLStmt::Response {
                to: self.did_to_local_name(&msg.to),
                correlation_id: msg
                    .meta
                    .correlation_id
                    .clone()
                    .unwrap_or_else(|| msg.id.clone()),
                payload: msg.payload.clone(),
            },
            MPMessageType::Ack => MLStmt::Ack {
                to: self.did_to_local_name(&msg.to),
                correlation_id: msg
                    .meta
                    .correlation_id
                    .clone()
                    .unwrap_or_else(|| msg.id.clone()),
            },
            MPMessageType::Error => MLStmt::Error {
                to: self.did_to_local_name(&msg.to),
                correlation_id: msg.meta.correlation_id.clone(),
                message: msg
                    .payload
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error")
                    .to_string(),
            },
            MPMessageType::Hello => {
                let agent = msg
                    .payload
                    .get("agent")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let protocols = msg
                    .payload
                    .get("protocols")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let capabilities = msg
                    .payload
                    .get("capabilities")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let trust_requirements = msg
                    .payload
                    .get("trust_requirements")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                MLStmt::Hello {
                    agent,
                    protocols,
                    capabilities,
                    trust_requirements,
                }
            }
            MPMessageType::Bye => {
                let agent = msg
                    .payload
                    .get("agent")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let reason = msg.payload.get("reason").and_then(|v| v.as_str()).map(String::from);
                MLStmt::Bye { agent, reason }
            }
        };

        Ok(MLExpr::Top(Box::new(stmt)))
    }

    /// Convert an MP DID to a local ML agent name.
    /// e.g. "agent:memphis/marcin" -> "marcin"
    fn did_to_local_name(&self, did: &str) -> String {
        if did.starts_with(crate::MEMPHIS_DID_PREFIX) {
            did.strip_prefix(&format!("{}/", crate::MEMPHIS_DID_PREFIX))
                .unwrap_or(did)
                .to_string()
        } else if did.starts_with(crate::FOREIGN_DID_PREFIX) {
            did.strip_prefix(&format!("{}/", crate::FOREIGN_DID_PREFIX))
                .map(|s| s.replace('/', "."))
                .unwrap_or_else(|| did.to_string())
        } else {
            did.to_string()
        }
    }

    /// Pretty-print an MLExpr to a string.
    fn expr_to_string(&self, expr: &MLExpr, _indent: usize) -> String {
        match expr {
            MLExpr::Top(stmt) => self.stmt_to_string(stmt),
            MLExpr::Value(v) => self.value_to_string(v),
        }
    }

    fn stmt_to_string(&self, stmt: &MLStmt) -> String {
        match stmt {
            MLStmt::Request {
                from,
                to,
                action,
                payload,
                deadline,
                trust_level,
                confidential,
                correlation_id,
            } => {
                let mut parts = vec![
                    format!("(request :from {}", from),
                    format!(":to {}", to),
                    format!(":action {}", action),
                ];
                if let Some(ref dl) = deadline {
                    parts.push(format!(":deadline \"{dl}\""));
                }
                if let Some(ref tl) = trust_level {
                    parts.push(format!(":trust {}", format!("{:?}", tl).to_lowercase()));
                }
                if *confidential {
                    parts.push(":confidential true".to_string());
                }
                if let Some(ref cid) = correlation_id {
                    parts.push(format!(":correlation-id \"{cid}\""));
                }
                parts.push(format!(":payload {}", self.json_to_ml(&payload)));
                parts.push(")".to_string());
                parts.join(" ")
            }
            MLStmt::Response {
                to,
                correlation_id,
                payload,
            } => {
                format!(
                    "(response :to {} :correlation-id \"{}\" :payload {})",
                    to,
                    correlation_id,
                    self.json_to_ml(payload)
                )
            }
            MLStmt::Ack {
                to,
                correlation_id,
            } => {
                format!(
                    "(ack :to {} :correlation-id \"{}\")",
                    to, correlation_id
                )
            }
            MLStmt::Error {
                to,
                correlation_id,
                message,
            } => {
                if let Some(ref cid) = correlation_id {
                    format!(
                        "(error :to {} :correlation-id \"{}\" :message \"{}\")",
                        to, cid, message
                    )
                } else {
                    format!(
                        "(error :to {} :message \"{}\")",
                        to, message
                    )
                }
            }
            MLStmt::Hello {
                agent,
                protocols,
                capabilities,
                trust_requirements,
            } => {
                let protocols_str = self.list_to_ml(
                    &protocols.iter().map(|p| MLValue::String(p.clone())).collect::<Vec<_>>(),
                );
                let caps_str = self.list_to_ml(
                    &capabilities.iter().map(|c| MLValue::String(c.clone())).collect::<Vec<_>>(),
                );
                let reqs_str = self.list_to_ml(
                    &trust_requirements
                        .iter()
                        .map(|r| MLValue::String(r.clone()))
                        .collect::<Vec<_>>(),
                );
                format!(
                    "(hello :agent {} :protocols {} :capabilities {} :trust-requirements {})",
                    agent, protocols_str, caps_str, reqs_str
                )
            }
            MLStmt::Bye { agent, reason } => {
                if let Some(ref r) = reason {
                    format!("(bye :agent {} :reason \"{r}\")", agent)
                } else {
                    format!("(bye :agent {})", agent)
                }
            }
            MLStmt::Action { action, args } => {
                let args_str = args
                    .iter()
                    .map(|(k, v)| format!(":{k} {}", self.json_to_ml(v)))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("({action} {args_str})")
            }
        }
    }

    fn value_to_string(&self, v: &MLValue) -> String {
        match v {
            MLValue::Number(n) => n.to_string(),
            MLValue::String(s) => format!("\"{s}\""),
            MLValue::Boolean(true) => "true".to_string(),
            MLValue::Boolean(false) => "false".to_string(),
            MLValue::Symbol(s) => s.clone(),
            MLValue::List(items) => self.list_to_ml(items),
            MLValue::Record(m) => {
                let pairs = m
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", self.json_to_ml(v)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{}}}", pairs)
            }
            MLValue::Null => "nil".to_string(),
        }
    }

    fn list_to_ml(&self, items: &[MLValue]) -> String {
        format!("[{}]", items.iter().map(|v| self.value_to_string(v)).collect::<Vec<_>>().join(" "))
    }

    fn json_to_ml(&self, v: &Value) -> String {
        match v {
            Value::Null => "nil".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => format!("\"{s}\""),
            Value::Array(arr) => {
                format!(
                    "[{}]",
                    arr.iter().map(|x| self.json_to_ml(x)).collect::<Vec<_>>().join(" ")
                )
            }
            Value::Object(m) => {
                let pairs = m
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", self.json_to_ml(v)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{}}}", pairs)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Bidirectional Translator
// ---------------------------------------------------------------------------

/// Full bidirectional ML ↔ MP translator.
#[derive(Debug, Clone, Default)]
pub struct BidirectionalTranslator {
    ml_to_mp: MlToMpTranslator,
    mp_to_ml: MpToMlTranslator,
}

impl BidirectionalTranslator {
    pub fn new(identity_map: AgentIdentityMap) -> Self {
        Self {
            ml_to_mp: MlToMpTranslator::new(identity_map.clone()),
            mp_to_ml: MpToMlTranslator::new(identity_map),
        }
    }

    pub fn with_secret(mut self, secret: &[u8]) -> Self {
        self.ml_to_mp = self.ml_to_mp.with_secret(secret);
        self
    }

    /// Translate ML expression to MP JSON bytes.
    pub fn ml_to_mp_json(&self, expr: &MLExpr) -> Result<Vec<u8>, ProtocolError> {
        let msg = self.ml_to_mp.translate(expr)?;
        msg.validate()?;
        msg.to_json()
    }

    /// Translate MP JSON bytes to ML S-expression string.
    pub fn mp_to_ml_string(&self, bytes: &[u8]) -> Result<String, ProtocolError> {
        let msg = MPMessage::from_json(bytes)?;
        self.mp_to_ml.translate(&msg)
    }

    /// Translate MP JSON bytes to MLExpr AST.
    pub fn mp_to_ml_expr(&self, bytes: &[u8]) -> Result<MLExpr, ProtocolError> {
        let msg = MPMessage::from_json(bytes)?;
        self.mp_to_ml.translate_to_expr(&msg)
    }

    /// Round-trip: ML -> MP -> ML
    pub fn roundtrip_ml(&self, expr: &MLExpr) -> Result<String, ProtocolError> {
        let json = self.ml_to_mp_json(expr)?;
        self.mp_to_ml_string(&json)
    }

    /// Round-trip: MP -> ML -> MP (re-serialize)
    pub fn roundtrip_mp(&self, bytes: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let expr = self.mp_to_ml_expr(bytes)?;
        self.ml_to_mp_json(&expr)
    }
}

// ---------------------------------------------------------------------------
// ML S-expression parser (simple recursive descent)
// ---------------------------------------------------------------------------

/// Parse an ML S-expression string into an MLExpr AST.
pub fn parse_ml(s: &str) -> Result<MLExpr, ProtocolError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ProtocolError::TranslationError("empty input".into()));
    }

    if !s.starts_with('(') {
        // Bare value — try to parse as a literal
        return Ok(MLExpr::Value(parse_value(s)?));
    }

    // Collect tokens
    let tokens = tokenize(s)?;
    let mut tokens = TokenBuffer::new(tokens.into_iter());

    // Skip opening paren
    tokens.next();

    let keyword = tokens
        .next()
        .ok_or_else(|| ProtocolError::TranslationError("empty list".into()))?;

    let stmt = match keyword.as_str() {
        "request" => parse_request(&mut tokens)?,
        "response" => parse_response(&mut tokens)?,
        "ack" => parse_ack(&mut tokens)?,
        "error" => parse_error(&mut tokens)?,
        "hello" => parse_hello(&mut tokens)?,
        "bye" => parse_bye(&mut tokens)?,
        _ => {
            // Generic action: (action :key val ...)
            let args = parse_keyword_args(&mut tokens)?;
            MLStmt::Action {
                action: keyword,
                args,
            }
        }
    };

    // Closing paren check removed - handled in main loop

    Ok(MLExpr::Top(Box::new(stmt)))
}

fn tokenize(s: &str) -> Result<Vec<String>, ProtocolError> {
    let s = s.trim();
    let mut tokens = Vec::new();
    let mut chars = s.chars().peekable();
    let mut current = String::new();

    while let Some(c) = chars.next() {
        match c {
            '(' | ')' | '[' | ']' | '{' | '}' | ':' => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                tokens.push(c.to_string());
            }
            '"' => {
                // String literal
                current.push(c);
                while let Some(ch) = chars.next() {
                    current.push(ch);
                    if ch == '"' {
                        break;
                    }
                }
                tokens.push(current.clone());
                current.clear();
            }
            ' ' | '\t' | '\n' | '\r' => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

fn parse_request(tokens: &mut TokenBuffer<std::vec::IntoIter<String>>) -> Result<MLStmt, ProtocolError> {
    let mut from = None;
    let mut to = None;
    let mut action = None;
    let mut payload = Value::Null;
    let mut deadline = None;
    let mut trust_level = None;
    let mut confidential = false;
    let mut correlation_id = None;

    while let Some(tok) = tokens.next() {
        if tok == ")" {
            tokens.push_back(tok);
            break;
        }
        match tok.as_str() {
            ":from" | "from" => {
                from = Some(tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :from value".into())
                })?);
            }
            ":to" | "to" => {
                to = Some(tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :to value".into())
                })?);
            }
            ":action" | "action" => {
                action = Some(tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :action value".into())
                })?);
            }
            ":payload" | "payload" => {
                payload = parse_value_from_tokens(tokens)?;
            }
            ":deadline" | "deadline" => {
                deadline = Some(tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :deadline value".into())
                })?);
            }
            ":trust" | "trust" => {
                let val = tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :trust value".into())
                })?;
                trust_level = Some(match val.to_lowercase().as_str() {
                    "high" => TrustLevel::High,
                    "medium" => TrustLevel::Medium,
                    "low" => TrustLevel::Low,
                    "none" => TrustLevel::None,
                    _ => return Err(ProtocolError::TranslationError(
                        format!("unknown trust level: {val}").into(),
                    )),
                });
            }
            ":confidential" | "confidential" => {
                let val = tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :confidential value".into())
                })?;
                confidential = val == "true" || val == "true";
            }
            ":correlation-id" | "correlation-id" => {
                correlation_id = Some(tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :correlation-id value".into())
                })?);
            }
            _ => {
                // Skip unknown tokens
            }
        }
    }

    Ok(MLStmt::Request {
        from: from.ok_or_else(|| ProtocolError::TranslationError(":from required".into()))?,
        to: to.ok_or_else(|| ProtocolError::TranslationError(":to required".into()))?,
        action: action.unwrap_or_else(|| "request".to_string()),
        payload,
        deadline,
        trust_level,
        confidential,
        correlation_id,
    })
}

fn parse_response(tokens: &mut TokenBuffer<std::vec::IntoIter<String>>) -> Result<MLStmt, ProtocolError> {
    let mut to = None;
    let mut correlation_id = None;
    let mut payload = Value::Null;

    while let Some(tok) = tokens.next() {
        if tok == ")" {
            tokens.push_back(tok);
            break;
        }
        match tok.as_str() {
            ":to" | "to" => {
                to = Some(tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :to value".into())
                })?);
            }
            ":correlation-id" | "correlation-id" => {
                correlation_id = Some(tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :correlation-id value".into())
                })?);
            }
            ":payload" | "payload" => {
                payload = parse_value_from_tokens(tokens)?;
            }
            _ => {}
        }
    }

    Ok(MLStmt::Response {
        to: to.ok_or_else(|| ProtocolError::TranslationError(":to required".into()))?,
        correlation_id: correlation_id.ok_or_else(|| {
            ProtocolError::TranslationError(":correlation-id required".into())
        })?,
        payload,
    })
}

fn parse_ack(tokens: &mut TokenBuffer<std::vec::IntoIter<String>>) -> Result<MLStmt, ProtocolError> {
    let mut to = None;
    let mut correlation_id = None;

    while let Some(tok) = tokens.next() {
        if tok == ")" {
            tokens.push_back(tok);
            break;
        }
        match tok.as_str() {
            ":to" | "to" => {
                to = Some(tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :to value".into())
                })?);
            }
            ":correlation-id" | "correlation-id" => {
                correlation_id = Some(tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :correlation-id value".into())
                })?);
            }
            _ => {}
        }
    }

    Ok(MLStmt::Ack {
        to: to.ok_or_else(|| ProtocolError::TranslationError(":to required".into()))?,
        correlation_id: correlation_id.ok_or_else(|| {
            ProtocolError::TranslationError(":correlation-id required".into())
        })?,
    })
}

fn parse_error(tokens: &mut TokenBuffer<std::vec::IntoIter<String>>) -> Result<MLStmt, ProtocolError> {
    let mut to = None;
    let mut correlation_id = None;
    let mut message = None;

    while let Some(tok) = tokens.next() {
        if tok == ")" {
            tokens.push_back(tok);
            break;
        }
        match tok.as_str() {
            ":to" | "to" => {
                to = Some(tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :to value".into())
                })?);
            }
            ":correlation-id" | "correlation-id" => {
                correlation_id = Some(tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :correlation-id value".into())
                })?);
            }
            ":message" | "message" => {
                message = Some(tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :message value".into())
                })?);
            }
            _ => {}
        }
    }

    Ok(MLStmt::Error {
        to: to.ok_or_else(|| ProtocolError::TranslationError(":to required".into()))?,
        correlation_id,
        message: message.unwrap_or_else(|| "unknown error".to_string()),
    })
}

fn parse_hello(tokens: &mut TokenBuffer<std::vec::IntoIter<String>>) -> Result<MLStmt, ProtocolError> {
    let mut agent = None;
    let mut protocols = Vec::new();
    let mut capabilities = Vec::new();
    let mut trust_requirements = Vec::new();

    while let Some(tok) = tokens.next() {
        if tok == ")" {
            tokens.push_back(tok);
            break;
        }
        match tok.as_str() {
            ":agent" | "agent" => {
                agent = Some(tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :agent value".into())
                })?);
            }
            ":protocols" | "protocols" => {
                protocols = parse_list_from_tokens(tokens)?;
            }
            ":capabilities" | "capabilities" => {
                capabilities = parse_list_from_tokens(tokens)?;
            }
            ":trust-requirements" | "trust-requirements" => {
                trust_requirements = parse_list_from_tokens(tokens)?;
            }
            _ => {}
        }
    }

    Ok(MLStmt::Hello {
        agent: agent.ok_or_else(|| ProtocolError::TranslationError(":agent required".into()))?,
        protocols,
        capabilities,
        trust_requirements,
    })
}

fn parse_bye(tokens: &mut TokenBuffer<std::vec::IntoIter<String>>) -> Result<MLStmt, ProtocolError> {
    let mut agent = None;
    let mut reason = None;

    while let Some(tok) = tokens.next() {
        if tok == ")" {
            tokens.push_back(tok);
            break;
        }
        match tok.as_str() {
            ":agent" | "agent" => {
                agent = Some(tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :agent value".into())
                })?);
            }
            ":reason" | "reason" => {
                reason = Some(tokens.next().ok_or_else(|| {
                    ProtocolError::TranslationError("missing :reason value".into())
                })?);
            }
            _ => {}
        }
    }

    Ok(MLStmt::Bye {
        agent: agent.ok_or_else(|| ProtocolError::TranslationError(":agent required".into()))?,
        reason,
    })
}

fn parse_keyword_args(
    tokens: &mut TokenBuffer<std::vec::IntoIter<String>>,
) -> Result<Vec<(String, Value)>, ProtocolError> {
    let mut args = Vec::new();
    while let Some(tok) = tokens.next() {
        if tok == ")" {
            tokens.push_back(tok);
            break;
        }
        if tok.starts_with(':') {
            let key = tok.trim_start_matches(':').to_string();
            let val = parse_value_from_tokens(tokens)?;
            args.push((key, val));
        }
    }
    Ok(args)
}

fn parse_list_from_tokens(
    tokens: &mut TokenBuffer<std::vec::IntoIter<String>>,
) -> Result<Vec<String>, ProtocolError> {
    // Could be ML list: [item1 item2] or bare symbol
    let first = tokens.next();
    match first {
        Some(tok) if tok == "[" => {
            let mut items = Vec::new();
            while let Some(item) = tokens.next() {
                if item == "]" {
                    break;
                }
                items.push(item.trim_matches('"').to_string());
            }
            Ok(items)
        }
        Some(tok) => {
            // Single value
            Ok(vec![tok.trim_matches('"').to_string()])
        }
        None => Ok(Vec::new()),
    }
}

fn parse_value(s: &str) -> Result<MLValue, ProtocolError> {
    let s = s.trim();
    if s == "nil" || s == "null" {
        return Ok(MLValue::Null);
    }
    if s == "true" {
        return Ok(MLValue::Boolean(true));
    }
    if s == "false" {
        return Ok(MLValue::Boolean(false));
    }
    if s.starts_with('"') && s.ends_with('"') {
        return Ok(MLValue::String(s[1..s.len() - 1].to_string()));
    }
    if let Ok(n) = s.parse::<f64>() {
        return Ok(MLValue::Number(n));
    }
    if s.starts_with('[') && s.ends_with(']') {
        // Parse list content
        let inner = &s[1..s.len() - 1];
        if inner.trim().is_empty() {
            return Ok(MLValue::List(Vec::new()));
        }
        let items: Result<Vec<MLValue>, _> =
            inner.split_whitespace().map(|x| parse_value(x)).collect();
        return items.map(MLValue::List);
    }
    if s.starts_with('{') && s.ends_with('}') {
        // Parse record — simplified
        return Ok(MLValue::Record(serde_json::Map::new()));
    }
    Ok(MLValue::Symbol(s.to_string()))
}

fn parse_value_from_tokens(
    tokens: &mut TokenBuffer<std::vec::IntoIter<String>>,
) -> Result<Value, ProtocolError> {
    let Some(tok) = tokens.next() else {
        return Ok(Value::Null);
    };

    if tok == "[" {
        let mut items = Vec::new();
        while let Some(item) = tokens.next() {
            if item == "]" {
                break;
            }
            // Put back so parse_value can handle it
            tokens.push_back(item);
            let val = parse_value_from_tokens(tokens)?;
            items.push(val);
        }
        return Ok(Value::Array(items));
    }

    if tok == "{" {
        let mut map = serde_json::Map::new();
        while let Some(key) = tokens.next() {
            if key == "}" {
                break;
            }
            let val = parse_value_from_tokens(tokens)?;
            let clean_key = key.trim_matches(':');
            map.insert(clean_key.to_string(), val);
        }
        return Ok(Value::Object(map));
    }

    let v = parse_value(&tok)?;
    Ok(v.to_json())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_identity_map() -> AgentIdentityMap {
        let mut map = AgentIdentityMap::new();
        map.register_local("marcin");
        map.register_local("synjar");
        map.register_foreign("synjar", "synjar", "ml-v1", "matrix", "localhost:3777", true, vec!["search".to_string()]);
        map
    }

    #[test]
    fn test_ml_request_to_mp() {
        let map = test_identity_map();
        let translator = MlToMpTranslator::new(map);

        let ml = parse_ml(r#"(request :from marcin :to synjar :action search :payload {query: "docs"})"#).unwrap();
        let msg = translator.translate(&ml).unwrap();

        assert_eq!(msg.msg_type, MPMessageType::Request);
        assert_eq!(msg.from, "agent:memphis/marcin");
        assert_eq!(msg.to, "agent:synjar");
        assert_eq!(msg.action, "search");
        assert!(msg.signature.is_some());
    }

    #[test]
    fn test_mp_to_ml_request() {
        let map = test_identity_map();
        let translator = MpToMlTranslator::new(map);

        let json = serde_json::json!({
            "id": "msg-001",
            "type": "request",
            "from": "agent:memphis/marcin",
            "to": "agent:synjar",
            "action": "search",
            "payload": { "query": "documents" },
            "meta": {
                "deadline": "2026-03-28T00:00:00Z",
                "trust_level": "high",
                "confidential": true
            },
            "timestamp": "2026-03-27T20:30:00Z",
            "version": "mp-v1"
        });

        let msg: MPMessage = serde_json::from_value(json).unwrap();
        let ml_str = translator.translate(&msg).unwrap();

        assert!(ml_str.contains("(request"));
        assert!(ml_str.contains(":from marcin"));
        assert!(ml_str.contains(":to synjar"));
    }

    #[test]
    fn test_roundtrip() {
        let map = test_identity_map();
        let translator = BidirectionalTranslator::new(map);

        let ml = parse_ml(r#"(request :from marcin :to synjar :action search :payload {query: "docs"})"#).unwrap();
        let roundtripped = translator.roundtrip_ml(&ml).unwrap();

        assert!(roundtripped.contains("(request"));
    }

    #[test]
    fn test_hello_bye_ml() {
        let map = test_identity_map();

        let ml_hello = parse_ml(r#"(hello :agent marcin :protocols [ml-v1 mp-v1] :capabilities [search recall])"#).unwrap();
        let t1 = MlToMpTranslator::new(map.clone());
        let msg = t1.translate(&ml_hello).unwrap();
        assert_eq!(msg.msg_type, MPMessageType::Hello);

        let ml_bye = parse_ml(r#"(bye :agent marcin :reason "done")"#).unwrap();
        let msg2 = t1.translate(&ml_bye).unwrap();
        assert_eq!(msg2.msg_type, MPMessageType::Bye);
    }

    #[test]
    fn test_identity_resolution() {
        let mut map = AgentIdentityMap::new();
        map.register_local("marcin");
        map.register_foreign("synjar", "synjar", "ml-v1", "http", "localhost:8080", false, vec![]);

        let t = MlToMpTranslator::new(map);

        let did1 = t.resolve_did("marcin").unwrap();
        assert_eq!(did1, "agent:memphis/marcin");

        let did2 = t.resolve_did("agent:synjar").unwrap();
        assert_eq!(did2, "agent:synjar");
    }

    #[test]
    fn test_message_validation() {
        // Valid message
        let json = serde_json::json!({
            "id": "msg-001",
            "type": "request",
            "from": "agent:memphis/marcin",
            "to": "agent:synjar",
            "action": "search",
            "payload": {},
            "meta": {},
            "timestamp": "2026-03-27T20:30:00Z",
            "version": "mp-v1"
        });
        let msg: MPMessage = serde_json::from_value(json).unwrap();
        assert!(msg.validate().is_ok());

        // Invalid: empty ID
        let bad = MPMessage {
            id: String::new(),
            ..msg.clone()
        };
        assert!(bad.validate().is_err());

        // Invalid: bad DID
        let bad2 = MPMessage { from: "not-a-did".to_string(), ..msg.clone() };
        assert!(bad2.validate().is_err());
    }

    #[test]
    fn test_signature_roundtrip() {
        let secret = b"test-secret-key";
        let map = test_identity_map();
        let translator = BidirectionalTranslator::new(map).with_secret(secret);

        let ml = parse_ml(r#"(request :from marcin :to synjar :action search :payload {query: "test"})"#).unwrap();
        let json_bytes = translator.ml_to_mp_json(&ml).unwrap();

        // Signature should be present
        let msg = MPMessage::from_json(&json_bytes).unwrap();
        assert!(msg.signature.is_some());

        // Verify signature
        msg.verify_signature(secret).unwrap();
    }
}
