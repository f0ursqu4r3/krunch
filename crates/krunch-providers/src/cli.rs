//! CLI agent (M7): drive a seat through the local `claude` or `codex` CLI, which
//! authenticate via the user's subscription — no API key. The prompt is the
//! concatenated messages.
//!
//! Seats run *clean-room*: both CLIs are told to skip the user's personal setup
//! (hooks, CLAUDE.md/AGENTS.md, plugins, skills, MCP servers, session persistence),
//! because that setup otherwise fires SessionStart hooks on every seat attempt,
//! injects tens of thousands of tokens of user context into each panelist prompt,
//! and skews panelist behavior with the user's personal instructions.
//!
//! Both CLIs default to a *buffered* output mode (`claude -p --output-format text`,
//! plain `codex exec`) that withholds all stdout until the process exits — so the
//! seat card would sit on "awaiting stream" for the whole generation, then dump the
//! full answer at once. To stream live we ask each CLI for its realtime JSONL mode
//! (`claude --output-format stream-json`, `codex exec --json`) and parse events line
//! by line, forwarding assistant text to the sink as it arrives and capturing the
//! final token usage.

use std::process::Stdio;

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::AsyncReadExt;
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

/// A meaningful thing decoded from one line of a CLI's JSONL stream.
#[derive(Debug, Clone, PartialEq)]
enum CliEvent {
    /// Assistant text to forward to the sink.
    Text(String),
    /// Final token usage, reported once near end of stream.
    Usage(Usage),
    /// A terminal failure the CLI reported *on stdout* (auth expiry, quota,
    /// model refusal). The CLI writes these into the JSONL stream and exits
    /// non-zero with an empty stderr, so the exit-code path alone can't explain
    /// them — we must catch them here to surface a real reason.
    Error(String),
}

impl CliKind {
    /// Parse a single JSONL line into an event, if it carries one. Lines we don't
    /// care about (thinking deltas, tool calls, hook chatter) and non-JSON log lines
    /// (e.g. codex's stderr-style warnings) yield `None` and are skipped.
    fn parse_line(&self, line: &str) -> Option<CliEvent> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let v: Value = serde_json::from_str(line).ok()?;
        match self {
            CliKind::Claude => parse_claude(&v),
            CliKind::Codex => parse_codex(&v),
        }
    }
}

/// Pull `input_tokens` / `output_tokens` out of a `usage` object.
fn usage_from(v: &Value) -> Usage {
    let get = |k: &str| v.get(k).and_then(Value::as_u64).map(|n| n as u32);
    Usage { input_tokens: get("input_tokens"), output_tokens: get("output_tokens") }
}

/// Claude `--output-format stream-json`: text arrives as `content_block_delta`
/// events with a `text_delta` (thinking/signature deltas are ignored); the final
/// `result` event carries authoritative usage.
fn parse_claude(v: &Value) -> Option<CliEvent> {
    match v.get("type")?.as_str()? {
        "stream_event" => {
            let event = v.get("event")?;
            if event.get("type")?.as_str()? == "content_block_delta" {
                let delta = event.get("delta")?;
                if delta.get("type")?.as_str()? == "text_delta" {
                    return Some(CliEvent::Text(delta.get("text")?.as_str()?.to_string()));
                }
            }
            None
        }
        "result" => {
            // A terminal result flagged `is_error` carries the CLI's failure
            // message in `result` — and only on stdout, where our stderr capture
            // never sees it. Surface it so the seat reports *why* it failed
            // (e.g. expired subscription auth) instead of a bare non-zero exit.
            if v.get("is_error").and_then(Value::as_bool).unwrap_or(false) {
                let msg = v
                    .get("result")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("claude reported an error")
                    .to_string();
                return Some(CliEvent::Error(msg));
            }
            let u = v.get("usage")?;
            let mut usage = usage_from(u);
            // Claude reports cache writes/reads separately from `input_tokens`; the
            // seat's real context size is the sum. (Codex needs no such fold — its
            // `input_tokens` already includes `cached_input_tokens`.)
            let cached: u64 = ["cache_creation_input_tokens", "cache_read_input_tokens"]
                .iter()
                .filter_map(|k| u.get(*k).and_then(Value::as_u64))
                .sum();
            if cached > 0 {
                usage.input_tokens = Some(usage.input_tokens.unwrap_or(0) + cached as u32);
            }
            Some(CliEvent::Usage(usage))
        }
        _ => None,
    }
}

