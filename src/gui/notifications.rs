//! Notification System
//!
//! Toast notification system for displaying transient messages to the user.

use eframe::egui;
use std::time::{Duration, Instant};

/// Notification type determines styling and behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationType {
    Success,
    Error,
    Info,
    Warning,
}

impl NotificationType {
    /// Get the color for this notification type
    pub fn color(&self) -> egui::Color32 {
        match self {
            Self::Success => egui::Color32::DARK_GREEN,
            Self::Error => egui::Color32::RED,
            Self::Info => egui::Color32::BLUE,
            Self::Warning => egui::Color32::YELLOW,
        }
    }

    /// Get the background color for this notification type
    pub fn background_color(&self) -> egui::Color32 {
        match self {
            Self::Success => egui::Color32::from_rgba_premultiplied(40, 80, 40, 230),
            Self::Error => egui::Color32::from_rgba_premultiplied(80, 40, 40, 230),
            Self::Info => egui::Color32::from_rgba_premultiplied(40, 40, 80, 230),
            Self::Warning => egui::Color32::from_rgba_premultiplied(80, 80, 40, 230),
        }
    }

    /// Get the icon for this notification type
    pub fn icon(&self) -> &str {
        match self {
            Self::Success => "✓",
            Self::Error => "✕",
            Self::Info => "ℹ",
            Self::Warning => "⚠",
        }
    }
}

/// A single toast notification
#[derive(Debug, Clone)]
pub struct Notification {
    /// Notification message
    pub message: String,
    /// Notification type
    pub notification_type: NotificationType,
    /// Auto-dismiss duration (None = manual dismiss)
    pub duration: Option<Duration>,
    /// When the notification was created
    pub timestamp: Instant,
    /// Whether the notification can be dismissed by clicking
    pub dismissible: bool,
}

impl Notification {
    /// Create a new notification
    pub fn new(message: String, notification_type: NotificationType) -> Self {
        let duration = match notification_type {
            NotificationType::Success => Some(Duration::from_secs(3)),
            NotificationType::Info => Some(Duration::from_secs(4)),
            NotificationType::Warning => Some(Duration::from_secs(5)),
            NotificationType::Error => None, // Errors must be manually dismissed
        };

        Self {
            message,
            notification_type,
            duration,
            timestamp: Instant::now(),
            dismissible: true,
        }
    }

    /// Create a success notification
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message.into(), NotificationType::Success)
    }

    /// Create an error notification
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message.into(), NotificationType::Error)
    }

    /// Create an info notification
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message.into(), NotificationType::Info)
    }

    /// Create a warning notification
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message.into(), NotificationType::Warning)
    }

    /// Check if this notification should auto-dismiss
    pub fn should_dismiss(&self) -> bool {
        if let Some(duration) = self.duration {
            self.timestamp.elapsed() >= duration
        } else {
            false
        }
    }

    /// Make this notification non-dismissible
    pub fn persistent(mut self) -> Self {
        self.dismissible = false;
        self
    }

    /// Set a custom duration
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
}

/// Manages a collection of toast notifications
#[derive(Debug, Default)]
pub struct NotificationManager {
    notifications: Vec<Notification>,
    max_notifications: usize,
}

