# Hook Handler Guide

## Overview

The Zana hook system provides a powerful event-driven architecture that allows plugins and system components to intercept, modify, and respond to application events. The system consists of three core components:

- **EventBus**: Central hub that routes events to registered handlers
- **HookHandler**: Trait interface for event handlers
- **HookEvent**: Enum of all events that flow through the system

The event flow follows this pipeline:

```
Event Source -> EventBus -> Handler 1 (priority 0) -> Handler 2 (priority 10) -> ... -> Subscribers
```

Handlers are called in priority order (lowest first). Each handler can:
- Continue (pass event to next handler)
- Stop (terminate event propagation)
- Modify (change event data before passing to next handler)
- Skip (decline to process this event)

## Creating Handlers

### Basic Handler Implementation

Implement the `HookHandler` trait to create a custom handler:

```rust
use Zana::hooks::{HookHandler, HookEvent, HookEventType, HookResult};
use async_trait::async_trait;

#[derive(Debug)]
struct MyHandler;

#[async_trait]
impl HookHandler for MyHandler {
    fn id(&self) -> &str {
        "my-handler"
    }

    fn subscribed_events(&self) -> Vec<HookEventType> {
        vec![HookEventType::AudioLevelChange]
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        if let HookEvent::AudioLevelChange { level, peak } = event {
            println!("Audio level: {}, peak: {}", level, peak);
        }
        HookResult::Continue
    }
}
```

### Required Trait Methods

- **id()**: Unique identifier for registration/unregistration
- **subscribed_events()**: List of event types to receive (use `HookEventType::All` for all events)
- **handle()**: Process the event, return a `HookResult`

### Optional Trait Methods

- **name()**: Human-readable name (defaults to id)
- **priority()**: Execution order (default: 100, lower runs first)
- **is_enabled()**: Whether handler is active (default: true)
- **on_register()**: Called when handler is registered
- **on_unregister()**: Called when handler is unregistered

### Function-Based Handlers

For simple cases, use `FnHookHandler`:

```rust
use Zana::hooks::handler::FnHookHandler;

let handler = FnHookHandler::new(
    "quick-handler",
    vec![HookEventType::TranscriptionComplete],
    |event| {
        if let HookEvent::TranscriptionComplete { text, .. } = event {
            println!("Transcription: {}", text);
        }
        HookResult::Continue
    }
)
.with_priority(50)
.with_name("Quick Handler");
```

## Registration

### Registering with EventBus

```rust
use std::sync::Arc;

// Get event bus from app state
let event_bus = &state.event_bus;

// Register handler
let handler = Arc::new(MyHandler);
event_bus.register(handler).await?;

// Handler receives events matching its subscriptions
```

### Unregistering

```rust
// Remove handler by ID
event_bus.unregister("my-handler").await?;
```

### Handler Lifecycle

When registered:
1. Handler's `on_register()` method is called
2. Handler is added to priority-sorted list
3. Handler begins receiving events

When unregistered:
1. Handler's `on_unregister()` method is called
2. Handler is removed from list
3. Handler stops receiving events

## Priority System

Priority determines execution order. Lower values execute first.

### Standard Priority Ranges

- **0-50**: System handlers (logging, validation, metrics)
- **100-200**: Plugin handlers (default: 100)
- **201+**: Low-priority handlers (analytics, monitoring)

### Example Priority Setup

```rust
struct SystemLogger;  // Runs first
impl HookHandler for SystemLogger {
    fn priority(&self) -> i32 { 0 }
    // ...
}

struct ValidationHandler;  // Runs second
impl HookHandler for ValidationHandler {
    fn priority(&self) -> i32 { 10 }
    // ...
}

struct PluginHandler;  // Runs third
impl HookHandler for PluginHandler {
    fn priority(&self) -> i32 { 100 }  // default
    // ...
}

struct AnalyticsHandler;  // Runs last
impl HookHandler for AnalyticsHandler {
    fn priority(&self) -> i32 { 200 }
    // ...
}
```

### When Handlers Have Same Priority

Handlers with equal priority are executed in registration order (first registered, first executed).

## Event Types

### Audio Events

- **AudioCaptureStart**: Audio capture began
  ```rust
  HookEvent::AudioCaptureStart { device_id, sample_rate, channels }
  ```

- **AudioCaptureStop**: Audio capture ended
  ```rust
  HookEvent::AudioCaptureStop { duration_ms }
  ```

- **AudioLevelChange**: Audio level update (frequent during recording)
  ```rust
  HookEvent::AudioLevelChange { level, peak }  // 0.0 - 1.0
  ```

