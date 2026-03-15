use anyhow::Result;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;

/// Start the daemon as a detached background process (with overlay).
pub fn start_daemon() -> Result<()> {
    spawn_daemon(false)
}

/// Restart the daemon without spawning a new overlay (overlay stays alive).
pub fn restart_daemon() -> Result<()> {
    let _ = stop_daemon_process();
    std::thread::sleep(std::time::Duration::from_millis(300));
    spawn_daemon(true)
}

fn spawn_daemon(no_overlay: bool) -> Result<()> {
    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("daemon-run");
    if no_overlay {
        cmd.arg("--no-overlay");
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let child = cmd.spawn()?;
    println!("Daemon started with PID {}", child.id());
    Ok(())
}

/// Stop the running daemon and its overlay
pub fn stop_daemon() -> Result<()> {
    stop_daemon_process()?;
    stop_overlay();
    Ok(())
}

/// Stop only the daemon process (keep overlay alive). Used by overlay for restart.
pub fn stop_daemon_process() -> Result<()> {
    let pid_path = super::pid_file_path()?;

    if !pid_path.exists() {
        // Not an error — daemon may have already exited
        return Ok(());
    }

    if super::is_pid_stale(&pid_path) {
        let _ = std::fs::remove_file(&pid_path);
        let sock = super::socket_path()?;
        if sock.exists() {
            let _ = std::fs::remove_file(&sock);
        }
        return Ok(());
    }

    let pid: i32 = std::fs::read_to_string(&pid_path)?.trim().parse()?;

    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }

    // Wait up to 2 seconds for process to exit
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if unsafe { libc::kill(pid, 0) } != 0 {
            let _ = std::fs::remove_file(&pid_path);
            let sock = super::socket_path()?;
            if sock.exists() {
                let _ = std::fs::remove_file(&sock);
            }
            return Ok(());
        }
    }

    anyhow::bail!("Daemon did not stop within 2 seconds (PID {})", pid);
}

/// Stop the overlay process if running
fn stop_overlay() {
    let overlay_pid_path = match super::overlay_pid_file_path() {
        Ok(p) => p,
        Err(_) => return,
    };
    if !overlay_pid_path.exists() {
        return;
    }
    if let Ok(contents) = std::fs::read_to_string(&overlay_pid_path) {
        if let Ok(pid) = contents.trim().parse::<i32>() {
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
        }
    }
    let _ = std::fs::remove_file(&overlay_pid_path);
}

/// Print daemon status by connecting to the socket
pub fn print_status() -> Result<()> {
    if !super::is_daemon_running() {
        println!("Daemon is not running.");
        return Ok(());
    }

    let sock_path = super::socket_path()?;
    if !sock_path.exists() {
        println!("Daemon is running but socket not found.");
        return Ok(());
    }

    let stream = UnixStream::connect(&sock_path)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    match reader.read_line(&mut line) {
        Ok(0) | Err(_) => {
            println!("Daemon is running (no state received).");
        }
        Ok(_) => {
            let state: crate::state::AppState = serde_json::from_str(line.trim())?;
            println!("Daemon is running:");
            println!("  State:  {}", state.state);
            println!("  Target: {}", state.target_app);
            println!("  Mode:   {}", state.mode);
            if !state.last_text.is_empty() {
                println!("  Last:   \"{}\"", state.last_text);
            }
        }
    }

    Ok(())
}

/// Set up logging to a file (for daemon mode where stdout is unavailable)
pub fn setup_daemon_logging() -> Result<()> {
    let log_path = super::state_dir()?.join("daemon.log");

    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)?;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::sync::Mutex::new(file))
        .with_ansi(false)
        .init();

    tracing::info!("Daemon logging initialized to {}", log_path.display());
    Ok(())
}
