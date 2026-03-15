use voice_terminal::audio;
use voice_terminal::config;
use voice_terminal::daemon;
use voice_terminal::injection;
use voice_terminal::input;
use voice_terminal::recognition;
use voice_terminal::state;
use voice_terminal::ui;

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

    /// Manage the background daemon
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },

    /// Show the floating status overlay
    Overlay,

    /// Internal: run the daemon process (not for direct use)
    #[command(hide = true)]
    DaemonRun {
        /// Skip auto-starting overlay (used during restart)
        #[arg(long, default_value_t = false)]
        no_overlay: bool,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Start the daemon in the background
    Start,
    /// Stop the running daemon
    Stop,
    /// Show daemon status
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Daemon mode sets up its own logging to a file after fork
    let is_daemon_run = matches!(cli.command, Some(Commands::DaemonRun { .. }));
    if !is_daemon_run {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .init();
    }

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
            if daemon::is_daemon_running() {
                anyhow::bail!("A daemon is already running. Use `voice-terminal daemon stop` first.");
            }
            let mut config = AppConfig::load()?;

            if let Some(m) = mode {
                config.activation.mode = m;
            }
            if let Some(h) = hotkey {
                config.activation.hotkey = h;
            }

            run(config).await?;
        }

        Some(Commands::Daemon { action }) => match action {
            DaemonAction::Start => {
                if daemon::is_daemon_running() {
                    println!("Daemon is already running.");
                    return Ok(());
                }
                daemon::lifecycle::start_daemon()?;
            }
            DaemonAction::Stop => {
                daemon::lifecycle::stop_daemon()?;
            }
            DaemonAction::Status => {
                daemon::lifecycle::print_status()?;
            }
        },

        Some(Commands::Overlay) => {
            #[cfg(feature = "gui")]
            {
                // Write overlay PID for dedup
                let overlay_pid_path = daemon::overlay_pid_file_path()?;
                std::fs::write(&overlay_pid_path, std::process::id().to_string())?;

                let config = AppConfig::load()?;
                let sock_path = daemon::socket_path()?;
                let result = ui::overlay::run_overlay(
                    &sock_path,
                    config.overlay.opacity,
                    &config.overlay.position,
                );

                // Clean up PID file on exit
                let _ = std::fs::remove_file(&overlay_pid_path);

                // Stop daemon when overlay is closed
                if daemon::is_daemon_running() {
                    let _ = daemon::lifecycle::stop_daemon_process();
                }

                result?;
            }
            #[cfg(not(feature = "gui"))]
            {
                anyhow::bail!(
                    "Overlay requires the 'gui' feature. Rebuild with: cargo build --features gui"
                );
            }
        }

        Some(Commands::DaemonRun { no_overlay }) => {
            // Internal: this is the actual daemon process spawned by `daemon start`
            daemon::lifecycle::setup_daemon_logging()?;
            daemon::write_pid_file()?;

            // Register SIGTERM handler
            let sock_path_sig = daemon::socket_path()?;
            tokio::spawn(async move {
                let mut sig = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::terminate(),
                )
                .expect("Failed to register SIGTERM handler");
                sig.recv().await;
                tracing::info!("SIGTERM received, shutting down");
                let _ = daemon::remove_pid_file();
                let _ = std::fs::remove_file(&sock_path_sig);
                std::process::exit(0);
            });

            let config = AppConfig::load()?;

            // Auto-start overlay if configured, not already running, and not suppressed
            if !no_overlay
                && config.daemon.auto_start_overlay
                && !daemon::is_overlay_running()
            {
                if let Ok(exe) = std::env::current_exe() {
                    let mut cmd = std::process::Command::new(exe);
                    cmd.arg("overlay");
                    for var in &["DISPLAY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"] {
                        if let Ok(val) = std::env::var(var) {
                            cmd.env(var, val);
                        }
                    }
                    match cmd.spawn() {
                        Ok(child) => tracing::info!("Overlay started with PID {}", child.id()),
                        Err(e) => tracing::warn!("Failed to start overlay: {}", e),
                    }
                }
            }

            run_daemon(config).await?;
        }

        None => {
            if daemon::is_daemon_running() {
                anyhow::bail!("A daemon is already running. Use `voice-terminal daemon stop` first.");
            }
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

    let recognizer = WhisperRecognizer::new_with_languages(&model_path, &config.whisper.language, &config.whisper.language_priority())?;
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
                // Check if there's a command after the wake word in the same utterance
                let remainder = wake_detector.strip_wake_word(&text);
                if !remainder.is_empty() {
                    // User said wake word + command in one shot (e.g., "hey terminal list files")
                    println!("🔔 Wake word detected in: \"{}\"", text);
                    println!("📝 \"{}\"", remainder);
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    text_injector.inject_text(remainder)?;
                } else {
                    // Just the wake word, record the follow-up command
                    println!("🔔 Wake word detected!\n🎙️  Recording command...");

                    let handle = audio_capture.start_recording()?;

                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
                        let current = handle.current_samples();
                        if vad.should_stop_recording(&current) {
                            break;
                        }
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

// --- Daemon mode functions ---

async fn run_daemon(config: AppConfig) -> Result<()> {
    let model_path = config.model_path()?;
    if !model_path.exists() {
        let data_dir = AppConfig::data_dir()?.join("models");
        WhisperRecognizer::download_model(&config.whisper.model_size, &data_dir).await?;
    }

    let recognizer = WhisperRecognizer::new_with_languages(&model_path, &config.whisper.language, &config.whisper.language_priority())?;
    let audio_capture = AudioCapture::new(config.audio.sample_rate);
    let mut text_injector = TextInjector::new()?;

    // Start socket server
    let sock_path = daemon::socket_path()?;
    let server = daemon::SocketServer::new(&sock_path)?;
    let broadcaster = server.broadcaster();

    std::thread::spawn(move || server.accept_loop());

    // Broadcast initial state
    let mut app_state = state::AppState::default();
    app_state.mode = config.activation.mode.to_string();
    app_state.state = state::DaemonState::Listening;
    app_state.target_app = ui::active_window::get_active_window_name();
    broadcaster.broadcast(&app_state)?;

    // Shared current state for the periodic active window poller
    let shared_state = std::sync::Arc::new(std::sync::Mutex::new(app_state.clone()));

    // Periodically poll active window and update target_app (only when listening)
    let periodic_broadcaster = broadcaster.clone();
    let shared_state_poller = shared_state.clone();
    tokio::spawn(async move {
        let mut last_app = String::new();
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            let app = ui::active_window::get_active_window_name();
            if app != last_app {
                last_app = app.clone();
                let mut st = shared_state_poller.lock().unwrap();
                st.target_app = app;
                // Only broadcast from poller — the mode loop also broadcasts on transitions
                let _ = periodic_broadcaster.broadcast(&st);
            }
        }
    });

    tracing::info!("Daemon running in {} mode", config.activation.mode);

    let result = match config.activation.mode {
        ActivationMode::Toggle => {
            run_toggle_mode_daemon(&config, &recognizer, &audio_capture, &mut text_injector, &broadcaster, &shared_state).await
        }
        ActivationMode::Vad => {
            run_vad_mode_daemon(&config, &recognizer, &audio_capture, &mut text_injector, &broadcaster, &shared_state).await
        }
        ActivationMode::WakeWord => {
            run_wake_word_mode_daemon(&config, &recognizer, &audio_capture, &mut text_injector, &broadcaster, &shared_state).await
        }
    };

    // Cleanup
    let _ = daemon::remove_pid_file();
    let _ = std::fs::remove_file(&sock_path);
    tracing::info!("Daemon shutdown complete");

    result
}

type SharedState = std::sync::Arc<std::sync::Mutex<state::AppState>>;

/// Update local state, sync to shared state (for periodic poller), and broadcast
fn broadcast_state(
    app_state: &state::AppState,
    shared: &SharedState,
    broadcaster: &daemon::Broadcaster,
) {
    *shared.lock().unwrap() = app_state.clone();
    let _ = broadcaster.broadcast(app_state);
}

async fn run_toggle_mode_daemon(
    config: &AppConfig,
    recognizer: &WhisperRecognizer,
    audio_capture: &AudioCapture,
    text_injector: &mut TextInjector,
    broadcaster: &daemon::Broadcaster,
    shared_state: &std::sync::Arc<std::sync::Mutex<state::AppState>>,
) -> Result<()> {
    let hotkey_listener = HotkeyListener::new(&config.activation.hotkey)?;
    let rx = hotkey_listener.start()?;

    let mut app_state = state::AppState::default();
    app_state.mode = config.activation.mode.to_string();
    app_state.state = state::DaemonState::Listening;
    app_state.target_app = ui::active_window::get_active_window_name();
    broadcast_state(&app_state, shared_state, broadcaster);

    let mut recording_handle = None;

    loop {
        match rx.recv() {
            Ok(input::hotkey::HotkeyEvent::Activated) => {
                app_state.state = state::DaemonState::Recording;
                app_state.target_app = ui::active_window::get_active_window_name();
                broadcast_state(&app_state, shared_state, broadcaster);
                recording_handle = Some(audio_capture.start_recording()?);
            }
            Ok(input::hotkey::HotkeyEvent::Deactivated) => {
                if let Some(handle) = recording_handle.take() {
                    app_state.state = state::DaemonState::Processing;
                    broadcast_state(&app_state, shared_state, broadcaster);

                    let samples = handle.stop();
                    if !samples.is_empty() {
                        match recognizer.transcribe(&samples) {
                            Ok(text) if !text.is_empty() => {
                                app_state.last_text = text.clone();
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                text_injector.inject_text(&text)?;
                            }
                            Ok(_) => {}
                            Err(e) => tracing::error!("Transcription error: {}", e),
                        }
                    }

                    app_state.state = state::DaemonState::Listening;
                    app_state.target_app = ui::active_window::get_active_window_name();
                    broadcast_state(&app_state, shared_state, broadcaster);
                }
            }
            Err(_) => break,
        }
    }

    Ok(())
}

async fn run_vad_mode_daemon(
    config: &AppConfig,
    recognizer: &WhisperRecognizer,
    audio_capture: &AudioCapture,
    text_injector: &mut TextInjector,
    broadcaster: &daemon::Broadcaster,
    shared_state: &SharedState,
) -> Result<()> {
    let hotkey_listener = HotkeyListener::new(&config.activation.hotkey)?;
    let rx = hotkey_listener.start()?;
    let vad = VoiceActivityDetector::new(
        config.vad.energy_threshold,
        config.vad.silence_duration_ms,
        config.vad.min_speech_duration_ms,
        config.audio.sample_rate,
    );

    let mut app_state = state::AppState::default();
    app_state.mode = config.activation.mode.to_string();
    app_state.state = state::DaemonState::Listening;
    app_state.target_app = ui::active_window::get_active_window_name();
    broadcast_state(&app_state, shared_state, broadcaster);

    loop {
        match rx.recv() {
            Ok(input::hotkey::HotkeyEvent::Activated) => {
                app_state.state = state::DaemonState::Recording;
                app_state.target_app = ui::active_window::get_active_window_name();
                broadcast_state(&app_state, shared_state, broadcaster);

                let handle = audio_capture.start_recording()?;

                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    let samples = handle.current_samples();
                    if vad.should_stop_recording(&samples) {
                        break;
                    }
                }

                app_state.state = state::DaemonState::Processing;
                broadcast_state(&app_state, shared_state, broadcaster);

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
                        app_state.last_text = text.clone();
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        text_injector.inject_text(&text)?;
                    }
                    Ok(_) => {}
                    Err(e) => tracing::error!("Transcription error: {}", e),
                }

                app_state.state = state::DaemonState::Listening;
                app_state.target_app = ui::active_window::get_active_window_name();
                broadcast_state(&app_state, shared_state, broadcaster);
            }
            Ok(input::hotkey::HotkeyEvent::Deactivated) => {}
            Err(_) => break,
        }
    }

    Ok(())
}

