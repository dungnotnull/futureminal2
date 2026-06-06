//! Futureminal Blockchain — Optional auditability and secrets vault.
//!
//! All features are OFF by default. When enabled:
//! - Commands are hashed and chained into an immutable audit log.
//! - Secrets are encrypted locally with AES-256-GCM.
//! - Metadata is optionally notarized on-chain.

pub mod adapter;
pub mod audit;
pub mod provenance;
pub mod vault;

use tracing::info;

/// Initialize the blockchain subsystem (no-op if feature disabled).
pub fn init() {
    info!("futureminal-blockchain v{} initialized", env!("CARGO_PKG_VERSION"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        init();
    }
}
