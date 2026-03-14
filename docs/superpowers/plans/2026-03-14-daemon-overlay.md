# Daemon Mode + Floating Overlay Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement. Steps use checkbox syntax for tracking.

**Goal:** Add background daemon mode with IPC and a floating egui overlay showing recording state and target application name.

**Spec:** `docs/superpowers/specs/2026-03-14-daemon-overlay-design.md`

**Design decisions deviating from spec:**
- **Active window detection:** Uses `osascript` (macOS) and `xdotool` (Linux) shell-outs instead of `objc2`/`x11rb` crate bindings. Deliberate simplification to avoid heavy native dependencies. Same result, fewer build issues.
- **Fork:** Uses `libc::fork()` directly instead of `nix` crate. Simpler, fewer deps; we only need fork+setsid.
- **Client→daemon commands** (stop, toggle_recording via socket): Deferred to follow-up. Socket is broadcast-only for v1.

**Parallelism note:** Chunks 1 and 2 are independent and can be implemented in parallel.

---

## Chunk 1: Shared State + Socket Protocol

### Task 1: Create AppState struct

**Files:** Create `src/state.rs`, `tests/state_tests.rs`. Modify `src/lib.rs`, `Cargo.toml`.

- [ ] Step 1: Add `serde_json = "1"` and `libc = "0.2"` to Cargo.toml dependencies
- [ ] Step 2: Create `src/state.rs` with `DaemonState` enum (Idle/Listening/Recording/Processing, serde snake_case, derives Debug/Clone/PartialEq/Serialize/Deserialize, Display impl) and `AppState` struct (state: DaemonState, target_app: String, mode: String, last_text: String, timestamp: u64). Include `to_framed_json() -> Result<String>` (serde_json::to_string + push '\n') and `from_framed_json(line: &str) -> Result<Self>` (trim trailing '\n', serde_json::from_str). Default: Idle, "Unknown", "toggle", empty string, current unix timestamp.
- [ ] Step 3: Add `pub mod state;` to `src/lib.rs`
- [ ] Step 4: Create `tests/state_tests.rs` testing: default values, JSON serialize/deserialize roundtrip, all 4 DaemonState variants serialize to snake_case strings, newline framing roundtrip (to_framed_json ends with '\n', from_framed_json parses it back)
- [ ] Step 5: Run `cargo test --test state_tests` — all pass
- [ ] Step 6: Commit "feat: add AppState struct with JSON serialization"

### Task 2: Unix socket server and protocol

**Files:** Create `src/daemon/mod.rs`, `src/daemon/socket.rs`, `tests/daemon_tests.rs`. Modify `src/lib.rs`.

- [ ] Step 1: Create `src/daemon/mod.rs` with helper functions: `state_dir() -> Result<PathBuf>` (uses `dirs::state_dir` with `dirs::data_local_dir` fallback, creates dir), `socket_path()` (state_dir/voice-terminal.sock), `pid_file_path()` (state_dir/daemon.pid), `is_pid_stale(path: &Path) -> bool` (reads PID string, parses to i32, checks `libc::kill(pid, 0) != 0`, plus on macOS checks `sysctl` or `/proc` on Linux that the process name contains "voice-terminal" to avoid PID reuse false positives — falls back to kill-only check if name check fails), `write_pid_file()`, `remove_pid_file()`, `is_daemon_running() -> bool` (pid_path exists + not stale)
- [ ] Step 2: Create `src/daemon/socket.rs` with `Broadcaster` struct (holds `Arc<Mutex<Vec<UnixStream>>>`) with methods: `broadcast(&self, state: &AppState) -> Result<()>` (serializes via `to_framed_json`, writes to all clients, `retain_mut` removes clients where write/flush fails), `client_count() -> usize`. And `SocketServer` struct (holds UnixListener + Broadcaster) with: `new(path: &Path) -> Result<Self>` (removes stale socket file, create_dir_all parent, bind, set_nonblocking(false)), `broadcaster() -> Broadcaster` (clone), `accept_loop_once() -> Result<()>` (single accept for testing), `accept_loop()` (blocking loop, logs connect/error, breaks on accept error)
- [ ] Step 3: Add `pub mod daemon;` to `src/lib.rs`
- [ ] Step 4: Create `tests/daemon_tests.rs` testing: `socket_path()` contains "voice-terminal" and ends with ".sock", `pid_file_path()` contains "voice-terminal" and ends with "daemon.pid", socket server roundtrip (temp dir, create SocketServer, spawn accept_loop_once in thread, connect UnixStream client, sleep 50ms, broadcast AppState{Recording, "Terminal", ...}, client reads line with BufReader, deserialize and assert state==Recording and target_app=="Terminal"), stale PID (write PID 9999999 to temp file → is_stale==true, write own PID → is_stale==false)
- [ ] Step 5: Run `cargo test --test daemon_tests` — all pass
- [ ] Step 6: Run `cargo test` — all suites pass (no regressions)
- [ ] Step 7: Commit "feat: add daemon socket server with newline-delimited JSON protocol"

