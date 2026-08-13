//! Plugin Commands
//!
//! Tauri commands for plugin management.

use crate::hooks::HookEvent;
use crate::plugins::{PluginCapabilities, PluginKind, PluginManifest};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
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

/// Return the directory in which installed plugins are stored.
fn configured_plugins_dir() -> PathBuf {
    if cfg!(debug_assertions) {
        PathBuf::from("plugins")
    } else {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Zana")
            .join("plugins")
    }
}

/// Validate a plugin ID before using it as a path component.
///
/// Manifest validation also checks the character set, but commands must validate IDs
/// independently: uninstall receives an ID directly, without a manifest.  Requiring a
/// single normal path component prevents absolute paths and `..` traversal even if this
/// function is later used with a different manifest implementation.
fn validate_plugin_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("Plugin ID cannot be empty".to_string());
    }

    if !id
        .chars()
        .all(|character| character.is_alphanumeric() || character == '-' || character == '_')
    {
        return Err(
            "Plugin ID must only contain alphanumeric characters, hyphens, and underscores"
                .to_string(),
        );
    }

    let mut components = Path::new(id).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(component)), None) if component == OsStr::new(id) => Ok(()),
        _ => Err("Plugin ID must be a single path component".to_string()),
    }
}

/// Validate a manifest before deriving a destination path from its ID.
fn validate_install_manifest(manifest: &PluginManifest) -> Result<(), String> {
    manifest
        .validate()
        .map_err(|error| format!("Plugin manifest validation failed: {}", error))?;
    validate_plugin_id(&manifest.plugin.id)
}

/// Ensure a plugin root exists, is a real directory (not a symlink), and return its
/// canonical path.  Canonicalizing the root lets all subsequent containment checks use
/// absolute paths and avoids lexical `..` tricks.
fn prepare_plugins_dir(path: &Path) -> Result<PathBuf, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "Plugins directory must not be a symlink: {:?}",
                    path
                ));
            }
            if !metadata.is_dir() {
                return Err(format!("Plugins path is not a directory: {:?}", path));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(path)
                .map_err(|error| format!("Failed to create plugins directory: {}", error))?;
        }
        Err(error) => {
            return Err(format!("Failed to inspect plugins directory: {}", error));
        }
    }

    // Re-check after create_dir_all so a symlink cannot be accepted as the root.
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Failed to inspect plugins directory: {}", error))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "Plugins directory must not be a symlink: {:?}",
            path
        ));
    }
    if !metadata.is_dir() {
        return Err(format!("Plugins path is not a directory: {:?}", path));
    }

    path.canonicalize()
        .map_err(|error| format!("Failed to resolve plugins directory: {}", error))
}

/// Resolve an already-existing plugin root for uninstall.  Uninstall should not create
/// directories as a side effect of a failed or malicious request.
fn existing_plugins_dir(path: &Path) -> Result<PathBuf, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            "Plugins directory does not exist".to_string()
        } else {
            format!("Failed to inspect plugins directory: {}", error)
        }
    })?;

    if metadata.file_type().is_symlink() {
        return Err(format!(
            "Plugins directory must not be a symlink: {:?}",
            path
        ));
    }
    if !metadata.is_dir() {
        return Err(format!("Plugins path is not a directory: {:?}", path));
    }

    path.canonicalize()
        .map_err(|error| format!("Failed to resolve plugins directory: {}", error))
}

/// Build a destination path that is guaranteed to be a direct child of `plugins_dir`.
fn plugin_destination(plugins_dir: &Path, id: &str) -> Result<PathBuf, String> {
    validate_plugin_id(id)?;

    let destination = plugins_dir.join(id);
    if destination.parent() != Some(plugins_dir)
        || !destination.starts_with(plugins_dir)
        || destination.file_name() != Some(OsStr::new(id))
    {
        return Err("Plugin destination is outside the plugins directory".to_string());
    }

    Ok(destination)
}

/// Check a source tree without following symlinks.  Plugin files are copied as regular
/// files only; symlinks and special files could otherwise make an install read or copy
/// data outside the selected source directory.
fn validate_copy_source(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Failed to inspect plugin source {:?}: {}", path, error))?;

    if metadata.file_type().is_symlink() {
        return Err(format!("Plugin source contains a symlink: {:?}", path));
    }
    if metadata.is_file() {
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(format!("Plugin source contains a special file: {:?}", path));
    }

    for entry in fs::read_dir(path)
        .map_err(|error| format!("Failed to read plugin source {:?}: {}", path, error))?
    {
        let entry =
            entry.map_err(|error| format!("Failed to read plugin source entry: {}", error))?;
        validate_copy_source(&entry.path())?;
    }

    Ok(())
}

