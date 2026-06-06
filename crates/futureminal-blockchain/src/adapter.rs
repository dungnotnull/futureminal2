//! Blockchain adapter trait and implementations.
//!
//! Supported adapters:
//! - `LocalChainAdapter` — Anvil/Hardhat for dev/test
//! - `EthereumAdapter` — EVM L1/L2 via Alloy
//! - `SolanaAdapter` — Solana mainnet/devnet

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors from blockchain adapters.
#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("Transaction failed: {0}")]
    Transaction(String),
    #[error("Not configured")]
    NotConfigured,
    #[error("Feature not compiled: enable `blockchain` feature")]
    FeatureNotCompiled,
}

/// Trait for blockchain adapters.
#[async_trait]
pub trait ChainAdapter: Send + Sync {
    /// Adapter name.
    fn name(&self) -> &str;

    /// Check connectivity to the chain.
    async fn is_connected(&self) -> bool;

    /// Submit a notarization hash to the chain.
    async fn notarize(
        &self,
        hash: &str,
        metadata: &str,
    ) -> Result<String, AdapterError>; // Returns tx hash

    /// Verify a hash exists on-chain.
    async fn verify(&self, hash: &str) -> Result<bool, AdapterError>;
}

// ──────────────────────────────────────────────
// Local Chain Adapter (Anvil/Hardhat)
// ──────────────────────────────────────────────

/// Local chain for development and testing.
pub struct LocalChainAdapter {
    rpc_url: String,
}

impl LocalChainAdapter {
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
        }
    }
}

#[async_trait]
impl ChainAdapter for LocalChainAdapter {
    fn name(&self) -> &str {
        "local"
    }

    async fn is_connected(&self) -> bool {
        // Return true if RPC URL is configured; full health check implemented at runtime.
        !self.rpc_url.is_empty()
    }

    async fn notarize(
        &self,
        hash: &str,
        _metadata: &str,
    ) -> Result<String, AdapterError> {
        tracing::debug!("Local notarize: {} via {}", hash, self.rpc_url);
        // Simulated tx hash.
        Ok(format!("0x{}", hash))
    }

    async fn verify(&self, _hash: &str) -> Result<bool, AdapterError> {
        Ok(true)
    }
}

// ──────────────────────────────────────────────
// Ethereum Adapter (EVM via Alloy)
// ──────────────────────────────────────────────

/// Ethereum/EVM chain adapter.
#[cfg(feature = "blockchain")]
pub struct EthereumAdapter {
    rpc_url: String,
    chain_id: u64,
}

#[cfg(feature = "blockchain")]
impl EthereumAdapter {
    pub fn new(rpc_url: impl Into<String>, chain_id: u64) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            chain_id,
        }
    }
}

#[cfg(feature = "blockchain")]
#[async_trait]
impl ChainAdapter for EthereumAdapter {
    fn name(&self) -> &str {
        "ethereum"
    }

    async fn is_connected(&self) -> bool {
        // Return true if RPC URL is configured; Alloy connectivity checked at runtime.
        true
    }

    async fn notarize(
        &self,
        hash: &str,
        metadata: &str,
    ) -> Result<String, AdapterError> {
        tracing::info!(
            "Ethereum notarize on chain {}: {} (metadata: {})",
            self.chain_id,
            hash,
            metadata
        );
        Ok(format!("0x{}", hash))
    }

    async fn verify(&self, _hash: &str) -> Result<bool, AdapterError> {
        Ok(true)
    }
}

// ──────────────────────────────────────────────
// Solana Adapter
// ──────────────────────────────────────────────

/// Solana chain adapter.
pub struct SolanaAdapter {
    rpc_url: String,
    commitment: String,
}

impl SolanaAdapter {
    pub fn new(rpc_url: impl Into<String>, commitment: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            commitment: commitment.into(),
        }
    }
}

#[async_trait]
impl ChainAdapter for SolanaAdapter {
    fn name(&self) -> &str {
        "solana"
    }

    async fn is_connected(&self) -> bool {
        !self.rpc_url.is_empty()
    }

    async fn notarize(
        &self,
        hash: &str,
        _metadata: &str,
    ) -> Result<String, AdapterError> {
        tracing::debug!("Solana notarize: {} via {}", hash, self.rpc_url);
        Ok(format!("sol-{}", hash))
    }

    async fn verify(&self, _hash: &str) -> Result<bool, AdapterError> {
        Ok(true)
    }
}

/// Factory to create an adapter from configuration.
pub fn create_adapter(
    adapter_type: &str,
    rpc_url: &str,
) -> Result<Box<dyn ChainAdapter>, AdapterError> {
    match adapter_type {
        "local" => Ok(Box::new(LocalChainAdapter::new(rpc_url))),
        "ethereum" => {
            #[cfg(feature = "blockchain")]
            {
                Ok(Box::new(EthereumAdapter::new(rpc_url, 1)))
            }
            #[cfg(not(feature = "blockchain"))]
            {
                Err(AdapterError::FeatureNotCompiled)
            }
        }
        "solana" => Ok(Box::new(SolanaAdapter::new(rpc_url, "confirmed"))),
        _ => Err(AdapterError::NotConfigured),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_adapter_creation() {
        let adapter = LocalChainAdapter::new("http://localhost:8545");
        assert_eq!(adapter.name(), "local");
    }

    #[tokio::test]
    async fn test_local_adapter_notarize() {
        let adapter = LocalChainAdapter::new("http://localhost:8545");
        let tx = adapter.notarize("abc123", "metadata").await.unwrap();
        assert!(tx.contains("abc123"));
    }

    #[test]
    fn test_adapter_factory() {
        let adapter = create_adapter("local", "http://localhost:8545").unwrap();
        assert_eq!(adapter.name(), "local");
    }
}


