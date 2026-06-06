//! Session persistence for Futureminal.
//!
//! Sessions (tabs, panes, working directories, command history) can be saved
//! to disk and restored across application restarts. Data is stored in JSON
//! for portability and human inspectability.

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tracing::{debug, error, info, warn};

/// Errors that can occur during session operations.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Session not found: {0}")]
    NotFound(String),
}

/// A persisted terminal session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_accessed: chrono::DateTime<chrono::Utc>,
    pub tabs: Vec<Tab>,
    pub active_tab_id: Option<String>,
}

/// A tab within a session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tab {
    pub id: String,
    pub name: String,
    pub panes: Vec<Pane>,
    pub active_pane_id: Option<String>,
}

/// A pane (split) within a tab.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pane {
    pub id: String,
    pub shell: String,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub dimensions: PaneDimensions,
}

/// Layout dimensions for a pane.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PaneDimensions {
    pub cols: u16,
    pub rows: u16,
}

/// Manages the lifecycle of saved sessions.
#[derive(Debug, Default)]
pub struct SessionManager {
    sessions_dir: PathBuf,
    sessions: HashMap<String, Session>,
}

impl SessionManager {
    /// Create a new session manager with the default persistence directory.
    pub fn new() -> Self {
        let sessions_dir = Self::default_sessions_dir();
        let mut manager = Self {
            sessions_dir,
            sessions: HashMap::new(),
        };
        if let Err(e) = manager.load_all() {
            warn!("Failed to load existing sessions: {}", e);
        }
        manager
    }

    /// Create a new session manager with an explicit directory.
    pub fn with_dir(dir: PathBuf) -> Self {
        Self {
            sessions_dir: dir,
            sessions: HashMap::new(),
        }
    }

    /// Returns the default sessions persistence directory.
    fn default_sessions_dir() -> PathBuf {
        let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("futureminal").join("sessions")
    }

    /// Create a new empty session.
    pub fn create(&mut self, name: impl Into<String>) -> Session {
        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            tabs: vec![self.default_tab()],
            active_tab_id: None,
        };
        self.sessions.insert(session.id.clone(), session.clone());
        session
    }

    /// Save a session to disk.
    pub fn save(&self, session: &Session) -> Result<(), SessionError> {
        std::fs::create_dir_all(&self.sessions_dir)?;
        let path = self.sessions_dir.join(format!("{}.json", session.id));
        let json = serde_json::to_string_pretty(session)?;
        std::fs::write(&path, json)?;
        info!("Saved session {} to {}", session.id, path.display());
        Ok(())
    }

    /// Load a session from disk by ID.
    pub fn load(&mut self, id: &str) -> Result<Session, SessionError> {
        if let Some(session) = self.sessions.get(id) {
            return Ok(session.clone());
        }
        let path = self.sessions_dir.join(format!("{}.json", id));
        if !path.exists() {
            return Err(SessionError::NotFound(id.into()));
        }
        let contents = std::fs::read_to_string(&path)?;
        let session: Session = serde_json::from_str(&contents)?;
        self.sessions.insert(id.into(), session.clone());
        Ok(session)
    }

    /// Load all sessions from the persistence directory.
    fn load_all(&mut self) -> Result<(), SessionError> {
        if !self.sessions_dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(contents) => {
                    if let Ok(session) = serde_json::from_str::<Session>(&contents) {
                        self.sessions.insert(session.id.clone(), session);
                    } else {
                        warn!("Failed to parse session file: {}", path.display());
                    }
                }
                Err(e) => {
                    error!("Failed to read session file {}: {}", path.display(), e);
                }
            }
        }
        Ok(())
    }

    /// Delete a session from memory and disk.
    pub fn delete(&mut self, id: &str) -> Result<(), SessionError> {
        self.sessions.remove(id);
        let path = self.sessions_dir.join(format!("{}.json", id));
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// List all known session names and IDs.
    pub fn list(&self) -> Vec<(&str, &str)> {
        self.sessions
            .values()
            .map(|s| (s.id.as_str(), s.name.as_str()))
            .collect()
    }

    /// Returns the number of tracked sessions.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Returns true if no sessions are tracked.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    fn default_tab(&self) -> Tab {
        Tab {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Tab 1".into(),
            panes: vec![Pane {
                id: uuid::Uuid::new_v4().to_string(),
                shell: default_shell_path(),
                cwd: std::env::current_dir().ok(),
                env: std::collections::HashMap::new(),
                dimensions: PaneDimensions { cols: 80, rows: 24 },
            }],
            active_pane_id: None,
        }
    }
}

/// Returns the default shell for the current platform.
fn default_shell_path() -> String {
    if cfg!(target_os = "windows") {
        std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".into())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_session_crud() {
        let tmp = TempDir::new().unwrap();
        let mut mgr = SessionManager::with_dir(tmp.path().to_path_buf());

        let session = mgr.create("test-session");
        assert_eq!(session.name, "test-session");
        assert_eq!(mgr.len(), 1);

        mgr.save(&session).unwrap();
        assert!(tmp.path().join(format!("{}.json", session.id)).exists());

        let loaded = mgr.load(&session.id).unwrap();
        assert_eq!(loaded.name, "test-session");

        mgr.delete(&session.id).unwrap();
        assert_eq!(mgr.len(), 0);
    }

    #[test]
    fn test_session_load_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let mut mgr = SessionManager::with_dir(tmp.path().to_path_buf());
        assert!(mgr.load("nonexistent-id").is_err());
    }
}

