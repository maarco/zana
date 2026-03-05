//! GUI Module
//!
//! Implements the egui-based user interface for Zana.

mod app;
mod dialogs;
mod event_handler;
mod notifications;
mod orb;
mod settings;
mod shortcuts;

#[cfg(test)]
mod tests;

pub use app::{
    ZanaApp, RecordingCommand, RecordingEvent, TranscriptionCommand, TranscriptionEvent,
};
pub use dialogs::DialogState;
pub use event_handler::GuiEventHandler;
pub use notifications::{Notification, NotificationManager, NotificationType};
pub use orb::OrbRenderer;
pub use settings::SettingsPanel;
pub use shortcuts::{ShortcutAction, ShortcutHandler};
