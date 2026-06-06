//! Futureminal AI — Multi-provider AI abstraction with privacy guardrails.
//!
//! # Architecture
//! ```text
//! User Input
//!     │
//!     ▼
//! futureminal-ai::Router
//!     ├─► LocalProvider (Ollama, LM Studio)
//!     ├─► CloudProvider (Anthropic, OpenAI, Gemini, etc.)
//!     └─► AutoRouter (privacy-aware selection)
//! ```
//!
//! All cloud-bound data passes through `sanitizer::sanitize_for_cloud()`.

pub mod audit;
pub mod provider;
pub mod router;
pub mod sanitizer;

use tracing::info;

/// Initialize the AI subsystem.
pub fn init() {
    info!("futureminal-ai v{} initialized", env!("CARGO_PKG_VERSION"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        init();
    }
}
