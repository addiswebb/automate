use std::time::Instant;

use egui::{emath::RectTransform, vec2, Pos2, Rect, Ui, Vec2};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use xcap::Monitor;

pub const ROW_HEIGHT: f32 = 24.0;

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
        rdev::Key::KeyA => "a".to_string(),
        rdev::Key::KeyB => "b".to_string(),
        rdev::Key::KeyC => "c".to_string(),
        rdev::Key::KeyD => "d".to_string(),
        rdev::Key::KeyE => "e".to_string(),
        rdev::Key::KeyF => "f".to_string(),
        rdev::Key::KeyG => "g".to_string(),
        rdev::Key::KeyH => "h".to_string(),
        rdev::Key::KeyI => "i".to_string(),
        rdev::Key::KeyJ => "j".to_string(),
        rdev::Key::KeyK => "k".to_string(),
        rdev::Key::KeyL => "l".to_string(),
        rdev::Key::KeyM => "m".to_string(),
        rdev::Key::KeyN => "n".to_string(),
        rdev::Key::KeyO => "o".to_string(),
        rdev::Key::KeyP => "p".to_string(),
        rdev::Key::KeyQ => "q".to_string(),
        rdev::Key::KeyR => "r".to_string(),
        rdev::Key::KeyS => "s".to_string(),
        rdev::Key::KeyT => "t".to_string(),
        rdev::Key::KeyU => "u".to_string(),
        rdev::Key::KeyV => "v".to_string(),
        rdev::Key::KeyW => "w".to_string(),
        rdev::Key::KeyX => "x".to_string(),
        rdev::Key::KeyY => "y".to_string(),
        rdev::Key::KeyZ => "z".to_string(),
        rdev::Key::Space => "space".to_string(),
        rdev::Key::Tab => "tab".to_string(),
        rdev::Key::UpArrow => "uparrow".to_string(),
        rdev::Key::PrintScreen => "printscreen".to_string(),
        rdev::Key::ScrollLock => "scrolllock".to_string(),
        rdev::Key::Pause => "pause".to_string(),
        rdev::Key::NumLock => "numlock".to_string(),
        rdev::Key::BackQuote => "`".to_string(),
        rdev::Key::Num1 => "1".to_string(),
        rdev::Key::Num2 => "2".to_string(),
        rdev::Key::Num3 => "3".to_string(),
        rdev::Key::Num4 => "4".to_string(),
        rdev::Key::Num5 => "5".to_string(),
        rdev::Key::Num6 => "6".to_string(),
        rdev::Key::Num7 => "7".to_string(),
        rdev::Key::Num8 => "8".to_string(),
        rdev::Key::Num9 => "9".to_string(),
        rdev::Key::Num0 => "0".to_string(),
        rdev::Key::Minus => "-".to_string(),
        rdev::Key::Equal => "=".to_string(),
        rdev::Key::LeftBracket => "(".to_string(),
        rdev::Key::RightBracket => ")".to_string(),
        rdev::Key::SemiColon => ";".to_string(),
        rdev::Key::Quote => '"'.to_string(),
        rdev::Key::BackSlash => "\"".to_string(),
        rdev::Key::IntlBackslash => "\"".to_string(),
        rdev::Key::Comma => ",".to_string(),
        rdev::Key::Dot => ".".to_string(),
        rdev::Key::Slash => "/".to_string(),
        rdev::Key::Insert => "insert".to_string(),
        rdev::Key::KpReturn => "kpreturn".to_string(),
        rdev::Key::KpMinus => "kpminus".to_string(),
        rdev::Key::KpPlus => "kpplus".to_string(),
        rdev::Key::KpMultiply => "kpmultiply".to_string(),
        rdev::Key::KpDivide => "kpdivide".to_string(),
        rdev::Key::Kp0 => "kp0".to_string(),
        rdev::Key::Kp1 => "kp1".to_string(),
        rdev::Key::Kp2 => "kp2".to_string(),
        rdev::Key::Kp3 => "kp3".to_string(),
        rdev::Key::Kp4 => "kp4".to_string(),
        rdev::Key::Kp5 => "kp5".to_string(),
        rdev::Key::Kp6 => "kp6".to_string(),
        rdev::Key::Kp7 => "kp7".to_string(),
        rdev::Key::Kp8 => "kp8".to_string(),
        rdev::Key::Kp9 => "kp9".to_string(),
        rdev::Key::KpDelete => "kpdelete".to_string(),
        rdev::Key::Function => "function".to_string(),
        rdev::Key::Unknown(_) => "".to_string(),
        rdev::Key::Alt => "alt".to_string(),
        rdev::Key::AltGr => "altgr".to_string(),
        rdev::Key::Backspace => "backspace".to_string(),
        rdev::Key::CapsLock => "capslock".to_string(),
        rdev::Key::ControlLeft => "ctrlleft".to_string(),
        rdev::Key::ControlRight => "ctrlright".to_string(),
        rdev::Key::Delete => "delete".to_string(),
        rdev::Key::DownArrow => "downarrow".to_string(),
        rdev::Key::End => "end".to_string(),
        rdev::Key::Escape => "esc".to_string(),
        rdev::Key::F1 => "f1".to_string(),
        rdev::Key::F10 => "f10".to_string(),
        rdev::Key::F11 => "f11".to_string(),
        rdev::Key::F12 => "f12".to_string(),
        rdev::Key::F2 => "f2".to_string(),
        rdev::Key::F3 => "f3".to_string(),
        rdev::Key::F4 => "f4".to_string(),
        rdev::Key::F5 => "f5".to_string(),
        rdev::Key::F6 => "f6".to_string(),
        rdev::Key::F7 => "f7".to_string(),
        rdev::Key::F8 => "f8".to_string(),
        rdev::Key::F9 => "f9".to_string(),
        rdev::Key::Home => "home".to_string(),
        rdev::Key::LeftArrow => "leftarrow".to_string(),
        rdev::Key::MetaLeft => "metaleft".to_string(),
        rdev::Key::MetaRight => "metaright".to_string(),
        rdev::Key::PageDown => "pagedown".to_string(),
        rdev::Key::PageUp => "pageup".to_string(),
        rdev::Key::Return => "return".to_string(),
        rdev::Key::RightArrow => "rightarrow".to_string(),
        rdev::Key::ShiftLeft => "shiftleft".to_string(),
        rdev::Key::ShiftRight => "shiftright".to_string(),
    }
}
pub fn button_to_char(b: &rdev::Button) -> String {
    match b {
        rdev::Button::Left => "⏴".to_string(),
        rdev::Button::Right => "⏵".to_string(),
        rdev::Button::Middle => "◼".to_string(),
        _ => "".to_string(),
    }
}
pub fn scroll_to_char(delta: &Vec2) -> String {
    return if delta.x != 0. {
        "⬌".to_string()
    } else if delta.y != 0. {
        "⬍".to_string()
    } else {
        "".to_string()
    };
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

pub fn simulate_move(pos: &Vec2, offset: &Vec2) {
    rdev::simulate(&rdev::EventType::MouseMove {
        x: (pos.x + offset.x) as f64,
        y: (pos.y + offset.y) as f64,
    })
    .expect("Failed to simulate Mouse Movement (Probably due to an anti-cheat installed)");
}

#[allow(unused)]
pub fn image_in_image_search(
    target: DynamicImage,
    tolerance: u8,
    min_confidence: f32,
) -> Option<Vec2> {
    let now = Instant::now();
    let step_size = 1;
    // let tolerance = 0;
    // let min_confidence = 0.9;
    let (w, h) = (1920, 1080);
    if let Some(screenshot) = screenshot() {
        let screenshot: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_vec(w, h, screenshot).unwrap();
        let mut screenshot = DynamicImage::ImageRgba8(screenshot);

        let screen_pixels: Vec<_> = screenshot.pixels().map(|p| p.2 .0).collect();
        let target_pixels: Vec<_> = target.pixels().map(|p| p.2 .0).collect();

        for y in (0..h - target.height()).step_by(step_size) {
            for x in (0..w - target.width()).step_by(step_size) {
                let mut matching_pixels = 0;
                let mut total_pixels = 0;
                'outer: for dy in 0..target.height() {
                    for dx in 0..target.width() {
                        let screenshot_index: usize =
                            ((y + dy) * screenshot.width() + (x + dx)) as usize;
                        let target_index: usize = (dy * target.width() + dx) as usize;

                        let screenshot_pixel = screen_pixels[screenshot_index];
                        let target_pixel = target_pixels[target_index];

                        if target_pixel[3] < 128 {
                            continue;
                        }

                        total_pixels += 1;

                        // if screenshot_pixel == target_pixel {
                        if within_tolerance(screenshot_pixel[0], target_pixel[0], tolerance)
                            && within_tolerance(screenshot_pixel[1], target_pixel[1], tolerance)
                            && within_tolerance(screenshot_pixel[2], target_pixel[2], tolerance)
                        {
                            matching_pixels += 1;
                        } else {
                            break 'outer;
                        }
                    }
                }

                let confidence = if total_pixels == 0 {
                    0.0
                } else {
                    matching_pixels as f32 / total_pixels as f32
                };
                if confidence >= min_confidence {
                    screenshot
                        .crop(x, y, target.width() + 20, target.height() + 20)
                        .save("test.png")
                        .unwrap();
                    log::info!(
                        "Magic found {}% move in {:?}",
                        confidence * 100.,
                        now.elapsed()
                    );
                    return Some(vec2(
                        (x + target.width() / 2) as f32,
                        (y + target.height() / 2) as f32,
                    ));
                }
            }
        }
    }

    None
}

