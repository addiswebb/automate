use egui::{KeyboardShortcut, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub enum KeybindType {
    SaveFile,
    NewFile,
    OpenFile,
    Undo,
    Redo,
    ToggleSettings,
    NextKeyframe,
    PreviousKeyframe,
    TogglePlay,
    ResetTime,
    ToggleRecording,
    ToggleExecution,
    AddKeyframe,
    SelectAll,
}

pub enum SettingsPage {
    Preferences,
    Shortcuts,
}
impl Default for SettingsPage {
    fn default() -> Self {
        Self::Preferences
    }
}

#[derive(Deserialize, Serialize)]
pub struct Keybind {
    pub text: String,
    pub kind: KeybindType,
    pub keybind: KeyboardShortcut,
}
impl Keybind {
    pub fn new(text: String, kind: KeybindType, keybind: KeyboardShortcut) -> Self {
        Self {
            text,
            kind,
            keybind,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Settings {
    #[serde(skip)]
    pub keybind_search: String,
    pub keybinds: Vec<Keybind>,
    pub offset: Vec2,
    #[serde(skip)]
    pub page: SettingsPage,
    #[serde(skip)]
    pub show: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            keybind_search: "".to_string(),
            keybinds: vec![
                Keybind::new(
                    "Save File".to_string(),
                    KeybindType::SaveFile,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::S),
                ),
                Keybind::new(
                    "New File".to_string(),
                    KeybindType::NewFile,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::N),
                ),
                Keybind::new(
                    "Open File".to_string(),
                    KeybindType::OpenFile,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::O),
                ),
                Keybind::new(
                    "Undo".to_string(),
                    KeybindType::Undo,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Z),
                ),
                Keybind::new(
                    "Redo".to_string(),
                    KeybindType::Redo,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Y),
                ),
                Keybind::new(
                    "Toggle Settings".to_string(),
                    KeybindType::ToggleSettings,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Comma),
                ),
                Keybind::new(
                    "Next Keyframe".to_string(),
                    KeybindType::NextKeyframe,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::ArrowRight),
                ),
                Keybind::new(
                    "Previous Keyframe".to_string(),
                    KeybindType::PreviousKeyframe,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::ArrowLeft),
                ),
                Keybind::new(
                    "Toggle Play".to_string(),
                    KeybindType::TogglePlay,
                    KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Space),
                ),
                Keybind::new(
                    "Reset Time".to_string(),
                    KeybindType::ResetTime,
                    KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::ArrowLeft),
                ),
                Keybind::new(
                    "Toggle Recording".to_string(),
                    KeybindType::ToggleRecording,
                    KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F8),
                ),
                Keybind::new(
                    "Toggle Execution".to_string(),
                    KeybindType::ToggleExecution,
                    KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Escape),
                ),
                Keybind::new(
                    "Add Keyframe".to_string(),
                    KeybindType::AddKeyframe,
                    KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F9),
                ),
                Keybind::new(
                    "Select All".to_string(),
                    KeybindType::SelectAll,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::A),
                ),
            ],
            offset: Vec2::NAN,
            page: SettingsPage::Preferences,
            show: false,
        }
    }
}