/// Codex `exec --json`: assistant text arrives as completed `agent_message` items
/// (message-level, not token-level — codex has no finer stream); tool calls and
/// reasoning items are ignored; `turn.completed` carries usage.
fn parse_codex(v: &Value) -> Option<CliEvent> {
    match v.get("type")?.as_str()? {
        "item.completed" => {
            let item = v.get("item")?;
            if item.get("type")?.as_str()? == "agent_message" {
                return Some(CliEvent::Text(item.get("text")?.as_str()?.to_string()));
            }
            None
        }
        "turn.completed" => Some(CliEvent::Usage(usage_from(v.get("usage")?))),
        _ => None,
    }
}

/// Classify a CLI-reported error message into a permanent provider error. Auth
/// failures (an expired subscription/OAuth session) are the common case and get
/// their own kind so the record and the seat reason read `permanent/auth` rather
/// than an opaque `permanent/other`.
fn cli_error(kind: CliKind, message: &str) -> ProviderError {
    let program = match kind {
        CliKind::Claude => "claude",
        CliKind::Codex => "codex",
    };
    let lower = message.to_lowercase();
    let permanent = if ["authenticat", "oauth", "unauthorized", "log in", "login", "credential"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        PermanentKind::Auth
    } else {
        PermanentKind::Other
    };
    ProviderError::Permanent {
        kind: permanent,
        status: None,
        detail: format!("`{program}`: {}", message.trim()),
    }
}

/// Incremental UTF-8 decoder for byte-chunk streaming. Bytes arrive from the child
/// in arbitrary reads that may split a multibyte character; this emits only the
/// complete-character prefix and buffers the incomplete tail for the next read.
#[derive(Default)]
struct Utf8Streamer {
    pending: Vec<u8>,
}

impl Utf8Streamer {
    /// Feed raw bytes; return whatever newly decodes to valid UTF-8.
    fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        match std::str::from_utf8(&self.pending) {
            Ok(s) => {
                let out = s.to_string();
                self.pending.clear();
                out
            }
            Err(e) => {
                let valid = e.valid_up_to();
                // Safe: `valid_up_to` is a validated boundary.
                let out = String::from_utf8_lossy(&self.pending[..valid]).into_owned();
                self.pending.drain(..valid);
                // If the tail is longer than any real char, it's genuinely
                // malformed — flush it lossily so we never stall.
                if self.pending.len() > 3 {
                    let rest = String::from_utf8_lossy(&self.pending).into_owned();
                    self.pending.clear();
                    return out + &rest;
                }
                out
            }
        }
    }

    /// Flush any buffered tail at end of stream (lossily if incomplete).
    fn finish(&mut self) -> String {
        if self.pending.is_empty() {
            return String::new();
        }
        let out = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        out
    }
}

