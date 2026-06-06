//! Local audit log for AI interactions.
//!
//! Every AI interaction is logged locally as a hashed, non-reversible record.
//! Content is NEVER stored � only metadata (timestamps, provider, token usage).

use std::io::Write;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tracing::{error, info, warn};

/// A single audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditEntry {
    pub id: uuid::Uuid,
    pub timestamp: DateTime<Utc>,
    pub provider: String,
    pub model: String,
    /// SHA-256 hash of the prompt (not the content itself).
    pub prompt_hash: String,
    pub tokens_used: Option<u32>,
    pub response_time_ms: u64,
    pub action_taken: String,
    pub user_accepted: bool,
    pub privacy_mode: String,
    pub sanitized: bool,
}

/// The local AI audit logger.
pub struct AiAuditLog {
    log_path: PathBuf,
    entries: Vec<AuditEntry>,
}

impl AiAuditLog {
    /// Create a new audit logger with the default log path.
    pub fn new() -> Self {
        let log_path = Self::default_log_path();
        let mut logger = Self {
            log_path,
            entries: Vec::new(),
        };
        if let Err(e) = logger.load() {
            warn!("Failed to load existing audit log: {}", e);
        }
        logger
    }

    /// Create a logger with an explicit path.
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            log_path: path,
            entries: Vec::new(),
        }
    }

    fn default_log_path() -> PathBuf {
        let data = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        data.join("futureminal").join("ai-audit.jsonl")
    }

    /// Append an entry to the audit log.
    pub fn log(&mut self,
        entry: AuditEntry,
    ) -> anyhow::Result<()> {
        self.entries.push(entry.clone());

        std::fs::create_dir_all(self.log_path.parent().unwrap_or(Path::new(".")))?;
        let line = serde_json::to_string(&entry)?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?
            .write_all(format!("{}\n", line).as_bytes())?;

        info!("AI audit entry {} logged", entry.id);
        Ok(())
    }

    /// Load existing entries from disk.
    fn load(&mut self) -> anyhow::Result<()> {
        if !self.log_path.exists() {
            return Ok(());
        }
        let contents = std::fs::read_to_string(&self.log_path)?;
        for line in contents.lines() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<AuditEntry>(line) {
                Ok(entry) => self.entries.push(entry),
                Err(e) => warn!("Failed to parse audit log line: {}", e),
            }
        }
        Ok(())
    }

    /// Compute SHA-256 hash of prompt text.
    pub fn hash_prompt(prompt: &str) -> String {
        use std::hash::{Hash, Hasher};
        // In production, use ring::digest::digest(ring::digest::SHA256, ...)
        // Compute SHA-256 hash of the prompt.
                use ring::digest::{Context, SHA256};
        let mut ctx = Context::new(&SHA256);
        ctx.update(prompt.as_bytes());
        let digest = ctx.finish();
        format!("sha256:{}", hex::encode(digest.as_ref()))
    }

    /// Returns all entries.
    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    /// Returns statistics grouped by provider.
    pub fn stats_by_provider(&self) -> HashMap<String, ProviderStats> {
        let mut stats: HashMap<String, ProviderStats> = HashMap::new();
        for entry in &self.entries {
            let s = stats.entry(entry.provider.clone()).or_default();
            s.request_count += 1;
            s.total_tokens += entry.tokens_used.unwrap_or(0) as u64;
            s.total_response_time_ms += entry.response_time_ms;
            if entry.user_accepted {
                s.accepted_count += 1;
            }
        }
        stats
    }
}

/// Aggregated statistics for a provider.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProviderStats {
    pub request_count: u64,
    pub total_tokens: u64,
    pub total_response_time_ms: u64,
    pub accepted_count: u64,
}

impl ProviderStats {
    pub fn avg_response_time_ms(&self) -> u64 {
        if self.request_count == 0 {
            0
        } else {
            self.total_response_time_ms / self.request_count
        }
    }

    pub fn acceptance_rate(&self) -> f64 {
        if self.request_count == 0 {
            0.0
        } else {
            self.accepted_count as f64 / self.request_count as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_entry_serde() {
        let entry = AuditEntry {
            id: uuid::Uuid::new_v4(),
            timestamp: Utc::now(),
            provider: "anthropic".into(),
            model: "claude-3".into(),
            prompt_hash: "sha256:abc".into(),
            tokens_used: Some(123),
            response_time_ms: 890,
            action_taken: "command_generated".into(),
            user_accepted: true,
            privacy_mode: "strict".into(),
            sanitized: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: AuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, parsed);
    }

    #[test]
    fn test_provider_stats() {
        let mut stats = ProviderStats::default();
        stats.request_count = 10;
        stats.accepted_count = 8;
        assert_eq!(stats.acceptance_rate(), 0.8);
        assert_eq!(stats.avg_response_time_ms(), 0);
    }
}



