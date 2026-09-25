//! Model Context Protocol client (official Rust SDK, `rmcp`).
//!
//! Users register MCP servers in Settings (stdio subprocesses or streamable
//! HTTP endpoints). Their tools are exposed to the agent loop as
//! `mcp__<server>__<tool>`.
//!
//! Security:
//! * **Starting a stdio server is command execution.** The command line is
//!   classified by the policy engine (Denied → refused), then natively
//!   confirmed once per app session and server configuration, and audited.
//!   The child gets the executor's cleared environment plus the configured
//!   variables; secret variables come from the keychain (`mcp.<server>.<NAME>`).
//! * **HTTP servers** must pass the `local_only` endpoint check; an optional
//!   bearer token comes from the keychain (`mcp.<server>.token`).
//! * **Every tool call** is treated as `Mutating` (native confirmation) unless
//!   the user listed it in the server's `read_only_tools`, is audited
//!   (`action: mcp_call`, arguments redacted), times out after
//!   `security.command_timeout_secs`, and its output is returned to the model
//!   as untrusted data by the agent loop.

use crate::ai::provider::ToolSpec;
use crate::error::{AppError, AppResult};
use crate::security::audit::{AuditRecord, Confirmation, Decision};
use crate::security::confirm::{self, ConfirmRequest};
use crate::security::executor;
use crate::security::policy::{self, RiskTier, Source};
use crate::settings::{McpServerConfig, McpTransport};
use crate::state::AppState;
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{StreamableHttpClientTransport, TokioChildProcess};
use rmcp::{RoleClient, ServiceExt};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tauri::{AppHandle, Runtime};

/// Prefix of namespaced MCP tool names.
pub const PREFIX: &str = "mcp__";

/// A live connection.
struct Connection {
    fingerprint: String,
    service: RunningService<RoleClient, ()>,
    tools: Vec<rmcp::model::Tool>,
}

/// Connections keyed by server name.
#[derive(Default)]
pub struct McpManager {
    conns: tokio::sync::Mutex<HashMap<String, Connection>>,
}

/// Summary returned by `mcp_test_server`.
#[derive(Debug, Clone, Serialize)]
pub struct ServerStatus {
    /// Server name.
    pub name: String,
    /// Tool names it exposes.
    pub tools: Vec<String>,
}

/// Stable fingerprint of a server configuration; a change forces reconnect
/// (and, for stdio, a fresh confirmation).
fn fingerprint(cfg: &McpServerConfig) -> String {
    crate::security::audit::sha256_hex(
        serde_json::to_string(&cfg.transport)
            .unwrap_or_default()
            .as_bytes(),
    )
}

/// Tool names must be `[A-Za-z0-9_-]{1,64}` for every provider.
pub fn namespaced(server: &str, tool: &str) -> String {
    let clean: String = tool
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut n = format!("{PREFIX}{server}__{clean}");
    n.truncate(64);
    n
}

/// The command line of a stdio server, quoted for display and policy checks.
fn display_command(command: &str, args: &[String]) -> String {
    let words = std::iter::once(command).chain(args.iter().map(String::as_str));
    shlex::try_join(words).unwrap_or_else(|_| format!("{command} {}", args.join(" ")))
}

async fn secret(state: &AppState, id: String) -> AppResult<Option<String>> {
    let store = state.secrets.clone();
    tokio::task::spawn_blocking(move || store.get(&id)).await?
}

impl McpManager {
    /// Drop all connections (called when settings change).
    pub async fn disconnect_all(&self) {
        let mut conns = self.conns.lock().await;
        for (_, c) in conns.drain() {
            let _ = c.service.cancel().await;
        }
    }

