use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::time::SystemTime;

/// Represents the current state of the speech-to-text daemon
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonState {
    Idle,
    Listening,
    Recording,
    Processing,
}

impl Display for DaemonState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DaemonState::Idle => write!(f, "idle"),
            DaemonState::Listening => write!(f, "listening"),
            DaemonState::Recording => write!(f, "recording"),
            DaemonState::Processing => write!(f, "processing"),
        }
    }
}

/// Application state that is broadcast to connected clients
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppState {
    pub state: DaemonState,
    pub target_app: String,
    pub mode: String,
    pub last_text: String,
    pub timestamp: u64,
}

impl AppState {
    /// Serialize to JSON and add newline framing
    pub fn to_framed_json(&self) -> Result<String, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(format!("{}\n", json))
    }

    /// Deserialize from a framed JSON line
    pub fn from_framed_json(line: &str) -> Result<Self, serde_json::Error> {
        let trimmed = line.trim_end_matches('\n');
        serde_json::from_str(trimmed)
    }
}

impl Default for AppState {
    fn default() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            state: DaemonState::Idle,
            target_app: "Unknown".to_string(),
            mode: "toggle".to_string(),
            last_text: String::new(),
            timestamp,
        }
    }
}