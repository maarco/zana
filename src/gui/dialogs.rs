//! Modal Dialog System
//!
//! Provides modal dialogs for error messages and confirmations.

use eframe::egui;

/// Dialog state for managing modal dialogs
#[derive(Default)]
pub struct DialogState {
    /// Error dialog state
    pub show_error: bool,
    error_title: String,
    error_message: String,
    error_details: Option<String>,

    /// Confirm dialog state
    pub show_confirm: bool,
    confirm_message: String,
    confirm_title: String,
    confirm_callback: Option<Box<dyn FnOnce() -> bool + 'static>>,

    /// Dialog result (for waiting on user response)
    pending_result: Option<bool>,
}

impl DialogState {
    /// Create a new dialog state
    pub fn new() -> Self {
        Self::default()
    }

    /// Show an error dialog
    pub fn show_error_dialog(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.show_error = true;
        self.error_title = title.into();
        self.error_message = message.into();
        self.error_details = None;
    }

    /// Show an error dialog with details
    pub fn show_error_dialog_with_details(
        &mut self,
        title: impl Into<String>,
        message: impl Into<String>,
        details: impl Into<String>,
    ) {
        self.show_error = true;
        self.error_title = title.into();
        self.error_message = message.into();
        self.error_details = Some(details.into());
    }

    /// Show a confirmation dialog
    ///
    /// The callback will be called if the user clicks "OK" or "Yes"
    pub fn show_confirm_dialog(
        &mut self,
        title: impl Into<String>,
        message: impl Into<String>,
        callback: Box<dyn FnOnce() -> bool + 'static>,
    ) {
        self.show_confirm = true;
        self.confirm_title = title.into();
        self.confirm_message = message.into();
        self.confirm_callback = Some(callback);
        self.pending_result = None;
    }

    /// Show a simple confirmation dialog
    ///
    /// Returns true if user confirmed, false if cancelled
    pub fn confirm(&mut self, title: impl Into<String>, message: impl Into<String>) -> bool {
        self.show_confirm = true;
        self.confirm_title = title.into();
        self.confirm_message = message.into();
        self.confirm_callback = None;
        self.pending_result = Some(false);
        false
    }

    /// Check if a confirmation result is ready
    pub fn take_confirm_result(&mut self) -> Option<bool> {
        self.pending_result.take()
    }

    /// Close the current error dialog
    pub fn close_error(&mut self) {
        self.show_error = false;
        self.error_title.clear();
        self.error_message.clear();
        self.error_details = None;
    }

    /// Close the current confirm dialog
    pub fn close_confirm(&mut self) {
        self.show_confirm = false;
        self.confirm_message.clear();
        self.confirm_title.clear();
        self.confirm_callback = None;
        self.pending_result = None;
    }

    /// Show any active dialogs
    pub fn show(&mut self, ctx: &egui::Context) {
        if self.show_error {
            self.show_error_dialog_ui(ctx);
        }

        if self.show_confirm {
            self.show_confirm_dialog_ui(ctx);
        }
    }

