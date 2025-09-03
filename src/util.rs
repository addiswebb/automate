use egui::{emath::RectTransform, vec2, Pos2, Rect, Ui, Vec2};
use image::{DynamicImage, ImageBuffer, Rgba};
use xcap::Monitor;

pub const ROW_HEIGHT: f32 = 24.0;

/// Converts a given `t` in seconds to a window space rect using `d` duration to determine the width
///
/// `max_t` used specifically for keyframes to clip them between `0s` and the visible end of the sequencer
pub fn time_to_rect(t: f32, d: f32, max_t: f32, spacing: Vec2, res_rect: Rect) -> Option<Rect> {
    let to_screen =
        RectTransform::from_to(Rect::from_min_size(Pos2::ZERO, res_rect.size()), res_rect);
    let mut p1 = Pos2 { x: t, y: 0.0 };
    let height = ROW_HEIGHT - (spacing.y * 2.0);
    let width = d.clamp(3.0, f32::INFINITY);
    let mut p2 = p1
        + Vec2 {
            x: width,
            y: height,
        };
    if max_t != 0.0 {
        // 0.0 is for non-keyframe use cases to ignore
        if p1.x > max_t || p2.x < 0.0 {
            return None;
        }
        if p2.x > max_t {
            p2.x = max_t - 1.0;
        }
        if p1.x < 0.0 {
            p1.x = 0.0;
        }
    }
    Some(Rect {
        min: to_screen.transform_pos(p1),
        max: to_screen.transform_pos(p2),
    })
}

pub fn selection_contains_keyframe(selection: &Rect, keyframe: Rect) -> bool {
    if selection.min.x + selection.width() >= keyframe.min.x
        && selection.min.x <= keyframe.min.x + keyframe.width()
        && selection.min.y + selection.height() >= keyframe.min.y
        && selection.min.y <= keyframe.min.y + keyframe.height()
    {
        true
    } else {
        false
    }
}

