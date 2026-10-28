//! Native Chrome DevTools Protocol preview rendering against the configured canvas page.
use super::*;
use anyhow::{ensure, Context};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::{collections::HashSet, net::TcpStream, sync::Mutex};
use tungstenite::{stream::MaybeTlsStream, Message, WebSocket};

pub struct HeadlessTemplatePreviewGenerator {
    executable: PathBuf,
    render_url: String,
    max_bytes: usize,
    max_dimension: u32,
    render_size: u32,
    timeout: Duration,
    queue: Mutex<()>,
}
impl HeadlessTemplatePreviewGenerator {
    pub fn new(
        executable: PathBuf,
        render_url: String,
        max_bytes: usize,
        max_dimension: u32,
        render_size: u32,
        timeout: Duration,
    ) -> Self {
        Self {
            executable,
            render_url,
            max_bytes,
            max_dimension,
            render_size,
            timeout,
            queue: Mutex::new(()),
        }
    }
    fn render(
        &self,
        document: &Value,
        cancellation: Option<&RequestCancellation>,
    ) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        let _guard = loop {
            ensure!(
                !cancellation.is_some_and(RequestCancellation::is_cancelled),
                "preview was cancelled"
            );
            match self.queue.try_lock() {
                Ok(guard) => break guard,
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    anyhow::bail!("preview queue poisoned")
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    thread::sleep(Duration::from_millis(20))
                }
            }
        };
        // Browser launch has Playwright's independent default startup budget. Each
        // subsequent navigation/locator/screenshot operation gets the configured budget.
        let deadline = Instant::now() + Duration::from_secs(30);
        let directory = tempdir()?;
        let executable = &self.executable;
        let child = Command::new(executable)
            .args([
                "--headless",
                "--disable-gpu",
                "--disable-background-timer-throttling",
                "--disable-backgrounding-occluded-windows",
                "--disable-renderer-backgrounding",
                "--no-first-run",
                "--no-default-browser-check",
                "--no-startup-window",
                "--remote-debugging-port=0",
                "--remote-debugging-address=127.0.0.1",
            ])
            .arg(format!("--user-data-dir={}", directory.path().display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let mut browser = BrowserProcess(child);
        let port = loop {
            if let Some(port) = fs::read_to_string(directory.path().join("DevToolsActivePort"))
                .ok()
                .and_then(|text| text.lines().next().and_then(|p| p.parse::<u16>().ok()))
            {
                break port;
            }
            ensure!(
                Instant::now() < deadline
                    && browser.0.try_wait()?.is_none()
                    && !cancellation.is_some_and(RequestCancellation::is_cancelled),
                "browser startup failed or timed out"
            );
            thread::sleep(Duration::from_millis(20));
        };
        let client = reqwest::blocking::Client::builder()
            .timeout(deadline.saturating_duration_since(Instant::now()))
            .no_proxy()
            .build()?;
        let page: Value = client
            .put(format!("http://127.0.0.1:{port}/json/new?about:blank"))
            .send()?
            .error_for_status()?
            .json()?;
        let socket_url = page["webSocketDebuggerUrl"]
            .as_str()
            .context("browser page socket missing")?;
        let (socket, _) = tungstenite::connect(socket_url)?;
        let mut url = url::Url::parse(&self.render_url)?;
        let pairs: Vec<_> = url
            .query_pairs()
            .into_owned()
            .filter(|(key, _)| key != "maxDimension")
            .collect();
        url.query_pairs_mut()
            .clear()
            .extend_pairs(pairs)
            .append_pair("maxDimension", &self.render_size.to_string());
        let mut cdp = Cdp {
            socket,
            next_id: 0,
            deadline,
            origin: url.origin().ascii_serialization(),
            cancellation: cancellation.cloned(),
            loaded_documents: HashSet::new(),
        };
        cdp.call("Page.enable", json!({}))?;
        cdp.call("Page.setLifecycleEventsEnabled", json!({"enabled":true}))?;
        cdp.call("Runtime.enable", json!({}))?;
        cdp.foreground()?;
        cdp.call("Emulation.setDeviceMetricsOverride",json!({"width":self.render_size,"height":self.render_size,"deviceScaleFactor":1,"mobile":false}))?;
        cdp.call(
            "Fetch.enable",
            json!({"patterns":[{"urlPattern":"*","requestStage":"Request"}]}),
        )?;
        let input = serde_json::to_string(document)?;
        cdp.call("Page.addScriptToEvaluateOnNewDocument",json!({"source":format!("Object.defineProperty(window,'__TTS_MERMAID_TEMPLATE_PREVIEW_INPUT__',{{configurable:true,value:{input}}});")}))?;
        cdp.deadline = Instant::now() + self.timeout;
        let navigation = cdp.call("Page.navigate", json!({"url":url.as_str()}))?;
        ensure!(
            navigation.get("errorText").is_none(),
            "preview navigation failed"
        );
        let loader = navigation["loaderId"]
            .as_str()
            .context("preview navigation loader missing")?;
        while !cdp.loaded_documents.contains(loader) {
            cdp.receive("waiting for preview DOMContentLoaded")?;
        }
        cdp.deadline = Instant::now() + self.timeout;
        loop {
            let result=cdp.evaluate("document.querySelector('[data-preview-state]')?.getAttribute('data-preview-state')",false)?;
            if let Some(state) = result.as_str() {
                ensure!(state == "ready", "preview page could not render slide");
                break;
            }
            ensure!(Instant::now() < cdp.deadline, "preview page timed out");
            thread::sleep(Duration::from_millis(20));
        }
        cdp.deadline = Instant::now() + self.timeout;
        cdp.prepare_capture()?;
        cdp.deadline = Instant::now() + self.timeout;
        let bounds = cdp.capture_bounds()?;
        ensure!(
            bounds["width"]
                .as_f64()
                .is_some_and(|w| w > 0.0 && w <= f64::from(self.max_dimension))
                && bounds["height"]
                    .as_f64()
                    .is_some_and(|h| h > 0.0 && h <= f64::from(self.max_dimension)),
            "preview bounds invalid"
        );
        let result = cdp.call(
            "Page.captureScreenshot",
            json!({"format":"png","clip":bounds,"captureBeyondViewport":true}),
        )?;
        let encoded = result["data"].as_str().context("screenshot missing")?;
        ensure!(
            encoded.len() <= self.max_bytes.saturating_mul(4).div_ceil(3) + 4,
            "preview exceeds size limit"
        );
        let bytes = STANDARD.decode(encoded)?;
        let Some((width, height)) = png_dimensions(&bytes) else {
            return Ok(None);
        };
        if bytes.len() > self.max_bytes || width > self.max_dimension || height > self.max_dimension
        {
            return Ok(None);
        }
        Ok(Some(GeneratedTemplatePreview {
            bytes,
            width,
            height,
        }))
    }
}
impl TemplatePreviewGenerator for HeadlessTemplatePreviewGenerator {
    fn requires_document(&self) -> bool {
        true
    }
    fn generate(&self, source: &[u8]) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        let _ = source;
        anyhow::bail!("headless rendering requires a normalized document")
    }
    fn generate_document(
        &self,
        _source: &[u8],
        document: &Value,
    ) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        self.generate_document_cancellable(_source, document, None)
    }
    fn generate_document_cancellable(
        &self,
        _source: &[u8],
        document: &Value,
        cancellation: Option<&RequestCancellation>,
    ) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        match self.render(document, cancellation) {
            Ok(value) => Ok(value),
            Err(error) => {
                let _ = error;
                tracing::warn!("headless preview unavailable");
                Ok(None)
            }
        }
    }
}
struct BrowserProcess(std::process::Child);
impl Drop for BrowserProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Cdp {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    next_id: u64,
    deadline: Instant,
    origin: String,
    cancellation: Option<RequestCancellation>,
    loaded_documents: HashSet<String>,
}
impl Cdp {
    fn foreground(&mut self) -> anyhow::Result<()> {
        // Playwright enables focus emulation for every page. Explicit activation also
        // prevents a different target from pausing the animation frames used below.
        self.call(
            "Emulation.setFocusEmulationEnabled",
            json!({"enabled":true}),
        )?;
        self.call("Page.bringToFront", json!({}))?;
        Ok(())
    }
    fn prepare_capture(&mut self) -> anyhow::Result<()> {
        self.foreground()?;
        self.evaluate("(async()=>{await document.fonts.ready;await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));return true})()",true).context("waiting for preview fonts and animation frames")?;
        Ok(())
    }
    fn capture_bounds(&mut self) -> anyhow::Result<Value> {
        self.evaluate(PREPARE_SCREENSHOT, false)
            .context("preparing preview screenshot")?;
        self.evaluate(WAIT_FOR_SURFACE, true)
            .context("waiting for a visible stable preview surface")
    }
    fn send(&mut self, method: &str, params: Value) -> anyhow::Result<u64> {
        self.next_id += 1;
        let id = self.next_id;
        self.socket.send(Message::Text(
            json!({"id":id,"method":method,"params":params})
                .to_string()
                .into(),
        ))?;
        Ok(id)
    }
    fn call(&mut self, method: &str, params: Value) -> anyhow::Result<Value> {
        let id = self.send(method, params)?;
        loop {
            let event = self.receive(method)?;
            if event["id"].as_u64() == Some(id) {
                ensure!(
                    event.get("error").is_none(),
                    "browser protocol command failed: {method}"
                );
                return Ok(event["result"].clone());
            }
        }
    }
    fn receive(&mut self, operation: &str) -> anyhow::Result<Value> {
        loop {
            ensure!(
                Instant::now() < self.deadline
                    && !self
                        .cancellation
                        .as_ref()
                        .is_some_and(RequestCancellation::is_cancelled),
                "browser operation timed out: {operation}"
            );
            if let MaybeTlsStream::Plain(stream) = self.socket.get_mut() {
                stream.set_read_timeout(Some(
                    self.deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_millis(200)),
                ))?;
                stream.set_write_timeout(Some(
                    self.deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_millis(200)),
                ))?;
            }
            let message = match self.socket.read() {
                Ok(message) => message,
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            let Message::Text(text) = message else {
                continue;
            };
            let event: Value = serde_json::from_str(text.as_str())?;
            if event["method"] == "Page.lifecycleEvent"
                && event["params"]["name"] == "DOMContentLoaded"
            {
                if let Some(loader) = event["params"]["loaderId"].as_str() {
                    self.loaded_documents.insert(loader.to_owned());
                }
            }
            if event["method"] == "Fetch.requestPaused" {
                let address = event["params"]["request"]["url"].as_str().unwrap_or("");
                let allowed = url::Url::parse(address).is_ok_and(|u| {
                    matches!(u.scheme(), "data" | "blob")
                        || u.origin().ascii_serialization() == self.origin
                });
                let request_id = &event["params"]["requestId"];
                if allowed {
                    self.send("Fetch.continueRequest", json!({"requestId":request_id}))?;
                } else {
                    self.send(
                        "Fetch.failRequest",
                        json!({"requestId":request_id,"errorReason":"BlockedByClient"}),
                    )?;
                }
            }
            return Ok(event);
        }
    }
    fn evaluate(&mut self, expression: &str, await_promise: bool) -> anyhow::Result<Value> {
        let result = self.call(
            "Runtime.evaluate",
            json!({"expression":expression,"returnByValue":true,"awaitPromise":await_promise}),
        )?;
        ensure!(
            result.get("exceptionDetails").is_none(),
            "preview script failed"
        );
        Ok(result["result"]["value"].clone())
    }
}

