//! # ml-protocol — ML ↔ Memphis Protocol (MP) translation
//!
//! Memphis Protocol (MP) is the signed, timestamped JSON format used for
//! agent-to-agent communication in the Agora Federation.
//!
//! This crate provides:
//! - Translation between ML S-expressions and MP JSON messages
//! - Agent identity mapping (ML agent DID ↔ MP DID)
//! - MP JSON schema validation
//! - Message signing and verification

mod translator;

pub use translator::*;
pub use error::ProtocolError;

/// Memphis Protocol (MP) message version
pub const MP_VERSION: &str = "mp-v1";

/// Agent DID prefix for Memphis agents
pub const MEMPHIS_DID_PREFIX: &str = "agent:memphis";

/// Agent DID prefix for foreign agents
pub const FOREIGN_DID_PREFIX: &str = "agent:foreign";

/// Minimum deadline offset (1 hour in seconds)
pub const MIN_DEADLINE_SECS: i64 = 3600;

/// Maximum deadline offset (30 days in seconds)
pub const MAX_DEADLINE_SECS: i64 = 2_592_000;

// ---------------------------------------------------------------------------
// Core MP Message types
// ---------------------------------------------------------------------------

use serde::{Deserialize, Serialize};

/// The main Memphis Protocol message envelope.
///
/// All fields are required unless marked optional.
///
/// # Example
/// ```json
/// {
///   "id": "msg-2026-03-27-001",
///   "type": "request",
///   "from": "agent:memphis/marcin",
///   "to": "agent:synjar",
///   "action": "search",
///   "payload": { "query": "documents" },
///   "meta": {
///     "deadline": "2026-03-28T00:00:00Z",
///     "trust_level": "high",
///     "confidential": true
///   },
///   "signature": "base64-sha256-...",
///   "timestamp": "2026-03-27T20:30:00Z",
///   "version": "mp-v1"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MPMessage {
    /// Unique message identifier (ULID or UUID)
    pub id: String,

    /// Message type
    #[serde(rename = "type")]
    pub msg_type: MPMessageType,

    /// Sender DID (agent:memphis/... or agent:foreign/...)
    pub from: String,

    /// Recipient DID
    pub to: String,

    /// Action/verb being requested (search, recall, execute, etc.)
    pub action: String,

    /// Message payload — arbitrary JSON value
    #[serde(default)]
    pub payload: serde_json::Value,

    /// Metadata (deadline, trust_level, confidential, etc.)
    #[serde(default)]
    pub meta: MPMessageMeta,

    /// Base64-encoded SHA-256 signature over canonical JSON
    pub signature: Option<String>,

    /// ISO 8601 timestamp of message creation
    pub timestamp: String,

    /// MP protocol version
    #[serde(default = "default_version")]
    pub version: String,
}

fn default_version() -> String {
    MP_VERSION.to_string()
}

/// MP message types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MPMessageType {
    Request,
    Response,
    Ack,
    Error,
    Hello,
    Bye,
}

impl std::fmt::Display for MPMessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MPMessageType::Request => write!(f, "request"),
            MPMessageType::Response => write!(f, "response"),
            MPMessageType::Ack => write!(f, "ack"),
            MPMessageType::Error => write!(f, "error"),
            MPMessageType::Hello => write!(f, "hello"),
            MPMessageType::Bye => write!(f, "bye"),
        }
    }
}

/// Metadata attached to an MP message
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MPMessageMeta {
    /// Deadline for receiving a response (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,

    /// Trust level expected for this interaction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_level: Option<TrustLevel>,

    /// Whether the payload contains confidential data
    #[serde(default)]
    pub confidential: bool,

    /// Reply-to address if different from `from`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,

    /// Correlation ID for request/response pairing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,

    /// Custom metadata key-value pairs
    #[serde(default)]
    pub extras: serde_json::Map<String, serde_json::Value>,
}

/// Trust levels for agent-to-agent interactions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TrustLevel {
    High,
    Medium,
    Low,
    None,
}

impl Default for TrustLevel {
    fn default() -> Self {
        TrustLevel::Medium
    }
}

// ---------------------------------------------------------------------------
// Agent Identity
// ---------------------------------------------------------------------------

/// Represents an agent's Distributed Identifier (DID).
///
/// Memphis DIDs follow the scheme: `agent:memphis/<local-name>`
/// Foreign DIDs follow: `agent:foreign/<system>/<name>`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentDid(String);

