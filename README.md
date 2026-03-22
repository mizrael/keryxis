# Keryxis

**The herald who proclaims your words** — a local, privacy-first speech-to-text tool that injects dictated text into any application.

Keryxis runs as a background daemon with a floating overlay, using OpenAI's Whisper model locally on your machine. No cloud APIs, no data leaves your computer.

## Features

- **Three activation modes:**
  - **Press to talk** — press a hotkey to start/stop recording
  - **Auto-stop** — press a hotkey, recording stops automatically when you go silent
  - **Hands-free** — always listening for a configurable wake word (e.g., "hey terminal")
- **Works with any application** — injects text into whatever window is focused (terminal, editor, browser, etc.)
- **Multi-language** — supports 99 languages with priority-based detection (try English first, then Italian, etc.)
- **Background daemon** with floating overlay showing recording state and target application
- **Settings panel** — change mode, hotkey, wake word, model size, and languages from the overlay UI
- **Local & private** — Whisper runs entirely on your machine with Metal GPU acceleration on macOS

## Requirements

- **macOS** (primary) or **Linux**
- **Microphone** access
- **macOS Accessibility permission** — required for text injection (System Settings → Privacy & Security → Accessibility → add Keryxis or your terminal)
- ~75MB disk space for the Whisper tiny model (more for larger models)

## Installation

### Pre-built release

Download the latest release from the [Releases](https://github.com/mizrael/keryxis/releases) page. No Rust toolchain needed.

### Build from source

Requires the [Rust toolchain](https://rustup.rs/).

```bash
cd keryxis
cargo build --release

# The binary is at target/release/keryxis
```

**Build without GUI** (headless/server, no overlay window):

```bash
cargo build --release --no-default-features --features metal
```

**Build with CUDA** (Linux with NVIDIA GPU):

```bash
cargo build --release --no-default-features --features cuda,gui
```

## Setup

### 1. Download a Whisper model

```bash
cargo run --release -- download-model --size tiny
```

Available sizes: `tiny` (75MB, fastest), `base` (150MB), `small` (500MB), `medium` (1.5GB), `large` (3GB, most accurate).

### 2. Grant Accessibility permission (macOS)

Go to **System Settings → Privacy & Security → Accessibility** and add your terminal app (Terminal.app, iTerm2, or VS Code).

### 3. Start the daemon

```bash
cargo run --release -- daemon start
```

This starts the background daemon and opens the floating overlay. You're ready to go!

## Usage

### Daemon commands

```bash
# Start daemon + overlay
keryxis daemon start

# Check status
keryxis daemon status

# Stop daemon + overlay
keryxis daemon stop

# Open overlay separately
keryxis overlay
```

### Foreground mode (no daemon)

```bash
# Run in foreground with default settings
keryxis start

# Run with specific mode
keryxis start --mode vad
keryxis start --mode wake-word

# Run with a different hotkey
keryxis start --hotkey "Ctrl+Shift+R"
```

### Configuration via CLI

```bash
# Show current config
keryxis config --show

# Change settings
keryxis config --mode vad
keryxis config --hotkey "Alt+R"
keryxis config --wake-word "hey computer"
keryxis config --model small
keryxis config --language it
```

## The Overlay

The floating overlay shows:

```
● RDY > Terminal [Press to talk]  ≡ ⚙
```

- **Status indicator**: green (ready), red pulsing (recording), yellow (processing/disconnected), gray (idle)
- **Target app**: which application will receive the text
- **Mode label**: current activation mode
- **≡** — toggle live daemon log viewer
- **⚙** — open settings panel

### Settings panel

Click ⚙ to configure:

- **Mode** — Press to talk / Auto-stop / Hands-free
- **Hotkey** — click the field and press your desired key combination (e.g., Alt+Space, Cmd+R)
- **Wake word** — the phrase that activates hands-free mode
- **Model** — Whisper model size (Tiny through Large)
- **Languages** — ordered priority list; click `+ Language` to add, click a language to remove, `^` to reorder

Changes auto-restart the daemon when you click Save.

## Configuration

Config file: `~/.config/keryxis/config.toml`

```toml
[activation]
mode = "toggle"           # toggle, vad, or wake_word
hotkey = "Alt+Space"
wake_word = "hey terminal"

[whisper]
model_size = "tiny"       # tiny, base, small, medium, large
language = ""             # single language override (legacy)
languages = ["en", "it"]  # priority list — tried in order

[vad]
energy_threshold = 0.01   # speech detection sensitivity (0.0 - 1.0)
silence_duration_ms = 1500
min_speech_duration_ms = 500

[audio]
sample_rate = 16000
channels = 1

[daemon]
auto_start_overlay = true

[overlay]
position = "top-right"    # top-right, top-left, bottom-right, bottom-left
opacity = 0.85            # overlay background opacity (0.0 - 1.0)
```

## How It Works

1. **Audio capture** — records from your microphone via `cpal` (cross-platform)
2. **Voice activity detection** — energy-based VAD detects speech onset and silence
3. **Speech recognition** — local Whisper model (via `whisper-rs` / `whisper.cpp`) transcribes audio
4. **Text injection** — `enigo` simulates keyboard input into the focused application
5. **Daemon** — background process communicates state to the overlay via Unix socket (newline-delimited JSON)

### Multi-language priority

When multiple languages are configured (e.g., `["en", "it"]`), Keryxis tries each in order. English is tried first — if the transcription is non-empty, it's used. Otherwise, Italian is tried. This is faster than auto-detect because each attempt with a specific language skips Whisper's language detection step.

## Files & Paths

| Path | Purpose |
|------|---------|
| `~/.config/keryxis/config.toml` | Configuration |
| `~/.local/share/keryxis/models/` | Whisper model files |
| `~/.local/state/keryxis/daemon.pid` | Daemon PID file |
| `~/.local/state/keryxis/daemon.log` | Daemon log file |
| `~/.local/state/keryxis/keryxis.sock` | Unix socket for IPC |

## Supported Hotkeys

Modifiers: `Alt` / `Option`, `Ctrl` / `Control`, `Shift`, `Cmd` / `Meta` / `Super`

Keys: `A`-`Z`, `F1`-`F12`, `Space`, `Tab`, `Return`, `Escape`, `Backspace`

Examples: `Alt+Space`, `Ctrl+Shift+R`, `Cmd+E`, `F5`

## Troubleshooting

**"Accessibility permission required"** — Add your terminal to System Settings → Privacy & Security → Accessibility. Toggle it off and on if already listed.

**Overlay shows "OFF"** — Daemon isn't running. Run `keryxis daemon start`.

**Wake word not detected** — Whisper may transcribe your wake word differently. Check the daemon log (`≡` button in overlay) to see what Whisper hears. Punctuation is stripped before matching.

**Transcription is slow** — Use the `tiny` model. Ensure Metal GPU acceleration is working (check daemon log for "using device Metal").

**No audio captured** — Check that your microphone is working and not muted. On macOS, ensure microphone permission is granted.

## License

MIT
