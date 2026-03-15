use keryxis::input::HotkeyListener;

#[test]
fn test_hotkey_parse_simple_key() {
    let listener = HotkeyListener::new("Space");
    assert!(listener.is_ok(), "Failed to parse 'Space'");
}

#[test]
fn test_hotkey_parse_with_modifier() {
    let listener = HotkeyListener::new("Alt+Space");
    assert!(listener.is_ok(), "Failed to parse 'Alt+Space'");
}

#[test]
fn test_hotkey_parse_multiple_modifiers() {
    let listener = HotkeyListener::new("Ctrl+Shift+R");
    assert!(listener.is_ok(), "Failed to parse 'Ctrl+Shift+R'");
}

#[test]
fn test_hotkey_parse_function_keys() {
    for i in 1..=12 {
        let key = format!("F{}", i);
        let listener = HotkeyListener::new(&key);
        assert!(listener.is_ok(), "Failed to parse '{}'", key);
    }
}

#[test]
fn test_hotkey_parse_letter_keys() {
    for c in 'a'..='z' {
        let key = c.to_string();
        let listener = HotkeyListener::new(&key);
        assert!(listener.is_ok(), "Failed to parse '{}'", key);
    }
}

#[test]
fn test_hotkey_parse_meta_key() {
    let listener = HotkeyListener::new("Cmd+Space");
    assert!(listener.is_ok(), "Failed to parse 'Cmd+Space'");

    let listener = HotkeyListener::new("Meta+Space");
    assert!(listener.is_ok(), "Failed to parse 'Meta+Space'");

    let listener = HotkeyListener::new("Super+Space");
    assert!(listener.is_ok(), "Failed to parse 'Super+Space'");
}

#[test]
fn test_hotkey_parse_special_keys() {
    for key_name in &["Tab", "Return", "Enter", "Escape", "Esc", "Backspace"] {
        let listener = HotkeyListener::new(key_name);
        assert!(listener.is_ok(), "Failed to parse '{}'", key_name);
    }
}

#[test]
fn test_hotkey_parse_case_insensitive() {
    let listener = HotkeyListener::new("alt+space");
    assert!(listener.is_ok(), "Failed to parse 'alt+space'");

    let listener = HotkeyListener::new("ALT+SPACE");
    assert!(listener.is_ok(), "Failed to parse 'ALT+SPACE'");
}

#[test]
fn test_hotkey_parse_invalid_key() {
    let listener = HotkeyListener::new("InvalidKey");
    assert!(listener.is_err());
}

#[test]
fn test_hotkey_parse_empty() {
    let listener = HotkeyListener::new("");
    // Empty string is technically parseable (the empty string is the trigger)
    // but should fail since it's not a valid key
    assert!(listener.is_err());
}

#[test]
fn test_hotkey_parse_complex_combo() {
    let listener = HotkeyListener::new("Ctrl+Alt+Shift+F12");
    assert!(listener.is_ok(), "Failed to parse 'Ctrl+Alt+Shift+F12'");
}

#[test]
fn test_hotkey_parse_option_alias() {
    // "Option" is macOS alias for Alt
    let listener = HotkeyListener::new("Option+Space");
    assert!(listener.is_ok(), "Failed to parse 'Option+Space'");
}

#[test]
fn test_hotkey_parse_control_alias() {
    let listener = HotkeyListener::new("Control+A");
    assert!(listener.is_ok(), "Failed to parse 'Control+A'");
}
