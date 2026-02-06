//! Plugin Registry
//!
//! Central registry for all loaded plugins.

use super::manifest::{PluginKind, PluginManifest};
use super::traits::Plugin;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Registered plugin entry
pub struct PluginEntry {
    /// Plugin manifest
    pub manifest: PluginManifest,
    /// Plugin instance
    pub plugin: Arc<RwLock<dyn Plugin>>,
    /// Plugin kind
    pub kind: PluginKind,
    /// Whether plugin is enabled
    pub enabled: bool,
}

/// Central plugin registry
#[derive(Default)]
pub struct PluginRegistry {
    /// All registered plugins by ID
    plugins: HashMap<String, PluginEntry>,

    /// Orb style plugins (for quick lookup)
    orb_styles: Vec<String>,

    /// Audio processor plugins (in order)
    audio_processors: Vec<String>,

    /// Post-processor plugins (in order)
    post_processors: Vec<String>,

    /// Integration plugins
    integrations: Vec<String>,
}

impl PluginRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a plugin
    pub fn register(&mut self, plugin: Arc<RwLock<dyn Plugin>>, manifest: PluginManifest) {
        let id = manifest.plugin.id.clone();
        let kind = manifest.plugin.plugin_type.kind;

        // Add to type-specific list
        match kind {
            PluginKind::OrbStyle => {
                self.orb_styles.push(id.clone());
            }
            PluginKind::AudioProcessor => {
                self.audio_processors.push(id.clone());
            }
            PluginKind::PostProcessor => {
                self.post_processors.push(id.clone());
            }
            PluginKind::Integration => {
                self.integrations.push(id.clone());
            }
        }

        // Add to main registry
        self.plugins.insert(
            id.clone(),
            PluginEntry {
                manifest,
                plugin,
                kind,
                enabled: true,
            },
        );

        log::info!("Registered plugin: {} ({:?})", id, kind);
    }

    /// Unregister a plugin by ID
    pub fn unregister(&mut self, id: &str) -> Option<PluginEntry> {
        if let Some(entry) = self.plugins.remove(id) {
            // Remove from type-specific list
            match entry.kind {
                PluginKind::OrbStyle => {
                    self.orb_styles.retain(|i| i != id);
                }
                PluginKind::AudioProcessor => {
                    self.audio_processors.retain(|i| i != id);
                }
                PluginKind::PostProcessor => {
                    self.post_processors.retain(|i| i != id);
                }
                PluginKind::Integration => {
                    self.integrations.retain(|i| i != id);
                }
            }

            log::info!("Unregistered plugin: {}", id);
            Some(entry)
        } else {
            None
        }
    }

    /// Get a plugin by ID
    pub fn get(&self, id: &str) -> Option<&PluginEntry> {
        self.plugins.get(id)
    }

    /// Get a mutable reference to a plugin
    pub fn get_mut(&mut self, id: &str) -> Option<&mut PluginEntry> {
        self.plugins.get_mut(id)
    }

    /// Check if a plugin is registered
    pub fn has(&self, id: &str) -> bool {
        self.plugins.contains_key(id)
    }

    /// Get all registered plugins
    pub fn all(&self) -> impl Iterator<Item = &PluginEntry> {
        self.plugins.values()
    }

    /// Get all orb style plugin IDs
    pub fn orb_style_ids(&self) -> &[String] {
        &self.orb_styles
    }

    /// Get all orb style plugins
    pub fn orb_styles(&self) -> impl Iterator<Item = &PluginEntry> {
        self.orb_styles
            .iter()
            .filter_map(|id| self.plugins.get(id))
    }

    /// Get all audio processor plugin IDs
    pub fn audio_processor_ids(&self) -> &[String] {
        &self.audio_processors
    }

    /// Get all post-processor plugin IDs
    pub fn post_processor_ids(&self) -> &[String] {
        &self.post_processors
    }

    /// Get all integration plugin IDs
    pub fn integration_ids(&self) -> &[String] {
        &self.integrations
    }

    /// Get plugin count
    pub fn count(&self) -> usize {
        self.plugins.len()
    }

    /// Get count by type
    pub fn count_by_type(&self, kind: PluginKind) -> usize {
        match kind {
            PluginKind::OrbStyle => self.orb_styles.len(),
            PluginKind::AudioProcessor => self.audio_processors.len(),
            PluginKind::PostProcessor => self.post_processors.len(),
            PluginKind::Integration => self.integrations.len(),
        }
    }

    /// Enable a plugin
    pub fn enable(&mut self, id: &str) -> bool {
        if let Some(entry) = self.plugins.get_mut(id) {
            entry.enabled = true;
            log::info!("Enabled plugin: {}", id);
            true
        } else {
            false
        }
    }

    /// Disable a plugin
    pub fn disable(&mut self, id: &str) -> bool {
        if let Some(entry) = self.plugins.get_mut(id) {
            entry.enabled = false;
            log::info!("Disabled plugin: {}", id);
            true
        } else {
            false
        }
    }

    /// Check if a plugin is enabled
    pub fn is_enabled(&self, id: &str) -> bool {
        self.plugins
            .get(id)
            .map(|e| e.enabled)
            .unwrap_or(false)
    }

    /// Get list of all plugin manifests
    pub fn manifests(&self) -> Vec<&PluginManifest> {
        self.plugins.values().map(|e| &e.manifest).collect()
    }

    /// Clear all plugins (mainly for testing)
    pub fn clear(&mut self) {
        self.plugins.clear();
        self.orb_styles.clear();
        self.audio_processors.clear();
        self.post_processors.clear();
        self.integrations.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full tests require mock Plugin implementations
    // which would need additional setup

    #[test]
    fn test_registry_creation() {
        let registry = PluginRegistry::new();
        assert_eq!(registry.count(), 0);
    }
}
