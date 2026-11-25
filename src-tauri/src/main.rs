//! Zana main entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(deprecated)]
#![allow(unexpected_cfgs)]

#[cfg(target_os = "macos")]
use std::sync::Mutex as StdMutex;
use tauri::menu::{
    Menu, MenuBuilder, MenuItem, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder,
};
use tauri::Listener;
use tauri::Manager;
use Zana_app::state::AppState;

#[cfg(target_os = "macos")]
mod panel;

#[cfg(target_os = "macos")]
use tauri_nspanel::WebviewWindowExt;

mod commands {
    pub use Zana_app::commands::*;
}

fn create_app_menu(app: &tauri::AppHandle) -> Result<Menu<tauri::Wry>, tauri::Error> {
    let app_menu = SubmenuBuilder::new(app, "Zana")
        .item(&PredefinedMenuItem::about(app, Some("About Zana"), None)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("preferences", "Preferences...")
                .accelerator("CmdOrCtrl+,")
                .build(app)?,
        )
        .separator()
        .item(&PredefinedMenuItem::services(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::hide(app, Some("Hide Zana"))?)
        .item(&PredefinedMenuItem::hide_others(app, Some("Hide Others"))?)
        .item(&PredefinedMenuItem::show_all(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::quit(app, Some("Quit Zana"))?)
        .build()?;

    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .item(&PredefinedMenuItem::undo(app, None)?)
        .item(&PredefinedMenuItem::redo(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, None)?)
        .item(&PredefinedMenuItem::copy(app, None)?)
        .item(&PredefinedMenuItem::paste(app, None)?)
        .item(&PredefinedMenuItem::select_all(app, None)?)
        .build()?;

    let window_menu = SubmenuBuilder::new(app, "Window")
        .item(&PredefinedMenuItem::minimize(app, None)?)
        .item(&PredefinedMenuItem::maximize(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::close_window(app, None)?)
        .build()?;

    let help_menu = SubmenuBuilder::new(app, "Help")
        .item(
            &MenuItemBuilder::with_id("show_help", "Zana Help")
                .accelerator("CmdOrCtrl+?")
                .build(app)?,
        )
        .build()?;

    MenuBuilder::new(app)
        .item(&app_menu)
        .item(&edit_menu)
        .item(&window_menu)
        .item(&help_menu)
        .build()
}

fn main() {
    std::panic::set_hook(Box::new(|panic_info| {
        let crash_log = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("Zana")
            .join("crash.log");

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let message = format!(
            "\n=== CRASH REPORT ===\nTime: {}\nVersion: {}\n{}\n",
            timestamp,
            env!("CARGO_PKG_VERSION"),
            panic_info
        );

        if let Some(parent) = crash_log.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_log)
            .and_then(|mut file| std::io::Write::write_all(&mut file, message.as_bytes()));

        eprintln!("{message}");
        eprintln!("Crash log written to: {crash_log:?}");
    }));

    let mut builder = tauri::Builder::default();

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            log::info!("[SingleInstance] Duplicate launch blocked");

            if let Some(window) = app.get_webview_window("preferences") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("zana".into()),
                    }),
                ])
                .build(),
        )
        // Auto-updater disabled until signing keys are configured.
        // .plugin(tauri_plugin_updater::Builder::new().build())
        .menu(create_app_menu)
        .setup(|app| {
            let is_first_run = Zana_app::onboarding::is_first_run();

            log::info!("Initializing application state...");
            let state = AppState::new().expect("Failed to initialize app state");
            app.manage(state);

            let settings = Zana_app::state::Settings::load();
            start_voicebot(app.handle(), &settings);

            if is_first_run {
                log::info!("First run detected - showing onboarding window");

                use tauri::{WebviewUrl, WebviewWindowBuilder};

                let onboarding_window = WebviewWindowBuilder::new(
                    app,
                    "onboarding",
                    WebviewUrl::App("onboarding.html".into()),
                )
                .title("Zana Genesis")
                .inner_size(900.0, 700.0)
                .center()
                .resizable(false)
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .build()
                .expect("Failed to create onboarding window");

                #[cfg(debug_assertions)]
                onboarding_window.open_devtools();

                return Ok(());
            }

            let app_handle = app.handle().clone();
            app.listen("voicebot-config-changed", move |_| {
                if let Some(window) = app_handle.get_webview_window("voicebot") {
                    let _ =
                        window.eval("typeof newConversation === 'function' && newConversation()");
                }
            });

            let quit = MenuItem::with_id(app, "quit", "Quit Zana", true, Some("CmdOrCtrl+Q"))?;
            let about = MenuItem::with_id(app, "about", "About Zana", true, None::<&str>)?;
            let preferences = MenuItem::with_id(
                app,
                "preferences",
                "Preferences...",
                true,
                Some("CmdOrCtrl+,"),
            )?;
            let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
            let separator2 = tauri::menu::PredefinedMenuItem::separator(app)?;
            let menu =
                Menu::with_items(app, &[&about, &separator, &preferences, &separator2, &quit])?;

            if let Some(tray) = app.tray_by_id("main") {
                let _ = tray.set_menu(Some(menu));
                tray.on_menu_event(move |app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "about" => {
                        use tauri::{WebviewUrl, WebviewWindowBuilder};
                        if app.get_webview_window("about").is_none() {
                            let _ = WebviewWindowBuilder::new(
                                app,
                                "about",
                                WebviewUrl::App("about.html".into()),
                            )
                            .title("About Zana")
                            .decorations(false)
                            .transparent(true)
                            .shadow(true)
                            .inner_size(400.0, 440.0)
                            .center()
                            .resizable(false)
                            .build();
                        } else if let Some(window) = app.get_webview_window("about") {
                            let _ = window.set_focus();
                        }
                    }
                    "preferences" => {
                        use tauri::{WebviewUrl, WebviewWindowBuilder};
                        if app.get_webview_window("preferences").is_none() {
                            let _ = WebviewWindowBuilder::new(
                                app,
                                "preferences",
                                WebviewUrl::App("preferences.html".into()),
                            )
                            .title("Zana Preferences")
                            .decorations(false)
                            .transparent(true)
                            .shadow(true)
                            .inner_size(500.0, 680.0)
                            .center()
                            .resizable(false)
                            .build();
                        } else if let Some(window) = app.get_webview_window("preferences") {
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                });
                log::info!("System tray menu attached");
            } else {
                log::warn!("Could not find tray icon by id 'main'");
            }

            log::info!("Zana initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Onboarding
            commands::is_first_run,
            commands::check_accessibility_permission,
            commands::open_accessibility_settings,
            commands::open_microphone_settings,
            commands::open_preferences_window,
            commands::mark_onboarding_complete,
            commands::complete_onboarding_and_restart,
            // Diagnostics
            commands::run_diagnostics,
            // Preferences and Voicebot
            commands::get_preferences,
            commands::save_preferences,
            commands::test_ai_connection,
            commands::get_voicebot_config,
            commands::set_voicebot_terminal,
            commands::set_voicebot_context_command,
            commands::set_voicebot_wake_word,
            commands::set_voicebot_voice,
            commands::set_voicebot_fillers,
            commands::set_voicebot_setting,
            commands::voicebot_self_settings,
            commands::voicebot_screenshot,
            commands::save_voicebot_prompt,
            commands::save_voicebot_mcp,
            commands::voicebot_status,
            commands::voicebot_join,
            commands::voicebot_screen_context,
            commands::open_voicebot_folder,
            commands::voicebot_debug_tail,
            commands::clear_voicebot_voiceprint,
            commands::clear_voicebot_debug,
            open_voicebot_debug,
            voicebot_paste,
            // Updates
            commands::check_for_updates,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                Zana_app::voicebot::stop();
            }
        });
}

