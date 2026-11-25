use tauri::Manager;

use crate::cleanup::{self, CleanupReport, InventoryEntry, OwnedTarget};

fn targets_for_app(app: &tauri::AppHandle) -> Result<Vec<OwnedTarget>, String> {
    let app_config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("could not resolve the Tauri config directory: {error}"))?;
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("could not resolve the Tauri data directory: {error}"))?;
    let app_local_data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| format!("could not resolve the Tauri local-data directory: {error}"))?;
    let app_cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("could not resolve the Tauri cache directory: {error}"))?;
    let app_log_dir = app
        .path()
        .app_log_dir()
        .map_err(|error| format!("could not resolve the Tauri log directory: {error}"))?;
    let zana_config_dir = dirs::config_dir()
        .ok_or_else(|| "could not resolve Zana's config directory".to_string())?;
    let zana_local_data_dir = dirs::data_local_dir()
        .ok_or_else(|| "could not resolve Zana's local-data directory".to_string())?;

    cleanup::targets_from_paths(
        zana_config_dir,
        zana_local_data_dir,
        app_config_dir,
        app_data_dir,
        app_local_data_dir,
        app_cache_dir,
        app_log_dir,
    )
}

#[tauri::command]
pub fn get_zana_data_inventory(app: tauri::AppHandle) -> Result<Vec<InventoryEntry>, String> {
    Ok(cleanup::inventory(&targets_for_app(&app)?))
}

/// Stop Zana and move its owned data roots to the recoverable Trash.
///
/// The UI must show its in-product confirmation first and pass `confirmed: true`.
/// Shared Hugging Face and uv cache roots are intentionally not targets.
#[tauri::command]
pub fn remove_all_zana_data(
    app: tauri::AppHandle,
    confirmed: bool,
) -> Result<CleanupReport, String> {
    if !confirmed {
        return Err("Remove All Zana Data requires explicit confirmation".to_string());
    }

    let targets = targets_for_app(&app)?;
    crate::voicebot::stop();
    let report = cleanup::move_targets_to_trash(&targets, &cleanup::trash_dir()?)?;

    // The logger and WebView may still have open handles to the moved roots. Exit
    // after the command result reaches the Preferences page so no new data is
    // recreated after the user's cleanup decision.
    let app_to_exit = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        app_to_exit.exit(0);
    });

    Ok(report)
}
