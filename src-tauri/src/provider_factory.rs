//! The real [`AgentProvider`]: validates the endpoint, resolves the credential
//! from the keychain, and constructs the matching provider adapter (PLAN §2/§9).

use krunch_core::config::{is_loopback_url, Provider, SeatConfig};
use krunch_engine::{AgentProvider, EngineError};
use krunch_providers::agent::Agent;
use krunch_providers::{AnthropicAgent, CliAgent, CliKind, DemoAgent, OpenAiCompatibleAgent};

use crate::credentials;

/// Builds live provider agents from seat config (M7: HTTP, CLI, or demo).
pub struct KeychainProviderFactory;

impl KeychainProviderFactory {
    /// Resolve a credential for an HTTP seat. Loopback endpoints (local servers)
    /// may run key-free, so a missing credential there degrades to an empty key
    /// rather than an error (PLAN §9 / M7).
    fn http_key(seat: &SeatConfig) -> Result<String, EngineError> {
        credentials::validate_endpoint(&seat.base_url)
            .map_err(|e| EngineError::AgentBuild { seat: seat.id, detail: e.to_string() })?;
        match credentials::resolve(&seat.credential_ref) {
            Ok(key) => Ok(key),
            Err(detail) => {
                if is_loopback_url(&seat.base_url) {
                    Ok(String::new())
                } else {
                    Err(EngineError::AgentBuild { seat: seat.id, detail })
                }
            }
        }
    }

    fn model_opt(seat: &SeatConfig) -> Option<String> {
        let m = seat.model.trim();
        if m.is_empty() {
            None
        } else {
            Some(m.to_string())
        }
    }
}

impl AgentProvider for KeychainProviderFactory {
    fn build(&self, seat: &SeatConfig) -> Result<Box<dyn Agent>, EngineError> {
        let agent: Box<dyn Agent> = match seat.provider {
            Provider::Anthropic => {
                Box::new(AnthropicAgent::new(seat.base_url.clone(), Self::http_key(seat)?))
            }
            Provider::OpenAiCompatible => {
                Box::new(OpenAiCompatibleAgent::new(seat.base_url.clone(), Self::http_key(seat)?))
            }
            Provider::ClaudeCli => Box::new(CliAgent::new(CliKind::Claude, Self::model_opt(seat))),
            Provider::CodexCli => Box::new(CliAgent::new(CliKind::Codex, Self::model_opt(seat))),
            Provider::Demo => Box::new(DemoAgent::default()),
        };
        Ok(agent)
    }
}
