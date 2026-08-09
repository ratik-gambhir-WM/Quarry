# ADR 0002: Desktop security, errors, secrets, and diagnostics

Status: Accepted

The webview receives stable, structured errors containing a code, safe message, retryability, and operation ID. Raw internal service, database, and filesystem errors remain in Rust and are not serialized. API keys are Rust-owned; any account DTO masks the key before serialization.

The production Tauri window uses a restrictive CSP and only the capabilities required for native open/save dialogs. Frontend-triggered filesystem events are not an authorization mechanism. Native selections create in-memory canonical path grants; later commands re-canonicalize paths, check the grant or granted-root containment, and enforce file and aggregate size limits. Summary and activity-log exports choose their destination in Rust and use bounded atomic writes.

Session activity logs contain command/event names, duration, status, and recursively redacted/truncated shapes. Keys, credentials, email addresses, document text, file bodies, and absolute paths are not retained. A logging failure must never change the observed product operation.