/// Read a child's stdout in byte chunks, split it into JSONL lines, and stream the
/// assistant text from each parsed event to `sink` as it arrives. Returns the full
/// assembled text plus the final reported usage. Generic over the reader so the
/// streaming behavior is unit-testable without spawning a process.
async fn pump<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
    kind: CliKind,
    sink: &mut dyn TokenSink,
) -> Result<(String, Usage), ProviderError> {
    let mut streamer = Utf8Streamer::default();
    let mut line_buf = String::new();
    let mut text = String::new();
    let mut usage = Usage::default();
    let mut buf = [0u8; 4096];

    let mut apply = |ev: CliEvent,
                     text: &mut String,
                     usage: &mut Usage|
     -> Result<(), ProviderError> {
        match ev {
            CliEvent::Text(t) => {
                text.push_str(&t);
                sink.on_token(&t);
            }
            CliEvent::Usage(u) => *usage = u,
            // A terminal error the CLI reported on stdout ends the stream.
            CliEvent::Error(msg) => return Err(cli_error(kind, &msg)),
        }
        Ok(())
    };

    loop {
        let n = reader.read(&mut buf).await.map_err(|e| ProviderError::Transient {
            kind: TransientKind::Network,
            status: None,
            detail: e.to_string(),
        })?;
        if n == 0 {
            break;
        }
        line_buf.push_str(&streamer.push(&buf[..n]));
        // Drain every complete line; the final partial line stays buffered.
        while let Some(nl) = line_buf.find('\n') {
            let line: String = line_buf.drain(..=nl).collect();
            if let Some(ev) = kind.parse_line(&line) {
                apply(ev, &mut text, &mut usage)?;
            }
        }
    }
    // Flush any buffered UTF-8 tail and a final newline-less line.
    line_buf.push_str(&streamer.finish());
    if let Some(ev) = kind.parse_line(&line_buf) {
        apply(ev, &mut text, &mut usage)?;
    }
    Ok((text, usage))
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

    /// The program + args for this CLI (pure, for testing). Both use realtime JSONL
    /// output so tokens stream live rather than dumping at process exit; claude's
    /// stream-json print mode additionally requires `--verbose`.
    ///
    /// Both also get clean-room flags so a seat never inherits the user's personal
    /// setup. For claude that is `--safe-mode` (disables CLAUDE.md, hooks, skills,
    /// plugins, MCP; auth and model selection still work — unlike `--bare`, which
    /// refuses OAuth/keychain auth and would break subscription seats), plus no
    /// session persistence, no MCP outside `--mcp-config`, and no built-in tools
    /// (`--tools ""` — panelists only emit text, and it keeps tool schemas out of
    /// the prompt). For codex that is `--ignore-user-config` (auth still works),
    /// no execpolicy rules, no session files, and no AGENTS.md project docs.
    fn command_args(&self, prompt: &str) -> (String, Vec<String>) {
        match self.kind {
            CliKind::Claude => {
                let mut args = vec![
                    "-p".to_string(),
                    prompt.to_string(),
                    "--output-format".into(),
                    "stream-json".into(),
                    "--include-partial-messages".into(),
                    "--verbose".into(),
                    "--safe-mode".into(),
                    "--no-session-persistence".into(),
                    "--strict-mcp-config".into(),
                    "--tools".into(),
                    String::new(),
                ];
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
                vec![
                    "exec".into(),
                    "--json".into(),
                    "--skip-git-repo-check".into(),
                    "--ignore-user-config".into(),
                    "--ignore-rules".into(),
                    "--ephemeral".into(),
                    "-c".into(),
                    "project_doc_max_bytes=0".into(),
                    prompt.to_string(),
                ],
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

        let mut stdout = child.stdout.take().expect("piped stdout");

        // JSONL streaming: emit assistant text as fast as the CLI flushes events.
        let outcome = tokio::select! {
            _ = cancel.cancelled() => {
                let _ = child.start_kill();
                return Err(ProviderError::Cancelled);
            }
            r = tokio::time::timeout(budget.total_timeout, pump(&mut stdout, self.kind, sink)) => r,
        };

        let (text, usage) = match outcome {
            Err(_elapsed) => {
                let _ = child.start_kill();
                return Err(ProviderError::Transient {
                    kind: TransientKind::TotalTimeout,
                    status: None,
                    detail: "CLI exceeded total budget".into(),
                });
            }
            Ok(Err(e)) => return Err(e),
            Ok(Ok(pair)) => pair,
        };

        let status = child.wait().await.map_err(|e| ProviderError::Transient {
            kind: TransientKind::Network,
            status: None,
            detail: e.to_string(),
        })?;
        if !status.success() {
            // A terminal error line on stdout (auth expiry, quota) is caught in
            // `pump` above and returns early; reaching here means the CLI exited
            // non-zero without one. stderr is often empty in that case, so name
            // the likely cause rather than emitting a bare exit code.
            let mut err = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                use tokio::io::AsyncReadExt;
                let _ = stderr.read_to_string(&mut err).await;
            }
            let detail = if err.trim().is_empty() {
                format!("`{program}` exited with {status} (no stderr — check it is installed and authenticated: run `{program}`)")
            } else {
                format!("`{program}` exited with {status}: {}", err.trim())
            };
            return Err(ProviderError::Permanent {
                kind: PermanentKind::Other,
                status: None,
                detail,
            });
        }

        Ok(Completion {
            text,
            finish_reason: FinishReason::Stop,
            truncated: None,
            usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_streamer_handles_a_split_multibyte_char() {
        let mut s = Utf8Streamer::default();
        // "é" is 0xC3 0xA9 — split across two reads.
        assert_eq!(s.push(b"H\xc3"), "H"); // trailing 0xC3 buffered
        assert_eq!(s.push(b"\xa9llo"), "éllo"); // completes é, then llo
        assert_eq!(s.finish(), "");
    }

    #[test]
    fn utf8_streamer_flushes_tail_on_finish() {
        let mut s = Utf8Streamer::default();
        assert_eq!(s.push(b"ab"), "ab");
        assert_eq!(s.push(b"\xe2\x82"), ""); // first 2 bytes of € (3-byte char)
        assert_eq!(s.push(b"\xac"), "€"); // completes €
        assert_eq!(s.finish(), "");
    }

    #[test]
    fn claude_parser_extracts_text_deltas_and_usage() {
        let text = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"hello"}}}"#;
        assert_eq!(CliKind::Claude.parse_line(text), Some(CliEvent::Text("hello".into())));

        // Thinking/signature deltas and assistant snapshots are NOT assistant text.
        let thinking = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"abc"}}}"#;
        assert_eq!(CliKind::Claude.parse_line(thinking), None);
        let snapshot = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#;
        assert_eq!(CliKind::Claude.parse_line(snapshot), None);

        let result = r#"{"type":"result","result":"hello","usage":{"input_tokens":12,"output_tokens":58}}"#;
        assert_eq!(
            CliKind::Claude.parse_line(result),
            Some(CliEvent::Usage(Usage { input_tokens: Some(12), output_tokens: Some(58) }))
        );
    }

    #[test]
    fn claude_result_flags_stdout_error() {
        // Auth expiry: the CLI reports it as a terminal `result` with `is_error`
        // on *stdout* (stderr is empty), so it must be caught as an Error event,
        // not parsed as usage.
        let line = r#"{"type":"result","subtype":"success","is_error":true,"result":"Failed to authenticate: OAuth session expired and could not be refreshed","usage":{"input_tokens":0,"output_tokens":0}}"#;
        assert_eq!(
            CliKind::Claude.parse_line(line),
            Some(CliEvent::Error(
                "Failed to authenticate: OAuth session expired and could not be refreshed".into()
            ))
        );

        // A normal (non-error) result still parses as usage.
        let ok = r#"{"type":"result","is_error":false,"result":"hi","usage":{"input_tokens":3,"output_tokens":4}}"#;
        assert_eq!(
            CliKind::Claude.parse_line(ok),
            Some(CliEvent::Usage(Usage { input_tokens: Some(3), output_tokens: Some(4) }))
        );
    }

    #[test]
    fn cli_error_classifies_auth_vs_other() {
        let auth = cli_error(CliKind::Claude, "Failed to authenticate: OAuth session expired");
        assert!(!auth.is_transient());
        assert_eq!(auth.kind_label(), "permanent/auth");
        assert!(auth.to_string().contains("authenticate"));
        // Non-auth messages stay `other`.
        assert_eq!(cli_error(CliKind::Codex, "disk full").kind_label(), "permanent/other");
    }

    #[tokio::test]
    async fn pump_surfaces_stdout_error_as_classified_permanent() {
        // The real failing stream: a system init line, then a terminal result
        // flagged is_error. pump must return a classified Auth error carrying the
        // message — not silently yield empty text and defer to a bare exit code.
        let payload = concat!(
            r#"{"type":"system","subtype":"init"}"#,
            "\n",
            r#"{"type":"result","subtype":"success","is_error":true,"result":"Failed to authenticate: OAuth session expired and could not be refreshed","usage":{"input_tokens":0,"output_tokens":0}}"#,
            "\n",
        )
        .to_string();

        let (mut w, mut r) = tokio::io::duplex(1024);
        let writer = tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            w.write_all(payload.as_bytes()).await.unwrap();
            drop(w);
        });

        let err = pump(&mut r, CliKind::Claude, &mut |_c: &str| {}).await.unwrap_err();
        writer.await.unwrap();

        assert_eq!(err.kind_label(), "permanent/auth");
        assert!(err.to_string().contains("OAuth session expired"));
    }

    #[test]
    fn claude_usage_folds_cache_tokens_into_input() {
        // On a warm prompt cache, claude puts nearly all context in the cache fields
        // and `input_tokens` alone would report a misleading ~10 tokens.
        let result = r#"{"type":"result","usage":{"input_tokens":10,"cache_creation_input_tokens":11220,"cache_read_input_tokens":20477,"output_tokens":72}}"#;
        assert_eq!(
            CliKind::Claude.parse_line(result),
            Some(CliEvent::Usage(Usage {
                input_tokens: Some(10 + 11220 + 20477),
                output_tokens: Some(72)
            }))
        );
    }

    #[test]
    fn codex_parser_extracts_agent_messages_and_usage() {
        let msg = r#"{"type":"item.completed","item":{"id":"item_2","type":"agent_message","text":"hello"}}"#;
        assert_eq!(CliKind::Codex.parse_line(msg), Some(CliEvent::Text("hello".into())));

        // Tool calls are not assistant text.
        let cmd = r#"{"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"ls"}}"#;
        assert_eq!(CliKind::Codex.parse_line(cmd), None);

        let turn = r#"{"type":"turn.completed","usage":{"input_tokens":33534,"output_tokens":214}}"#;
        assert_eq!(
            CliKind::Codex.parse_line(turn),
            Some(CliEvent::Usage(Usage { input_tokens: Some(33534), output_tokens: Some(214) }))
        );

        // Non-JSON log lines (codex prints warnings to stdout) are skipped.
        assert_eq!(CliKind::Codex.parse_line("Reading additional input from stdin..."), None);
    }

    #[tokio::test]
    async fn pump_streams_text_deltas_and_reassembles_split_lines() {
        // A duplex pipe with tiny capacity forces pump to read in small pieces, so
        // JSONL lines arrive split across reads (incl. a split 😀 mid-byte-sequence)
        // and must be re-buffered before parsing. The sink should see only the
        // assistant text deltas, live and in order; usage comes from the result line.
        let line1 = "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello \"}}}\n";
        let line2 = "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"😀!\"}}}\n";
        let line3 = "{\"type\":\"result\",\"result\":\"Hello 😀!\",\"usage\":{\"input_tokens\":2,\"output_tokens\":9}}\n";
        let payload = format!("{line1}{line2}{line3}");

        let (mut w, mut r) = tokio::io::duplex(2);
        let writer = tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            w.write_all(payload.as_bytes()).await.unwrap();
            drop(w);
        });

        let mut chunks: Vec<String> = Vec::new();
        let (text, usage) =
            pump(&mut r, CliKind::Claude, &mut |c: &str| chunks.push(c.to_string())).await.unwrap();
        writer.await.unwrap();

        assert_eq!(text, "Hello 😀!"); // reassembled across split reads
        assert_eq!(chunks, vec!["Hello ".to_string(), "😀!".to_string()]); // per-delta, live
        assert_eq!(usage, Usage { input_tokens: Some(2), output_tokens: Some(9) });
    }

    #[test]
    fn claude_command_includes_model() {
        let a = CliAgent::new(CliKind::Claude, Some("claude-sonnet-5".into()));
        let (prog, args) = a.command_args("hello");
        assert_eq!(prog, "claude");
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"hello".to_string()));
        // Realtime streaming, not the buffered `text` mode.
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"--include-partial-messages".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
        assert!(!args.contains(&"text".to_string()));
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"claude-sonnet-5".to_string()));
    }

    #[test]
    fn claude_command_is_clean_room() {
        let a = CliAgent::new(CliKind::Claude, Some("claude-sonnet-5".into()));
        let (_, args) = a.command_args("hello");
        // No user hooks/CLAUDE.md/skills/plugins/MCP (but NOT --bare, which would
        // reject subscription OAuth), no session files, no built-in tools.
        assert!(args.contains(&"--safe-mode".to_string()));
        assert!(!args.contains(&"--bare".to_string()));
        assert!(args.contains(&"--no-session-persistence".to_string()));
        assert!(args.contains(&"--strict-mcp-config".to_string()));
        let tools_at = args.iter().position(|a| a == "--tools").expect("--tools present");
        assert_eq!(args[tools_at + 1], "", "--tools takes an empty list");
        // The variadic `--tools` must not swallow the flag that follows it.
        assert_eq!(args[tools_at + 2], "--model");
    }

    #[test]
    fn codex_command_skips_git_check_and_model() {
        let a = CliAgent::new(CliKind::Codex, Some("ignored".into()));
        let (prog, args) = a.command_args("hello");
        assert_eq!(prog, "codex");
        assert!(args.contains(&"exec".to_string()));
        assert!(args.contains(&"--json".to_string())); // realtime JSONL events
        assert!(args.contains(&"--skip-git-repo-check".to_string()));
        assert!(!args.contains(&"--model".to_string()));
        // Prompt stays last so no flag can absorb it.
        assert_eq!(args.last().map(String::as_str), Some("hello"));
    }

    #[test]
    fn codex_command_is_clean_room() {
        let a = CliAgent::new(CliKind::Codex, None);
        let (_, args) = a.command_args("hello");
        // No user config.toml (auth still works), no execpolicy rules, no session
        // files, no AGENTS.md project docs.
        assert!(args.contains(&"--ignore-user-config".to_string()));
        assert!(args.contains(&"--ignore-rules".to_string()));
        assert!(args.contains(&"--ephemeral".to_string()));
        let c_at = args.iter().position(|a| a == "-c").expect("-c present");
        assert_eq!(args[c_at + 1], "project_doc_max_bytes=0");
    }

    /// Real end-to-end streaming against the installed `claude` CLI. Ignored by
    /// default (needs the binary + subscription auth + network); run explicitly:
    /// `cargo test -p krunch-providers -- --ignored streams_live`.
    #[tokio::test]
    #[ignore = "requires the local claude CLI + auth"]
    async fn streams_live_from_the_claude_cli() {
        use std::sync::{Arc, Mutex};

        let agent = CliAgent::new(CliKind::Claude, None);
        let req = CompletionRequest {
            model: String::new(),
            messages: vec![crate::types::Message::user(
                "Output only the numbers 1 through 15, one per line, nothing else.",
            )],
            sampling: Default::default(),
        };
        let chunks = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink_chunks = chunks.clone();
        let mut sink = move |c: &str| sink_chunks.lock().unwrap().push(c.to_string());

        let completion = agent
            .stream_completion(&req, Budget::default(), CancellationToken::new(), &mut sink)
            .await
            .expect("live stream");

        let chunks = chunks.lock().unwrap();
        // Old buffered `--output-format text` mode delivered the whole answer in one
        // blob; per-delta streaming means many sink calls arriving as tokens land.
        assert!(chunks.len() > 1, "expected incremental deltas, got {}: {chunks:?}", chunks.len());
        assert!(completion.text.contains('1') && completion.text.contains("15"));
        assert!(completion.usage.output_tokens.is_some(), "usage should be captured");
        // Clean-room check: with the user's hooks/CLAUDE.md/plugins loaded, total
        // input context (incl. cache tokens, which we fold in) runs 30k+; a
        // --safe-mode seat with no tools is a few thousand.
        let input = completion.usage.input_tokens.expect("input usage should be captured");
        assert!(input < 10_000, "seat is not clean-room: {input} input tokens");
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