async fn run_wake_word_mode_daemon(
    config: &AppConfig,
    recognizer: &WhisperRecognizer,
    audio_capture: &AudioCapture,
    text_injector: &mut TextInjector,
    broadcaster: &daemon::Broadcaster,
    shared_state: &SharedState,
) -> Result<()> {
    let wake_detector = WakeWordDetector::new(&config.activation.wake_word);
    let vad = VoiceActivityDetector::new(
        config.vad.energy_threshold,
        config.vad.silence_duration_ms,
        config.vad.min_speech_duration_ms,
        config.audio.sample_rate,
    );

    let mut app_state = state::AppState::default();
    app_state.mode = config.activation.mode.to_string();
    app_state.state = state::DaemonState::Listening;
    app_state.target_app = ui::active_window::get_active_window_name();
    broadcast_state(&app_state, shared_state, broadcaster);

    let silence_samples = (config.vad.silence_duration_ms as usize
        * config.audio.sample_rate as usize)
        / 1000;

    // Wake word uses faster VAD: shorter silence, shorter min speech
    let wake_silence_ms: u64 = 600;
    let wake_vad = VoiceActivityDetector::new(
        config.vad.energy_threshold,
        wake_silence_ms,
        250,
        config.audio.sample_rate,
    );
    let wake_silence_samples =
        (wake_silence_ms as usize * config.audio.sample_rate as usize) / 1000;
    let chunk_size = (config.audio.sample_rate as usize) / 10;

    // Outer loop: restarts continuous recording when needed
    'restart: loop {
        let handle = audio_capture.start_recording()?;
        let mut has_speech = false;
        let mut speech_start: usize = 0;

        loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
        let samples = handle.current_samples();
        let current_len = samples.len();

        // Detect speech onset
        if current_len >= chunk_size {
            let tail = &samples[current_len - chunk_size..];
            if wake_vad.is_speech(tail) && !has_speech {
                // Mark speech start (include a small lookback for context)
                speech_start = current_len.saturating_sub(chunk_size * 3);
                has_speech = true;
            }
        }

        // Detect speech→silence transition
        if has_speech && current_len > speech_start {
            let region = &samples[speech_start..];
            if wake_vad.should_stop_recording(region) {
                let speech_end = current_len.saturating_sub(wake_silence_samples);
                let speech_segment: Vec<f32> = samples[speech_start..speech_end].to_vec();
                has_speech = false;

                // Skip tiny segments (noise/clicks)
                if speech_segment.len() < config.audio.sample_rate as usize / 4 {
                    continue;
                }

                // Transcribe the speech segment to check for wake word
                match recognizer.transcribe(&speech_segment) {
                    Ok(text) if wake_detector.detect(&text) => {
                        let remainder = wake_detector.strip_wake_word(&text);

                        if !remainder.is_empty() {
                            // Wake word + command in one shot
                            app_state.state = state::DaemonState::Processing;
                            broadcast_state(&app_state, shared_state, broadcaster);

                            app_state.last_text = remainder.to_string();
                            tokio::time::sleep(tokio::time::Duration::from_millis(100))
                                .await;
                            text_injector.inject_text(remainder)?;

                            app_state.state = state::DaemonState::Listening;
                            app_state.target_app =
                                ui::active_window::get_active_window_name();
                            broadcast_state(&app_state, shared_state, broadcaster);
                        } else {
                            // Just wake word — stop continuous recording, record command separately
                            let _ = handle.stop();

                            app_state.state = state::DaemonState::Recording;
                            app_state.target_app =
                                ui::active_window::get_active_window_name();
                            broadcast_state(&app_state, shared_state, broadcaster);

                            let cmd_handle = audio_capture.start_recording()?;

                            // Grace period before checking for silence
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                            loop {
                                tokio::time::sleep(tokio::time::Duration::from_millis(150))
                                    .await;
                                let current = cmd_handle.current_samples();
                                if vad.should_stop_recording(&current) {
                                    break;
                                }
                                if current.len() > config.audio.sample_rate as usize * 30 {
                                    break;
                                }
                            }

                            app_state.state = state::DaemonState::Processing;
                            broadcast_state(&app_state, shared_state, broadcaster);

                            let command_samples = cmd_handle.stop();
                            let trimmed_cmd = if command_samples.len() > silence_samples {
                                &command_samples[..command_samples.len() - silence_samples]
                            } else {
                                &command_samples[..]
                            };

                            match recognizer.transcribe(trimmed_cmd) {
                                Ok(cmd_text) if !cmd_text.is_empty() => {
                                    app_state.last_text = cmd_text.clone();
                                    tokio::time::sleep(
                                        tokio::time::Duration::from_millis(100),
                                    )
                                    .await;
                                    text_injector.inject_text(&cmd_text)?;
                                }
                                Ok(_) => {}
                                Err(e) => tracing::error!("Transcription error: {}", e),
                            }

                            app_state.state = state::DaemonState::Listening;
                            app_state.target_app =
                                ui::active_window::get_active_window_name();
                            broadcast_state(&app_state, shared_state, broadcaster);

                            // Restart continuous recording for next wake word
                            continue 'restart;
                        }
                    }
                    Ok(_) => {} // Not the wake word
                    Err(e) => {
                        tracing::debug!("Wake word check error: {}", e);
                    }
                }
            }
        }

        // Safety: if buffer gets too large (60s), reset to avoid memory growth
        if current_len > config.audio.sample_rate as usize * 60 {
            let _ = handle.stop();
            continue 'restart;
        }
        } // inner loop
    } // 'restart loop
}