pub fn keys_to_string(keys: &Vec<rdev::Key>) -> String {
    let mut string = String::new();
    for key in keys {
        string.push_str(key_to_char(key).as_str());
    }
    string
}
#[allow(unused)]
pub fn strings_to_keys(string: &String) -> Vec<rdev::Key> {
    let mut keys = vec![];
    for x in string.split(' ') {
        let key = string_to_keys(x);
        // This fails if x is a string of characters, not a single character
        if let Some(key) = key {
            keys.push(key);
        } else {
            // if it failes then loop through the individual characters of the string too
            for y in x.chars().into_iter() {
                let key = string_to_keys(y.to_string().as_str());
                if let Some(key) = key {
                    keys.push(key);
                }
            }
        }
    }
    return keys;
}
#[allow(unused)]
pub fn string_to_keys(c: &str) -> Option<rdev::Key> {
    match c {
        "a" => Some(rdev::Key::KeyA),
        "b" => Some(rdev::Key::KeyB),
        "c" => Some(rdev::Key::KeyC),
        "d" => Some(rdev::Key::KeyD),
        "e" => Some(rdev::Key::KeyE),
        "f" => Some(rdev::Key::KeyF),
        "g" => Some(rdev::Key::KeyG),
        "h" => Some(rdev::Key::KeyH),
        "i" => Some(rdev::Key::KeyI),
        "j" => Some(rdev::Key::KeyJ),
        "k" => Some(rdev::Key::KeyK),
        "l" => Some(rdev::Key::KeyL),
        "m" => Some(rdev::Key::KeyM),
        "n" => Some(rdev::Key::KeyN),
        "o" => Some(rdev::Key::KeyO),
        "p" => Some(rdev::Key::KeyP),
        "q" => Some(rdev::Key::KeyQ),
        "r" => Some(rdev::Key::KeyR),
        "s" => Some(rdev::Key::KeyS),
        "t" => Some(rdev::Key::KeyT),
        "u" => Some(rdev::Key::KeyU),
        "v" => Some(rdev::Key::KeyV),
        "w" => Some(rdev::Key::KeyW),
        "x" => Some(rdev::Key::KeyX),
        "y" => Some(rdev::Key::KeyY),
        "z" => Some(rdev::Key::KeyZ),
        "space" => Some(rdev::Key::Space),
        "tab" => Some(rdev::Key::Tab),
        "uparrow" => Some(rdev::Key::UpArrow),
        "printscreen" => Some(rdev::Key::PrintScreen),
        "scrolllock" => Some(rdev::Key::ScrollLock),
        "pause" => Some(rdev::Key::Pause),
        "numlock" => Some(rdev::Key::NumLock),
        "`" => Some(rdev::Key::BackQuote),
        "1" => Some(rdev::Key::Num1),
        "2" => Some(rdev::Key::Num2),
        "3" => Some(rdev::Key::Num3),
        "4" => Some(rdev::Key::Num4),
        "5" => Some(rdev::Key::Num5),
        "6" => Some(rdev::Key::Num6),
        "7" => Some(rdev::Key::Num7),
        "8" => Some(rdev::Key::Num8),
        "9" => Some(rdev::Key::Num9),
        "0" => Some(rdev::Key::Num0),
        "-" => Some(rdev::Key::Minus),
        "=" => Some(rdev::Key::Equal),
        "(" => Some(rdev::Key::LeftBracket),
        ")" => Some(rdev::Key::RightBracket),
        ";" => Some(rdev::Key::SemiColon),
        "\"" => Some(rdev::Key::Quote),
        "\\" => Some(rdev::Key::BackSlash),
        "intlbackslash" => Some(rdev::Key::IntlBackslash),
        ")," => Some(rdev::Key::Comma),
        "." => Some(rdev::Key::Dot),
        "/" => Some(rdev::Key::Slash),
        "insert" => Some(rdev::Key::Insert),
        "kpreturn" => Some(rdev::Key::KpReturn),
        "kpminus" => Some(rdev::Key::KpMinus),
        "kpplus" => Some(rdev::Key::KpPlus),
        "kpmultiply" => Some(rdev::Key::KpMultiply),
        "kpdivide" => Some(rdev::Key::KpDivide),
        "kp0" => Some(rdev::Key::Kp0),
        "kp1" => Some(rdev::Key::Kp1),
        "kp2" => Some(rdev::Key::Kp2),
        "kp3" => Some(rdev::Key::Kp3),
        "kp4" => Some(rdev::Key::Kp4),
        "kp5" => Some(rdev::Key::Kp5),
        "kp6" => Some(rdev::Key::Kp6),
        "kp7" => Some(rdev::Key::Kp7),
        "kp8" => Some(rdev::Key::Kp8),
        "kp9" => Some(rdev::Key::Kp9),
        "kpdelete" => Some(rdev::Key::KpDelete),
        "function" => Some(rdev::Key::Function),
        "alt" => Some(rdev::Key::Alt),
        "altgr" => Some(rdev::Key::AltGr),
        "backspace" => Some(rdev::Key::Backspace),
        "capslock" => Some(rdev::Key::CapsLock),
        "ctrlleft" => Some(rdev::Key::ControlLeft),
        "ctrlright" => Some(rdev::Key::ControlRight),
        "delete" => Some(rdev::Key::Delete),
        "downarrow" => Some(rdev::Key::DownArrow),
        "end" => Some(rdev::Key::End),
        "esc" => Some(rdev::Key::Escape),
        "f1" => Some(rdev::Key::F1),
        "f10" => Some(rdev::Key::F10),
        "f11" => Some(rdev::Key::F11),
        "f12" => Some(rdev::Key::F12),
        "f2" => Some(rdev::Key::F2),
        "f3" => Some(rdev::Key::F3),
        "f4" => Some(rdev::Key::F4),
        "f5" => Some(rdev::Key::F5),
        "f6" => Some(rdev::Key::F6),
        "f7" => Some(rdev::Key::F7),
        "f8" => Some(rdev::Key::F8),
        "f9" => Some(rdev::Key::F9),
        "home" => Some(rdev::Key::Home),
        "leftarrow" => Some(rdev::Key::LeftArrow),
        "metaleft" => Some(rdev::Key::MetaLeft),
        "metaright" => Some(rdev::Key::MetaRight),
        "pagedown" => Some(rdev::Key::PageDown),
        "pageup" => Some(rdev::Key::PageUp),
        "return" => Some(rdev::Key::Return),
        "rightarrow" => Some(rdev::Key::RightArrow),
        "shiftleft" => Some(rdev::Key::ShiftLeft),
        "shiftright" => Some(rdev::Key::ShiftRight),
        _ => None,
    }
}
pub fn key_to_char(k: &rdev::Key) -> String {
    match k {
        rdev::Key::KeyA => "a",
        rdev::Key::KeyB => "b",
        rdev::Key::KeyC => "c",
        rdev::Key::KeyD => "d",
        rdev::Key::KeyE => "e",
        rdev::Key::KeyF => "f",
        rdev::Key::KeyG => "g",
        rdev::Key::KeyH => "h",
        rdev::Key::KeyI => "i",
        rdev::Key::KeyJ => "j",
        rdev::Key::KeyK => "k",
        rdev::Key::KeyL => "l",
        rdev::Key::KeyM => "m",
        rdev::Key::KeyN => "n",
        rdev::Key::KeyO => "o",
        rdev::Key::KeyP => "p",
        rdev::Key::KeyQ => "q",
        rdev::Key::KeyR => "r",
        rdev::Key::KeyS => "s",
        rdev::Key::KeyT => "t",
        rdev::Key::KeyU => "u",
        rdev::Key::KeyV => "v",
        rdev::Key::KeyW => "w",
        rdev::Key::KeyX => "x",
        rdev::Key::KeyY => "y",
        rdev::Key::KeyZ => "z",
        rdev::Key::Space => "_",
        rdev::Key::Tab => egui_phosphor::regular::ARROW_LINE_RIGHT,
        rdev::Key::UpArrow => egui_phosphor::regular::ARROW_UP,
        rdev::Key::PrintScreen => egui_phosphor::regular::PRINTER,
        rdev::Key::ScrollLock => egui_phosphor::regular::LOCK_SIMPLE,
        rdev::Key::Pause => egui_phosphor::regular::PAUSE,
        rdev::Key::NumLock => egui_phosphor::regular::LOCK,
        rdev::Key::BackQuote => "`",
        rdev::Key::Num1 => "1",
        rdev::Key::Num2 => "2",
        rdev::Key::Num3 => "3",
        rdev::Key::Num4 => "4",
        rdev::Key::Num5 => "5",
        rdev::Key::Num6 => "6",
        rdev::Key::Num7 => "7",
        rdev::Key::Num8 => "8",
        rdev::Key::Num9 => "9",
        rdev::Key::Num0 => "0",
        rdev::Key::Minus => egui_phosphor::regular::MINUS,
        rdev::Key::Equal => egui_phosphor::regular::EQUALS,
        rdev::Key::LeftBracket => "(",
        rdev::Key::RightBracket => ")",
        rdev::Key::SemiColon => ";",
        rdev::Key::Quote => "\"",
        rdev::Key::BackSlash => "\"",
        rdev::Key::IntlBackslash => "\"",
        rdev::Key::Comma => ",",
        rdev::Key::Dot => ".",
        rdev::Key::Slash => "/",
        rdev::Key::Insert => "insert",
        rdev::Key::KpReturn => egui_phosphor::regular::ARROW_U_DOWN_LEFT,
        rdev::Key::KpMinus => egui_phosphor::regular::MINUS,
        rdev::Key::KpPlus => egui_phosphor::regular::PLUS,
        rdev::Key::KpMultiply => "*",
        rdev::Key::KpDivide => "\\",
        rdev::Key::Kp0 => "0",
        rdev::Key::Kp1 => "1",
        rdev::Key::Kp2 => "2",
        rdev::Key::Kp3 => "3",
        rdev::Key::Kp4 => "4",
        rdev::Key::Kp5 => "5",
        rdev::Key::Kp6 => "6",
        rdev::Key::Kp7 => "7",
        rdev::Key::Kp8 => "8",
        rdev::Key::Kp9 => "9",
        rdev::Key::KpDelete => "del",
        rdev::Key::Function => "fn",
        rdev::Key::Unknown(_) => egui_phosphor::regular::WARNING,
        rdev::Key::Alt => egui_phosphor::regular::OPTION,
        rdev::Key::AltGr => "altgr",
        rdev::Key::Backspace => egui_phosphor::regular::BACKSPACE,
        rdev::Key::CapsLock => egui_phosphor::regular::ARROW_FAT_LINE_UP,
        rdev::Key::ControlLeft => "ctrlleft",
        rdev::Key::ControlRight => "ctrlright",
        rdev::Key::Delete => "del",
        rdev::Key::DownArrow => egui_phosphor::regular::ARROW_DOWN,
        rdev::Key::End => "end",
        rdev::Key::Escape => "esc",
        rdev::Key::F1 => "Ff1",
        rdev::Key::F10 => "Ff10",
        rdev::Key::F11 => "Ff11",
        rdev::Key::F12 => "Ff12",
        rdev::Key::F2 => "Ff2",
        rdev::Key::F3 => "Ff3",
        rdev::Key::F4 => "Ff4",
        rdev::Key::F5 => "Ff5",
        rdev::Key::F6 => "Ff6",
        rdev::Key::F7 => "F7",
        rdev::Key::F8 => "F8",
        rdev::Key::F9 => "F9",
        rdev::Key::Home => egui_phosphor::regular::HOUSE,
        rdev::Key::LeftArrow => egui_phosphor::regular::ARROW_LEFT,
        rdev::Key::MetaLeft => "metaleft",
        rdev::Key::MetaRight => "metaright",
        rdev::Key::PageDown => "pagedown",
        rdev::Key::PageUp => "pageup",
        rdev::Key::Return => egui_phosphor::regular::ARROW_U_DOWN_LEFT,
        rdev::Key::RightArrow => egui_phosphor::regular::ARROW_RIGHT,
        rdev::Key::ShiftLeft => egui_phosphor::regular::ARROW_FAT_UP,
        rdev::Key::ShiftRight => egui_phosphor::regular::ARROW_FAT_UP,
    }
    .to_string()
}
pub fn button_to_char(b: &rdev::Button) -> String {
    match b {
        rdev::Button::Left => egui_phosphor::regular::MOUSE_LEFT_CLICK.to_string(),
        rdev::Button::Right => egui_phosphor::regular::MOUSE_RIGHT_CLICK.to_string(),
        rdev::Button::Middle => egui_phosphor::regular::MOUSE_MIDDLE_CLICK.to_string(),
        _ => "".to_string(),
    }
}
pub fn scroll_to_char(delta: &Vec2) -> String {
    return if delta.x != 0. {
        egui_phosphor::regular::ARROWS_HORIZONTAL
    } else {
        ""
    }
    .to_string();
}

