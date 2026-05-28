//! System Hook Handlers
//!
//! Built-in handlers that provide core functionality for the Zana hook system.
//! These handlers are automatically registered during application startup.

use super::event::{HookEvent, HookEventType};
use super::{HookHandler, HookResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Logging Handler - Logs all events at DEBUG level
///
/// This handler runs first (priority 0) and logs all events for debugging
/// and monitoring purposes.
#[derive(Debug)]
pub struct LoggingHandler;

#[async_trait]
impl HookHandler for LoggingHandler {
    fn id(&self) -> &str {
        "logging-handler"
    }

    fn name(&self) -> &str {
        "System Logging Handler"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn subscribed_events(&self) -> Vec<HookEventType> {
        vec![HookEventType::All]
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        log::debug!("[HookHandler] {:?}", event);
        HookResult::Continue
    }
}

/// Metrics Handler - Tracks event counts and timing
///
/// Collects metrics about events flowing through the system, including
/// total counts per event type and timing information.
#[derive(Debug)]
pub struct MetricsHandler {
    counts: Arc<RwLock<HashMap<HookEventType, u64>>>,
    total_events: Arc<AtomicU64>,
}

impl MetricsHandler {
    /// Create a new metrics handler
    pub fn new() -> Self {
        Self {
            counts: Arc::new(RwLock::new(HashMap::new())),
            total_events: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Get the count for a specific event type
    pub async fn get_count(&self, event_type: HookEventType) -> u64 {
        let counts = self.counts.read().await;
        counts.get(&event_type).copied().unwrap_or(0)
    }

    /// Get all event counts
    pub async fn get_all_counts(&self) -> HashMap<HookEventType, u64> {
        self.counts.read().await.clone()
    }

    /// Get total number of events processed
    pub fn get_total_count(&self) -> u64 {
        self.total_events.load(Ordering::Relaxed)
    }

    /// Reset all metrics
    pub async fn reset(&self) {
        let mut counts = self.counts.write().await;
        counts.clear();
        self.total_events.store(0, Ordering::Relaxed);
    }
}

impl Default for MetricsHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HookHandler for MetricsHandler {
    fn id(&self) -> &str {
        "metrics-handler"
    }

    fn name(&self) -> &str {
        "System Metrics Handler"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn subscribed_events(&self) -> Vec<HookEventType> {
        vec![HookEventType::All]
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        // Increment total counter
        self.total_events.fetch_add(1, Ordering::Relaxed);

        // Increment per-event-type counter
        let event_type = event.event_type();
        let mut counts = self.counts.write().await;
        *counts.entry(event_type).or_insert(0) += 1;

        HookResult::Continue
    }
}

/// Validation Handler - Validates event data integrity
///
/// Validates that events contain proper data structure and required fields.
/// Returns Skip for events that fail validation, preventing them from
/// reaching other handlers.
#[derive(Debug)]
pub struct ValidationHandler {
    strict_mode: bool,
}

impl ValidationHandler {
    /// Create a new validation handler
    pub fn new() -> Self {
        Self { strict_mode: false }
    }

    /// Create a validation handler in strict mode
    ///
    /// In strict mode, validation failures return Stop instead of Skip,
    /// which halts event propagation entirely.
    pub fn strict() -> Self {
        Self { strict_mode: true }
    }

    /// Validate an event's data structure
    fn validate_event(&self, event: &HookEvent) -> Result<(), String> {
        match event {
            HookEvent::AudioLevelChange { level, peak } => {
                if !(0.0..=1.0).contains(level) {
                    return Err(format!("Invalid audio level: {}", level));
                }
                // Allow peaks up to 10.0 for extreme clipping/hot signals
                // Audio hardware can produce values well above 1.0 with high gain
                if *peak < 0.0 || *peak > 10.0 {
                    return Err(format!("Invalid audio peak: {}", peak));
                }
            }
            HookEvent::TranscriptionProgress { percent } if !(0.0..=100.0).contains(percent) => {
                return Err(format!("Invalid progress percentage: {}", percent));
            }
            HookEvent::TranscriptionSegment {
                start_ms,
                end_ms,
                text,
            } => {
                if *end_ms <= *start_ms {
                    return Err(format!(
                        "Invalid segment time range: start={}, end={}",
                        start_ms, end_ms
                    ));
                }
                if text.trim().is_empty() {
                    return Err("Empty transcription segment text".to_string());
                }
            }
            HookEvent::Error { code, message } => {
                if code.trim().is_empty() {
                    return Err("Empty error code".to_string());
                }
                if message.trim().is_empty() {
                    return Err("Empty error message".to_string());
                }
            }
            HookEvent::PluginLoaded {
                id,
                name,
                version,
                plugin_type: _,
            } => {
                if id.trim().is_empty() {
                    return Err("Empty plugin ID".to_string());
                }
                if name.trim().is_empty() {
                    return Err("Empty plugin name".to_string());
                }
                if version.trim().is_empty() {
                    return Err("Empty plugin version".to_string());
                }
            }
            _ => {
                // Other events don't require validation
            }
        }
        Ok(())
    }
}

impl Default for ValidationHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HookHandler for ValidationHandler {
    fn id(&self) -> &str {
        "validation-handler"
    }

    fn name(&self) -> &str {
        "System Validation Handler"
    }

    fn priority(&self) -> i32 {
        10
    }

    fn subscribed_events(&self) -> Vec<HookEventType> {
        vec![HookEventType::All]
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        match self.validate_event(event) {
            Ok(()) => HookResult::Continue,
            Err(e) => {
                log::warn!("[ValidationHandler] Event validation failed: {}", e);
                if self.strict_mode {
                    HookResult::Stop
                } else {
                    HookResult::Skip
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_logging_handler() {
        let handler = LoggingHandler;
        assert_eq!(handler.id(), "logging-handler");
        assert_eq!(handler.priority(), 0);
        assert_eq!(handler.subscribed_events(), vec![HookEventType::All]);
    }

    #[tokio::test]
    async fn test_metrics_handler() {
        let handler = MetricsHandler::new();

        // Initially zero
        assert_eq!(handler.get_total_count(), 0);

        // Process an event
        let mut event = HookEvent::AppStarted;
        handler.handle(&mut event).await;

        // Count should be incremented
        assert_eq!(handler.get_total_count(), 1);
        assert_eq!(handler.get_count(HookEventType::AppStarted).await, 1);

        // Reset
        handler.reset().await;
        assert_eq!(handler.get_total_count(), 0);
    }

    #[tokio::test]
    async fn test_validation_handler_valid() {
        let handler = ValidationHandler::new();

        let valid_events = vec![
            HookEvent::AudioLevelChange {
                level: 0.5,
                peak: 0.8,
            },
            HookEvent::TranscriptionProgress { percent: 50.0 },
            HookEvent::AppStarted,
        ];

        for mut event in valid_events {
            let result = handler.handle(&mut event).await;
            assert_eq!(result, HookResult::Continue);
        }
    }

    #[tokio::test]
    async fn test_validation_handler_invalid() {
        let handler = ValidationHandler::new();

        let invalid_events = vec![
            HookEvent::AudioLevelChange {
                level: 1.5,
                peak: 0.8,
            },
            HookEvent::TranscriptionProgress { percent: 150.0 },
            HookEvent::Error {
                code: String::new(),
                message: "Test".to_string(),
            },
        ];

        for mut event in invalid_events {
            let result = handler.handle(&mut event).await;
            assert_eq!(result, HookResult::Skip);
        }
    }

    #[tokio::test]
    async fn test_validation_handler_strict_mode() {
        let handler = ValidationHandler::strict();

        let mut event = HookEvent::AudioLevelChange {
            level: 1.5,
            peak: 0.8,
        };

        let result = handler.handle(&mut event).await;
        assert_eq!(result, HookResult::Stop);
    }

    #[tokio::test]
    async fn test_validation_handler_segment() {
        let handler = ValidationHandler::new();

        // Valid segment
        let mut valid_segment = HookEvent::TranscriptionSegment {
            start_ms: 0,
            end_ms: 1000,
            text: "Hello world".to_string(),
        };
        assert_eq!(
            handler.handle(&mut valid_segment).await,
            HookResult::Continue
        );

        // Invalid time range
        let mut invalid_segment = HookEvent::TranscriptionSegment {
            start_ms: 1000,
            end_ms: 500,
            text: "Hello world".to_string(),
        };
        assert_eq!(handler.handle(&mut invalid_segment).await, HookResult::Skip);

        // Empty text
        let mut empty_segment = HookEvent::TranscriptionSegment {
            start_ms: 0,
            end_ms: 1000,
            text: String::new(),
        };
        assert_eq!(handler.handle(&mut empty_segment).await, HookResult::Skip);
    }
}
