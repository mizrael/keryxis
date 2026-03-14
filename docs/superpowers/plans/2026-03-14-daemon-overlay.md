# Daemon Mode + Floating Overlay Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement. Steps use checkbox syntax for tracking.

**Goal:** Add background daemon mode with IPC and a floating egui overlay showing recording state and target application name.

**Spec:** `docs/superpowers/specs/2026-03-14-daemon-overlay-design.md`

---

## Chunk 1: Shared State + Socket Protocol

### Task 1: Create AppState struct

**Files:** Create `src/state.rs`, `tests/state_tests.rs`. Modify `src/lib.rs`, `Cargo.toml`.

- [ ] Step 1: Add `serde_json = "1"` and `libc = "0.2"` to Cargo.toml dependencies
- [ ] Step 2: Create `src/state.rs` with `DaemonState` enum (Idle/Listening/Recording/Processing, serde snake_case) and `AppState` struct (state, target_app, mode, last_text, timestamp). Include `to_framed_json()` (append newline) and `from_framed_json()` (parse trimmed line) methods. Default: Idle, "Unknown", "toggle", empty, current timestamp.
- [ ] Step 3: Add `pub mod state;` to `src/lib.rs`
- [ ] Step 4: Create `tests/state_tests.rs` testing: default values, JSON serialize/deserialize roundtrip, all DaemonState variants serialize to snake_case, newline framing roundtrip
- [ ] Step 5: Run `cargo test --test state_tests` — all pass
- [ ] Step 6: Commit "feat: add AppState struct with JSON serialization"

### Task 2: Unix socket server and protocol

**Files:** Create `src/daemon/mod.rs`, `src/daemon/socket.rs`, `tests/daemon_tests.rs`. Modify `src/lib.rs`.

- [ ] Step 1: Create `src/daemon/mod.rs` with helper functions: `state_dir()` (uses `dirs::state_dir` or `dirs::data_local_dir` fallback), `socket_path()` (returns state_dir/voice-terminal.sock), `pid_file_path()` (returns state_dir/daemon.pid), `is_pid_stale(path)` (reads PID, checks with `libc::kill(pid, 0)`), `write_pid_file()`, `remove_pid_file()`, `is_daemon_running()` (checks PID file + stale detection)
- [ ] Step 2: Create `src/daemon/socket.rs` with `Broadcaster` (holds `Arc<Mutex<Vec<UnixStream>>>`, methods: `broadcast(state)` serializes to framed JSON and writes to all clients removing dead ones, `client_count()`) and `SocketServer` (binds UnixListener, methods: `new(path)` removes stale socket, `broadcaster()` returns clone, `accept_loop_once()` for testing, `accept_loop()` blocking loop for production)
- [ ] Step 3: Add `pub mod daemon;` to `src/lib.rs`
- [ ] Step 4: Create `tests/daemon_tests.rs` testing: socket_path resolution, pid_file_path resolution, socket server accept + broadcast roundtrip (create temp dir, start server, connect client, broadcast state, read from client), stale PID detection (fake PID=9999999 is stale, own PID is not stale)
- [ ] Step 5: Run `cargo test --test daemon_tests` — all pass
- [ ] Step 6: Run `cargo test` — all suites pass (no regressions)
- [ ] Step 7: Commit "feat: add daemon socket server with newline-delimited JSON protocol"

---

## Chunk 2: Active Window Detection + Config Extensions

### Task 3: Platform-specific active window detection

**Files:** Create `src/ui/mod.rs`, `src/ui/active_window.rs`, `tests/active_window_tests.rs`. Modify `src/lib.rs`.

