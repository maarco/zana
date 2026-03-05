//! Hook Handler Trait
//!
//! Defines the interface for components that want to handle hook events.
//! Handlers can intercept, modify, or stop event propagation.

use super::event::{HookEvent, HookEventType};
use async_trait::async_trait;
use std::fmt::Debug;

/// Result of handling a hook event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookResult {
    /// Continue to next handler (event unchanged)
    Continue,

    /// Stop event propagation (no more handlers will be called)
    Stop,

    /// Event was modified, continue with modified version
    Modified,

    /// Skip this handler (handler declined to process)
    Skip,
}

/// Trait for hook event handlers
///
/// Implement this trait to create a hook handler that can intercept
/// and respond to events in the Zana system.
///
/// # Example
///
/// ```rust
/// use Zana::hooks::{HookHandler, HookEvent, HookEventType, HookResult};
/// use async_trait::async_trait;
///
/// struct LoggingHandler;
///
/// #[async_trait]
/// impl HookHandler for LoggingHandler {
///     fn id(&self) -> &str {
///         "logging-handler"
///     }
///
///     fn subscribed_events(&self) -> Vec<HookEventType> {
///         vec![HookEventType::All]
///     }
///
///     async fn handle(&self, event: &mut HookEvent) -> HookResult {
///         log::info!("Event: {:?}", event);
///         HookResult::Continue
///     }
/// }
/// ```
#[async_trait]
pub trait HookHandler: Send + Sync {
    /// Unique identifier for this handler
    ///
    /// Used for registration, unregistration, and debugging.
    fn id(&self) -> &str;

    /// Human-readable name for this handler
    fn name(&self) -> &str {
        self.id()
    }

    /// Priority determines execution order
    ///
    /// Lower values execute first. Default is 100.
    /// System handlers typically use 0-50.
    /// Plugin handlers typically use 100-200.
    fn priority(&self) -> i32 {
        100
    }

    /// Which events this handler subscribes to
    ///
    /// Return `vec![HookEventType::All]` to receive all events.
    fn subscribed_events(&self) -> Vec<HookEventType>;

    /// Handle an event
    ///
    /// The event is passed as mutable, allowing handlers to modify it.
    /// Return value controls propagation:
    /// - `Continue` - Pass to next handler unchanged
    /// - `Stop` - Stop propagation, no more handlers called
    /// - `Modified` - Pass modified event to next handler
    /// - `Skip` - This handler declined to process
    async fn handle(&self, event: &mut HookEvent) -> HookResult;

    /// Called when the handler is registered
    ///
    /// Override this to perform initialization.
    async fn on_register(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Called when the handler is unregistered
    ///
    /// Override this to perform cleanup.
    async fn on_unregister(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Whether this handler is enabled
    ///
    /// Disabled handlers are skipped during event propagation.
    fn is_enabled(&self) -> bool {
        true
    }
}

/// Builder for creating simple hook handlers
///
/// Useful for creating handlers without implementing the full trait.
///
/// # Example
///
/// ```rust
/// let handler = HookHandlerBuilder::new("my-handler")
///     .priority(50)
///     .subscribe(HookEventType::AudioLevelChange)
///     .on_event(|event| async move {
///         if let HookEvent::AudioLevelChange { level, .. } = event {
///             println!("Audio level: {}", level);
///         }
///         HookResult::Continue
///     })
///     .build();
/// ```
pub struct HookHandlerBuilder {
    id: String,
    name: Option<String>,
    priority: i32,
    subscriptions: Vec<HookEventType>,
    enabled: bool,
}

impl HookHandlerBuilder {
    /// Create a new handler builder
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            priority: 100,
            subscriptions: Vec::new(),
            enabled: true,
        }
    }

    /// Set the handler name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the handler priority
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Subscribe to an event type
    pub fn subscribe(mut self, event_type: HookEventType) -> Self {
        self.subscriptions.push(event_type);
        self
    }

    /// Subscribe to multiple event types
    pub fn subscribe_all(mut self, event_types: impl IntoIterator<Item = HookEventType>) -> Self {
        self.subscriptions.extend(event_types);
        self
    }

    /// Set enabled state
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// A simple function-based hook handler
pub struct FnHookHandler<F>
where
    F: Fn(&mut HookEvent) -> HookResult + Send + Sync,
{
    id: String,
    name: String,
    priority: i32,
    subscriptions: Vec<HookEventType>,
    enabled: bool,
    handler: F,
}

impl<F> FnHookHandler<F>
where
    F: Fn(&mut HookEvent) -> HookResult + Send + Sync,
{
    /// Create a new function-based handler
    pub fn new(
        id: impl Into<String>,
        subscriptions: Vec<HookEventType>,
        handler: F,
    ) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id,
            priority: 100,
            subscriptions,
            enabled: true,
            handler,
        }
    }

    /// Set priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

#[async_trait]
impl<F> HookHandler for FnHookHandler<F>
where
    F: Fn(&mut HookEvent) -> HookResult + Send + Sync,
{
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    fn subscribed_events(&self) -> Vec<HookEventType> {
        self.subscriptions.clone()
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        (self.handler)(event)
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestHandler {
        id: String,
        subscriptions: Vec<HookEventType>,
    }

    #[async_trait]
    impl HookHandler for TestHandler {
        fn id(&self) -> &str {
            &self.id
        }

        fn subscribed_events(&self) -> Vec<HookEventType> {
            self.subscriptions.clone()
        }

        async fn handle(&self, _event: &mut HookEvent) -> HookResult {
            HookResult::Continue
        }
    }

    #[tokio::test]
    async fn test_handler_trait() {
        let handler = TestHandler {
            id: "test".to_string(),
            subscriptions: vec![HookEventType::AudioLevelChange],
        };

        assert_eq!(handler.id(), "test");
        assert_eq!(handler.priority(), 100);
        assert!(handler.is_enabled());
    }

    #[tokio::test]
    async fn test_fn_handler() {
        let handler = FnHookHandler::new(
            "fn-test",
            vec![HookEventType::All],
            |_event| HookResult::Continue,
        )
        .with_priority(50);

        assert_eq!(handler.id(), "fn-test");
        assert_eq!(handler.priority(), 50);
    }
}
