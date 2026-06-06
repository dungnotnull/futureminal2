//! Keymap — translates key events into terminal actions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A key event identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyEvent {
    pub key: String,
    pub mods: Vec<Modifier>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    Shift,
    Ctrl,
    Alt,
    Super,
}

/// Actions that can be triggered by key bindings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyAction {
    SendBytes(Vec<u8>),
    SendString(String),
    Copy,
    Paste,
    ClearScreen,
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    SplitHorizontal,
    SplitVertical,
    ToggleFullscreen,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    ToggleSearch,
    ToggleAiPanel,
    TogglePluginPanel(String),
    Quit,
    Nop,
}

/// The keymap maps key events to actions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Keymap {
    pub bindings: HashMap<String, KeyAction>,
}

impl Default for Keymap {
    fn default() -> Self {
        let mut bindings = HashMap::new();
        // Default bindings
        bindings.insert("ctrl+c".into(), KeyAction::SendBytes(vec![0x03]));
        bindings.insert("ctrl+d".into(), KeyAction::SendBytes(vec![0x04]));
        bindings.insert("ctrl+l".into(), KeyAction::ClearScreen);
        bindings.insert("ctrl+shift+c".into(), KeyAction::Copy);
        bindings.insert("ctrl+shift+v".into(), KeyAction::Paste);
        bindings.insert("ctrl+t".into(), KeyAction::NewTab);
        bindings.insert("ctrl+w".into(), KeyAction::CloseTab);
        bindings.insert("ctrl+tab".into(), KeyAction::NextTab);
        bindings.insert("ctrl+shift+tab".into(), KeyAction::PrevTab);
        bindings.insert("ctrl+equal".into(), KeyAction::IncreaseFontSize);
        bindings.insert("ctrl+minus".into(), KeyAction::DecreaseFontSize);
        bindings.insert("ctrl+0".into(), KeyAction::ResetFontSize);
        bindings.insert("ctrl+f".into(), KeyAction::ToggleSearch);
        bindings.insert("ctrl+shift+a".into(), KeyAction::ToggleAiPanel);
        bindings.insert("ctrl+q".into(), KeyAction::Quit);
        Self { bindings }
    }
}

impl Keymap {
    /// Lookup an action by key identifier string (e.g. "ctrl+c").
    pub fn lookup(&self, key: &str) -> Option<&KeyAction> {
        self.bindings.get(key)
    }

    /// Register a new binding.
    pub fn bind(&mut self, key: impl Into<String>, action: KeyAction) {
        self.bindings.insert(key.into(), action);
    }

    /// Remove a binding.
    pub fn unbind(&mut self, key: &str) {
        self.bindings.remove(key);
    }

    /// Convert a KeyAction into bytes to send to the PTY.
    pub fn action_to_bytes(&self, action: &KeyAction) -> Option<Vec<u8>> {
        match action {
            KeyAction::SendBytes(b) => Some(b.clone()),
            KeyAction::SendString(s) => Some(s.as_bytes().to_vec()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_keymap() {
        let km = Keymap::default();
        assert!(km.lookup("ctrl+c").is_some());
        assert_eq!(
            km.lookup("ctrl+c"),
            Some(&KeyAction::SendBytes(vec![0x03]))
        );
    }

    #[test]
    fn test_custom_binding() {
        let mut km = Keymap::default();
        km.bind("f1", KeyAction::ToggleAiPanel);
        assert_eq!(km.lookup("f1"), Some(&KeyAction::ToggleAiPanel));
    }
}