impl NotificationManager {
    /// Create a new notification manager
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            max_notifications: 5,
        }
    }

    /// Set the maximum number of notifications to display
    pub fn with_max_notifications(mut self, max: usize) -> Self {
        self.max_notifications = max;
        self
    }

    /// Add a notification
    pub fn add(&mut self, notification: Notification) {
        self.notifications.push(notification);

        // Remove oldest if we exceed max
        while self.notifications.len() > self.max_notifications {
            self.notifications.remove(0);
        }
    }

    /// Add a success notification
    pub fn success(&mut self, message: impl Into<String>) {
        self.add(Notification::success(message));
    }

    /// Add an error notification
    pub fn error(&mut self, message: impl Into<String>) {
        self.add(Notification::error(message));
    }

    /// Add an info notification
    pub fn info(&mut self, message: impl Into<String>) {
        self.add(Notification::info(message));
    }

    /// Add a warning notification
    pub fn warning(&mut self, message: impl Into<String>) {
        self.add(Notification::warning(message));
    }

    /// Remove all notifications
    pub fn clear(&mut self) {
        self.notifications.clear();
    }

    /// Remove a specific notification by index
    pub fn remove(&mut self, index: usize) {
        if index < self.notifications.len() {
            self.notifications.remove(index);
        }
    }

    /// Update notifications (remove expired ones)
    pub fn update(&mut self) {
        self.notifications.retain(|n| !n.should_dismiss());
    }

    /// Get the number of active notifications
    pub fn count(&self) -> usize {
        self.notifications.len()
    }

    /// Check if there are any notifications
    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
    }

    /// Show notifications in the UI
    pub fn show(&mut self, ctx: &egui::Context) {
        self.update();

        if self.is_empty() {
            return;
        }

        egui::Area::new(egui::Id::new("notification_area"))
            .anchor(egui::Align2::RIGHT_TOP, [10.0, 10.0])
            .show(ctx, |ui| {
                ui.set_width(300.0);

                // Use a reverse iterator so newest notifications appear at the top
                let mut remove_idx = None;
                for (i, notification) in self.notifications.iter().enumerate().rev() {
                    Self::show_notification(ui, notification, i);

                    // Check for dismiss click
                    if notification.dismissible {
                        let response = ui.allocate_response(
                            egui::Vec2::new(300.0, 60.0),
                            egui::Sense::click(),
                        );
                        if response.clicked() {
                            remove_idx = Some(i);
                        }
                    }
                }

                // Remove clicked notification
                if let Some(idx) = remove_idx {
                    self.remove(idx);
                }
            });
    }

    /// Show a single notification
    fn show_notification(ui: &mut egui::Ui, notification: &Notification, index: usize) {
        let bg_color = notification.notification_type.background_color();
        let text_color = notification.notification_type.color();
        let icon = notification.notification_type.icon();

        // Add spacing between notifications
        if index > 0 {
            ui.add_space(8.0);
        }

        // Draw notification panel
        egui::Frame::NONE
            .fill(bg_color)
            .stroke(egui::Stroke::new(1.0, text_color))
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Icon
                    ui.label(
                        egui::RichText::new(icon)
                            .size(18.0)
                            .color(text_color),
                    );

                    ui.add_space(8.0);

                    // Message
                    ui.label(
                        egui::RichText::new(&notification.message)
                            .size(14.0)
                            .color(egui::Color32::WHITE),
                    );

                    // Show dismiss hint
                    if notification.dismissible {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new("✕")
                                    .size(12.0)
                                    .weak()
                            );
                        });
                    }
                });

                // Show progress bar for auto-dismiss
                if let Some(duration) = notification.duration {
                    let elapsed = notification.timestamp.elapsed();
                    let progress = (elapsed.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0);

                    if progress < 1.0 {
                        ui.add_space(4.0);
                        ui.add(egui::ProgressBar::new(progress as f32)
                            .desired_width(200.0));
                    }
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_type_colors() {
        assert_eq!(NotificationType::Success.color(), egui::Color32::DARK_GREEN);
        assert_eq!(NotificationType::Error.color(), egui::Color32::RED);
        assert_eq!(NotificationType::Info.color(), egui::Color32::BLUE);
        assert_eq!(NotificationType::Warning.color(), egui::Color32::YELLOW);
    }

    #[test]
    fn test_notification_type_icons() {
        assert_eq!(NotificationType::Success.icon(), "✓");
        assert_eq!(NotificationType::Error.icon(), "✕");
        assert_eq!(NotificationType::Info.icon(), "ℹ");
        assert_eq!(NotificationType::Warning.icon(), "⚠");
    }

    #[test]
    fn test_notification_constructors() {
        let success = Notification::success("Test success");
        assert_eq!(success.message, "Test success");
        assert_eq!(success.notification_type, NotificationType::Success);

        let error = Notification::error("Test error");
        assert_eq!(error.message, "Test error");
        assert_eq!(error.notification_type, NotificationType::Error);

        let info = Notification::info("Test info");
        assert_eq!(info.message, "Test info");
        assert_eq!(info.notification_type, NotificationType::Info);

        let warning = Notification::warning("Test warning");
        assert_eq!(warning.message, "Test warning");
        assert_eq!(warning.notification_type, NotificationType::Warning);
    }

    #[test]
    fn test_notification_should_dismiss() {
        let mut notif = Notification::success("Test");

        // Should not dismiss immediately
        assert!(!notif.should_dismiss());

        // Should dismiss after duration
        notif.timestamp = Instant::now() - Duration::from_secs(4);
        assert!(notif.should_dismiss());
    }

    #[test]
    fn test_notification_persistent() {
        let notif = Notification::error("Test");

        // Errors are persistent by default (no duration)
        assert!(notif.duration.is_none());
        assert!(!notif.should_dismiss());
    }

    #[test]
    fn test_notification_with_duration() {
        let notif = Notification::info("Test")
            .with_duration(Duration::from_secs(10));

        assert_eq!(notif.duration, Some(Duration::from_secs(10)));
    }

    #[test]
    fn test_notification_manager_new() {
        let manager = NotificationManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_notification_manager_add() {
        let mut manager = NotificationManager::new();

        manager.success("Test message");

        assert!(!manager.is_empty());
        assert_eq!(manager.count(), 1);
    }

    #[test]
    fn test_notification_manager_max_notifications() {
        let mut manager = NotificationManager::new()
            .with_max_notifications(3);

        manager.success("1");
        manager.info("2");
        manager.warning("3");
        manager.error("4");

        // Should only keep 3 most recent
        assert_eq!(manager.count(), 3);
    }

    #[test]
    fn test_notification_manager_clear() {
        let mut manager = NotificationManager::new();

        manager.success("Test");
        manager.clear();

        assert!(manager.is_empty());
    }

    #[test]
    fn test_notification_manager_remove() {
        let mut manager = NotificationManager::new();

        manager.success("1");
        manager.info("2");
        manager.remove(0);

        assert_eq!(manager.count(), 1);
    }
}
