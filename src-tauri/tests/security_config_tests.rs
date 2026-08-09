use std::{fs, path::PathBuf};

fn read_json(relative_path: &str) -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let contents = fs::read_to_string(path).expect("configuration file should be readable");
    serde_json::from_str(&contents).expect("configuration should be valid JSON")
}

#[test]
fn tauri_uses_a_restrictive_csp_and_safe_window_defaults() {
    let config = read_json("tauri.conf.json");
    let security = &config["app"]["security"];
    let csp = security["csp"]
        .as_str()
        .expect("CSP must be enabled as a string");

    assert!(csp.contains("default-src 'self'"));
    assert!(csp.contains("object-src 'none'"));
    assert!(csp.contains("frame-ancestors 'none'"));
    assert_eq!(security["freezePrototype"], true);

    let window = &config["app"]["windows"][0];
    assert!(window["width"].as_u64().unwrap_or_default() >= 1_200);
    assert!(window["minWidth"].as_u64().unwrap_or_default() >= 1_024);
    assert!(window["minHeight"].as_u64().unwrap_or_default() >= 700);
}

#[test]
fn main_capability_does_not_grant_default_opener_access() {
    let capability = read_json("capabilities/default.json");
    let permissions = capability["permissions"]
        .as_array()
        .expect("permissions should be an array");

    assert!(!permissions.iter().any(|permission| {
        permission
            .as_str()
            .is_some_and(|identifier| identifier.starts_with("opener:"))
    }));
}
