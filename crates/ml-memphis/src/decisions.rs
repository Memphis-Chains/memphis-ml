use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DecisionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEntry {
    pub id: String,
    pub timestamp: String,
    pub program_id: String,
    pub context: serde_json::Value,
    pub decision: String,
    pub reason: String,
    pub alternatives: Vec<String>,
    pub outcome: Option<String>,
    pub machine_id: String,
}

pub struct DecisionWriter {
    path: PathBuf,
}

impl DecisionWriter {
    pub fn new(data_dir: impl AsRef<std::path::Path>) -> Result<Self, DecisionError> {
        let path = data_dir.as_ref().join("decisions.chain");
        Ok(DecisionWriter { path })
    }

    pub async fn append(&self, entry: DecisionEntry) -> Result<(), DecisionError> {
        let json = serde_json::to_string(&entry)?;
        let signature = Self::sign_entry(&json);
        let line = format!("{}\n", serde_json::json!({
            "entry": json,
            "signature": signature,
        }));
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        tokio::io::AsyncWriteExt::write_all(&mut file, line.as_bytes()).await?;
        Ok(())
    }

    fn sign_entry(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}
