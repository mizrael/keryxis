use keryxis::ui::active_window;

#[test]
fn test_get_active_window_returns_string() {
    let name = active_window::get_active_window_name();
    assert!(!name.is_empty());
}

#[test]
fn test_get_active_window_no_panic() {
    for _ in 0..10 {
        let _ = active_window::get_active_window_name();
    }
}
