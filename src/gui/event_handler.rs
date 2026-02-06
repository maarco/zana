//! GUI Event Handler
//!
//! Bridges the hook event system with the egui GUI.
//! Subscribes to events and queues them for processing in the GUI thread.

use crate::hooks::{EventBus, HookEvent, HookEventType};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Maximum number of events to buffer in the queue
const EVENT_QUEUE_CAPACITY: usize = 256;

/// GUI Event Handler
///
/// Subscribes to EventBus events and provides them to the GUI for processing.
/// Uses an async channel to bridge between the async event system and sync GUI.
pub struct GuiEventHandler {
    /// Event sender for internal queue
    tx: mpsc::Sender<HookEvent>,
    /// Event receiver for internal queue
    rx: mpsc::Receiver<HookEvent>,
}

impl GuiEventHandler {
    /// Create a new GUI event handler
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(EVENT_QUEUE_CAPACITY);
        Self { tx, rx }
    }

    /// Get a clone of the sender for use in background tasks
    pub fn sender(&self) -> mpsc::Sender<HookEvent> {
        self.tx.clone()
    }

    /// Subscribe to events from the EventBus
    ///
    /// This spawns background tasks that listen to the EventBus
    /// and forward events to the GUI's internal queue.
    pub async fn subscribe(&self, event_bus: Arc<EventBus>) -> anyhow::Result<()> {
        // Subscribe to AudioLevelChange events
        let tx = self.tx.clone();
        let mut rx_audio = event_bus.subscribe(HookEventType::AudioLevelChange).await;
        tokio::spawn(async move {
            while let Ok(event) = rx_audio.recv().await {
                // Ignore send errors (GUI may be closed)
                let _ = tx.send(event).await;
            }
        });

        // Subscribe to AudioFftReady events
        let tx = self.tx.clone();
        let mut rx_fft = event_bus.subscribe(HookEventType::AudioFftReady).await;
        tokio::spawn(async move {
            while let Ok(event) = rx_fft.recv().await {
                let _ = tx.send(event).await;
            }
        });

        // Subscribe to TranscriptionProgress events
        let tx = self.tx.clone();
        let mut rx_progress = event_bus.subscribe(HookEventType::TranscriptionProgress).await;
        tokio::spawn(async move {
            while let Ok(event) = rx_progress.recv().await {
                let _ = tx.send(event).await;
            }
        });

        // Subscribe to TranscriptionComplete events
        let tx = self.tx.clone();
        let mut rx_complete = event_bus.subscribe(HookEventType::TranscriptionComplete).await;
        tokio::spawn(async move {
            while let Ok(event) = rx_complete.recv().await {
                let _ = tx.send(event).await;
            }
        });

        // Subscribe to Error events
        let tx = self.tx.clone();
        let mut rx_error = event_bus.subscribe(HookEventType::Error).await;
        tokio::spawn(async move {
            while let Ok(event) = rx_error.recv().await {
                let _ = tx.send(event).await;
            }
        });

        // Also subscribe to TranscriptionError events
        let tx = self.tx.clone();
        let mut rx_trans_error = event_bus.subscribe(HookEventType::TranscriptionError).await;
        tokio::spawn(async move {
            while let Ok(event) = rx_trans_error.recv().await {
                let _ = tx.send(event).await;
            }
        });

        Ok(())
    }

    /// Try to receive an event without blocking
    ///
    /// Returns None if no events are available.
    pub fn try_recv(&mut self) -> Option<HookEvent> {
        self.rx.try_recv().ok()
    }

    /// Process all pending events
    ///
    /// Calls the provided handler function for each queued event.
    /// Returns the number of events processed.
    pub fn process_pending<F>(&mut self, mut handler: F) -> usize
    where
        F: FnMut(&HookEvent),
    {
        let mut count = 0;
        while let Some(event) = self.try_recv() {
            handler(&event);
            count += 1;

            // Limit processing to prevent UI freeze
            if count >= 100 {
                break;
            }
        }
        count
    }
}

