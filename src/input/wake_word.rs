use crate::recognition::WhisperRecognizer;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Detects a configurable wake word using continuous Whisper transcription.
///
/// This is a simple approach: we continuously transcribe short audio chunks
/// and check if the wake word appears in the transcription.
/// For production use, a dedicated wake word engine (e.g., Porcupine) would be
/// more efficient, but this keeps the dependency count low.
pub struct WakeWordDetector {
    wake_word: String,
    is_listening: Arc<AtomicBool>,
}

impl WakeWordDetector {
    pub fn new(wake_word: &str) -> Self {
        Self {
            wake_word: wake_word.to_lowercase(),
            is_listening: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if the given transcription contains the wake word
    pub fn detect(&self, transcription: &str) -> bool {
        let text = transcription.to_lowercase();
        text.contains(&self.wake_word)
    }

    /// Set listening state
    pub fn set_listening(&self, listening: bool) {
        self.is_listening.store(listening, Ordering::SeqCst);
    }

    /// Check if currently listening
    pub fn is_listening(&self) -> bool {
        self.is_listening.load(Ordering::SeqCst)
    }

    /// Get the configured wake word
    pub fn wake_word(&self) -> &str {
        &self.wake_word
    }
}

/// Continuously listens for the wake word using short audio chunks.
/// Returns when the wake word is detected.
pub async fn listen_for_wake_word(
    recognizer: &WhisperRecognizer,
    detector: &WakeWordDetector,
    audio_samples: &[f32],
    chunk_duration_secs: f32,
    sample_rate: u32,
) -> bool {
    let chunk_size = (chunk_duration_secs * sample_rate as f32) as usize;

    // Process the latest chunk
    if audio_samples.len() < chunk_size {
        return false;
    }

    let latest_chunk = &audio_samples[audio_samples.len() - chunk_size..];

    match recognizer.transcribe(latest_chunk) {
        Ok(text) => {
            if detector.detect(&text) {
                tracing::info!("Wake word detected: \"{}\"", text);
                return true;
            }
        }
        Err(e) => {
            tracing::debug!("Wake word transcription error: {}", e);
        }
    }

    false
}
