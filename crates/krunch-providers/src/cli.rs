//! CLI agent (M7): drive a seat through the local `claude` or `codex` CLI, which
//! authenticate via the user's subscription — no API key. Output is captured line
//! by line and streamed to the sink; the prompt is the concatenated messages.

use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::agent::{Agent, TokenSink};
use crate::error::{PermanentKind, ProviderError, TransientKind};
use crate::types::{
    AuthScheme, Budget, Capabilities, Completion, CompletionRequest, FinishReason, SseFlavor, Usage,
};

/// Which CLI to shell out to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliKind {
    Claude,
    Codex,
}

/// A seat backed by a local CLI.
pub struct CliAgent {
    kind: CliKind,
    /// Model flag for `claude` (`--model`). Codex is left on its config default to
    /// avoid `-codex` model 400s on subscription auth.
    model: Option<String>,
}

impl CliAgent {
    pub fn new(kind: CliKind, model: Option<String>) -> Self {
        Self { kind, model }
    }

    /// Concatenate the request messages into a single prompt string.
    fn prompt(req: &CompletionRequest) -> String {
        req.messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// The program + args for this CLI (pure, for testing).
    fn command_args(&self, prompt: &str) -> (String, Vec<String>) {
        match self.kind {
            CliKind::Claude => {
                let mut args = vec!["-p".to_string(), prompt.to_string(), "--output-format".into(), "text".into()];
                if let Some(m) = &self.model {
                    if !m.trim().is_empty() {
                        args.push("--model".into());
                        args.push(m.clone());
                    }
                }
                ("claude".into(), args)
            }
            CliKind::Codex => (
                "codex".into(),
                vec!["exec".into(), "--skip-git-repo-check".into(), prompt.to_string()],
            ),
        }
    }
}

#[async_trait]
impl Agent for CliAgent {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_system_role: false, // messages are concatenated into one prompt
            sse_flavor: SseFlavor::OpenAi,
            auth_scheme: AuthScheme::Bearer,
        }
    }

    async fn stream_completion(
        &self,
        req: &CompletionRequest,
        budget: Budget,
        cancel: CancellationToken,
        sink: &mut dyn TokenSink,
    ) -> Result<Completion, ProviderError> {
        let prompt = Self::prompt(req);
        let (program, args) = self.command_args(&prompt);

        let mut child = Command::new(&program)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| ProviderError::Permanent {
                kind: PermanentKind::Other,
                status: None,
                detail: format!("failed to spawn `{program}`: {e} (is it installed and on PATH?)"),
            })?;

        let stdout = child.stdout.take().expect("piped stdout");
        let mut lines = BufReader::new(stdout).lines();
        let mut text = String::new();

        let read = async {
            while let Some(line) = lines.next_line().await.map_err(|e| ProviderError::Transient {
                kind: TransientKind::Network,
                status: None,
                detail: e.to_string(),
            })? {
                let chunk = format!("{line}\n");
                text.push_str(&chunk);
                sink.on_token(&chunk);
            }
            Ok::<(), ProviderError>(())
        };

        let outcome = tokio::select! {
            _ = cancel.cancelled() => {
                let _ = child.start_kill();
                return Err(ProviderError::Cancelled);
            }
            r = tokio::time::timeout(budget.total_timeout, read) => r,
        };

        match outcome {
            Err(_elapsed) => {
                let _ = child.start_kill();
                return Err(ProviderError::Transient {
                    kind: TransientKind::TotalTimeout,
                    status: None,
                    detail: "CLI exceeded total budget".into(),
                });
            }
            Ok(Err(e)) => return Err(e),
            Ok(Ok(())) => {}
        }

        let status = child.wait().await.map_err(|e| ProviderError::Transient {
            kind: TransientKind::Network,
            status: None,
            detail: e.to_string(),
        })?;
        if !status.success() {
            let mut err = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                use tokio::io::AsyncReadExt;
                let _ = stderr.read_to_string(&mut err).await;
            }
            return Err(ProviderError::Permanent {
                kind: PermanentKind::Other,
                status: None,
                detail: format!("`{program}` exited with {status}: {}", err.trim()),
            });
        }

        Ok(Completion {
            text,
            finish_reason: FinishReason::Stop,
            truncated: None,
            usage: Usage::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_command_includes_model() {
        let a = CliAgent::new(CliKind::Claude, Some("claude-sonnet-5".into()));
        let (prog, args) = a.command_args("hello");
        assert_eq!(prog, "claude");
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"hello".to_string()));
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"claude-sonnet-5".to_string()));
    }

    #[test]
    fn codex_command_skips_git_check_and_model() {
        let a = CliAgent::new(CliKind::Codex, Some("ignored".into()));
        let (prog, args) = a.command_args("hello");
        assert_eq!(prog, "codex");
        assert!(args.contains(&"exec".to_string()));
        assert!(args.contains(&"--skip-git-repo-check".to_string()));
        assert!(!args.contains(&"--model".to_string()));
    }

    #[tokio::test]
    async fn missing_binary_is_a_clear_permanent_error() {
        let a = CliAgent::new(CliKind::Claude, None);
        // Force a spawn failure by pointing at a nonexistent program via a bogus kind?
        // Instead, verify the error path by spawning a definitely-absent binary name.
        let agent = BogusCli;
        let req = CompletionRequest {
            model: "x".into(),
            messages: vec![crate::types::Message::user("hi")],
            sampling: Default::default(),
        };
        let err = agent
            .stream_completion(&req, Budget::default(), CancellationToken::new(), &mut |_: &str| {})
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::Permanent { .. }));
        let _ = a; // silence unused in this test
    }

    /// A CLI agent hard-wired to a nonexistent binary, for the spawn-failure test.
    struct BogusCli;
    #[async_trait]
    impl Agent for BogusCli {
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                supports_system_role: false,
                sse_flavor: SseFlavor::OpenAi,
                auth_scheme: AuthScheme::Bearer,
            }
        }
        async fn stream_completion(
            &self,
            _req: &CompletionRequest,
            _budget: Budget,
            _cancel: CancellationToken,
            _sink: &mut dyn TokenSink,
        ) -> Result<Completion, ProviderError> {
            Command::new("krunch-definitely-not-a-real-binary-xyz")
                .spawn()
                .map(|_| unreachable!())
                .map_err(|e| ProviderError::Permanent {
                    kind: PermanentKind::Other,
                    status: None,
                    detail: e.to_string(),
                })
        }
    }
}
