//! Futureminal Renderer - Stub for compilation.
//!
//! This is a minimal stub. The full wgpu-based renderer needs to be
//! ported from wgpu 0.20 to wgpu 29.x to match the Warp workspace.

use tracing::info;

/// Renderer configuration.
#[derive(Debug, Clone)]
pub struct RendererConfig {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            scale_factor: 1.0,
        }
    }
}

/// Terminal renderer.
pub struct Renderer;

impl Renderer {
    pub async fn new(_config: RendererConfig) -> anyhow::Result<Self> {
        info!("Renderer stub initialized");
        Ok(Self)
    }

    pub fn render(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Initialize the renderer subsystem.
pub fn init() {
    info!("futureminal-renderer stub initialized");
}
