//! Theme engine for Futureminal.
//!
//! Loads TOML theme files and resolves colors for rendering.

use crate::grid::{Color, NamedColor};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

/// A loaded color theme.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Theme {
    pub name: String,
    pub author: String,
    pub version: String,
    pub colors: ThemeColors,
    pub ui: UiColors,
}

/// Terminal palette colors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeColors {
    pub background: ColorDef,
    pub foreground: ColorDef,
    pub cursor: ColorDef,
    pub selection: ColorDef,
    pub normal: HashMap<String, ColorDef>,
    pub bright: HashMap<String, ColorDef>,
}

/// UI chrome colors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct UiColors {
    pub panel_bg: Option<ColorDef>,
    pub panel_fg: Option<ColorDef>,
    pub tab_active_bg: Option<ColorDef>,
    pub tab_active_fg: Option<ColorDef>,
    pub tab_inactive_bg: Option<ColorDef>,
    pub tab_inactive_fg: Option<ColorDef>,
    pub border: Option<ColorDef>,
}

/// A color definition in a theme file (hex string).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ColorDef {
    Hex(String),
    Rgb { r: u8, g: u8, b: u8 },
}

impl ColorDef {
    /// Parse a hex color string (#RRGGBB or #RGB) into RGB components.
    pub fn to_rgb(&self) -> (u8, u8, u8) {
        match self {
            ColorDef::Hex(s) => parse_hex_color(s).unwrap_or((0, 0, 0)),
            ColorDef::Rgb { r, g, b } => (*r, *g, *b),
        }
    }

    /// Convert to a grid Color.
    pub fn to_grid_color(&self) -> Color {
        let (r, g, b) = self.to_rgb();
        Color::TrueColor { r, g, b }
    }
}

/// Theme engine manages loading and caching themes.
#[derive(Debug, Default)]
pub struct ThemeEngine {
    themes: HashMap<String, Theme>,
    active: String,
    theme_dir: PathBuf,
}

impl ThemeEngine {
    pub fn new() -> Self {
        let theme_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("futureminal")
            .join("themes");
        Self {
            themes: HashMap::new(),
            active: "dark".into(),
            theme_dir,
        }
    }

    /// Load a theme by name from the theme directory.
    pub fn load(name: &str) -> anyhow::Result<Self> {
        let mut engine = Self::new();
        engine.active = name.into();

        // Load built-in themes
        let builtin = PathBuf::from("themes");
        if builtin.exists() {
            engine.load_from_dir(&builtin)?;
        }

        // Load user themes
        if engine.theme_dir.exists() {
            engine.load_from_dir(&engine.theme_dir.clone())?;
        }

        Ok(engine)
    }

    fn load_from_dir(&mut self, dir: &Path) -> anyhow::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(contents) => {
                    match toml::from_str::<Theme>(&contents) {
                        Ok(theme) => {
                            info!("Loaded theme: {} from {}", theme.name, path.display());
                            self.themes.insert(theme.name.clone(), theme);
                        }
                        Err(e) => {
                            warn!("Failed to parse theme {}: {}", path.display(), e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read theme {}: {}", path.display(), e);
                }
            }
        }
        Ok(())
    }

    /// Get the active theme.
    pub fn current(&self) -> Option<&Theme> {
        self.themes.get(&self.active)
    }

    /// Resolve a named color to its actual RGB value via the active theme.
    pub fn resolve(&self, color: &Color) -> Color {
        let theme = match self.current() {
            Some(t) => t,
            None => return *color,
        };
        match color {
            Color::Named(NamedColor::Foreground) => theme.colors.foreground.to_grid_color(),
            Color::Named(NamedColor::Background) => theme.colors.background.to_grid_color(),
            Color::Named(named) => {
                let name = format!("{:?}", named).to_lowercase();
                if let Some(def) = theme.colors.normal.get(&name) {
                    def.to_grid_color()
                } else if let Some(def) = theme.colors.bright.get(&name) {
                    def.to_grid_color()
                } else {
                    *color
                }
            }
            _ => *color,
        }
    }

    /// Switch to a different theme.
    pub fn set_active(&mut self, name: &str) {
        if self.themes.contains_key(name) {
            self.active = name.into();
            info!("Switched to theme: {}", name);
        } else {
            warn!("Theme not found: {}", name);
        }
    }

    /// List available theme names.
    pub fn list(&self) -> Vec<&str> {
        self.themes.keys().map(|s| s.as_str()).collect()
    }
}

fn parse_hex_color(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim_start_matches('#');
    match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            Some((r, g, b))
        }
        3 => {
            let r = u8::from_str_radix(&s[0..1], 16).ok()?;
            let g = u8::from_str_radix(&s[1..2], 16).ok()?;
            let b = u8::from_str_radix(&s[2..3], 16).ok()?;
            Some((r * 17, g * 17, b * 17))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_parsing() {
        assert_eq!(parse_hex_color("#FF5733"), Some((255, 87, 51)));
        assert_eq!(parse_hex_color("#F73"), Some((255, 119, 51)));
    }

    #[test]
    fn test_colordef_to_rgb() {
        let c = ColorDef::Hex("#00FF00".into());
        assert_eq!(c.to_rgb(), (0, 255, 0));
    }
}
