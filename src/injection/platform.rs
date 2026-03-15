use anyhow::Result;
use enigo::{Enigo, Keyboard, Settings};

use crate::ui::active_window;

/// Injects text into the currently focused application using OS-level keyboard simulation
pub struct TextInjector {
    enigo: Enigo,
}

impl TextInjector {
    pub fn new() -> Result<Self> {
        let enigo = Enigo::new(&Settings::default()).map_err(|e| {
            let msg = format!("{:?}", e);
            if msg.contains("NoPermission") || msg.contains("permission") {
                anyhow::anyhow!(
                    "Accessibility permission required!\n\n\
                     On macOS, go to:\n\
                     System Settings → Privacy & Security → Accessibility\n\
                     and add your terminal app (e.g., Terminal.app, iTerm2, VS Code).\n\n\
                     Then restart keryxis."
                )
            } else {
                anyhow::anyhow!("Failed to initialize text injector: {:?}", e)
            }
        })?;
        Ok(Self { enigo })
    }

    /// Type the given text into the currently focused application.
    /// Skips injection if the overlay itself is focused.
    pub fn inject_text(&mut self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        let active = active_window::get_active_window_name();
        if active.to_lowercase().contains("keryxis") || active == "keryxis" {
            tracing::info!("Skipping injection — overlay is focused (active: \"{}\")", active);
            return Ok(());
        }

        tracing::info!("Injecting text: \"{}\"", text);

        self.enigo
            .text(text)
            .map_err(|e| anyhow::anyhow!("Failed to inject text: {:?}", e))?;

        Ok(())
    }
}
