//! The real [`AgentProvider`]: validates the endpoint, resolves the credential
//! from the keychain, and constructs the matching provider adapter (PLAN §2/§9).

use krunch_core::config::{Provider, SeatConfig};
use krunch_engine::{AgentProvider, EngineError};
use krunch_providers::agent::Agent;
use krunch_providers::{AnthropicAgent, OpenAiCompatibleAgent};

use crate::credentials;

/// Builds live provider agents from seat config.
pub struct KeychainProviderFactory;

impl AgentProvider for KeychainProviderFactory {
    fn build(&self, seat: &SeatConfig) -> Result<Box<dyn Agent>, EngineError> {
        credentials::validate_endpoint(&seat.base_url)
            .map_err(|e| EngineError::AgentBuild { seat: seat.id, detail: e.to_string() })?;
        let key = credentials::resolve(&seat.credential_ref)
            .map_err(|detail| EngineError::AgentBuild { seat: seat.id, detail })?;

        let agent: Box<dyn Agent> = match seat.provider {
            Provider::Anthropic => Box::new(AnthropicAgent::new(seat.base_url.clone(), key)),
            Provider::OpenAiCompatible => {
                Box::new(OpenAiCompatibleAgent::new(seat.base_url.clone(), key))
            }
        };
        Ok(agent)
    }
}
