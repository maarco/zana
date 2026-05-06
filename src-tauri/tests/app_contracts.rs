use serde_json::Value;
use std::{fs, path::PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_file(path: &str) -> PathBuf {
    manifest_dir().join("..").join(path)
}

fn read_repo_file(path: &str) -> String {
    let path = repo_file(path);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("failed to read {path:?}: {err}"))
}

#[test]
fn preferences_window_has_tauri_capability() {
    let tauri_config: Value =
        serde_json::from_str(&read_repo_file("src-tauri/tauri.conf.json")).unwrap();
    let mut windows = Vec::new();

    if let Some(capabilities) = tauri_config
        .pointer("/app/security/capabilities")
        .and_then(Value::as_array)
    {
        for capability in capabilities {
            if let Some(capability_windows) = capability.get("windows").and_then(Value::as_array) {
                windows.extend(capability_windows.iter().filter_map(Value::as_str));
            }
        }
    }

    let default_capability: Value =
        serde_json::from_str(&read_repo_file("src-tauri/capabilities/default.json")).unwrap();
    if let Some(default_windows) = default_capability.get("windows").and_then(Value::as_array) {
        windows.extend(default_windows.iter().filter_map(Value::as_str));
    }

    assert!(
        windows.contains(&"preferences"),
        "preferences window must be listed in Tauri capabilities so its save commands can run"
    );
}

#[test]
fn secondary_windows_have_close_capabilities() {
    let tauri_config: Value =
        serde_json::from_str(&read_repo_file("src-tauri/tauri.conf.json")).unwrap();
    let capabilities = tauri_config
        .pointer("/app/security/capabilities")
        .and_then(Value::as_array)
        .expect("tauri config must declare capabilities");

    for window_name in ["preferences", "about", "onboarding"] {
        let can_close = capabilities.iter().any(|capability| {
            let includes_window = capability
                .get("windows")
                .and_then(Value::as_array)
                .is_some_and(|windows| windows.iter().any(|window| window == window_name));

            let allows_close = capability
                .get("permissions")
                .and_then(Value::as_array)
                .is_some_and(|permissions| {
                    permissions
                        .iter()
                        .any(|permission| permission == "core:window:allow-close")
                });

            includes_window && allows_close
        });

        assert!(
            can_close,
            "{window_name} must have Tauri close permission for Save/Cancel/close buttons"
        );
    }
}

#[test]
fn preferences_save_uses_backend_save_command() {
    let html = read_repo_file("src-ui/preferences.html");

    assert!(
        html.contains("invoke('save_preferences'")
            || html.contains("invoke(\"save_preferences\""),
        "Save must call a single backend save_preferences command instead of closing after only saving orb style"
    );
}

#[test]
fn set_model_persists_selected_model() {
    let source = read_repo_file("src-tauri/src/commands/transcription.rs");
    let start = source.find("pub async fn set_model").unwrap();
    let end = source[start..]
        .find("/// Transcribe")
        .map(|offset| start + offset)
        .unwrap_or(source.len());
    let body = &source[start..end];

    assert!(
        body.contains("save_settings().await"),
        "set_model must persist the selected model instead of only mutating in-memory settings"
    );
}

#[test]
fn mac_orb_panel_uses_compact_geometry() {
    let source = read_repo_file("src-tauri/src/main.rs");
    let start = source.find("fn create_orb_window").unwrap();
    let end = source[start..]
        .find("/// Show the orb window")
        .map(|offset| start + offset)
        .unwrap_or(source.len());
    let body = &source[start..end];

    assert!(
        !body.contains("screen_width / 2.0") && !body.contains("screen_height / 2.0"),
        "macOS orb panel should use compact visual geometry, not a half-screen transparent hitbox"
    );
}

#[test]
fn orb_markup_exposes_explicit_drag_target() {
    let html = read_repo_file("src-ui/orb.html");

    assert!(
        html.contains("data-drag-region")
            || html.contains("id=\"drag-region\"")
            || html.contains("startDragging")
            || html.contains("start_dragging"),
        "orb markup must expose an explicit drag target for moving the overlay"
    );
}
