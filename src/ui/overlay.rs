#[cfg(feature = "gui")]
use crate::config::{ActivationMode, AppConfig, ModelSize};
#[cfg(feature = "gui")]
use crate::state::{AppState, DaemonCommand, DaemonState};

#[cfg(feature = "gui")]
use std::io::{BufRead, BufReader, Write};
#[cfg(feature = "gui")]
use std::os::unix::net::UnixStream;
#[cfg(feature = "gui")]
use std::sync::{Arc, Mutex};

#[cfg(feature = "gui")]
struct DaemonConnection {
    state: Arc<Mutex<AppState>>,
    stream: Arc<Mutex<Option<UnixStream>>>,
    connected: Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(feature = "gui")]
impl DaemonConnection {
    fn new(sock_path: &std::path::Path) -> Self {
        let state = Arc::new(Mutex::new(AppState::default()));
        let stream: Arc<Mutex<Option<UnixStream>>> = Arc::new(Mutex::new(None));
        let connected = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let state_r = state.clone();
        let stream_w = stream.clone();
        let connected_w = connected.clone();
        let sock = sock_path.to_path_buf();

        std::thread::spawn(move || loop {
            match UnixStream::connect(&sock) {
                Ok(s) => {
                    // Store a clone for sending commands
                    if let Ok(writer_clone) = s.try_clone() {
                        *stream_w.lock().unwrap() = Some(writer_clone);
                    }
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
                    *stream_w.lock().unwrap() = None;
                }
                Err(_) => {
                    connected_w.store(false, std::sync::atomic::Ordering::SeqCst);
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        });

        Self {
            state,
            stream,
            connected,
        }
    }

    fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn send_command(&self, cmd: &DaemonCommand) {
        if let Ok(mut guard) = self.stream.lock() {
            if let Some(ref mut s) = *guard {
                if let Ok(json) = cmd.to_framed_json() {
                    let _ = s.write_all(json.as_bytes());
                    let _ = s.flush();
                }
            }
        }
    }
}

/// Run the floating overlay GUI.
#[cfg(feature = "gui")]
pub fn run_overlay(
    sock_path: &std::path::Path,
    _opacity: f32,
    _position: &str,
) -> anyhow::Result<()> {
    let conn = DaemonConnection::new(sock_path);

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
        "Voice Terminal",
        native_options,
        Box::new(move |_cc| {
            let config = AppConfig::load().unwrap_or_default();
            Ok(Box::new(OverlayApp {
                conn,
                show_settings: false,
                settings: SettingsState::from_config(&config),
                capturing_hotkey: false,
                captured_keys: Vec::new(),
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
    language: String,
    original_mode: ActivationMode,
    original_hotkey: String,
    original_wake_word: String,
    original_model: ModelSize,
    original_language: String,
}

#[cfg(feature = "gui")]
impl SettingsState {
    fn from_config(config: &AppConfig) -> Self {
        Self {
            mode: config.activation.mode.clone(),
            hotkey: config.activation.hotkey.clone(),
            wake_word: config.activation.wake_word.clone(),
            model: config.whisper.model_size.clone(),
            language: config.whisper.language.clone(),
            original_mode: config.activation.mode.clone(),
            original_hotkey: config.activation.hotkey.clone(),
            original_wake_word: config.activation.wake_word.clone(),
            original_model: config.whisper.model_size.clone(),
            original_language: config.whisper.language.clone(),
        }
    }

    fn has_changes(&self) -> bool {
        self.mode != self.original_mode
            || self.hotkey != self.original_hotkey
            || self.wake_word != self.original_wake_word
            || self.model != self.original_model
            || self.language != self.original_language
    }

    fn reset(&mut self) {
        self.mode = self.original_mode.clone();
        self.hotkey = self.original_hotkey.clone();
        self.wake_word = self.original_wake_word.clone();
        self.model = self.original_model.clone();
        self.language = self.original_language.clone();
    }

    fn apply(&mut self) {
        self.original_mode = self.mode.clone();
        self.original_hotkey = self.hotkey.clone();
        self.original_wake_word = self.wake_word.clone();
        self.original_model = self.model.clone();
        self.original_language = self.language.clone();
    }
}

#[cfg(feature = "gui")]
struct OverlayApp {
    conn: DaemonConnection,
    show_settings: bool,
    settings: SettingsState,
    capturing_hotkey: bool,
    captured_keys: Vec<egui::Key>,
}

#[cfg(feature = "gui")]
impl OverlayApp {
    fn save_and_restart(&mut self) {
        if let Ok(mut config) = AppConfig::load() {
            config.activation.mode = self.settings.mode.clone();
            config.activation.hotkey = self.settings.hotkey.clone();
            config.activation.wake_word = self.settings.wake_word.clone();
            config.whisper.model_size = self.settings.model.clone();
            config.whisper.language = self.settings.language.clone();

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

        ctx.request_repaint_after(std::time::Duration::from_millis(200));

        let target_height = if self.show_settings { 380.0 } else { 50.0 };
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(340.0, target_height)));

        let bg = egui::Color32::from_rgba_unmultiplied(30, 30, 30, 220);
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
                    let gear_color = if self.show_settings {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::from_rgb(150, 150, 150)
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("⚙").size(14.0).color(gear_color),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        self.show_settings = !self.show_settings;
                        if !self.show_settings {
                            self.settings.reset();
                            self.capturing_hotkey = false;
                        }
                    }
                });
            });

            // === Settings panel ===
            if self.show_settings {
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                // Header + daemon status
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("⚙ Settings")
                            .size(13.0)
                            .color(egui::Color32::from_rgb(220, 220, 220))
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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

                // Language
                ui.label(
                    egui::RichText::new("Language")
                        .size(11.0)
                        .color(egui::Color32::from_rgb(140, 140, 140)),
                );
                let lang_label = match self.settings.language.as_str() {
                    "auto" => "Auto-detect",
                    "en" => "English",
                    "it" => "Italian",
                    "es" => "Spanish",
                    "fr" => "French",
                    "de" => "German",
                    "pt" => "Portuguese",
                    "ja" => "Japanese",
                    "zh" => "Chinese",
                    other => other,
                };
                egui::ComboBox::from_id_salt("lang_select")
                    .selected_text(lang_label)
                    .width(ui.available_width() - 8.0)
                    .show_ui(ui, |ui| {
                        for (code, label) in [
                            ("auto", "Auto-detect"),
                            ("en", "English"),
                            ("it", "Italian"),
                            ("es", "Spanish"),
                            ("fr", "French"),
                            ("de", "German"),
                            ("pt", "Portuguese"),
                            ("ja", "Japanese"),
                            ("zh", "Chinese"),
                        ] {
                            ui.selectable_value(
                                &mut self.settings.language,
                                code.to_string(),
                                label,
                            );
                        }
                    });

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
        });

        // Dragging (only when settings are closed)
        if !self.show_settings {
            let interact = ctx.input(|i| i.pointer.any_pressed());
            if interact {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        }
    }
}