    /// Ensure `cfg` is connected, starting/confirming it if needed.
    async fn connect<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        state: &AppState,
        cfg: &McpServerConfig,
    ) -> AppResult<()> {
        let fp = fingerprint(cfg);
        let mut conns = self.conns.lock().await;
        if conns.get(&cfg.name).is_some_and(|c| c.fingerprint == fp) {
            return Ok(());
        }
        if let Some(old) = conns.remove(&cfg.name) {
            let _ = old.service.cancel().await;
        }
        let settings = state.settings.read().await.clone();
        let service = match &cfg.transport {
            McpTransport::Stdio {
                command,
                args,
                env,
                secret_env,
            } => {
                let shown = display_command(command, args);
                let home = state.home.clone();
                let cwd = executor::resolve_cwd(None, home.as_deref())?;
                let pcfg = state.policy_config(&settings, Some(&cwd));
                let class = policy::classify(&shown, &pcfg);
                let base = AuditRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    source: Source::User,
                    action: "mcp_start".into(),
                    command: format!("[{}] {shown}", cfg.name),
                    cwd: Some(cwd.display().to_string()),
                    tier: class.tier,
                    decision: Decision::Denied,
                    confirmation: Confirmation::Skipped,
                    exit_code: None,
                    duration_ms: None,
                    detail: Some(class.reasons.join("; ")),
                };
                if class.tier == RiskTier::Denied {
                    state.audit.record(base).await?;
                    return Err(AppError::PolicyDenied(class.reasons.join("; ")));
                }
                let c = confirm::ask(
                    app,
                    &ConfirmRequest {
                        title: "Start MCP server?".into(),
                        subject: format!("Server: {}\nCommand:\n{shown}", cfg.name),
                        details: vec![
                            format!("Working directory: {}", cwd.display()),
                            "The server runs as your user until OMNIX exits or settings change."
                                .into(),
                        ],
                        tier: RiskTier::Mutating,
                        source: Source::User,
                        reasons: vec!["starts a long-running local program".into()],
                        approve_label: "Start".into(),
                    },
                    Duration::from_secs(settings.security.confirmation_timeout_secs),
                )
                .await;
                if c != Confirmation::Approved {
                    state
                        .audit
                        .record(AuditRecord {
                            decision: Decision::NotApproved,
                            confirmation: c,
                            ..base
                        })
                        .await?;
                    return Err(AppError::NotApproved(format!(
                        "MCP server `{}` was not started",
                        cfg.name
                    )));
                }
                let mut cmd = tokio::process::Command::new(command);
                cmd.args(args)
                    .current_dir(&cwd)
                    .env_clear()
                    .envs(executor::safe_env())
                    .envs(env);
                for name in secret_env {
                    match secret(state, format!("mcp.{}.{name}", cfg.name)).await? {
                        Some(v) => {
                            cmd.env(name, v);
                        }
                        None => {
                            return Err(AppError::InvalidInput(format!(
                                "secret `{name}` for MCP server `{}` is not set",
                                cfg.name
                            )))
                        }
                    }
                }
                let transport = TokioChildProcess::new(cmd).map_err(|e| {
                    AppError::Execution(format!("could not start `{command}`: {e}"))
                })?;
                let svc = ().serve(transport).await.map_err(|e| {
                    AppError::Unavailable(format!(
                        "MCP server `{}` failed to initialise: {e}",
                        cfg.name
                    ))
                });
                state
                    .audit
                    .record(AuditRecord {
                        decision: if svc.is_ok() {
                            Decision::Allowed
                        } else {
                            Decision::Failed
                        },
                        confirmation: c,
                        detail: svc.as_ref().err().map(ToString::to_string),
                        ..base
                    })
                    .await?;
                svc?
            }
            McpTransport::Http { url, bearer_token } => {
                crate::ai::endpoint::ensure_endpoint_allowed(url, settings.security.local_only)
                    .await?;
                let mut config = StreamableHttpClientTransportConfig::with_uri(url.as_str());
                if *bearer_token {
                    let token = secret(state, format!("mcp.{}.token", cfg.name))
                        .await?
                        .ok_or_else(|| {
                            AppError::InvalidInput(format!(
                                "token for MCP server `{}` is not set",
                                cfg.name
                            ))
                        })?;
                    config = config.auth_header(token);
                }
                let transport =
                    StreamableHttpClientTransport::with_client(state.http.clone(), config);
                ().serve(transport).await.map_err(|e| {
                    AppError::Unavailable(format!("MCP server `{}` at {url}: {e}", cfg.name))
                })?
            }
        };
        let tools = tokio::time::timeout(Duration::from_secs(30), service.peer().list_all_tools())
            .await
            .map_err(|_| {
                AppError::Unavailable(format!(
                    "MCP server `{}` did not list tools in time",
                    cfg.name
                ))
            })?
            .map_err(|e| AppError::Unavailable(format!("MCP server `{}`: {e}", cfg.name)))?;
        conns.insert(
            cfg.name.clone(),
            Connection {
                fingerprint: fp,
                service,
                tools,
            },
        );
        Ok(())
    }

    /// Connect to one server and report its tools (Settings "Test").
    pub async fn test<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        state: &AppState,
        name: &str,
    ) -> AppResult<ServerStatus> {
        let settings = state.settings.read().await.clone();
        let cfg = settings
            .mcp
            .servers
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| {
                AppError::InvalidInput(format!(
                    "no MCP server named `{name}` (save settings first)"
                ))
            })?;
        self.connect(app, state, cfg).await?;
        let conns = self.conns.lock().await;
        let tools = conns
            .get(name)
            .map(|c| c.tools.iter().map(|t| t.name.to_string()).collect())
            .unwrap_or_default();
        Ok(ServerStatus {
            name: name.to_string(),
            tools,
        })
    }

    /// Tool specs for all enabled servers. Servers that fail to connect are
    /// skipped and reported in the returned warnings.
    pub async fn tool_specs<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        state: &AppState,
    ) -> (Vec<ToolSpec>, Vec<String>) {
        let settings = state.settings.read().await.clone();
        let mut specs = Vec::new();
        let mut warnings = Vec::new();
        for cfg in settings.mcp.servers.iter().filter(|s| s.enabled) {
            if let Err(e) = self.connect(app, state, cfg).await {
                warnings.push(format!("MCP server `{}` unavailable: {e}", cfg.name));
                continue;
            }
            let conns = self.conns.lock().await;
            if let Some(c) = conns.get(&cfg.name) {
                for t in &c.tools {
                    specs.push(ToolSpec {
                        name: namespaced(&cfg.name, &t.name),
                        description: format!(
                            "[MCP server {}] {}",
                            cfg.name,
                            t.description.as_deref().unwrap_or("(no description)")
                        ),
                        parameters: Value::Object((*t.input_schema).clone()),
                    });
                }
            }
        }
        (specs, warnings)
    }

    /// Call a namespaced tool with policy, confirmation and audit.
    pub async fn call<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        state: &AppState,
        namespaced_name: &str,
        arguments: &Value,
        source: Source,
    ) -> AppResult<(String, bool)> {
        let settings = state.settings.read().await.clone();
        // Resolve the namespaced name back to (server, original tool name).
        let (cfg, tool_name) = {
            let conns = self.conns.lock().await;
            settings
                .mcp
                .servers
                .iter()
                .filter(|s| s.enabled)
                .find_map(|s| {
                    conns.get(&s.name).and_then(|c| {
                        c.tools
                            .iter()
                            .find(|t| namespaced(&s.name, &t.name) == namespaced_name)
                            .map(|t| (s.clone(), t.name.to_string()))
                    })
                })
                .ok_or_else(|| {
                    AppError::InvalidInput(format!("unknown MCP tool `{namespaced_name}`"))
                })?
        };
        let args_obj = match arguments {
            Value::Object(m) => m.clone(),
            _ => {
                return Err(AppError::InvalidInput(
                    "MCP tool arguments must be an object".into(),
                ))
            }
        };
        let args_text = serde_json::to_string_pretty(arguments).unwrap_or_default();
        let tier = if cfg.read_only_tools.iter().any(|t| t == &tool_name) {
            RiskTier::ReadOnly
        } else {
            RiskTier::Mutating
        };
        let base = AuditRecord {
            id: uuid::Uuid::new_v4().to_string(),
            source,
            action: "mcp_call".into(),
            command: format!(
                "{}/{tool_name} {}",
                cfg.name,
                serde_json::to_string(arguments).unwrap_or_default()
            ),
            cwd: None,
            tier,
            decision: Decision::Denied,
            confirmation: Confirmation::NotRequired,
            exit_code: None,
            duration_ms: None,
            detail: None,
        };
        // Same rule as commands: Mutating always, ReadOnly when configured.
        let mut confirmation = Confirmation::NotRequired;
        if tier == RiskTier::Mutating || settings.security.require_confirmation {
            let shown: String = args_text.chars().take(1500).collect();
            confirmation = confirm::ask(
                app,
                &ConfirmRequest {
                    title: "Run MCP tool?".into(),
                    subject: format!(
                        "Server: {}\nTool: {tool_name}\nArguments:\n{shown}",
                        cfg.name
                    ),
                    details: vec![],
                    tier,
                    source,
                    reasons: vec![if tier == RiskTier::Mutating {
                        "MCP tools may change data in external systems".into()
                    } else {
                        "read confirmation is enabled in Settings → Security".into()
                    }],
                    approve_label: "Run".into(),
                },
                Duration::from_secs(settings.security.confirmation_timeout_secs),
            )
            .await;
            if confirmation != Confirmation::Approved {
                state
                    .audit
                    .record(AuditRecord {
                        decision: Decision::NotApproved,
                        confirmation,
                        ..base
                    })
                    .await?;
                return Err(AppError::NotApproved(
                    "MCP tool call was not approved".into(),
                ));
            }
        }
        let started = std::time::Instant::now();
        let result = {
            let conns = self.conns.lock().await;
            let conn = conns.get(&cfg.name).ok_or_else(|| {
                AppError::Unavailable(format!("MCP server `{}` disconnected", cfg.name))
            })?;
            let params = CallToolRequestParams::new(tool_name.clone()).with_arguments(args_obj);
            tokio::time::timeout(
                Duration::from_secs(settings.security.command_timeout_secs),
                conn.service.call_tool(params),
            )
            .await
        };
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let outcome: AppResult<(String, bool)> = match result {
            Err(_) => Err(AppError::Execution("MCP tool call timed out".into())),
            Ok(Err(e)) => Err(AppError::Unavailable(format!("MCP tool call failed: {e}"))),
            Ok(Ok(r)) => {
                let mut text: Vec<String> = r
                    .content
                    .iter()
                    .filter_map(|c| c.as_text().map(|t| t.text.clone()))
                    .collect();
                if let Some(sc) = &r.structured_content {
                    text.push(sc.to_string());
                }
                if text.is_empty() && !r.content.is_empty() {
                    text.push("(non-text content omitted)".into());
                }
                Ok((text.join("\n"), r.is_error.unwrap_or(false)))
            }
        };
        state
            .audit
            .record(AuditRecord {
                decision: match &outcome {
                    Ok((_, false)) => Decision::Allowed,
                    _ => Decision::Failed,
                },
                confirmation,
                duration_ms: Some(duration_ms),
                detail: outcome.as_ref().err().map(ToString::to_string),
                ..base
            })
            .await?;
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespacing_is_provider_safe() {
        assert_eq!(
            namespaced("github", "create_issue"),
            "mcp__github__create_issue"
        );
        assert_eq!(namespaced("fs", "read.file/x"), "mcp__fs__read_file_x");
        assert!(namespaced("a", &"x".repeat(100)).len() <= 64);
    }

    #[test]
    fn command_display_quotes_arguments() {
        assert_eq!(
            display_command(
                "npx",
                &["-y".into(), "@scope/server".into(), "my dir".into()]
            ),
            "npx -y @scope/server 'my dir'"
        );
    }

    #[test]
    fn fingerprint_changes_with_transport() {
        let mut a = McpServerConfig {
            name: "x".into(),
            enabled: true,
            transport: McpTransport::Http {
                url: "http://localhost:1".into(),
                bearer_token: false,
            },
            read_only_tools: vec![],
        };
        let f1 = fingerprint(&a);
        a.read_only_tools.push("t".into());
        assert_eq!(
            f1,
            fingerprint(&a),
            "read-only list does not require reconnect"
        );
        a.transport = McpTransport::Http {
            url: "http://localhost:2".into(),
            bearer_token: false,
        };
        assert_ne!(f1, fingerprint(&a));
    }
}

