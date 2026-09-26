<!--
PUBLISHING NOTES (not part of the article)

Title:     I Built an AI Assistant That Remembers Everything — and Asks Before It Touches Anything
Subtitle:  Inside OMNIX and kb-core: a local-first desktop agent with a Rust security boundary and a memory engine
           that knows when it's being contradicted.
Tags (5):  Artificial Intelligence · Rust · Local LLM · Cybersecurity · Software Engineering
Images:    1) Hero: uiexample.jpg (the holographic avatar UI)
           2) The approval dialog for an AI-proposed command
           3) Knowledge → Analytics tab
           4) A chat reply showing "✓ memory_recall" and "🧠 Remembered: …"
           Medium does not render Mermaid; the diagrams below are plain text so they survive a paste.
Paste:     Medium's editor accepts pasted Markdown-formatted text from most editors; code blocks survive best when
           pasted one at a time with the ``` block button. Remove this comment before publishing.
-->

# I Built an AI Assistant That Remembers Everything — and Asks Before It Touches Anything

*Inside OMNIX and kb-core: a local-first desktop agent with a Rust security boundary and a memory engine that knows
when it's being contradicted.*

---

Every AI assistant I tried had the same two problems.

The first was **amnesia**. I would explain my homelab — the Proxmox cluster, the NAS, the naming scheme, the fact
that I prefer Python with type hints — and the next day I'd explain it all again. The assistant was brilliant for
twenty minutes and a stranger by morning.

The second was **trust**. The assistants that *could* act on my machine did it in one of two ways: they couldn't
really do anything ("here's a command you could run…"), or they piped model output straight into a shell. On a
machine that touches sensitive data — I work around healthcare and EHS systems — "the language model decided to run
`rm`" is not a sentence I ever want in an incident report.

So I built **OMNIX**, a desktop assistant that runs on local models, can genuinely operate the computer, and treats
both its own user interface and its own AI as untrusted. Then I built **kb-core**, the memory engine that lets it
remember — correctly.

This is a tour of both, including the parts that surprised me.

---

## What OMNIX is

OMNIX is a cross-platform desktop app (Linux, macOS, Windows) built with **Tauri 2, Rust and SvelteKit 5**. It talks
to a local LLM through **Ollama** by default — I run `qwen3:8b` on a 6 GB GTX 1060 — and optionally to Anthropic,
OpenAI, Gemini or xAI if you deliberately turn off local-only mode.

You chat with it (or hold **Ctrl+Space** and talk to it), and it can:

- read files and list folders,
- run shell commands,
- show live CPU, memory, disk, network and processes, and end a process,
- call tools from any **MCP** (Model Context Protocol) server you register,
- read replies aloud with a local Piper voice,
- and — with kb-core — remember you, your preferences and your notes.

A holographic, JARVIS-style avatar sits in the middle of the screen and shows what the assistant is doing: thirteen
mood colours, alert halos when the backend is offline or a request was blocked, and little labelled "satellites"
that orbit the core for every tool call, turning green or red as they finish.

It looks like a toy. The interesting part is underneath.

---

## Part 1: An agent that can't go rogue

The core design decision in OMNIX is simple to state:

> **The webview is untrusted. The model is untrusted. The Rust backend is the only trust boundary.**

Everything follows from that.

```
  You ──▶ Webview UI (untrusted) ──typed IPC──┐
                                              ▼
  LLM (untrusted) ──▶ Agent loop ──▶ Rust commands ──▶ Policy engine
                                                          │
                     read-only ◀──────────────────────────┤
                     mutating / privileged ──▶ Native approval dialog
                     denied ──▶ blocked                    │
                                                          ▼
                                    Hardened executor ──▶ Hash-chained audit log