// Helper function to check if a color value is within a tolerance range
pub fn within_tolerance(value1: u8, value2: u8, tolerance: u8) -> bool {
    let min_value = value2.saturating_sub(tolerance);
    let max_value = value2.saturating_add(tolerance);
    // Check if the color value is within tolerance range
    value1 >= min_value && value1 <= max_value
}

use opencv::core::{Mat, MatTraitConst, Point, VecN};

pub fn template_match_opencv(target: DynamicImage) -> Option<Vec2> {
    let now = Instant::now();
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

        log::info!("Magic found target in {:?}", now.elapsed());
        let pos = vec2(
            (top_left.x as u32 + target.width() / 2) as f32,
            (top_left.y as u32 + target.height() / 2) as f32,
        );
        // Todo(addis): detect when it failed to find the target image
        return Some(pos);
    }

    None
}

pub fn image_dif_opencv(vec1: &Vec<u8>, vec2: &Vec<u8>) -> f32 {
    let src1 =
        opencv::core::Mat::new_rows_cols_with_bytes::<VecN<u8, 4>>(1920, 1080, &vec1).unwrap();
    let src2 =
        opencv::core::Mat::new_rows_cols_with_bytes::<VecN<u8, 4>>(1920, 1080, &vec2).unwrap();

    let mut src1x = Mat::default();
    let mut src2x = Mat::default();

    opencv::imgproc::cvt_color(&src1, &mut src1x, opencv::imgproc::COLOR_RGBA2GRAY, 0).unwrap();
    opencv::imgproc::cvt_color(&src2, &mut src2x, opencv::imgproc::COLOR_RGBA2GRAY, 0).unwrap();

    let mut diff = Mat::default();
    opencv::core::absdiff(&src1x, &src2x, &mut diff).unwrap();

    let result = opencv::core::count_non_zero(&diff).unwrap();
    let size = diff.size().unwrap();
    return result as f32 / size.area() as f32;
}
