//! Guards ADR-011 rule 3: strict CSP and minimal per-window capabilities.
//! `style-src 'unsafe-inline'` is required because React emits inline style attributes.

use serde_json::Value;

const CSP: &str = "default-src 'self'; connect-src ipc: http://ipc.localhost; img-src 'self' data:; script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self'";

fn load(path: &str) -> Value {
    let full = format!("{}/{path}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&full).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn perms(name: &str) -> Vec<String> {
    load(&format!("capabilities/{name}.json"))["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn csp_is_exact() {
    let conf = load("tauri.conf.json");
    assert_eq!(conf["app"]["security"]["csp"].as_str(), Some(CSP));
}

#[test]
fn no_forbidden_permissions() {
    for name in ["default", "settings"] {
        for p in perms(name) {
            for bad in [
                "shell:",
                "fs:",
                "http:",
                "opener:",
                "global-shortcut:",
                "autostart:",
                "clipboard",
                "core:default",
            ] {
                assert!(!p.contains(bad), "{name} has {p}");
            }
        }
    }
}

#[test]
fn launcher_lacks_settings_commands() {
    let p = perms("default");
    for c in [
        "set-secret",
        "delete-secret",
        "list-secrets",
        "set-provider",
    ] {
        assert!(!p.contains(&format!("allow-{c}")), "launcher has {c}");
    }
}

#[test]
fn settings_lacks_launcher_commands() {
    let p = perms("settings");
    for c in [
        "launch",
        "search",
        "ask-stream",
        "web-search",
        "tray-action",
        "home-items",
    ] {
        assert!(!p.contains(&format!("allow-{c}")), "settings has {c}");
    }
}
