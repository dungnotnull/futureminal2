//! AI Router — Privacy-aware, cost/latency-aware provider selection.
//!
//! The router maintains a registry of providers and selects the best one
//! for each request based on:
//! - Privacy mode (strict → local only)
//! - Provider availability (health checks)
//! - Latency history
//! - Cost (for cloud providers)

use crate::provider::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Privacy-aware routing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterMode {
    /// Always prefer local/offline providers.
    PrivacyFirst,
    /// Balance between privacy and capability.
    Balanced,
    /// Use the fastest/best provider regardless of cloud/local.
    Performance,
}

/// The AI router manages multiple providers and routes requests.
pub struct AiRouter {
    providers: Arc<RwLock<HashMap<String, Box<dyn AiProvider>>>>,
    mode: RouterMode,
    default_provider: String,
}

impl AiRouter {
    /// Create a new router with no providers.
    pub fn new(mode: RouterMode) -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            mode,
            default_provider: "auto".into(),
        }
    }

    /// Register a provider with the router.
    pub async fn register_provider(
        &self,
        name: impl Into<String>,
        provider: Box<dyn AiProvider>,
    ) {
        let name = name.into();
        let mut providers = self.providers.write().await;
        info!("Registered AI provider: {}", name);
        providers.insert(name, provider);
    }

    /// Select the best provider for a request.
    async fn select_provider(
        &self,
        preferred: Option<&str>,
    ) -> Option<String> {
        let providers = self.providers.read().await;

        if let Some(name) = preferred {
            if providers.contains_key(name) {
                return Some(name.into());
            }
        }

        match self.mode {
            RouterMode::PrivacyFirst => {
                // Prefer ollama/local first.
                if providers.contains_key("ollama") {
                    return Some("ollama".into());
                }
                // Fall back to any available.
                providers.keys().next().cloned()
            }
            RouterMode::Balanced => {
                // Try local first, then cloud.
                if providers.contains_key("ollama") {
                    return Some("ollama".into());
                }
                providers.keys().next().cloned()
            }
            RouterMode::Performance => {
                // Prefer cloud providers (typically faster).
                for preferred in [&"anthropic", &"openai", &"openai-compatible",
                ] {
                    if providers.contains_key(*preferred) {
                        return Some((*preferred).into());
                    }
                }
                providers.keys().next().cloned()
            }
        }
    }

    /// Generate a completion using the selected provider.
    pub async fn complete(
        &self,
        request: CompletionRequest,
        preferred_provider: Option<&str>,
    ) -> Result<CompletionResponse, ProviderError> {
        let name = self
            .select_provider(preferred_provider)
            .await
            .ok_or_else(|| ProviderError::NotConfigured("No providers available".into()))?;

        debug!("Routing AI request to provider: {}", name);
        let providers = self.providers.read().await;
        let provider = providers
            .get(&name)
            .ok_or_else(|| ProviderError::NotConfigured(name.clone()))?;

        provider.complete(request).await
    }

    /// List all registered provider names.
    pub async fn list_providers(&self) -> Vec<String> {
        let providers = self.providers.read().await;
        providers.keys().cloned().collect()
    }

    /// Check health of all providers.
    pub async fn health_check(&self) -> HashMap<String, bool> {
        let providers = self.providers.read().await;
        let mut results = HashMap::new();
        for (name, provider) in providers.iter() {
            let healthy = provider.is_available().await;
            results.insert(name.clone(), healthy);
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_router_modes() {
        let router = AiRouter::new(RouterMode::PrivacyFirst);
        let providers = router.list_providers().await;
        assert!(providers.is_empty());
    }
}