impl Default for GuiEventHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Subscribe to events from the EventBus with a given sender
///
/// This is a standalone function that can be used from background tasks.
pub async fn subscribe_to_events(
    event_bus: Arc<EventBus>,
    tx: mpsc::Sender<HookEvent>,
) -> anyhow::Result<()> {
    // Subscribe to AudioLevelChange events
    let tx_clone = tx.clone();
    let mut rx_audio = event_bus.subscribe(HookEventType::AudioLevelChange).await;
    tokio::spawn(async move {
        while let Ok(event) = rx_audio.recv().await {
            let _ = tx_clone.send(event).await;
        }
    });

    // Subscribe to AudioFftReady events
    let tx_clone = tx.clone();
    let mut rx_fft = event_bus.subscribe(HookEventType::AudioFftReady).await;
    tokio::spawn(async move {
        while let Ok(event) = rx_fft.recv().await {
            let _ = tx_clone.send(event).await;
        }
    });

    // Subscribe to TranscriptionProgress events
    let tx_clone = tx.clone();
    let mut rx_progress = event_bus.subscribe(HookEventType::TranscriptionProgress).await;
    tokio::spawn(async move {
        while let Ok(event) = rx_progress.recv().await {
            let _ = tx_clone.send(event).await;
        }
    });

    // Subscribe to TranscriptionComplete events
    let tx_clone = tx.clone();
    let mut rx_complete = event_bus.subscribe(HookEventType::TranscriptionComplete).await;
    tokio::spawn(async move {
        while let Ok(event) = rx_complete.recv().await {
            let _ = tx_clone.send(event).await;
        }
    });

    // Subscribe to Error events
    let tx_clone = tx.clone();
    let mut rx_error = event_bus.subscribe(HookEventType::Error).await;
    tokio::spawn(async move {
        while let Ok(event) = rx_error.recv().await {
            let _ = tx_clone.send(event).await;
        }
    });

    // Also subscribe to TranscriptionError events
    let tx_clone = tx.clone();
    let mut rx_trans_error = event_bus.subscribe(HookEventType::TranscriptionError).await;
    tokio::spawn(async move {
        while let Ok(event) = rx_trans_error.recv().await {
            let _ = tx_clone.send(event).await;
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::TranscriptionSegmentData;

    #[tokio::test]
    async fn test_event_handler_creation() {
        let mut handler = GuiEventHandler::new();
        // Should be able to create handler without blocking
        assert!(handler.try_recv().is_none());
    }

    #[tokio::test]
    async fn test_event_subscription() {
        let event_bus = Arc::new(EventBus::new());
        let mut handler = GuiEventHandler::new();

        // Subscribe to events
        handler.subscribe(event_bus.clone()).await.unwrap();

        // Emit an AudioLevelChange event
        event_bus
            .emit(HookEvent::AudioLevelChange {
                level: 0.5,
                peak: 0.8,
            })
            .await;

        // Give time for event to propagate
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Should receive the event
        let event = handler.try_recv();
        assert!(event.is_some());

        if let Some(HookEvent::AudioLevelChange { level, peak }) = event {
            assert_eq!(level, 0.5);
            assert_eq!(peak, 0.8);
        } else {
            panic!("Expected AudioLevelChange event");
        }
    }

    #[tokio::test]
    async fn test_audio_fft_event_subscription() {
        let event_bus = Arc::new(EventBus::new());
        let mut handler = GuiEventHandler::new();

        handler.subscribe(event_bus.clone()).await.unwrap();

        // Emit an AudioFftReady event
        let bins = vec![0.1, 0.5, 0.9, 0.3];
        event_bus
            .emit(HookEvent::AudioFftReady {
                bins: bins.clone(),
                bin_count: bins.len(),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let event = handler.try_recv();
        assert!(event.is_some());

        if let Some(HookEvent::AudioFftReady { bins, bin_count }) = event {
            assert_eq!(bin_count, 4);
            assert_eq!(bins.len(), 4);
        } else {
            panic!("Expected AudioFftReady event");
        }
    }

    #[tokio::test]
    async fn test_transcription_progress_subscription() {
        let event_bus = Arc::new(EventBus::new());
        let mut handler = GuiEventHandler::new();

        handler.subscribe(event_bus.clone()).await.unwrap();

        event_bus
            .emit(HookEvent::TranscriptionProgress { percent: 45.0 })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let event = handler.try_recv();
        assert!(event.is_some());

        if let Some(HookEvent::TranscriptionProgress { percent }) = event {
            assert_eq!(percent, 45.0);
        } else {
            panic!("Expected TranscriptionProgress event");
        }
    }

    #[tokio::test]
    async fn test_transcription_complete_subscription() {
        let event_bus = Arc::new(EventBus::new());
        let mut handler = GuiEventHandler::new();

        handler.subscribe(event_bus.clone()).await.unwrap();

        let segments = vec![TranscriptionSegmentData {
            start_ms: 0,
            end_ms: 1000,
            text: "Hello world".to_string(),
        }];

        event_bus
            .emit(HookEvent::TranscriptionComplete {
                text: "Hello world".to_string(),
                segments: segments.clone(),
                processing_ms: 500,
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let event = handler.try_recv();
        assert!(event.is_some());

        if let Some(HookEvent::TranscriptionComplete {
            text,
            segments,
            processing_ms,
        }) = event
        {
            assert_eq!(text, "Hello world");
            assert_eq!(segments.len(), 1);
            assert_eq!(processing_ms, 500);
        } else {
            panic!("Expected TranscriptionComplete event");
        }
    }

    #[tokio::test]
    async fn test_error_event_subscription() {
        let event_bus = Arc::new(EventBus::new());
        let mut handler = GuiEventHandler::new();

        handler.subscribe(event_bus.clone()).await.unwrap();

        event_bus
            .emit(HookEvent::Error {
                code: "TEST_ERR".to_string(),
                message: "Test error".to_string(),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let event = handler.try_recv();
        assert!(event.is_some());

        if let Some(HookEvent::Error { code, message }) = event {
            assert_eq!(code, "TEST_ERR");
            assert_eq!(message, "Test error");
        } else {
            panic!("Expected Error event");
        }
    }

    #[tokio::test]
    async fn test_process_pending_events() {
        let event_bus = Arc::new(EventBus::new());
        let mut handler = GuiEventHandler::new();

        handler.subscribe(event_bus.clone()).await.unwrap();

        // Emit multiple events
        for i in 0..5 {
            event_bus
                .emit(HookEvent::AudioLevelChange {
                    level: i as f32 / 10.0,
                    peak: 0.8,
                })
                .await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let mut count = 0;
        handler.process_pending(|event| {
            if let HookEvent::AudioLevelChange { .. } = event {
                count += 1;
            }
        });

        assert_eq!(count, 5);
    }

    #[tokio::test]
    async fn test_try_recv_returns_none_when_empty() {
        let mut handler = GuiEventHandler::new();
        assert!(handler.try_recv().is_none());
    }
}
