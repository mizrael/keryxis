#[cfg(feature = "gui")]
use crate::config::{ActivationMode, AppConfig, ModelSize};
#[cfg(feature = "gui")]
use crate::state::{AppState, DaemonState};

#[cfg(feature = "gui")]
use std::io::{BufRead, BufReader};
#[cfg(feature = "gui")]
use std::os::unix::net::UnixStream;
#[cfg(feature = "gui")]
use std::sync::{Arc, Mutex};

#[cfg(feature = "gui")]
struct DaemonConnection {
    state: Arc<Mutex<AppState>>,
    connected: Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(feature = "gui")]
impl DaemonConnection {
    fn new(sock_path: &std::path::Path) -> Self {
        let state = Arc::new(Mutex::new(AppState::default()));
        let connected = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let state_r = state.clone();
        let connected_w = connected.clone();
        let sock = sock_path.to_path_buf();

        std::thread::spawn(move || loop {
            match UnixStream::connect(&sock) {
                Ok(s) => {
                    connected_w.store(true, std::sync::atomic::Ordering::SeqCst);

                    let reader = BufReader::new(s);
                    for line in reader.lines() {
                        match line {
                            Ok(l) if !l.trim().is_empty() => {
                                if let Ok(new_state) = AppState::from_framed_json(&l) {
                                    *state_r.lock().unwrap() = new_state;
                                }
                            }
                            Err(_) => break,
                            _ => {}
                        }
                    }
                    connected_w.store(false, std::sync::atomic::Ordering::SeqCst);
                }
                Err(_) => {
                    connected_w.store(false, std::sync::atomic::Ordering::SeqCst);
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        });

        Self {
            state,
            connected,
        }
    }

    fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Run the floating overlay GUI.
#[cfg(feature = "gui")]
pub fn run_overlay(
    sock_path: &std::path::Path,
    opacity: f32,
    position: &str,
) -> anyhow::Result<()> {
    let conn = DaemonConnection::new(sock_path);
    let position_owned = position.to_string();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([310.0, 50.0])
            .with_always_on_top()
            .with_decorations(false)
            .with_transparent(true)
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "Keryxis",
        native_options,
        Box::new(move |_cc| {
            let config = AppConfig::load().unwrap_or_default();

            // Start log tailing thread
            let log_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let log_lines_writer = log_lines.clone();
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader, Seek, SeekFrom};
                let log_path = crate::daemon::state_dir()
                    .unwrap_or_default()
                    .join("daemon.log");
                loop {
                    if let Ok(file) = std::fs::File::open(&log_path) {
                        let mut reader = BufReader::new(file);
                        // Start from end of file
                        let _ = reader.seek(SeekFrom::End(0));
                        loop {
                            let mut line = String::new();
                            match reader.read_line(&mut line) {
                                Ok(0) => {
                                    // No new data, wait
                                    std::thread::sleep(std::time::Duration::from_millis(300));
                                }
                                Ok(_) => {
                                    let trimmed = line.trim_end().to_string();
                                    if !trimmed.is_empty() {
                                        let mut lines = log_lines_writer.lock().unwrap();
                                        lines.push(trimmed);
                                        // Keep last 200 lines
                                        if lines.len() > 200 {
                                            let drain = lines.len() - 200;
                                            lines.drain(..drain);
                                        }
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            });

            Ok(Box::new(OverlayApp {
                conn,
                show_settings: false,
                show_logs: false,
                settings: SettingsState::from_config(&config),
                capturing_hotkey: false,
                captured_keys: Vec::new(),
                log_lines,
                opacity,
                position: position_owned.clone(),
                positioned: false,
                daemon_control_pending: false,
                daemon_action_thread: None,
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Overlay error: {}", e))
}

#[cfg(feature = "gui")]
struct SettingsState {
    mode: ActivationMode,
    hotkey: String,
    wake_word: String,
    model: ModelSize,
    languages: Vec<String>,
    original_mode: ActivationMode,
    original_hotkey: String,
    original_wake_word: String,
    original_model: ModelSize,
    original_languages: Vec<String>,
}

#[cfg(feature = "gui")]
impl SettingsState {
    fn from_config(config: &AppConfig) -> Self {
        let langs = config.whisper.language_priority();
        Self {
            mode: config.activation.mode.clone(),
            hotkey: config.activation.hotkey.clone(),
            wake_word: config.activation.wake_word.clone(),
            model: config.whisper.model_size.clone(),
            languages: langs.clone(),
            original_mode: config.activation.mode.clone(),
            original_hotkey: config.activation.hotkey.clone(),
            original_wake_word: config.activation.wake_word.clone(),
            original_model: config.whisper.model_size.clone(),
            original_languages: langs,
        }
    }

    fn has_changes(&self) -> bool {
        self.mode != self.original_mode
            || self.hotkey != self.original_hotkey
            || self.wake_word != self.original_wake_word
            || self.model != self.original_model
            || self.languages != self.original_languages
    }

    fn reset(&mut self) {
        self.mode = self.original_mode.clone();
        self.hotkey = self.original_hotkey.clone();
        self.wake_word = self.original_wake_word.clone();
        self.model = self.original_model.clone();
        self.languages = self.original_languages.clone();
    }

    fn apply(&mut self) {
        self.original_mode = self.mode.clone();
        self.original_hotkey = self.hotkey.clone();
        self.original_wake_word = self.wake_word.clone();
        self.original_model = self.model.clone();
        self.original_languages = self.languages.clone();
    }
}

#[cfg(feature = "gui")]
struct OverlayApp {
    conn: DaemonConnection,
    show_settings: bool,
    show_logs: bool,
    settings: SettingsState,
    capturing_hotkey: bool,
    captured_keys: Vec<egui::Key>,
    log_lines: Arc<Mutex<Vec<String>>>,
    opacity: f32,
    position: String,
    positioned: bool,
    daemon_control_pending: bool,
    daemon_action_thread: Option<std::thread::JoinHandle<()>>,
}

#[cfg(feature = "gui")]
impl OverlayApp {
    fn save_and_restart(&mut self) {
        if let Ok(mut config) = AppConfig::load() {
            config.activation.mode = self.settings.mode.clone();
            config.activation.hotkey = self.settings.hotkey.clone();
            config.activation.wake_word = self.settings.wake_word.clone();
            config.whisper.model_size = self.settings.model.clone();
            config.whisper.languages = self.settings.languages.clone();
            config.whisper.language = String::new();

            if let Err(e) = config.save() {
                tracing::error!("Failed to save config: {}", e);
                return;
            }
            self.settings.apply();
        }
        // Restart daemon in background thread — overlay stays alive and reconnects
        std::thread::spawn(|| {
            if let Err(e) = crate::daemon::lifecycle::restart_daemon() {
                tracing::error!("Failed to restart daemon: {}", e);
            }
        });
    }

    fn mode_label(mode: &ActivationMode) -> &'static str {
        match mode {
            ActivationMode::Toggle => "Press to talk",
            ActivationMode::Vad => "Auto-stop",
            ActivationMode::WakeWord => "Hands-free",
        }
    }

    fn mode_description(mode: &ActivationMode) -> &'static str {
        match mode {
            ActivationMode::Toggle => "Press hotkey to start/stop",
            ActivationMode::Vad => "Press hotkey, stops on silence",
            ActivationMode::WakeWord => "Say wake word to activate",
        }
    }

    fn render_progress_bar(&self, ui: &mut egui::Ui, state: &crate::state::AppState) {
        use crate::state::ModelLoadingState;
        
        match &state.model_loading {
            ModelLoadingState::Idle => {
                // No progress bar needed
            }
            ModelLoadingState::Downloading {
                name,
                current,
                total,
            } => {
                let progress = if *total > 0 {
                    (*current as f32 / *total as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(format!("⬇️  Downloading {}...", name))
                            .size(11.0)
                            .color(egui::Color32::from_rgb(100, 150, 255)),
                    );
                    ui.add(
                        egui::ProgressBar::new(progress)
                            .text(format!("{}%", (progress * 100.0) as u32))
                            .show_percentage(),
                    );
                });
            }
            ModelLoadingState::Loading { name } => {
                ui.vertical(|ui| {
                    let t = ui.ctx().input(|i| i.time);
                    let pulse = ((t * 3.0).sin() * 0.5 + 0.5) as f32;
                    
                    ui.label(
                        egui::RichText::new(format!("⏳ Loading {}...", name))
                            .size(11.0)
                            .color(egui::Color32::from_rgb(
                                (255.0 * pulse) as u8,
                                200,
                                100,
                            )),
                    );
                    
                    ui.add(
                        egui::ProgressBar::new(pulse).show_percentage()
                    );
                });
            }
            ModelLoadingState::Ready { name } => {
                ui.label(
                    egui::RichText::new(format!("✅ {} ready", name))
                        .size(11.0)
                        .color(egui::Color32::from_rgb(100, 255, 100)),
                );
            }
            ModelLoadingState::Error { message } => {
                ui.label(
                    egui::RichText::new(format!("❌ Error: {}", message))
                        .size(11.0)
                        .color(egui::Color32::from_rgb(255, 100, 100)),
                );
            }
        }
    }

    fn spawn_daemon_control(&mut self, is_running: bool) {
        self.daemon_control_pending = true;
        
        let is_running_copy = is_running;
        let handle = std::thread::spawn(move || {
            if is_running_copy {
                println!("Stopping daemon...");
                // Use stop_daemon_process to stop daemon without closing overlay
                match crate::daemon::lifecycle::stop_daemon_process() {
                    Ok(_) => println!("Daemon stopped successfully"),
                    Err(e) => eprintln!("Failed to stop daemon: {}", e),
                }
            } else {
                println!("Starting daemon...");
                match crate::daemon::lifecycle::start_daemon() {
                    Ok(_) => println!("Daemon started successfully"),
                    Err(e) => eprintln!("Failed to start daemon: {}", e),
                }
            }
        });
        
        self.daemon_action_thread = Some(handle);
    }
}

#[cfg(feature = "gui")]
impl eframe::App for OverlayApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Fully transparent — let the OS compositor handle the background
        egui::Rgba::TRANSPARENT.to_array()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let state = self.conn.state.lock().unwrap().clone();
        let connected = self.conn.is_connected();

        // Check if daemon control thread has completed
        if self.daemon_control_pending {
            if let Some(handle) = self.daemon_action_thread.take() {
                if handle.is_finished() {
                    self.daemon_control_pending = false;
                } else {
                    self.daemon_action_thread = Some(handle);
                }
            }
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(200));

        let target_height = if self.show_settings {
            440.0
        } else if self.show_logs {
            300.0
        } else {
            50.0
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(340.0, target_height)));

        // Position window on first frame based on config
        if !self.positioned {
            let screen = ctx.input(|i| i.screen_rect);
            if screen.width() > 0.0 && screen.height() > 0.0 {
                self.positioned = true;
                let margin = 20.0;
                let win_w = 340.0;
                let pos = match self.position.as_str() {
                    "top-left" => egui::pos2(margin, margin),
                    "bottom-left" => egui::pos2(margin, screen.max.y - target_height - margin),
                    "bottom-right" => egui::pos2(screen.max.x - win_w - margin, screen.max.y - target_height - margin),
                    _ => egui::pos2(screen.max.x - win_w - margin, margin), // top-right default
                };
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
            }
        }

        let bg_alpha = (self.opacity.clamp(0.0, 1.0) * 255.0) as u8;
        let bg = egui::Color32::from_rgba_unmultiplied(30, 30, 30, bg_alpha);
        let frame = egui::Frame::NONE
            .fill(bg)
            .rounding(egui::Rounding::same(10))
            .inner_margin(egui::Margin::same(10));

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            // === Main status bar ===
            ui.horizontal(|ui| {
                // Yellow when disconnected, otherwise state-based color
                let color = if !connected {
                    egui::Color32::from_rgb(255, 200, 50)
                } else {
                    match state.state {
                        DaemonState::Idle => egui::Color32::GRAY,
                        DaemonState::Listening => egui::Color32::from_rgb(50, 205, 50),
                        DaemonState::Recording => {
                            let t = ctx.input(|i| i.time);
                            let pulse = ((t * 3.0).sin() * 0.3 + 0.7) as f32;
                            let r = (255.0 * pulse) as u8;
                            egui::Color32::from_rgb(r, 40, 40)
                        }
                        DaemonState::Processing => egui::Color32::from_rgb(255, 200, 50),
                    }
                };

                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 5.0, color);

                // Status label
                let (status_text, text_color) = if !connected {
                    ("OFF", egui::Color32::from_rgb(255, 200, 50))
                } else {
                    match state.state {
                        DaemonState::Idle => ("IDLE", egui::Color32::GRAY),
                        DaemonState::Listening => ("RDY", egui::Color32::from_rgb(50, 205, 50)),
                        DaemonState::Recording => ("REC", color),
                        DaemonState::Processing => ("...", egui::Color32::from_rgb(255, 200, 50)),
                    }
                };
                ui.label(
                    egui::RichText::new(status_text)
                        .size(11.0)
                        .color(text_color)
                        .strong()
                        .monospace(),
                );

                // Target app + mode label
                let mode_label = Self::mode_label(&self.settings.mode);
                let app_text = if connected {
                    format!("> {}  [{}]", state.target_app, mode_label)
                } else {
                    "disconnected".to_string()
                };
                ui.label(
                    egui::RichText::new(app_text)
                        .size(12.0)
                        .color(if connected {
                            egui::Color32::from_rgb(180, 180, 180)
                        } else {
                            egui::Color32::from_rgb(120, 120, 120)
                        }),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let hover_fill = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25);
                    let btn_rounding = egui::Rounding::same(4);

                    // Gear button (settings)
                    let gear_color = if self.show_settings {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::from_rgb(150, 150, 150)
                    };
                    let gear_btn = ui.add(
                        egui::Button::new(
                            egui::RichText::new("⚙").size(14.0).color(gear_color),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .rounding(btn_rounding),
                    );
                    if gear_btn.hovered() {
                        ui.painter().rect_filled(gear_btn.rect, btn_rounding, hover_fill);
                    }
                    if gear_btn.clicked() {
                        self.show_settings = !self.show_settings;
                        self.show_logs = false;
                        if !self.show_settings {
                            self.settings.reset();
                            self.capturing_hotkey = false;
                        }
                    }

                    // Log button
                    let log_color = if self.show_logs {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::from_rgb(150, 150, 150)
                    };
                    let log_btn = ui.add(
                        egui::Button::new(
                            egui::RichText::new("\u{2261}").size(14.0).color(log_color).monospace(),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .rounding(btn_rounding),
                    );
                    if log_btn.hovered() {
                        ui.painter().rect_filled(log_btn.rect, btn_rounding, hover_fill);
                    }
                    if log_btn.on_hover_text("Toggle logs").clicked() {
                        self.show_logs = !self.show_logs;
                        self.show_settings = false;
                    }
                });
            });

            // === Progress bar (model loading) ===
            self.render_progress_bar(ui, &state);

            // === Settings panel ===
            if self.show_settings {
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                // Header + daemon status + control button
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("⚙ Settings")
                            .size(13.0)
                            .color(egui::Color32::from_rgb(220, 220, 220))
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Daemon control button
                        let (button_text, button_color) = if self.daemon_control_pending {
                            if connected {
                                ("Stopping...", egui::Color32::from_rgb(200, 100, 100))
                            } else {
                                ("Starting...", egui::Color32::from_rgb(100, 150, 200))
                            }
                        } else if connected {
                            ("Stop Daemon", egui::Color32::from_rgb(220, 80, 80))
                        } else {
                            ("Start Daemon", egui::Color32::from_rgb(80, 200, 80))
                        };

                        let btn = ui.add_enabled(
                            !self.daemon_control_pending,
                            egui::Button::new(
                                egui::RichText::new(button_text)
                                    .size(11.0)
                                    .color(egui::Color32::WHITE)
                            )
                            .fill(button_color)
                            .rounding(egui::Rounding::same(4)),
                        );

                        if btn.clicked() && !self.daemon_control_pending {
                            self.spawn_daemon_control(connected);
                        }

                        ui.add_space(8.0);

                        // Daemon status indicator
                        let (status_color, status_text) = if connected {
                            (egui::Color32::from_rgb(50, 205, 50), "Daemon running")
                        } else {
                            (egui::Color32::from_rgb(200, 60, 60), "Daemon stopped")
                        };
                        let (dot_rect, _) =
                            ui.allocate_exact_size(egui::vec2(6.0, 6.0), egui::Sense::hover());
                        ui.painter()
                            .circle_filled(dot_rect.center(), 3.0, status_color);
                        ui.label(
                            egui::RichText::new(status_text)
                                .size(10.0)
                                .color(egui::Color32::from_rgb(130, 130, 130)),
                        );
                    });
                });

                ui.add_space(8.0);

                // Mode
                ui.label(
                    egui::RichText::new("Mode")
                        .size(11.0)
                        .color(egui::Color32::from_rgb(140, 140, 140)),
                );
                egui::ComboBox::from_id_salt("mode_select")
                    .selected_text(Self::mode_label(&self.settings.mode))
                    .width(ui.available_width() - 8.0)
                    .show_ui(ui, |ui| {
                        for mode in [
                            ActivationMode::Toggle,
                            ActivationMode::Vad,
                            ActivationMode::WakeWord,
                        ] {
                            let label = Self::mode_label(&mode);
                            let desc = Self::mode_description(&mode);
                            let text = format!("{} — {}", label, desc);
                            ui.selectable_value(&mut self.settings.mode, mode, text);
                        }
                    });

                ui.add_space(6.0);

                // Hotkey
                ui.label(
                    egui::RichText::new("Hotkey")
                        .size(11.0)
                        .color(egui::Color32::from_rgb(140, 140, 140)),
                );
                let hotkey_text = if self.capturing_hotkey {
                    "Press key combo..."
                } else {
                    &self.settings.hotkey
                };
                let hotkey_btn = ui.add_sized(
                    [ui.available_width(), 28.0],
                    egui::Button::new(
                        egui::RichText::new(hotkey_text)
                            .size(13.0)
                            .color(if self.capturing_hotkey {
                                egui::Color32::from_rgb(100, 160, 255)
                            } else {
                                egui::Color32::from_rgb(200, 200, 200)
                            })
                            .monospace(),
                    )
                    .fill(egui::Color32::from_rgb(42, 42, 42)),
                );
                if hotkey_btn.clicked() {
                    self.capturing_hotkey = true;
                    self.captured_keys.clear();
                }

                if self.capturing_hotkey {
                    ctx.input(|i| {
                        for event in &i.events {
                            if let egui::Event::Key {
                                key,
                                pressed: true,
                                modifiers,
                                ..
                            } = event
                            {
                                if matches!(key, egui::Key::Escape) {
                                    self.capturing_hotkey = false;
                                    self.captured_keys.clear();
                                    return;
                                }
                                // Skip if only a modifier was pressed (no actual key yet)
                                // egui doesn't fire Key events for bare modifier presses,
                                // so any Key event here is a real key with modifiers.
                                let mut parts = Vec::new();
                                if modifiers.ctrl {
                                    parts.push("Ctrl".to_string());
                                }
                                if modifiers.alt {
                                    parts.push("Alt".to_string());
                                }
                                if modifiers.shift {
                                    parts.push("Shift".to_string());
                                }
                                if modifiers.mac_cmd || modifiers.command {
                                    parts.push("Cmd".to_string());
                                }
                                // Map the key to our hotkey parser's expected format
                                let key_name = match key {
                                    egui::Key::Space => "Space",
                                    egui::Key::Tab => "Tab",
                                    egui::Key::Enter => "Return",
                                    egui::Key::Escape => "Escape",
                                    egui::Key::Backspace => "Backspace",
                                    egui::Key::F1 => "F1",
                                    egui::Key::F2 => "F2",
                                    egui::Key::F3 => "F3",
                                    egui::Key::F4 => "F4",
                                    egui::Key::F5 => "F5",
                                    egui::Key::F6 => "F6",
                                    egui::Key::F7 => "F7",
                                    egui::Key::F8 => "F8",
                                    egui::Key::F9 => "F9",
                                    egui::Key::F10 => "F10",
                                    egui::Key::F11 => "F11",
                                    egui::Key::F12 => "F12",
                                    egui::Key::A => "A",
                                    egui::Key::B => "B",
                                    egui::Key::C => "C",
                                    egui::Key::D => "D",
                                    egui::Key::E => "E",
                                    egui::Key::F => "F",
                                    egui::Key::G => "G",
                                    egui::Key::H => "H",
                                    egui::Key::I => "I",
                                    egui::Key::J => "J",
                                    egui::Key::K => "K",
                                    egui::Key::L => "L",
                                    egui::Key::M => "M",
                                    egui::Key::N => "N",
                                    egui::Key::O => "O",
                                    egui::Key::P => "P",
                                    egui::Key::Q => "Q",
                                    egui::Key::R => "R",
                                    egui::Key::S => "S",
                                    egui::Key::T => "T",
                                    egui::Key::U => "U",
                                    egui::Key::V => "V",
                                    egui::Key::W => "W",
                                    egui::Key::X => "X",
                                    egui::Key::Y => "Y",
                                    egui::Key::Z => "Z",
                                    other => {
                                        // Fallback for other keys
                                        self.captured_keys.push(*other);
                                        return;
                                    }
                                };
                                parts.push(key_name.to_string());
                                self.settings.hotkey = parts.join("+");
                                self.capturing_hotkey = false;
                                self.captured_keys.clear();
                            }
                        }
                    });
                }

                ui.add_space(6.0);

                // Wake word
                ui.label(
                    egui::RichText::new("Wake word")
                        .size(11.0)
                        .color(egui::Color32::from_rgb(140, 140, 140)),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.wake_word)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .text_color(egui::Color32::from_rgb(200, 200, 200)),
                );

                ui.add_space(6.0);

                // Model
                ui.label(
                    egui::RichText::new("Model")
                        .size(11.0)
                        .color(egui::Color32::from_rgb(140, 140, 140)),
                );
                egui::ComboBox::from_id_salt("model_select")
                    .selected_text(format!("{:?}", self.settings.model))
                    .width(ui.available_width() - 8.0)
                    .show_ui(ui, |ui| {
                        for (model, label) in [
                            (ModelSize::Tiny, "Tiny (fast)"),
                            (ModelSize::Base, "Base"),
                            (ModelSize::Small, "Small"),
                            (ModelSize::Medium, "Medium"),
                            (ModelSize::Large, "Large (accurate)"),
                        ] {
                            ui.selectable_value(&mut self.settings.model, model, label);
                        }
                    });

                ui.add_space(6.0);

                // Languages (priority order — checked = enabled, order = priority)
                ui.label(
                    egui::RichText::new("Languages (priority order)")
                        .size(11.0)
                        .color(egui::Color32::from_rgb(140, 140, 140)),
                );

                let all_langs = [
                    ("en", "English"),
                    ("it", "Italian"),
                    ("es", "Spanish"),
                    ("fr", "French"),
                    ("de", "German"),
                    ("pt", "Portuguese"),
                    ("ja", "Japanese"),
                    ("zh", "Chinese"),
                ];

                // Show selected languages first (in order), then unselected
                let mut to_add: Option<String> = None;
                let mut to_remove: Option<String> = None;
                let mut to_move_up: Option<usize> = None;

                for (idx, lang_code) in self.settings.languages.iter().enumerate() {
                    let label = all_langs
                        .iter()
                        .find(|(c, _)| c == lang_code)
                        .map(|(_, l)| *l)
                        .unwrap_or(lang_code.as_str());
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{}.", idx + 1))
                                .size(11.0)
                                .color(egui::Color32::from_rgb(100, 100, 100))
                                .monospace(),
                        );
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(label)
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(200, 200, 200)),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            to_remove = Some(lang_code.clone());
                        }
                        if idx > 0 {
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("^")
                                            .size(10.0)
                                            .color(egui::Color32::from_rgb(140, 140, 140)),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text("Move up")
                                .clicked()
                            {
                                to_move_up = Some(idx);
                            }
                        }
                    });
                }

                // Show unselected languages as add buttons
                ui.horizontal_wrapped(|ui| {
                    for (code, label) in &all_langs {
                        if !self.settings.languages.contains(&code.to_string()) {
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(format!("+ {}", label))
                                            .size(11.0)
                                            .color(egui::Color32::from_rgb(100, 140, 200)),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                to_add = Some(code.to_string());
                            }
                        }
                    }
                });

                // Apply changes
                if let Some(code) = to_add {
                    self.settings.languages.push(code);
                }
                if let Some(code) = to_remove {
                    self.settings.languages.retain(|c| c != &code);
                }
                if let Some(idx) = to_move_up {
                    if idx > 0 {
                        self.settings.languages.swap(idx, idx - 1);
                    }
                }

                ui.add_space(10.0);

                // Footer
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Auto-restarts daemon")
                            .size(10.0)
                            .color(egui::Color32::from_rgb(100, 100, 100)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let save_enabled = self.settings.has_changes();
                        if ui
                            .add_enabled(
                                save_enabled,
                                egui::Button::new(
                                    egui::RichText::new("Save").size(12.0).color(
                                        if save_enabled {
                                            egui::Color32::WHITE
                                        } else {
                                            egui::Color32::from_rgb(100, 100, 100)
                                        },
                                    ),
                                )
                                .fill(if save_enabled {
                                    egui::Color32::from_rgb(37, 99, 235)
                                } else {
                                    egui::Color32::from_rgb(60, 60, 60)
                                }),
                            )
                            .clicked()
                        {
                            self.save_and_restart();
                            self.show_settings = false;
                        }

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("Cancel")
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(180, 180, 180)),
                                )
                                .fill(egui::Color32::from_rgb(55, 55, 55)),
                            )
                            .clicked()
                        {
                            self.settings.reset();
                            self.show_settings = false;
                            self.capturing_hotkey = false;
                        }
                    });
                });
            }

            // === Log panel ===
            if self.show_logs {
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                ui.label(
                    egui::RichText::new("Daemon Log")
                        .size(12.0)
                        .color(egui::Color32::from_rgb(200, 200, 200))
                        .strong(),
                );
                ui.add_space(4.0);

                let log_frame = egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(20, 20, 20))
                    .rounding(egui::Rounding::same(4))
                    .inner_margin(egui::Margin::same(6));

                log_frame.show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(180.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            let lines = self.log_lines.lock().unwrap();
                            if lines.is_empty() {
                                ui.label(
                                    egui::RichText::new("No log entries yet...")
                                        .size(10.0)
                                        .color(egui::Color32::from_rgb(100, 100, 100))
                                        .monospace(),
                                );
                            } else {
                                for line in lines.iter() {
                                    // Color based on log level
                                    let color = if line.contains("ERROR") {
                                        egui::Color32::from_rgb(255, 80, 80)
                                    } else if line.contains("WARN") {
                                        egui::Color32::from_rgb(255, 200, 50)
                                    } else if line.contains("INFO") {
                                        egui::Color32::from_rgb(140, 160, 140)
                                    } else {
                                        egui::Color32::from_rgb(110, 110, 110)
                                    };
                                    // Truncate timestamp for compact display
                                    let display = if line.len() > 30 && line.starts_with("20") {
                                        &line[20..]
                                    } else {
                                        line
                                    };
                                    ui.label(
                                        egui::RichText::new(display)
                                            .size(10.0)
                                            .color(color)
                                            .monospace(),
                                    );
                                }
                            }
                        });
                });
            }
        });
        // Allow dragging from the top status bar area (first 50px) always,
        // and from anywhere when no panels are open
        let can_drag_anywhere = !self.show_settings && !self.show_logs;
        let should_drag = ctx.input(|i| {
            if i.pointer.any_pressed() {
                if can_drag_anywhere {
                    true
                } else if let Some(pos) = i.pointer.interact_pos() {
                    pos.y < 50.0
                } else {
                    false
                }
            } else {
                false
            }
        });
        if should_drag {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    }
}
