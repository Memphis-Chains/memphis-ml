//! Memphis journal writer.
//!
//! Appends structured, signed JSON entries to `journal.chain`.
//! File-based backend — works without a live Memphis server.

use std::path::PathBuf;
use std::io::Write;
use sha2::{Sha256, Digest};
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Journal entry — one record per ML operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Unique entry id
    pub id: String,
    /// ISO-8601 timestamp
    pub timestamp: String,
    /// Program / runtime instance id
    pub program_id: String,
    /// Machine identifier
    pub machine_id: String,
    /// Operation type: gate_on | gate_off | gate_toggle | sensor_read | actuator_set
    pub op: String,
    /// Target id (gate name, sensor name, actuator name)
    pub target: String,
    /// Operation payload (state value, sensor reading, power level)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// Outcome: ok | error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    /// Error message if outcome is error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl JournalEntry {
    /// Create a new journal entry.
    pub fn new(program_id: &str, machine_id: &str, op: &str, target: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            program_id: program_id.to_string(),
            machine_id: machine_id.to_string(),
            op: op.to_string(),
            target: target.to_string(),
            value: None,
            outcome: None,
            error: None,
        }
    }

    /// Set a numeric value.
    pub fn with_value(mut self, value: impl Into<serde_json::Value>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Set outcome.
    pub fn with_outcome(mut self, outcome: &str) -> Self {
        self.outcome = Some(outcome.to_string());
        self
    }

    /// Set error.
    pub fn with_error(mut self, error: &str) -> Self {
        self.outcome = Some("error".to_string());
        self.error = Some(error.to_string());
        self
    }
}

/// Journal writer — appends signed entries to `journal.chain`.
pub struct JournalWriter {
    path: PathBuf,
}

impl JournalWriter {
    /// Create a new journal writer, storing files under `data_dir`.
    pub fn new(data_dir: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = data_dir.into().join("journal.chain");
        Ok(Self { path })
    }

    /// Append a journal entry to the chain file.
    /// Each line is a JSON object with `entry` (stringified JSON) and `signature` (SHA-256).
    pub async fn append(&self, entry: JournalEntry) -> anyhow::Result<()> {
        let json = serde_json::to_string(&entry)?;
        let signature = Self::sign_entry(&json);
        let line = format!(
            "{}\n",
            serde_json::json!({
                "entry": json,
                "signature": signature,
            })
        );

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;

        tokio::io::AsyncWriteExt::write_all(&mut file, line.as_bytes()).await?;
        Ok(())
    }

    /// Append synchronously (for non-async contexts).
    pub fn append_sync(&self, entry: JournalEntry) -> anyhow::Result<()> {
        let json = serde_json::to_string(&entry)?;
        let signature = Self::sign_entry(&json);
        let line = format!(
            "{}\n",
            serde_json::json!({
                "entry": json,
                "signature": signature,
            })
        );

        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?
            .write_all(line.as_bytes())?;

        Ok(())
    }

    fn sign_entry(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Path to the journal file.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl std::fmt::Debug for JournalWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JournalWriter")
            .field("path", &self.path)
            .finish()
    }
}