    /// Show the error dialog UI
    fn show_error_dialog_ui(&mut self, ctx: &egui::Context) {
        let _keep_open = &mut self.show_error;
        let title = self.error_title.clone();
        let message = self.error_message.clone();
        let details = self.error_details.clone();

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([400.0, 200.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);

                    // Error icon
                    ui.label(
                        egui::RichText::new("✕")
                            .size(48.0)
                            .color(egui::Color32::RED)
                    );

                    ui.add_space(16.0);

                    // Error message
                    ui.label(
                        egui::RichText::new(&message)
                            .size(16.0)
                            .color(egui::Color32::WHITE)
                    );

                    // Error details (if any)
                    if let Some(details) = &details {
                        ui.add_space(12.0);
                        egui::ScrollArea::vertical()
                            .max_height(80.0)
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(details)
                                        .size(12.0)
                                        .monospace()
                                        .weak()
                                );
                            });
                    }

                    ui.add_space(20.0);

                    // Copy error button
                    if ui.button("Copy Error").clicked() {
                        let error_text = if let Some(details) = &details {
                            format!("{}\n\nDetails:\n{}", message, details)
                        } else {
                            message.clone()
                        };

                        // Copy to clipboard
                        ctx.copy_text(error_text);
                    }

                    ui.add_space(8.0);

                    // Close button
                    if ui.add_sized([120.0, 30.0], egui::Button::new("Close")).clicked() {
                        self.close_error();
                    }
                });
            });
    }

    /// Show the confirm dialog UI
    fn show_confirm_dialog_ui(&mut self, ctx: &egui::Context) {
        let _keep_open = &mut self.show_confirm;
        let title = self.confirm_title.clone();
        let message = self.confirm_message.clone();

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([400.0, 150.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);

                    // Warning icon
                    ui.label(
                        egui::RichText::new("⚠")
                            .size(48.0)
                            .color(egui::Color32::YELLOW)
                    );

                    ui.add_space(16.0);

                    // Confirmation message
                    ui.label(
                        egui::RichText::new(&message)
                            .size(16.0)
                            .color(egui::Color32::WHITE)
                    );

                    ui.add_space(20.0);

                    // Buttons
                    ui.horizontal(|ui| {
                        ui.add_space(40.0);

                        // Cancel button
                        if ui.add_sized([120.0, 30.0], egui::Button::new("Cancel")).clicked() {
                            let _callback = self.confirm_callback.take();
                            self.pending_result = Some(false);
                            self.close_confirm();
                        }

                        ui.add_space(16.0);

                        // Confirm button
                        if ui.add_sized(
                            [120.0, 30.0],
                            egui::Button::new("Confirm").fill(egui::Color32::RED)
                        ).clicked()
                        {
                            if let Some(callback) = self.confirm_callback.take() {
                                let _ = callback();
                            }
                            self.pending_result = Some(true);
                            self.close_confirm();
                        }
                    });
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dialog_state_new() {
        let state = DialogState::new();
        assert!(!state.show_error);
        assert!(!state.show_confirm);
    }

    #[test]
    fn test_show_error_dialog() {
        let mut state = DialogState::new();

        state.show_error_dialog("Test Error", "Something went wrong");

        assert!(state.show_error);
        assert_eq!(state.error_title, "Test Error");
        assert_eq!(state.error_message, "Something went wrong");
        assert!(state.error_details.is_none());
    }

    #[test]
    fn test_show_error_dialog_with_details() {
        let mut state = DialogState::new();

        state.show_error_dialog_with_details(
            "Test Error",
            "Something went wrong",
            "Stack trace here",
        );

        assert!(state.show_error);
        assert_eq!(state.error_details, Some("Stack trace here".to_string()));
    }

    #[test]
    fn test_close_error() {
        let mut state = DialogState::new();

        state.show_error_dialog("Test", "Error");
        assert!(state.show_error);

        state.close_error();
        assert!(!state.show_error);
        assert!(state.error_title.is_empty());
        assert!(state.error_message.is_empty());
    }

    #[test]
    fn test_show_confirm_dialog() {
        let mut state = DialogState::new();
        let callback_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let callback_called_clone = callback_called.clone();

        state.show_confirm_dialog(
            "Confirm",
            "Are you sure?",
            Box::new(move || {
                callback_called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                true
            }),
        );

        assert!(state.show_confirm);
        assert_eq!(state.confirm_title, "Confirm");
        assert_eq!(state.confirm_message, "Are you sure?");
    }

    #[test]
    fn test_confirm_simple() {
        let mut state = DialogState::new();

        let result = state.confirm("Confirm", "Are you sure?");

        assert!(!result); // Should return false immediately
        assert!(state.show_confirm);
        assert!(state.pending_result.is_some());
    }

    #[test]
    fn test_take_confirm_result() {
        let mut state = DialogState::new();

        state.confirm("Test", "Message");
        assert!(state.pending_result.is_some());

        let result = state.take_confirm_result();
        assert_eq!(result, Some(false));

        // Should be None after taking
        assert!(state.take_confirm_result().is_none());
    }

    #[test]
    fn test_close_confirm() {
        let mut state = DialogState::new();

        state.confirm("Test", "Message");
        assert!(state.show_confirm);

        state.close_confirm();
        assert!(!state.show_confirm);
        assert!(state.confirm_callback.is_none());
        assert!(state.pending_result.is_none());
    }
}