- **AudioFftReady**: FFT data available for visualization
  ```rust
  HookEvent::AudioFftReady { bins, bin_count }
  ```

- **AudioBufferReady**: Raw audio buffer ready
  ```rust
  HookEvent::AudioBufferReady { sample_count, sample_rate, channels }
  ```

### Transcription Events

- **TranscriptionStart**: Transcription processing began
  ```rust
  HookEvent::TranscriptionStart { model, audio_duration_ms }
  ```

- **TranscriptionProgress**: Progress update (0.0 - 100.0)
  ```rust
  HookEvent::TranscriptionProgress { percent }
  ```

- **TranscriptionSegment**: Partial text segment available
  ```rust
  HookEvent::TranscriptionSegment { start_ms, end_ms, text }
  ```

- **TranscriptionComplete**: Full transcription finished
  ```rust
  HookEvent::TranscriptionComplete { text, segments, processing_ms }
  ```

- **TranscriptionError**: Transcription failed
  ```rust
  HookEvent::TranscriptionError { error }
  ```

### Plugin Events

- **PluginLoaded**: Plugin was loaded
  ```rust
  HookEvent::PluginLoaded { id, name, version, plugin_type }
  ```

- **PluginUnloaded**: Plugin was unloaded
  ```rust
  HookEvent::PluginUnloaded { id }
  ```

- **PluginError**: Plugin encountered an error
  ```rust
  HookEvent::PluginError { id, error }
  ```

- **PluginConfigChanged**: Plugin configuration changed
  ```rust
  HookEvent::PluginConfigChanged { id, key, value }
  ```

- **PluginEnabled**: Plugin was enabled
  ```rust
  HookEvent::PluginEnabled { id }
  ```

- **PluginDisabled**: Plugin was disabled
  ```rust
  HookEvent::PluginDisabled { id }
  ```

### UI Events

- **OrbStyleChanged**: Orb visualization style changed
  ```rust
  HookEvent::OrbStyleChanged { previous_style, new_style }
  ```

- **ThemeChanged**: Application theme changed
  ```rust
  HookEvent::ThemeChanged { theme }  // Light, Dark, System, Custom
  ```

- **WindowResized**: Window was resized
  ```rust
  HookEvent::WindowResized { width, height }
  ```

- **RecordButtonPressed**: Recording button clicked
  ```rust
  HookEvent::RecordButtonPressed
  ```

- **SettingsOpened**: Settings panel opened
  ```rust
  HookEvent::SettingsOpened
  ```

- **SettingsClosed**: Settings panel closed
  ```rust
  HookEvent::SettingsClosed
  ```

### Settings Events

- **SettingChanged**: A setting was modified
  ```rust
  HookEvent::SettingChanged { key, old_value, new_value }
  ```

- **ProfileChanged**: Transcription profile changed
  ```rust
  HookEvent::ProfileChanged { previous_profile, new_profile }
  ```

- **ModelChanged**: Whisper model changed
  ```rust
  HookEvent::ModelChanged { previous_model, new_model }
  ```

### System Events

- **AppStarted**: Application initialized
  ```rust
  HookEvent::AppStarted
  ```

- **AppShutdown**: Application is shutting down
  ```rust
  HookEvent::AppShutdown
  ```

- **Error**: Application error occurred
  ```rust
  HookEvent::Error { code, message }
  ```

## Examples

### Example 1: Logging Handler

Logs all events at DEBUG level:

```rust
use Zana::hooks::{HookHandler, HookEvent, HookEventType, HookResult};
use async_trait::async_trait;

#[derive(Debug)]
struct LoggingHandler;

#[async_trait]
impl HookHandler for LoggingHandler {
    fn id(&self) -> &str {
        "system-logger"
    }

    fn name(&self) -> &str {
        "System Event Logger"
    }

    fn priority(&self) -> i32 {
        0  // Run first
    }

    fn subscribed_events(&self) -> Vec<HookEventType> {
        vec![HookEventType::All]  // Log everything
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        log::debug!("Event: {:?}", event);
        HookResult::Continue
    }
}
```

### Example 2: Audio Level Filter

Stops propagation if audio level is too low:

```rust
#[derive(Debug)]
struct AudioGateHandler {
    threshold: f32,
}

#[async_trait]
impl HookHandler for AudioGateHandler {
    fn id(&self) -> &str {
        "audio-gate"
    }

    fn subscribed_events(&self) -> Vec<HookEventType> {
        vec![HookEventType::AudioLevelChange]
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        if let HookEvent::AudioLevelChange { level, .. } = event {
            if *level < self.threshold {
                log::debug!("Audio below threshold, skipping");
                return HookResult::Skip;
            }
        }
        HookResult::Continue
    }
}
```

