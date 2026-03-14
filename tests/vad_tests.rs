use voice_terminal::audio::VoiceActivityDetector;

#[test]
fn test_rms_energy_silence() {
    let vad = VoiceActivityDetector::new(0.01, 1000, 500, 16000);
    let silence = vec![0.0f32; 1600];
    assert_eq!(vad.rms_energy(&silence), 0.0);
}

#[test]
fn test_rms_energy_loud() {
    let vad = VoiceActivityDetector::new(0.01, 1000, 500, 16000);
    let loud = vec![0.5f32; 1600];
    let energy = vad.rms_energy(&loud);
    assert!(energy > 0.4, "Expected energy > 0.4, got {}", energy);
}

#[test]
fn test_rms_energy_empty() {
    let vad = VoiceActivityDetector::new(0.01, 1000, 500, 16000);
    assert_eq!(vad.rms_energy(&[]), 0.0);
}

#[test]
fn test_is_speech_with_silence() {
    let vad = VoiceActivityDetector::new(0.01, 1000, 500, 16000);
    let silence = vec![0.0f32; 1600];
    assert!(!vad.is_speech(&silence));
}

#[test]
fn test_is_speech_with_tone() {
    let vad = VoiceActivityDetector::new(0.01, 1000, 500, 16000);
    let tone: Vec<f32> = (0..1600)
        .map(|i| (i as f32 / 16000.0 * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5)
        .collect();
    assert!(vad.is_speech(&tone));
}

#[test]
fn test_should_stop_pure_silence() {
    // Pure silence (no speech ever) should NOT trigger stop (no speech detected)
    let vad = VoiceActivityDetector::new(0.01, 1000, 500, 16000);
    let silence = vec![0.0f32; 16000 * 3]; // 3 seconds
    assert!(!vad.should_stop_recording(&silence));
}

#[test]
fn test_should_stop_speech_then_silence() {
    let vad = VoiceActivityDetector::new(0.01, 1000, 500, 16000);

    // 1 second of speech (440Hz sine wave) + 1.5 seconds of silence
    let mut samples = Vec::new();
    for i in 0..16000 {
        let t = i as f32 / 16000.0;
        samples.push((t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5);
    }
    samples.extend(vec![0.0f32; 24000]); // 1.5 sec silence > 1 sec threshold

    assert!(vad.should_stop_recording(&samples));
}

#[test]
fn test_should_not_stop_ongoing_speech() {
    let vad = VoiceActivityDetector::new(0.01, 1000, 500, 16000);

    // Continuous speech (no silence at end)
    let samples: Vec<f32> = (0..32000)
        .map(|i| (i as f32 / 16000.0 * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5)
        .collect();

    assert!(!vad.should_stop_recording(&samples));
}

#[test]
fn test_should_not_stop_too_short_total_audio() {
    // Total audio shorter than silence_duration should not trigger stop
    let vad = VoiceActivityDetector::new(0.01, 1000, 500, 16000);

    // Only 500ms total audio (less than silence_duration)
    let mut samples = Vec::new();
    for i in 0..3200 {
        // 200ms speech
        let t = i as f32 / 16000.0;
        samples.push((t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5);
    }
    samples.extend(vec![0.0f32; 4800]); // 300ms silence, total = 500ms

    // Not enough silence to reach the 1000ms threshold
    assert!(!vad.should_stop_recording(&samples));
}

#[test]
fn test_should_not_stop_insufficient_silence() {
    let vad = VoiceActivityDetector::new(0.01, 1500, 500, 16000);

    // 1 second speech + only 500ms silence (threshold is 1500ms)
    let mut samples = Vec::new();
    for i in 0..16000 {
        let t = i as f32 / 16000.0;
        samples.push((t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5);
    }
    samples.extend(vec![0.0f32; 8000]); // only 500ms silence

    assert!(!vad.should_stop_recording(&samples));
}

#[test]
fn test_different_energy_thresholds() {
    // Low threshold should detect quiet sounds
    let sensitive_vad = VoiceActivityDetector::new(0.001, 1000, 500, 16000);
    let quiet: Vec<f32> = vec![0.005f32; 1600];
    assert!(sensitive_vad.is_speech(&quiet));

    // High threshold should not detect quiet sounds
    let insensitive_vad = VoiceActivityDetector::new(0.1, 1000, 500, 16000);
    assert!(!insensitive_vad.is_speech(&quiet));
}

#[test]
fn test_very_short_audio_no_crash() {
    let vad = VoiceActivityDetector::new(0.01, 1000, 500, 16000);
    // Should handle gracefully without panicking
    assert!(!vad.should_stop_recording(&[]));
    assert!(!vad.should_stop_recording(&[0.0]));
    assert!(!vad.should_stop_recording(&[0.0; 100]));
}
