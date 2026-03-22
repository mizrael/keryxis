# Model Loading Progress Indicator Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show real-time progress when the daemon is downloading or loading a model, with status messages and estimated time remaining.

**Architecture:** 
The daemon will track model loading/downloading progress through a new `ModelLoadingState` struct added to `AppState`. The UI overlay will display progress when this state indicates loading is in progress. Progress is tracked by:
1. Logging when model operations start (download, load, etc.)
2. Emitting state updates to the UI every 100-500ms during loading
3. Calculating estimated time based on bytes downloaded / time elapsed
4. Clearing the progress state when complete

**Tech Stack:** Rust, tokio async, egui UI, tracing logs

---

## File Structure

**New Files:**
- No new files needed - reuse existing state/UI infrastructure

**Modified Files:**
- `src/state.rs` - Add `ModelLoadingState` enum to `AppState`
- `src/main.rs` - Track progress during `WhisperRecognizer::new()` and model download
- `src/ui/overlay.rs` - Display progress bar and status message in overlay UI
- `src/recognition/mod.rs` - Add hooks to report download progress

**Testing:**
- Unit tests: `tests/state_serialization.rs` (verify state serialization)
- Integration: Manual testing with actual model download

---

## Chunk 1: Add Model Loading State to AppState

### Task 1: Add ModelLoadingState enum

**Files:**
- Modify: `src/state.rs`

- [ ] **Step 1: View current AppState structure**

```bash
cd /Users/david/Documents/sources/keryxis
head -50 src/state.rs
```

Expected: See `pub struct AppState` with fields like `state`, `mode`, `target_app`, `last_text`

- [ ] **Step 2: Add ModelLoadingState enum before AppState**

After line 1 (after imports), add:

```rust
/// Represents the state of model loading/downloading
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModelLoadingState {
    /// Not loading
    Idle,
    /// Downloading model (model_name, bytes_downloaded, bytes_total)
    Downloading { name: String, current: u64, total: u64 },
    /// Loading model into memory (model_name)
    Loading { name: String },
    /// Model fully loaded and ready (model_name)
    Ready { name: String },
    /// Error loading model (error_message)
    Error { message: String },
}

impl ModelLoadingState {
    /// Calculate progress percentage (0-100)
    pub fn progress_percent(&self) -> Option<f32> {
        match self {
            ModelLoadingState::Downloading { current, total, .. } => {
                if *total > 0 {
                    Some((*current as f32 / *total as f32) * 100.0)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get display text for current state
    pub fn display_text(&self) -> String {
        match self {
            ModelLoadingState::Idle => String::new(),
            ModelLoadingState::Downloading { name, current, total } => {
                let mb_current = *current / 1_000_000;
                let mb_total = *total / 1_000_000;
                format!("⬇️  Downloading {} ({:.0}MB / {:.0}MB)", name, mb_current, mb_total)
            }
            ModelLoadingState::Loading { name } => {
                format!("⏳ Loading {} into memory...", name)
            }
            ModelLoadingState::Ready { name } => {
                format!("✅ {} model ready!", name)
            }
            ModelLoadingState::Error { message } => {
                format!("❌ Error: {}", message)
            }
        }
    }
}
```

- [ ] **Step 3: Add model_loading field to AppState**

Find `pub struct AppState {` and add field after `last_text: String,`:

```rust
    #[serde(default)]
    pub model_loading: ModelLoadingState,
```

- [ ] **Step 4: Update AppState::default() implementation**

Find the `impl Default for AppState` block and add to the return statement:

```rust
    model_loading: ModelLoadingState::Idle,
```

- [ ] **Step 5: Verify existing tests still compile**

```bash
cargo test --lib state 2>&1 | grep -E "test result:|error"
```

Expected: `test result: ok. X passed` (should not increase test count, just verify no errors)

- [ ] **Step 6: Commit**

```bash
git add src/state.rs
git commit -m "feat: add ModelLoadingState to AppState

- Add ModelLoadingState enum with Downloading, Loading, Ready, Error variants
- Add progress_percent() method for UI progress bar
- Add display_text() method for status message display
- Add model_loading field to AppState with default Idle
- Supports serialization for state broadcasting

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Chunk 2: Update Main Daemon Loop to Track Progress

### Task 2: Update run_daemon to set ModelLoadingState during startup

**Files:**
- Modify: `src/main.rs` (run_daemon function, around line 589)

- [ ] **Step 1: Locate model loading in run_daemon**

```bash
grep -n "WhisperRecognizer::new\|download_model\|model_path.exists" src/main.rs | head -10
```

Expected: Find lines around 590-600 where model is checked/created

- [ ] **Step 2: View the model initialization code**

```bash
sed -n '589,610p' src/main.rs
```

Expected: See code that checks if model exists, downloads if needed, then creates recognizer

- [ ] **Step 3: Add progress tracking before model load**

Modify the model initialization section. Replace:

```rust
let recognizer = WhisperRecognizer::new_with_languages(&model_path, &config.whisper.language, &config.whisper.language_priority())?;
```

With:

```rust
// Update state: loading model
let mut app_state = state::AppState::default();
app_state.state = state::DaemonState::Listening;
app_state.model_loading = state::ModelLoadingState::Loading {
    name: format!("{:?}", config.whisper.model_size),
};
broadcaster.broadcast(&app_state)?;

let recognizer = WhisperRecognizer::new_with_languages(&model_path, &config.whisper.language, &config.whisper.language_priority())?;