fn invalid_copy_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

/// Copy a plugin directory recursively without following symlinks or overwriting an
/// existing destination.  `std::fs::copy` follows a source symlink, so every source and
/// destination entry is checked with `symlink_metadata` first.  A failed copy is
/// transactional: any destination entries created by this call are removed before the
/// error is returned.
fn copy_dir(src: &Path, dst: &Path) -> io::Result<()> {
    let source_metadata = fs::symlink_metadata(src)?;
    if source_metadata.file_type().is_symlink() {
        return Err(invalid_copy_input(format!(
            "plugin source contains a symlink: {:?}",
            src
        )));
    }
    if !source_metadata.is_dir() {
        return Err(invalid_copy_input(format!(
            "plugin source is not a directory: {:?}",
            src
        )));
    }

    match fs::symlink_metadata(dst) {
        Ok(destination_metadata) => {
            if destination_metadata.file_type().is_symlink() {
                return Err(invalid_copy_input(format!(
                    "plugin destination is a symlink: {:?}",
                    dst
                )));
            }
            if !destination_metadata.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("plugin destination is not a directory: {:?}", dst),
                ));
            }
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("plugin destination already exists: {:?}", dst),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(dst)?;
        }
        Err(error) => return Err(error),
    }

    let result = copy_dir_entries(src, dst);
    if result.is_err() {
        let _ = remove_path_safely(dst);
    }
    result
}

fn copy_dir_entries(src: &Path, dst: &Path) -> io::Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = dst.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path)?;

        if metadata.file_type().is_symlink() {
            return Err(invalid_copy_input(format!(
                "plugin source contains a symlink: {:?}",
                source_path
            )));
        }
        if metadata.is_dir() {
            match fs::symlink_metadata(&destination_path) {
                Ok(destination_metadata) => {
                    if destination_metadata.file_type().is_symlink() {
                        return Err(invalid_copy_input(format!(
                            "plugin destination is a symlink: {:?}",
                            destination_path
                        )));
                    }
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!(
                            "plugin destination entry already exists: {:?}",
                            destination_path
                        ),
                    ));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    fs::create_dir(&destination_path)?;
                }
                Err(error) => return Err(error),
            }
            copy_dir_entries(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            if fs::symlink_metadata(&destination_path).is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "plugin destination entry already exists: {:?}",
                        destination_path
                    ),
                ));
            }
            fs::copy(&source_path, &destination_path)?;
        } else {
            return Err(invalid_copy_input(format!(
                "plugin source contains a special file: {:?}",
                source_path
            )));
        }
    }

    Ok(())
}

/// Remove a directory without ever following a symlink.  This is used for both failed
/// install cleanup and uninstall so a plugin-controlled symlink cannot redirect deletion.
fn remove_path_safely(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;

    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return fs::remove_file(path);
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        remove_path_safely(&entry.path())?;
    }

    fs::remove_dir(path)
}

fn ensure_no_path_overlap(source: &Path, destination: &Path) -> Result<(), String> {
    if source == destination || source.starts_with(destination) || destination.starts_with(source) {
        return Err(
            "Plugin source and destination must not contain one another or be the same path"
                .to_string(),
        );
    }
    Ok(())
}

