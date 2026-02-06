//! Plugin Commands
//!
//! Tauri commands for plugin management.

use crate::hooks::HookEvent;
use crate::plugins::{PluginCapabilities, PluginKind, PluginManifest};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

/// Plugin information for list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Unique plugin identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Semantic version
    pub version: String,
    /// Short description
    pub description: String,
    /// Author name
    pub author: String,
    /// Plugin kind
    pub kind: PluginKind,
    /// Plugin capabilities
    pub capabilities: PluginCapabilities,
    /// Whether plugin is enabled
    pub enabled: bool,
}

/// Response for list plugins
#[derive(Debug, Serialize, Deserialize)]
pub struct ListPluginsResponse {
    pub success: bool,
    pub plugins: Vec<PluginInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// List all loaded plugins
#[tauri::command]
pub async fn list_plugins(state: State<'_, AppState>) -> Result<ListPluginsResponse, String> {
    let registry = state.plugin_registry.read().await;

    let plugins: Vec<PluginInfo> = registry
        .all()
        .map(|entry| PluginInfo {
            id: entry.manifest.plugin.id.clone(),
            name: entry.manifest.plugin.name.clone(),
            version: entry.manifest.plugin.version.clone(),
            description: entry.manifest.plugin.description.clone(),
            author: entry.manifest.plugin.author.clone(),
            kind: entry.manifest.plugin.plugin_type.kind,
            capabilities: entry.manifest.plugin.capabilities.clone(),
            enabled: entry.enabled,
        })
        .collect();

    Ok(ListPluginsResponse {
        success: true,
        plugins,
        error: None,
    })
}

/// Enable a plugin
#[tauri::command]
pub async fn enable_plugin(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut registry = state.plugin_registry.write().await;

    if !registry.has(&id) {
        return Err(format!("Plugin not found: {}", id));
    }

    let enabled = registry.enable(&id);

    if !enabled {
        return Err(format!("Failed to enable plugin: {}", id));
    }

    // Emit plugin enabled event
    state
        .event_bus
        .emit(HookEvent::PluginEnabled { id: id.clone() })
        .await;

    log::info!("Enabled plugin: {}", id);
    Ok(())
}

/// Disable a plugin
#[tauri::command]
pub async fn disable_plugin(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut registry = state.plugin_registry.write().await;

    if !registry.has(&id) {
        return Err(format!("Plugin not found: {}", id));
    }

    let disabled = registry.disable(&id);

    if !disabled {
        return Err(format!("Failed to disable plugin: {}", id));
    }

    // Emit plugin disabled event
    state
        .event_bus
        .emit(HookEvent::PluginDisabled { id: id.clone() })
        .await;

    log::info!("Disabled plugin: {}", id);
    Ok(())
}

/// Copy directory recursively
fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Install a plugin from a directory path
///
/// Copies the plugin directory to the plugins directory and loads the plugin.
/// Emits HookEvent::PluginLoaded after successful installation.
#[tauri::command]
pub async fn install_plugin(
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let source_path = std::path::Path::new(&path);

    // Validate source path exists and is a directory
    if !source_path.exists() {
        return Err(format!("Source path does not exist: {}", path));
    }
    if !source_path.is_dir() {
        return Err(format!("Source path is not a directory: {}", path));
    }

    // Read the manifest first to get the plugin ID
    let manifest_path = source_path.join("plugin.toml");
    if !manifest_path.exists() {
        return Err(format!("plugin.toml not found in: {}", path));
    }

    // Parse the manifest to get the plugin ID
    let manifest = PluginManifest::from_file(&manifest_path)
        .map_err(|e| format!("Failed to parse plugin.toml: {}", e))?;

    let plugin_id = &manifest.plugin.id;

    // Determine plugins directory
    let plugins_dir = if cfg!(debug_assertions) {
        std::path::PathBuf::from("plugins")
    } else {
        dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("kvoice")
            .join("plugins")
    };

    // Ensure plugins directory exists
    if !plugins_dir.exists() {
        std::fs::create_dir_all(&plugins_dir)
            .map_err(|e| format!("Failed to create plugins directory: {}", e))?;
    }

    let destination = plugins_dir.join(plugin_id);

    // Check if plugin already exists
    if destination.exists() {
        return Err(format!("Plugin already installed: {}", plugin_id));
    }

    // Copy the plugin directory
    copy_dir(source_path, &destination)
        .map_err(|e| format!("Failed to copy plugin directory: {}", e))?;

    // Load the plugin
    let plugin_manager = state.plugin_manager.lock().await;
    plugin_manager
        .load_plugin(&destination)
        .await
        .map_err(|e| format!("Failed to load plugin: {}", e))?;

    log::info!("Plugin installed successfully: {}", plugin_id);
    Ok(())
}

/// Uninstall a plugin by ID
///
/// Unloads the plugin and removes its directory from the plugins folder.
/// Emits HookEvent::PluginUnloaded after successful uninstallation.
#[tauri::command]
pub async fn uninstall_plugin(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Determine plugins directory
    let plugins_dir = if cfg!(debug_assertions) {
        std::path::PathBuf::from("plugins")
    } else {
        dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("kvoice")
            .join("plugins")
    };

    let plugin_dir = plugins_dir.join(&id);

    // Check if plugin directory exists
    if !plugin_dir.exists() {
        return Err(format!("Plugin directory not found: {}", id));
    }

    // Unload the plugin first
    {
        let plugin_manager = state.plugin_manager.lock().await;
        plugin_manager
            .unload_plugin(&id)
            .await
            .map_err(|e| format!("Failed to unload plugin: {}", e))?;
    }

    // Remove the plugin directory
    std::fs::remove_dir_all(&plugin_dir)
        .map_err(|e| format!("Failed to remove plugin directory: {}", e))?;

    log::info!("Plugin uninstalled successfully: {}", id);
    Ok(())
}
