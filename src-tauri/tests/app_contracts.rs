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
fn preferences_exposes_rewrite_provider_fields() {
    let html = read_repo_file("src-ui/preferences.html");
    let settings = read_repo_file("src-tauri/src/commands/settings.rs");
    let state = read_repo_file("src-tauri/src/state.rs");
    let transcription = read_repo_file("src-tauri/src/commands/transcription.rs");

    for field in [
        "rewrite-api-key",
        "rewrite-model",
        "rewrite-api-url",
        "rewrite-timeout-ms",
    ] {
        assert!(
            html.contains(field),
            "preferences UI must expose persisted rewrite provider field {field}"
        );
    }

    for property in [
        "rewrite_api_key",
        "rewrite_model",
        "rewrite_api_url",
        "rewrite_timeout_ms",
    ] {
        assert!(
            settings.contains(property),
            "settings command must map {property}"
        );
    }

    assert!(
        state.contains("CloudRewriteSettings"),
        "settings must persist rewrite provider config"
    );
    assert!(
        transcription.contains("cloud_rewrite_config(&rewrite_settings)"),
        "rewrite pipeline must read persisted provider config before env fallback"
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
fn mac_bundle_identity_and_signing_are_stable() {
    let tauri_config: Value =
        serde_json::from_str(&read_repo_file("src-tauri/tauri.conf.json")).unwrap();

    assert_eq!(
        tauri_config.get("identifier").and_then(Value::as_str),
        Some("app.zana"),
        "bundle identifier must stay app.zana so macOS permissions remain stable"
    );
    assert_eq!(
        tauri_config.pointer("/bundle/macOS/signingIdentity"),
        Some(&Value::String("-".to_string())),
        "macOS app bundles must be ad-hoc signed so LaunchServices sees a sealed .app"
    );

    let plist = read_repo_file("src-tauri/Info.plist");
    assert!(
        plist.contains("<key>LSRequiresCarbon</key>") && plist.contains("<false/>"),
        "Zana must not advertise Carbon-only launch requirements"
    );
    assert!(
        plist.contains("<key>CFBundleDisplayName</key>")
            && plist.contains("<string>qVoice</string>"),
        "visible app display name should be qVoice while bundle identity stays stable"
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
        html.contains("id=\"drag-region\"") && html.contains("startDragging"),
        "orb markup must expose an explicit drag target that calls Tauri startDragging"
    );
}

#[test]
fn orb_window_has_start_dragging_capability() {
    let tauri_config: Value =
        serde_json::from_str(&read_repo_file("src-tauri/tauri.conf.json")).unwrap();
    let capabilities = tauri_config
        .pointer("/app/security/capabilities")
        .and_then(Value::as_array)
        .expect("tauri config must declare capabilities");

    let can_start_dragging = capabilities.iter().any(|capability| {
        let includes_orb = capability
            .get("windows")
            .and_then(Value::as_array)
            .is_some_and(|windows| windows.iter().any(|window| window == "orb"));

        let allows_dragging = capability
            .get("permissions")
            .and_then(Value::as_array)
            .is_some_and(|permissions| {
                permissions
                    .iter()
                    .any(|permission| permission == "core:window:allow-start-dragging")
            });

        includes_orb && allows_dragging
    });

    assert!(
        can_start_dragging,
        "orb window must have Tauri startDragging permission"
    );
}
