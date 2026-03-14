use voice_terminal::state::{AppState, DaemonState};

#[test]
fn test_default_values() {
    let state = AppState::default();
    
    assert_eq!(state.state, DaemonState::Idle);
    assert_eq!(state.target_app, "Unknown");
    assert_eq!(state.mode, "toggle");
    assert_eq!(state.last_text, "");
    // timestamp should be reasonable (within the last minute)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!((now - state.timestamp) < 60, "Timestamp should be recent");
}

#[test]
fn test_json_serialization_roundtrip() {
    let state = AppState {
        state: DaemonState::Recording,
        target_app: "Terminal".to_string(),
        mode: "push-to-talk".to_string(),
        last_text: "Hello world".to_string(),
        timestamp: 1234567890,
    };

    // Serialize to JSON
    let json = serde_json::to_string(&state).unwrap();
    
    // Deserialize back
    let deserialized: AppState = serde_json::from_str(&json).unwrap();
    
    assert_eq!(state, deserialized);
}

#[test]
fn test_daemon_state_snake_case_serialization() {
    // Test all variants serialize to snake_case
    let idle = serde_json::to_string(&DaemonState::Idle).unwrap();
    assert_eq!(idle, "\"idle\"");
    
    let listening = serde_json::to_string(&DaemonState::Listening).unwrap();
    assert_eq!(listening, "\"listening\"");
    
    let recording = serde_json::to_string(&DaemonState::Recording).unwrap();
    assert_eq!(recording, "\"recording\"");
    
    let processing = serde_json::to_string(&DaemonState::Processing).unwrap();
    assert_eq!(processing, "\"processing\"");
}

#[test]
fn test_framed_json_roundtrip() {
    let state = AppState {
        state: DaemonState::Listening,
        target_app: "Code Editor".to_string(),
        mode: "continuous".to_string(),
        last_text: "Test message".to_string(),
        timestamp: 9876543210,
    };

    // Test to_framed_json adds newline
    let framed = state.to_framed_json().unwrap();
    assert!(framed.ends_with('\n'), "Framed JSON should end with newline");
    
    // Test from_framed_json parses it back
    let parsed = AppState::from_framed_json(&framed).unwrap();
    assert_eq!(state, parsed);
    
    // Test that from_framed_json can handle strings without trailing newline too
    let without_newline = framed.trim_end_matches('\n');
    let parsed_no_newline = AppState::from_framed_json(without_newline).unwrap();
    assert_eq!(state, parsed_no_newline);
}

#[test]
fn test_daemon_state_display() {
    assert_eq!(DaemonState::Idle.to_string(), "idle");
    assert_eq!(DaemonState::Listening.to_string(), "listening");
    assert_eq!(DaemonState::Recording.to_string(), "recording");
    assert_eq!(DaemonState::Processing.to_string(), "processing");
}