#[cfg(feature = "gui")]
use crate::state::{AppState, DaemonState};

#[cfg(feature = "gui")]
use std::io::{BufRead, BufReader};
#[cfg(feature = "gui")]
use std::os::unix::net::UnixStream;
#[cfg(feature = "gui")]
use std::sync::{Arc, Mutex};

/// Run the floating overlay GUI. Connects to the daemon socket and displays state.
#[cfg(feature = "gui")]
pub fn run_overlay(
    sock_path: &std::path::Path,
    _opacity: f32,
    _position: &str,
) -> anyhow::Result<()> {
    let state = Arc::new(Mutex::new(AppState::default()));

    // Connect to daemon socket in background thread
    let state_reader = state.clone();
    let sock_path_owned = sock_path.to_path_buf();
    std::thread::spawn(move || loop {
        match UnixStream::connect(&sock_path_owned) {
            Ok(stream) => {
                tracing::info!("Connected to daemon socket");
                let reader = BufReader::new(stream);
                for line in reader.lines() {
                    match line {
                        Ok(l) => {
                            if let Ok(new_state) = AppState::from_framed_json(&l) {
                                let mut s = state_reader.lock().unwrap();
                                *s = new_state;
                            }
                        }
                        Err(_) => break,
                    }
                }
                tracing::warn!("Disconnected from daemon, retrying...");
            }
            Err(_) => {}
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    });

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([240.0, 50.0])
            .with_always_on_top()
            .with_decorations(false)
            .with_transparent(true)
            .with_drag_and_drop(true),
        ..Default::default()
    };

    let state_gui = state.clone();
    eframe::run_native(
        "Voice Terminal",
        native_options,
        Box::new(move |_cc| {
            Ok(Box::new(OverlayApp {
                state: state_gui,
                dragging: false,
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Overlay error: {}", e))
}

#[cfg(feature = "gui")]
struct OverlayApp {
    state: Arc<Mutex<AppState>>,
    dragging: bool,
}

#[cfg(feature = "gui")]
impl eframe::App for OverlayApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let state = self.state.lock().unwrap().clone();

        ctx.request_repaint_after(std::time::Duration::from_millis(200));

        // Allow dragging the window by clicking anywhere on it
        let interact = ctx.input(|i| {
            if i.pointer.any_pressed() {
                true
            } else {
                false
            }
        });
        if interact && !self.dragging {
            self.dragging = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if ctx.input(|i| i.pointer.any_released()) {
            self.dragging = false;
        }

        let bg = egui::Color32::from_rgba_unmultiplied(30, 30, 30, 200);
        let frame = egui::Frame::NONE
            .fill(bg)
            .rounding(egui::Rounding::same(8))
            .inner_margin(egui::Margin::same(8));

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            ui.horizontal(|ui| {
                // State indicator circle
                let (color, label) = match state.state {
                    DaemonState::Idle => (egui::Color32::GRAY, "💤"),
                    DaemonState::Listening => (egui::Color32::from_rgb(50, 205, 50), "👂"),
                    DaemonState::Recording => {
                        // Pulse the red circle
                        let t = ctx.input(|i| i.time);
                        let pulse = ((t * 3.0).sin() * 0.3 + 0.7) as f32;
                        let r = (255.0 * pulse) as u8;
                        (egui::Color32::from_rgb(r, 40, 40), "🎙️")
                    }
                    DaemonState::Processing => {
                        (egui::Color32::from_rgb(255, 200, 50), "⏳")
                    }
                };

                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 6.0, color);

                ui.spacing_mut().item_spacing.x = 6.0;

                ui.label(egui::RichText::new(label).size(14.0));

                ui.label(
                    egui::RichText::new(format!("→ {}", state.target_app))
                        .size(13.0)
                        .color(egui::Color32::from_rgb(200, 200, 200)),
                );
            });
        });
    }
}
