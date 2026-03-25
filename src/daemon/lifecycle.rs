use anyhow::Result;
use std::io::{BufRead, BufReader};

/// Start the daemon as a detached background process (with overlay).
pub fn start_daemon() -> Result<()> {
    spawn_daemon(false)
}

/// Restart the daemon without spawning a new overlay (overlay stays alive).
pub fn restart_daemon() -> Result<()> {
    let _ = stop_daemon_process();
    // Poll until old daemon exits (up to 3s)
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if !super::is_daemon_running() {
            break;
        }
    }
    // Extra delay to let the OS release the TCP port
    std::thread::sleep(std::time::Duration::from_millis(500));
    spawn_daemon(true)
}

/// Start the daemon process without spawning a new overlay (keeps existing overlay alive).
/// Used by overlay for starting daemon from UI button without creating duplicate overlay.
pub fn start_daemon_process_only() -> Result<()> {
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
        return Ok(());
    }

    if super::is_pid_stale(&pid_path) {
        let _ = std::fs::remove_file(&pid_path);
        #[cfg(unix)]
        {
            let sock = super::socket_path()?;
            if sock.exists() {
                let _ = std::fs::remove_file(&sock);
            }
        }
        return Ok(());
    }

    let pid: u32 = std::fs::read_to_string(&pid_path)?.trim().parse()?;

    super::terminate_process(pid);

    // Wait up to 2 seconds for process to exit
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        #[cfg(unix)]
        {
            if unsafe { libc::kill(pid as i32, 0) } != 0 {
                let _ = std::fs::remove_file(&pid_path);
                let sock = super::socket_path()?;
                if sock.exists() {
                    let _ = std::fs::remove_file(&sock);
                }
                return Ok(());
            }
        }
        #[cfg(windows)]
        {
            if !super::is_pid_stale(&pid_path) {
                continue;
            }
            let _ = std::fs::remove_file(&pid_path);
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
        if let Ok(pid) = contents.trim().parse::<u32>() {
            super::terminate_process(pid);
        }
    }
    let _ = std::fs::remove_file(&overlay_pid_path);
}

/// Print daemon status by connecting to the IPC endpoint
pub fn print_status(#[cfg(unix)] _port: u16, #[cfg(windows)] port: u16) -> Result<()> {
    if !super::is_daemon_running() {
        println!("Daemon is not running.");
        return Ok(());
    }

    #[cfg(unix)]
    let stream = {
        let sock_path = super::socket_path()?;
        if !sock_path.exists() {
            println!("Daemon is running but socket not found.");
            return Ok(());
        }
        std::os::unix::net::UnixStream::connect(&sock_path)?
    };

    #[cfg(windows)]
    let stream = {
        match std::net::TcpStream::connect(("127.0.0.1", port)) {
            Ok(s) => s,
            Err(_) => {
                println!("Daemon is running but could not connect to IPC.");
                return Ok(());
            }
        }
    };

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
