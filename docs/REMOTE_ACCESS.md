# Remote access from a phone: design (proposed)

Status: **proposal, nothing built yet.** This document is the design to review before any code is written. When a
phase ships, the parts it implements move into `SECURITY.md`, `TECHNICAL_REFERENCE.md` and `USER_GUIDE.md`.

## 1. Goal

Use OMNIX from a phone with the same models, memory, tools, voice and host control as at the desk. The phone is a
thin client: all inference, memory, tool execution and auditing stay on the OMNIX machine. Nothing is reduced on
mobile except one deliberate thing in the first version: **actions that need approval are approved at the desk**
(§6).

Non-goals: access from the public internet, running models on the phone, cloud relays, multi-user accounts.

## 2. Decisions and what was rejected

| Area | Chosen | Rejected, and why |
|---|---|---|
| Network | **Tailscale** tailnet only, HTTPS via `tailscale serve` (MagicDNS name, real certificate) | Open ports / port forwarding (public exposure); `tailscale funnel` (public by design, the gateway refuses to run while it is on) |
| Gateway | **Built into the Rust backend** (`src-tauri/src/remote/`, `axum`) calling the same `agent::run_turn`, policy, confirm and audit code | A Python/FastAPI wrapper: second implementation of the trust boundary that would bypass policy, native confirmation and the audit chain |
| Transport | One **WebSocket** per phone session for chat events; small HTTPS endpoints for uploads and voice | NATS/JetStream: an extra server for one client, and at-least-once delivery can run a command twice |
| Device auth | **QR pairing** + a device key pair held in the phone's hardware keystore; challenge-response when the socket opens | mTLS + per-request signatures + rotating JWTs stacked together: WireGuard already encrypts and identifies the device; three layers add bugs, not safety |
| Client | **Tauri 2 mobile** (Android first) built from the existing SvelteKit UI | Flutter: a second UI codebase in another language |
| Notifications | Self-hosted **ntfy** on the tailnet, content-free payloads (phase 4) | FCM/APNs with content: message text would leave the local network |
| Capabilities | The phone sends **chat and a fixed set of requests**; OMNIX decides what runs | A remote terminal, or the client choosing `allowed_tools`: breaks invariant 1 (the client is untrusted) |

## 3. Architecture

```
Phone (Tauri Android app, SvelteKit UI)
  │  WireGuard (Tailscale), HTTPS/WSS to https://<host>.<tailnet>.ts.net
  ▼
tailscale serve :443  ──►  127.0.0.1:8765  OMNIX remote gateway (inside the desktop app)
                                  │
                                  ├─ agent::run_turn (same loop, same tools, emit → WebSocket)
                                  ├─ security::policy → confirm (native, at the desk) → executor → audit
                                  ├─ voice.rs (Speaches STT, Piper TTS)
                                  └─ memory (kb-core), system status (read-only)
```

- The gateway listens on **loopback only**. `tailscale serve` terminates TLS and forwards tailnet traffic to it, so
  nothing listens on the LAN or a public interface. The gateway refuses to start if it is asked to bind anything
  but `127.0.0.1`, and `doctor.sh` fails if `tailscale funnel` is on.
- The desktop app must be running (it already has tray and autostart). A headless, services-only machine is not a
  target for the first version, because the agent loop lives in the app.
- Off by default. Turning it on is a security-weakening settings change and so needs a native confirmation, like
  turning off `local_only`.

## 4. Pairing and authentication

**Pairing (once per phone)**

1. Settings → Remote → *Pair a phone*. The desktop shows a QR code: gateway URL + a one-time code (128-bit, single
   use, expires after 5 minutes).
2. The phone creates a P-256 key pair in the Android Keystore (StrongBox if present, user-authentication-bound for
   approvals in phase 3). The private key never leaves the hardware.
3. The phone sends `POST /v1/pair {code, device_name, platform, public_key}`.
4. The desktop shows a **native** dialog: *Pair "Pixel 8" (Android) with OMNIX?* Default deny.
5. On approval the device (id, name, public key, paired-at) is stored in `~/.config/omnix/remote.json`, mode 600,
   protected from commands like `ops.json`. Public keys are not secrets; nothing secret is stored.

**Session**

1. The phone opens `wss://…/v1/session`. The server sends `{"type":"hello","nonce":"<32 random bytes, b64>"}`.
2. The phone answers `{"type":"auth","device_id":"…","sig":"<ECDSA over 'omnix-remote-v1' ‖ nonce ‖ device_id>"}`
   within 10 s.
3. The server verifies against the stored key. Anything else closes the socket. There is no bearer token to steal
   for the socket itself.
4. Over the authenticated socket the server issues an **upload token** (256-bit random, memory-only, 15 minutes,
   bound to the device) for the HTTPS endpoints. It is renewed over the socket.

**Revocation**: Settings → Remote → device → *Revoke* removes the key and closes that device's open sockets at
once. Turning the gateway off closes all of them.

## 5. API (v1)

Every message carries `"v": 1`. Unknown types are rejected; there is no generic "invoke any command" message.

### WebSocket `/v1/session`

Client → server

| type | fields | notes |
|---|---|---|
| `auth` | `device_id`, `sig` | first message only |
| `chat.send` | `id`, `text` (≤ 100 000 chars), `attachments?` (upload ids) | rejected with kind `busy` while a turn runs |
| `chat.cancel` | `id` | same as the desktop Stop button |
| `chat.reset` | `id` | new conversation |
| `memory.feedback` | `id`, `memory_id`, `wrong: true` | goes through the existing `clean_memory_patch`-style whitelist |
| `status.get` | `id` | read-only host and model summary |
| `ping` | | keep-alive |

