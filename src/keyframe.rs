use egui::Vec2;
use serde::{Deserialize, Serialize};
use uuid::{Bytes, Uuid};

#[derive(Clone,Copy, Debug, Serialize, Deserialize)]
pub enum KeyframeType {
    KeyBtn(rdev::Key),      //0
    MouseBtn(rdev::Button), //1
    MouseMove(Vec2),        //2
    Scroll(Vec2)            //3
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Keyframe {
    pub timestamp: f32,
    pub duration: f32,
    pub keyframe_type: KeyframeType,
    pub kind: u8,
    pub uid: Bytes,
}



impl Default for Keyframe {
    fn default() -> Self {
        Self {
            timestamp: 0.0,
            duration: 0.0,
            keyframe_type: KeyframeType::KeyBtn(rdev::Key::Space),
            kind: 0,
            uid: Uuid::new_v4().to_bytes_le(),
        }
    }
}