/// End-to-end check of the rmcp client plumbing (stdio transport, tool
/// listing, tool call) against a tiny Python MCP server fixture.
#[cfg(all(test, unix))]
mod stdio_e2e {
    use super::*;

    #[tokio::test]
    async fn lists_and_calls_tools_over_stdio() {
        if crate::security::elevation::find_in_path("python3").is_none() {
            eprintln!("python3 not available; skipping");
            return;
        }
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/echo_mcp_server.py"
        );
        let mut cmd = tokio::process::Command::new("python3");
        cmd.arg(fixture).env_clear().envs(executor::safe_env());
        let transport = TokioChildProcess::new(cmd).expect("spawn");
        let service = ().serve(transport).await.expect("initialize");
        let tools = service.peer().list_all_tools().await.expect("list");
        assert_eq!(tools.len(), 1);
        assert_eq!(namespaced("echo", &tools[0].name), "mcp__echo__echo");
        let mut args = serde_json::Map::new();
        args.insert("text".into(), Value::String("hi".into()));
        let r = service
            .call_tool(CallToolRequestParams::new("echo").with_arguments(args))
            .await
            .expect("call");
        assert_eq!(
            r.content[0].as_text().map(|t| t.text.as_str()),
            Some("echo: hi")
        );
        let _ = service.cancel().await;
    }
}
