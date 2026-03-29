//! Memphis runtime bridge.
//!
//! Bridges ML execution to Memphis journal/chains/decision logging.
//!
//! ## Architecture
//!
//! ```text
//! UserCode → TracedRuntime → Runtime<M>
//!                           → MemphisRuntimeBridge
//!                              → journal.chain
//!                              → decisions.chain
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! let bridge = MemphisRuntimeBridge::new("/data", "machine-1", "program-1")?;
//! bridge.log_gate("garage_door", "on").await?;
//! bridge.log_sensor_read("temp.living_room", 22.5).await?;
//! ```

use crate::journal::{JournalEntry, JournalWriter};
use crate::decisions::{DecisionEntry, DecisionWriter};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Errors that can occur in the Memphis runtime bridge.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("journal error: {0}")]
    Journal(#[from] anyhow::Error),
    #[error("decision error: {0}")]
    Decision(#[from] crate::decisions::DecisionError),
    #[error("backend unavailable: {0}")]
    Unavailable(String),
}

/// Memphis runtime bridge — logs ML ops to Memphis journal/chains.
///
/// Uses file-based storage (journal.chain + decisions.chain) by default.
/// This allows the bridge to work without a live Memphis server.
pub struct MemphisRuntimeBridge {
    program_id: String,
    machine_id: String,
    journal: Arc<JournalWriter>,
    decisions: Arc<Mutex<Option<DecisionWriter>>>,
    /// Fallback when Memphis server is unavailable
    fallback: BridgeFallback,
}

/// Fallback mode when Memphis is unavailable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BridgeFallback {
    /// Log to local files only (journal.chain + decisions.chain)
    FileOnly,
    /// Log and panic on first error (strict mode for testing)
    Strict,
    /// Ignore errors silently
    Silent,
}

impl Default for BridgeFallback {
    fn default() -> Self {
        Self::FileOnly
    }
}

impl MemphisRuntimeBridge {
    /// Create a new bridge, storing chain files under `data_dir`.
    ///
    /// `machine_id` — unique identifier for this machine (e.g. "memphis-pi-1")
    /// `program_id` — program / runtime instance id (e.g. "hvac-control-v2")
    pub fn new(data_dir: impl AsRef<Path>, machine_id: &str, program_id: &str) -> anyhow::Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();

        // Ensure data directory exists
        std::fs::create_dir_all(&data_dir)?;

        let journal = JournalWriter::new(&data_dir)?;
        let decisions = DecisionWriter::new(&data_dir).ok(); // ok if file creation fails

