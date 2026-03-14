/// Get the name of the currently active (frontmost) application.
/// Returns "Unknown" if detection fails.
pub fn get_active_window_name() -> String {
    #[cfg(target_os = "macos")]
    {
        get_active_window_macos()
    }
    #[cfg(target_os = "linux")]
    {
        get_active_window_linux()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        "Unknown".to_string()
    }
}

#[cfg(target_os = "macos")]
fn get_active_window_macos() -> String {
    use std::process::Command;
    let output = Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get name of first application process whose frontmost is true",
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if name.is_empty() { "Unknown".to_string() } else { name }
        }
        _ => "Unknown".to_string(),
    }
}

#[cfg(target_os = "linux")]
fn get_active_window_linux() -> String {
    use std::process::Command;
    let output = Command::new("xdotool")
        .args(["getactivewindow", "getwindowname"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if name.is_empty() { "Unknown".to_string() } else { name }
        }
        _ => "Unknown".to_string(),
    }
}
