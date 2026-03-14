use anyhow::Result;
use std::path::{Path, PathBuf};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::config::ModelSize;

/// Wrapper around whisper-rs for speech-to-text
pub struct WhisperRecognizer {
    ctx: WhisperContext,
    language: String,
}

impl WhisperRecognizer {
    /// Create a new recognizer by loading the specified model
    pub fn new(model_path: &Path, language: &str) -> Result<Self> {
        tracing::info!("Loading Whisper model from: {}", model_path.display());

        if !model_path.exists() {
            anyhow::bail!(
                "Model file not found: {}. Run `voice-terminal download-model` first.",
                model_path.display()
            );
        }

        let ctx = WhisperContext::new_with_params(
            model_path.to_str().unwrap(),
            WhisperContextParameters::default(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to load Whisper model: {:?}", e))?;

        tracing::info!("Whisper model loaded successfully");

        Ok(Self {
            ctx,
            language: language.to_string(),
        })
    }

    /// Transcribe audio samples (16kHz mono f32) to text
    pub fn transcribe(&self, samples: &[f32]) -> Result<String> {
        if samples.is_empty() {
            return Ok(String::new());
        }

        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| anyhow::anyhow!("Failed to create Whisper state: {:?}", e))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        if self.language == "auto" {
            params.set_language(Some("auto"));
        } else {
            params.set_language(Some(&self.language));
        }
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        params.set_single_segment(true);
        // Use 1 thread for tiny model speed
        params.set_n_threads(4);

        tracing::debug!("Transcribing {} samples...", samples.len());

        state
            .full(params, samples)
            .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {:?}", e))?;

        let num_segments = state.full_n_segments()?;
        let mut text = String::new();

        for i in 0..num_segments {
            if let Ok(segment_text) = state.full_get_segment_text(i) {
                text.push_str(&segment_text);
            }
        }

        let text = text.trim().to_string();
        tracing::info!("Transcribed: \"{}\"", text);
        Ok(text)
    }

    /// Download a Whisper model if not already present
    pub async fn download_model(model_size: &ModelSize, target_dir: &Path) -> Result<PathBuf> {
        let file_name = model_size.file_name();
        let target_path = target_dir.join(file_name);

        if target_path.exists() {
            tracing::info!("Model already exists: {}", target_path.display());
            return Ok(target_path);
        }

        std::fs::create_dir_all(target_dir)?;

        let url = model_size.huggingface_url();
        tracing::info!("Downloading model from: {}", url);
        println!("Downloading {} model... This may take a while.", file_name);

        // Use curl for downloading since we don't want to add reqwest as a dependency
        let output = tokio::process::Command::new("curl")
            .args(["-L", "-o", target_path.to_str().unwrap(), &url, "--progress-bar"])
            .status()
            .await?;

        if !output.success() {
            anyhow::bail!("Failed to download model from {}", url);
        }

        tracing::info!("Model downloaded to: {}", target_path.display());
        println!("Model downloaded successfully!");

        Ok(target_path)
    }
}
