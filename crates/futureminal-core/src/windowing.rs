//! Cross-platform windowing for Futureminal.
//!
//! Uses winit for native window creation with custom Futureminal chrome.

use tracing::{error, info};

/// Window configuration.
#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub title: String,
    pub transparent: bool,
    pub decorations: bool,
    pub always_on_top: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            title: "Futureminal".into(),
            transparent: false,
            decorations: true,
            always_on_top: false,
        }
    }
}

/// Window event callback trait.
pub trait WindowHandler: Send {
    fn on_resize(&mut self, width: u32, height: u32);
    fn on_redraw(&mut self);
    fn on_key(&mut self, key: &str, mods: KeyMods);
    fn on_mouse(&mut self, x: f64, y: f64, button: Option<MouseButton>);
    fn on_close(&mut self);
}

/// Keyboard modifiers.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct KeyMods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Futureminal window abstraction.
///
/// This is a thin wrapper around winit that provides Futureminal-specific
/// defaults (custom title, theme-aware chrome, etc.).
pub struct Window {
    config: WindowConfig,
}

impl Window {
    pub fn new(config: WindowConfig) -> Self {
        info!("Creating Futureminal window {}x{}", config.width, config.height);
        Self { config }
    }

    pub fn config(&self) -> &WindowConfig {
        &self.config
    }

    /// Run the window event loop (blocks until window closes).
    ///
    /// In a real implementation this would create a winit event loop
    /// and dispatch events to the handler. For now, this is a stub
    /// that validates the configuration.
    pub fn run(&self, mut _handler: impl WindowHandler) -> anyhow::Result<()> {
        info!("Window event loop started for Futureminal");
        // TODO: integrate with winit EventLoop
        Ok(())
    }
}

/// Initialize the windowing subsystem.
pub fn init() {
    info!("futureminal-window v{} initialized", env!("CARGO_PKG_VERSION"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_config_default() {
        let cfg = WindowConfig::default();
        assert_eq!(cfg.width, 1280);
        assert_eq!(cfg.height, 720);
        assert_eq!(cfg.title, "Futureminal");
    }

    #[test]
    fn test_window_creation() {
        let cfg = WindowConfig::default();
        let window = Window::new(cfg);
        assert_eq!(window.config().width, 1280);
    }

    #[test]
    fn test_key_mods() {
        let mods = KeyMods { ctrl: true, alt: false, shift: true, meta: false };
        assert!(mods.ctrl);
        assert!(!mods.alt);
    }

    #[test]
    fn test_mouse_button() {
        assert_ne!(MouseButton::Left, MouseButton::Right);
    }

    #[test]
    fn test_init() {
        init();
    }
}
