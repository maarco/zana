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
fn onboarding_is_voicebot_setup_without_retired_dictation() {
    let onboarding = read_repo_file("src-ui/onboarding.html");
    let command_module = read_repo_file("src-tauri/src/commands/onboarding.rs");
    let main = read_repo_file("src-tauri/src/main.rs");

    assert!(onboarding.contains("chat model") && onboarding.contains("API key"));
    assert!(onboarding.contains("wake word"));
    assert!(onboarding.contains("Accessibility lets Voicebot type or paste"));
    assert!(onboarding.contains("openMicrophoneSettings"));
    assert!(onboarding.contains("openVoicebotPreferences"));
    assert!(onboarding.contains("complete_onboarding_and_restart"));
    assert!(!onboarding.contains("download_whisper_model"));
    assert!(!onboarding.contains("Whisper AI"));
    assert!(!onboarding.contains("Fn key"));
    assert!(!command_module.contains("download_whisper_model"));
    assert!(command_module.contains("open_microphone_settings"));
    assert!(command_module.contains("open_preferences_window"));
    assert!(main.contains("commands::complete_onboarding_and_restart"));
    assert!(!main.contains("commands::download_whisper_model"));
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
fn voicebot_debug_clear_opens_its_lock_without_truncating_it() {
    let source = read_repo_file("src-tauri/src/commands/voicebot.rs");
    let clear = source
        .split_once("pub async fn clear_voicebot_debug()")
        .expect("debug clear command must exist")
        .1
        .split_once("if path.exists()")
        .expect("debug clear command must preserve its log clearing step")
        .0;

    assert!(
        clear.contains(".create(true)")
            && clear.contains(".read(true)")
            && clear.contains(".write(true)")
            && clear.contains(".truncate(false)")
            && clear.contains(".open(lock_path)"),
        "debug clear must create/open the lock without truncating it"
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
fn retired_dictation_stack_is_absent_from_live_app() {
    let root_cargo = read_repo_file("Cargo.toml");
    let app_cargo = read_repo_file("src-tauri/Cargo.toml");
    let commands = read_repo_file("src-tauri/src/commands/mod.rs");
    let lib = read_repo_file("src-tauri/src/lib.rs");
    let main = read_repo_file("src-tauri/src/main.rs");
    let settings = read_repo_file("src-tauri/src/commands/settings.rs");
    let diagnostics = read_repo_file("src-tauri/src/commands/diagnostics.rs");
    let preferences = read_repo_file("src-ui/preferences.html");
    let config = read_repo_file("src-tauri/tauri.conf.json");

    assert_eq!(
        root_cargo.matches("members = [\"src-tauri\"]").count(),
        1,
        "the workspace must contain only the live Tauri crate"
    );
    for dependency in [
        "cpal",
        "hound",
        "whisper-rs",
        "audiopus",
        "symphonia",
        "webm-iterable",
        "wasmtime",
    ] {
        assert!(
            !root_cargo.contains(dependency) && !app_cargo.contains(dependency),
            "retired dependency remains declared: {dependency}"
        );
    }
    for marker in [
        "mod audio",
        "mod plugins",
        "mod transcription",
        "start_recording",
        "stop_recording",
        "list_models",
        "voicebot_dictation",
        "get_transcript_history",
        "open_transcriptions_window",
        "set_orb_style",
        "get_orb_style",
    ] {
        assert!(
            !commands.contains(marker)
                && !main.contains(marker)
                && !settings.contains(marker)
                && !lib.contains(marker),
            "retired command or module remains reachable: {marker}"
        );
    }
    assert!(!diagnostics.contains("AudioCapture"));
    for marker in ["Show in Menu Bar", "list_audio_devices", "Whisper Model"] {
        assert!(
            !preferences.contains(marker),
            "retired Preferences control remains visible: {marker}"
        );
    }
    assert!(
        !config.contains("\"transcriptions\"")
            && !config.contains("\"main\"")
            && !config.contains("\"orb\""),
        "retired windows remain in Tauri capabilities"
    );
    for path in [
        "src",
        "tests",
        "plugins",
        "src-ui/index.html",
        "src-ui/orb.html",
        "src-ui/transcriptions.html",
    ] {
        assert!(
            !repo_file(path).exists(),
            "retired source or UI path still exists: {path}"
        );
    }
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
    for rpc in ["zana.paste", "zana.screen", "voicebot_join"] {
        assert!(orb.contains(rpc), "the Voicebot orb must handle {rpc}");
    }
    assert!(
        !orb.contains("zanaDictation") && !orb.contains("dictating"),
        "the Voicebot orb must not retain the retired dictation bridge"
    );
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
    assert_eq!(
        Zana_app::voicebot::CODE_FILES.len(),
        16,
        "the Voicebot bundle file list must include all runtime files"
    );
    for file in [
        "agent.py",
        "prompt_guidance.py",
        "tool_guidance.md",
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
