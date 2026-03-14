use anyhow::Result;
use enigo::{Enigo, Keyboard, Settings};

/// Injects text into the currently focused application using OS-level keyboard simulation
pub struct TextInjector {
    enigo: Enigo,
}

impl TextInjector {
    pub fn new() -> Result<Self> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow::anyhow!("Failed to initialize text injector: {:?}", e))?;
        Ok(Self { enigo })
    }

    /// Type the given text into the currently focused application
    pub fn inject_text(&mut self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        tracing::info!("Injecting text: \"{}\"", text);

        self.enigo
            .text(text)
            .map_err(|e| anyhow::anyhow!("Failed to inject text: {:?}", e))?;

        Ok(())
    }
}
