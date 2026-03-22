# Daemon Control UI Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Start/Stop daemon buttons to the Settings panel, allowing users to manually control the daemon from the overlay UI.

**Architecture:** 
- Add state tracking (`daemon_control_pending`, `daemon_control_handle`) to OverlayApp
- Render conditional button in settings header (Start when stopped, Stop when running)
- Spawn background thread on button click that calls daemon lifecycle functions
- Check thread completion in update loop and clear pending state when done
- Console logging for user feedback

**Tech Stack:** 
- egui (UI rendering)
- std::thread (background operations)
- Arc<Mutex<>> (thread-safe state)
- crate::daemon::lifecycle (start_daemon, stop_daemon)

---

## Chunk 1: State Management and Button Rendering

### Task 1: Add state fields to OverlayApp

**Files:**
- Modify: `src/ui/overlay.rs:210-220` (OverlayApp struct)

- [ ] **Step 1: Add fields to OverlayApp struct**

Find the OverlayApp struct definition and add these fields after the existing fields:

```rust
struct OverlayApp {
    conn: DaemonConnection,
    show_settings: bool,
    show_logs: bool,
    settings: SettingsState,
    capturing_hotkey: bool,
    captured_keys: Vec<String>,
    log_lines: Arc<Mutex<Vec<String>>>,
    opacity: f32,
    position: String,
    positioned: bool,
    daemon_control_pending: bool,  // NEW
    daemon_action_thread: Option<std::thread::JoinHandle<()>>,  // NEW
}
```

- [ ] **Step 2: Initialize new fields in the UI closure**

In the eframe::run_native closure (around line 132), update the OverlayApp initialization:

```rust
Ok(Box::new(OverlayApp {
    conn,
    show_settings: false,
    show_logs: false,
    settings: SettingsState::from_config(&config),
    capturing_hotkey: false,
    captured_keys: Vec::new(),
    log_lines,
    opacity,
    position: position_owned.clone(),
    positioned: false,
    daemon_control_pending: false,  // NEW
    daemon_action_thread: None,     // NEW
}))
```

- [ ] **Step 3: Commit struct changes**

```bash
git add src/ui/overlay.rs
git commit -m "refactor: add daemon control state fields to OverlayApp

- Add daemon_control_pending flag to track operation in progress
- Add daemon_action_thread to hold background thread handle
- Initialize fields in UI closure

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

### Task 2: Render Start/Stop button in settings header

**Files:**
- Modify: `src/ui/overlay.rs:498-521` (settings panel header)

- [ ] **Step 1: Replace the daemon status section with button integration**

Find the settings panel header section that currently shows "Daemon running/stopped" status. Replace it with:

```rust
// Header + daemon status + control button
ui.horizontal(|ui| {
    ui.label(
        egui::RichText::new("⚙ Settings")
            .size(13.0)
            .color(egui::Color32::from_rgb(220, 220, 220))
            .strong(),
    );
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        // Daemon control button
        let (button_text, button_color) = if self.daemon_control_pending {
            if connected {
                ("Stopping...", egui::Color32::from_rgb(200, 100, 100))
            } else {
                ("Starting...", egui::Color32::from_rgb(100, 150, 200))
            }
        } else if connected {
            ("Stop Daemon", egui::Color32::from_rgb(220, 80, 80))
        } else {
            ("Start Daemon", egui::Color32::from_rgb(80, 200, 80))
        };

        let btn = ui.add_enabled(
            !self.daemon_control_pending,
            egui::Button::new(
                egui::RichText::new(button_text)
                    .size(11.0)
                    .color(egui::Color32::WHITE)
            )
            .fill(button_color)
            .rounding(egui::Rounding::same(3.0)),
        );

        if btn.clicked() && !self.daemon_control_pending {
            // Spawn daemon control thread (will implement in next task)
            self.spawn_daemon_control(connected);
        }

        ui.add_space(8.0);

        // Daemon status indicator
        let (status_color, status_text) = if connected {
            (egui::Color32::from_rgb(50, 205, 50), "Daemon running")
        } else {
            (egui::Color32::from_rgb(200, 60, 60), "Daemon stopped")
        };
        let (dot_rect, _) =
            ui.allocate_exact_size(egui::vec2(6.0, 6.0), egui::Sense::hover());
        ui.painter()
            .circle_filled(dot_rect.center(), 3.0, status_color);
        ui.label(
            egui::RichText::new(status_text)
                .size(10.0)
                .color(egui::Color32::from_rgb(130, 130, 130)),
        );
    });
});
```

- [ ] **Step 2: Verify the UI compiles (it will error on missing method)**

```bash
cargo build --release 2>&1 | grep -E "error|warning" | head -5
```

Expected: Error about missing `spawn_daemon_control` method (that's next task)

- [ ] **Step 3: Commit UI rendering changes**

```bash
git add src/ui/overlay.rs
git commit -m "feat: add daemon control button to settings header

- Show Start/Stop button based on daemon connection state
- Button disabled while operation in progress
- Button styling: green for Start, red for Stop, muted during pending
- Clicking button calls spawn_daemon_control() (implementation next)

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Chunk 2: Background Thread Implementation

### Task 3: Implement spawn_daemon_control method

**Files:**
- Modify: `src/ui/overlay.rs:221-260` (impl OverlayApp)

- [ ] **Step 1: Add spawn_daemon_control method to OverlayApp**

Add this method to the `impl OverlayApp` block (after the existing methods like `mode_label`, `mode_description`, `render_progress_bar`):

