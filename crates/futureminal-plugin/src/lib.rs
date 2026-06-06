//! Futureminal Plugin - Stub for compilation.
//!
//! This is a minimal stub. Full WASM + Lua plugin host needs mlua
//! and wasmtime integration which requires system Lua libraries.

use tracing::info;

/// Plugin manifest.
#[derive(Debug, Clone)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
}

/// Plugin host.
pub struct PluginHost;

impl PluginHost {
    pub fn new() -> anyhow::Result<Self> {
        info!("Plugin host stub initialized");
        Ok(Self)
    }

    pub fn load(&mut self, _manifest: PluginManifest) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn unload(&mut self, _id: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Initialize the plugin subsystem.
pub fn init() {
    info!("futureminal-plugin stub initialized");
}
