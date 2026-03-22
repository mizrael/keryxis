use anyhow::Result;
use std::path::{Path, PathBuf};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::config::ModelSize;

/// Wrapper around whisper-rs for speech-to-text
pub struct WhisperRecognizer {
    ctx: WhisperContext,
    language: String,
    languages: Vec<String>,
}

impl WhisperRecognizer {
    /// Create a new recognizer by loading the specified model
    pub fn new(model_path: &Path, language: &str) -> Result<Self> {
        Self::new_with_languages(model_path, language, &[])
    }

    /// Create a recognizer with a priority list of languages
    pub fn new_with_languages(model_path: &Path, language: &str, languages: &[String]) -> Result<Self> {
        tracing::info!("Loading Whisper model from: {}", model_path.display());

        if !model_path.exists() {
            anyhow::bail!(
                "Model file not found: {}. Run `keryxis download-model` first.",
                model_path.display()
            );
        }

        let mut params = WhisperContextParameters::default();

        // Disable GPU on non-Apple-Silicon Macs: the ggml Metal backend
        // crashes (GGML_ASSERT) on discrete AMD/Intel GPUs without unified memory.
        #[cfg(target_os = "macos")]
        {
            if !Self::has_apple_silicon() {
                tracing::info!("Non-Apple-Silicon Mac detected, using CPU for inference");
                params.use_gpu(false);
            }
        }

        let ctx = WhisperContext::new_with_params(
            model_path.to_str().unwrap(),
            params,
        )
        .map_err(|e| anyhow::anyhow!("Failed to load Whisper model: {:?}", e))?;

        tracing::info!("Whisper model loaded successfully");

        Ok(Self {
            ctx,
            language: language.to_string(),
            languages: languages.to_vec(),
        })
    }

    /// Check if running on Apple Silicon (arm64) vs Intel Mac
    #[cfg(target_os = "macos")]
    fn has_apple_silicon() -> bool {
        std::env::consts::ARCH == "aarch64"
    }

    /// Transcribe audio samples (16kHz mono f32) to text using a single language
    fn transcribe_with_language(&self, samples: &[f32], language: &str) -> Result<String> {
        if samples.is_empty() {
            return Ok(String::new());
        }

        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| anyhow::anyhow!("Failed to create Whisper state: {:?}", e))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(language));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        params.set_single_segment(true);
        params.set_n_threads(std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(4).min(8));

        state
            .full(params, samples)
            .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {:?}", e))?;

        let num_segments = state.full_n_segments();
        let mut text = String::new();

        for i in 0..num_segments {
            if let Some(segment) = state.get_segment(i) {
                if let Ok(segment_text) = segment.to_str() {
                    text.push_str(segment_text);
                }
            }
        }

        Ok(text.trim().to_string())
    }

    /// Transcribe using the configured language priority list
    pub fn transcribe(&self, samples: &[f32]) -> Result<String> {
        if !self.languages.is_empty() {
            self.transcribe_with_languages(samples, &self.languages)
        } else {
            self.transcribe_with_languages(samples, &[self.language.clone()])
        }
    }

    /// Transcribe audio trying multiple languages in priority order.
    /// Returns the first non-empty result.
    pub fn transcribe_with_languages(&self, samples: &[f32], languages: &[String]) -> Result<String> {
        if samples.is_empty() {
            return Ok(String::new());
        }

        if languages.is_empty() {
            return self.transcribe_with_language(samples, "auto");
        }

        // If only one language, just use it directly
        if languages.len() == 1 {
            let text = self.transcribe_with_language(samples, &languages[0])?;
            tracing::info!("Transcribed [{}]: \"{}\"", languages[0], text);
            return Ok(text);
        }

        // Try each language in priority order, return first non-empty result
        for lang in languages {
            let text = self.transcribe_with_language(samples, lang)?;
            if !text.is_empty() {
                tracing::info!("Transcribed [{}]: \"{}\"", lang, text);
                return Ok(text);
            }
            tracing::debug!("Language {} produced empty result, trying next", lang);
        }

        // All languages produced empty — return empty
        Ok(String::new())
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