- [ ] Step 1: Create `src/ui/active_window.rs` with `get_active_window_name() -> String`. On macOS: shell out to `osascript -e 'tell application "System Events" to get name of first application process whose frontmost is true'`. On Linux: shell out to `xdotool getactivewindow getwindowname`. On failure or other OS: return "Unknown".
- [ ] Step 2: Create `src/ui/mod.rs` exporting `pub mod active_window;`
- [ ] Step 3: Add `pub mod ui;` to `src/lib.rs`
- [ ] Step 4: Create `tests/active_window_tests.rs` testing: returns non-empty string (or "Unknown"), no panics on repeated calls
- [ ] Step 5: Run `cargo test --test active_window_tests` — all pass
- [ ] Step 6: Commit "feat: add cross-platform active window detection"

### Task 4: Extend configuration

**Files:** Modify `src/config/settings.rs`, `config/default.toml`, `tests/config_tests.rs`.

- [ ] Step 1: Add `DaemonConfig` struct (field: `auto_start_overlay: bool`, default true) and `OverlayConfig` struct (fields: `position: String` default "top-right", `opacity: f32` default 0.85) to `src/config/settings.rs`
- [ ] Step 2: Add `pub daemon: DaemonConfig` and `pub overlay: OverlayConfig` fields to `AppConfig`, update Default impl
- [ ] Step 3: Append `[daemon]` and `[overlay]` sections to `config/default.toml`
- [ ] Step 4: Add tests to `tests/config_tests.rs`: default daemon config values, default overlay config values, serialization includes new sections, deserialization of full config with new sections
- [ ] Step 5: Run `cargo test --test config_tests` — all pass
- [ ] Step 6: Run `cargo test` — all suites pass
- [ ] Step 7: Commit "feat: add daemon and overlay configuration sections"

---

## Chunk 3: Daemon Lifecycle (start/stop/status)

### Task 5: Daemon start with fork, stop, and status commands

**Files:** Create `src/daemon/lifecycle.rs`. Modify `src/daemon/mod.rs`, `src/main.rs`.

- [ ] Step 1: Create `src/daemon/lifecycle.rs` with: `daemonize() -> Result<bool>` (calls `libc::fork()` before any runtime init; returns true for parent/false for child; child calls `setsid`), `stop_daemon()` (reads PID file, checks stale, sends SIGTERM, waits up to 2s, cleans up PID+socket files), `print_status()` (connects to socket, reads one line, prints state), `setup_daemon_logging()` (opens log file at state_dir/daemon.log truncated, inits tracing-subscriber with file writer and no ANSI)
- [ ] Step 2: Add `pub mod lifecycle;` to `src/daemon/mod.rs`
- [ ] Step 3: Add `Daemon { action: DaemonAction }` and `Overlay` variants to `Commands` enum in main.rs. Add `DaemonAction` enum with `Start`, `Stop`, `Status` subcommands.
- [ ] Step 4: Add match arms: `Daemon::Start` checks `is_daemon_running`, calls `daemonize()`, parent exits, child calls `setup_daemon_logging`, `write_pid_file`, `run_daemon(config).await`. `Daemon::Stop` calls `stop_daemon()`. `Daemon::Status` calls `print_status()`. `Overlay` placeholder prints "not yet implemented".
- [ ] Step 5: Add foreground conflict guard to `Start` and `None` arms: if `daemon::is_daemon_running()`, bail with message.
- [ ] Step 6: Implement `run_daemon(config)` function: loads model, inits recognizer/capture/injector, starts SocketServer + accept_loop in thread, spawns active_window polling task (every 500ms), calls `run_toggle_mode_daemon` (or VAD/WakeWord variant).
- [ ] Step 7: Implement `run_toggle_mode_daemon` — same as `run_toggle_mode` but takes `&Broadcaster` param, broadcasts AppState at each transition (Listening→Recording→Processing→Listening), updates `target_app` via `get_active_window_name()` and `last_text`.
- [ ] Step 8: Add stub `run_vad_mode_daemon` and `run_wake_word_mode_daemon` that delegate to non-daemon versions (state broadcasting deferred).
- [ ] Step 9: Add imports to main.rs: `use voice_terminal::{daemon, state, ui};`
- [ ] Step 10: Run `cargo build` — compiles (warnings OK for stubs)
- [ ] Step 11: Run `cargo test` — all suites pass
- [ ] Step 12: Commit "feat: add daemon lifecycle with fork, socket broadcasting, and CLI commands"

