# Zana Plugin & Hook Integration Spec

> Historical note: this document is an implementation planning artifact. It may
> describe deleted, renamed, or completed work. Use `src-tauri/` and
> `docs/README.md` as the current source of truth.

## Overview
Complete the integration of three partially-implemented systems in Zana to enable full plugin functionality.

## Current State Analysis

### 1. marketplace/ - COMPLETELY DEAD (TO BE DELETED)
- Status: Not in lib.rs, references non-existent client.rs
- Action: DELETE ENTIRELY
- Files: src-tauri/src/marketplace/{mod.rs, types.rs}

### 2. plugins/ - SCAFFOLDING ONLY (NEEDS INTEGRATION)
- Status: Fully defined but never instantiated
- Files exist: traits.rs, registry.rs, manifest.rs, gpu.rs
- Missing: Integration into AppState, loading logic, Tauri commands
- Action: INTEGRATE

### 3. hooks/handler.rs - PARTIALLY UNUSED (NEEDS WIRING)
- Status: HookHandler trait defined, EventBus.register() exists
- Problem: register() NEVER called outside tests
- FnHookHandler, HookHandlerBuilder unused
- Action: WIRE UP

## Architecture Goals

```
AppState
├── event_bus: Arc<EventBus>
├── plugin_registry: Arc<RwLock<PluginRegistry>>   [NEW]
└── plugin_manager: Arc<Mutex<PluginManager>>      [NEW]
```

## Phase A: Foundation (Run in Parallel)

### Task A1: Delete Marketplace
**Agent: PhaseA_DeleteMarketplace**
- Remove src-tauri/src/marketplace/ directory entirely
- Verify no references in codebase (grep for "marketplace")
- Update Cargo.toml if needed

### Task A2: Create PluginManager
**Agent: PhaseA_CreatePluginManager**
- File: src-tauri/src/plugins/manager.rs
- Struct: PluginManager
  - Methods:
    - `new(registry: Arc<RwLock<PluginRegistry>>, event_bus: Arc<EventBus>, plugins_dir: PathBuf) -> Self`
    - `load_all() -> Result<usize>` - discover and load all plugins
    - `load_plugin(path: &Path) -> Result<()>` - load single plugin
    - `unload_plugin(id: &str) -> Result<()>`
    - `reload_plugin(id: &str) -> Result<()>`
    - `get_manifests() -> Vec<PluginManifest>`
- Read plugin.toml files
- Parse manifests using PluginManifest::from_toml()
- Register with PluginRegistry
- Emit HookEvent::PluginLoaded after successful load

### Task A3: Update AppState
**Agent: PhaseA_UpdateAppState**
- File: src-tauri/src/state.rs
- Add fields:
  ```rust
  pub plugin_registry: Arc<RwLock<PluginRegistry>>,
  pub plugin_manager: Arc<Mutex<PluginManager>>,
  ```
- Initialize in AppState::new():
  - Create PluginRegistry
  - Determine plugins directory (app_data/plugins or dev plugins/)
  - Create PluginManager
  - Call plugin_manager.load_all()
  - Log loaded plugin count

## Phase B: Plugin Commands (Run in Parallel)

### Task B1: Plugin List Command
**Agent: PhaseB_PluginListCommand**
- File: src-tauri/src/commands/plugins.rs (NEW)
- Command: `list_plugins(state: State<AppState>) -> Result<Vec<PluginManifest>>`
- Returns manifests from plugin_registry
- Includes: id, name, version, description, author, kind, capabilities

### Task B2: Plugin Enable/Disable Commands
**Agent: PhaseB_PluginToggleCommands**
- File: src-tauri/src/commands/plugins.rs
- Commands:
  - `enable_plugin(id: String, state: State<AppState>) -> Result<()>`
  - `disable_plugin(id: String, state: State<AppState>) -> Result<()>`
- Use PluginRegistry::enable() / disable()
- Emit HookEvent::PluginEnabled / PluginDisabled

### Task B3: Plugin Install/Uninstall Commands
**Agent: PhaseB_PluginInstallCommands**
- File: src-tauri/src/commands/plugins.rs
- Commands:
  - `install_plugin(path: String, state: State<AppState>) -> Result<()>`
  - `uninstall_plugin(id: String, state: State<AppState>) -> Result<()>`
- install_plugin: copy directory to plugins/, call load_plugin()
- uninstall_plugin: call unload_plugin(), remove directory
- Emit HookEvent::PluginInstalled / PluginUninstalled

### Task B4: Register Plugin Commands
**Agent: PhaseB_RegisterCommands**
- File: src-tauri/src/commands/mod.rs
- Add: `pub mod plugins;`
- Re-export all plugin commands
- File: src-tauri/src/main.rs (or lib.rs)
- Add plugin commands to Tauri invoke handler:
  ```rust
  .invoke_handler(tauri::generate_handler![
      // ... existing commands
      list_plugins,
      enable_plugin,
      disable_plugin,
      install_plugin,
      uninstall_plugin,
  ])
  ```

## Phase C: Hook Handler Integration (Run in Parallel)

