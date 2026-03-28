//! Memphis runtime bridge — logs ML ops to Memphis journal/chains.

use std::path::Path;
use anyhow::Result;

pub struct MemphisRuntimeBridge;

impl MemphisRuntimeBridge {
    pub fn new(_data_dir: impl AsRef<Path>, _machine_id: &str, _program_id: &str) -> Result<Self> {
        Ok(Self)
    }
}
