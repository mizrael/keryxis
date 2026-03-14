use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Activation mode for voice recording
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum ActivationMode {
    /// Press hotkey once to start, once to stop
    Toggle,
    /// Press hotkey once, recording stops on silence
    Vad,
    /// Always listening for a wake word
    WakeWord,
}

impl Default for ActivationMode {
    fn default() -> Self {
        Self::Toggle
    }
}

impl std::fmt::Display for ActivationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Toggle => write!(f, "toggle"),
            Self::Vad => write!(f, "vad"),
            Self::WakeWord => write!(f, "wake_word"),
        }
    }
}

/// Whisper model size
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum ModelSize {
    Tiny,
    Base,
    Small,
    Medium,
    Large,
}

impl Default for ModelSize {
    fn default() -> Self {
        Self::Base
    }
}

impl ModelSize {
    pub fn file_name(&self) -> &str {
        match self {
            Self::Tiny => "ggml-tiny.en.bin",
            Self::Base => "ggml-base.en.bin",
            Self::Small => "ggml-small.en.bin",
            Self::Medium => "ggml-medium.en.bin",
            Self::Large => "ggml-large.bin",
        }
    }

    pub fn huggingface_url(&self) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            self.file_name()
        )
    }
}

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub activation: ActivationConfig,
    pub whisper: WhisperConfig,
    pub vad: VadConfig,
    pub audio: AudioConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationConfig {
    pub mode: ActivationMode,
    /// Hotkey string, e.g. "Alt+Space" or "Ctrl+Shift+R"
    pub hotkey: String,
    /// Wake word phrase (used in WakeWord mode)
    pub wake_word: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperConfig {
    pub model_size: ModelSize,
    /// Path to model file (auto-resolved if not set)
    pub model_path: Option<PathBuf>,
    /// Language code (e.g., "en", "es", "auto")
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    /// Energy threshold for simple VAD (0.0 - 1.0)
    pub energy_threshold: f32,
    /// Silence duration (ms) before auto-stop
    pub silence_duration_ms: u64,
    /// Minimum speech duration (ms) to accept
    pub min_speech_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Sample rate for recording (Whisper expects 16000)
    pub sample_rate: u32,
    /// Number of channels (mono = 1)
    pub channels: u16,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            activation: ActivationConfig {
                mode: ActivationMode::Toggle,
                hotkey: "Alt+Space".to_string(),
                wake_word: "hey terminal".to_string(),
            },
            whisper: WhisperConfig {
                model_size: ModelSize::Base,
                model_path: None,
                language: "en".to_string(),
            },
            vad: VadConfig {
                energy_threshold: 0.01,
                silence_duration_ms: 1500,
                min_speech_duration_ms: 500,
            },
            audio: AudioConfig {
                sample_rate: 16000,
                channels: 1,
            },
        }
    }
}

impl AppConfig {
    /// Load configuration from default path or create default
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            let config: AppConfig = toml::from_str(&contents)?;
            Ok(config)
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save configuration to default path
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        tracing::info!("Configuration saved to {}", path.display());
        Ok(())
    }

    /// Get the configuration file path
    pub fn config_path() -> anyhow::Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("voice-terminal").join("config.toml"))
    }

    /// Get the data directory for models
    pub fn data_dir() -> anyhow::Result<PathBuf> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        Ok(data_dir.join("voice-terminal"))
    }

    /// Resolve the Whisper model path
    pub fn model_path(&self) -> anyhow::Result<PathBuf> {
        if let Some(ref path) = self.whisper.model_path {
            Ok(path.clone())
        } else {
            let data_dir = Self::data_dir()?;
            Ok(data_dir
                .join("models")
                .join(self.whisper.model_size.file_name()))
        }
    }
}