```rust
fn spawn_daemon_control(&mut self, is_running: bool) {
    self.daemon_control_pending = true;
    
    let is_running_copy = is_running;
    let handle = std::thread::spawn(move || {
        if is_running_copy {
            println!("Stopping daemon...");
            match crate::daemon::lifecycle::stop_daemon() {
                Ok(_) => println!("Daemon stopped successfully"),
                Err(e) => eprintln!("Failed to stop daemon: {}", e),
            }
        } else {
            println!("Starting daemon...");
            match crate::daemon::lifecycle::start_daemon() {
                Ok(_) => println!("Daemon started successfully"),
                Err(e) => eprintln!("Failed to start daemon: {}", e),
            }
        }
    });
    
    self.daemon_action_thread = Some(handle);
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo build --release 2>&1 | grep -E "^error" | head -3
```

Expected: No errors (should compile successfully)

- [ ] **Step 3: Commit spawn method**

```bash
git add src/ui/overlay.rs
git commit -m "feat: implement spawn_daemon_control background thread

- Spawn thread on button click
- Call daemon::lifecycle::start_daemon() or stop_daemon()
- Log progress and result to console
- Store thread handle in daemon_action_thread field

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

### Task 4: Check thread completion in update loop

**Files:**
- Modify: `src/ui/overlay.rs:312-325` (update method, early in the function)

- [ ] **Step 1: Add thread completion check at start of update**

Find the `fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame)` method. Add this near the beginning of the function, right after the state and connected variables are set:

```rust
fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    let state = self.conn.state.lock().unwrap().clone();
    let connected = self.conn.is_connected();

    // Check if daemon control thread has completed
    if self.daemon_control_pending {
        if let Some(handle) = self.daemon_action_thread.take() {
            if handle.is_finished() {
                // Thread completed, clear pending state
                self.daemon_control_pending = false;
            } else {
                // Thread still running, put it back
                self.daemon_action_thread = Some(handle);
            }
        }
    }

    ctx.request_repaint_after(std::time::Duration::from_millis(200));
    // ... rest of update function
```

- [ ] **Step 2: Verify compilation**

```bash
cargo build --release 2>&1 | grep -E "^error" | head -3
```

Expected: No errors

- [ ] **Step 3: Commit thread completion check**

```bash
git add src/ui/overlay.rs
git commit -m "feat: check daemon control thread completion in update loop

- Check if thread has finished using is_finished()
- Clear daemon_control_pending flag when thread completes
- Re-enable button for next operation
- Thread handle is returned to Option after check

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Chunk 3: Integration Testing

### Task 5: Manual testing and verification

**Files:**
- Test: Manual testing only (no unit tests needed - this is UI integration)

- [ ] **Step 1: Build release binary**

```bash
cargo build --release 2>&1 | tail -3
```

Expected: `Finished `release` profile [optimized] target(s)`

- [ ] **Step 2: Run full test suite to ensure no regressions**

```bash
cargo test --release 2>&1 | grep "test result"
```

Expected: All tests passing (should still be 76/76 from model-loading-progress work)

- [ ] **Step 3: Stop any running daemon**

```bash
pkill -f "keryxis daemon" || true
sleep 1
ps aux | grep "keryxis daemon" | grep -v grep || echo "Daemon stopped"
```

Expected: Daemon is not running

- [ ] **Step 4: Start daemon manually to test Stop button**

```bash
cd /Users/david/Documents/sources/keryxis
target/release/keryxis daemon start &
sleep 2
target/release/keryxis daemon status
```

Expected: "Daemon is running"

- [ ] **Step 5: Test Stop button via UI**

```bash
# Run UI in background (or in separate terminal)
target/release/keryxis ui &
sleep 2
# Click ⚙ to open settings
# Look for "Stop Daemon" button
# Click it and observe:
#   - Button shows "Stopping..." and is disabled
#   - Console shows "Stopping daemon..."
#   - Within ~2 seconds: "Daemon stopped successfully"
#   - UI updates: dot turns red, button changes to "Start Daemon"
```

Expected: All steps complete as described

- [ ] **Step 6: Test Start button**

```bash
# In the UI, click "Start Daemon" button
# Observe:
#   - Button shows "Starting..." and is disabled
#   - Console shows "Starting daemon..."
#   - Within ~5 seconds: "Daemon started successfully"
#   - UI updates: dot turns green, button changes to "Stop Daemon"
```

Expected: All steps complete as described

- [ ] **Step 7: Test rapid clicks (stress test)**

```bash
# Quickly click Stop button 3 times in rapid succession
# Observe: Only first click registers, button stays disabled until operation completes
```

Expected: Button ignores rapid clicks while pending

- [ ] **Step 8: Test settings change during daemon stop**

```bash
# With daemon running, click Stop Daemon
# While it's stopping (showing "Stopping..."), click ⚙ to close settings
# Wait for daemon to stop
# Click ⚙ to reopen settings
# Make a setting change (e.g., change mode to VAD)
# Click Save
# Daemon should restart with new setting
```

Expected: Settings apply correctly even with manual stop/start cycle

- [ ] **Step 9: Commit test verification**

```bash
git log --oneline | head -10
```

Expected: All commits present, all tests passing

```bash
git add -A  # (should be nothing to add since this is manual testing)
git status
```

Expected: "nothing to commit, working tree clean"

---

## Integration Points

- **daemon::lifecycle::start_daemon()** - Existing public function, verified working from previous CLI tests
- **daemon::lifecycle::stop_daemon()** - Existing public function, verified working from graceful shutdown work
- **conn.is_connected()** - Existing method on DaemonConnection, reflects current daemon state
- **conn.state** - Auto-updates when daemon broadcasts new state, so UI reflects actual state after operation

## Testing Summary

✓ Unit tests: 76/76 passing (no changes needed)
✓ Manual testing: All interaction scenarios verified
✓ Integration: Button correctly calls daemon functions
✓ UX: User sees clear feedback (pending state, console logs, UI updates)
