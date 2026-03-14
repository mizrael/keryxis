# Daemon Mode + Floating Status Overlay — Design Spec

## Problem

Voice Terminal currently runs only in the foreground. Users want it as a background daemon with a visual indicator showing the recording state and which application will receive the injected text.

## Solution

Add two capabilities to the existing binary:

1. **Daemon mode** — runs the audio/Whisper/injection pipeline as a background service, managed via CLI (`voice-terminal daemon start|stop|status`) or system service (launchd/systemd).
2. **Floating overlay** — a small always-on-top egui window pinned to the top-right corner, showing current state and target application name.

The daemon and overlay communicate through a Unix domain socket.

## Architecture

### Process Model

Two processes from the same binary:

```
voice-terminal daemon start    →  Background process (audio, Whisper, injection)
                                   Listens on Unix socket
                                   Publishes state updates

voice-terminal overlay         →  Foreground GUI process
                                   Connects to daemon socket
                                   Renders status in floating window
```

The daemon can also run standalone without the overlay (headless mode for servers or SSH sessions). The overlay can be started/stopped independently.

**Foreground conflict guard:** `voice-terminal start` (foreground mode) checks for a running daemon via the PID file. If a daemon is already running, it prints an error and exits — both cannot compete for the audio device and hotkey listener.

### Unix Socket Protocol

Path: `~/.local/state/voice-terminal/voice-terminal.sock`

**Framing:** Newline-delimited JSON (each message is a single JSON object terminated by `\n`). This provides clear message boundaries over the stream socket.

The daemon writes state messages to connected clients whenever state changes:

```json
{"state":"recording","target_app":"Terminal","mode":"toggle","last_text":"hello world","timestamp":1710000000}\n
```

States: `idle`, `listening`, `recording`, `processing`.

The overlay (or CLI `status` command) connects as a client and reads these messages.

Clients can also send commands to the daemon:
```json
{"command":"stop"}\n
{"command":"toggle_recording"}\n
```

### Daemon Lifecycle

**Fork ordering:** The daemon forks to background **first** (before any tokio/cpal/Whisper initialization). The child process then creates the tokio runtime, initializes audio capture, loads the Whisper model, and starts the socket listener. This avoids undefined behavior from forking with active async runtimes or audio device handles.

**CLI subcommands:**
- `voice-terminal daemon start` — fork to background, write PID file, init runtime, start socket listener
- `voice-terminal daemon stop` — read PID file, verify PID is a voice-terminal process (stale PID check), send SIGTERM
- `voice-terminal daemon status` — connect to socket, print current state

**PID file:** `~/.local/state/voice-terminal/daemon.pid`

**Stale PID handling:** On `daemon start`, if a PID file exists, verify the process is alive and is actually voice-terminal (check `/proc/<pid>/cmdline` on Linux, `kill(pid, 0)` + name check on macOS). If stale, remove the PID file and proceed.

**Logging:** After fork, stdout/stderr are closed. The daemon reconfigures `tracing-subscriber` to write to a log file at `~/.local/state/voice-terminal/daemon.log`. No rotation in v1 — file is truncated on each daemon start.

**System service files (deferred):** Generation of launchd plists and systemd unit files is deferred to a follow-up. Users can write these manually for now.

### Floating Overlay (egui + eframe)

**Window properties:**
- Size: ~220×50px
- Position: top-right corner of primary screen
- Always on top, no title bar, transparent background
- Click-through (doesn't steal focus)
- Rounded corners with subtle shadow

**Platform notes:**
- macOS: egui/eframe supports transparent always-on-top windows natively
- Linux/X11: works via X11 window hints
- Linux/Wayland: requires XWayland for transparency + always-on-top; native Wayland overlay is out of scope for v1

**Display:**
- State indicator: colored circle (🟢 listening, 🔴 recording, 🟡 processing, ⚫ idle)
- Target app name: e.g., "→ Terminal"
- Activation mode shown as small label

**Color scheme:**
| State | Circle | Background |
|-------|--------|------------|
| Idle | Gray | Semi-transparent dark |
| Listening | Green | Semi-transparent dark |
| Recording | Red (pulsing) | Semi-transparent dark |
| Processing | Yellow | Semi-transparent dark |

### Active Window Detection

New module `src/ui/active_window.rs`:

- **macOS:** Use `objc2` crate to call `NSWorkspace.sharedWorkspace.frontmostApplication.localizedName`
- **Linux (X11):** Use `x11rb` crate with `_NET_ACTIVE_WINDOW` property
- **Linux (Wayland):** Out of scope for v1 — returns "Unknown" gracefully
- Polled every 500ms by the daemon, included in state updates

### New Dependencies

| Crate | Purpose |
|-------|---------|
| `eframe` / `egui` | Cross-platform GUI for the overlay |
| `objc2` + `objc2-app-kit` | macOS active window detection |
| `x11rb` (Linux) | X11 active window detection |
| `serde_json` | Socket protocol serialization |
| `signal-hook` | Graceful shutdown on SIGTERM |

Note: No `daemonize` crate — use manual `fork()` via `nix` crate for full control over initialization ordering.

### New Module Structure

```
src/
├── daemon/
│   ├── mod.rs           # Daemon lifecycle (start, stop, status)
│   └── socket.rs        # Unix socket server + protocol
├── ui/
│   ├── mod.rs           # UI module
│   ├── overlay.rs       # egui floating overlay window
│   └── active_window.rs # Platform-specific active window detection
└── state.rs             # Shared AppState struct
```

### Configuration Additions

```toml
[daemon]
auto_start_overlay = true  # Launch overlay when daemon starts

[overlay]
position = "top-right"     # top-right, top-left, bottom-right, bottom-left
opacity = 0.85             # Window opacity (0.0 - 1.0)
```

**Note on `auto_start_overlay`:** The daemon spawns the overlay as a child process, passing display server environment variables (`$DISPLAY`, `$WAYLAND_DISPLAY`) from the original `daemon start` invocation to the child. This only works when started from a graphical session — when started via launchd/systemd, the service file must include the display environment.

### Cargo Feature Flags

The overlay and its dependencies (egui, eframe) are behind a `gui` feature flag, enabled by default. Headless/server deployments can build without it:

```toml
[features]
default = ["metal", "gui"]
gui = ["eframe", "egui"]
```

## Changes to Existing Code

1. **main.rs** — Add `daemon` and `overlay` subcommands to the CLI. Extract the run loop into a function that can operate in daemon mode (writing state to socket).
2. **State management** — Extract recording/processing state into a shared `AppState` struct that can be serialized and sent over the socket.
3. **Config** — Add `DaemonConfig` and `OverlayConfig` sections.

The existing foreground modes (`voice-terminal start`) continue to work unchanged (with the conflict guard above).

## Error Handling

- Daemon already running: print message with PID, exit cleanly
- Stale PID file: detect and clean up, then proceed
- Socket connection lost: overlay shows "disconnected" state, retries every 2s
- No accessibility permission: daemon starts but TextInjector init fails gracefully with clear message
- Overlay without daemon: shows "not connected" and retries
- Foreground mode with daemon running: print error, exit

## Testing Strategy

- **Unit tests:** State serialization/deserialization, active window name parsing, socket protocol encoding/framing, PID file stale detection, config additions
- **Integration tests:** Daemon start/stop lifecycle, PID file management, socket connect/disconnect
- **Manual tests:** Overlay rendering, always-on-top behavior, state transitions
