//! Keyboard Shortcut Handler
//!
//! Manages keyboard shortcuts for the application.

use eframe::egui;

/// Action triggered by a keyboard shortcut
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    Record,
    Stop,
    Transcribe,
    Settings,
    Hide,
    None,
}

/// Keyboard shortcut state
#[derive(Debug, Default)]
pub struct ShortcutHandler {
    /// Track which keys are currently pressed to prevent repeat triggers
    record_pressed: bool,
    stop_pressed: bool,
    transcribe_pressed: bool,
    settings_pressed: bool,
    hide_pressed: bool,
}

impl ShortcutHandler {
    /// Create a new shortcut handler
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle keyboard input and return the triggered action
    pub fn handle_input(&mut self, ctx: &egui::Context) -> ShortcutAction {
        let mut action = ShortcutAction::None;

        ctx.input(|i| {
            // Don't trigger shortcuts when using modifier keys
            if i.modifiers.shift || i.modifiers.ctrl || i.modifiers.alt || i.modifiers.command {
                return;
            }

            // Record shortcut: R
            if i.key_pressed(egui::Key::R) && !self.record_pressed {
                self.record_pressed = true;
                action = ShortcutAction::Record;
            } else if !i.key_pressed(egui::Key::R) {
                self.record_pressed = false;
            }

            // Stop shortcut: S
            if i.key_pressed(egui::Key::S) && !self.stop_pressed {
                self.stop_pressed = true;
                action = ShortcutAction::Stop;
            } else if !i.key_pressed(egui::Key::S) {
                self.stop_pressed = false;
            }

            // Transcribe shortcut: T
            if i.key_pressed(egui::Key::T) && !self.transcribe_pressed {
                self.transcribe_pressed = true;
                action = ShortcutAction::Transcribe;
            } else if !i.key_pressed(egui::Key::T) {
                self.transcribe_pressed = false;
            }

            // Settings shortcut: Comma (,)
            if i.key_pressed(egui::Key::Comma) && !self.settings_pressed {
                self.settings_pressed = true;
                action = ShortcutAction::Settings;
            } else if !i.key_pressed(egui::Key::Comma) {
                self.settings_pressed = false;
            }

            // Hide shortcut: H
            if i.key_pressed(egui::Key::H) && !self.hide_pressed {
                self.hide_pressed = true;
                action = ShortcutAction::Hide;
            } else if !i.key_pressed(egui::Key::H) {
                self.hide_pressed = false;
            }
        });

        action
    }

    /// Get the label for a shortcut action
    pub fn action_label(action: ShortcutAction) -> &'static str {
        match action {
            ShortcutAction::Record => "Record",
            ShortcutAction::Stop => "Stop",
            ShortcutAction::Transcribe => "Transcribe",
            ShortcutAction::Settings => "Settings",
            ShortcutAction::Hide => "Hide",
            ShortcutAction::None => "",
        }
    }

    /// Get the keyboard shortcut text for an action
    pub fn action_shortcut(action: ShortcutAction) -> &'static str {
        match action {
            ShortcutAction::Record => "R",
            ShortcutAction::Stop => "S",
            ShortcutAction::Transcribe => "T",
            ShortcutAction::Settings => ",",
            ShortcutAction::Hide => "H",
            ShortcutAction::None => "",
        }
    }

    /// Format a tooltip with the shortcut hint
    pub fn tooltip_with_shortcut(action: ShortcutAction, base_text: &str) -> String {
        let shortcut = Self::action_shortcut(action);
        if shortcut.is_empty() {
            base_text.to_string()
        } else {
            format!("{} ({})", base_text, shortcut)
        }
    }

    /// Create a tooltip text for a button
    pub fn button_tooltip(action: ShortcutAction) -> String {
        let base_text = Self::action_label(action);
        let shortcut = Self::action_shortcut(action);
        if shortcut.is_empty() {
            base_text.to_string()
        } else {
            format!("Press {} to {}", shortcut, base_text.to_lowercase())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortcut_action_labels() {
        assert_eq!(ShortcutHandler::action_label(ShortcutAction::Record), "Record");
        assert_eq!(ShortcutHandler::action_label(ShortcutAction::Stop), "Stop");
        assert_eq!(ShortcutHandler::action_label(ShortcutAction::Transcribe), "Transcribe");
        assert_eq!(ShortcutHandler::action_label(ShortcutAction::Settings), "Settings");
        assert_eq!(ShortcutHandler::action_label(ShortcutAction::None), "");
    }

    #[test]
    fn test_shortcut_action_shortcuts() {
        assert_eq!(ShortcutHandler::action_shortcut(ShortcutAction::Record), "R");
        assert_eq!(ShortcutHandler::action_shortcut(ShortcutAction::Stop), "S");
        assert_eq!(ShortcutHandler::action_shortcut(ShortcutAction::Transcribe), "T");
        assert_eq!(ShortcutHandler::action_shortcut(ShortcutAction::Settings), ",");
        assert_eq!(ShortcutHandler::action_shortcut(ShortcutAction::None), "");
    }

    #[test]
    fn test_tooltip_with_shortcut() {
        let tooltip = ShortcutHandler::tooltip_with_shortcut(
            ShortcutAction::Record,
            "Start recording"
        );
        assert_eq!(tooltip, "Start recording (R)");

        let tooltip = ShortcutHandler::tooltip_with_shortcut(
            ShortcutAction::None,
            "No shortcut"
        );
        assert_eq!(tooltip, "No shortcut");
    }

    #[test]
    fn test_button_tooltip() {
        let tooltip = ShortcutHandler::button_tooltip(ShortcutAction::Record);
        assert_eq!(tooltip, "Press R to record");

        let tooltip = ShortcutHandler::button_tooltip(ShortcutAction::Stop);
        assert_eq!(tooltip, "Press S to stop");

        let tooltip = ShortcutHandler::button_tooltip(ShortcutAction::Transcribe);
        assert_eq!(tooltip, "Press T to transcribe");
    }

    #[test]
    fn test_shortcut_handler_new() {
        let handler = ShortcutHandler::new();
        assert!(!handler.record_pressed);
        assert!(!handler.stop_pressed);
        assert!(!handler.transcribe_pressed);
        assert!(!handler.settings_pressed);
        assert!(!handler.hide_pressed);
    }
}