### Example 3: Text Post-Processor

Modifies transcription text:

```rust
#[derive(Debug)]
struct TextCleanerHandler;

#[async_trait]
impl HookHandler for TextCleanerHandler {
    fn id(&self) -> &str {
        "text-cleaner"
    }

    fn subscribed_events(&self) -> Vec<HookEventType> {
        vec![HookEventType::TranscriptionComplete]
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        if let HookEvent::TranscriptionComplete { text, .. } = event {
            // Remove extra whitespace
            *text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            return HookResult::Modified;
        }
        HookResult::Continue
    }
}
```

### Example 4: Metrics Collector

Tracks event counts and timing:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
struct MetricsHandler {
    event_counts: Arc<Mutex<HashMap<String, u64>>>,
}

#[async_trait]
impl HookHandler for MetricsHandler {
    fn id(&self) -> &str {
        "metrics-collector"
    }

    fn priority(&self) -> i32 {
        50
    }

    fn subscribed_events(&self) -> Vec<HookEventType> {
        vec![HookEventType::All]
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        let event_type = format!("{:?}", event.event_type());
        let mut counts = self.event_counts.lock().await;
        *counts.entry(event_type).or_insert(0) += 1;
        HookResult::Continue
    }
}

impl MetricsHandler {
    async fn get_counts(&self) -> HashMap<String, u64> {
        self.event_counts.lock().await.clone()
    }
}
```

### Example 5: Conditional Handler

Handler that can be enabled/disabled:

```rust
#[derive(Debug)]
struct ConditionalHandler {
    enabled: std::sync::atomic::AtomicBool,
}

