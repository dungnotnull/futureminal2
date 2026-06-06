//! Script and config provenance tracking.
//!
//! Tracks the origin, modification history, and verification status
//! of scripts and configuration files via hash-based attestation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn};

/// Provenance record for a script or config file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProvenanceRecord {
    pub id: uuid::Uuid,
    pub path: PathBuf,
    pub original_hash: String,
    pub current_hash: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub creator: String,
    pub modifications: Vec<Modification>,
    pub verified: bool,
    pub chain_tx: Option<String>,
}

/// A recorded modification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Modification {
    pub timestamp: DateTime<Utc>,
    pub new_hash: String,
    pub description: Option<String>,
}

/// Provenance tracker for scripts and configs.
pub struct ProvenanceTracker {
    records: Vec<ProvenanceRecord>,
}

impl Default for ProvenanceTracker {
    fn default() -> Self {
        Self { records: Vec::new() }
    }
}

impl ProvenanceTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new script/config with its original hash.
    pub fn register(
        &mut self,
        path: PathBuf,
        creator: impl Into<String>,
    ) -> anyhow::Result<&ProvenanceRecord> {
        let hash = compute_file_hash(&path)?;
        let now = Utc::now();
        let record = ProvenanceRecord {
            id: uuid::Uuid::new_v4(),
            path: path.clone(),
            original_hash: hash.clone(),
            current_hash: hash,
            created_at: now,
            modified_at: now,
            creator: creator.into(),
            modifications: Vec::new(),
            verified: false,
            chain_tx: None,
        };
        self.records.push(record);
        info!("Registered provenance for: {}", path.display());
        Ok(self.records.last().unwrap())
    }

    /// Check if a script has been modified since registration.
    pub fn check(&mut self, path: &std::path::Path) -> anyhow::Result<ProvenanceStatus> {
        let current_hash = compute_file_hash(path)?;

        if let Some(record) = self.records.iter_mut().find(|r| r.path == path) {
            if record.current_hash != current_hash {
                record.modifications.push(Modification {
                    timestamp: Utc::now(),
                    new_hash: current_hash.clone(),
                    description: None,
                });
                record.current_hash = current_hash;
                record.modified_at = Utc::now();
                record.verified = false;
                warn!(
                    "Provenance change detected for {} ({} modifications)",
                    path.display(),
                    record.modifications.len()
                );
                Ok(ProvenanceStatus::Modified {
                    modification_count: record.modifications.len(),
                })
            } else {
                Ok(ProvenanceStatus::Unchanged)
            }
        } else {
            Ok(ProvenanceStatus::NotTracked)
        }
    }

    /// Mark a record as verified on-chain.
    pub fn mark_verified(&mut self,
        path: &std::path::Path,
        tx_hash: impl Into<String>,
    ) {
        if let Some(record) = self.records.iter_mut().find(|r| r.path == path) {
            record.verified = true;
            record.chain_tx = Some(tx_hash.into());
        }
    }

    /// Find a record by path.
    pub fn get(&self, path: &std::path::Path) -> Option<&ProvenanceRecord> {
        self.records.iter().find(|r| r.path == path)
    }
}

/// Status of a provenance check.
#[derive(Debug, Clone, PartialEq)]
pub enum ProvenanceStatus {
    Unchanged,
    Modified { modification_count: usize },
    NotTracked,
}

/// Compute a stable hash of a file's contents.
fn compute_file_hash(path: &std::path::Path) -> anyhow::Result<String> {
    use std::hash::{Hash, Hasher};
    let contents = std::fs::read(path)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    contents.hash(&mut hasher);
    Ok(format!("{:x}", hasher.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_provenance_lifecycle() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "original script").unwrap();

        let mut tracker = ProvenanceTracker::new();
        tracker.register(file.path().to_path_buf(), "alice@example.com").unwrap();

        let status = tracker.check(file.path()).unwrap();
        assert_eq!(status, ProvenanceStatus::Unchanged);

        // Modify the file
        write!(file, "modified script").unwrap();
        let status = tracker.check(file.path()).unwrap();
        assert!(matches!(status, ProvenanceStatus::Modified { .. }));
    }
}