### Task C1: Plugin Hook Adapter
**Agent: PhaseC_PluginHookAdapter**
- File: src-tauri/src/plugins/hook_adapter.rs (NEW)
- Struct: PluginHookAdapter (implements HookHandler)
  - Wraps a Plugin instance
  - Delegates handle() to plugin's lifecycle hooks
  - Maps HookEvents to plugin callbacks
  - Example: AudioLevelChange -> plugin.on_audio_level()
- Purpose: Bridge Plugin trait to HookHandler trait
- Used by PluginManager to auto-register plugins as handlers

### Task C2: Update PluginManager Registration
**Agent: PhaseC_PluginManagerRegistration**
- File: src-tauri/src/plugins/manager.rs
- Update load_plugin():
  - After creating plugin instance
  - Wrap in PluginHookAdapter
  - Call event_bus.register(Arc::new(adapter))
  - Store handler ID for later unregister
- Update unload_plugin():
  - Call event_bus.unregister(handler_id)
  - Then remove from registry

### Task C3: Example System Handlers
**Agent: PhaseC_ExampleHandlers**
- File: src-tauri/src/hooks/system_handlers.rs (NEW)
- Create example handlers to demonstrate usage:
  1. **LoggingHandler**: Logs all events at DEBUG level
     - Priority: 0 (runs first)
     - Subscriptions: [HookEventType::All]
  2. **MetricsHandler**: Tracks event counts/timing
     - Priority: 0
     - Updates internal counters
  3. **ValidationHandler**: Validates event data
     - Priority: 10
     - Returns Skip for invalid events
- Register in AppState::new() via event_bus.register()

### Task C4: Document Hook Handler Usage
**Agent: PhaseC_DocumentHandlers**
- File: docs/HOOK_HANDLER_GUIDE.md (NEW)
- Sections:
  - Overview: Hook system architecture
  - Creating Handlers: Implement HookHandler trait
  - Registration: How to register with EventBus
  - Priority System: Execution order (0-50 system, 100-200 plugins)
  - Event Types: All available HookEventType variants
  - Examples: LoggingHandler, custom handlers
  - Plugin Integration: How plugins become handlers

## Phase D: Testing & Verification (Sequential after A, B, C)

### Task D1: Integration Tests
**Agent: PhaseD_IntegrationTests**
- File: src-tauri/src/plugins/tests/integration.rs (NEW)
- Tests:
  - `test_plugin_loading()`: Load plugins/, verify count
  - `test_plugin_enable_disable()`: Toggle plugin state
  - `test_plugin_hook_integration()`: Verify handlers registered
  - `test_event_propagation()`: Emit event, verify plugin receives
  - `test_plugin_unload()`: Unload, verify handler unregistered

### Task D2: Command Tests
**Agent: PhaseD_CommandTests**
- File: src-tauri/src/commands/tests/plugins.rs (NEW)
- Test each command:
  - list_plugins
  - enable_plugin / disable_plugin
  - install_plugin / uninstall_plugin
- Use mock AppState with test plugins

### Task D3: Manual Verification
**Agent: PhaseD_ManualVerification**
- Create checklist: docs/INTEGRATION_CHECKLIST.md
- Items:
  - [ ] App starts without errors
  - [ ] Plugins loaded (check logs)
  - [ ] list_plugins returns nebula-aura, nebula-aura-gpu
  - [ ] enable_plugin/disable_plugin work
  - [ ] Event emission triggers plugin handlers
  - [ ] Unload removes handlers
  - [ ] No marketplace references remain

## Success Criteria

1. **Marketplace Removed**: No traces of marketplace/ in codebase
2. **Plugins Loaded**: AppState contains PluginRegistry + PluginManager
3. **Commands Work**: All 5 plugin commands functional
4. **Hooks Integrated**: Plugins auto-register as HookHandlers
5. **Tests Pass**: All integration and command tests pass
6. **Documentation**: Hook handler guide complete
7. **Example Handlers**: System handlers demonstrate usage

## File Modifications Summary

### New Files:
- src-tauri/src/plugins/manager.rs
- src-tauri/src/plugins/hook_adapter.rs
- src-tauri/src/commands/plugins.rs
- src-tauri/src/hooks/system_handlers.rs
- docs/HOOK_HANDLER_GUIDE.md
- docs/INTEGRATION_CHECKLIST.md
- src-tauri/src/plugins/tests/integration.rs
- src-tauri/src/commands/tests/plugins.rs

### Modified Files:
- src-tauri/src/state.rs (add plugin fields)
- src-tauri/src/commands/mod.rs (add plugins module)
- src-tauri/src/main.rs or lib.rs (register commands)
- src-tauri/src/lib.rs (remove marketplace, add manager)

### Deleted:
- src-tauri/src/marketplace/ (entire directory)

## Dependencies
- Phase A: No dependencies (run in parallel)
- Phase B: Depends on A (PluginManager must exist)
- Phase C: Depends on A (PluginManager must exist)
- Phase D: Depends on A, B, C (all code complete)

## Notes
- Use existing plugin.toml parser from manifest.rs
- Plugins directory: $APP_DATA/plugins in production, ./plugins in dev
- HookHandler priority: 0-50 system, 100-200 plugins
- EventBus already has register()/unregister() - just need to call them
- PluginRegistry already has enable()/disable() - just expose via commands
