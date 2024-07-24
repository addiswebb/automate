use egui::Vec2;
use serde::{Deserialize, Serialize};
use uuid::{Bytes, Uuid};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum KeyframeType {
    /// Simulates a key press
    KeyBtn(rdev::Key), //0
    /// Simulates a mouse button press
    MouseBtn(rdev::Button), //1
    /// Moves the mouse to the given position
    MouseMove(Vec2), //2
    /// Simulates a scroll action given a 2D delta vector
    Scroll(Vec2), //3
    /// Pauses the sequencer for the set amount of time in `seconds`
    Wait(f32), //4
    /// Similar to KeyBtn but presses multiple keys at once. Significantly faster
    KeyStrokes(Vec<rdev::Key>), //5
    /// Using a target image, it attempts to move the mouse to that position
    MagicMove(String), // 6
    /// Loop the keyframes within this keyframes timeframe
    Loop(u32, u32), // 7
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keyframe {
    pub timestamp: f32,
    pub duration: f32,
    pub keyframe_type: KeyframeType,
    pub kind: u8,
    pub uid: Bytes,
}
impl Keyframe {
    pub fn mouse_move(timestamp: f32, pos: Vec2) -> Self {
        Self {
            timestamp,
            duration: 0.1,
            keyframe_type: KeyframeType::MouseMove(pos),
            kind: 1,
            uid: Uuid::new_v4().to_bytes_le(),
        }
    }
    pub fn mouse_button(timestamp: f32, duration: f32, btn: rdev::Button) -> Self {
        Self {
            timestamp,
            duration,
            keyframe_type: KeyframeType::MouseBtn(btn),
            kind: 2,
            uid: Uuid::new_v4().to_bytes_le(),
        }
    }
    pub fn key_btn(timestamp: f32, duration: f32, key: rdev::Key) -> Self {
        Self {
            timestamp,
            duration,
            keyframe_type: KeyframeType::KeyBtn(key),
            kind: 0,
            uid: Uuid::new_v4().to_bytes_le(),
        }
    }
    pub fn scroll(timestamp: f32, delta: Vec2) -> Self {
        Self {
            timestamp,
            duration: 0.1,
            keyframe_type: KeyframeType::Scroll(delta),
            kind: 3,
            uid: Uuid::new_v4().to_bytes_le(),
        }
    }
    pub fn calculate_duration(&mut self, dt: f32) -> &mut Self {
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
