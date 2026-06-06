//! Configuration loading and validation for Futureminal.
//!
//! Configuration is stored in TOML format. The canonical location is:
//! - macOS/Linux: `~/.config/futureminal/config.toml`
//! - Windows: `%APPDATA%\\futureminal\\config.toml`
//!
//! A per-user override may also exist at `.futureminal/config.toml` in the
//! working directory (useful for project-specific settings).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;
use tracing::{info, warn};

/// Errors that can occur when loading or validating configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error reading config file: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Config file not found at any search path")]
    NotFound,
}

/// Root configuration structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub terminal: TerminalConfig,
    pub ui: UiConfig,
    pub ai: AiConfig,
    pub blockchain: BlockchainConfig,
    pub plugins: PluginsConfig,
    pub keybindings: KeybindingsConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            terminal: TerminalConfig::default(),
            ui: UiConfig::default(),
            ai: AiConfig::default(),
            blockchain: BlockchainConfig::default(),
            plugins: PluginsConfig::default(),
            keybindings: KeybindingsConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from the standard search paths.
    ///
    /// Search order:
    /// 1. `$FUTUREMINAL_CONFIG` (explicit path)
    /// 2. `./.futureminal/config.toml`
    /// 3. Platform config directory (`~/.config/futureminal/config.toml`)
    pub fn load() -> Result<Self, ConfigError> {
        if let Ok(explicit) = std::env::var("FUTUREMINAL_CONFIG") {
            let path = PathBuf::from(explicit);
            info!("Loading config from FUTUREMINAL_CONFIG: {}", path.display());
            return Self::load_from_path(&path);
        }

        let local = PathBuf::from(".futureminal/config.toml");
        if local.exists() {
            info!("Loading config from local override: {}", local.display());
            return Self::load_from_path(&local);
        }

        let platform = platform_config_path();
        if platform.exists() {
            info!("Loading config from platform path: {}", platform.display());
            return Self::load_from_path(&platform);
        }

        warn!("No config file found; using defaults");
        Ok(Config::default())
    }

    /// Load and validate configuration from a specific file path.
    pub fn load_from_path(path: &std::path::Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration invariants.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.terminal.scrollback_lines == 0 {
            return Err(ConfigError::Validation(
                "scrollback_lines must be > 0".into()));
        }
        if self.ui.font_size == 0 {
            return Err(ConfigError::Validation(
                "font_size must be > 0".into()));
        }
        if self.ui.window_opacity < 0.0 || self.ui.window_opacity > 1.0 {
            return Err(ConfigError::Validation(
                "window_opacity must be in [0.0, 1.0]".into()));
        }
        Ok(())
    }
}

/// Terminal behavior configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerminalConfig {
    pub scrollback_lines: usize,
    pub default_shell: ShellKind,
    pub cursor_blink: bool,
    pub cursor_style: CursorStyle,
    pub copy_on_select: bool,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            scrollback_lines: 25_000_000,
            default_shell: ShellKind::Auto,
            cursor_blink: false,
            cursor_style: CursorStyle::Block,
            copy_on_select: true,
        }
    }
}

/// Supported shell kinds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ShellKind {
    Auto,
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

/// Cursor visual styles.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CursorStyle {
    Block,
    Line,
    Underline,
}

/// UI appearance configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UiConfig {
    pub theme: String,
    pub font_family: String,
    pub font_size: u8,
    pub line_height: f32,
    pub window_opacity: f32,
    pub padding: UiPadding,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            font_family: "JetBrainsMono Nerd Font".into(),
            font_size: 13,
            line_height: 1.2,
            window_opacity: 0.95,
            padding: UiPadding::default(),
        }
    }
}

/// Window padding in logical pixels.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UiPadding {
    pub x: u16,
    pub y: u16,
}

impl Default for UiPadding {
    fn default() -> Self {
        Self { x: 8, y: 4 }
    }
}

/// AI integration configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AiConfig {
    pub enabled: bool,
    pub default_provider: String,
    pub privacy_mode: PrivacyMode,
    pub providers: AiProviders,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_provider: "auto".into(),
            privacy_mode: PrivacyMode::Strict,
            providers: AiProviders::default(),
        }
    }
}

/// Privacy guardrail levels.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PrivacyMode {
    Strict,
    Balanced,
    Performance,
}

/// Per-provider AI settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AiProviders {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<AnthropicProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai: Option<OpenAiProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local: Option<LocalProviderConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai_compatible: Option<OpenAiCompatibleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnthropicProviderConfig {
    pub api_key_env: String,
    pub model: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAiProviderConfig {
    pub api_key_env: String,
    pub model: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalProviderConfig {
    pub backend: String,
    pub url: String,
    pub model: String,
    pub enabled: bool,
    pub offline_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAiCompatibleConfig {
    pub url: String,
    pub api_key_env: String,
    pub model: String,
    pub enabled: bool,
}

/// Blockchain configuration (off by default).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BlockchainConfig {
    pub enabled: bool,
    pub provider: String,
    pub rpc_url: String,
}

impl Default for BlockchainConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "local".into(),
            rpc_url: "".into(),
        }
    }
}

/// Plugin sources and defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginsConfig {
    pub sources: Vec<String>,
    pub auto_update: bool,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            sources: vec!["https://plugins.futureminal.dev".into()],
            auto_update: false,
        }
    }
}

/// User-defined keybinding overrides.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct KeybindingsConfig {
    // Future: per-action keybinding overrides.
    // For now, the UI ships with sensible defaults.
}

/// Returns the platform-specific configuration directory for Futureminal.
fn platform_config_path() -> PathBuf {
    let config_dir = if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"))
    } else {
        dirs::data_dir().unwrap_or_else(|| PathBuf::from("%APPDATA%"))
    };
    config_dir.join("futureminal").join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let cfg = Config::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let cfg = Config::default();
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(cfg, parsed);
    }

    #[test]
    fn test_invalid_scrollback() {
        let mut cfg = Config::default();
        cfg.terminal.scrollback_lines = 0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_invalid_opacity() {
        let mut cfg = Config::default();
        cfg.ui.window_opacity = 1.5;
        assert!(cfg.validate().is_err());
    }
}