/// Correctly scales a given time `i` to screen position
pub fn scale(ui: &Ui, i: f32, scale: f32) -> f32 {
    let width = ui.max_rect().size().x;
    let s = 20.0 + scale * 40.0;
    let num_of_digits = width / s;
    let spacing = width / (num_of_digits);
    i * spacing
}

/// Takes a screenshot of the primary monitor and returns it as a `Vec<u8>` in `Rgba` format
pub fn screenshot() -> Option<Vec<u8>> {
    let monitors = Monitor::all().unwrap();
    let monitor = monitors.iter().find(|m| m.is_primary());

    if let Some(monitor) = monitor {
        let image: xcap::image::ImageBuffer<xcap::image::Rgba<u8>, Vec<u8>> =
            monitor.capture_image().unwrap();
        Some(image.into_raw())
    } else {
        None
    }
}

/// Simulate a mouse move accounting for multiple monitors with the offset
pub fn simulate_move(pos: &Vec2, offset: &Vec2) {
    rdev::simulate(&rdev::EventType::MouseMove {
        x: (pos.x + offset.x) as f64,
        y: (pos.y + offset.y) as f64,
    })
    .expect(
        "Failed to simulate Mouse Movement (Probably due to a kernel level anti-cheat running)",
    );
}

use opencv::core::{Mat, MatTraitConst, Point, VecN};

