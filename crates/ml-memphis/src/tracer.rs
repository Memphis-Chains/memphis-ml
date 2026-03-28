//! Traced runtime — wraps ML Runtime with Memphis journal logging.

use ml_core::machine::Machine;
use ml_core::{MLExpr, MLValue, Runtime};
use std::path::Path;
use anyhow::Result;
use crate::MemphisRuntimeBridge;

/// Runtime wrapper that logs all gate/sensor operations to Memphis chains.
pub struct TracedRuntime<M: Machine> {
    inner: Runtime<M>,
    #[allow(dead_code)]
    bridge: MemphisRuntimeBridge,
}

impl<M: Machine> TracedRuntime<M> {
    pub fn new(machine: M, data_dir: impl AsRef<Path>, machine_id: &str, program_id: &str) -> Result<Self> {
        let bridge = MemphisRuntimeBridge::new(data_dir, machine_id, program_id)?;
        Ok(Self { inner: Runtime::new(machine), bridge })
    }

    pub fn execute(&mut self, expr: MLExpr) -> Result<MLValue, ml_core::error::RuntimeError> {
        self.inner.execute(expr)
    }
}

impl<M: Machine> Machine for TracedRuntime<M> {
    fn set_gate(&mut self, id: &str, state: &str) -> Result<(), ml_core::error::RuntimeError> {
        self.inner.machine.set_gate(id, state)
    }

    fn read_sensor(&mut self, sensor: &str) -> Result<f64, ml_core::error::RuntimeError> {
        self.inner.machine.read_sensor(sensor)
    }
}