impl AgentDid {
    /// Parse a DID string into an AgentDid.
    pub fn parse(raw: &str) -> Result<Self, ProtocolError> {
        if raw.is_empty() {
            return Err(ProtocolError::InvalidDid("DID cannot be empty".into()));
        }
        if !raw.starts_with("agent:") {
            return Err(ProtocolError::InvalidDid(format!(
                "DID must start with 'agent:', got: {raw}"
            )));
        }
        Ok(Self(raw.to_string()))
    }

    /// Create a Memphis DID from a local agent name.
    pub fn memphis(name: &str) -> Self {
        Self(format!("{MEMPHIS_DID_PREFIX}/{name}"))
    }

    /// Create a foreign DID.
    pub fn foreign(system: &str, name: &str) -> Self {
        Self(format!("{FOREIGN_DID_PREFIX}/{system}/{name}"))
    }

    /// Returns the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns true if this is a Memphis DID.
    pub fn is_memphis(&self) -> bool {
        self.0.starts_with(MEMPHIS_DID_PREFIX)
    }

    /// Returns true if this is a foreign DID.
    pub fn is_foreign(&self) -> bool {
        self.0.starts_with(FOREIGN_DID_PREFIX)
    }

    /// Extract the local name from a Memphis DID.
    /// Returns None for foreign DIDs.
    pub fn local_name(&self) -> Option<&str> {
        self.0.strip_prefix(&format!("{MEMPHIS_DID_PREFIX}/")).filter(|s| !s.is_empty())
    }
}

impl std::fmt::Display for AgentDid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<AgentDid> for String {
    fn from(did: AgentDid) -> Self {
        did.0
    }
}

/// Maps ML agent identifiers (local names, DIDs) to MP DID strings.
#[derive(Debug, Clone, Default)]
pub struct AgentIdentityMap {
    /// Maps local ML agent name -> AgentDid
    local: std::collections::HashMap<String, AgentDid>,
    /// Maps any DID string -> metadata
    registry: std::collections::HashMap<String, AgentRegistryEntry>,
}

#[derive(Debug, Clone)]
pub struct AgentRegistryEntry {
    pub did: AgentDid,
    pub dialect: String,
    pub capabilities: Vec<String>,
    pub transport: String,
    pub endpoint: String,
    pub can_translate: bool,
}

impl AgentIdentityMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a local Memphis agent.
    pub fn register_local(&mut self, name: &str) {
        let did = AgentDid::memphis(name);
        self.local.insert(name.to_string(), did.clone());
        self.registry.insert(
            did.as_str().to_string(),
            AgentRegistryEntry {
                did,
                dialect: "ml-v1".to_string(),
                capabilities: vec![],
                transport: "local".to_string(),
                endpoint: String::new(),
                can_translate: false,
            },
        );
    }

    /// Register a foreign agent (from federation config).
    pub fn register_foreign(
        &mut self,
        name: &str,
        system: &str,
        dialect: &str,
        transport: &str,
        endpoint: &str,
        can_translate: bool,
        capabilities: Vec<String>,
    ) {
        let did = AgentDid::foreign(system, name);
        self.registry.insert(
            did.as_str().to_string(),
            AgentRegistryEntry {
                did: did.clone(),
                dialect: dialect.to_string(),
                capabilities,
                transport: transport.to_string(),
                endpoint: endpoint.to_string(),
                can_translate,
            },
        );
    }

    /// Resolve a local ML agent name to an MP DID.
    pub fn resolve_to_did(&self, name: &str) -> Option<AgentDid> {
        self.local.get(name).cloned()
    }

    /// Get registry entry for a DID.
    pub fn get_entry(&self, did: &str) -> Option<&AgentRegistryEntry> {
        self.registry.get(did)
    }

    /// Returns all registered DIDs.
    pub fn all_dids(&self) -> impl Iterator<Item = &str> {
        self.registry.keys().map(|k| k.as_str())
    }
}

// ---------------------------------------------------------------------------
// MP Message builder
// ---------------------------------------------------------------------------

