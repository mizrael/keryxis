use anyhow::Result;
use crate::recognition::WhisperRecognizer;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

/// Events emitted by the wake word detector
#[derive(Debug, Clone)]
pub enum WakeWordEvent {
    Detected(String), // Contains the transcription that triggered detection
}

/// Handle to receive wake word events with timeout support
pub struct WakeWordDetectorHandle {
    rx: mpsc::Receiver<WakeWordEvent>,
}

impl WakeWordDetectorHandle {
    /// Receive the next wake word event, blocking up to `timeout` duration.
    /// Returns `Ok(Some(event))` if event received, `Ok(None)` if timeout.
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Result<Option<WakeWordEvent>> {
        match self.rx.recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => anyhow::bail!("Wake word detector disconnected"),
        }
    }
}

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

    /// Check if the given transcription contains the wake word.
    /// Strips punctuation before matching so "Hey, buddy!" matches "hey buddy".
    pub fn detect(&self, transcription: &str) -> bool {
        let normalized = Self::normalize(transcription);
        normalized.contains(&self.wake_word)
    }

    /// Strip the wake word (and anything before it) from the transcription,
    /// returning only the command that follows.
    pub fn strip_wake_word<'a>(&self, transcription: &'a str) -> &'a str {
        let normalized = Self::normalize(transcription);
        if let Some(pos) = normalized.find(&self.wake_word) {
            let after = pos + self.wake_word.len();
            // Map back to original string position (same length since we only removed chars)
            // Walk the original string to find the corresponding position
            let mut orig_pos = 0;
            let mut norm_count = 0;
            for (i, c) in transcription.char_indices() {
                if !c.is_ascii_punctuation() {
                    norm_count += c.to_lowercase().count();
                } else {
                    continue;
                }
                if norm_count >= after {
                    orig_pos = i + c.len_utf8();
                    break;
                }
            }
            transcription[orig_pos..].trim_start_matches([',', '.', '!', ':', ';', ' '])
        } else {
            transcription
        }
    }

    /// Normalize text: lowercase and remove punctuation
    fn normalize(text: &str) -> String {
        text.to_lowercase()
            .chars()
            .filter(|c| !c.is_ascii_punctuation())
            .collect()
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

    /// Start listening for wake word events.
    /// Returns a handle that can receive WakeWordEvent with timeout support.
    pub fn start(self) -> Result<WakeWordDetectorHandle> {
        let (tx, rx) = mpsc::channel();
        let _is_listening = self.is_listening.clone();

        std::thread::spawn(move || {
            // Placeholder: this would be integrated with actual audio processing
            // For now, this thread is created but not actively listening
            drop(tx); // Close the channel if the detector is dropped
        });

        Ok(WakeWordDetectorHandle { rx })
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
