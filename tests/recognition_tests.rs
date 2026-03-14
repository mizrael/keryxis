/// Integration tests for the full transcription pipeline.
/// These require a Whisper model to be downloaded and available.
/// Run with: cargo test --features integration_tests -- --ignored
use voice_terminal::config::{AppConfig, ModelSize};
use voice_terminal::recognition::WhisperRecognizer;

/// Helper: generate a sine wave at the given frequency (simulates a tone, not speech)
fn generate_tone(frequency: f32, duration_secs: f32, sample_rate: u32) -> Vec<f32> {
    let num_samples = (duration_secs * sample_rate as f32) as usize;
    (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (t * frequency * 2.0 * std::f32::consts::PI).sin() * 0.5
        })
        .collect()
}

/// Helper: generate silence
fn generate_silence(duration_secs: f32, sample_rate: u32) -> Vec<f32> {
    let num_samples = (duration_secs * sample_rate as f32) as usize;
    vec![0.0f32; num_samples]
}

#[test]
#[ignore = "requires Whisper model to be downloaded"]
fn test_whisper_transcribe_silence() {
    let config = AppConfig::default();
    let model_path = config.model_path().unwrap();
    let recognizer = WhisperRecognizer::new(&model_path, "en").unwrap();

    let silence = generate_silence(2.0, 16000);
    let result = recognizer.transcribe(&silence).unwrap();

    // Silence should produce empty or near-empty output
    assert!(
        result.trim().is_empty() || result.len() < 20,
        "Expected minimal output for silence, got: '{}'",
        result
    );
}

#[test]
#[ignore = "requires Whisper model to be downloaded"]
fn test_whisper_transcribe_empty() {
    let config = AppConfig::default();
    let model_path = config.model_path().unwrap();
    let recognizer = WhisperRecognizer::new(&model_path, "en").unwrap();

    let result = recognizer.transcribe(&[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_whisper_model_not_found() {
    let result = WhisperRecognizer::new(std::path::Path::new("/nonexistent/model.bin"), "en");
    assert!(result.is_err());
}

#[test]
#[ignore = "requires Whisper model to be downloaded"]
fn test_whisper_transcribe_tone() {
    let config = AppConfig::default();
    let model_path = config.model_path().unwrap();
    let recognizer = WhisperRecognizer::new(&model_path, "en").unwrap();

    // A pure tone is not speech, so Whisper should produce minimal output
    let tone = generate_tone(440.0, 2.0, 16000);
    let result = recognizer.transcribe(&tone).unwrap();
    // Just verify it doesn't crash; output may vary
    println!("Tone transcription: '{}'", result);
}

#[test]
fn test_model_size_urls_are_valid() {
    for size in &[
        ModelSize::Tiny,
        ModelSize::Base,
        ModelSize::Small,
        ModelSize::Medium,
        ModelSize::Large,
    ] {
        let url = size.huggingface_url();
        assert!(url.starts_with("https://"));
        assert!(url.contains("huggingface.co"));
        assert!(url.contains("whisper.cpp"));
    }
}
