# OMNIX User Guide

OMNIX is an AI assistant that lives on your desktop. You can chat with it, talk to it, and ask it to check on
or work with your computer. It never changes anything on your system without asking you first.

This guide takes about 10 minutes to read. You don't need to be technical.

---

## Contents

1. [First-time setup (5 minutes)](#1-first-time-setup-5-minutes)
2. [A quick tour of the screen](#2-a-quick-tour-of-the-screen)
3. [Chatting with OMNIX](#3-chatting-with-omnix)
4. [Talking to OMNIX (voice)](#4-talking-to-omnix-voice)
5. [Commands: getting things done](#5-commands-getting-things-done)
6. [Approvals: you stay in control](#6-approvals-you-stay-in-control)
7. [Meet the avatar](#7-meet-the-avatar)
8. [Settings, section by section](#8-settings-section-by-section)
9. [Troubleshooting](#9-troubleshooting)
10. [Keyboard shortcuts](#10-keyboard-shortcuts)
11. [The Knowledge view, tab by tab](#11-the-knowledge-view-tab-by-tab)
12. [Getting the most out of memory](#12-getting-the-most-out-of-memory)
13. [Your data and privacy (FAQ)](#13-your-data-and-privacy-faq)
14. [System Control: watch, automate and maintain your computer](#14-system-control-watch-automate-and-maintain-your-computer)

---

## 1. First-time setup (5 minutes)

**The easy way (Linux):** from the OMNIX folder run `./scripts/bootstrap.sh`. It installs everything (Ollama and a
model, the speech-to-text server, Piper read-aloud), fills in these settings for you, and installs the app. When it
finishes, open OMNIX and start chatting. `./scripts/doctor.sh` checks that everything is still healthy.

**By hand:** you need OMNIX installed, plus [Ollama](https://ollama.com), the free program that runs the AI model on
your own computer.

1. **Install a model.** Open a terminal and run:
   ```bash
   ollama pull qwen3:8b
   ```
2. **Open OMNIX** and click **⚙️ Settings → AI Models**.
3. **Pick your model** from the dropdown. OMNIX lists every model Ollama has installed. Click **Test** to confirm it answers.
4. Click **Save Settings**.

That's all you need to start. Voice, memory, and cloud models are optional extras covered below.

> 🔒 **Private by default.** OMNIX starts in *local-only mode*. Your conversations stay on your computer or your private
> network, and cloud AI services are blocked until you deliberately turn this off.

---

## 2. A quick tour of the screen

```
┌───────────────┬──────────────────────────────────────────────┐
│  OMNIX        │                                              │
│               │         [ holographic avatar ]               │
│  🏠 Home       │                                              │
│  ⚡ Commands   │    📊 System Status  📁 List Files  ❓ Help    │
│  📜 History    │                                              │
│  ⚙️ Settings   │    [ Commands ]  [ Processes ]  [ Uptime ]    │
│  🧠 Knowledge  │                                              │
│  🎛️ System     │                                              │
│               ├──────────────────────────────────────────────┤
│  CPU ▓▓░░ 23% │ 🎤 │ Ask OMNIX anything…            │ Send ➜ │
│  RAM ▓▓▓░ 61% │                                              │
└───────────────┴──────────────────────────────────────────────┘
```

| Area | What it's for |
|---|---|
| **Home** | The avatar, quick-action buttons, and live stats |
| **Commands** | A cheat sheet of the `/` commands |
| **History** | The full conversation, with replies as they stream in |
| **Settings** | Everything configurable (see [section 8](#8-settings-section-by-section)) |
| **Knowledge** | Long-term memory: save and search memories, index documents (needs the memory service; see §3) |
| **System Control** | Live CPU, memory, GPUs, disks and network; processes, services and Docker containers; AI models; alerts, automations and scheduled tasks; cleanup and optimization (see [§14](#14-system-control-watch-automate-and-maintain-your-computer)) |
| **Input bar** | 🎤 microphone, text box, and Send |

OMNIX also sits in your **system tray**. Closing the window hides it there instead of quitting, so it's always one click away.

---

## 3. Chatting with OMNIX

Type in the box at the bottom and press **Enter** (or click **Send**).

- Replies **stream in live** on the History screen.
- Ask naturally: *"What's using the most memory right now?"* or *"Summarize the file ~/notes/meeting.md"*.
- OMNIX can **use tools** to answer: list folders, read files, run safe commands. Each tool it uses shows as a
  small note under the reply (🔧 requested, ✓ done, ✗ failed).
- Click **⏹ Stop** to cut a long answer short.
- Click **🧹 Clear** on the History screen to start a fresh conversation.
- If voice is set up, click **🔊 Read aloud** under any reply to hear it.

---

### Long-term memory

With the memory service set up (`./scripts/setup-memory.sh`; `bootstrap.sh` does it for you), OMNIX remembers
things between conversations and knows your notes:

- **It recalls on its own.** Before answering, OMNIX looks up memories and notes related to your message. When
  something relevant turns up, the reply shows a `✓ memory_recall` note. Ask *"what editor do I use?"* or *"write a
  script to back up my Proxmox VMs"* and it already has the context.
- **Tell it when a memory was wrong.** Under a reply, open **🧠 Memories used** to see which memories shaped it.
  Press 👎 on one that was wrong or beside the point and it ranks lower from then on, so better or newer memories win
  (it isn't deleted, and it still comes back if it's the only match). 👍 says it helped. To fix the memory itself,
  edit or delete it in the Knowledge view, or just tell OMNIX the right fact.
- **Tell it to remember.** *"Remember that my NAS is at 10.0.0.5."* It saves that as a memory. Saying the same thing
  twice strengthens the memory instead of duplicating it. If you later say something that contradicts it (*"I moved
  the NAS to 10.0.0.9"*), the old memory is retired automatically. You can still see it, and delete the new one to
  bring it back.
- **Let it learn (optional).** Turn on **Settings → Memory → Learn from conversations** and, after each reply, the
  local model saves lasting facts from *your* messages (never passwords or keys). You'll see a note like
  *🧠 Remembered: Paul set up a second Proxmox node called pve2*.
- **Your notes stay current.** Folders you add with `./scripts/setup-memory.sh ~/notes` are re-scanned every two
  minutes, so edits show up without re-importing.

In the **Knowledge** view you can add, pin (📌 = never fades) or delete memories, see where each came from (✍️ you,
🤖 assistant, 🧠 learned, 📥 imported), search everything, manage indexed documents, and see what the memory engine
is doing (**Analytics**). **Export** saves everything to a file only you can read, **Import** merges one back in, and
**Optimize** tidies up duplicates. To encrypt the memory database on disk, run `./scripts/setup-memory.sh --encrypt`
once (the key is kept in your login keyring, so the memory service starts only after you've logged in and the
keyring is unlocked).

Everything stays on your computer: the memory database, the embedding model, and the learning model all run
locally.

## 4. Talking to OMNIX (voice)

Voice is **push-to-talk only**. OMNIX never listens in the background.

### Three ways to talk

| How | Do this | Best for |
|---|---|---|
| **Hold** | Press and hold 🎤, speak, let go | Quick questions |
| **Tap** | Tap 🎤 once, speak, tap again to send | Longer requests (hands free between taps) |
| **Shortcut** | Hold **Ctrl + Space** from *any* app, speak, let go | When OMNIX isn't in front |

While you're recording you'll see:
- a **red mic** with a ring that grows as you speak (if it doesn't move, the mic isn't hearing you),
- a **timer** and a small level meter next to the button,
- the avatar turning **blue**, with its waveform ring jumping along with your voice.

Press **Esc** at any time to throw the recording away. Recordings stop automatically at 2 minutes.

When you let go, OMNIX shows **Transcribing…**, turns your speech into text, and sends it just as if you'd typed it.

### Setting up voice (one time)

A **yellow dot** on the 🎤 button means voice isn't set up yet. Click the button and OMNIX takes you straight to the right screen.

> If you installed with `./scripts/bootstrap.sh`, voice is already set up (Speaches on `http://127.0.0.1:8000` and
> Piper with the `en_US-lessac-medium` voice). Skip to step 4 to test it.

1. You need a **speech-to-text server**. OMNIX works with any
   [faster-whisper](https://github.com/SYSTRAN/faster-whisper)-based server that offers the standard
   `/v1/audio/transcriptions` endpoint. It can run on this computer or another machine on your network. One option is
   [Speaches](https://github.com/speaches-ai/speaches) (formerly *faster-whisper-server*), which runs in Docker:
   ```bash
   docker run -d --name omnix-speaches -p 127.0.0.1:8000:8000 ghcr.io/speaches-ai/speaches:latest-cpu
   curl -X POST http://localhost:8000/v1/models/Systran/faster-distil-whisper-small.en   # download a model once
   ```
   With an NVIDIA GPU use the `latest-cuda` image and add `--gpus all`.
2. In OMNIX go to **Settings → Voice**:
   - Turn on **Enable voice input/output**
   - **Speech-to-text server URL:** `http://localhost:8000`
   - **Whisper model:** the model id you downloaded, e.g. `Systran/faster-distil-whisper-small.en` (fast, CPU-friendly)
     or `Systran/faster-distil-whisper-large-v3` (most accurate; what the bootstrap script uses on a GPU)
   - **Language:** pick yours
3. Click **Save Settings**.
4. Click **🎙 Test microphone** and say a short sentence. You should see **✓ Heard: "…"**.

**Optional: read-aloud.** Install [Piper](https://github.com/OHF-Voice/piper1-gpl) (`pip install piper-tts`), download a
voice (`python3 -m piper.download_voices en_US-lessac-medium`), then set **Piper program** to the full path of `piper`
and **Voice** to the **full path of the `.onnx` file** (a bare voice name won't be found).

> 🔒 Your recordings go **only** to the speech server you configured, and in local-only mode that server must be on
> your own computer or private network.

---

## 5. Commands: getting things done

Start a message with `/` to run a command directly. Type `/` and suggestions pop up.

| Command | What it does | Example |
|---|---|---|
| `/monitor` | Shows CPU, memory, and the busiest programs | `/monitor` |
| `/execute` | Runs a terminal command | `/execute df -h` |
| `/file read` | Shows a text file | `/file read ~/notes/todo.md` |
| `/file list` | Lists a folder | `/file list ~/Documents` |
| `/file write` | Creates or replaces a file (asks first) | `/file write ~/notes/idea.md Buy milk` |
| `/remember` | Saves a fact to long-term memory; `#words` at the end become tags | `/remember I prefer metric units #prefs` |
| `/recall` | Finds your memories by meaning; on its own, lists the most recent | `/recall units` |
| `/search` | Searches everything OMNIX knows: memories plus indexed notes, documents and watched folders | `/search zfs backup` |

The Home screen buttons are shortcuts: **📊 System Status** runs `/monitor`, **📁 List Files** lists the current folder,
and **❓ Help** asks OMNIX what it can do.

`/remember`, `/recall` and `/search` need the memory service (kb-core, installed by setup). Results show how
relevant each one is; weak matches are left out. Everything you save is listed, editable and deletable in the
**Knowledge** view.

---

## 6. Approvals: you stay in control

Every command, whether you typed it or the AI suggested it, is checked and sorted into one of four groups:

| Group | Examples | What happens |
|---|---|---|
| ✅ **Look-only** | `ls`, `cat`, `git status`, `df` | Runs right away |
| ⚠️ **Changes something** | installing, writing, deleting, `git commit` | A **pop-up asks you first**, showing the exact command |
| 🔐 **Needs admin** | `sudo …` | Off unless you enable it; then asks you *and* your computer's password prompt |
| ⛔ **Dangerous** | wiping a disk, deleting everything, reading your passwords/keys | **Always blocked.** Can't be overridden |

About the approval pop-up:
- It says whether **you** or **the AI** proposed the action.
- **Deny is the default.** If you walk away, it times out as *deny*.
- Every decision is written to a tamper-evident log. **Settings → Security → Verify audit log** checks nobody edited it.

---

## 7. Meet the avatar

The Home screen shows OMNIX as a holographic core: a glowing reactor surrounded by rotating rings, a radar sweep,
and a waveform ring that moves with your voice while you talk and with OMNIX's voice while it replies. The core
follows your mouse. **Colour tells you what's going on**, in three layers:

- **Mood** colours the core and inner rings: what OMNIX is doing right now.
- **Condition** colours the outer halo and shows a ⚠ banner: a problem that needs your attention.
- **Signals** are ripples that pulse out from the core: one-off events, like a tool finishing.

A mood and a condition can show at the same time. For example, an amber core inside an orange halo means
"running your task, but the computer is under heavy load".

Click **KEY** in the avatar's corner to open the colour key in the app. It highlights whatever is showing right now.

### Mood (core and inner rings)

| Colour | Mood | It means | What it looks like |
|---|---|---|---|
| 🔵 Cyan `#29d8ff` | Standby | Ready and waiting | Slow ring drift, calm core breathing, an occasional scan |
| 🔵 Blue `#4f8bff` | Listening | Recording your voice | Waveform ring jumps with your mic level |
| 🟣 Violet `#8b7bff` | Thinking | Working out an answer | Inner rings speed up, data points orbit the core |
| 🟣 Purple `#c36bff` | Processing | Transcribing speech or computing | Counter-rotating rings, fast radar sweep |
| 🟠 Amber `#ffb02e` | Executing | Running a command or tool | Outer ring steps round like a gear |
| 🟢 Aqua `#4ff5d2` | Speaking | Replying to you | Waveform ring pulses with the voice |
| ⚪ Ice white `#d6ecff` | Focused | Concentrating on a task | Rings slow and tighten |
| 🟡 Gold `#ffd84a` | Happy | Task finished, all good | Warm glow, gentle spin-up |
| 🩷 Pink `#ff5ea8` | Excited | Something went really well | Fast spin with bursts of sparks |
| 🟢 Green `#3dff95` | Success | Your request completed | Double shockwave ring |
| 🩶 Slate `#9fb4c8` | Confused | Didn't catch that, or the input was unclear | Rings wobble back and forth |
| 🔴 Red `#ff3d4f` | Error | The request failed (details in the pop-up) | Glitch shake and flicker |
| 🔵 Navy `#3553b8` | Dormant | Quiet for 75 seconds | Dims and slows right down. Move the mouse or click the core to wake it |

### Condition (outer halo and ⚠ banner)

| Colour | Condition | It means | What it looks like |
|---|---|---|---|
| ⚫ Grey `#7c8594` | Backend offline | The OMNIX core stopped responding (two missed status checks in a row) | Grey halo, stuttering flicker, **LINK: LOST** |
| 🟠 Orange `#ff7a1a` | High system load | CPU above 85% or memory above 90% | Orange halo with fast-crawling warning ticks; the CPU/MEM gauges turn orange |
| 🩷 Magenta `#ff2bd6` | Blocked by policy | A command was denied by policy, you declined an approval, or local-only mode blocked a cloud call | Magenta halo, flashing hazard chevrons (clears after 6 s) |
| 🟢 Chartreuse `#d4ff3a` | Connection issue | The AI provider, speech server, or network couldn't be reached | Broken, crawling dashed halo (clears after 6 s) |

### Signals (ripples from the core)

| Colour | Signal | It means |
|---|---|---|
| 🟠 Amber | Tool dispatched | OMNIX asked to use a tool |
| 🟢 Green | Tool succeeded | A tool call came back OK |
| 🔴 Red | Tool failed | A tool call (or the stream) reported an error |
| ⚪ White | Notice | An informational message from the agent |

### Things to try

- **Watch it boot.** When OMNIX starts, the core ignites, the rings power on from the inside out, and the HUD comes online.
- **Rest your mouse on the stage.** A targeting reticle follows the cursor. Hold still for a moment and it **locks on**,
  showing the position and range from the core.
- **Click the core** to ping it (a spin-up and a ripple).
- **Grab the rings and flick them.** Drag around the core to spin the rings; let go mid-swipe and they keep coasting.
- **Watch tools run.** When OMNIX uses a tool, a labelled **satellite** launches and orbits the core with a data link.
  It's amber while the tool runs, then turns green ✓ or red ✗ when the result comes back, and fades out.
- **Ask a question.** While OMNIX is thinking, the orbiting data points link up into a flickering neural network.
- Status text **decodes** into place whenever it changes.

The top corners show the current mode, link state, and live CPU/memory gauges.

> If your system has *Reduce motion* turned on, the boot sequence, spinning, flicking, and text scrambling are skipped. Colours,
> banners, signals, and satellites still work.

---

## 8. Settings, section by section

Click **Save Settings** after making changes. Anything that weakens security (turning off local-only mode, allowing
admin commands, adding a new server) asks you to confirm in a pop-up.

| Section | What you can do there |
|---|---|
| **General** | Theme, launch at login, export/import your settings (API keys are never exported) |
| **AI Models** | Choose Ollama or a cloud provider, pick a model, adjust creativity (temperature) and response length |
| **MemResort** | Connect the optional MemResort memory service |
| **MCP Servers** | Add extra tools for the AI through the Model Context Protocol. Each tool call is checked and logged |
| **Voice** | Speech-to-text server, Whisper model, language, Piper read-aloud, and **Test microphone** |
| **Memory** | The memory service URL, **Recall automatically** (on), how many entries to recall and how relevant they must be, **Learn from conversations** (off; asks you to confirm when you turn it on), **Archive conversations** (on: a searchable copy of your chats), **Summarize conversations** (off: when you clear a chat, save a short summary as a memory), **Semantic search** (on; off = match words only), **Memory limit** (1000; when full, new memories are refused, nothing is deleted for you) and **Keep archived conversations** (0 = forever; older chat transcripts are deleted daily) |
| **Security** | Local-only mode, admin commands, autonomous mode, blocked commands, audit-log verification |
| **Performance** | Performance-related options |

**API keys** (for cloud providers) are stored in your operating system's secure keychain, never in a plain file. The
screen shows only "Key saved ✓", with no way to read the key back out.

---

## 9. Troubleshooting

| Problem | Try this |
|---|---|
| **🎤 has a yellow dot / clicking it opens Settings** | Voice isn't set up. Follow [Setting up voice](#setting-up-voice-one-time) |
| **"Microphone access was denied"** | Make sure voice is **enabled and saved**. OMNIX only allows the mic when voice is configured. Then check your OS privacy settings |
| **"No microphone was found"** | Plug in a mic, or choose a default input device in your system's sound settings |
| **Mic ring doesn't move when I talk** | The wrong input device is selected, or its volume is muted. Test it with **Settings → Voice → Test microphone** |
| **"That was too short"** | A quick tap *starts* recording. Speak, then tap again. Or hold the button the whole time you speak |
| **"Speech-to-text server at … "** error | The speech server isn't running or the URL is wrong. Open the URL in a browser to check it's reachable |
| **"No speech detected"** | Speak a little louder or closer to the mic, and check the language setting |
| **Ctrl + Space does nothing** | Another app (often the keyboard-language switcher) already uses it. Use the 🎤 button instead |
| **Not sure what's broken** | Run `./scripts/doctor.sh` in the OMNIX folder. It checks Ollama, the model, the speech server, Piper, and your settings, and says what to fix |
| **No models in the list** | Make sure Ollama is running (`ollama list`) and click ↻ to refresh |
| **Cloud provider is greyed out / blocked** | Local-only mode is on. Turn it off in **Settings → Security** if you really want cloud AI |
| **A command was blocked** | It's in the ⛔ dangerous group, or matches a rule in your blocklist. This is on purpose |
| **Knowledge view says memory is disabled** | Run `./scripts/setup-memory.sh`, then restart OMNIX |
| **OMNIX brings up an irrelevant or outdated memory** | Delete or edit it in **Knowledge → Memories**, or raise **Settings → Memory → Minimum relevance** |
| **A scheduled command didn't run: "needs your approval again"** | The command or its folder changed (or the rule was edited outside OMNIX). Delete the rule and create it again to re-approve |
| **A GPU shows "—" instead of numbers** | Its driver exposes no counters, or (NVIDIA) `nvidia-smi` isn't installed. The card is still listed so you know it's there |
| **Search says "keyword-only"** | The embedding model isn't reachable. Check `ollama list` shows `nomic-embed-text`; new items are embedded automatically once it's back |

---

## 10. Keyboard shortcuts

| Keys | Action |
|---|---|
| **Enter** | Send message |
| **Ctrl + Space** (hold) | Push-to-talk from anywhere |
| **Enter / Space** on the 🎤 button | Start / stop recording |
| **Esc** | Cancel a recording · close command suggestions |
| **/** | Start a command (shows suggestions) |

---

## 11. The Knowledge view, tab by tab

Open **🧠 Knowledge** in the sidebar. The four boxes at the top show how many **memories** and **documents** OMNIX has,
how many **folders** it keeps in sync, and how much disk space memory uses. If memory isn't set up you'll see a yellow
*Not configured* banner telling you how to fix it; if the embedding model is down you'll see a note that search is
keyword-only for now (nothing is lost; it catches up by itself).

**Buttons at the top right**

| Button | What it does |
|---|---|
| 📤 **Export** | Saves every memory and indexed document to a `.jsonl` file you choose. The file is readable only by you |
| 📥 **Import** | Merges an export file back in. Things you already have are merged, not duplicated |
| ⚡ **Optimize** | Tidies up: merges duplicate memories, finishes any pending indexing, and compacts the database |

### 💭 Memories

- **Add a memory**: type one fact per memory (*"My NAS is at 10.0.0.5"*), optionally with tags, a category and an
  importance from 1 to 10, then **💾 Save Memory**. If OMNIX already knew it, you'll see *"Already known: the existing
  memory was reinforced"* instead of getting a duplicate.
- Each memory shows its **category**, **importance stars**, and **where it came from**:
  ✍️ you · 🤖 assistant (the AI saved it because you asked) · 🧠 learned (from a conversation) · 📥 imported.
- 🔁 *n* = how many times you've said it again; 👁 *n* = how many times it was looked up on purpose (by the
  assistant's memory search). The automatic recall before each reply doesn't count, so a memory can't make itself
  stronger just by being recalled. 👎 *n* = how many times you flagged it as wrong under a reply; each flag ranks it
  lower. Saying the fact again yourself clears the flags.
- The small bar on the right is how **present** the memory is. Important, recent and often-used memories are strong;
  old, unused ones slowly fade (they're never deleted by fading, just ranked lower).
- 📌 **Pin** a memory so it never fades. 🗑️ deletes it. Fading only changes the order: an old memory that answers
  your question is still recalled.
- **🗂 Merged & replaced memories** (at the bottom) lists memories OMNIX hid because the local model decided they
  duplicated another one (🔀 *merged*) or that a newer fact replaced them (⏭ *replaced*). Nothing there is deleted.
  If the model got it wrong, press **↩ Restore** and the memory comes back exactly as it was. If it was right, press
  **✓ Keep**. Each entry says who decided: *by the local model*, *by the NLI model*, *both judges agreed* (with the
  optional second judge from `./scripts/setup-memory.sh --nli`, both must agree before anything is hidden), or
  *near-identical text*. New decisions are listed first, and until you've checked them a yellow banner at the top of the
  Knowledge view says how many are waiting (Analytics shows the same count as *Awaiting your review*).

### 📄 Documents

- **📄 Index Document** adds a single text or Markdown file you pick.
- **Watched folders** (if you've set any up) are listed with when they were last checked. **🔄 Sync folders** checks
  them right now instead of waiting for the next two-minute scan.
- Each document shows its size, how many searchable pieces (*chunks*) it was split into, and whether it's ✓ Indexed
  or ⏳ still Embedding (it's already findable by keyword while embedding).
- 🗑️ removes a document from the index. If it lives in a watched folder, it will come back on the next sync unless you
  delete or move the file itself.

### 📚 Knowledge Bases

Separate named knowledge bases are **planned**. Today everything lives in one store; use tags, categories and folders
to organise it.

### 🔍 Search

Type anything and press **Enter**. Search understands meaning *and* exact words, so both *"how do I back up my VMs"*
and *"vzdump"* work. Each result shows whether it's a memory (💭) or which note it came from (📝), and a **relevance**
percentage: roughly, under 25 % is noise, over 50 % is a solid match. This is exactly what OMNIX sees when it recalls
things for a reply, so it's a good way to check why it did or didn't know something.

### 📊 Analytics

Real numbers from the memory engine: memories by category and by source, engine health (models in use, items waiting
to be indexed, superseded memories, links between memories, searches in the last 24 hours and how fast they were,
last optimisation, storage), and a **recent activity** feed (memories saved, merged, retired, documents indexed,
facts learned). The activity feed never shows the text of your memories.

---

## 12. Getting the most out of memory

- **One fact per memory, in plain words.** *"Paul's backup NAS is at 10.0.0.5 (Synology, share /backups)"* recalls
  better than a paragraph mixing five topics.
- **Just tell it.** In chat, *"remember that …"* saves a memory. *"Actually, I moved it to 10.0.0.9"* is enough to
  retire the old fact: OMNIX's local model notices the contradiction and marks the old memory as outdated.
- **Ask it what it knows.** *"What do you remember about my homelab?"* makes it search memory explicitly.
- **Keep notes in a folder.** `./scripts/setup-memory.sh ~/notes` once, and every edit you make to those files is
  picked up within two minutes. Headings help: each piece remembers which heading it came from.
- **Pin the essentials** (your name, role, key systems) so they never fade.
- **Too chatty?** If OMNIX brings up things that aren't relevant, raise **Settings → Memory → Minimum relevance**
  (for example to 0.5) or lower **Entries recalled per message**. Not chatty enough? Lower it to 0.3.
- **Turn on learning when you're comfortable.** **Learn from conversations** saves facts from your messages
  automatically and tells you each time (*🧠 Remembered: …*). Review them in the Knowledge view; delete anything you
  don't want kept.
- **From a terminal** (optional): `kb-core status`, `kb-core search "question"`, `kb-core remember "fact"`,
  `kb-core export ~/memory-backup.jsonl`.

---

## 13. Your data and privacy (FAQ)

**Does anything I say leave my computer?**
Not by default. *Local-only mode* is on from the start: the AI model, speech recognition, read-aloud and memory all
run on your computer (or your own private network). Cloud AI providers are blocked unless you turn local-only mode off
yourself, and OMNIX asks you to confirm when you do. There's no telemetry.

**Where are my memories stored?**
In one file, `~/.local/share/omnix/kb-core/memory.db`, readable only by your user account. Exports are also created
readable only by you.

**Can the AI change my system without asking?**
No. Anything that changes your system opens a pop-up that you must approve, and dangerous actions are always blocked.
That's true whether you or the AI proposed it. Saving a *memory* doesn't need a pop-up (it's just a note, and you can
see and delete it in the Knowledge view), but it is recorded in the audit log.

**What does "Learn from conversations" read?**
Only **your** messages, after the reply is finished, using the local model. It never reads the AI's replies (those
can contain text from files or web tools), and it refuses to store passwords, API keys or similar secrets.

**Could something I index trick the AI?**
OMNIX treats everything from memory and files as *information*, never as *instructions*, and anything it might
prompt the AI to do still needs your approval. If a memory looks wrong, delete it.

**What happened to a memory that disappeared?**
If you said something newer that contradicts it, it was *superseded*: hidden from search but not deleted. Deleting the
newer memory brings the old one back. Duplicates are merged into one memory that keeps all the tags and history.

**How do I back up or move my memory?**
**Knowledge → 📤 Export** (or `kb-core export file.jsonl`), and **📥 Import** on the other machine.

**How do I erase everything?**
Stop the service and delete the file: `systemctl --user stop omnix-kb-core && rm ~/.local/share/omnix/kb-core/memory.db*`,
then `systemctl --user start omnix-kb-core` to start fresh. Watched folders will be re-indexed automatically.

**Can OMNIX run things on its own?**
Only what you set up in System Control (or approve when the assistant proposes a schedule or alert). A scheduled or
automated *command* that changes anything needs your approval once, when you create it; after that OMNIX runs exactly
that command and nothing else. If the command is edited, even outside OMNIX, it stops running until you approve it
again. Admin (sudo) commands never run unattended. Every run is recorded in the audit log.

**Where's the record of what OMNIX did?**
In the audit log. **Settings → Security → 🔏 Verify audit log** checks that nobody has edited it.

---

## 14. System Control: watch, automate and maintain your computer

Open **🎛️ System** in the sidebar. The tabs on the left:

| Tab | What you get |
|---|---|
| **📊 Overview** | CPU, memory, **every GPU** (NVIDIA and AMD) and network with 15-minute history graphs, plus storage and basic facts about the machine |
| **📈 Performance** | The detail: each GPU's load, VRAM, temperature, power, clocks and which programs use its memory; every CPU core; memory and swap; each network interface; all temperature sensors. **Agent** numbers: replies, tool calls and failures, memories recalled, facts learned. **Model** numbers: tokens per second, time to first word, cold starts, and which models sit in memory and how much of each is on the GPU |
| **⚙️ Processes** | Running programs, sortable and filterable; **End** asks you first |
| **🔧 Services** | Your system and user services (failed ones first) and your **Docker** containers with live CPU and memory. Start, stop and restart ask you first; system services also show your computer's password prompt. **Logs** shows recent output |
| **🧬 Models** | Every AI model you have: size, whether it's loaded, and how much is on the GPU. **Use for chat**, **Load**, **Unload** (frees GPU memory), **Delete** (asks first), and **Download** new ones with a progress bar |
| **🚨 Alerts** | Get a desktop notification when something needs attention: GPU too hot, disk nearly full, memory tight, a service or container down, Ollama or the memory service unreachable. One-click presets, or build your own (*"GPU 1 temperature above 85 °C for 60 seconds"*). A condition has to last for its whole window, so a brief spike doesn't nag you |
| **🤖 Automation** | *When* something happens, *do* something. Triggers: an alert fires, a condition holds, a program starts or stops, a file changes, the computer is idle. Actions: a notification, a command, or an AI report |
| **⏰ Scheduler** | Do something on a schedule: every 15 minutes, weekdays at 8:00, nightly at 2:00, or any cron expression. **Quick add → morning briefing** writes you a short health report every weekday morning and saves it to memory |
| **🧹 Cleanup & Optimize** | **Optimize** lists real findings with a one-click fix (e.g. *"qwen2.5:14b is loaded but unused: unload to free 9 GB"*, *"ollama.service has failed: restart"*, *"/ is 93% full: find space"*). **Cleanup** measures reclaimable space (Trash, thumbnail and download caches, unused Docker layers, old logs) and deletes only what you tick, after one confirmation |

**Commands in automations and schedules.** A command that only reads (like `df -h`) just runs. One that changes
something asks for your approval *once*, when you create the rule. The rule then shows ✓ approved. Admin (`sudo`)
commands and anything OMNIX always blocks can't be scheduled. Every run appears under *Recent activity* and in the
audit log.

**AI reports.** The local model gets a measured snapshot of the computer (load, GPUs, disks, failed services,
containers, loaded models, firing alerts) and writes a short summary. It can't run anything. Tick *Save each report
to long-term memory* to be able to ask later *"what did last week's reports say about the disk?"*

**Ask in chat.** You don't need the tabs for most of this. Try:

- *"How are my GPUs doing?"* or *"What's using the most memory?"*
- *"Restart the omnix-speaches container"* (you'll get an approval pop-up)
- *"Show me the last logs of ollama.service"*
- *"Every weekday at 8, give me a health report"* (the assistant proposes a schedule; you confirm it)
- *"Warn me if the disk goes over 90%"*

Notifications from alerts and automations appear on your desktop and inside OMNIX, and the avatar flashes.

