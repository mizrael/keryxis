# Design: Daemon Start/Stop Controls in UI Settings

## Overview

Add Start/Stop buttons to the Settings panel in the overlay UI, allowing users to manually control the daemon without using the CLI. This addresses the UX gap where users couldn't restart or stop the daemon after model changes without terminal access.

## Problem

When users change settings (especially model size), the daemon restarts automatically. However, if they want to manually stop/start the daemon or restart after testing, they must use the CLI. The overlay already shows daemon status, but has no control.

## Solution

Integrate daemon lifecycle control into the Settings panel, right next to the existing "Daemon running/stopped" status indicator.

## User Experience

### Layout

**Settings Panel Header:**
```
⚙ Settings                    🟢 Daemon running
                              [Stop Daemon]
```

When stopped:
```
⚙ Settings                    🔴 Daemon stopped
                              [Start Daemon]
```

### Interaction Flow

1. User opens Settings (click ⚙)
2. Sees daemon status with action button
3. Clicks "Stop Daemon"
4. Button disables, shows "Stopping..." (visual feedback)
5. Console logs: "Stopping daemon..."
6. After operation completes (~0.5-2 seconds):
   - Console shows: "Daemon stopped successfully" or error message
   - UI updates: red dot, "Daemon stopped"
   - Button re-enables, changes to "Start Daemon"
7. User can click "Start Daemon" to restart

## Implementation

### State Management

**OverlayApp struct changes:**
- Add `daemon_control_pending: bool` — tracks if a start/stop operation is in progress
- Add `daemon_control_error: Option<String>` — stores error message if operation fails

### UI Rendering

**Location:** In the Settings panel header, right side, next to daemon status indicator

**Button logic:**
```
if connected:
    button_text = "Stop Daemon"
    button_color = red
else:
    button_text = "Start Daemon"
    button_color = green

if daemon_control_pending:
    button_text = "Stopping..." or "Starting..."
    button_disabled = true
```

### Backend Operations

**Thread spawning:** When user clicks button:
1. Set `daemon_control_pending = true` to disable button
2. Determine action (Start or Stop) based on `connected` flag
3. Spawn background thread:
   ```rust
   std::thread::spawn(|| {
       match action {
           Start => {
               println!("Starting daemon...");
               match crate::daemon::lifecycle::start_daemon() {
                   Ok(_) => println!("Daemon started successfully"),
                   Err(e) => eprintln!("Failed to start daemon: {}", e),
               }
           }
           Stop => {
               println!("Stopping daemon...");
               match crate::daemon::lifecycle::stop_daemon() {
                   Ok(_) => println!("Daemon stopped successfully"),
                   Err(e) => eprintln!("Failed to stop daemon: {}", e),
               }
           }
       }
       // Flag is checked in next ui.update() call
   })
   ```
4. In next `update()` call, check if thread is done (use Arc<Mutex<bool>> or similar)
5. When done: clear `daemon_control_pending`, update UI

**Error handling:** If operation fails, show error in console. Daemon status will eventually reflect actual state once connection updates.

**Reconnection:** Existing socket connection logic handles reconnection when daemon comes online, so UI automatically updates.

## Testing

### Manual Tests
1. Daemon running, click "Stop Daemon"
   - Button disables
   - Console shows "Stopping daemon..."
   - Within 2 seconds: daemon stops, dot turns red, button enables as "Start Daemon"
   
2. Daemon stopped, click "Start Daemon"
   - Button disables
   - Console shows "Starting daemon..."
   - Within 5 seconds: daemon starts, dot turns green, button enables as "Stop Daemon"

3. Rapid clicks (user stress test)
   - Only first click during `pending` state is honored
   - Button stays disabled until operation completes

4. Settings applied during daemon stop
   - Stop daemon
   - Make setting changes (mode, model, hotkey)
   - Click Save (auto-restarts daemon)
   - All settings apply correctly

## Files Modified

- `src/ui/overlay.rs` — Add daemon_control_pending field, button rendering, thread spawning
- Tests updated: `tests/` if needed

## Assumptions & Constraints

- Users have permission to start/stop daemon (typically true for local user)
- Daemon lifecycle functions already exist and work correctly (verified in graceful shutdown work)
- Socket connection auto-updates when daemon state changes (existing behavior)
- Target performance: Start/stop completes in <5 seconds (typical)