/// Start Voicebot's processes in the background and open its floating orb.
fn start_voicebot(app: &tauri::AppHandle, settings: &Zana_app::state::Settings) {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    let code = Zana_app::voicebot::code_dir(app.path().resource_dir().ok());
    if let Err(error) = Zana_app::voicebot::write_agent_config(settings) {
        log::error!("Voicebot config could not be written: {error}");
    }
    log::info!("Voicebot starting; code in {}", code.display());
    Zana_app::voicebot::start(code);

    #[cfg(target_os = "macos")]
    if let Err(error) = create_voicebot_debug_panel(app) {
        log::error!("The Voicebot Debug panel could not be created: {error}");
    }

    let (width, height) = (220.0, 220.0);
    let mut builder =
        WebviewWindowBuilder::new(app, "voicebot", WebviewUrl::App("voicebot.html".into()))
            .title("Voicebot")
            .inner_size(width, height)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .focused(false)
            .visible_on_all_workspaces(true)
            .skip_taskbar(true);

    if let Ok(Some(monitor)) = app.primary_monitor() {
        let scale = monitor.scale_factor();
        let size = monitor.size().to_logical::<f64>(scale);
        builder = builder.position(size.width - width - 24.0, size.height - height - 72.0);
    }

    let window = match builder.build() {
        Ok(window) => window,
        Err(error) => return log::error!("Failed to open the Voicebot window: {error}"),
    };

    #[cfg(target_os = "macos")]
    {
        use panel::OrbPanel;
        use tauri_nspanel::panel::{NSWindowCollectionBehavior, NSWindowStyleMask};

        match window.to_panel::<OrbPanel>() {
            Ok(panel) => {
                use cocoa::base::id;
                use objc::{msg_send, sel, sel_impl};

                panel.set_floating_panel(true);
                panel.set_hides_on_deactivate(false);
                panel.set_style_mask(NSWindowStyleMask::NonactivatingPanel);
                panel.set_collection_behavior(
                    NSWindowCollectionBehavior::CanJoinAllSpaces
                        | NSWindowCollectionBehavior::FullScreenAuxiliary,
                );
                panel.set_level(1000);

                let ns_panel = panel.as_panel() as *const _ as id;
                unsafe {
                    let _: () = msg_send![ns_panel, setMovableByWindowBackground: true];
                }

                panel.show();
                panel.order_front_regardless();
                *VOICEBOT_PANEL.lock().unwrap() = Some(panel);
            }
            Err(error) => log::error!("The Voicebot window could not become a panel: {error}"),
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

#[cfg(target_os = "macos")]
static VOICEBOT_PANEL: StdMutex<Option<tauri_nspanel::PanelHandle<tauri::Wry>>> =
    StdMutex::new(None);

/// Open the Voicebot Debug panel.
#[tauri::command]
async fn open_voicebot_debug(app: tauri::AppHandle) -> Result<(), String> {
    app.run_on_main_thread(|| {
        #[cfg(target_os = "macos")]
        if let Some(panel) = VOICEBOT_DEBUG_PANEL.lock().unwrap().as_ref() {
            panel.show();
            panel.order_front_regardless();
        }
    })
    .map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn create_voicebot_debug_panel(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use panel::OrbPanel;
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    use tauri_nspanel::panel::{NSWindowCollectionBehavior, NSWindowStyleMask};

    let window = WebviewWindowBuilder::new(
        app,
        "voicebot-debug",
        WebviewUrl::App("voicebot-debug.html".into()),
    )
    .title("Voicebot Debug")
    .inner_size(620.0, 680.0)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible_on_all_workspaces(true)
    .focused(false)
    .shadow(true)
    .build()?;

    let panel = window.to_panel::<OrbPanel>()?;
    panel.set_floating_panel(true);
    panel.set_hides_on_deactivate(false);
    panel.set_style_mask(NSWindowStyleMask::NonactivatingPanel | NSWindowStyleMask::Resizable);
    panel.set_collection_behavior(
        NSWindowCollectionBehavior::MoveToActiveSpace
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    panel.set_level(1000);
    panel.hide();
    *VOICEBOT_DEBUG_PANEL.lock().unwrap() = Some(panel);
    Ok(())
}

#[cfg(target_os = "macos")]
static VOICEBOT_DEBUG_PANEL: StdMutex<Option<tauri_nspanel::PanelHandle<tauri::Wry>>> =
    StdMutex::new(None);

/// Voicebot's type-up tool: paste text at the active cursor.
#[tauri::command]
fn voicebot_paste(text: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        paste_text(&text);
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        Err("Voicebot runs on macOS only".into())
    }
}

#[cfg(target_os = "macos")]
fn paste_text(text: &str) {
    use arboard::Clipboard;

    log::info!("Pasting {} chars", text.len());
    if let Ok(mut clipboard) = Clipboard::new() {
        if clipboard.set_text(text).is_err() {
            log::error!("Failed to set clipboard");
            return;
        }
    }

    std::thread::sleep(std::time::Duration::from_millis(50));
    let script = r#"tell application "System Events" to keystroke "v" using command down"#;
    let _ = std::process::Command::new("osascript")
        .args(["-e", script])
        .output();
}