impl MPMessage {
    /// Build a new request MPMessage.
    pub fn request(
        id: &str,
        from: &str,
        to: &str,
        action: &str,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: id.to_string(),
            msg_type: MPMessageType::Request,
            from: from.to_string(),
            to: to.to_string(),
            action: action.to_string(),
            payload,
            meta: MPMessageMeta::default(),
            signature: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: MP_VERSION.to_string(),
        }
    }

    /// Build a response MPMessage referencing a request.
    pub fn response(
        id: &str,
        from: &str,
        to: &str,
        correlation_id: &str,
        payload: serde_json::Value,
    ) -> Self {
        let mut meta = MPMessageMeta::default();
        meta.correlation_id = Some(correlation_id.to_string());
        Self {
            id: id.to_string(),
            msg_type: MPMessageType::Response,
            from: from.to_string(),
            to: to.to_string(),
            action: "response".to_string(),
            payload,
            meta,
            signature: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: MP_VERSION.to_string(),
        }
    }

    /// Build an ack MPMessage.
    pub fn ack(id: &str, from: &str, to: &str, correlation_id: &str) -> Self {
        let mut meta = MPMessageMeta::default();
        meta.correlation_id = Some(correlation_id.to_string());
        Self {
            id: id.to_string(),
            msg_type: MPMessageType::Ack,
            from: from.to_string(),
            to: to.to_string(),
            action: "ack".to_string(),
            payload: serde_json::Value::Null,
            meta,
            signature: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: MP_VERSION.to_string(),
        }
    }

    /// Build an error MPMessage.
    pub fn error(
        id: &str,
        from: &str,
        to: &str,
        correlation_id: Option<&str>,
        message: &str,
    ) -> Self {
        let mut meta = MPMessageMeta::default();
        meta.correlation_id = correlation_id.map(|s| s.to_string());
        Self {
            id: id.to_string(),
            msg_type: MPMessageType::Error,
            from: from.to_string(),
            to: to.to_string(),
            action: "error".to_string(),
            payload: serde_json::json!({ "message": message }),
            meta,
            signature: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: MP_VERSION.to_string(),
        }
    }

    /// Set the meta fields on this message (builder pattern).
    pub fn with_meta(mut self, meta: MPMessageMeta) -> Self {
        self.meta = meta;
        self
    }

    /// Set the deadline on this message's meta.
    pub fn with_deadline(mut self, deadline: &str) -> Self {
        self.meta.deadline = Some(deadline.to_string());
        self
    }

    /// Set the trust level on this message's meta.
    pub fn with_trust_level(mut self, level: TrustLevel) -> Self {
        self.meta.trust_level = Some(level);
        self
    }

    /// Sign this message with the given secret key material.
    /// Stores the base64-encoded SHA-256 signature.
    pub fn sign(&mut self, secret: &[u8]) {
        self.signature = Some(self.compute_signature(secret));
    }

    /// Compute the SHA-256 signature over the canonical JSON of this message.
    /// The signature field itself is excluded from the signed data.
    pub fn compute_signature(&self, secret: &[u8]) -> String {
        use sha2::{Sha256, Digest};
        // Canonical JSON without the signature field
        let canonical = serde_json::to_string(&SignedMessageView::new(self))
            .unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(secret);
        hasher.update(b":");
        hasher.update(canonical.as_bytes());
        let result = hasher.finalize();
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, result)
    }

    /// Verify the message signature. Returns Ok(()) if valid.
    pub fn verify_signature(&self, secret: &[u8]) -> Result<(), ProtocolError> {
        let Some(sig) = &self.signature else {
            return Err(ProtocolError::MissingSignature);
        };
        let expected = self.compute_signature(secret);
        if expected == *sig {
            Ok(())
        } else {
            Err(ProtocolError::InvalidSignature)
        }
    }

    /// Serialize this message to canonical JSON bytes.
    pub fn to_json(&self) -> Result<Vec<u8>, ProtocolError> {
        serde_json::to_vec(self).map_err(|e| ProtocolError::Serialization(e.to_string()))
    }

    /// Parse an MPMessage from JSON bytes.
    pub fn from_json(bytes: &[u8]) -> Result<Self, ProtocolError> {
        serde_json::from_slice(bytes).map_err(|e| ProtocolError::Parsing(e.to_string()))
    }

    /// Parse an MPMessage from a JSON string.
    pub fn from_str(s: &str) -> Result<Self, ProtocolError> {
        serde_json::from_str(s).map_err(|e| ProtocolError::Parsing(e.to_string()))
    }

    /// Validate this message against the MP schema.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_message(self)
    }
}

/// A view of MPMessage used for canonical signing (excludes signature field).
#[derive(Serialize)]
struct SignedMessageView<'a> {
    id: &'a str,
    #[serde(rename = "type")]
    msg_type: &'a MPMessageType,
    from: &'a str,
    to: &'a str,
    action: &'a str,
    payload: &'a serde_json::Value,
    meta: &'a MPMessageMeta,
    timestamp: &'a str,
    version: &'a str,
}

