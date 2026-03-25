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
- **Cross-platform** — runs on macOS, Linux, and Windows with pre-built binaries for all three
- **Microphone selection** — choose from available input devices in the settings panel

## Requirements

- **macOS**, **Linux**, or **Windows**
- **Microphone** access
- **macOS Accessibility permission** — required for text injection (System Settings → Privacy & Security → Accessibility → add Keryxis or your terminal)
- ~75MB disk space for the Whisper tiny model (more for larger models)

## Installation

### Pre-built release

Download the latest release for your platform from the [Releases](https://github.com/mizrael/keryxis/releases) page:

| Platform | File | Notes |
|----------|------|-------|
| macOS (Apple Silicon) | `keryxis-macos-arm64.tar.gz` | M1/M2/M3/M4 Macs |
| macOS (Intel) | `keryxis-macos-x86_64.tar.gz` | Older Intel Macs |
| Linux (x86_64) | `keryxis-linux-x86_64.tar.gz` | |
| Windows (x86_64) | `keryxis-windows-x86_64.zip` | |

No Rust toolchain needed.

### Build from source

Requires the [Rust toolchain](https://rustup.rs/).

**macOS** (default features include Metal GPU acceleration + GUI):

```bash
cargo build --release
```

**Linux** (no Metal, GUI enabled):

```bash
cargo build --release --no-default-features --features gui
```

**Windows** (no Metal, GUI enabled):

```bash
cargo build --release --no-default-features --features gui
```

**Build without GUI** (headless/server, no overlay window):

```bash
cargo build --release --no-default-features --features metal  # macOS
cargo build --release --no-default-features                   # Linux/Windows
```

**Build with CUDA** (Linux with NVIDIA GPU):

```bash
cargo build --release --no-default-features --features cuda,gui
```

## Setup

### 1. Platform-specific setup

**macOS:** Grant Accessibility permission — go to **System Settings → Privacy & Security → Accessibility** and add your terminal app (Terminal.app, iTerm2, or VS Code).

**Windows:** No additional setup required. You may need to allow microphone access in Windows Settings → Privacy → Microphone.

**Linux:** Ensure your user has access to audio devices (usually automatic).

### 2. Start Keryxis

Simply run the binary with no arguments:

```bash
keryxis
```

This starts the background daemon and opens the floating overlay. On first launch, the Whisper model (`tiny`, ~75MB) is downloaded automatically. On Windows, the console window hides automatically.

You can also start explicitly:

```bash
keryxis daemon start
```

To pre-download a specific model size (optional):

```bash
keryxis download-model --size small
```

Available sizes: `tiny` (75MB, fastest), `base` (150MB), `small` (500MB), `medium` (1.5GB), `large` (3GB, most accurate).

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

- **Status indicator**: green (ready), red pulsing (recording), yellow (processing/disconnected), gray (paused — overlay focused)
- **Target app**: which application will receive the text
- **Mode label**: current activation mode
- **≡** — toggle live daemon log viewer
- **⚙** — open settings panel

### Settings panel

Click ⚙ to configure:

- **Mode** — Press to talk / Auto-stop / Hands-free
- **Hotkey** — click the field and press your desired key combination (e.g., Alt+Space, Cmd+R)
- **Wake word** — the phrase that activates hands-free mode
- **Microphone** — select from available input devices, or use system default (click ⟳ to refresh the list)
- **Model** — Whisper model size (Tiny through Large)
- **Languages** — ordered priority list; click `+ Language` to add, click a language to remove, `^` to reorder

Changes auto-restart the daemon when you click Save.

## Configuration

Config file location:
- **macOS/Linux:** `~/.config/keryxis/config.toml`
- **Windows:** `%APPDATA%\keryxis\config.toml`

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
# device = "Headset Microphone (Jabra)"  # optional — omit for system default

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
5. **Daemon** — background process communicates state to the overlay via IPC (Unix socket on macOS/Linux, TCP on Windows) using newline-delimited JSON

### Multi-language priority

When multiple languages are configured (e.g., `["en", "it"]`), Keryxis tries each in order. English is tried first — if the transcription is non-empty, it's used. Otherwise, Italian is tried. This is faster than auto-detect because each attempt with a specific language skips Whisper's language detection step.

## Files & Paths

### macOS / Linux

| Path | Purpose |
|------|---------|
| `~/.config/keryxis/config.toml` | Configuration |
| `~/.local/share/keryxis/models/` | Whisper model files |
| `~/.local/state/keryxis/daemon.pid` | Daemon PID file |
| `~/.local/state/keryxis/daemon.log` | Daemon log file |
| `~/.local/state/keryxis/keryxis.sock` | Unix socket for IPC |

### Windows

| Path | Purpose |
|------|---------|
| `%APPDATA%\keryxis\config.toml` | Configuration |
| `%APPDATA%\keryxis\models\` | Whisper model files |
| `%LOCALAPPDATA%\keryxis\daemon.pid` | Daemon PID file |
| `%LOCALAPPDATA%\keryxis\daemon.log` | Daemon log file |
| TCP `127.0.0.1:19457` | IPC (replaces Unix socket) |

## Supported Hotkeys

Modifiers: `Alt` / `Option`, `Ctrl` / `Control`, `Shift`, `Cmd` / `Meta` / `Super`

Keys: `A`-`Z`, `F1`-`F12`, `Space`, `Tab`, `Return`, `Escape`, `Backspace`

Examples: `Alt+Space`, `Ctrl+Shift+R`, `Cmd+E`, `F5`

## Troubleshooting

**"Accessibility permission required"** (macOS) — Add your terminal to System Settings → Privacy & Security → Accessibility. Toggle it off and on if already listed.

**Overlay shows "OFF"** — Daemon isn't running. Run `keryxis daemon start` or just `keryxis`.

**Overlay shows "PAUSED"** — The overlay window is focused. Click on another application to resume.

**Wrong microphone** — Open settings (⚙) and select the correct microphone from the dropdown. Click ⟳ to refresh the device list.

**Wake word not detected** — Whisper may transcribe your wake word differently. Check the daemon log (`≡` button in overlay) to see what Whisper hears. Punctuation is stripped before matching.

**Transcription is slow** — Use the `tiny` model. Ensure Metal GPU acceleration is working on macOS (check daemon log for "using device Metal").

**No audio captured** — Check that your microphone is working and not muted. On macOS, ensure microphone permission is granted. On Windows, check Settings → Privacy → Microphone.

## License

MIT
