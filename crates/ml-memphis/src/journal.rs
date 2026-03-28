//! Memphis journal writer.

use std::path::Path;
use anyhow::Result;

pub struct JournalWriter;

impl JournalWriter {
    pub fn new(_data_dir: &Path) -> Result<Self> {
        Ok(Self)
    }
}
