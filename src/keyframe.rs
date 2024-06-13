use egui::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum KeyframeType {
    KeyBtn(rdev::Key),      //0
    MouseBtn(rdev::Button), //1
    MouseMove(Vec2),        //2
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keyframe {
    pub timestamp: f32,
    /*
       For mouse move, it interplates over the duration
       For mouse btn, it holds button for the duration
       For key btn, it holds button for durration (allows for repeated keystrokes)
    */
    pub duration: f32,
    pub keyframe_type: KeyframeType,
    pub id: u8,
}

impl Default for Keyframe {
    fn default() -> Self {
        Self {
            timestamp: 0.0,
            duration: 0.0,
            keyframe_type: KeyframeType::KeyBtn(rdev::Key::Space),
            id: 0,
        }
    }
}