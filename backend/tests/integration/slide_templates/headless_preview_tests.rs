//! Requires an installed Chromium; run explicitly with `--ignored`.
use crate::domains::templates::slide_templates::*;
use serde_json::json;

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

struct FixtureServer {
    address: SocketAddr,
    requests: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl FixtureServer {
    fn start(html: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_requests = requests.clone();
        let thread_stop = stop.clone();
        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        socket
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .unwrap();
                        let mut bytes = [0; 4096];
                        if socket.read(&mut bytes).is_ok() {
                            thread_requests.fetch_add(1, Ordering::Relaxed);
                            let response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
                                html.len()
                            );
                            let _ = socket.write_all(response.as_bytes());
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("fixture listener failed: {error}"),
                }
            }
        });
        Self {
            address,
            requests,
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
fn document() -> CanvasDocument {
    serde_json::from_value(json!({"presentation":{"title":"Injected fixture title","showBranding":false,"preserveElementOrder":true,"slides":[{"id":"slide","name":"slide","width":320,"height":180,"backgroundColor":"FFFFFF","elements":[]}]}})).unwrap()
}
fn enable_diagnostics() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();
}

#[test]
#[ignore = "launches installed Chromium and loopback HTTP fixtures"]
fn native_headless_injects_input_blocks_external_requests_and_bounds_capture() {
    enable_diagnostics();
    let external = FixtureServer::start("unexpected external response".to_owned());
    let html = format!(
        r#"<!doctype html><html><head><style>body{{margin:0}}#surface{{width:320px;height:180px;background:#070154;color:white;}}</style></head><body><div id="surface" data-template-preview-surface>Fixture preview</div><script>
    (async()=>{{
      let state='ready';
      const input=window.__TTS_MERMAID_TEMPLATE_PREVIEW_INPUT__;
      const sizes=new URL(location.href).searchParams.getAll('maxDimension');
      if(input?.presentation?.title!=='Injected fixture title'||sizes.length!==1||sizes[0]!=='640')state='error';
      try{{await fetch('http://{}/forbidden');state='error';}}catch{{}}
      const marker=document.createElement('div');marker.dataset.previewState=state;document.body.append(marker);
    }})();</script></body></html>"#,
        external.address
    );
    let fixture = FixtureServer::start(html);
    let generator = headless(
        format!("http://{}/preview?maxDimension=10", fixture.address),
        1024 * 1024,
        4096,
        640,
        Duration::from_secs(15),
    );
    let preview = generator
        .generate_document(&[], &document())
        .unwrap()
        .expect("native browser should render the verified fixture");
    assert_eq!((preview.width, preview.height), (320, 180));
    assert!(preview.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(fixture.requests.load(Ordering::Relaxed) > 0);
    assert_eq!(
        external.requests.load(Ordering::Relaxed),
        0,
        "cross-origin request must not reach the external listener"
    );
}

#[test]
#[ignore = "launches installed Chromium and loopback HTTP fixtures"]
fn native_headless_returns_unavailable_when_page_never_becomes_ready() {
    enable_diagnostics();
    let fixture = FixtureServer::start("<!doctype html><body>no preview marker</body>".to_owned());
    let generator = headless(
        format!("http://{}/preview", fixture.address),
        1024 * 1024,
        4096,
        640,
        Duration::from_secs(2),
    );
    let started = Instant::now();
    assert!(generator
        .generate_document(&[], &document())
        .unwrap()
        .is_none());
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
#[ignore = "launches installed Chromium and loopback HTTP fixtures"]
fn native_headless_cancellation_stops_the_browser_before_preview_timeout() {
    enable_diagnostics();
    use crate::domains::templates::slide_templates::RequestCancellation;
    let fixture = FixtureServer::start("<!doctype html><body>no preview marker</body>".to_owned());
    let generator = headless(
        format!("http://{}/preview", fixture.address),
        1024 * 1024,
        4096,
        640,
        Duration::from_secs(15),
    );
    let cancellation = RequestCancellation::default();
    let trigger = cancellation.clone();
    let cancel_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        trigger.0.store(true, Ordering::Release);
    });
    let started = Instant::now();
    assert!(generator
        .generate_document_cancellable(&[], &document(), Some(&cancellation))
        .unwrap()
        .is_none());
    cancel_thread.join().unwrap();
    assert!(started.elapsed() < Duration::from_secs(3));
}

fn headless(
    url: String,
    max_bytes: usize,
    max_dimension: u32,
    size: u32,
    timeout: Duration,
) -> NativePreview {
    let executable = crate::slide_template_test_support::chromium_candidates()
        .into_iter()
        .next()
        .expect("Chromium must be installed for explicit browser validation");
    NativePreview(Arc::new(
        crate::adapters::slide_templates::preview::HeadlessTemplatePreviewGenerator::new(
            executable,
            url,
            max_bytes,
            max_dimension,
            size,
            timeout,
        ),
    ))
}
