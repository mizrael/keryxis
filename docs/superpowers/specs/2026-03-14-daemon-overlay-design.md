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

### Unix Socket Protocol

Path: `~/.local/state/voice-terminal/voice-terminal.sock`

The daemon writes JSON state messages to connected clients whenever state changes:

```json
{
  "state": "recording",
  "target_app": "Terminal",
  "mode": "toggle",
  "last_text": "hello world",
  "timestamp": 1710000000
}
```

States: `idle`, `listening`, `recording`, `processing`.

The overlay (or CLI `status` command) connects as a client and reads these messages.

Clients can also send commands to the daemon:
```json
{"command": "stop"}
{"command": "toggle_recording"}
```

### Daemon Lifecycle

**CLI subcommands:**
- `voice-terminal daemon start` — fork to background, write PID file, start socket listener
- `voice-terminal daemon stop` — read PID file, send SIGTERM
- `voice-terminal daemon status` — connect to socket, print current state

**PID file:** `~/.local/state/voice-terminal/daemon.pid`

**System service files:**
- macOS: `~/Library/LaunchAgents/com.voice-terminal.plist` (installed via `voice-terminal daemon install`)
- Linux: `~/.config/systemd/user/voice-terminal.service` (installed via `voice-terminal daemon install`)

### Floating Overlay (egui + eframe)

**Window properties:**
- Size: ~220×50px
- Position: top-right corner of primary screen
- Always on top, no title bar, transparent background
- Click-through (doesn't steal focus)
- Rounded corners with subtle shadow

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
- **Linux:** Use X11 `_NET_ACTIVE_WINDOW` property via `x11rb` crate, or Wayland-specific protocol
- Polled every 500ms by the daemon, included in state updates

### New Dependencies

| Crate | Purpose |
|-------|---------|
| `eframe` / `egui` | Cross-platform GUI for the overlay |
| `objc2` + `objc2-app-kit` | macOS active window detection |
| `x11rb` (Linux) | X11 active window detection |
| `serde_json` | Socket protocol serialization |
| `daemonize` or manual fork | Background process management |
| `signal-hook` | Graceful shutdown on SIGTERM |

### New Module Structure

```
src/
├── daemon/
│   ├── mod.rs           # Daemon lifecycle (start, stop, status)
│   ├── socket.rs        # Unix socket server + protocol
│   └── service.rs       # launchd/systemd service file generation
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

## Changes to Existing Code

1. **main.rs** — Add `daemon` and `overlay` subcommands to the CLI. Extract the run loop into a function that can operate in daemon mode (writing state to socket).
2. **State management** — Extract recording/processing state into a shared `AppState` struct that can be serialized and sent over the socket.
3. **Config** — Add `DaemonConfig` and `OverlayConfig` sections.

The existing foreground modes (`voice-terminal start`) continue to work unchanged.

## Error Handling

- Daemon already running: print message with PID, exit cleanly
- Socket connection lost: overlay shows "disconnected" state, retries every 2s
- No accessibility permission: daemon starts but TextInjector init fails gracefully with clear message
- Overlay without daemon: shows "not connected" and retries

## Testing Strategy

- **Unit tests:** State serialization/deserialization, active window name parsing, socket protocol encoding
- **Integration tests:** Daemon start/stop lifecycle, PID file management, socket connect/disconnect
- **Manual tests:** Overlay rendering, always-on-top behavior, state transitions