Server → client

| type | fields | notes |
|---|---|---|
| `hello` | `nonce` | before auth |
| `ready` | `device_name`, `upload_token`, `expires_at` | after auth |
| `event` | `turn`, `event` | `event` is the existing `UiEvent` unchanged: `token`, `tool_call`, `tool_result`, `recalled`, `notice`, `error`, `done` |
| `ack` | `id` | request accepted |
| `error` | `id?`, `kind`, `message` | `kind` is the existing `AppError` kind |
| `status` | `…` | answer to `status.get` |
| `token` | `upload_token`, `expires_at` | renewal |

Example turn:

```json
→ {"v":1,"type":"chat.send","id":"c-12","text":"What's using the GPU?"}
← {"v":1,"type":"ack","id":"c-12"}
← {"v":1,"type":"event","turn":"t-5","event":{"type":"tool_call","id":"1","name":"host_status","arguments":{}}}
← {"v":1,"type":"event","turn":"t-5","event":{"type":"tool_result","id":"1","name":"host_status","ok":true,"summary":"…"}}
← {"v":1,"type":"event","turn":"t-5","event":{"type":"token","text":"Ollama has qwen3…"}}
← {"v":1,"type":"event","turn":"t-5","event":{"type":"done"}}
```

### HTTPS (header `Authorization: Bearer <upload_token>`)

| endpoint | purpose | limits |
|---|---|---|
| `POST /v1/pair` | pairing (one-time code instead of a token) | 5 attempts per code |
| `POST /v1/uploads` | photo or file for the next message → `{upload_id, mime, bytes}` | 20 MB; MIME sniffed; images only in v1; deleted after 24 h |
| `POST /v1/voice/transcribe` | WAV from the phone mic → text (existing `voice.rs` → Speaches) | 60 s of audio |
| `POST /v1/voice/speak` | text → WAV (Piper) | 5 000 chars |

Responses never contain server paths.

### Images

Attachments need image input in the provider layer (Ollama's `images` field) and a model that reports the
`vision` capability through `/api/show`. The capability is discovered at runtime (invariant 9); if the chat model
lacks it the reply says so rather than ignoring the photo.

## 6. Approvals

Everything the phone triggers goes through the same policy as a desktop request. Audit entries gain
`origin: "remote:<device name>"`, and native dialogs say *Requested from: Pixel 8 (remote)*.

**Phase 1–2 (desk only).** When a remote turn hits a tool that needs confirmation, OMNIX does not raise a dialog
that nobody is there to answer. The tool returns *"needs approval at the desktop"* at once, the request is listed
under **Pending approvals** in the desktop UI, and the phone shows it as waiting. Clicking it at the desk raises the
normal native dialog, built from the parsed request.

**Phase 3 (approve on the phone), only after a `SECURITY.md` review.**

- The server sends `approval.request {request_id, title, subject, details, tier, reasons, expires_at}`, text built
  server-side from the parsed request exactly as `confirm.rs` builds dialog text.
- The phone shows it and, after a biometric check that unlocks the keystore key, signs
  `request_id ‖ SHA-256(canonical request) ‖ expires_at`. Approvals are single use and expire after 60 s.
- Only the ordinary *confirm* tier can be approved remotely. **Privileged (sudo/pkexec), elevation, `Denied`
  rules, security-weakening settings, pairing and unpairing are desk-only forever.**
- Audited as a new `Confirmation::RemoteApproved { device }`, with a `policy.rs` test for every rule.

## 7. Threats

| Threat | Mitigation |
|---|---|
| Stolen or borrowed unlocked phone | Chat only; approvals stay at the desk (phase 1–2) or need a biometric (phase 3); revoke from the desktop |
| Another device on the tailnet | Needs a paired key; Tailscale ACLs should also limit port 443 on the host to your own devices |
| Gateway exposed publicly | Loopback bind enforced in code; `funnel` detected and refused; doctor check |
| Replay | Nonce per socket; approvals single use with expiry |
| Prompt injection via phone input or a photo | Same as desktop: model output is data, tools go through policy, tool results wrapped as untrusted |
| Malicious upload | Size cap, MIME sniffing, stored outside any indexed folder, not decoded by OMNIX, deleted after 24 h |
| Flooding | One turn at a time per device, message size cap, per-device rate limits |

## 8. Open questions

1. **Shared or separate conversation.** `AppState` holds one conversation. Recommendation: share it (the phone
   continues the desk chat), with a turn lock and a *"Phone is chatting"* notice on the desktop. Separate
   conversations per device need a refactor of `AppState`.
2. **Android key storage.** Tauri has no official keystore plugin; phase 2 needs a small Kotlin plugin.
3. **iOS** needs a Mac, Xcode and an Apple developer account. Later.

## 9. Roadmap

| Phase | Delivers | Main work |
|---|---|---|
| 0 | Baseline | Merge memory PR, tag a release |
| 1 | Gateway | `remote/` module (axum on loopback), pairing + native confirm, session auth, WebSocket chat reusing `run_turn`, desk-only approval queue, audit `origin`, settings page, doctor checks, tests |
| 2 | Android app | Tauri Android target; `src/lib/api.ts` gets a remote transport alongside IPC; keystore plugin; mobile layout; push-to-talk via phone mic; photo upload + vision |
| 3 | Approve on phone | `SECURITY.md` update first; approval messages, biometric-bound signatures, `Confirmation::RemoteApproved`, policy tests |
| 4 | Notifications | ntfy on the tailnet for alerts, automations and finished turns, content-free |
