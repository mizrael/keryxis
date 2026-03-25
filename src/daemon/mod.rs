pub mod lifecycle;
pub mod socket;

use std::path::{Path, PathBuf};

pub use socket::{Broadcaster, SocketServer};

/// Get the runtime state directory
pub fn state_dir() -> anyhow::Result<PathBuf> {
    let base = dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .ok_or_else(|| anyhow::anyhow!("Could not determine state directory"))?;
    let dir = base.join("keryxis");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Get the Unix socket path (used on Unix only; Windows uses TCP)
#[cfg(unix)]
pub fn socket_path() -> anyhow::Result<PathBuf> {
    Ok(state_dir()?.join("keryxis.sock"))
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
    let pid: u32 = match contents.trim().parse() {
        Ok(p) => p,
        Err(_) => return true,
    };
    let alive = is_process_alive(pid);
    if !alive {
        return true;
    }
    if !is_keryxis_process(pid) {
        return true;
    }
    false
}

#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(windows)]
fn is_process_alive(pid: u32) -> bool {
    use std::process::Command;
    Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output()
        .map(|o| {
            let out = String::from_utf8_lossy(&o.stdout);
            out.contains(&pid.to_string())
        })
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn is_keryxis_process(pid: u32) -> bool {
    use std::process::Command;
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let name = String::from_utf8_lossy(&out.stdout);
            name.trim().contains("keryxis")
        }
        _ => true,
    }
}

#[cfg(target_os = "linux")]
fn is_keryxis_process(pid: u32) -> bool {
    let cmdline_path = format!("/proc/{}/cmdline", pid);
    match std::fs::read_to_string(&cmdline_path) {
        Ok(cmdline) => cmdline.contains("keryxis"),
        Err(_) => true,
    }
}

#[cfg(windows)]
fn is_keryxis_process(pid: u32) -> bool {
    use std::process::Command;
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let name = String::from_utf8_lossy(&out.stdout);
            name.to_lowercase().contains("keryxis")
        }
        _ => true,
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn is_keryxis_process(_pid: u32) -> bool {
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

/// Terminate a process by PID (cross-platform)
pub fn terminate_process(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output();
    }
}
