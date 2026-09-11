//! Tests for the Office conversion mechanism.

use super::*;
use std::io::Cursor;

#[test]
fn converter_logs_are_drained_but_retained_only_up_to_the_limit() {
    let (retained, truncated) = drain_bounded(Cursor::new(b"0123456789"), 4).unwrap();

    assert_eq!(retained, b"0123");
    assert!(truncated);
}

#[cfg(unix)]
#[test]
fn converter_process_is_terminated_at_the_hard_timeout() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "sleep 2"]);

    let error = run_command_with_timeout(&mut command, Duration::from_millis(50)).unwrap_err();

    assert!(error.contains("timed out"));
    assert!(error.contains("terminated"));
}
