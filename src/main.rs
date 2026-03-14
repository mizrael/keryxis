use voice_terminal::audio;
use voice_terminal::config;
use voice_terminal::injection;
use voice_terminal::input;
use voice_terminal::recognition;

use anyhow::Result;
use clap::{Parser, Subcommand};

use audio::{AudioCapture, VoiceActivityDetector};
use config::{ActivationMode, AppConfig, ModelSize};
use injection::TextInjector;
use input::{HotkeyListener, WakeWordDetector};
use recognition::WhisperRecognizer;

#[derive(Parser)]
#[command(name = "voice-terminal")]
#[command(about = "Speech-to-text input for any application via local Whisper model")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start listening for voice input (default)
    Start {
        /// Activation mode
        #[arg(short, long)]
        mode: Option<ActivationMode>,

        /// Hotkey binding (e.g., "Alt+Space")
        #[arg(short = 'k', long)]
        hotkey: Option<String>,
    },

    /// Download the Whisper model
    DownloadModel {
        /// Model size to download
        #[arg(short, long, default_value = "base")]
        size: ModelSize,
    },

    /// Show or update configuration
    Config {
        /// Set activation mode
        #[arg(long)]
        mode: Option<ActivationMode>,

        /// Set hotkey
        #[arg(long)]
        hotkey: Option<String>,

        /// Set wake word
        #[arg(long)]
        wake_word: Option<String>,

        /// Set model size
        #[arg(long)]
        model: Option<ModelSize>,

        /// Set language
        #[arg(long)]
        language: Option<String>,

        /// Show current configuration
        #[arg(long)]
        show: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::DownloadModel { size }) => {
            let data_dir = AppConfig::data_dir()?.join("models");
            WhisperRecognizer::download_model(&size, &data_dir).await?;
        }

        Some(Commands::Config {
            mode,
            hotkey,
            wake_word,
            model,
            language,
            show,
        }) => {
            let mut config = AppConfig::load()?;

            if show
                && mode.is_none()
                && hotkey.is_none()
                && wake_word.is_none()
                && model.is_none()
                && language.is_none()
            {
                println!("{}", toml::to_string_pretty(&config)?);
                return Ok(());
            }

            if let Some(m) = mode {
                config.activation.mode = m;
            }
            if let Some(h) = hotkey {
                config.activation.hotkey = h;
            }
            if let Some(w) = wake_word {
                config.activation.wake_word = w;
            }
            if let Some(m) = model {
                config.whisper.model_size = m;
            }
            if let Some(l) = language {
                config.whisper.language = l;
            }

            config.save()?;
            println!("Configuration updated.");

            if show {
                println!("{}", toml::to_string_pretty(&config)?);
            }
        }

        Some(Commands::Start { mode, hotkey }) => {
            let mut config = AppConfig::load()?;

            if let Some(m) = mode {
                config.activation.mode = m;
            }
            if let Some(h) = hotkey {
                config.activation.hotkey = h;
            }

            run(config).await?;
        }

        None => {
            let config = AppConfig::load()?;
            run(config).await?;
        }
    }

    Ok(())
}

async fn run(config: AppConfig) -> Result<()> {
    let model_path = config.model_path()?;
    if !model_path.exists() {
        println!(
            "Whisper model not found at: {}\nDownloading {} model...",
            model_path.display(),
            config.whisper.model_size.file_name()
        );
        let data_dir = AppConfig::data_dir()?.join("models");
        WhisperRecognizer::download_model(&config.whisper.model_size, &data_dir).await?;
    }

    let recognizer = WhisperRecognizer::new(&model_path, &config.whisper.language)?;
    let audio_capture = AudioCapture::new(config.audio.sample_rate);
    let mut text_injector = TextInjector::new()?;

    println!("╔══════════════════════════════════════╗");
    println!("║       Voice Terminal v{}        ║", env!("CARGO_PKG_VERSION"));
    println!("╠══════════════════════════════════════╣");
    println!("║  Mode:   {:<27} ║", config.activation.mode);
    println!("║  Hotkey: {:<27} ║", config.activation.hotkey);
    if config.activation.mode == ActivationMode::WakeWord {
        println!(
            "║  Wake:   {:<27} ║",
            config.activation.wake_word
        );
    }
    println!("╚══════════════════════════════════════╝");
    println!();

    match config.activation.mode {
        ActivationMode::Toggle => {
            run_toggle_mode(&config, &recognizer, &audio_capture, &mut text_injector).await
        }
        ActivationMode::Vad => {
            run_vad_mode(&config, &recognizer, &audio_capture, &mut text_injector).await
        }
        ActivationMode::WakeWord => {
            run_wake_word_mode(&config, &recognizer, &audio_capture, &mut text_injector).await
        }
    }
}

