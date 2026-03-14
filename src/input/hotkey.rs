use anyhow::Result;
use rdev::{EventType, Key, listen};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

/// Events emitted by the hotkey listener
#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    Activated,
    Deactivated,
}

/// Listens for global hotkey events
pub struct HotkeyListener {
    trigger_key: Key,
    modifier_keys: Vec<Key>,
}

impl HotkeyListener {
    /// Create a new hotkey listener from a hotkey string (e.g., "Alt+Space", "Ctrl+Shift+R")
    pub fn new(hotkey_str: &str) -> Result<Self> {
        let parts: Vec<&str> = hotkey_str.split('+').collect();
        if parts.is_empty() {
            anyhow::bail!("Invalid hotkey string: {}", hotkey_str);
        }

        let trigger_key = parse_key(parts.last().unwrap().trim())?;
        let modifier_keys: Vec<Key> = parts[..parts.len() - 1]
            .iter()
            .map(|s| parse_key(s.trim()))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            trigger_key,
            modifier_keys,
        })
    }

    /// Start listening for hotkey events.
    /// Returns a receiver that emits HotkeyEvent when the hotkey is toggled.
    pub fn start(self) -> Result<mpsc::Receiver<HotkeyEvent>> {
        let (tx, rx) = mpsc::channel();
        let is_active = Arc::new(AtomicBool::new(false));
        let modifier_pressed: Arc<Vec<AtomicBool>> = Arc::new(
            self.modifier_keys
                .iter()
                .map(|_| AtomicBool::new(false))
                .collect(),
        );

        let trigger_key = self.trigger_key;
        let modifier_keys = self.modifier_keys.clone();

        std::thread::spawn(move || {
            let modifier_pressed = modifier_pressed;
            let is_active = is_active;

            if let Err(e) = listen(move |event| {
                match event.event_type {
                    EventType::KeyPress(key) => {
                        // Track modifier state
                        for (i, mod_key) in modifier_keys.iter().enumerate() {
                            if key == *mod_key {
                                modifier_pressed[i].store(true, Ordering::SeqCst);
                            }
                        }

                        // Check if trigger key pressed with all modifiers held
                        if key == trigger_key {
                            let all_modifiers_held = modifier_pressed
                                .iter()
                                .all(|m| m.load(Ordering::SeqCst));

                            if all_modifiers_held {
                                let was_active = is_active.fetch_xor(true, Ordering::SeqCst);
                                let event = if was_active {
                                    HotkeyEvent::Deactivated
                                } else {
                                    HotkeyEvent::Activated
                                };
                                let _ = tx.send(event);
                            }
                        }
                    }
                    EventType::KeyRelease(key) => {
                        for (i, mod_key) in modifier_keys.iter().enumerate() {
                            if key == *mod_key {
                                modifier_pressed[i].store(false, Ordering::SeqCst);
                            }
                        }
                    }
                    _ => {}
                }
            }) {
                tracing::error!("Hotkey listener error: {:?}", e);
            }
        });

        Ok(rx)
    }
}

/// Parse a key name string into an rdev Key
fn parse_key(name: &str) -> Result<Key> {
    match name.to_lowercase().as_str() {
        "alt" | "option" => Ok(Key::Alt),
        "ctrl" | "control" => Ok(Key::ControlLeft),
        "shift" => Ok(Key::ShiftLeft),
        "meta" | "cmd" | "command" | "super" => Ok(Key::MetaLeft),
        "space" => Ok(Key::Space),
        "tab" => Ok(Key::Tab),
        "return" | "enter" => Ok(Key::Return),
        "escape" | "esc" => Ok(Key::Escape),
        "backspace" => Ok(Key::Backspace),
        "f1" => Ok(Key::F1),
        "f2" => Ok(Key::F2),
        "f3" => Ok(Key::F3),
        "f4" => Ok(Key::F4),
        "f5" => Ok(Key::F5),
        "f6" => Ok(Key::F6),
        "f7" => Ok(Key::F7),
        "f8" => Ok(Key::F8),
        "f9" => Ok(Key::F9),
        "f10" => Ok(Key::F10),
        "f11" => Ok(Key::F11),
        "f12" => Ok(Key::F12),
        "a" => Ok(Key::KeyA),
        "b" => Ok(Key::KeyB),
        "c" => Ok(Key::KeyC),
        "d" => Ok(Key::KeyD),
        "e" => Ok(Key::KeyE),
        "f" => Ok(Key::KeyF),
        "g" => Ok(Key::KeyG),
        "h" => Ok(Key::KeyH),
        "i" => Ok(Key::KeyI),
        "j" => Ok(Key::KeyJ),
        "k" => Ok(Key::KeyK),
        "l" => Ok(Key::KeyL),
        "m" => Ok(Key::KeyM),
        "n" => Ok(Key::KeyN),
        "o" => Ok(Key::KeyO),
        "p" => Ok(Key::KeyP),
        "q" => Ok(Key::KeyQ),
        "r" => Ok(Key::KeyR),
        "s" => Ok(Key::KeyS),
        "t" => Ok(Key::KeyT),
        "u" => Ok(Key::KeyU),
        "v" => Ok(Key::KeyV),
        "w" => Ok(Key::KeyW),
        "x" => Ok(Key::KeyX),
        "y" => Ok(Key::KeyY),
        "z" => Ok(Key::KeyZ),
        _ => anyhow::bail!("Unknown key: '{}'. Supported: a-z, f1-f12, alt, ctrl, shift, meta/cmd, space, tab, return, escape, backspace", name),
    }
}