---

## Chunk 2: Active Window Detection + Config Extensions

### Task 3: Platform-specific active window detection

**Files:** Create `src/ui/mod.rs`, `src/ui/active_window.rs`, `tests/active_window_tests.rs`. Modify `src/lib.rs`.

- [ ] Step 1: Create `src/ui/active_window.rs` with `pub fn get_active_window_name() -> String` that dispatches to platform-specific impl via `#[cfg]`. macOS: `fn get_active_window_macos() -> String` runs `Command::new("osascript").args(["-e", "tell application \"System Events\" to get name of first application process whose frontmost is true"])`, returns stdout trimmed or "Unknown" on failure. Linux: `fn get_active_window_linux() -> String` runs `Command::new("xdotool").args(["getactivewindow", "getwindowname"])`, returns stdout trimmed or "Unknown" on failure. Other OS or any error: "Unknown".
- [ ] Step 2: Create `src/ui/mod.rs` with `pub mod active_window;`
- [ ] Step 3: Add `pub mod ui;` to `src/lib.rs`
- [ ] Step 4: Create `tests/active_window_tests.rs` with: `test_get_active_window_returns_string` (result is non-empty), `test_get_active_window_no_panic` (call 10 times in a loop, no panics)
- [ ] Step 5: Run `cargo test --test active_window_tests` — all pass
- [ ] Step 6: Commit "feat: add cross-platform active window detection"

### Task 4: Extend configuration

**Files:** Modify `src/config/settings.rs`, `config/default.toml`, `tests/config_tests.rs`.

- [ ] Step 1: Add `DaemonConfig` struct (field: `auto_start_overlay: bool`, default true) and `OverlayConfig` struct (fields: `position: String` default "top-right", `opacity: f32` default 0.85) to `src/config/settings.rs`. Both derive Debug/Clone/Serialize/Deserialize with Default impls.
- [ ] Step 2: Add `pub daemon: DaemonConfig` and `pub overlay: OverlayConfig` fields to `AppConfig`, add to Default impl
- [ ] Step 3: Append `[daemon]` and `[overlay]` sections to `config/default.toml`
- [ ] Step 4: Add tests to `tests/config_tests.rs`: assert default daemon.auto_start_overlay==true, assert default overlay.position=="top-right" and opacity≈0.85, assert serialization contains "[daemon]" and "[overlay]", assert deserialization of full TOML string with all sections including daemon(auto_start_overlay=false) and overlay(position="top-left", opacity=0.9) parses correctly
- [ ] Step 5: Run `cargo test --test config_tests` — all pass
- [ ] Step 6: Run `cargo test` — all suites pass
- [ ] Step 7: Commit "feat: add daemon and overlay configuration sections"

---

## Chunk 3: Daemon Lifecycle (start/stop/status)

### Task 5a: Daemon lifecycle functions and CLI commands

**Files:** Create `src/daemon/lifecycle.rs`. Modify `src/daemon/mod.rs`, `src/main.rs`.