/// Toggle mode: press hotkey to start, press again to stop
async fn run_toggle_mode(
    config: &AppConfig,
    recognizer: &WhisperRecognizer,
    audio_capture: &AudioCapture,
    text_injector: &mut TextInjector,
) -> Result<()> {
    let hotkey_listener = HotkeyListener::new(&config.activation.hotkey)?;
    let rx = hotkey_listener.start()?;

    println!("Press {} to start/stop recording...", config.activation.hotkey);

    let mut recording_handle = None;

    loop {
        match rx.recv() {
            Ok(input::hotkey::HotkeyEvent::Activated) => {
                println!("🎙️  Recording...");
                recording_handle = Some(audio_capture.start_recording()?);
            }
            Ok(input::hotkey::HotkeyEvent::Deactivated) => {
                if let Some(handle) = recording_handle.take() {
                    println!("⏹️  Processing...");
                    let samples = handle.stop();

                    if samples.is_empty() {
                        println!("No audio captured.");
                        continue;
                    }

                    match recognizer.transcribe(&samples) {
                        Ok(text) if !text.is_empty() => {
                            println!("📝 \"{}\"", text);
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            text_injector.inject_text(&text)?;
                        }
                        Ok(_) => println!("(no speech detected)"),
                        Err(e) => tracing::error!("Transcription error: {}", e),
                    }

                    println!("\nPress {} to record again...", config.activation.hotkey);
                }
            }
            Err(_) => break,
        }
    }

    Ok(())
}

/// VAD mode: press hotkey to start, auto-stops when silence is detected
async fn run_vad_mode(
    config: &AppConfig,
    recognizer: &WhisperRecognizer,
    audio_capture: &AudioCapture,
    text_injector: &mut TextInjector,
) -> Result<()> {
    let hotkey_listener = HotkeyListener::new(&config.activation.hotkey)?;
    let rx = hotkey_listener.start()?;
    let vad = VoiceActivityDetector::new(
        config.vad.energy_threshold,
        config.vad.silence_duration_ms,
        config.vad.min_speech_duration_ms,
        config.audio.sample_rate,
    );

    println!(
        "Press {} to start recording (auto-stops on silence)...",
        config.activation.hotkey
    );

    loop {
        match rx.recv() {
            Ok(input::hotkey::HotkeyEvent::Activated) => {
                println!("🎙️  Recording (will stop on silence)...");
                let handle = audio_capture.start_recording()?;

                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    let samples = handle.current_samples();
                    if vad.should_stop_recording(&samples) {
                        break;
                    }
                }

                println!("⏹️  Silence detected, processing...");
                let samples = handle.stop();

                let silence_samples = (config.vad.silence_duration_ms as usize
                    * config.audio.sample_rate as usize)
                    / 1000;
                let trimmed = if samples.len() > silence_samples {
                    &samples[..samples.len() - silence_samples]
                } else {
                    &samples
                };

                match recognizer.transcribe(trimmed) {
                    Ok(text) if !text.is_empty() => {
                        println!("📝 \"{}\"", text);
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        text_injector.inject_text(&text)?;
                    }
                    Ok(_) => println!("(no speech detected)"),
                    Err(e) => tracing::error!("Transcription error: {}", e),
                }

                println!(
                    "\nPress {} to record again...",
                    config.activation.hotkey
                );
            }
            Ok(input::hotkey::HotkeyEvent::Deactivated) => {}
            Err(_) => break,
        }
    }

    Ok(())
}

