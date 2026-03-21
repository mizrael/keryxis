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

/// Represents the state of model loading/downloading
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelLoadingState {
    /// Not loading
    Idle,
    /// Downloading model (model_name, bytes_downloaded, bytes_total)
    Downloading {
        name: String,
        current: u64,
        total: u64,
    },
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
                format!(
                    "⬇️  Downloading {} ({:.0}MB / {:.0}MB)",
                    name, mb_current, mb_total
                )
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

impl Default for ModelLoadingState {
    fn default() -> Self {
        ModelLoadingState::Idle
    }
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
    #[serde(default)]
    pub model_loading: ModelLoadingState,
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
            model_loading: ModelLoadingState::Idle,
        }
    }
}