/// Locates the center of a target image within a screenshot using OpenCV template matching
pub fn template_match_opencv(target: DynamicImage) -> Option<Vec2> {
    if let Some(screenshot) = screenshot() {
        let screenshot: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_vec(1920, 1080, screenshot).unwrap();
        let screenshot = DynamicImage::ImageRgba8(screenshot);

        let screenshot_vec = screenshot.to_luma8().to_vec();
        let screenshot_mat = opencv::core::Mat::new_rows_cols_with_bytes::<u8>(
            screenshot.height() as i32,
            screenshot.width() as i32,
            &screenshot_vec,
        )
        .unwrap();

        let target_vec = target.to_luma8().to_vec();
        let target_mat = opencv::core::Mat::new_rows_cols_with_bytes::<u8>(
            target.height() as i32,
            target.width() as i32,
            &target_vec,
        )
        .unwrap();

        let mut output = Mat::default();

        opencv::imgproc::match_template_def(&screenshot_mat, &target_mat, &mut output, 0).unwrap();

        let mut min_val: f64 = 0.0;
        let mut max_val: f64 = 0.0;
        let mut min_loc: Point = Point::new(0, 0);
        let mut max_loc: Point = Point::new(0, 0);

        opencv::core::min_max_loc(
            &output,
            Some(&mut min_val),
            Some(&mut max_val),
            Some(&mut min_loc),
            Some(&mut max_loc),
            &Mat::default(),
        )
        .unwrap();

        let top_left = min_loc;

        let pos = vec2(
            (top_left.x as u32 + target.width() / 2) as f32,
            (top_left.y as u32 + target.height() / 2) as f32,
        );
        // Todo(addis): detect when it failed to find the target image
        return Some(pos);
    }

    None
}
use opencv::{core::AlgorithmHint, imgproc};

/// Calculates the percentage difference between two images
///
/// 0% is an exact match
pub fn image_dif_opencv(vec1: &Vec<u8>, vec2: &Vec<u8>) -> f32 {
    let src1 =
        opencv::core::Mat::new_rows_cols_with_bytes::<VecN<u8, 4>>(1920, 1080, &vec1).unwrap();
    let src2 =
        opencv::core::Mat::new_rows_cols_with_bytes::<VecN<u8, 4>>(1920, 1080, &vec2).unwrap();

    let mut src1x = Mat::default();
    let mut src2x = Mat::default();
    
    imgproc::cvt_color(&src1, &mut src1x, imgproc::COLOR_RGBA2GRAY, 0,AlgorithmHint::ALGO_HINT_DEFAULT ).unwrap();
    imgproc::cvt_color(&src2, &mut src2x, imgproc::COLOR_RGBA2GRAY, 0, AlgorithmHint::ALGO_HINT_DEFAULT).unwrap();

    let mut diff = Mat::default();
    opencv::core::absdiff(&src1x, &src2x, &mut diff).unwrap();

    let result = opencv::core::count_non_zero(&diff).unwrap();
    let size = diff.size().unwrap();
    return (result as f32 / size.area() as f32) * 100.;
}
