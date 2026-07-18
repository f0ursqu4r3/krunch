//! Credential resolution + endpoint safety (PLAN §9).
//!
//! Keys live in the OS keychain, referenced by an opaque `credential_ref`. Before
//! a key is ever attached to a request we validate the endpoint origin: HTTPS is
//! required except for an explicit loopback opt-in (local Ollama/LM Studio).
//! Cross-origin redirects are disabled at the reqwest layer (see providers).

use url::Url;

const KEYRING_SERVICE: &str = "krunch";

/// Endpoint validation failures.
#[derive(Debug, thiserror::Error)]
pub enum EndpointError {
    #[error("invalid base_url: {0}")]
    Invalid(String),
    #[error("insecure endpoint: {0} — only https (or loopback http) may carry a credential")]
    Insecure(String),
}

/// True for localhost / 127.0.0.1 / ::1 (the only hosts allowed over plain http).
fn is_loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

/// Validate that a base_url may carry a credential (PLAN §9).
pub fn validate_endpoint(base_url: &str) -> Result<(), EndpointError> {
    let url = Url::parse(base_url).map_err(|e| EndpointError::Invalid(e.to_string()))?;
    let host = url.host_str().ok_or_else(|| EndpointError::Invalid("no host".into()))?;
    match url.scheme() {
        "https" => Ok(()),
        "http" if is_loopback(host) => Ok(()),
        _ => Err(EndpointError::Insecure(base_url.to_string())),
    }
}

/// Fetch the secret for a `credential_ref` from the OS keychain.
pub fn resolve(credential_ref: &str) -> Result<String, String> {
    keyring::Entry::new(KEYRING_SERVICE, credential_ref)
        .and_then(|e| e.get_password())
        .map_err(|e| format!("keychain: {e}"))
}

/// Store (or overwrite) the secret for a `credential_ref`. Invoked by the UI when
/// the user enters a key — krunch never sees keys except to hand them to the
/// keychain the user controls.
pub fn store(credential_ref: &str, secret: &str) -> Result<(), String> {
    keyring::Entry::new(KEYRING_SERVICE, credential_ref)
        .and_then(|e| e.set_password(secret))
        .map_err(|e| format!("keychain: {e}"))
}

/// Whether a secret exists for `credential_ref` (without revealing it).
pub fn exists(credential_ref: &str) -> bool {
    resolve(credential_ref).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_endpoints_are_allowed() {
        assert!(validate_endpoint("https://api.openai.com/v1").is_ok());
        assert!(validate_endpoint("https://api.anthropic.com").is_ok());
    }

    #[test]
    fn loopback_http_is_allowed() {
        assert!(validate_endpoint("http://localhost:11434/v1").is_ok());
        assert!(validate_endpoint("http://127.0.0.1:1234/v1").is_ok());
    }

    #[test]
    fn remote_http_is_rejected() {
        assert!(matches!(
            validate_endpoint("http://api.example.com/v1"),
            Err(EndpointError::Insecure(_))
        ));
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(matches!(validate_endpoint("not a url"), Err(EndpointError::Invalid(_))));
    }
}
