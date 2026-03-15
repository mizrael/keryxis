use voice_terminal::config::*;
use std::path::PathBuf;

#[test]
fn test_default_config() {
    let config = AppConfig::default();
    assert_eq!(config.activation.mode, ActivationMode::Toggle);
    assert_eq!(config.activation.hotkey, "Alt+Space");
    assert_eq!(config.activation.wake_word, "hey terminal");
    assert_eq!(config.whisper.model_size, ModelSize::Tiny);
    assert_eq!(config.whisper.languages, vec!["en".to_string()]);
    assert!(config.whisper.language.is_empty());
    assert_eq!(config.audio.sample_rate, 16000);
    assert_eq!(config.audio.channels, 1);
}

#[test]
fn test_default_vad_config() {
    let config = AppConfig::default();
    assert!((config.vad.energy_threshold - 0.01).abs() < f32::EPSILON);
    assert_eq!(config.vad.silence_duration_ms, 1500);
    assert_eq!(config.vad.min_speech_duration_ms, 500);
}

#[test]
fn test_model_size_file_names() {
    assert_eq!(ModelSize::Tiny.file_name(), "ggml-tiny.bin");
    assert_eq!(ModelSize::Base.file_name(), "ggml-base.bin");
    assert_eq!(ModelSize::Small.file_name(), "ggml-small.bin");
    assert_eq!(ModelSize::Medium.file_name(), "ggml-medium.bin");
    assert_eq!(ModelSize::Large.file_name(), "ggml-large.bin");
}

#[test]
fn test_model_huggingface_urls() {
    let url = ModelSize::Base.huggingface_url();
    assert!(url.starts_with("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/"));
    assert!(url.ends_with("ggml-base.bin"));
}

#[test]
fn test_activation_mode_display() {
    assert_eq!(format!("{}", ActivationMode::Toggle), "toggle");
    assert_eq!(format!("{}", ActivationMode::Vad), "vad");
    assert_eq!(format!("{}", ActivationMode::WakeWord), "wake_word");
}

#[test]
fn test_config_serialization_roundtrip() {
    let config = AppConfig::default();
    let serialized = toml::to_string_pretty(&config).expect("Failed to serialize");
    let deserialized: AppConfig = toml::from_str(&serialized).expect("Failed to deserialize");

    assert_eq!(deserialized.activation.mode, config.activation.mode);
    assert_eq!(deserialized.activation.hotkey, config.activation.hotkey);
    assert_eq!(
        deserialized.activation.wake_word,
        config.activation.wake_word
    );
    assert_eq!(
        deserialized.whisper.model_size,
        config.whisper.model_size
    );
    assert_eq!(deserialized.whisper.language, config.whisper.language);
    assert_eq!(deserialized.audio.sample_rate, config.audio.sample_rate);
}

#[test]
fn test_config_save_and_load() {
    let temp_dir = std::env::temp_dir().join("voice-terminal-test-config");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let config_path = temp_dir.join("config.toml");

    let mut config = AppConfig::default();
    config.activation.mode = ActivationMode::Vad;
    config.activation.hotkey = "Ctrl+Shift+R".to_string();
    config.whisper.model_size = ModelSize::Small;

    // Save manually
    let contents = toml::to_string_pretty(&config).unwrap();
    std::fs::write(&config_path, &contents).unwrap();

    // Load
    let loaded_contents = std::fs::read_to_string(&config_path).unwrap();
    let loaded: AppConfig = toml::from_str(&loaded_contents).unwrap();

    assert_eq!(loaded.activation.mode, ActivationMode::Vad);
    assert_eq!(loaded.activation.hotkey, "Ctrl+Shift+R");
    assert_eq!(loaded.whisper.model_size, ModelSize::Small);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_custom_config_deserialization() {
    let toml_str = r#"
[activation]
mode = "wake_word"
hotkey = "F5"
wake_word = "computer"

[whisper]
model_size = "tiny"
language = "es"

[vad]
energy_threshold = 0.05
silence_duration_ms = 2000
min_speech_duration_ms = 300

[audio]
sample_rate = 16000
channels = 1

[daemon]
auto_start_overlay = true

[overlay]
position = "top-right"
opacity = 0.85
"#;

    let config: AppConfig = toml::from_str(toml_str).expect("Failed to parse TOML");
    assert_eq!(config.activation.mode, ActivationMode::WakeWord);
    assert_eq!(config.activation.wake_word, "computer");
    assert_eq!(config.whisper.model_size, ModelSize::Tiny);
    assert_eq!(config.whisper.language, "es");
    assert!((config.vad.energy_threshold - 0.05).abs() < f32::EPSILON);
    assert_eq!(config.vad.silence_duration_ms, 2000);
}

#[test]
fn test_model_path_resolution() {
    let config = AppConfig::default();
    let path = config.model_path().unwrap();
    // Should end with the model filename
    assert!(path.to_str().unwrap().ends_with("ggml-tiny.bin"));
}

#[test]
fn test_model_path_custom() {
    let mut config = AppConfig::default();
    config.whisper.model_path = Some(PathBuf::from("/custom/path/model.bin"));
    let path = config.model_path().unwrap();
    assert_eq!(path, PathBuf::from("/custom/path/model.bin"));
}

#[test]
fn test_config_path_exists() {
    // Should return a valid path (doesn't need to exist)
    let path = AppConfig::config_path().unwrap();
    assert!(path.to_str().unwrap().contains("voice-terminal"));
    assert!(path.to_str().unwrap().ends_with("config.toml"));
}

#[test]
fn test_data_dir_exists() {
    let dir = AppConfig::data_dir().unwrap();
    assert!(dir.to_str().unwrap().contains("voice-terminal"));
}

#[test]
fn test_default_daemon_config() {
    let config = AppConfig::default();
    assert!(config.daemon.auto_start_overlay);
}

#[test]
fn test_default_overlay_config() {
    let config = AppConfig::default();
    assert_eq!(config.overlay.position, "top-right");
    assert!((config.overlay.opacity - 0.85).abs() < f32::EPSILON);
}

#[test]
fn test_daemon_config_serialization() {
    let config = AppConfig::default();
    let serialized = toml::to_string_pretty(&config).unwrap();
    assert!(serialized.contains("[daemon]"));
    assert!(serialized.contains("auto_start_overlay"));
    assert!(serialized.contains("[overlay]"));
    assert!(serialized.contains("position"));
}

#[test]
fn test_daemon_config_deserialization() {
    let toml_str = r#"
[activation]
mode = "toggle"
hotkey = "Alt+Space"
wake_word = "hey terminal"

[whisper]
model_size = "tiny"
language = "auto"

[vad]
energy_threshold = 0.01
silence_duration_ms = 1500
min_speech_duration_ms = 500

[audio]
sample_rate = 16000
channels = 1

[daemon]
auto_start_overlay = false

[overlay]
position = "top-left"
opacity = 0.9
"#;
    let config: AppConfig = toml::from_str(toml_str).unwrap();
    assert!(!config.daemon.auto_start_overlay);
    assert_eq!(config.overlay.position, "top-left");
    assert!((config.overlay.opacity - 0.9).abs() < f32::EPSILON);
}