---

## Chunk 4: Floating egui Overlay

### Task 6: Add egui/eframe dependencies behind feature flag

**Files:** Modify `Cargo.toml`.

- [ ] Step 1: Add to dependencies: `eframe = { version = "0.31", optional = true, default-features = false, features = ["default_fonts", "glow"] }` and `egui = { version = "0.31", optional = true }`
- [ ] Step 2: Update features: `default = ["metal", "gui"]`, add `gui = ["dep:eframe", "dep:egui"]`
- [ ] Step 3: Run `cargo check --features gui` — compiles
- [ ] Step 4: Run `cargo check --no-default-features` — compiles without egui
- [ ] Step 5: Commit "feat: add egui/eframe dependencies behind gui feature flag"

### Task 7: Implement floating overlay window

**Files:** Create `src/ui/overlay.rs`. Modify `src/ui/mod.rs`, `src/main.rs`.

- [ ] Step 1: Create `src/ui/overlay.rs` (all behind `#[cfg(feature = "gui")]`): `run_overlay(sock_path, opacity, position)` function that spawns a socket reader thread (connects to daemon, reads newline-delimited JSON into `Arc<Mutex<AppState>>`, reconnects on disconnect every 2s) and runs `eframe::run_native` with `OverlayApp`.
- [ ] Step 2: Implement `OverlayApp` struct (holds `Arc<Mutex<AppState>>`), impl `eframe::App`: `clear_color` returns transparent, `update` reads state, repaints every 200ms, renders CentralPanel with dark semi-transparent background (rounding 8px), horizontal layout with: colored circle (gray/green/red/yellow), emoji label, "→ AppName" text.
- [ ] Step 3: NativeOptions: inner_size 220x50, always_on_top, no decorations, transparent.
- [ ] Step 4: Add `#[cfg(feature = "gui")] pub mod overlay;` to `src/ui/mod.rs`
- [ ] Step 5: Wire `Commands::Overlay` in main.rs: load config, get socket path, call `ui::overlay::run_overlay()`. Add `#[cfg(not(feature = "gui"))]` fallback that bails.
- [ ] Step 6: Run `cargo build --release --features gui` — compiles
- [ ] Step 7: Run `cargo test` — all pass
- [ ] Step 8: Commit "feat: add floating egui overlay showing daemon state and target app"

---

## Chunk 5: Integration + Final Wiring

### Task 8: Auto-start overlay from daemon + end-to-end test

**Files:** Modify `src/main.rs`.

- [ ] Step 1: In `DaemonAction::Start` handler, after `write_pid_file()` and before `run_daemon()`, if `config.daemon.auto_start_overlay`: spawn `current_exe() overlay` as child process, preserving DISPLAY/WAYLAND_DISPLAY/XDG_RUNTIME_DIR env vars. Log success or warn on failure.
- [ ] Step 2: Delete stale config: `rm -f "$HOME/Library/Application Support/voice-terminal/config.toml"`
- [ ] Step 3: Build release: `cargo build --release`
- [ ] Step 4: Test `cargo run --release -- daemon start` — forks, prints PID, returns to shell
- [ ] Step 5: Test `cargo run --release -- daemon status` — shows state
- [ ] Step 6: Test `cargo run --release -- overlay` — floating window appears
- [ ] Step 7: Test hotkey while daemon running — overlay shows state transitions
- [ ] Step 8: Test `cargo run --release -- daemon stop` — stops cleanly
- [ ] Step 9: Test foreground conflict: `daemon start` then `start` → error message
- [ ] Step 10: Run `cargo test` — all pass
- [ ] Step 11: Commit "feat: auto-start overlay from daemon, integration verified"
