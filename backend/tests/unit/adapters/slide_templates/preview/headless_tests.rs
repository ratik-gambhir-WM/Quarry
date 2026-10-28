use super::*;

#[test]
#[ignore = "requires full Chromium with tab visibility and a loopback DevTools socket"]
fn capture_preparation_resumes_animation_frames_after_target_is_backgrounded() {
    let directory = tempdir().unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    let executable = crate::slide_template_test_support::chromium_candidates()
        .into_iter()
        .find(|path| {
            path.is_file()
                && !path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("chrome-headless-shell"))
        })
        .expect("full Chromium is installed for the tab-backgrounding fixture");
    let child = Command::new(executable)
        .args([
            "--headless",
            "--disable-gpu",
            "--no-first-run",
            "--no-startup-window",
            "--remote-debugging-port=0",
            "--remote-debugging-address=127.0.0.1",
        ])
        .arg(format!("--user-data-dir={}", directory.path().display()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut browser = BrowserProcess(child);
    let port = loop {
        if let Some(port) = fs::read_to_string(directory.path().join("DevToolsActivePort"))
            .ok()
            .and_then(|value| value.lines().next().and_then(|v| v.parse::<u16>().ok()))
        {
            break port;
        }
        assert!(
            Instant::now() < deadline && browser.0.try_wait().unwrap().is_none(),
            "Chromium startup failed"
        );
        thread::sleep(Duration::from_millis(20));
    };
    let page: Value = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap()
        .put(format!("http://127.0.0.1:{port}/json/new?about:blank"))
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .unwrap();
    let (socket, _) = tungstenite::connect(page["webSocketDebuggerUrl"].as_str().unwrap()).unwrap();
    let mut cdp = Cdp {
        socket,
        next_id: 0,
        deadline,
        origin: "null".to_owned(),
        cancellation: None,
        loaded_documents: HashSet::new(),
    };
    let other = cdp
        .call(
            "Target.createTarget",
            json!({"url":"about:blank","background":false}),
        )
        .unwrap();
    cdp.call(
        "Target.activateTarget",
        json!({"targetId":other["targetId"]}),
    )
    .unwrap();
    loop {
        if cdp.evaluate("document.visibilityState", false).unwrap() == "hidden" {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "fixture must background the render target"
        );
        thread::sleep(Duration::from_millis(20));
    }
    cdp.evaluate("globalThis.previewFrameRan=false;requestAnimationFrame(()=>{globalThis.previewFrameRan=true});true",false).unwrap();
    thread::sleep(Duration::from_millis(100));
    assert_eq!(
        cdp.evaluate("globalThis.previewFrameRan", false).unwrap(),
        false,
        "backgrounding must reproduce the stalled animation frame"
    );

    cdp.prepare_capture()
        .expect("capture preparation must reactivate the render target");
    assert_eq!(
        cdp.evaluate("globalThis.previewFrameRan", false).unwrap(),
        true
    );
    assert_eq!(
        cdp.evaluate("document.visibilityState", false).unwrap(),
        "visible"
    );
}
