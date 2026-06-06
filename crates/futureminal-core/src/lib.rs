//! Futureminal Core — PTY management, shell integration, session persistence,
//! terminal grid, VT parsing, configuration, and input handling.

#![forbid(unsafe_code)]

pub mod config;
pub mod grid;
pub mod keymap;
pub mod pty;
pub mod session;
pub mod shell;
pub mod theme;
pub mod vt;
pub mod windowing;

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Shared application state used across the core daemon.
#[derive(Debug, Clone)]
pub struct CoreState {
    pub config: Arc<RwLock<config::Config>>,
    pub session: Arc<RwLock<session::SessionManager>>,
    pub theme: Arc<RwLock<theme::ThemeEngine>>,
    pub keymap: Arc<RwLock<keymap::Keymap>>,
}

impl CoreState {
    /// Create a new `CoreState` from a validated configuration.
    pub fn new(config: config::Config) -> Self {
        let theme = theme::ThemeEngine::load(&config.ui.theme).unwrap_or_default();
        let keymap = keymap::Keymap::default();
        Self {
            config: Arc::new(RwLock::new(config)),
            session: Arc::new(RwLock::new(session::SessionManager::new())),
            theme: Arc::new(RwLock::new(theme)),
            keymap: Arc::new(RwLock::new(keymap)),
        }
    }

    /// Reload configuration from disk and apply changes.
    pub async fn reload_config(&self) -> anyhow::Result<()> {
        let new_config = config::Config::load()?;
        new_config.validate()?;
        let mut cfg = self.config.write().await;
        *cfg = new_config;
        info!("Configuration reloaded");
        Ok(())
    }
}

/// Initialize the core daemon subsystems.
pub fn init() {
    info!("futureminal-core v{} initialized", env!("CARGO_PKG_VERSION"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_does_not_panic() {
        init();
    }

    #[tokio::test]
    async fn test_core_state_creation() {
        let cfg = config::Config::default();
        let state = CoreState::new(cfg);
        let config = state.config.read().await;
        assert_eq!(config.terminal.scrollback_lines, 25_000_000);
    }
}