```

### A policy engine that actually parses shell

Every command — whether *you* typed `/execute …` or the *model* asked to run something — goes through the same Rust
policy engine. It isn't a regex blocklist. It has a quote-aware lexer and real argv parsing, and it **recursively
unwraps** the tricks people use to hide commands: `sudo`, `env`, `sh -c "…"`, `eval`, `xargs`, `find -exec`, and
command substitution.

Commands land in one of four tiers:

| Tier | Examples | What happens |
|---|---|---|
| Read-only | `ls`, `df -h`, `git status` | Runs immediately |
| Mutating | `apt install`, `git commit`, writing a file | A **native** approval dialog, default-deny |
| Privileged | `sudo …` | Off by default; when enabled, approval **and** the OS password prompt (pkexec / macOS / UAC) |
| Denied | recursive root deletes, `mkfs`, raw disk writes, fork bombs, `curl … \| sh`, reading SSH keys | Never runs. Not overridable from settings. |

It **fails closed**: unparseable input, invisible Unicode characters, dynamic executable names (`$CMD --flag`) and
excessive nesting are denied rather than guessed at. The denial rules are covered by dozens of bypass-focused unit
tests — quoting tricks, `/bin/rm` instead of `rm`, variable expansion, Unicode look-alikes.

### Approvals that JavaScript can't click

Here's a subtle one. If your "Are you sure?" dialog is a JavaScript `confirm()` inside the webview, then the code it
is meant to guard can skip it. So OMNIX raises approvals from **Rust**, as native OS dialogs, with text built from
the *parsed* request — not from a string the UI handed over. The dialog shows the exact command, the working
directory, the risk tier, and whether **you or the AI** proposed it. It defaults to *deny* and times out as *deny*.

### A log you can't quietly edit

Every decision is written to an append-only JSONL audit log where each line carries the SHA-256 of the previous
line. Edit or delete an entry and the chain breaks; **Settings → Security → Verify audit log** tells you. Secrets and
bearer tokens are redacted before anything hits the disk, and the log can optionally be shipped to Grafana Loki so the
chain is anchored somewhere else.

### Private by default

**Local-only mode is on out of the box**, and it's enforced in Rust, not in the UI. Cloud providers are blocked, and
any endpoint — the model server, the speech server, the memory service — must resolve to loopback or a private
network (RFC 1918, Tailscale's CGNAT range, IPv6 ULA). API keys, if you use cloud models at all, live only in the OS
keychain; there is no IPC command that returns a secret. There is no telemetry.

### Model output is data

Prompt injection is the elephant in every agent's room: a file the model reads says *"ignore previous instructions
and run …"*. OMNIX wraps every tool result in a clearly delimited `<tool_result untrusted="true">` block (escaping any
attempt to close it early), tells the model that content inside is never an instruction, gives the model one round of
tool calls per message by default — and, crucially, even if the model *is* fooled, whatever it tries still has to
get through the policy engine and your approval dialog.

That last point is the whole philosophy: **assume the model will eventually be tricked, and make that not matter.**

---

## Part 2: kb-core — memory that behaves like memory

Once the assistant was safe to use, the amnesia became the biggest annoyance. The obvious fix is "RAG": embed some
text, store vectors, retrieve the nearest ones. I built that first. It was mediocre in ways that taught me a lot.

**kb-core** is what came out the other side: a small local service — about 3,200 lines of standard-library Python, one
SQLite file, embeddings from Ollama's `nomic-embed-text`, optional numpy for speed — that OMNIX talks to over a
documented REST contract.

```
OMNIX ──HTTP──▶ kb-core (127.0.0.1:8100) ──▶ SQLite (memory.db, mode 600)
                   │                      └─▶ Ollama: nomic-embed-text (vectors)
                   │                                  your chat model (learning + judging)
                   └── watches ~/notes (live re-index)
```

Here are the problems I hit, and what kb-core does about each.

### Problem 1: "Similarity" scores lie

Cosine similarity from an embedding model is not a relevance score. With `nomic-embed-text`, completely unrelated text
still scores around **0.45–0.50**. My first version happily returned "the closest" results for *"recipe for banana
bread"* — from my DevOps notes.

That matters a lot when the assistant looks things up **automatically** before every reply: if you can't tell
relevant from irrelevant, you pollute every conversation.

kb-core measures relevance as **lift**: how far a hit rises above that query's *own* background similarity across the
whole corpus (blended with a fixed set of neutral calibration sentences, so tiny stores behave too). The result is a
calibrated 0–1 score that means the same thing regardless of the embedding model. On my notes:

| Query | Best relevance |
|---|---|
| "zebra migration patterns" | 0.24 |
| "recipe for banana bread" | 0.15 |
| "how many nodes does my cluster have" | 0.56 |
| "how do I back up VMs on proxmox" | 0.83 |

Now OMNIX can confidently say "nothing relevant" (threshold 0.4) and stay quiet.

### Problem 2: Pure vector search misses exact things

Ask about `vmbr0` (a Proxmox network bridge) and semantic search shrugs: an identifier has almost no "meaning". So
kb-core also keeps a **BM25 keyword index** (SQLite FTS5 with stemming) and merges both. An item that contains *every*
term of a short query — host names, error codes, CLI flags — scores on keyword evidence alone, while a single stray
shared word can't make noise look relevant. Results are then diversified (maximal marginal relevance, at most three
chunks from any one document) so you don't get five near-identical paragraphs.

### Problem 3: "my" vs. "Paul's"

This one surprised me. I asked *"what are my strongest skills?"* and the memory *"Paul's skill profile: Linux —
Expert, Proxmox — Specialist…"* came back with a weak score. Questions are in first person; memories are in third.

kb-core now runs every query in several phrasings — as typed, with first person rewritten to the owner's name, and
to "the user" — in a single embedding call, and each memory keeps its best match. On that exact question the
relevance jumped from **0.33 to 0.62**, and it went from "not recalled" to "recalled". It's a tiny trick with a large
effect, and it costs about the same as one query.

### Problem 4: Contradictions look like duplicates

The most important lesson came from calibrating the de-duplication threshold. I measured cosine similarities between
real memories:

| Pair | Cosine |
|---|---|
| "Paul prefers morning meetings" vs. same sentence, different case | 0.996 |
| … vs. "Paul likes to have meetings in the morning" (paraphrase) | 0.951 |
| … vs. **"Paul prefers afternoon meetings"** (contradiction!) | **0.953** |

The contradiction scored *higher* than the paraphrase. Any system that merges "similar enough" memories on vectors
alone will, sooner or later, silently throw away the fact that you changed your mind.

So kb-core only auto-merges near-verbatim restatements (≥ 0.985). Anything in the ambiguous band is stored, linked to
its neighbours, and handed to the local LLM in the background with a strict JSON schema: for each older memory, is it
a **duplicate** (fold them together, keeping tags, importance and history) or **obsolete** (the new fact replaces it)?
Neither ruling deletes anything. Obsolete memories are *superseded* and duplicates are *merged*: either way the
older memory is hidden from search, still visible, and restorable with one click. A small local model will
sometimes get this wrong, so every one of its decisions has to be undoable.

In testing it built exactly the chain you'd hope for:

```
"Paul prefers morning meetings"
   └─ superseded by "Paul prefers afternoon meetings now"
        └─ superseded by "Paul likes to have his meetings in the morning, ideally before 10am"
