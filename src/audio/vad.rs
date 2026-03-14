/// Simple energy-based Voice Activity Detection.
///
/// Monitors audio energy levels to detect speech vs silence.
/// When silence is detected for longer than the configured duration,
/// it signals that the user has stopped speaking.
pub struct VoiceActivityDetector {
    energy_threshold: f32,
    silence_duration_ms: u64,
    min_speech_duration_ms: u64,
    sample_rate: u32,
}

impl VoiceActivityDetector {
    pub fn new(
        energy_threshold: f32,
        silence_duration_ms: u64,
        min_speech_duration_ms: u64,
        sample_rate: u32,
    ) -> Self {
        Self {
            energy_threshold,
            silence_duration_ms,
            min_speech_duration_ms,
            sample_rate,
        }
    }

    /// Calculate RMS energy of an audio chunk
    pub fn rms_energy(&self, samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
        (sum_sq / samples.len() as f32).sqrt()
    }

    /// Check if the given audio chunk contains speech
    pub fn is_speech(&self, samples: &[f32]) -> bool {
        self.rms_energy(samples) > self.energy_threshold
    }

    /// Analyze the full audio buffer and determine if the user has stopped speaking.
    /// Returns true if speech was detected and then silence lasted longer than the threshold.
    pub fn should_stop_recording(&self, samples: &[f32]) -> bool {
        let chunk_size = (self.sample_rate as usize) / 10; // 100ms chunks
        if samples.len() < chunk_size {
            return false;
        }

        // Check if we have enough audio for minimum speech duration
        let min_samples =
            (self.min_speech_duration_ms as usize * self.sample_rate as usize) / 1000;
        if samples.len() < min_samples {
            return false;
        }

        // Check chunks from the end to detect trailing silence
        let silence_samples =
            (self.silence_duration_ms as usize * self.sample_rate as usize) / 1000;

        if samples.len() < silence_samples {
            return false;
        }

        // Check if the last N ms are silence
        let tail = &samples[samples.len() - silence_samples..];
        let tail_chunks: Vec<&[f32]> = tail.chunks(chunk_size).collect();
        let all_silent = tail_chunks
            .iter()
            .all(|chunk| !self.is_speech(chunk));

        if !all_silent {
            return false;
        }

        // Verify there was actual speech before the silence
        let speech_region = &samples[..samples.len() - silence_samples];
        let had_speech = speech_region
            .chunks(chunk_size)
            .any(|chunk| self.is_speech(chunk));

        had_speech
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silence_detection() {
        let vad = VoiceActivityDetector::new(0.01, 1000, 500, 16000);
        let silence = vec![0.0f32; 16000 * 3]; // 3 seconds of silence
        assert!(!vad.should_stop_recording(&silence));
    }

    #[test]
    fn test_speech_then_silence() {
        let vad = VoiceActivityDetector::new(0.01, 1000, 500, 16000);

        // Generate 1 second of "speech" (sine wave) followed by 1.5 seconds of silence
        let mut samples = Vec::new();
        for i in 0..16000 {
            let t = i as f32 / 16000.0;
            samples.push((t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5);
        }
        samples.extend(vec![0.0f32; 24000]); // 1.5 seconds silence

        assert!(vad.should_stop_recording(&samples));
    }

    #[test]
    fn test_rms_energy() {
        let vad = VoiceActivityDetector::new(0.01, 1000, 500, 16000);
        let silence = vec![0.0f32; 1000];
        assert_eq!(vad.rms_energy(&silence), 0.0);

        let loud = vec![0.5f32; 1000];
        assert!(vad.rms_energy(&loud) > 0.4);
    }
}