- [ ] Step 1: Create `src/daemon/lifecycle.rs` with 4 functions:
  - `daemonize() -> Result<bool>`: unsafe `libc::fork()`. On -1: bail with errno. On 0 (child): call `libc::setsid()`, return Ok(false). On >0 (parent): print "Daemon started with PID {pid}", return Ok(true).
  - `stop_daemon() -> Result<()>`: read pid_file_path, if not exists bail "No daemon running". If is_pid_stale: print "Stale PID file", remove pid+socket files, return Ok. Parse PID, print "Stopping daemon (PID {pid})...", `libc::kill(pid, SIGTERM)`. Poll `kill(pid, 0)` every 100ms up to 20 times (2s). If stopped: print "Daemon stopped.", clean up files, return Ok. Else bail "Daemon did not stop within 2 seconds".
  - `print_status() -> Result<()>`: if not is_daemon_running: print "Daemon is not running.", return. Connect UnixStream to socket_path with 2s read timeout. BufReader::read_line. Parse AppState from JSON. Print state/target/mode/last_text.
  - `setup_daemon_logging() -> Result<()>`: open state_dir/daemon.log truncated. Init tracing_subscriber with file writer (Mutex), no ANSI, env filter defaulting to "info".
- [ ] Step 2: Add `pub mod lifecycle;` to `src/daemon/mod.rs`
- [ ] Step 3: In `src/main.rs`, add to Commands enum: `Daemon { #[command(subcommand)] action: DaemonAction }` and `Overlay`. Add `#[derive(Subcommand)] enum DaemonAction { Start, Stop, Status }`.
- [ ] Step 4: Add match arms in main(): `Daemon { Start }` — check is_daemon_running (bail if yes), call daemonize() (parent returns Ok), child: setup_daemon_logging, write_pid_file, load config, run_daemon(config).await. `Daemon { Stop }` — stop_daemon(). `Daemon { Status }` — print_status(). `Overlay` — print "not yet implemented".
- [ ] Step 5: Add foreground conflict guard at top of `Start` and `None` arms: `if daemon::is_daemon_running() { bail!("A daemon is already running...") }`
- [ ] Step 6: Register SIGTERM handler in daemon child process (after setup_daemon_logging, before run_daemon): use `tokio::signal::unix::signal(SignalKind::terminate())` to create a signal stream. Spawn a tokio task that awaits the signal, then calls `daemon::remove_pid_file()`, removes socket file, and calls `std::process::exit(0)`.
- [ ] Step 7: Add `use voice_terminal::{daemon, state, ui};` to main.rs imports
- [ ] Step 8: Run `cargo build` — compiles
- [ ] Step 9: Commit "feat: add daemon CLI commands (start/stop/status) with fork and signal handling"

### Task 5b: Daemon run loop with state broadcasting

**Files:** Modify `src/main.rs`.

- [ ] Step 1: Implement `async fn run_daemon(config: AppConfig) -> Result<()>`: verify/download model, init WhisperRecognizer + AudioCapture + TextInjector. Create SocketServer at daemon::socket_path(), get broadcaster, spawn accept_loop in std::thread. Spawn tokio task polling `ui::active_window::get_active_window_name()` every 500ms (store in Arc<Mutex<String>>). Create initial AppState (mode from config, state=Listening), broadcast. Match on config.activation.mode → call run_toggle_mode_daemon / run_vad_mode_daemon / run_wake_word_mode_daemon. On return: remove_pid_file, remove socket file, log shutdown.
- [ ] Step 2: Implement `async fn run_toggle_mode_daemon(config, recognizer, audio_capture, text_injector, broadcaster) -> Result<()>`: create HotkeyListener, start(), get rx. Init AppState (Listening, target_app from get_active_window_name(), mode from config). Broadcast. Enter loop on rx.recv(): on Activated → set state=Recording, update target_app, broadcast, start_recording(). On Deactivated → set state=Processing, broadcast, stop recording, transcribe, if text non-empty: set last_text, inject_text. Set state=Listening, update target_app, broadcast. On recv error → break.
- [ ] Step 3: Add stub `run_vad_mode_daemon` and `run_wake_word_mode_daemon` that delegate to non-daemon versions with comment "// State broadcasting deferred to follow-up"
- [ ] Step 4: Run `cargo build` — compiles (warnings OK for stubs)
- [ ] Step 5: Run `cargo test` — all suites pass
- [ ] Step 6: Commit "feat: add daemon run loop with socket state broadcasting"

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

