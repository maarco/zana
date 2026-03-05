//! Event Bus
//!
//! Central hub for event routing in Zana.
//! Manages handler registration, event emission, and subscriptions.

use super::event::{HookEvent, HookEventType};
use super::handler::{HookHandler, HookResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Maximum number of events buffered in broadcast channels
const CHANNEL_CAPACITY: usize = 256;

/// The central event bus for Zana
///
/// Routes events through registered handlers and notifies subscribers.
/// Thread-safe and designed for async usage.
#[derive(Debug)]
pub struct EventBus {
    /// Registered handlers, sorted by priority
    handlers: RwLock<Vec<Arc<dyn HookHandler>>>,

    /// Broadcast channels for UI subscriptions
    subscribers: RwLock<HashMap<HookEventType, broadcast::Sender<HookEvent>>>,

    /// Global broadcast channel (receives all events)
    global_sender: broadcast::Sender<HookEvent>,

    /// Statistics
    stats: RwLock<EventBusStats>,
}

/// Statistics about event bus activity
#[derive(Debug, Default, Clone)]
pub struct EventBusStats {
    /// Total events emitted
    pub events_emitted: u64,
    /// Total events handled
    pub events_handled: u64,
    /// Events stopped by handlers
    pub events_stopped: u64,
    /// Events modified by handlers
    pub events_modified: u64,
    /// Handler errors
    pub handler_errors: u64,
}

impl EventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        let (global_sender, _) = broadcast::channel(CHANNEL_CAPACITY);

        Self {
            handlers: RwLock::new(Vec::new()),
            subscribers: RwLock::new(HashMap::new()),
            global_sender,
            stats: RwLock::new(EventBusStats::default()),
        }
    }

    /// Register a hook handler
    ///
    /// The handler will be called for events matching its subscriptions.
    /// Handlers are sorted by priority (lower = earlier).
    pub async fn register(&self, handler: Arc<dyn HookHandler>) -> anyhow::Result<()> {
        // Call handler's on_register hook
        handler.on_register().await?;

        let mut handlers = self.handlers.write().await;

        // Remove existing handler with same ID (if any)
        handlers.retain(|h| h.id() != handler.id());

        // Insert and sort by priority
        handlers.push(handler);
        handlers.sort_by_key(|h| h.priority());

        log::info!(
            "Registered handler: {} (priority: {})",
            handlers.last().unwrap().id(),
            handlers.last().unwrap().priority()
        );

        Ok(())
    }

    /// Unregister a handler by ID
    pub async fn unregister(&self, handler_id: &str) -> anyhow::Result<()> {
        let mut handlers = self.handlers.write().await;

        // Find and call on_unregister
        if let Some(handler) = handlers.iter().find(|h| h.id() == handler_id) {
            handler.on_unregister().await?;
        }

        // Remove from list
        let initial_len = handlers.len();
        handlers.retain(|h| h.id() != handler_id);

        if handlers.len() < initial_len {
            log::info!("Unregistered handler: {}", handler_id);
        }

        Ok(())
    }

    /// Emit an event through the hook pipeline
    ///
    /// The event passes through all matching handlers in priority order.
    /// Handlers can modify the event or stop propagation.
    ///
    /// Returns the (possibly modified) event after all handlers have processed it.
    pub async fn emit(&self, mut event: HookEvent) -> HookEvent {
        let event_type = event.event_type();

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.events_emitted += 1;
        }

        // Get handlers snapshot (to avoid holding lock during async calls)
        let handlers: Vec<Arc<dyn HookHandler>> = {
            let handlers = self.handlers.read().await;
            handlers.clone()
        };

        // Process through handlers
        for handler in handlers {
            // Skip disabled handlers
            if !handler.is_enabled() {
                continue;
            }

            // Check if handler subscribes to this event type
            let subscriptions = handler.subscribed_events();
            if !subscriptions.contains(&event_type) && !subscriptions.contains(&HookEventType::All)
            {
                continue;
            }

            // Call handler
            match handler.handle(&mut event).await {
                HookResult::Continue => {
                    let mut stats = self.stats.write().await;
                    stats.events_handled += 1;
                }
                HookResult::Stop => {
                    let mut stats = self.stats.write().await;
                    stats.events_handled += 1;
                    stats.events_stopped += 1;
                    log::debug!(
                        "Event {:?} stopped by handler {}",
                        event_type,
                        handler.id()
                    );
                    break;
                }
                HookResult::Modified => {
                    let mut stats = self.stats.write().await;
                    stats.events_handled += 1;
                    stats.events_modified += 1;
                    log::debug!(
                        "Event {:?} modified by handler {}",
                        event_type,
                        handler.id()
                    );
                }
                HookResult::Skip => {
                    // Handler declined to process
                }
            }
        }

        // Broadcast to subscribers
        self.broadcast(&event, event_type).await;

        event
    }

    /// Broadcast event to subscribers
    async fn broadcast(&self, event: &HookEvent, event_type: HookEventType) {
        // Send to global channel
        let _ = self.global_sender.send(event.clone());

        // Send to type-specific channels
        let subscribers = self.subscribers.read().await;
        if let Some(sender) = subscribers.get(&event_type) {
            let _ = sender.send(event.clone());
        }
    }

    /// Subscribe to all events
    ///
    /// Returns a receiver that will get all events.
    pub fn subscribe_all(&self) -> broadcast::Receiver<HookEvent> {
        self.global_sender.subscribe()
    }

    /// Subscribe to a specific event type
    ///
    /// Returns a receiver that will only get events of the specified type.
    pub async fn subscribe(&self, event_type: HookEventType) -> broadcast::Receiver<HookEvent> {
        let mut subscribers = self.subscribers.write().await;

        // Get or create channel for this event type
        let sender = subscribers.entry(event_type).or_insert_with(|| {
            let (sender, _) = broadcast::channel(CHANNEL_CAPACITY);
            sender
        });

        sender.subscribe()
    }

    /// Get current statistics
    pub async fn stats(&self) -> EventBusStats {
        self.stats.read().await.clone()
    }

    /// Get count of registered handlers
    pub async fn handler_count(&self) -> usize {
        self.handlers.read().await.len()
    }

    /// Get list of registered handler IDs
    pub async fn handler_ids(&self) -> Vec<String> {
        self.handlers
            .read()
            .await
            .iter()
            .map(|h| h.id().to_string())
            .collect()
    }

    /// Clear all handlers (mainly for testing)
    pub async fn clear(&self) {
        let mut handlers = self.handlers.write().await;

        // Call on_unregister for all handlers
        for handler in handlers.iter() {
            let _ = handler.on_unregister().await;
        }

        handlers.clear();
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::handler::FnHookHandler;

    #[tokio::test]
    async fn test_register_handler() {
        let bus = EventBus::new();

        let handler = Arc::new(FnHookHandler::new(
            "test",
            vec![HookEventType::All],
            |_| HookResult::Continue,
        ));

        bus.register(handler).await.unwrap();
        assert_eq!(bus.handler_count().await, 1);
    }

    #[tokio::test]
    async fn test_emit_event() {
        let bus = EventBus::new();

        let event = HookEvent::AudioLevelChange {
            level: 0.5,
            peak: 0.8,
        };

        let result = bus.emit(event).await;

        // Event should pass through unchanged (no handlers)
        if let HookEvent::AudioLevelChange { level, peak } = result {
            assert_eq!(level, 0.5);
            assert_eq!(peak, 0.8);
        } else {
            panic!("Event type changed unexpectedly");
        }
    }

    #[tokio::test]
    async fn test_handler_priority() {
        let bus = EventBus::new();

        // Register handlers with different priorities
        let h1 = Arc::new(
            FnHookHandler::new("low-priority", vec![HookEventType::All], |_| {
                HookResult::Continue
            })
            .with_priority(200),
        );

        let h2 = Arc::new(
            FnHookHandler::new("high-priority", vec![HookEventType::All], |_| {
                HookResult::Continue
            })
            .with_priority(50),
        );

        bus.register(h1).await.unwrap();
        bus.register(h2).await.unwrap();

        // High priority handler should be first
        let ids = bus.handler_ids().await;
        assert_eq!(ids[0], "high-priority");
        assert_eq!(ids[1], "low-priority");
    }

    #[tokio::test]
    async fn test_subscription() {
        let bus = EventBus::new();

        // Subscribe before emitting
        let mut rx = bus.subscribe(HookEventType::AudioLevelChange).await;

        // Emit event
        bus.emit(HookEvent::AudioLevelChange {
            level: 0.5,
            peak: 0.8,
        })
        .await;

        // Should receive the event
        let received = rx.try_recv();
        assert!(received.is_ok());
    }
}
