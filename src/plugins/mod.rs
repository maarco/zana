//! Plugin System
//!
//! The Zana plugin system enables extensibility through loadable plugins
//! that provide audio visualizations, audio processing, and integrations.
//!
//! # Plugin Types
//!
//! - **Orb Style**: Audio visualization plugins (Canvas 2D or GPU rendering)
//! - **Audio Processor**: Plugins that modify audio before transcription
//! - **Post-Processor**: Plugins that modify transcription output
//! - **Integration**: Plugins that connect to external services
//!
//! # Rendering Modes
//!
//! Orb style plugins can use:
//! - **Canvas 2D**: CPU-based rendering via DrawCommands
//! - **WebGL2**: GPU-accelerated with GLSL shaders
//! - **WebGPU**: Modern GPU API with WGSL shaders (preferred)
//!
//! # Plugin Structure
//!
//! Plugins are distributed as directories containing:
//! - `plugin.toml`: Manifest with metadata and configuration
//! - `src/render.js`: Rendering logic (for Canvas 2D plugins)
//! - `src/renderer.js`: GPU renderer (for WebGPU/WebGL2 plugins)
//! - `src/shaders/`: WGSL/GLSL shader files
//! - `assets/`: Preview images and other assets
//!
//! # Example
//!
//! ```rust
//! use Zana::plugins::{PluginManager, PluginManifest};
//!
//! // Load all plugins from the plugins directory
//! let mut manager = PluginManager::new(event_bus, plugins_dir);
//! manager.load_all().await?;
//!
//! // Get available orb styles
//! let styles = manager.registry.orb_style_ids();
//! ```

mod gpu;
mod loader;
mod manifest;
mod registry;
mod traits;

// Re-exports for public API
pub use loader::PluginManager;
pub use manifest::PluginManifest;
pub use registry::PluginRegistry;
pub use traits::{OrbStylePlugin, Plugin, PluginContext};