/// Verify the manifest after copying.  This protects against a source changing while it is
/// being copied and ensures the directory name cannot be associated with another plugin ID.
fn verify_installed_manifest(destination: &Path, expected_id: &str) -> Result<(), String> {
    validate_copy_source(destination)?;

    let manifest_path = destination.join("plugin.toml");
    let metadata = fs::symlink_metadata(&manifest_path)
        .map_err(|error| format!("Installed plugin manifest is unavailable: {}", error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Installed plugin manifest must be a regular file".to_string());
    }

    let manifest = PluginManifest::from_file(&manifest_path)
        .map_err(|error| format!("Failed to parse installed plugin.toml: {}", error))?;
    validate_install_manifest(&manifest)?;
    if manifest.plugin.id != expected_id {
        return Err(format!(
            "Installed plugin manifest ID '{}' does not match destination '{}'",
            manifest.plugin.id, expected_id
        ));
    }

    Ok(())
}

/// Read and validate the source manifest without following a manifest symlink.
fn read_source_manifest(source: &Path) -> Result<PluginManifest, String> {
    let manifest_path = source.join("plugin.toml");
    let metadata = fs::symlink_metadata(&manifest_path)
        .map_err(|error| format!("plugin.toml not found in {:?}: {}", source, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("plugin.toml must be a regular file, not a symlink".to_string());
    }

    let manifest = PluginManifest::from_file(&manifest_path)
        .map_err(|error| format!("Failed to parse plugin.toml: {}", error))?;
    validate_install_manifest(&manifest)?;
    Ok(manifest)
}

/// Install a plugin from a directory path
///
/// Copies the plugin directory to the plugins directory and loads the plugin.
/// Emits HookEvent::PluginLoaded after successful installation.
#[tauri::command]
pub async fn install_plugin(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let source_input = Path::new(&path);
    let source_metadata = fs::symlink_metadata(source_input)
        .map_err(|error| format!("Failed to inspect source path: {}", error))?;

    // Do not accept a source directory symlink.  The recursive preflight below also
    // rejects symlinks anywhere inside the plugin package.
    if source_metadata.file_type().is_symlink() {
        return Err(format!("Source path must not be a symlink: {}", path));
    }
    if !source_metadata.is_dir() {
        return Err(format!("Source path is not a directory: {}", path));
    }

    let source_path = source_input
        .canonicalize()
        .map_err(|error| format!("Failed to resolve source path: {}", error))?;
    validate_copy_source(&source_path)?;
    let manifest = read_source_manifest(&source_path)?;
    let plugin_id = manifest.plugin.id.clone();

    // Canonicalize the root before deriving the destination.  The destination is then
    // guaranteed to be a direct child of this root, rather than an attacker-controlled
    // path assembled from the manifest ID.
    let plugins_dir = prepare_plugins_dir(&configured_plugins_dir())?;
    let destination = plugin_destination(&plugins_dir, &plugin_id)?;
    ensure_no_path_overlap(&source_path, &destination)?;

    match fs::symlink_metadata(&destination) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(format!("Plugin destination is a symlink: {}", plugin_id));
            }
            return Err(format!("Plugin already installed: {}", plugin_id));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!("Failed to inspect plugin destination: {}", error));
        }
    }

    if let Err(error) = copy_dir(&source_path, &destination) {
        let _ = remove_path_safely(&destination);
        return Err(format!("Failed to copy plugin directory: {}", error));
    }

    if let Err(error) = verify_installed_manifest(&destination, &plugin_id) {
        let _ = remove_path_safely(&destination);
        return Err(error);
    }

    // Load the plugin.  If loading fails, do not leave a partially installed directory
    // behind.  Keep an already-loaded plugin intact when load_plugin returns
    // AlreadyLoaded; otherwise unload can accidentally remove an unrelated instance.
    let was_loaded = {
        let registry = state.plugin_registry.read().await;
        registry.has(&plugin_id)
    };
    let load_result = {
        let plugin_manager = state.plugin_manager.lock().await;
        plugin_manager.load_plugin(&destination).await
    };
    if let Err(error) = load_result {
        if !was_loaded {
            let _ = {
                let plugin_manager = state.plugin_manager.lock().await;
                plugin_manager.unload_plugin(&plugin_id).await
            };
        }
        let _ = remove_path_safely(&destination);
        return Err(format!("Failed to load plugin: {}", error));
    }

    log::info!("Plugin installed successfully: {}", plugin_id);
    Ok(())
}

