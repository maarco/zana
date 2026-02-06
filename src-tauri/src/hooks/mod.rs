//! Hook System
//!
//! The kVoice hook system enables extensibility by allowing plugins
//! and components to intercept and respond to events throughout the
//! application.
//!
//! # Architecture
//!
//! The hook system consists of three main components:
//!
//! - **Events** (`event.rs`): Typed events that represent actions in the system
//! - **Handlers** (`handler.rs`): Components that respond to events
//! - **Event Bus** (`bus.rs`): Central routing hub for events
//!
//! # Example
//!
//! ```rust
//! use kvoice::hooks::{EventBus, HookEvent, HookEventType, HookHandler, HookResult};
//! use std::sync::Arc;
//!
//! // Create the event bus
//! let bus = Arc::new(EventBus::new());
//!
//! // Register a handler
//! let handler = Arc::new(MyHandler);
//! bus.register(handler).await?;
//!
//! // Emit an event
//! bus.emit(HookEvent::AudioLevelChange {
//!     level: 0.5,
//!     peak: 0.8,
//! }).await;
//! ```

mod bus;
mod event;
mod handler;
mod system_handlers;

pub use bus::{EventBus, EventBusStats};
pub use event::{HookEvent, HookEventType, PluginType, Theme, TranscriptionSegmentData};
pub use handler::{FnHookHandler, HookHandler, HookHandlerBuilder, HookResult};
pub use system_handlers::{LoggingHandler, MetricsHandler, ValidationHandler};