        Ok(Self {
            program_id: program_id.to_string(),
            machine_id: machine_id.to_string(),
            journal: Arc::new(journal),
            decisions: Arc::new(Mutex::new(decisions)),
            fallback: BridgeFallback::FileOnly,
        })
    }

    /// Create a bridge with explicit fallback mode.
    pub fn with_fallback(
        data_dir: impl AsRef<Path>,
        machine_id: &str,
        program_id: &str,
        fallback: BridgeFallback,
    ) -> anyhow::Result<Self> {
        let mut bridge = Self::new(data_dir, machine_id, program_id)?;
        bridge.fallback = fallback;
        Ok(bridge)
    }

    /// Log a gate-on action.
    pub async fn log_gate_on(&self, gate_id: &str) -> Result<(), BridgeError> {
        self.log_gate(gate_id, "on").await
    }

    /// Log a gate-off action.
    pub async fn log_gate_off(&self, gate_id: &str) -> Result<(), BridgeError> {
        self.log_gate(gate_id, "off").await
    }

    /// Log a gate-toggle action.
    pub async fn log_gate_toggle(&self, gate_id: &str) -> Result<(), BridgeError> {
        self.log_gate(gate_id, "toggle").await
    }

    /// Log a gate action with state.
    async fn log_gate(&self, gate_id: &str, state: &str) -> Result<(), BridgeError> {
        let op = match state {
            "on" => "gate_on",
            "off" => "gate_off",
            _ => "gate_toggle",
        };

        let entry = JournalEntry::new(&self.program_id, &self.machine_id, op, gate_id)
            .with_value(state);

        self.append_journal(entry).await?;

        // Record as a decision (gate state changes are notable events)
        self.record_decision(
            op,
            gate_id,
            serde_json::json!({ "state": state }),
            &[format!("gate_{}", state)],
            Some(&format!("gate '{}' set to {}", gate_id, state)),
        ).await;

        Ok(())
    }

    /// Log a sensor read action.
    pub async fn log_sensor_read(&self, sensor_id: &str, value: f64) -> Result<(), BridgeError> {
        let entry = JournalEntry::new(&self.program_id, &self.machine_id, "sensor_read", sensor_id)
            .with_value(value)
            .with_outcome("ok");

        self.append_journal(entry).await?;

        // Record temperature decisions for notable thresholds
        if sensor_id.starts_with("temp.") {
            self.record_temp_decision(sensor_id, value).await;
        }

        Ok(())
    }

    /// Log an actuator set action.
    pub async fn log_actuator(&self, actuator_id: &str, power: f64) -> Result<(), BridgeError> {
        let entry = JournalEntry::new(&self.program_id, &self.machine_id, "actuator_set", actuator_id)
            .with_value(power)
            .with_outcome("ok");

        self.append_journal(entry).await
    }

    /// Log a gate action result (for Machine trait implementation).
    pub async fn log_gate_result(&self, gate_id: &str, state: &str, outcome: Result<(), String>) -> Result<(), BridgeError> {
        let op = match state {
            "on" => "gate_on",
            "off" => "gate_off",
            _ => "gate_toggle",
        };

        let mut entry = JournalEntry::new(&self.program_id, &self.machine_id, op, gate_id)
            .with_value(state);

        match outcome {
            Ok(()) => { entry = entry.with_outcome("ok"); }
            Err(e) => { entry = entry.with_error(&e); }
        }

        self.append_journal(entry).await
    }

    /// Log a sensor read result (for Machine trait implementation).
    pub async fn log_sensor_result(&self, sensor_id: &str, outcome: Result<f64, String>) -> Result<(), BridgeError> {
        let mut entry = JournalEntry::new(&self.program_id, &self.machine_id, "sensor_read", sensor_id);

        match outcome {
            Ok(v) => {
                entry = entry.with_value(v).with_outcome("ok");
            }
            Err(e) => {
                entry = entry.with_error(&e);
            }
        }

        self.append_journal(entry).await
    }

    /// Append a journal entry (handles fallback modes).
    async fn append_journal(&self, entry: JournalEntry) -> Result<(), BridgeError> {
        let journal = Arc::clone(&self.journal);

        match journal.append(entry).await {
            Ok(()) => Ok(()),
            Err(e) => {
                match self.fallback {
                    BridgeFallback::Silent => Ok(()),
                    BridgeFallback::Strict => Err(BridgeError::Journal(e)),
                    BridgeFallback::FileOnly => {
                        // Try sync fallback
                        let entry_sync = JournalEntry::new(&self.program_id, &self.machine_id, "unknown", "unknown");
                        if let Err(e2) = journal.append_sync(entry_sync) {
                            eprintln!("[memphis-bridge] journal write failed: {} (sync also failed: {})", e, e2);
                        }
                        Ok(())
                    }
                }
            }
        }
    }

    /// Record a decision entry (fire-and-forget, doesn't fail the operation).
    async fn record_decision(
        &self,
        decision_type: &str,
        _target: &str,
        context: serde_json::Value,
        alternatives: &[String],
        reason: Option<&str>,
    ) {
        let decisions = self.decisions.lock().await;

        if let Some(writer) = decisions.as_ref() {
            let entry = DecisionEntry {
                id: uuid::Uuid::new_v4().to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                program_id: self.program_id.clone(),
                context,
                decision: decision_type.to_string(),
                reason: reason.unwrap_or("").to_string(),
                alternatives: alternatives.to_vec(),
                outcome: None,
                machine_id: self.machine_id.clone(),
            };

            if let Err(e) = writer.append(entry).await {
                eprintln!("[memphis-bridge] decision write failed: {}", e);
            }
        }
    }

    /// Record a temperature-based decision for notable readings.
    async fn record_temp_decision(&self, sensor_id: &str, value: f64) {
        let (threshold, label) = if value > 35.0 {
            ("high_temp", "high temperature alert")
        } else if value < 5.0 {
            ("low_temp", "low temperature alert")
        } else {
            return; // Not notable
        };

        self.record_decision(
            threshold,
            sensor_id,
            serde_json::json!({
                "sensor": sensor_id,
                "value_celsius": value,
            }),
            &["ignore".to_string(), "trigger_cooling".to_string(), "trigger_heating".to_string(), "log_only".to_string()],
            Some(label),
        ).await;
    }

    /// Returns the program_id.
    pub fn program_id(&self) -> &str {
        &self.program_id
    }

    /// Returns the machine_id.
    pub fn machine_id(&self) -> &str {
        &self.machine_id
    }

    /// Returns a reference to the underlying journal writer (for sync logging).
    pub fn journal(&self) -> &JournalWriter {
        &self.journal
    }
}

impl std::fmt::Debug for MemphisRuntimeBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemphisRuntimeBridge")
            .field("program_id", &self.program_id)
            .field("machine_id", &self.machine_id)
            .field("fallback", &self.fallback)
            .finish()
    }
}