impl<'a> SignedMessageView<'a> {
    fn new(msg: &'a MPMessage) -> Self {
        Self {
            id: &msg.id,
            msg_type: &msg.msg_type,
            from: &msg.from,
            to: &msg.to,
            action: &msg.action,
            payload: &msg.payload,
            meta: &msg.meta,
            timestamp: &msg.timestamp,
            version: &msg.version,
        }
    }
}

// ---------------------------------------------------------------------------
// Schema validation
// ---------------------------------------------------------------------------

/// Validate an MPMessage against the MP schema rules.
/// These are the syntactic/structural validation rules.
/// Semantic validation (e.g. timestamp freshness) is handled separately.
pub fn validate_message(msg: &MPMessage) -> Result<(), ProtocolError> {
    // ID: non-empty, reasonable length
    if msg.id.is_empty() {
        return Err(ProtocolError::ValidationError("id cannot be empty".into()));
    }
    if msg.id.len() > 256 {
        return Err(ProtocolError::ValidationError("id too long (max 256)".into()));
    }

    // From/To: non-empty DIDs
    if msg.from.is_empty() {
        return Err(ProtocolError::ValidationError("from cannot be empty".into()));
    }
    if msg.to.is_empty() {
        return Err(ProtocolError::ValidationError("to cannot be empty".into()));
    }

    // DIDs must start with "agent:"
    if !msg.from.starts_with("agent:") {
        return Err(ProtocolError::InvalidDid(format!(
            "from must be agent DID, got: {}", msg.from
        )));
    }
    if !msg.to.starts_with("agent:") {
        return Err(ProtocolError::InvalidDid(format!(
            "to must be agent DID, got: {}", msg.to
        )));
    }

    // Action: non-empty
    if msg.action.is_empty() {
        return Err(ProtocolError::ValidationError("action cannot be empty".into()));
    }

    // Version must be supported
    if msg.version != MP_VERSION {
        return Err(ProtocolError::UnsupportedVersion(msg.version.clone()));
    }

    // Timestamp: valid ISO 8601
    if let Err(e) = chrono::DateTime::parse_from_rfc3339(&msg.timestamp) {
        return Err(ProtocolError::ValidationError(format!(
            "invalid timestamp: {e}"
        )));
    }

    // Deadline (if set): valid ISO 8601
    if let Some(ref dl) = msg.meta.deadline {
        if let Err(e) = chrono::DateTime::parse_from_rfc3339(dl) {
            return Err(ProtocolError::ValidationError(format!(
                "invalid deadline: {e}"
            )));
        }
    }

    // For Error type, payload should have a "message" field
    if msg.msg_type == MPMessageType::Error {
        if !msg.payload.get("message").is_some() {
            return Err(ProtocolError::ValidationError(
                "error message must have payload.message field".into(),
            ));
        }
    }

    Ok(())
}

/// Validate JSON bytes as MPMessage schema.
pub fn validate_json(bytes: &[u8]) -> Result<MPMessage, ProtocolError> {
    let msg = MPMessage::from_json(bytes)?;
    msg.validate()?;
    Ok(msg)
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

pub mod error {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub enum ProtocolError {
        InvalidDid(String),
        InvalidSignature,
        MissingSignature,
        UnsupportedVersion(String),
        Parsing(String),
        Serialization(String),
        ValidationError(String),
        TranslationError(String),
        IdentityNotFound(String),
    }

    impl std::fmt::Display for ProtocolError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ProtocolError::InvalidDid(s) => write!(f, "invalid DID: {s}"),
                ProtocolError::InvalidSignature => write!(f, "invalid signature"),
                ProtocolError::MissingSignature => write!(f, "missing signature"),
                ProtocolError::UnsupportedVersion(v) => {
                    write!(f, "unsupported MP version: {v}")
                }
                ProtocolError::Parsing(s) => write!(f, "parse error: {s}"),
                ProtocolError::Serialization(s) => write!(f, "serialization error: {s}"),
                ProtocolError::ValidationError(s) => write!(f, "validation error: {s}"),
                ProtocolError::TranslationError(s) => write!(f, "translation error: {s}"),
                ProtocolError::IdentityNotFound(s) => {
                    write!(f, "identity not found: {s}")
                }
            }
        }
    }

    impl std::error::Error for ProtocolError {}
}

pub use error::ProtocolError;