impl ConditionalHandler {
    fn enable(&self) {
        self.enabled.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn disable(&self) {
        self.enabled.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

#[async_trait]
impl HookHandler for ConditionalHandler {
    fn id(&self) -> &str {
        "conditional-handler"
    }

    fn subscribed_events(&self) -> Vec<HookEventType> {
        vec![HookEventType::TranscriptionSegment]
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(std::sync::atomic::Ordering::SeqCst)
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        // Only processes when enabled
        if let HookEvent::TranscriptionSegment { text, .. } = event {
            println!("Processing segment: {}", text);
        }
        HookResult::Continue
    }
}
```

## Plugin Integration

### Plugin Hook Handler Pattern

Plugins integrate with the hook system through the `hook_handler()` method:

```rust
use Zana::plugins::{Plugin, PluginContext, PluginManifest};
use Zana::hooks::{HookHandler, HookEvent, HookEventType, HookResult};
use async_trait::async_trait;

#[derive(Debug)]
struct MyPlugin {
    manifest: PluginManifest,
}

#[async_trait]
impl Plugin for MyPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn init(&mut self, _context: PluginContext) -> anyhow::Result<()> {
        // Initialization logic
        Ok(())
    }

    async fn shutdown(&mut self) -> anyhow::Result<()> {
        // Cleanup logic
        Ok(())
    }

    fn on_config_change(&mut self, _config: &HashMap<String, JsonValue>) {
        // Handle config changes
    }

    // Return the plugin's hook handler
    fn hook_handler(&self) -> Option<Arc<dyn HookHandler>> {
        Some(Arc::new(MyPluginHandler::new()))
    }
}

#[derive(Debug, Clone)]
struct MyPluginHandler {
    // Plugin handler state
}

impl MyPluginHandler {
    fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl HookHandler for MyPluginHandler {
    fn id(&self) -> &str {
        "my-plugin-handler"
    }

    fn name(&self) -> &str {
        "My Plugin Hook Handler"
    }

    fn priority(&self) -> i32 {
        100  // Standard plugin priority
    }

    fn subscribed_events(&self) -> Vec<HookEventType> {
        vec![
            HookEventType::TranscriptionComplete,
            HookEventType::AudioLevelChange,
        ]
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        match event {
            HookEvent::TranscriptionComplete { text, .. } => {
                // Process transcription
                log::info!("Plugin received: {}", text);
                HookResult::Continue
            }
            HookEvent::AudioLevelChange { level, .. } => {
                // React to audio changes
                if *level > 0.8 {
                    log::warn!("High audio level detected!");
                }
                HookResult::Continue
            }
            _ => HookResult::Skip,
        }
    }
}
```

### Plugin Manager Registration

The `PluginManager` automatically registers plugin handlers:

```rust
// In PluginManager::load_plugin()
async fn load_plugin(&self, path: &Path) -> anyhow::Result<()> {
    // Load and initialize plugin
    let mut plugin = load_plugin_from_path(path)?;
    plugin.init(context).await?;

    // Get handler if plugin provides one
    if let Some(handler) = plugin.hook_handler() {
        // Register with event bus
        self.event_bus.register(handler).await?;

        // Store handler ID for unregistration later
        self.handler_ids
            .write()
            .await
            .insert(plugin.manifest().id.clone(), handler.id().to_string());
    }

    // Store plugin in registry
    self.registry.write().await.register(plugin);
    Ok(())
}
```

### Plugin-Specific Handler Patterns

#### Audio Visualization Plugin

```rust
struct VisualizerHandler {
    peak_level: Arc<Mutex<f32>>,
}

#[async_trait]
impl HookHandler for VisualizerHandler {
    fn subscribed_events(&self) -> Vec<HookEventType> {
        vec![HookEventType::AudioLevelChange, HookEventType::AudioFftReady]
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        match event {
            HookEvent::AudioLevelChange { peak, .. } => {
                *self.peak_level.lock().await = *peak;
            }
            HookEvent::AudioFftReady { bins, .. } => {
                // Update visualization with FFT data
            }
            _ => {}
        }
        HookResult::Continue
    }
}
```

#### Transcription Enhancer Plugin

```rust
struct EnhancerHandler;

#[async_trait]
impl HookHandler for EnhancerHandler {
    fn subscribed_events(&self) -> Vec<HookEventType> {
        vec![HookEventType::TranscriptionComplete]
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        if let HookEvent::TranscriptionComplete { text, .. } = event {
            // Add punctuation, capitalize, etc.
            *text = enhance_text(text);
            return HookResult::Modified;
        }
        HookResult::Continue
    }
}
```

#### Integration Plugin (External Service)

```rust
struct IntegrationHandler {
    api_client: reqwest::Client,
    endpoint: String,
}

#[async_trait]
impl HookHandler for IntegrationHandler {
    fn subscribed_events(&self) -> Vec<HookEventType> {
        vec![HookEventType::TranscriptionComplete]
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        if let HookEvent::TranscriptionComplete { text, .. } = event {
            // Send to external service
            let _ = self.api_client
                .post(&self.endpoint)
                .json(&serde_json::json!({"text": text}))
                .send()
                .await;

            // Don't block other handlers even if API fails
            HookResult::Continue
        } else {
            HookResult::Skip
        }
    }
}
```

## Best Practices

1. **Choose Appropriate Priority**: System handlers (0-50), plugins (100-200)
2. **Return Correct Result**: Use `Continue` for most cases, `Stop` to halt propagation
3. **Handle Errors Gracefully**: Don't panic in handlers, log errors and continue
4. **Avoid Blocking I/O**: Use async operations, never block the event loop
5. **Be Idempotent**: Handlers should produce same result if called multiple times
6. **Document Side Effects**: Clearly document what handlers modify
7. **Use Specific Subscriptions**: Subscribe to specific events instead of `All` when possible
8. **Test Thoroughly**: Test handlers in isolation and in combination
9. **Handle Missing Data**: Use pattern matching with fallback to handle unexpected variants
10. **Clean Up Resources**: Use `on_unregister()` to release resources

## Debugging

### Enable Debug Logging

```rust
// In handler
log::debug!("Handler {} processing event {:?}", self.id(), event);
```

### List Registered Handlers

```rust
let ids = event_bus.handler_ids().await;
println!("Registered handlers: {:?}", ids);
```

### Get Event Bus Statistics

```rust
let stats = event_bus.stats().await;
println!("Events emitted: {}", stats.events_emitted);
println!("Events handled: {}", stats.events_handled);
println!("Events stopped: {}", stats.events_stopped);
println!("Handler errors: {}", stats.handler_errors);
```

### Subscribe to Events for Debugging

```rust
let mut rx = event_bus.subscribe_all();
tokio::spawn(async move {
    while let Ok(event) = rx.recv().await {
        println!("Received event: {:?}", event);
    }
});
```

## See Also

- `src-tauri/src/handler.rs` - HookHandler trait definition
- `src-tauri/src/event.rs` - Event type definitions
- `src-tauri/src/bus.rs` - EventBus implementation
- `src-tauri/src/plugins/traits.rs` - Plugin trait with hook_handler() method
