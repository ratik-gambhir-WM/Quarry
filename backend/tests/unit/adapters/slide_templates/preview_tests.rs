use super::*;
use std::{os::unix::fs::PermissionsExt, sync::atomic::Ordering};

fn shell_path(path: &std::path::Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}
fn executable(directory: &std::path::Path, body: &str) -> PathBuf {
    let path = directory.join("fake-quicklook");
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path
}
fn document() -> super::Value {
    serde_json::from_value(serde_json::json!({"presentation":{"title":"Cancellation","preserveElementOrder":true,"showBranding":false,"slides":[]}})).unwrap()
}

#[test]
fn quicklook_output_limit_counts_decoded_utf8_bytes_across_chunks() {
    let mut invalid_at_limit = vec![0xff; 10_922];
    invalid_at_limit.extend(b"aa");
    let valid_across_chunks = format!("a{}bbb", "😀".repeat(8191)).into_bytes();
    assert_eq!(valid_across_chunks.len(), QUICK_LOOK_MAX_PROCESS_OUTPUT);
    let mut incomplete_at_eof = vec![b'a'; QUICK_LOOK_MAX_PROCESS_OUTPUT - 2];
    incomplete_at_eof.extend([0xf0, 0x9f]);

    for (bytes, expected_overflow) in [
        (invalid_at_limit, false),
        (vec![0xff; 10_923], true),
        (valid_across_chunks, false),
        (incomplete_at_eof, true),
    ] {
        let overflow = Arc::new(AtomicBool::new(false));
        drain_process_output(std::io::Cursor::new(bytes), Arc::clone(&overflow))
            .unwrap()
            .join()
            .unwrap()
            .unwrap();
        assert_eq!(overflow.load(Ordering::Acquire), expected_overflow);
    }
}

#[test]
fn quicklook_cancellation_kills_reaps_and_cleans_up_running_child() {
    let directory = tempdir().unwrap();
    let pid_path = directory.path().join("pid");
    let input_record = directory.path().join("input");
    let executable = executable(
        directory.path(),
        &format!(
            "printf '%s' \"$$\" > {}\nprintf '%s' \"$6\" > {}\nexec /bin/sleep 30",
            shell_path(&pid_path),
            shell_path(&input_record)
        ),
    );
    let generator =
        QuickLookTemplatePreviewGenerator::new(executable, 1024, 100, 100, Duration::from_secs(30));
    let cancellation = super::RequestCancellation::default();
    let operation_cancellation = cancellation.clone();
    let (send, receive) = std::sync::mpsc::channel();
    let operation = thread::spawn(move || {
        send.send(generator.generate_document_cancellable(
            b"pptx",
            &document(),
            Some(&operation_cancellation),
        ))
        .unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !input_record.exists() {
        assert!(Instant::now() < deadline, "fake QuickLook did not start");
        thread::sleep(Duration::from_millis(5));
    }
    cancellation.0.store(true, Ordering::Release);
    let result = receive.recv_timeout(Duration::from_secs(1));
    if result.is_err() {
        // Ensure a regressed test does not leave its intentionally long-running fake process alive.
        if let Ok(pid) = fs::read_to_string(&pid_path) {
            let _ = Command::new("kill").args(["-9", pid.trim()]).status();
        }
    }
    assert!(result
        .expect("cancellation must finish before the preview timeout")
        .unwrap()
        .is_none());
    operation.join().unwrap();
    let pid = fs::read_to_string(&pid_path).unwrap();
    assert!(
        !Command::new("kill")
            .args(["-0", pid.trim()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success(),
        "QuickLook child must be dead and reaped"
    );
    let input = PathBuf::from(fs::read_to_string(input_record).unwrap());
    assert!(!input.exists(), "temporary preview input must be removed");
}

#[test]
fn quicklook_accepts_uppercase_png_extension_and_honors_byte_limit() {
    let directory = tempdir().unwrap();
    let png = directory.path().join("source.png");
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend(1_u32.to_be_bytes());
    bytes.extend(1_u32.to_be_bytes());
    fs::write(&png, &bytes).unwrap();
    let executable = executable(
        directory.path(),
        &format!("cp {} \"$5/result.PNG\"", shell_path(&png)),
    );
    let generator = QuickLookTemplatePreviewGenerator::new(
        executable.clone(),
        1024,
        100,
        100,
        Duration::from_secs(2),
    );
    assert_eq!(generator.generate(b"pptx").unwrap().unwrap().bytes, bytes);
    let oversized = QuickLookTemplatePreviewGenerator::new(
        executable,
        bytes.len() - 1,
        100,
        100,
        Duration::from_secs(2),
    );
    assert!(oversized.generate(b"pptx").unwrap().is_none());
}

#[test]
fn quicklook_enforces_independent_stdout_and_stderr_buffer_limits() {
    let directory = tempdir().unwrap();
    let png = directory.path().join("source.png");
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend(1_u32.to_be_bytes());
    bytes.extend(1_u32.to_be_bytes());
    fs::write(&png, &bytes).unwrap();
    let script = executable(
        directory.path(),
        &format!(
            "cp {} \"$5/result.png\"\nhead -c 32768 /dev/zero\nhead -c 32768 /dev/zero >&2",
            shell_path(&png),
        ),
    );
    let generator =
        QuickLookTemplatePreviewGenerator::new(script, 1024, 100, 100, Duration::from_secs(2));
    assert!(
        generator.generate(b"pptx").unwrap().is_some(),
        "each stream may reach exactly 32 KiB independently"
    );

    for stream in ["", ">&2"] {
        let pid_path = directory.path().join("overflow-pid");
        executable(
                directory.path(),
                &format!(
                    "printf '%s' \"$$\" > {}\ncp {} \"$5/result.png\"\nhead -c 32769 /dev/zero {stream}\nexec /bin/sleep 30",
                    shell_path(&pid_path),
                    shell_path(&png),
                ),
            );
        let started = Instant::now();
        assert!(
            generator.generate(b"pptx").unwrap().is_none(),
            "overflow must reject even an otherwise valid preview"
        );
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "overflow must stop the running process before its timeout"
        );
        let pid = fs::read_to_string(pid_path).unwrap();
        assert!(
            !Command::new("kill")
                .args(["-0", pid.trim()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success(),
            "overflowed child must be killed and reaped"
        );
    }
}