- [ ] Step 1: Create `src/ui/overlay.rs` (all behind `#[cfg(feature = "gui")]`). Define `pub fn run_overlay(sock_path: &Path, opacity: f32, position: &str) -> Result<()>`. Create `Arc<Mutex<AppState>>` shared state. Spawn socket reader thread: loop { try UnixStream::connect(sock_path), on success wrap in BufReader, iterate lines(), parse each as AppState via from_framed_json, update shared state. On disconnect: log warning, sleep 2s, retry. }
- [ ] Step 2: In same file, set up eframe::NativeOptions: ViewportBuilder with inner_size(220,50), always_on_top(), decorations(false), transparent(true). Call eframe::run_native("Voice Terminal", options, closure creating OverlayApp).
- [ ] Step 3: Define `OverlayApp` struct holding `Arc<Mutex<AppState>>`. Impl `eframe::App`: `clear_color` returns [0,0,0,0] (transparent). `update`: lock state clone, request_repaint_after(200ms). Render CentralPanel with Frame::NONE.fill(rgba 30,30,30,200).rounding(8).inner_margin(8). Horizontal layout: allocate 12x12 rect, paint filled circle (color: gray=Idle, green=Listening, red=Recording, yellow=Processing), spacing 6px, emoji label (💤/👂/🎙️/⏳), "→ {target_app}" text in light gray size 13. When state is Recording, pulse the red circle opacity using `ctx.input(|i| i.time)` sine wave. When socket is disconnected (state.target_app == "Unknown" and state == Idle for >5s), show "disconnected" label.
- [ ] Step 4: Add `#[cfg(feature = "gui")] pub mod overlay;` to `src/ui/mod.rs`
- [ ] Step 5: Replace `Commands::Overlay` placeholder in main.rs: `#[cfg(feature = "gui")]` block loads config, gets socket_path, calls `ui::overlay::run_overlay()`. `#[cfg(not(feature = "gui"))]` block bails with message about rebuilding with gui feature.
- [ ] Step 6: Run `cargo build --release --features gui` — compiles
- [ ] Step 7: Run `cargo test` — all pass
- [ ] Step 8: Commit "feat: add floating egui overlay showing daemon state and target app"

---

## Chunk 5: Integration + Final Wiring

### Task 8: Auto-start overlay from daemon + end-to-end test

**Files:** Modify `src/main.rs`.

- [ ] Step 1: In `DaemonAction::Start` handler, after `write_pid_file()` and before `run_daemon()`, if `config.daemon.auto_start_overlay`: get `std::env::current_exe()`, build Command with arg "overlay", preserve env vars DISPLAY/WAYLAND_DISPLAY/XDG_RUNTIME_DIR. spawn(). On success: log "Overlay started with PID {id}". On failure: log warning (non-fatal).
- [ ] Step 2: Build release: `cargo build --release`
- [ ] Step 3: Test `cargo run --release -- daemon start` — forks, prints PID, returns to shell
- [ ] Step 4: Test `cargo run --release -- daemon status` — shows state
- [ ] Step 5: Test `cargo run --release -- overlay` — floating window appears in top-right
- [ ] Step 6: Test hotkey while daemon running — overlay transitions colors (green→red→yellow→green)
- [ ] Step 7: Test `cargo run --release -- daemon stop` — prints "Daemon stopped.", overlay disconnects
- [ ] Step 8: Test foreground conflict: `daemon start` then `start` → error message about daemon already running
- [ ] Step 9: Run `cargo test` — all pass
- [ ] Step 10: Commit "feat: auto-start overlay from daemon, integration verified"
