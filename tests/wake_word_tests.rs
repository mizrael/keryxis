use keryxis::input::WakeWordDetector;

#[test]
fn test_wake_word_detection_exact_match() {
    let detector = WakeWordDetector::new("hey terminal");
    assert!(detector.detect("hey terminal"));
}

#[test]
fn test_wake_word_detection_case_insensitive() {
    let detector = WakeWordDetector::new("hey terminal");
    assert!(detector.detect("Hey Terminal"));
    assert!(detector.detect("HEY TERMINAL"));
    assert!(detector.detect("hEy TeRmInAl"));
}

#[test]
fn test_wake_word_detection_within_sentence() {
    let detector = WakeWordDetector::new("hey terminal");
    assert!(detector.detect("I said hey terminal run this command"));
    assert!(detector.detect("hey terminal, open a file"));
}

#[test]
fn test_wake_word_no_false_positive() {
    let detector = WakeWordDetector::new("hey terminal");
    assert!(!detector.detect("hello world"));
    assert!(!detector.detect("hey there"));
    assert!(!detector.detect("terminal only"));
    assert!(!detector.detect(""));
}

#[test]
fn test_wake_word_custom_word() {
    let detector = WakeWordDetector::new("computer");
    assert!(detector.detect("computer"));
    assert!(detector.detect("Computer, what time is it?"));
    assert!(!detector.detect("hey terminal"));
}

#[test]
fn test_wake_word_get_word() {
    let detector = WakeWordDetector::new("hey terminal");
    assert_eq!(detector.wake_word(), "hey terminal");
}

#[test]
fn test_wake_word_listening_state() {
    let detector = WakeWordDetector::new("hey terminal");
    assert!(!detector.is_listening());

    detector.set_listening(true);
    assert!(detector.is_listening());

    detector.set_listening(false);
    assert!(!detector.is_listening());
}

#[test]
fn test_wake_word_empty_input() {
    let detector = WakeWordDetector::new("hey terminal");
    assert!(!detector.detect(""));
}

#[test]
fn test_wake_word_unicode() {
    let detector = WakeWordDetector::new("hola computadora");
    assert!(detector.detect("Hola Computadora"));
    assert!(detector.detect("digo hola computadora ahora"));
    assert!(!detector.detect("hello computer"));
}

#[test]
fn test_wake_word_with_punctuation() {
    let detector = WakeWordDetector::new("hey terminal");
    // Whisper often inserts punctuation — should still match
    assert!(detector.detect("Hey terminal."));
    assert!(detector.detect("Hey, terminal!"));
    // This shouldn't match — different words between hey and terminal
    assert!(!detector.detect("Hey there, nice terminal"));
}

#[test]
fn test_strip_wake_word_basic() {
    let detector = WakeWordDetector::new("hey terminal");
    assert_eq!(detector.strip_wake_word("Hey terminal list files"), "list files");
}

#[test]
fn test_strip_wake_word_with_punctuation() {
    let detector = WakeWordDetector::new("hey terminal");
    assert_eq!(detector.strip_wake_word("Hey terminal, list files"), "list files");
    assert_eq!(detector.strip_wake_word("Hey terminal! Do something"), "Do something");
}

#[test]
fn test_strip_wake_word_only() {
    let detector = WakeWordDetector::new("hey terminal");
    assert_eq!(detector.strip_wake_word("Hey terminal"), "");
    assert_eq!(detector.strip_wake_word("hey terminal."), "");
}

#[test]
fn test_strip_wake_word_preserves_case() {
    let detector = WakeWordDetector::new("hey terminal");
    assert_eq!(detector.strip_wake_word("Hey Terminal Write Hello World"), "Write Hello World");
}

#[test]
fn test_strip_wake_word_no_match() {
    let detector = WakeWordDetector::new("hey terminal");
    assert_eq!(detector.strip_wake_word("hello world"), "hello world");
}