/// Wake word mode: always listening, activates on wake word detection.
/// Uses energy-based VAD to avoid running Whisper continuously — only
/// transcribes when actual speech is detected.
async fn run_wake_word_mode(
    config: &AppConfig,
    recognizer: &WhisperRecognizer,
    audio_capture: &AudioCapture,
    text_injector: &mut TextInjector,
) -> Result<()> {
    let wake_detector = WakeWordDetector::new(&config.activation.wake_word);
    let vad = VoiceActivityDetector::new(
        config.vad.energy_threshold,
        config.vad.silence_duration_ms,
        config.vad.min_speech_duration_ms,
        config.audio.sample_rate,
    );

    println!(
        "👂 Listening for wake word: \"{}\"...",
        config.activation.wake_word
    );

    loop {
        // Phase 1: Record until we detect speech followed by silence (a complete utterance)
        let handle = audio_capture.start_recording()?;

        // Wait for a complete utterance (speech then silence)
        let mut has_speech = false;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
            let samples = handle.current_samples();

            // Check if there's any speech energy in the latest chunk
            let chunk_size = (config.audio.sample_rate as usize) / 10;
            if samples.len() >= chunk_size {
                let tail = &samples[samples.len() - chunk_size..];
                if vad.is_speech(tail) {
                    has_speech = true;
                }
            }

            // Once we've seen speech, wait for silence to end the utterance
            if has_speech && vad.should_stop_recording(&samples) {
                break;
            }

            // Safety: don't accumulate more than 10 seconds waiting for wake word
            if samples.len() > config.audio.sample_rate as usize * 10 {
                // Reset if we've been recording too long without a valid utterance
                break;
            }
        }

        let samples = handle.stop();

        if !has_speech || samples.is_empty() {
            continue;
        }

        // Phase 2: Transcribe and check for wake word
        // Trim trailing silence before transcribing
        let silence_samples = (config.vad.silence_duration_ms as usize
            * config.audio.sample_rate as usize)
            / 1000;
        let trimmed = if samples.len() > silence_samples {
            &samples[..samples.len() - silence_samples]
        } else {
            &samples[..]
        };

        match recognizer.transcribe(trimmed) {
            Ok(text) if wake_detector.detect(&text) => {
                println!("🔔 Wake word detected in: \"{}\"\n🎙️  Recording command...", text);

                // Phase 3: Record the actual command after wake word
                let handle = audio_capture.start_recording()?;

                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
                    let current = handle.current_samples();
                    if vad.should_stop_recording(&current) {
                        break;
                    }
                    // Safety timeout: 30 seconds max
                    if current.len() > config.audio.sample_rate as usize * 30 {
                        break;
                    }
                }

                println!("⏹️  Processing...");
                let command_samples = handle.stop();

                let trimmed_cmd = if command_samples.len() > silence_samples {
                    &command_samples[..command_samples.len() - silence_samples]
                } else {
                    &command_samples[..]
                };

                match recognizer.transcribe(trimmed_cmd) {
                    Ok(text) if !text.is_empty() => {
                        println!("📝 \"{}\"", text);
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        text_injector.inject_text(&text)?;
                    }
                    Ok(_) => println!("(no speech detected after wake word)"),
                    Err(e) => tracing::error!("Transcription error: {}", e),
                }

                println!(
                    "\n👂 Listening for wake word: \"{}\"...",
                    config.activation.wake_word
                );
            }
            Ok(text) if !text.is_empty() => {
                tracing::debug!("Heard: \"{}\" (no wake word)", text);
            }
            Ok(_) => {}
            Err(e) => {
                tracing::debug!("Wake word check error: {}", e);
            }
        }
    }
}
