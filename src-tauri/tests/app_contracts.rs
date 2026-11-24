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

    // Preferences > Model: the provider fields are Voicebot's chat model now.
    for field in ["rewrite-api-key", "rewrite-model", "rewrite-api-url"] {
        assert!(
            html.contains(field),
            "preferences UI must expose persisted model provider field {field}"
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
fn transcriptions_live_in_their_own_window() {
    let prefs = read_repo_file("src-ui/preferences.html");
    let window = read_repo_file("src-ui/transcriptions.html");
    let settings = read_repo_file("src-tauri/src/commands/settings.rs");
    let main = read_repo_file("src-tauri/src/main.rs");

    // Zana is the Voicebot app: Preferences no longer shows dictation history.
    assert!(
        !prefs.contains("transcript-history-list"),
        "the transcript history list must not be rendered inline in preferences"
    );

    // The transcript history list itself lives in the dedicated window.
    for marker in ["transcript-history-list", "get_transcript_history"] {
        assert!(
            window.contains(marker),
            "transcriptions window must render local history via marker {marker}"
        );
    }

    assert!(
        settings.contains("pub async fn get_transcript_history"),
        "settings commands must expose transcript history to the transcriptions window"
    );
    assert!(
        settings.contains("pub async fn open_transcriptions_window"),
        "settings commands must expose a command to open the transcriptions window"
    );
    for handler in [
        "commands::get_transcript_history",
        "commands::open_transcriptions_window",
    ] {
        assert!(
            main.contains(handler),
            "Tauri command handler must register {handler}"
        );
    }
}

#[test]
fn transcripts_persist_in_local_only_mode() {
    let src = read_repo_file("src-tauri/src/commands/transcription.rs");

    // Regression guard: transcript history must be recorded for every dictation,
    // not only when cloud rewrite is enabled. Local-only dictation used to hit the
    // `else` branch and silently drop the transcript.
    assert!(
        src.contains("Persist every transcription locally"),
        "transcribe flow must record history regardless of AI-polish mode"
    );

    let else_branch = src
        .find("state.clear_recording_context().await;")
        .expect("local-only branch must clear recording context");
    let save_call = src
        .find("record_transcript_history(")
        .expect("transcribe flow must call record_transcript_history");
    assert!(
        save_call > else_branch,
        "record_transcript_history must run after the rewrite if/else so local-only dictation is saved too"
    );
}

#[test]
fn rewrite_pipeline_sends_only_its_visible_messages() {
    let transcription = read_repo_file("src-tauri/src/commands/transcription.rs");
    let state = read_repo_file("src-tauri/src/state.rs");

    assert!(
        !transcription.contains("Technical contract:"),
        "rewrite pipeline must not append hidden system instructions outside the visible template"
    );
    assert!(
        !transcription.contains("Response contract:\\n"),
        "rewrite pipeline must not append a hidden response contract outside the visible template"
    );
    assert!(
        state.contains("Submit exactly one submit_result tool call"),
        "default system prompt should include the tool-call instruction users can edit"
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
fn mac_bundle_description_matches_tauri_metadata() {
    let config: Value = serde_json::from_str(&read_repo_file("src-tauri/tauri.conf.json")).unwrap();
    let description = config
        .pointer("/bundle/longDescription")
        .and_then(Value::as_str)
        .unwrap();
    let plist = read_repo_file("src-tauri/Info.plist");
    assert!(
        plist.contains(&format!(
            "<key>app.zana.Description</key>\n    <string>{description}</string>"
        )),
        "macOS bundle description must match the Tauri long description"
    );
}

#[test]
fn bundle_descriptions_describe_voicebot_without_device_only_claims() {
    let config: Value = serde_json::from_str(&read_repo_file("src-tauri/tauri.conf.json")).unwrap();
    let short_description = config
        .pointer("/bundle/shortDescription")
        .and_then(Value::as_str)
        .unwrap()
        .to_lowercase();
    let long_description = config
        .pointer("/bundle/longDescription")
        .and_then(Value::as_str)
        .unwrap()
        .to_lowercase();

    assert!(short_description.contains("voice agent"));
    assert!(long_description.contains("voice agent"));
    for description in [&short_description, &long_description] {
        assert!(!description.contains("whisper"));
        assert!(!description.contains("never leaves your device"));
    }
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
        tauri_config.get("productName").and_then(Value::as_str),
        Some("Zana"),
        "generated macOS bundle should be Zana.app while identifier stays app.zana"
    );

    let plist = read_repo_file("src-tauri/Info.plist");
    assert!(
        plist.contains("<key>LSRequiresCarbon</key>") && plist.contains("<false/>"),
        "Zana must not advertise Carbon-only launch requirements"
    );
    assert!(
        plist.contains("<key>CFBundleDisplayName</key>") && plist.contains("<string>Zana</string>"),
        "visible app display name should be Zana while bundle identity stays stable"
    );
}

#[test]
fn mac_release_requires_stable_developer_id_signature() {
    let tauri_config: Value =
        serde_json::from_str(&read_repo_file("src-tauri/tauri.conf.json")).unwrap();
    let signing_identity = tauri_config
        .pointer("/bundle/macOS/signingIdentity")
        .and_then(Value::as_str);
    let sign_script = read_repo_file("scripts/sign-and-notarize.sh");
    let release_script = read_repo_file("scripts/release-macos.sh");

    assert_ne!(
        signing_identity,
        Some("-"),
        "tauri.conf.json must not default macOS builds to ad-hoc signing"
    );
    assert!(
        sign_script.contains("APPLE_SIGNING_IDENTITY")
            && sign_script.contains("codesign -dv --verbose=4")
            && sign_script.contains("spctl --assess"),
        "macOS signing must use a stable Developer ID identity and verify install identity"
    );
    assert!(
        release_script.contains("scripts/sign-and-notarize.sh")
            && release_script.contains("APPLE_SIGNING_IDENTITY"),
        "local macOS release builds must run signing/notarization as part of the release flow"
    );
}

#[test]
fn cut_release_is_tag_driven_and_guarded() {
    let workflow = read_repo_file(".github/workflows/release.yml");
    let script = read_repo_file("scripts/cut-release.sh");

    assert!(
        workflow.contains("workflow_dispatch:")
            && workflow.contains("refs/heads/main")
            && workflow.contains("refs/tags/$TAG")
            && workflow.contains("missing secrets"),
        "GitHub release workflow must run from main for an existing tag and fail on missing signing secrets"
    );
    assert!(
        script.contains("gh workflow run release.yml"),
        "cut-release script must dispatch the release workflow after pushing the tag"
    );
    assert!(
        script.contains("git diff --quiet")
            && script.contains("git tag -a")
            && script.contains("git push origin"),
        "cut-release script must require a clean tree, create an annotated tag, and push it"
    );
}

#[test]
fn public_release_metadata_is_consistent() {
    let root_cargo = read_repo_file("Cargo.toml");
    let tauri_config: Value =
        serde_json::from_str(&read_repo_file("src-tauri/tauri.conf.json")).unwrap();
    let readme = read_repo_file("README.md");

    assert!(
        root_cargo.contains("repository = \"https://github.com/maarco/zana\""),
        "public repository metadata must point at the intended GitHub repo"
    );
    let workspace_version = root_cargo
        .lines()
        .find_map(|line| line.strip_prefix("version = "))
        .and_then(|value| value.trim().trim_matches('"').split_whitespace().next());
    assert_eq!(
        tauri_config.get("version").and_then(Value::as_str),
        workspace_version,
        "Tauri bundle version must stay aligned with the workspace version before public release"
    );
    assert!(
        readme.contains("[CONTRIBUTING.md](CONTRIBUTING.md)")
            && !readme.contains("docs/demo.gif")
            && !readme.contains("Coming Soon"),
        "README must link existing public docs and avoid broken demo/coming-soon release copy"
    );
}

#[test]
fn preferences_exposes_update_check_banner() {
    let html = read_repo_file("src-ui/preferences.html");
    let main_rs = read_repo_file("src-tauri/src/main.rs");
    let update_rs = read_repo_file("src-tauri/src/commands/update.rs");

    assert!(
        html.contains("id=\"update-banner\"") && html.contains("checkForUpdates"),
        "preferences top bar must show an update affordance when a release is available"
    );
    assert!(
        main_rs.contains("commands::check_for_updates"),
        "update check command must be registered with Tauri"
    );
    assert!(
        update_rs.contains("https://api.github.com/repos/{owner}/{repo}/releases/latest"),
        "update checks must read the latest GitHub release"
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

#[test]
fn orb_runtime_parks_animation_when_hidden() {
    let html = read_repo_file("src-ui/orb.html");
    let runtime = read_repo_file("src-ui/orb-runtime.js");

    assert!(
        html.contains("orb-runtime.js")
            && html.find("orb-runtime.js") < html.find("orb-lifecycle.js"),
        "orb runtime must load before lifecycle and legacy renderers patch requestAnimationFrame"
    );
    assert!(
        runtime.contains("window.requestAnimationFrame")
            && runtime.contains("pausedFrames")
            && runtime.contains("sleepSoon")
            && runtime.contains("installControlBridge"),
        "orb runtime must park animation frames while the hidden panel is inactive"
    );
}

#[test]
fn mac_and_windows_hotkey_start_failures_clear_recording_state() {
    let source = read_repo_file("src-tauri/src/main.rs");

    assert!(
        source.contains("fn handle_recording_start_failed"),
        "hotkey recording paths need a shared rollback helper for failed audio starts"
    );
    assert!(
        source.matches("handle_recording_start_failed(&app_clone").count() >= 2,
        "both macOS Fn and Windows Ctrl start paths must clear native recording state on start failure"
    );
}

#[test]
fn short_hotkey_release_cancels_pending_capture() {
    let source = read_repo_file("src-tauri/src/main.rs");

    assert!(
        source.contains("cancel_recording_without_transcription(app)"),
        "short hotkey release must cancel pending capture instead of only hiding the orb"
    );
    assert!(
        source.contains("Recording start skipped because the trigger was released"),
        "delayed capture start must re-check the native recording flag before opening the microphone"
    );
    assert!(
        source
            .matches("Recording start completed after the trigger was released")
            .count()
            >= 2,
        "both hotkey paths must cancel capture if release races with async microphone start"
    );
}

#[test]
fn failed_stop_does_not_transcribe_stale_audio() {
    let source = read_repo_file("src-tauri/src/main.rs");

    assert!(
        source.matches("return;").count() >= 2 && source.contains("Failed to stop recording"),
        "hotkey stop paths must return before transcribe when stop_recording_inner fails"
    );
}

#[test]
fn tauri_file_logging_is_registered_for_crash_forensics() {
    let source = read_repo_file("src-tauri/src/main.rs");

    assert!(
        source.contains("tauri_plugin_log"),
        "main.rs must register the log plugin for runtime crash forensics"
    );
}

#[test]
fn tauri_log_plugin_config_is_owned_by_rust_builder() {
    let tauri_config: Value =
        serde_json::from_str(&read_repo_file("src-tauri/tauri.conf.json")).unwrap();

    assert!(
        tauri_config.pointer("/plugins/log").is_none(),
        "tauri-plugin-log v2 expects no plugins.log config object when targets are configured in Rust"
    );
}

#[test]
fn app_registers_single_instance_guard() {
    let cargo = read_repo_file("src-tauri/Cargo.toml");
    let main = read_repo_file("src-tauri/src/main.rs");

    assert!(
        cargo.contains("tauri-plugin-single-instance"),
        "Cargo.toml must include the single-instance plugin so duplicate launches cannot stack resident processes"
    );
    assert!(
        main.contains("tauri_plugin_single_instance::init"),
        "main.rs must register the single-instance plugin during Tauri builder setup"
    );
}

#[test]
fn whisper_full_runs_on_blocking_thread() {
    let source = read_repo_file("src-tauri/src/stt/whisper.rs");

    assert!(
        source.contains("tokio::task::spawn_blocking"),
        "Whisper full() must run on a blocking thread, not inline on an async runtime worker"
    );
    assert!(
        source.contains(".full(params, &samples)") || source.contains(".full(params, samples"),
        "the blocking task must own or borrow audio samples inside the blocking closure"
    );
}

#[test]
fn voicebot_preferences_commands_and_bundle_stay_wired() {
    let preferences = read_repo_file("src-ui/preferences.html");
    let orb = read_repo_file("src-ui/voicebot.html");
    let main_rs = read_repo_file("src-tauri/src/main.rs");
    let config = read_repo_file("src-tauri/tauri.conf.json");
    let supervisor = read_repo_file("src-tauri/src/voicebot.rs");

    for id in [
        "voicebot-orb-design",
        "voicebot-fillers-toggle",
        "voicebot-filler-phrases",
        "voicebot-filler-days",
        "voicebot-voice",
        "model-reasoning",
        "vision-model",
        "voicebot-terminal-toggle",
        "voicebot-context-command",
        "voicebot-wake-word",
        "voicebot-prompt",
        "voicebot-mcp",
        "voicebot-save",
        "voicebot-debug",
        "voicebot-voiceprint-clear",
    ] {
        assert!(
            preferences.contains(id),
            "Preferences must keep the Voicebot control {id}"
        );
    }
    for command in [
        "commands::get_voicebot_config",
        "commands::set_voicebot_voice",
        "commands::set_voicebot_fillers",
        "commands::set_voicebot_setting",
        "commands::voicebot_self_settings",
        "commands::set_voicebot_terminal",
        "commands::save_voicebot_prompt",
        "commands::save_voicebot_mcp",
        "commands::voicebot_join",
        "commands::voicebot_screen_context",
        "commands::set_voicebot_wake_word",
        "commands::voicebot_debug_tail",
        "open_voicebot_debug",
        "commands::voicebot_screenshot",
        "voicebot_paste",
    ] {
        assert!(main_rs.contains(command), "main.rs must register {command}");
    }
    assert!(
        orb.contains("orb-designs.js")
            && read_repo_file("src-ui/orb-designs.js").contains("window.ZanaOrbDesigns"),
        "the orb must load its designs"
    );
    for rpc in [
        "zana.paste",
        "zana.screen",
        "voicebot_join",
        "zanaDictation",
    ] {
        assert!(orb.contains(rpc), "the Voicebot orb must handle {rpc}");
    }
    // Zana is the Voicebot app: no on/off switch, and Preferences > Model reaches the agent.
    assert!(
        !preferences.contains("voicebot-mode-toggle") && !main_rs.contains("set_voicebot_mode"),
        "Voicebot is always on; there is no Voicebot Mode switch"
    );
    assert!(
        preferences.contains("data-section=\"model\"") && supervisor.contains("\"llm\""),
        "Preferences > Model must be written into the agent's config"
    );
    // Every file the supervisor needs ships in the bundle.
    for file in [
        "agent.py",
        "voice_server.py",
        "port_reporting.py",
        "speech.py",
        "listening.py",
        "jobs.py",
        "retention.py",
        "fillers.py",
        "orb_state.py",
        "voiceprint.py",
        "updater.py",
        "system_prompt.default.md",
        "requirements.txt",
        "requirements-nodeps.txt",
    ] {
        assert!(
            config.contains(&format!("\"../voicebot/{file}\": \"voicebot/{file}\"")),
            "tauri.conf.json must bundle voicebot/{file}"
        );
        assert!(
            supervisor.contains(&format!("\"{file}\"")),
            "voicebot.rs CODE_FILES must list {file}"
        );
        assert!(
            repo_file(&format!("voicebot/{file}")).exists(),
            "voicebot/{file} must exist"
        );
    }
    assert!(
        main_rs.contains("Zana_app::voicebot::stop()"),
        "Zana must stop Voicebot's processes when it exits"
    );
}

#[test]
fn speech_server_reports_its_os_assigned_port_before_agent_startup() {
    let supervisor = read_repo_file("src-tauri/src/voicebot.rs");
    let server = read_repo_file("voicebot/voice_server.py");
    assert!(
        supervisor.contains(".env(\"VOICE_SERVER_PORT\", \"0\")")
            && supervisor.contains("VOICE_SERVER_PORT_FILE")
            && server.contains("report_bound_port(self, path)"),
        "the app must use a port chosen by the listening speech server, not a stale probe"
    );
}

#[test]
fn voicebot_orb_keeps_the_five_state_frontend_contract_wired() {
    let voicebot = include_str!("../../src-ui/voicebot.html");
    let orb_state = include_str!("../../src-ui/voicebot-orb-state.js");
    let aura = include_str!("../../src-ui/vendor/aura.js");

    assert!(voicebot.contains("<script src=\"voicebot-orb-state.js\"></script>"));
    assert!(voicebot.contains("'zana.orb.state'"));
    assert!(voicebot.contains("'lk.agent.state'")); // Legacy agents still work.
    assert!(voicebot.contains("ParticipantAttributesChanged"));
    assert!(voicebot.contains("ParticipantDisconnected"));
    for state in ["idle", "listening", "thinking", "talking", "working"] {
        assert!(
            orb_state.contains(&format!("{state}:")),
            "missing {state} style"
        );
    }
    assert!(
        aura.contains("working: {"),
        "working needs its own animation"
    );
}