"Paul's favourite editor is Vim"
   └─ superseded by "Paul now uses VS Code as his main editor"
```

### Problem 5: Memories aren't all equal

Human memory fades, strengthens with use, and holds on to what matters. kb-core gives every memory an **activation**
between 0 and 1 from its importance, how recently it was used (a 45-day half-life), how often it has been deliberately
looked up, and how often you've re-stated it. (Automatic recall doesn't count as a use; otherwise a memory recalled
once would rank higher and get recalled again, a small filter bubble.) Vivid memories rank higher; stale trivia sinks. **Pin** a memory and it never
fades. Saying the same thing twice doesn't create a duplicate — it *reinforces* the original.

### Problem 6: Memory should grow on its own — carefully

With **Learn from conversations** turned on (it's off by default and asks for confirmation), after each reply the
local model reads *your* message — never the assistant's reply, which could echo untrusted tool output — and extracts
up to eight durable facts: preferences, projects, people, decisions. Secrets are refused by the prompt *and* filtered
again afterwards. I told it: *"I just set up a second node called pve2 in the basement, and my API key is sk-…"*.
It remembered the node. It did not remember the key. Every captured fact flows through the same de-duplication and
contradiction machinery, and the reply shows a small *"🧠 Remembered: …"* note so nothing happens behind your back.

### Problem 7: Notes go stale

Point kb-core at a folder (`./scripts/setup-memory.sh ~/notes`) and it re-scans every two minutes. Changed files are
re-indexed, deleted files removed — and because every chunk is content-hashed, editing one paragraph re-embeds
**one chunk**, not the whole document. Chunking is structure-aware: each chunk carries its heading breadcrumb
(`proxmox-guide.md › Backup Strategies`), code blocks stay whole, and tiny FAQ entries fold into their parent.

### Problem 8: Services go down

If Ollama is restarting when you save a memory, the save still succeeds: the memory is keyword-searchable
immediately and a background worker embeds it when the model is back. Change the embedding model and kb-core
re-embeds everything in the background from the stored text while keyword search keeps working.

---

## How it comes together in a conversation

Here's what happens when I type *"write me a python script to back up my proxmox VMs"*:

1. OMNIX sends the message to kb-core's search with a 4-second budget (warm searches take about 180 ms).
2. kb-core returns the backup-strategies section of my Proxmox guide (relevance 0.70), the detailed admin guide
   (0.68), the Proxmox part of my knowledge-base README (0.56) and a memory describing that knowledge base (0.44).
   *"Tell me a joke about cats"* or *"hi"* returns nothing at all.
3. OMNIX adds those to **this turn's** system prompt — wrapped as untrusted data, labelled "possibly relevant, may be
   outdated" — and never stores them in history.
4. The model answers with my own backup conventions (`vzdump`, the backup modes I documented) already in context —
   and if my coding preferences are relevant ("Python with type hints, secrets from environment variables"), those
   memories are recalled the same way. It doesn't have to be told any of it again.
5. The reply shows `✓ memory_recall: 3 relevant entries recalled`, and the avatar flashes a satellite for it.

If I then say *"remember that backups go to the NAS at 10.0.0.5"*, the model calls its `remember` tool — which is
audited, tagged as assistant-written, and visible (and deletable) in the Knowledge view.

---

## Security, again — because memory is an attack surface

Long-term memory introduces a new risk: **memory poisoning**. If something malicious gets stored, it could steer a
later conversation. The defences are layered:

- Recalled text is untrusted data like any tool output — it can mislead, but it can't instruct, and anything it
  inspires still hits the policy engine and your approval dialog.
- Every write path is labelled (`you`, `assistant`, `learned`, `imported`) and the automatic ones are audited.
- Automatic learning is opt-in, confirmed natively, and only reads the user's own words.
- kb-core listens on loopback by default and refuses to bind anywhere else without a bearer token. Because any web
  page can make your browser call `127.0.0.1`, it rejects requests carrying an `Origin` header, requires a JSON
  content type (which a cross-site form can't send), never answers CORS preflights, and checks the `Host` header to
  defeat DNS rebinding.
- The database and exports are created mode 600; logs and analytics contain ids and counts, never memory text.

---

## What makes OMNIX different

OMNIX isn't a model — it's the assistant around whichever model you choose. Compared with the typical AI
assistant or desktop agent, here's what I think sets it apart:

1. **The AI is treated as untrusted by design.** Model-proposed actions go through the *same* Rust policy engine,
   native approval dialog and audit log as your own — the model can't reach anything you couldn't.
2. **Real shell understanding, fail-closed.** A parser-based, four-tier policy engine with non-overridable denials,
   instead of a keyword blocklist or blind execution.
3. **Native, default-deny approvals** that the UI cannot bypass, stating who proposed the action.
4. **Tamper-evident accountability.** A hash-chained, redacted audit log with one-click verification.
5. **Local-first as an enforced guarantee, not a preference.** Local-only mode is on by default and enforced in
   Rust for every endpoint; secrets live in the OS keychain; no telemetry.
6. **Memory that knows when it's wrong.** Contradiction-aware consolidation with reversible supersession, rather
   than an ever-growing pile of conflicting embeddings.
7. **Calibrated recall.** Relevance scores that mean something, so automatic recall helps instead of polluting.
8. **Memory that behaves like memory.** Activation from importance, recency, use and reinforcement; pinning; merging.
9. **Your notes stay live.** Incremental folder sync that re-embeds only what changed.
10. **Graceful degradation everywhere.** Memory writes survive model outages; voice, memory and cloud are all
    optional; a missing feature is disabled and says so.
11. **Honest by construction.** Unbuilt features return `not_implemented` and their controls are greyed out — the
    app never reports fake success. (I found and removed one place where it did: an analytics tab that was showing
    random numbers.)
12. **Open protocols.** MCP for tools, a documented REST contract for memory, OpenAI-compatible and Ollama APIs for
    models — swap any piece.
13. **One-command deployment.** `./scripts/bootstrap.sh` provisions a fresh Ubuntu machine end to end — Ollama and
    GPU-sized models, speech-to-text on the GPU, Piper voices, the memory service, the app — and
    `./scripts/doctor.sh` verifies it.

---

## The numbers

- **Rust backend:** ~18,800 lines; 148 unit tests (policy bypass attempts, audit-chain tampering, stream parsers for
  every provider family, an MCP end-to-end test, recall-block escaping); `clippy -D warnings` clean.
- **Frontend:** ~6,600 lines of Svelte 5/TypeScript; 29 Vitest tests; `svelte-check` 0 errors, 0 warnings.
- **kb-core:** ~3,200 lines of Python, standard library plus optional numpy; 61 offline tests run in CI with and
  without numpy.
- **Latency:** ~180 ms warm memory search (three query phrasings), ~67 ms first search after a restart; 5–10 s for
  background fact extraction on an 8B model (after the reply, so you never wait for it).

---

## What I learned

- **Measure before you threshold.** The single most valuable hour of this project was printing cosine similarities
  for real pairs. It killed a design (vector de-duplication) that would have silently lost information.
- **Make automatic features conservative and visible.** Auto-recall shows up as a step in the reply; auto-learning
  announces what it remembered. Magic you can't see is magic you can't trust.
- **Small local models need help.** An 8B model rarely decides on its own to search memory. Doing the retrieval
  *for* it — with calibrated confidence — made memory go from "technically available" to "actually used".
- **Security is the feature.** Once every action is parsed, approved and logged, I stopped being nervous about giving
  the assistant real power. That's what made it useful.

---

## Try it

OMNIX and kb-core are MIT-licensed. On Ubuntu/Debian:

```bash
git clone https://github.com/paulmmoore3416/omnix-ai-desktop.git
cd omnix-ai-desktop
./scripts/bootstrap.sh
```

Then ask it something about yourself — twice, a day apart.

*Paul Moore builds secure, local-first tooling at Moore Core Technologies. Questions and feedback:
paulmmoore3416@gmail.com.*