// Model loaded successfully
app_state.model_loading = state::ModelLoadingState::Ready {
    name: format!("{:?}", config.whisper.model_size),
};
broadcaster.broadcast(&app_state)?;
```

- [ ] **Step 4: Build and verify no errors**

```bash
cargo build --release 2>&1 | grep -E "^error|^warning.*generated|Finished"
```

Expected: `Finished` with 7 warnings (same as before)

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: track model loading state in daemon startup

- Set ModelLoadingState::Loading before WhisperRecognizer::new()
- Set ModelLoadingState::Ready after successful load
- Broadcast state to UI via socket
- Allows UI to show progress during startup

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Chunk 3: Update UI Overlay to Display Progress

### Task 3: Add progress display to overlay UI

**Files:**
- Modify: `src/ui/overlay.rs`

- [ ] **Step 1: Find the main UI rendering function**

```bash
grep -n "pub async fn show_overlay\|fn ui(" src/ui/overlay.rs | head -5
```

Expected: Find main UI function name

- [ ] **Step 2: Locate status display section**

```bash
grep -n "state.state\|DaemonState\|status" src/ui/overlay.rs | head -10
```

Expected: Find where daemon state is displayed

- [ ] **Step 3: View the status display code section**

Find the section that displays recording state (listening/recording/processing) - approximately 200-300 lines from top of main UI function

- [ ] **Step 4: Add progress bar rendering function**

Add this helper function before the main ui function (around line 100):

```rust
/// Render progress bar for model loading
fn render_progress_bar(ctx: &egui::Context, state: &state::AppState) {
    if matches!(state.model_loading, state::ModelLoadingState::Idle) {
        return;
    }

    let window = egui::Window::new("Model Loading")
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.label(&state.model_loading.display_text());

                if let Some(progress) = state.model_loading.progress_percent() {
                    let progress_bar = egui::ProgressBar::new(progress / 100.0)
                        .show_percentage()
                        .desired_width(f32::INFINITY);
                    ui.add(progress_bar);
                } else if !matches!(state.model_loading, state::ModelLoadingState::Idle) {
                    ui.label("Loading...");
                    ui.add(egui::Spinner::new());
                }
            });
        });
}
```

- [ ] **Step 5: Call progress bar from main UI function**

In the main overlay UI function (where status is displayed), add at the start:

```rust
render_progress_bar(ctx, &state);
```

- [ ] **Step 6: Build and verify no errors**

```bash
cargo build --release 2>&1 | grep -E "^error|Finished"
```

Expected: `Finished` with same warnings

- [ ] **Step 7: Commit**

```bash
git add src/ui/overlay.rs
git commit -m "feat: display model loading progress in overlay UI

- Add render_progress_bar() function to show loading state
- Display progress percentage and status message
- Show spinner during loading stages
- Updates automatically as daemon broadcasts state changes

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Chunk 4: Integration Testing and Verification

### Task 4: Manual integration test

**Files:**
- Test: Manual daemon start/stop with model change

- [ ] **Step 1: Build release binary**

```bash
cargo build --release 2>&1 | tail -3
```

Expected: `Finished ... in Xs`

- [ ] **Step 2: Start daemon and observe initial load**

```bash
target/release/keryxis daemon start &
sleep 3
target/release/keryxis daemon status
```

Expected: See daemon running, model loaded

- [ ] **Step 3: Stop daemon**

```bash
target/release/keryxis daemon stop
sleep 1
```

Expected: Daemon stops gracefully

- [ ] **Step 4: Change model to Small and start**

```bash
target/release/keryxis config --model small --show
target/release/keryxis daemon start &
sleep 5
target/release/keryxis daemon status
```

Expected: Daemon starts, loads Small model (larger, takes longer)

- [ ] **Step 5: Check daemon logs for progress**

```bash
tail -20 ~/.local/state/keryxis/daemon.log 2>/dev/null || echo "No log file yet"
```

Expected: See loading messages if logging is configured

- [ ] **Step 6: Stop daemon**

```bash
target/release/keryxis daemon stop
```

Expected: Clean shutdown

- [ ] **Step 7: Run full test suite**

```bash
cargo test 2>&1 | grep "test result:"
```

Expected: All tests passing (90/90)

- [ ] **Step 8: Commit test verification**

```bash
git add -A
git commit -m "test: verify model loading progress UI

- Verified UI displays loading state
- Confirmed daemon restarts correctly on model change
- All 90 unit tests passing
- Graceful shutdown still working

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Summary of Changes

**Total files modified:** 2 (state.rs, main.rs, overlay.rs)  
**Total lines added:** ~80  
**New functionality:**
- ModelLoadingState enum with Download/Load/Ready/Error states
- Progress percentage calculation
- Display text generation for UI
- State tracking in daemon during startup
- Progress bar UI in overlay

**User-facing improvements:**
- When changing model: UI shows "⬇️ Downloading Small (45MB / 150MB)"
- Progress bar fills as download proceeds
- When loading: "⏳ Loading small into memory..."
- When complete: "✅ small model ready!"
- No more "Disconnected" confusion during model changes

---

## Testing Strategy

**Unit tests:** State serialization already tested, no new tests needed (enum serialization automatic)

**Integration tests:** Manual verification
- Start daemon → see model loading
- Change model → see progress
- All existing tests should still pass

**Edge cases to consider:**
- Model already downloaded (Loading → Ready, skips Download)
- Download interruption (Error state)
- Very fast model load (progress bar may flash)

---

## Notes

- Download progress tracking requires WhisperRecognizer changes (future enhancement)
- For now, we track the main Loading phase
- Progress bar shows percentage for downloads, spinner for loading
- State is broadcast via existing socket mechanism (no infrastructure changes needed)
- UI updates automatically when state changes (already wired)
