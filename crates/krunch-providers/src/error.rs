//! Provider errors + retry classification (PLAN §3c/§8).
//!
//! The transient allowlist is strict: connection failures, connect/idle/total
//! timeouts, HTTP 429, and HTTP 5xx. Everything else (auth, 4xx, malformed
//! response) is permanent and must not be retried.

/// A diagnostic-friendly provider error. Mirrors the fields persisted in
/// `error_records` (PLAN §7): kind, HTTP status, detail.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ProviderError {
    /// Retryable per the strict allowlist.
    #[error("transient {kind}: {detail}")]
    Transient {
        kind: TransientKind,
        status: Option<u16>,
        detail: String,
    },
    /// Not retryable.
    #[error("permanent {kind}: {detail}")]
    Permanent {
        kind: PermanentKind,
        status: Option<u16>,
        detail: String,
    },
    /// The caller cancelled (abandon). Never retried.
    #[error("cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransientKind {
    Connect,
    IdleTimeout,
    TotalTimeout,
    RateLimited,
    ServerError,
    Network,
}

impl std::fmt::Display for TransientKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Connect => "connect",
            Self::IdleTimeout => "idle_timeout",
            Self::TotalTimeout => "total_timeout",
            Self::RateLimited => "rate_limited",
            Self::ServerError => "server_error",
            Self::Network => "network",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermanentKind {
    Auth,
    BadRequest,
    NotFound,
    MalformedResponse,
    Other,
}

impl std::fmt::Display for PermanentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Auth => "auth",
            Self::BadRequest => "bad_request",
            Self::NotFound => "not_found",
            Self::MalformedResponse => "malformed_response",
            Self::Other => "other",
        };
        f.write_str(s)
    }
}

impl ProviderError {
    /// True if this error is on the strict retry allowlist.
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Transient { .. })
    }

    /// The HTTP status, if any (for the diagnostic record).
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Transient { status, .. } | Self::Permanent { status, .. } => *status,
            Self::Cancelled => None,
        }
    }

    /// A short, stable machine label for the diagnostic record.
    pub fn kind_label(&self) -> String {
        match self {
            Self::Transient { kind, .. } => format!("transient/{kind}"),
            Self::Permanent { kind, .. } => format!("permanent/{kind}"),
            Self::Cancelled => "cancelled".into(),
        }
    }

    /// Classify an HTTP status code into the correct error variant.
    pub fn from_status(status: u16, detail: impl Into<String>) -> Self {
        let detail = detail.into();
        match status {
            429 => Self::Transient {
                kind: TransientKind::RateLimited,
                status: Some(status),
                detail,
            },
            500..=599 => Self::Transient {
                kind: TransientKind::ServerError,
                status: Some(status),
                detail,
            },
            401 | 403 => Self::Permanent {
                kind: PermanentKind::Auth,
                status: Some(status),
                detail,
            },
            404 => Self::Permanent {
                kind: PermanentKind::NotFound,
                status: Some(status),
                detail,
            },
            400..=499 => Self::Permanent {
                kind: PermanentKind::BadRequest,
                status: Some(status),
                detail,
            },
            _ => Self::Permanent { kind: PermanentKind::Other, status: Some(status), detail },
        }
    }

    /// Classify a `reqwest` error (connection/timeout/etc.) at the network layer.
    pub fn from_reqwest(err: &reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::Transient {
                kind: TransientKind::TotalTimeout,
                status: None,
                detail: err.to_string(),
            }
        } else if err.is_connect() {
            Self::Transient {
                kind: TransientKind::Connect,
                status: None,
                detail: err.to_string(),
            }
        } else {
            Self::Transient {
                kind: TransientKind::Network,
                status: err.status().map(|s| s.as_u16()),
                detail: err.to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_classification_matches_allowlist() {
        assert!(ProviderError::from_status(429, "").is_transient());
        assert!(ProviderError::from_status(500, "").is_transient());
        assert!(ProviderError::from_status(503, "").is_transient());

        assert!(!ProviderError::from_status(400, "").is_transient());
        assert!(!ProviderError::from_status(401, "").is_transient());
        assert!(!ProviderError::from_status(403, "").is_transient());
        assert!(!ProviderError::from_status(404, "").is_transient());
    }

    #[test]
    fn auth_errors_are_permanent_with_status() {
        let e = ProviderError::from_status(401, "bad key");
        assert!(!e.is_transient());
        assert_eq!(e.status(), Some(401));
        assert_eq!(e.kind_label(), "permanent/auth");
    }

    #[test]
    fn rate_limit_label_is_stable() {
        let e = ProviderError::from_status(429, "slow down");
        assert_eq!(e.kind_label(), "transient/rate_limited");
        assert_eq!(e.status(), Some(429));
    }
}
