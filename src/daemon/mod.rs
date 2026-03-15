pub mod lifecycle;
pub mod socket;

use std::path::{Path, PathBuf};

pub use socket::{Broadcaster, SocketServer};

/// Get the runtime state directory
pub fn state_dir() -> anyhow::Result<PathBuf> {
    let base = dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .ok_or_else(|| anyhow::anyhow!("Could not determine state directory"))?;
    let dir = base.join("voice-terminal");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Get the Unix socket path
pub fn socket_path() -> anyhow::Result<PathBuf> {
    Ok(state_dir()?.join("voice-terminal.sock"))
}

/// Get the PID file path
pub fn pid_file_path() -> anyhow::Result<PathBuf> {
    Ok(state_dir()?.join("daemon.pid"))
}

/// Check if a PID file points to a process that no longer exists
pub fn is_pid_stale(pid_path: &Path) -> bool {
    let contents = match std::fs::read_to_string(pid_path) {
        Ok(c) => c,
        Err(_) => return true,
    };
    let pid: i32 = match contents.trim().parse() {
        Ok(p) => p,
        Err(_) => return true,
    };
    // Check if process exists
    let alive = unsafe { libc::kill(pid, 0) == 0 };
    if !alive {
        return true;
    }
    // Verify the process is actually voice-terminal (avoid PID reuse)
    if !is_voice_terminal_process(pid) {
        return true;
    }
    false
}

#[cfg(target_os = "macos")]
fn is_voice_terminal_process(pid: i32) -> bool {
    use std::process::Command;
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let name = String::from_utf8_lossy(&out.stdout);
            name.trim().contains("voice-terminal")
        }
        _ => true, // can't verify, assume it's ours
    }
}

#[cfg(target_os = "linux")]
fn is_voice_terminal_process(pid: i32) -> bool {
    let cmdline_path = format!("/proc/{}/cmdline", pid);
    match std::fs::read_to_string(&cmdline_path) {
        Ok(cmdline) => cmdline.contains("voice-terminal"),
        Err(_) => true, // can't verify, assume it's ours
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn is_voice_terminal_process(_pid: i32) -> bool {
    true
}

/// Get the overlay PID file path
pub fn overlay_pid_file_path() -> anyhow::Result<PathBuf> {
    Ok(state_dir()?.join("overlay.pid"))
}

/// Check if an overlay is already running
pub fn is_overlay_running() -> bool {
    let pid_path = match overlay_pid_file_path() {
        Ok(p) => p,
        Err(_) => return false,
    };
    if !pid_path.exists() {
        return false;
    }
    !is_pid_stale(&pid_path)
}

/// Write the current process PID to the PID file
pub fn write_pid_file() -> anyhow::Result<()> {
    let path = pid_file_path()?;
    std::fs::write(&path, std::process::id().to_string())?;
    Ok(())
}

/// Remove the PID file
pub fn remove_pid_file() -> anyhow::Result<()> {
    let path = pid_file_path()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// Check if a daemon is already running
pub fn is_daemon_running() -> bool {
    let pid_path = match pid_file_path() {
        Ok(p) => p,
        Err(_) => return false,
    };
    if !pid_path.exists() {
        return false;
    }
    !is_pid_stale(&pid_path)
}
