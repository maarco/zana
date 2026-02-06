//! kVoice Main Entry Point
//!
//! Initializes the Tauri application with NSPanel overlay for fullscreen support.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(deprecated)]
#![allow(unexpected_cfgs)]

use kvoice_app::state::AppState;
use kvoice_app::HookEvent;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex as StdMutex;
use std::time::Instant;
use tauri::Manager;
#[cfg(not(target_os = "macos"))]
use tauri::Emitter;
use tauri::menu::{Menu, MenuBuilder, MenuItem, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;

#[cfg(target_os = "macos")]
mod panel;

#[cfg(target_os = "macos")]
use tauri_nspanel::WebviewWindowExt;

#[cfg(target_os = "macos")]
#[allow(unused_imports)]
use tauri_nspanel::Panel; // for as_panel()

/// Evaluate JavaScript directly in the panel's WKWebView
/// This bypasses Tauri's IPC which doesn't work with NSPanel
#[cfg(target_os = "macos")]
fn eval_js_in_panel(panel: &tauri_nspanel::PanelHandle<tauri::Wry>, js: &str) {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::{msg_send, sel, sel_impl};

    unsafe {
        // Get the NSPanel pointer via raw casting
        let ns_panel = panel.as_panel();
        let ns_panel_ptr = ns_panel as *const _ as id;

        // Get content view
        let content_view: id = msg_send![ns_panel_ptr, contentView];
        if content_view == nil {
            log::warn!("[eval_js] contentView is null");
            return;
        }

        // Find WKWebView in subviews (it's usually nested)
        if let Some(webview) = find_wkwebview_cocoa(content_view) {
            // Create NSString from the JavaScript
            let js_nsstring = NSString::alloc(nil).init_str(js);

            // Call evaluateJavaScript:completionHandler:
            let _: () = msg_send![webview, evaluateJavaScript:js_nsstring completionHandler:nil];
        } else {
            log::warn!("[eval_js] WKWebView not found in panel");
        }
    }
}

/// Recursively find WKWebView in view hierarchy using cocoa types
#[cfg(target_os = "macos")]
unsafe fn find_wkwebview_cocoa(view: cocoa::base::id) -> Option<cocoa::base::id> {
    use cocoa::base::{id, nil};
    use objc::{msg_send, sel, sel_impl, class};

    if view == nil {
        return None;
    }

    // Check if this view is a WKWebView
    let wkwebview_class = class!(WKWebView);
    let is_wkwebview: bool = msg_send![view, isKindOfClass:wkwebview_class];

    if is_wkwebview {
        log::debug!("[eval_js] Found WKWebView!");
        return Some(view);
    }

    // Check subviews
    let subviews: id = msg_send![view, subviews];
    if subviews == nil {
        return None;
    }

    let count: usize = msg_send![subviews, count];
    for i in 0..count {
        let subview: id = msg_send![subviews, objectAtIndex:i];
        if let Some(webview) = find_wkwebview_cocoa(subview) {
            return Some(webview);
        }
    }

    None
}

// Re-export commands for tauri::generate_handler! macro
mod commands {
    pub use kvoice_app::commands::*;
}

// Global state for Fn key handling
static FN_KEY_PRESSED: AtomicBool = AtomicBool::new(false);
static LAST_FN_PRESS: StdMutex<Option<Instant>> = StdMutex::new(None);
static LAST_FN_RELEASE: StdMutex<Option<Instant>> = StdMutex::new(None);
static IS_RECORDING: AtomicBool = AtomicBool::new(false);
static LATCHED_RECORDING: AtomicBool = AtomicBool::new(false); // Recording started by double-tap

#[cfg(target_os = "macos")]
static ORB_PANEL: StdMutex<Option<tauri_nspanel::PanelHandle<tauri::Wry>>> = StdMutex::new(None);

// Store event monitor handles as usize (can't store raw id pointers directly in static)
// Cast to/from id when needed: id as usize to store, stored_value as id to use
#[cfg(target_os = "macos")]
static FN_KEY_MONITORS: StdMutex<Option<(usize, usize)>> = StdMutex::new(None);

// Cancel flag for pending hide operations
#[cfg(target_os = "macos")]
static HIDE_CANCELLED: AtomicBool = AtomicBool::new(false);

/// Create application menu with standard shortcuts
fn create_app_menu(app: &tauri::AppHandle) -> Result<Menu<tauri::Wry>, tauri::Error> {
    // App menu (macOS standard)
    let app_menu = SubmenuBuilder::new(app, "kVoice")
        .item(&PredefinedMenuItem::about(app, None, None)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("preferences", "Preferences...")
                .accelerator("CmdOrCtrl+,")
                .build(app)?,
        )
        .separator()
        .item(&PredefinedMenuItem::services(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::hide(app, None)?)
        .item(&PredefinedMenuItem::hide_others(app, None)?)
        .item(&PredefinedMenuItem::show_all(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::quit(app, None)?)
        .build()?;

    // Edit menu with standard editing commands
    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .item(&PredefinedMenuItem::undo(app, None)?)
        .item(&PredefinedMenuItem::redo(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, None)?)
        .item(&PredefinedMenuItem::copy(app, None)?)
        .item(&PredefinedMenuItem::paste(app, None)?)
        .item(&PredefinedMenuItem::select_all(app, None)?)
        .build()?;

    // Window menu
    let window_menu = SubmenuBuilder::new(app, "Window")
        .item(&PredefinedMenuItem::minimize(app, None)?)
        .item(&PredefinedMenuItem::maximize(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::close_window(app, None)?)
        .build()?;

    // Help menu
    let help_menu = SubmenuBuilder::new(app, "Help")
        .item(
            &MenuItemBuilder::with_id("show_help", "kVoice Help")
                .accelerator("CmdOrCtrl+?")
                .build(app)?,
        )
        .build()?;

    let menu = MenuBuilder::new(app)
        .item(&app_menu)
        .item(&edit_menu)
        .item(&window_menu)
        .item(&help_menu)
        .build()?;

    Ok(menu)
}

fn main() {
    // Setup crash handler to log panics
    std::panic::set_hook(Box::new(|panic_info| {
        let crash_log = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("kvoice")
            .join("crash.log");

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let message = format!(
            "\n=== CRASH REPORT ===\nTime: {}\nVersion: {}\n{}\n",
            timestamp,
            env!("CARGO_PKG_VERSION"),
            panic_info
        );

        // Try to write crash log
        if let Some(parent) = crash_log.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_log)
            .and_then(|mut f| std::io::Write::write_all(&mut f, message.as_bytes()));

        eprintln!("{}", message);
        eprintln!("Crash log written to: {:?}", crash_log);
    }));

    // Suppress whisper.cpp verbose logs (they go directly to stderr)
    std::env::set_var("GGML_LOG_LEVEL", "error");
    std::env::set_var("WHISPER_LOG_LEVEL", "error");
    std::env::set_var("GGML_METAL_LOG_LEVEL", "error");

    // Redirect whisper.cpp logs to Rust logging (suppresses stderr output)
    whisper_rs::install_logging_hooks();

    // Initialize logging - whisper logs will go through here now
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,whisper_rs=warn")).init();

    log::info!("Starting kVoice...");

    let mut builder = tauri::Builder::default();

    // Register macOS NSPanel plugin
    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        // Auto-updater disabled until signing keys are configured
        // .plugin(tauri_plugin_updater::Builder::new().build())
        .menu(|app| create_app_menu(app))
        .setup(|app| {
            let is_first_run = kvoice_app::onboarding::is_first_run();

            // ALWAYS create application state (needed for commands)
            log::info!("Initializing application state...");
            let state = AppState::new().expect("Failed to initialize app state");
            let event_bus = state.event_bus.clone();
            let event_bus_for_emit = event_bus.clone();
            let plugin_manager = state.plugin_manager.clone();

            // Store state for commands
            app.manage(state);

            // Check if first run - show onboarding window
            if is_first_run {
                log::info!("First run detected - showing onboarding window");

                use tauri::{WebviewUrl, WebviewWindowBuilder};

                let _onboarding_window = WebviewWindowBuilder::new(
                    app,
                    "onboarding",
                    WebviewUrl::App("onboarding.html".into())
                )
                .title("kVoice Genesis")
                .inner_size(900.0, 700.0)
                .center()
                .resizable(false)
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .build()
                .expect("Failed to create onboarding window");

                // Enable dev tools in debug mode
                #[cfg(debug_assertions)]
                {
                    _onboarding_window.open_devtools();
                }

                // Don't setup Fn key monitor during onboarding
                log::info!("Onboarding mode - skipping Fn key monitor");
                return Ok(());
            }

            // Normal startup (not first run) - setup everything
            log::info!("Normal startup - loading plugins and setting up monitors");

            // Load plugins
            tauri::async_runtime::spawn(async move {
                if let Err(e) = plugin_manager.lock().await.load_all().await {
                    log::error!("Failed to load plugins: {}", e);
                }
            });

            // Forward audio level events to frontend
            use kvoice_app::hooks::HookEventType;
            #[cfg(not(target_os = "macos"))]
            let app_handle_for_events = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut receiver = event_bus.subscribe(HookEventType::AudioLevelChange).await;
                let mut _event_count = 0u64;
                while let Ok(event) = receiver.recv().await {
                    if let HookEvent::AudioLevelChange { level, peak } = event {
                        _event_count += 1;

                        // Inject audio level directly into panel's WKWebView
                        // Tauri IPC doesn't work with NSPanel - use direct ObjC calls
                        #[cfg(target_os = "macos")]
                        {
                            let panel_guard = ORB_PANEL.lock().unwrap();
                            if let Some(ref panel) = *panel_guard {
                                let js = format!(
                                    "window.setAudioLevel && window.setAudioLevel({}, {})",
                                    level, peak
                                );
                                eval_js_in_panel(panel, &js);
                            }
                            drop(panel_guard);
                        }

                        // Fallback: also emit for non-macOS or regular windows
                        #[cfg(not(target_os = "macos"))]
                        {
                            let _ = app_handle_for_events.emit("audio-level", serde_json::json!({
                                "level": level,
                                "peak": peak
                            }));
                        }
                    }
                }
            });

            // Emit app started event
            tauri::async_runtime::spawn(async move {
                event_bus_for_emit.emit(HookEvent::AppStarted).await;
            });

            // Setup Fn key monitor (macOS)
            #[cfg(target_os = "macos")]
            {
                let app_handle = app.handle().clone();
                setup_fn_key_monitor(app_handle);
            }

            // Setup config file watcher for hot reload
            #[cfg(target_os = "macos")]
            {
                setup_config_watcher(app.handle().clone());
            }

            // Setup system tray icon and menu
            {
                let quit = MenuItem::with_id(app, "quit", "Quit kVoice", true, Some("Cmd+Q"))?;
                let about = MenuItem::with_id(app, "about", "About kVoice", true, None::<&str>)?;
                let preferences = MenuItem::with_id(app, "preferences", "Preferences...", true, Some("Cmd+,"))?;
                let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
                let separator2 = tauri::menu::PredefinedMenuItem::separator(app)?;

                let menu = Menu::with_items(app, &[&about, &separator, &preferences, &separator2, &quit])?;

                let _tray = TrayIconBuilder::new()
                    .icon(app.default_window_icon().cloned().unwrap())
                    .menu(&menu)
                    .tooltip("kVoice - Voice to Text")
                    .on_menu_event(|app, event| {
                        match event.id.as_ref() {
                            "quit" => {
                                log::info!("Quit requested from tray menu");
                                app.exit(0);
                            }
                            "about" => {
                                log::info!("About requested from tray menu");
                                use tauri::{WebviewUrl, WebviewWindowBuilder};
                                if app.get_webview_window("about").is_none() {
                                    let _ = WebviewWindowBuilder::new(
                                        app,
                                        "about",
                                        WebviewUrl::App("about.html".into())
                                    )
                                    .title("About kVoice")
                                    .inner_size(400.0, 500.0)
                                    .center()
                                    .resizable(false)
                                    .build();
                                } else if let Some(window) = app.get_webview_window("about") {
                                    let _ = window.set_focus();
                                }
                            }
                            "preferences" => {
                                log::info!("Preferences requested from tray menu");
                                use tauri::{WebviewUrl, WebviewWindowBuilder};
                                if app.get_webview_window("preferences").is_none() {
                                    let _ = WebviewWindowBuilder::new(
                                        app,
                                        "preferences",
                                        WebviewUrl::App("preferences.html".into())
                                    )
                                    .title("kVoice Preferences")
                                    .inner_size(500.0, 600.0)
                                    .center()
                                    .resizable(false)
                                    .build();
                                } else if let Some(window) = app.get_webview_window("preferences") {
                                    let _ = window.set_focus();
                                }
                            }
                            _ => {}
                        }
                    })
                    .build(app)?;

                log::info!("System tray icon created");
            }

            log::info!("kVoice initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Audio commands
            commands::list_audio_devices,
            commands::start_recording,
            commands::stop_recording,
            commands::get_audio_metrics,
            commands::is_recording,
            // Transcription commands
            commands::list_models,
            commands::download_model,
            commands::set_model,
            commands::transcribe,
            commands::transcribe_preview,
            // Plugin commands
            commands::list_plugins,
            commands::enable_plugin,
            commands::disable_plugin,
            commands::install_plugin,
            commands::uninstall_plugin,
            // Onboarding commands
            commands::is_first_run,
            commands::check_accessibility_permission,
            commands::open_accessibility_settings,
            commands::download_whisper_model,
            commands::mark_onboarding_complete,
            commands::complete_onboarding_and_exit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Create the orb window as an NSPanel (macOS only)
#[cfg(target_os = "macos")]
fn create_orb_window(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use cocoa::appkit::NSScreen;
    use panel::OrbPanel;
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    use tauri_nspanel::panel::{NSWindowCollectionBehavior, NSWindowStyleMask};

    log::info!("Creating orb window as NSPanel...");

    // Get screen dimensions for fullscreen
    let (screen_width, screen_height) = unsafe {
        let main_screen = NSScreen::mainScreen(std::ptr::null_mut());
        let frame = NSScreen::frame(main_screen);
        (frame.size.width, frame.size.height)
    };

    log::info!("[CreateOrb] Screen size: {}x{}", screen_width, screen_height);

    // Create WebviewWindow - FULLSCREEN
    let orb = WebviewWindowBuilder::new(app, "orb", WebviewUrl::App("orb.html".into()))
        .title("kVoice Orb")
        .inner_size(screen_width, screen_height)
        .position(0.0, 0.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .visible(false)
        .shadow(false)
        .build()?;

    // Convert to NSPanel for fullscreen support
    let panel = orb.to_panel::<OrbPanel>()?;

    // Configure NSPanel
    panel.set_floating_panel(true);
    panel.set_hides_on_deactivate(false);
    panel.set_movable_by_window_background(true);  // Enable drag anywhere to move

    // NonactivatingPanel prevents focus stealing
    panel.set_style_mask(NSWindowStyleMask::NonactivatingPanel | NSWindowStyleMask::Resizable);

    // FullScreenAuxiliary allows appearing over fullscreen apps
    panel.set_collection_behavior(
        NSWindowCollectionBehavior::MoveToActiveSpace
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );

    // Window level 1000 = above fullscreen
    panel.set_level(1000);

    // Make panel ignore mouse events (click-through) for fullscreen overlay
    {
        use cocoa::base::id;
        use objc::{msg_send, sel, sel_impl};
        let ns_panel = panel.as_panel();
        let ns_panel_ptr = ns_panel as *const _ as id;
        unsafe {
            let _: () = msg_send![ns_panel_ptr, setIgnoresMouseEvents: true];
        }
        log::info!("[CreateOrb] Panel set to ignore mouse events (click-through)");
    }

    // Store panel reference (clone for storage)
    *ORB_PANEL.lock().unwrap() = Some(panel.clone());

    log::info!("Orb NSPanel created successfully");
    Ok(())
}

/// Show the orb window with fade in
#[cfg(target_os = "macos")]
fn show_orb(app: &tauri::AppHandle) {
    use cocoa::base::id;
    use cocoa::foundation::NSPoint;
    use objc::{msg_send, sel, sel_impl};

    // Cancel any pending hide operations
    HIDE_CANCELLED.store(true, Ordering::SeqCst);
    log::info!("[ShowOrb] Cancelled any pending hide operations");

    // Create orb if not exists
    if ORB_PANEL.lock().unwrap().is_none() {
        log::info!("[ShowOrb] Creating new orb window");
        if let Err(e) = create_orb_window(app) {
            log::error!("Failed to create orb window: {}", e);
            return;
        }
        log::info!("[ShowOrb] Orb window created successfully");
    } else {
        log::info!("[ShowOrb] Orb window already exists");
    }

    // Show panel at fullscreen position (0, 0)
    if let Some(ref panel) = *ORB_PANEL.lock().unwrap() {
        log::info!("[ShowOrb] Showing orb panel (fullscreen)");

        // Position at origin for fullscreen
        let ns_panel = panel.as_panel();
        let ns_panel_ptr = ns_panel as *const _ as id;
        let origin = NSPoint { x: 0.0, y: 0.0 };
        unsafe {
            let _: () = msg_send![ns_panel_ptr, setFrameOrigin:origin];
        }
        log::info!("[ShowOrb] Positioned at origin (0, 0)");

        // Reset orb state and trigger fade in
        eval_js_in_panel(panel, "window.resetOrb && window.resetOrb()");
        eval_js_in_panel(panel, "window.fadeIn && window.fadeIn()");

        panel.show();
        panel.order_front_regardless();
        log::info!("[ShowOrb] Orb panel shown with fade in");
    }
}

/// Hide the orb window with fade out (saves position first)
#[cfg(target_os = "macos")]
fn hide_orb(app: &tauri::AppHandle) {
    use cocoa::base::id;

    log::info!("[HideOrb] Starting fade out (fullscreen mode)");

    // Reset cancel flag at start of hide
    HIDE_CANCELLED.store(false, Ordering::SeqCst);

    // Trigger fade out (for fullscreen, we just fade to opacity 0)
    {
        let panel_guard = ORB_PANEL.lock().unwrap();
        if let Some(ref panel) = *panel_guard {
            eval_js_in_panel(panel, "window.fadeOut && window.fadeOut()");
            log::info!("[HideOrb] Triggered fade out");
        }
    }

    // For fullscreen overlay, after fade we order the panel out
    let app_clone = app.clone();
    std::thread::spawn(move || {
        // Wait for fade out animation
        std::thread::sleep(std::time::Duration::from_millis(1500));

        // Check if hide was cancelled (user started new recording)
        if HIDE_CANCELLED.load(Ordering::SeqCst) {
            log::info!("[HideOrb] Hide cancelled - new recording started");
            return;
        }

        // Order panel out (hide it) after fade completes
        let _ = app_clone.run_on_main_thread(move || {
            use objc::{msg_send, sel, sel_impl};

            // Double-check cancel flag on main thread
            if HIDE_CANCELLED.load(Ordering::SeqCst) {
                log::info!("[HideOrb] Hide cancelled on main thread");
                return;
            }

            if let Some(ref panel) = *ORB_PANEL.lock().unwrap() {
                let ns_panel = panel.as_panel();
                let ns_panel_ptr = ns_panel as *const _ as id;

                // Order out the panel (hide it)
                unsafe {
                    let _: () = msg_send![ns_panel_ptr, orderOut: std::ptr::null::<objc::runtime::Object>()];
                }

                log::info!("[HideOrb] Orb panel ordered out (fullscreen mode)");
            }
        });
    });
}

/// Setup config file watcher for hot reload
#[cfg(target_os = "macos")]
fn setup_config_watcher(_app: tauri::AppHandle) {
    use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;
    use std::time::Duration;

    std::thread::spawn(move || {
        // Try multiple paths to find the config file
        let possible_paths = [
            // Development paths
            std::path::PathBuf::from("src-ui/orb_config.json"),
            std::path::PathBuf::from("../src-ui/orb_config.json"),
            // Absolute dev path
            std::path::PathBuf::from("/Users/malmazan/dev/kVoice/src-ui/orb_config.json"),
        ];

        let config_path = possible_paths
            .into_iter()
            .find(|p| p.exists());

        let config_path = match config_path {
            Some(p) => p,
            None => {
                log::warn!("[ConfigWatcher] Config file not found in any location, hot reload disabled");
                return;
            }
        };

        log::info!("[ConfigWatcher] Watching {:?}", config_path);

        let (tx, rx) = channel();

        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                if let Ok(event) = res {
                    let _ = tx.send(event);
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        ) {
            Ok(w) => w,
            Err(e) => {
                log::error!("[ConfigWatcher] Failed to create watcher: {}", e);
                return;
            }
        };

        if let Err(e) = watcher.watch(&config_path, RecursiveMode::NonRecursive) {
            log::error!("[ConfigWatcher] Failed to watch file: {}", e);
            return;
        }

        // Listen for file changes
        loop {
            match rx.recv() {
                Ok(event) => {
                    if event.kind.is_modify() {
                        log::info!("[ConfigWatcher] Config file changed, reloading...");

                        // Read the new config
                        if let Ok(content) = std::fs::read_to_string(&config_path) {
                            // Push to webview
                            let panel_guard = ORB_PANEL.lock().unwrap();
                            if let Some(ref panel) = *panel_guard {
                                let escaped = content.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
                                let js = format!("window.updateConfig && window.updateConfig(\"{}\")", escaped);
                                eval_js_in_panel(panel, &js);
                                log::info!("[ConfigWatcher] Config pushed to webview");
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });
}

/// Setup Fn key monitor using NSEvent (macOS only)
#[cfg(target_os = "macos")]
fn setup_fn_key_monitor(app: tauri::AppHandle) {
    use cocoa::base::id;
    use objc::rc::autoreleasepool;
    use objc::{class, msg_send, sel, sel_impl};

    log::info!("Setting up Fn key monitor...");

    // Check accessibility permissions
    let has_permissions = unsafe {
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        AXIsProcessTrusted()
    };

    if !has_permissions {
        log::warn!("Accessibility permissions not granted - Fn key monitoring disabled");
        log::warn!("Grant access in System Settings > Privacy & Security > Accessibility");
        return;
    }

    log::info!("Accessibility permissions granted");

    let app_clone = app.clone();
    let app_clone2 = app.clone();

    // NSFlagsChangedMask = 1 << 12
    let flags_changed_mask: u64 = 1 << 12;
    const FN_KEY_CODE: u16 = 63;

    unsafe {
        autoreleasepool(|| {
            // Global monitor (when app not focused)
            let block = block::ConcreteBlock::new(move |event: id| {
                let key_code: u16 = msg_send![event, keyCode];
                let flags: u64 = msg_send![event, modifierFlags];
                let fn_flag: u64 = 1 << 23; // NSEventModifierFlagFunction
                let fn_is_pressed = (flags & fn_flag) != 0;
                let was_pressed = FN_KEY_PRESSED.load(Ordering::SeqCst);

                // Log all flag change events for debugging
                log::debug!("[FnMonitor] keyCode={}, flags=0x{:x}, fn_pressed={}, was_pressed={}",
                    key_code, flags, fn_is_pressed, was_pressed);

                if fn_is_pressed && !was_pressed && key_code == FN_KEY_CODE {
                    FN_KEY_PRESSED.store(true, Ordering::SeqCst);
                    *LAST_FN_PRESS.lock().unwrap() = Some(Instant::now());
                    log::info!("Fn key PRESSED (global, keyCode={})", key_code);
                    handle_fn_press(&app_clone);
                } else if !fn_is_pressed && was_pressed {
                    FN_KEY_PRESSED.store(false, Ordering::SeqCst);
                    log::info!("Fn key RELEASED (global, keyCode={})", key_code);
                    handle_fn_release(&app_clone);
                }
            });
            let block = block.copy();

            let global_monitor: id = msg_send![
                class!(NSEvent),
                addGlobalMonitorForEventsMatchingMask:flags_changed_mask
                handler:block
            ];

            // Local monitor (when app focused)
            let block_local = block::ConcreteBlock::new(move |event: id| -> id {
                let key_code: u16 = msg_send![event, keyCode];
                let flags: u64 = msg_send![event, modifierFlags];
                let fn_flag: u64 = 1 << 23;
                let fn_is_pressed = (flags & fn_flag) != 0;
                let was_pressed = FN_KEY_PRESSED.load(Ordering::SeqCst);

                // Log all flag change events for debugging
                log::debug!("[FnMonitor-Local] keyCode={}, flags=0x{:x}, fn_pressed={}, was_pressed={}",
                    key_code, flags, fn_is_pressed, was_pressed);

                if fn_is_pressed && !was_pressed && key_code == FN_KEY_CODE {
                    FN_KEY_PRESSED.store(true, Ordering::SeqCst);
                    *LAST_FN_PRESS.lock().unwrap() = Some(Instant::now());
                    log::info!("Fn key PRESSED [local, keyCode={}]", key_code);
                    handle_fn_press(&app_clone2);
                } else if !fn_is_pressed && was_pressed {
                    FN_KEY_PRESSED.store(false, Ordering::SeqCst);
                    log::info!("Fn key RELEASED [local, keyCode={}]", key_code);
                    handle_fn_release(&app_clone2);
                }

                event
            });
            let block_local = block_local.copy();

            let local_monitor: id = msg_send![
                class!(NSEvent),
                addLocalMonitorForEventsMatchingMask:flags_changed_mask
                handler:block_local
            ];

            // Store monitor handles as usize (cast from id pointers)
            *FN_KEY_MONITORS.lock().unwrap() = Some((global_monitor as usize, local_monitor as usize));

            log::info!("Fn key monitors active");
        });
    }
}

/// Handle Fn key press - show orb and start recording
#[cfg(target_os = "macos")]
fn handle_fn_press(app: &tauri::AppHandle) {
    // Check timing for double-tap detection
    let ms_since_release = LAST_FN_RELEASE
        .lock()
        .unwrap()
        .map(|t| t.elapsed().as_millis())
        .unwrap_or(999999);

    log::info!("[FnPress] Fn key pressed ({}ms since last release, IS_RECORDING={}, LATCHED={})",
        ms_since_release,
        IS_RECORDING.load(Ordering::SeqCst),
        LATCHED_RECORDING.load(Ordering::SeqCst));

    // Check for double-tap (tap within 300ms of last release)
    let is_double_tap = ms_since_release < 300;

    if is_double_tap {
        log::info!("[FnPress] *** DOUBLE-TAP DETECTED *** ({}ms)", ms_since_release);
    }

    // If already recording in latched mode, single tap stops it
    if IS_RECORDING.load(Ordering::SeqCst) && LATCHED_RECORDING.load(Ordering::SeqCst) {
        log::info!("[FnPress] Single tap while latched - stopping recording");
        stop_recording(app);
        return;
    }

    // If double-tap, start latched recording mode
    if is_double_tap && !IS_RECORDING.load(Ordering::SeqCst) {
        log::info!("[FnPress] *** STARTING LATCHED MODE ***");
        LATCHED_RECORDING.store(true, Ordering::SeqCst);
    }

    // Show orb
    show_orb(app);

    // Start recording
    if !IS_RECORDING.load(Ordering::SeqCst) {
        IS_RECORDING.store(true, Ordering::SeqCst);
        log::info!("[FnPress] IS_RECORDING flag set to true");

        // Small delay to ensure orb window has loaded
        let app_clone = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(150));
            log::info!("[FnPress] Delay complete, emitting events");

            // Use direct WKWebView eval - Tauri IPC doesn't work with NSPanel
            let panel_guard = ORB_PANEL.lock().unwrap();
            if let Some(ref panel) = *panel_guard {
                log::info!("[FnPress] Setting recording state via WKWebView eval");
                eval_js_in_panel(panel, "window.setRecordingState && window.setRecordingState(true, false)");
                log::info!("[FnPress] setRecordingState(true) called");
            } else {
                log::error!("[FnPress] No orb panel available!");
            }
            drop(panel_guard);

            // Start audio capture via state
            tauri::async_runtime::spawn(async move {
                log::info!("[FnPress] Starting audio capture...");
                if let Some(state) = app_clone.try_state::<AppState>() {
                    let capture = state.audio_capture.lock().await;
                    match capture.start(None).await {
                        Ok(_) => log::info!("[FnPress] Audio capture started successfully"),
                        Err(e) => log::error!("[FnPress] Failed to start recording: {}", e),
                    }
                } else {
                    log::error!("[FnPress] No app state available!");
                }
            });

            log::info!("[FnPress] Recording started");
        });
    } else {
        log::warn!("[FnPress] Already recording, ignoring");
    }
}

/// Handle Fn key release - stop recording, transcribe, paste, hide
#[cfg(target_os = "macos")]
fn handle_fn_release(app: &tauri::AppHandle) {
    log::info!("[FnRelease] Fn key released, IS_RECORDING={}, LATCHED={}",
        IS_RECORDING.load(Ordering::SeqCst),
        LATCHED_RECORDING.load(Ordering::SeqCst));

    // Save release time for double-tap detection
    *LAST_FN_RELEASE.lock().unwrap() = Some(Instant::now());

    // If in latched mode, user must tap again to stop - ignore release
    if LATCHED_RECORDING.load(Ordering::SeqCst) {
        log::info!("[FnRelease] Latched mode active - ignoring release, tap again to stop");
        return;
    }

    // Check minimum hold duration (300ms)
    let held_long_enough = LAST_FN_PRESS
        .lock()
        .unwrap()
        .map(|t| t.elapsed().as_millis() >= 300)
        .unwrap_or(false);

    if !held_long_enough {
        log::info!("[FnRelease] Released too quickly ({} ms), ignoring",
            LAST_FN_PRESS.lock().unwrap().map(|t| t.elapsed().as_millis()).unwrap_or(0));
        hide_orb(app);
        return;
    }

    // Stop recording (hold-mode)
    if IS_RECORDING.load(Ordering::SeqCst) {
        log::info!("[FnRelease] Stopping recording (hold mode)");
        stop_recording(app);
    }
}

/// Stop recording, transcribe, and paste
#[cfg(target_os = "macos")]
fn stop_recording(app: &tauri::AppHandle) {
    if !IS_RECORDING.load(Ordering::SeqCst) {
        return;
    }

    IS_RECORDING.store(false, Ordering::SeqCst);
    LATCHED_RECORDING.store(false, Ordering::SeqCst); // Clear latched mode
    log::info!("[StopRecording] Stopping recording");

    // Use direct WKWebView eval - Tauri IPC doesn't work with NSPanel
    {
        let panel_guard = ORB_PANEL.lock().unwrap();
        if let Some(ref panel) = *panel_guard {
            eval_js_in_panel(panel, "window.setRecordingState && window.setRecordingState(false, true)");
        }
    }

    // Stop capture and transcribe
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(state) = app_clone.try_state::<AppState>() {
            // Stop capture
            let capture = state.audio_capture.lock().await;
            match capture.stop().await {
                Ok(audio) => {
                    log::info!("Recording stopped: {} samples", audio.samples.len());

                    // Transcribe
                    let whisper = state.whisper_engine.lock().await;
                    match whisper
                        .transcribe(&audio.samples, kvoice_app::WhisperModel::Small)
                        .await
                    {
                        Ok(result) => {
                            log::info!("Transcription: {}", result.text);

                            // Use direct WKWebView eval - Tauri IPC doesn't work with NSPanel
                            {
                                let panel_guard = ORB_PANEL.lock().unwrap();
                                if let Some(ref panel) = *panel_guard {
                                    let escaped = result.text.replace('\\', "\\\\").replace('"', "\\\"");
                                    let js = format!("window.setTranscriptionComplete && window.setTranscriptionComplete(\"{}\")", escaped);
                                    eval_js_in_panel(panel, &js);
                                }
                            }

                            // Paste text
                            if !result.text.trim().is_empty() {
                                paste_text(&result.text);
                            }
                        }
                        Err(e) => {
                            log::error!("Transcription failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to stop recording: {}", e);
                }
            }
        }

        // Wait for poof animation to finish before starting fade out
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;

        // Only hide if not recording again (user might have pressed Fn again)
        if !IS_RECORDING.load(Ordering::SeqCst) {
            hide_orb(&app_clone);
        } else {
            log::info!("[HideOrb] Skipping hide - new recording started");
        }
    });
}

/// Paste text to active input using clipboard + Cmd+V
/// Transcription stays in clipboard (not restored to original)
#[cfg(target_os = "macos")]
fn paste_text(text: &str) {
    use arboard::Clipboard;

    log::info!("Pasting {} chars", text.len());

    // Set text to clipboard (keeps transcription available even if paste fails)
    if let Ok(mut clipboard) = Clipboard::new() {
        if clipboard.set_text(text).is_err() {
            log::error!("Failed to set clipboard");
            return;
        }
    }

    // Small delay
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Simulate Cmd+V using AppleScript
    let script = r#"tell application "System Events" to keystroke "v" using command down"#;
    let _ = std::process::Command::new("osascript")
        .args(["-e", script])
        .output();

    // Transcription stays in clipboard - user can paste again if needed
}
