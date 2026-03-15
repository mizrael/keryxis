use keryxis::audio::AudioCapture;

#[test]
fn test_audio_capture_creation() {
    let capture = AudioCapture::new(16000);
    assert!(!capture.is_recording());
}

#[test]
fn test_audio_capture_not_recording_initially() {
    let capture = AudioCapture::new(16000);
    assert!(!capture.is_recording());
}

#[test]
fn test_audio_capture_different_sample_rates() {
    // Should not panic with various sample rates
    let _capture_8k = AudioCapture::new(8000);
    let _capture_16k = AudioCapture::new(16000);
    let _capture_44k = AudioCapture::new(44100);
    let _capture_48k = AudioCapture::new(48000);
}

// Note: start_recording() tests require a real audio device,
// so they're guarded by a feature flag or run only in environments
// with audio hardware.
#[cfg(feature = "integration_tests")]
mod integration {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_audio_capture_record_and_stop() {
        let capture = AudioCapture::new(16000);
        let handle = capture.start_recording().expect("Failed to start recording");
        assert!(capture.is_recording());

        std::thread::sleep(Duration::from_millis(500));

        let samples = handle.stop();
        // Should have captured some samples (even if silence)
        assert!(!samples.is_empty(), "Expected some audio samples");
    }

    #[test]
    fn test_audio_capture_sample_count() {
        let capture = AudioCapture::new(16000);
        let handle = capture.start_recording().expect("Failed to start recording");

        std::thread::sleep(Duration::from_millis(100));
        let count = handle.sample_count();
        assert!(count > 0, "Expected sample count > 0 after 100ms");

        handle.stop();
    }
}