// Screenshot preparation mirrors the reference's relevant element-screenshot
// behavior; the browser is disposed after capture, so no restoration is needed.
const PREPARE_SCREENSHOT: &str = r#"(() => {
  const roots = [document];
  for (let index = 0; index < roots.length; index++) {
    for (const element of roots[index].querySelectorAll('*')) {
      if (element.shadowRoot) roots.push(element.shadowRoot);
    }
  }
  const cancelled = new WeakSet();
  for (const root of roots) {
    for (const element of root.querySelectorAll('input,textarea,[contenteditable]')) {
      element.style.setProperty('caret-color', 'transparent', 'important');
    }
    const finish = () => {
      for (const animation of root.getAnimations()) {
        if (!animation.effect || animation.playbackRate === 0 || cancelled.has(animation)) continue;
        try {
          if (Number.isFinite(animation.effect.getComputedTiming().endTime)) animation.finish();
          else { animation.cancel(); cancelled.add(animation); }
        } catch {}
      }
    };
    finish();
    root.addEventListener('transitionrun', finish);
    root.addEventListener('animationstart', finish);
  }
  return true;
})()"#;

const WAIT_FOR_SURFACE: &str = r#"(async () => {
  await document.fonts.ready;
  const frame = () => new Promise(resolve => requestAnimationFrame(resolve));
  let previous;
  let previousElement;
  for (;;) {
    await frame();
    const matches = document.querySelectorAll('[data-template-preview-surface]');
    if (matches.length > 1) throw new Error('Preview surface must be unique');
    const element = matches[0];
    const rect = element?.getBoundingClientRect();
    const visible = element && rect.width > 0 && rect.height > 0
      && getComputedStyle(element).visibility === 'visible'
      && (!element.checkVisibility || element.checkVisibility());
    if (!visible) { previous = undefined; previousElement = undefined; continue; }
    const values = [rect.x, rect.y, rect.width, rect.height];
    if (previousElement === element && previous && values.every((value, i) => value === previous[i])) {
      element.scrollIntoViewIfNeeded(true);
      const box = element.getBoundingClientRect();
      const x = Math.floor(box.x + scrollX + 0.001);
      const y = Math.floor(box.y + scrollY + 0.001);
      const right = Math.ceil(box.x + scrollX + box.width - 0.001);
      const bottom = Math.ceil(box.y + scrollY + box.height - 0.001);
      return {x, y, width: right - x, height: bottom - y, scale: 1};
    }
    previous = values;
    previousElement = element;
  }
})()"#;

#[cfg(test)]
#[path = "../../../../tests/unit/adapters/slide_templates/preview/headless_tests.rs"]
mod tests;
