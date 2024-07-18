use egui::Vec2;
use serde::{Deserialize, Serialize};
use uuid::{Bytes, Uuid};

#[derive(Clone,Copy, Debug, Serialize, Deserialize)]
pub enum KeyframeType {
    KeyBtn(rdev::Key),      //0
    MouseBtn(rdev::Button), //1
    MouseMove(Vec2),        //2
    Scroll(Vec2),           //3
    Wait(f32),              //4
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keyframe {
    pub timestamp: f32,
    pub duration: f32,
    pub keyframe_type: KeyframeType,
    pub kind: u8,
    pub uid: Bytes,
}
impl Keyframe{
    pub fn mouse_move(timestamp: f32, pos: Vec2) -> Self{
        Self { 
            timestamp,
            duration: 0.1,
            keyframe_type: KeyframeType::MouseMove(pos),
            kind: 1,
            uid: Uuid::new_v4().to_bytes_le(),
        }
    }
    pub fn mouse_button(timestamp: f32, duration: f32, btn: rdev::Button) -> Self{
        Self { 
            timestamp,
            duration,
            keyframe_type: KeyframeType::MouseBtn(btn),
            kind: 2,
            uid: Uuid::new_v4().to_bytes_le(),
        }
    }
    pub fn key_btn(timestamp: f32, duration: f32, key: rdev::Key) -> Self{
        Self { 
            timestamp,
            duration,
            keyframe_type: KeyframeType::KeyBtn(key),
            kind: 0,
            uid: Uuid::new_v4().to_bytes_le(),
        }
    }
    pub fn scroll(timestamp: f32, delta: Vec2) -> Self{
        Self { 
            timestamp,
            duration: 0.1,
            keyframe_type: KeyframeType::Scroll(delta),
            kind: 3,
            uid: Uuid::new_v4().to_bytes_le(),
        }
    }
    pub fn calculate_duration(&mut self, dt: f32) -> &mut Self{
        self.duration = dt - self.timestamp;
        self
    }
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