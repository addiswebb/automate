use std::ops::Add;

use egui::{KeyboardShortcut, Vec2};
use rdev::Button;
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
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub enum MonitorEdge {
    Left,
    Right,
    Bottom,
    Top,
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
    pub fail_detection: bool,
    pub max_fail_error: u32,
    pub offset: Vec2,
    pub retake_screenshots: bool,
    #[serde(skip)]
    pub page: SettingsPage,
    #[serde(skip)]
    pub show: bool,
    pub add_keyframe_data: AddKeyframeData,
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
            fail_detection: true,
            max_fail_error: 20,
            offset: Vec2::NAN,
            retake_screenshots: false,
            page: SettingsPage::Preferences,
            show: false,
            add_keyframe_data: AddKeyframeData {
                show: false,
                key_str: String::new(),
                move_pos: Vec2::ZERO,
                mouse_btn: Button::Left,
                wait: 0.0,
                magic_move_path: String::new(),
                loop_iterations: 1,
            },
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct AddKeyframeData {
    #[serde(skip)]
    pub show: bool,
    pub key_str: String,
    pub move_pos: Vec2,
    pub mouse_btn: Button,
    pub wait: f32,
    pub magic_move_path: String,
    pub loop_iterations: u32,
}
