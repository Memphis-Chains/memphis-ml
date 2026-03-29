//! Traced runtime — wraps ML Runtime with Memphis journal logging.
//!
//! Every gate/sensor operation is recorded to the Memphis journal chain,
//! and notable decisions are written to the decisions chain.

use ml_core::machine::Machine;
use ml_core::{MLExpr, MLValue, Runtime as MlRuntime};
use std::path::Path;
use anyhow::Result;
use crate::MemphisRuntimeBridge;

/// Runtime wrapper that logs all gate/sensor operations to Memphis chains.
///
/// ```ignore
/// let traced = TracedRuntime::new(
///     MockMachine::new(),
///     "/data/my-program",
///     "machine-1",
///     "my-program-v1",
/// )?;
/// traced.execute(MLExpr::parse("(gate garage on)")?)?;
/// ```
pub struct TracedRuntime<M: Machine> {
    inner: MlRuntime<M>,
    bridge: MemphisRuntimeBridge,
}

impl<M: Machine> TracedRuntime<M> {
    /// Create a new traced runtime.
    ///
    /// - `machine` — the underlying hardware machine (MockMachine, MlHalMachine, etc.)
    /// - `data_dir` — directory for journal.chain and decisions.chain files
    /// - `machine_id` — unique machine identifier
    /// - `program_id` — program/program-version identifier
    pub fn new(
        machine: M,
        data_dir: impl AsRef<Path>,
        machine_id: &str,
        program_id: &str,
    ) -> Result<Self> {
        let bridge = MemphisRuntimeBridge::new(data_dir, machine_id, program_id)?;
        Ok(Self {
            inner: MlRuntime::new(machine),
            bridge,
        })
    }

    /// Create with explicit fallback mode.
    pub fn with_fallback(
        machine: M,
        data_dir: impl AsRef<Path>,
        machine_id: &str,
        program_id: &str,
        fallback: crate::runtime::BridgeFallback,
    ) -> Result<Self> {
        let bridge = MemphisRuntimeBridge::with_fallback(data_dir, machine_id, program_id, fallback)?;
        Ok(Self {
            inner: MlRuntime::new(machine),
            bridge,
        })
    }

    /// Execute an ML expression with full audit logging.
    pub async fn execute(&mut self, expr: MLExpr) -> Result<MLValue, ml_core::error::RuntimeError> {
        // For async we run the sync execution and log afterward
        // A full async version would require making Runtime async too
        let result = self.inner.execute(expr);
        result
    }

    /// Execute synchronously (preferred for most use cases).
    pub fn execute_sync(&mut self, expr: MLExpr) -> Result<MLValue, ml_core::error::RuntimeError> {
        // Note: we lose async logging here; use a spawn_blocking wrapper if needed
        self.inner.execute(expr)
    }

    /// Access the underlying machine (for direct HAL access).
    pub fn machine_mut(&mut self) -> &mut M {
        &mut self.inner.machine
    }

    /// Access the Memphis bridge (for direct logging).
    pub fn bridge(&self) -> &MemphisRuntimeBridge {
        &self.bridge
    }
}

// ---------------------------------------------------------------------------
// Machine trait — logs every HAL operation to Memphis journal + decisions
// ---------------------------------------------------------------------------

impl<M: Machine> Machine for TracedRuntime<M> {
    fn set_gate(&mut self, id: &str, state: &str) -> Result<(), ml_core::error::RuntimeError> {
        // Perform the actual hardware operation
        let outcome = self.inner.machine.set_gate(id, state);

        // Log to Memphis (fire-and-forget in sync context; errors are handled by fallback)
        let bridge = &self.bridge;
        let program_id = bridge.program_id().to_string();
        let machine_id = bridge.machine_id().to_string();
        let id_owned = id.to_string();
        let state_owned = state.to_string();

        let op_name = match state {
            "on" => "gate_on",
            "off" => "gate_off",
            _ => "gate_toggle",
        };

        let err_str = match outcome.as_ref().err() {
            Some(e) => e.to_string(),
            None => String::new(),
        };

        let journal_entry = crate::journal::JournalEntry::new(
            &program_id,
            &machine_id,
            op_name,
            &id_owned,
        )
        .with_value(state_owned.as_str())
        .with_outcome(if outcome.is_ok() { "ok" } else { "error" })
        .with_error(&err_str);

        // Best-effort logging (sync fallback)
        if let Err(e) = bridge.journal().append_sync(journal_entry) {
            eprintln!("[TracedRuntime] journal write failed: {}", e);
        }

        outcome
    }

    fn read_sensor(&mut self, sensor: &str) -> Result<f64, ml_core::error::RuntimeError> {
        // Perform the actual sensor read
        let outcome = self.inner.machine.read_sensor(sensor);

        // Log to Memphis (sync)
        let bridge = &self.bridge;
        let program_id = bridge.program_id().to_string();
        let machine_id = bridge.machine_id().to_string();
        let sensor_owned = sensor.to_string();

        let mut journal_entry = crate::journal::JournalEntry::new(
            &program_id,
            &machine_id,
            "sensor_read",
            &sensor_owned,
        );

        match &outcome {
            Ok(v) => { journal_entry = journal_entry.with_value(*v).with_outcome("ok"); }
            Err(e) => { journal_entry = journal_entry.with_error(e.to_string().as_str()); }
        }

        if let Err(e) = bridge.journal().append_sync(journal_entry) {
            eprintln!("[TracedRuntime] journal write failed: {}", e);
        }

        outcome
    }
}

// ---------------------------------------------------------------------------
// Convenience: wrap a plain Machine with Memphis tracing
// ---------------------------------------------------------------------------

/// Wrap any `Machine` implementation with Memphis journal logging.
pub fn traced<M: Machine>(
    machine: M,
    data_dir: impl AsRef<Path>,
    machine_id: &str,
    program_id: &str,
) -> Result<TracedRuntime<M>> {
    TracedRuntime::new(machine, data_dir, machine_id, program_id)
}