/// Uninstall a plugin by ID
///
/// Unloads the plugin and removes its directory from the plugins folder.
/// Emits HookEvent::PluginUnloaded after successful uninstallation.
#[tauri::command]
pub async fn uninstall_plugin(id: String, state: State<'_, AppState>) -> Result<(), String> {
    // Validate the command argument before it ever reaches PathBuf::join.
    validate_plugin_id(&id)?;

    let plugins_dir = existing_plugins_dir(&configured_plugins_dir())?;
    let plugin_dir = plugin_destination(&plugins_dir, &id)?;

    let metadata = fs::symlink_metadata(&plugin_dir).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            format!("Plugin directory not found: {}", id)
        } else {
            format!("Failed to inspect plugin directory: {}", error)
        }
    })?;
    if metadata.file_type().is_symlink() {
        return Err(format!("Plugin directory must not be a symlink: {}", id));
    }
    if !metadata.is_dir() {
        return Err(format!("Plugin path is not a directory: {}", id));
    }

    // The path is expected to be a direct child of the canonical root.  Resolve it and
    // check containment again before unloading/removing it in case the directory was
    // replaced after the initial metadata check.
    let canonical_plugin_dir = plugin_dir
        .canonicalize()
        .map_err(|error| format!("Failed to resolve plugin directory: {}", error))?;
    if !canonical_plugin_dir.starts_with(&plugins_dir)
        || canonical_plugin_dir.parent() != Some(plugins_dir.as_path())
    {
        return Err(format!(
            "Plugin directory is outside the plugins directory: {}",
            id
        ));
    }

    // Never let an ID remove an unrelated directory that happens to have the same name.
    // A missing manifest is treated as a corrupt installation and is not removed silently.
    let manifest = read_source_manifest(&canonical_plugin_dir)?;
    if manifest.plugin.id != id {
        return Err(format!(
            "Plugin manifest ID '{}' does not match requested ID '{}'",
            manifest.plugin.id, id
        ));
    }

    // Unload the plugin first
    {
        let plugin_manager = state.plugin_manager.lock().await;
        plugin_manager
            .unload_plugin(&id)
            .await
            .map_err(|error| format!("Failed to unload plugin: {}", error))?;
    }

    // Remove only the validated directory and never follow nested symlinks.
    remove_path_safely(&canonical_plugin_dir)
        .map_err(|error| format!("Failed to remove plugin directory: {}", error))?;

    log::info!("Plugin uninstalled successfully: {}", id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::TempDir;

    fn valid_manifest(id: &str) -> String {
        format!(
            r#"
[plugin]
 id = "{id}"
 name = "Test Plugin"
 version = "1.0.0"
 description = "A test plugin"
 author = "Tests"

[plugin.type]
 kind = "orb-style"
"#
        )
    }

    #[test]
    fn plugin_ids_are_single_safe_path_components() {
        for id in ["", ".", "..", "../outside", "nested/plugin", "/tmp/plugin"] {
            assert!(validate_plugin_id(id).is_err(), "accepted unsafe ID {id:?}");
        }

        assert!(validate_plugin_id("safe-plugin_1").is_ok());
    }

    #[test]
    fn manifest_id_is_validated_before_destination_creation() {
        let manifest = PluginManifest::from_toml_str(&valid_manifest("../outside")).unwrap();
        assert!(validate_install_manifest(&manifest).is_err());
    }

    #[test]
    fn destination_is_a_child_of_the_canonical_root() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().canonicalize().unwrap();
        let destination = plugin_destination(&root, "safe-plugin").unwrap();

        assert_eq!(destination.parent(), Some(root.as_path()));
        assert!(destination.starts_with(&root));
        assert!(plugin_destination(&root, "../outside").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn copy_dir_rejects_symlinked_plugin_files() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source");
        let destination = temp_dir.path().join("destination");
        let outside = temp_dir.path().join("outside.txt");
        fs::create_dir(&source).unwrap();
        File::create(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, source.join("plugin.toml")).unwrap();

        assert!(validate_copy_source(&source).is_err());
        assert!(copy_dir(&source, &destination).is_err());
        assert!(!destination.exists());
    }

    #[cfg(unix)]
    #[test]
    fn safe_removal_does_not_follow_nested_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugin");
        let outside = temp_dir.path().join("outside");
        fs::create_dir(&plugin_dir).unwrap();
        fs::create_dir(&outside).unwrap();
        File::create(outside.join("must-survive.txt")).unwrap();
        std::os::unix::fs::symlink(&outside, plugin_dir.join("linked")).unwrap();

        remove_path_safely(&plugin_dir).unwrap();

        assert!(!plugin_dir.exists());
        assert!(outside.join("must-survive.txt").exists());
    }

    #[test]
    fn copied_manifest_must_match_destination_id() {
        let temp_dir = TempDir::new().unwrap();
        let destination = temp_dir.path().join("safe-plugin");
        fs::create_dir(&destination).unwrap();
        fs::write(
            destination.join("plugin.toml"),
            valid_manifest("different-id"),
        )
        .unwrap();

        assert!(verify_installed_manifest(&destination, "safe-plugin").is_err());
    }
}
