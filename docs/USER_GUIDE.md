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
| **Knowledge** | Long-term memory and documents (needs a memory service) |
| **System Control** | Live CPU, memory, disk, network, and a process list |
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

The Home screen buttons are shortcuts: **📊 System Status** runs `/monitor`, **📁 List Files** lists the current folder,
and **❓ Help** asks OMNIX what it can do.

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
| **Memory** | Connect the optional long-term memory / knowledge-base service |
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

---

## 10. Keyboard shortcuts

| Keys | Action |
|---|---|
| **Enter** | Send message |
| **Ctrl + Space** (hold) | Push-to-talk from anywhere |
| **Enter / Space** on the 🎤 button | Start / stop recording |
| **Esc** | Cancel a recording · close command suggestions |
| **/** | Start a command (shows suggestions) |
