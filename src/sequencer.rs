use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{thread, time::Instant};

use crate::keyframe::{Keyframe, KeyframeType};
use crate::util::*;
use eframe::egui::{self, pos2, Ui, Vec2};
use egui::{vec2, Align2, ColorImage, FontId, TextureHandle};
use egui::{Pos2, Rect};
use serde::{Deserialize, Serialize};
use uuid::{Bytes, Uuid};

#[derive(Debug, Serialize, Deserialize)]
pub struct SequencerState {
    pub repeats: i32,
    pub speed: f32,
    pub keyframes: Vec<Keyframe>,
    // pub images: HashMap<Bytes, Vec<u8>>,
}
/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(Deserialize, Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct Sequencer {
    #[serde(skip)]
    pub changed: Arc<AtomicBool>,
    #[serde(skip)]
    should_sort: bool,
    #[serde(skip)]
    dragging: bool,
    #[serde(skip)]
    drag_start: Pos2,
    #[serde(skip)]
    selecting: bool,
    #[serde(skip)]
    selection: Rect,
    #[serde(skip)]
    resizing: bool,
    #[serde(skip)]
    pub keyframes: Arc<Mutex<Vec<Keyframe>>>,
    #[serde(skip)]
    pub selected_keyframes: Vec<uuid::Bytes>,
    #[serde(skip)]
    pub keyframe_state: Arc<Mutex<Vec<usize>>>,
    scale: f32, // egui coord points:seconds
    repeats: i32,
    speed: f32,
    #[serde(skip)]
    scroll: f32,
    #[serde(skip)]
    time: f32,
    #[serde(skip)]
    prev_time: f32,
    #[serde(skip)]
    play: Arc<AtomicBool>,
    pub mouse_movement_record_resolution: Arc<AtomicI32>,
    #[serde(skip)]
    pub recording: Arc<AtomicBool>,
    #[serde(skip)]
    was_recording: bool,
    clear_before_recording: bool,
    #[serde(skip)]
    recording_instant: Arc<Mutex<Instant>>,
    #[serde(skip)]
    pub loaded_file: String,
    #[serde(skip)]
    pub clip_board: Vec<Keyframe>,
    #[serde(skip)]
    once_bool: bool,
    #[serde(skip)]
    pub calibrate: Arc<AtomicBool>,
    #[serde(skip)]
    current_image: Option<TextureHandle>,
    #[serde(skip)]
    current_image_uid: Bytes,
    #[serde(skip)]
    pub images: Arc<Mutex<HashMap<Bytes, Vec<u8>>>>,
}

impl Sequencer {
    /// Creates a new sequencer
    ///
    /// Also manages creating the keystrokes recording thread
    pub fn new() -> Self {
        let keyframes: Arc<Mutex<Vec<Keyframe>>> = Arc::new(Mutex::new(vec![]));
        let keyframe_state = Arc::new(Mutex::new(vec![]));
        let recording = Arc::new(AtomicBool::new(false));
        let play = Arc::new(AtomicBool::new(false));
        let mouse_movement_record_resolution = Arc::new(AtomicI32::new(20));
        let recording_instant = Arc::new(Mutex::new(Instant::now()));
        let changed = Arc::new(AtomicBool::new(false));
        let calibrate = Arc::new(AtomicBool::new(false));
        let images = Arc::new(Mutex::new(HashMap::new()));

        let shared_kfs = Arc::clone(&keyframes);
        let shared_pkfs = Arc::clone(&keyframe_state);
        let shared_rec = Arc::clone(&recording);
        let shared_play = Arc::clone(&play);
        let shared_count = Arc::clone(&mouse_movement_record_resolution);
        let shared_instant = Arc::clone(&recording_instant);
        let shared_changed = Arc::clone(&changed);
        let shared_calibrate = Arc::clone(&calibrate);
        let shared_images = Arc::clone(&images);

        let mut was_recording = false;

        // Todo(addis): combine key and mouse vecs into one
        let mut mouse_keyframes = vec![];
        let mut key_keyframes = vec![];

        let mut previous_mouse_position = Vec2::ZERO;
        // this needs to get reset every time recording starts
        let mut mouse_move_count = 20;

        // Spawn the recording thread
        let _ = thread::Builder::new()
            .name("Record Thread".to_owned())
            .spawn(move || {
                log::info!("Created Recording Thread");
                if let Err(error) = rdev::listen(move |event: rdev::Event| {
                    // Offset Calibration
                    if shared_calibrate.load(Ordering::Relaxed) {
                        match &event.event_type {
                            rdev::EventType::MouseMove { x, y } => {
                                shared_kfs.lock().unwrap().push(Keyframe {
                                    timestamp: f32::NAN,
                                    duration: f32::NAN,
                                    keyframe_type: KeyframeType::MouseMove(Vec2::new(
                                        *x as f32, *y as f32,
                                    )),
                                    kind: u8::MAX, // This is code to say the keyframe is for calibration only and must be deleted after use
                                    uid: Uuid::nil().to_bytes_le(),
                                });
                            }
                            _ => {}
                        }
                    } else {
                        let is_recording = shared_rec.load(Ordering::Relaxed);
                        let mut tmp_keyframe = None;
                        let dt = Instant::now().duration_since(*shared_instant.lock().unwrap());
                        // Handle global keybinds without focus
                        match &event.event_type {
                            rdev::EventType::KeyRelease(key) => {
                                match key {
                                    // Keybind(F8): Toggle recording
                                    rdev::Key::F8 => {
                                        if is_recording {
                                            shared_rec.swap(false, Ordering::Relaxed);
                                        } else {
                                            shared_rec.swap(true, Ordering::Relaxed);
                                            key_keyframes = vec![];
                                            mouse_move_count = 20;
                                            previous_mouse_position = Vec2::ZERO;
                                        }
                                    }
                                    // Keybind(esc): Toggle play execution
                                    rdev::Key::Escape => {
                                        shared_play.swap(false, Ordering::Relaxed);
                                    }
                                    // Keybind(F9): Manually add a mouse move keyframe (can be used for filling in missed movements due to record resolution)
                                    rdev::Key::F9 => {
                                        tmp_keyframe = Some(Keyframe::mouse_move(
                                            dt.as_secs_f32(),
                                            previous_mouse_position,
                                        ))
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                        if is_recording && tmp_keyframe.is_none() {
                            // Checks if there are no keyframes (Would only be the case if a new recording has started and there is no start screenshot)
                            if !was_recording {
                                // START
                                // Todo(addis): record start keyframe and save it somewhere
                            }
                            tmp_keyframe = match &event.event_type {
                                // Button & Key Press events just push info
                                rdev::EventType::ButtonPress(btn) => {
                                    let keyframe =
                                        Keyframe::mouse_button(dt.as_secs_f32(), 0., btn.clone());
                                    if let Some(screenshot) = screenshot() {
                                        shared_images
                                            .lock()
                                            .unwrap()
                                            .insert(keyframe.uid, screenshot);
                                    }
                                    mouse_keyframes.push(keyframe);
                                    None
                                }
                                rdev::EventType::KeyPress(key) => {
                                    let keyframe =
                                        Keyframe::key_btn(dt.as_secs_f32(), 0., key.clone());
                                    if let Some(screenshot) = screenshot() {
                                        shared_images
                                            .lock()
                                            .unwrap()
                                            .insert(keyframe.uid, screenshot);
                                    }
                                    key_keyframes.push(keyframe);
                                    None
                                }
                                // Button & Key Release events search for the matching keypress event to create a full keyframe
                                rdev::EventType::ButtonRelease(btn) => {
                                    let index = mouse_keyframes.iter().position(|kf| {
                                        if let KeyframeType::MouseBtn(b) = kf.keyframe_type {
                                            b == *btn
                                        } else {
                                            false
                                        }
                                    });
                                    match index {
                                        Some(index) => {
                                            let mut keyframe = mouse_keyframes.remove(index);
                                            keyframe.calculate_duration(dt.as_secs_f32());
                                            Some(keyframe)
                                        }
                                        None => {
                                            println!("Failed to find button release");
                                            None
                                        }
                                    }
                                }
                                rdev::EventType::KeyRelease(key) => {
                                    let index = key_keyframes.iter().position(|kf| {
                                        if let KeyframeType::KeyBtn(k) = kf.keyframe_type {
                                            k == *key
                                        } else {
                                            false
                                        }
                                    });
                                    match index {
                                        Some(index) => {
                                            let mut keyframe = key_keyframes.remove(index);
                                            keyframe.calculate_duration(dt.as_secs_f32());
                                            Some(keyframe)
                                        }
                                        None => {
                                            println!("Failed to find key release");
                                            None
                                        }
                                    }
                                }
                                rdev::EventType::MouseMove { x, y } => {
                                    let pos = Vec2::new(*x as f32, *y as f32);
                                    mouse_move_count -= 1;
                                    match previous_mouse_position == pos {
                                        false => match mouse_move_count <= 0 {
                                            true => {
                                                previous_mouse_position = pos;
                                                mouse_move_count =
                                                    shared_count.load(Ordering::Relaxed);
                                                Some(Keyframe::mouse_move(dt.as_secs_f32(), pos))
                                            }
                                            false => None,
                                        },
                                        true => None,
                                    }
                                }
                                rdev::EventType::Wheel { delta_x, delta_y } => {
                                    match *delta_x == 0 && *delta_y == 0 {
                                        true => None,
                                        false => Some(Keyframe::scroll(
                                            dt.as_secs_f32(),
                                            Vec2::new(*delta_x as f32, *delta_y as f32),
                                        )),
                                    }

                                    // Todo(addis): create and save a screenshot here
                                }
                            };
                            // If a keyframe was created push the necessary data to sequencer
                            if let Some(keyframe) = tmp_keyframe {
                                shared_kfs.lock().unwrap().push(keyframe);
                                shared_pkfs.lock().unwrap().push(0);
                                shared_changed.swap(true, Ordering::Relaxed);
                            }
                        }
                        was_recording = is_recording;
                    }
                }) {
                    log::error!("Error: {:?}", error)
                }
            });
        Self {
            keyframes,
            changed,
            should_sort: false,
            drag_start: pos2(0., 0.),
            dragging: false,
            selection: Rect::ZERO,
            selecting: false,
            resizing: false,
            scale: 0.01,
            repeats: 1,
            speed: 1.0,
            scroll: 0.0,
            time: 0.0,
            prev_time: 0.0,
            play,
            mouse_movement_record_resolution,
            selected_keyframes: vec![],
            keyframe_state,
            recording,
            clear_before_recording: true,
            was_recording: false,
            recording_instant,
            loaded_file: "".to_string(),
            clip_board: vec![],
            once_bool: false,
            calibrate,
            current_image: None,
            current_image_uid: Uuid::nil().to_bytes_le(),
            images,
        }
    }

    /// Returns the current time where the playhead is
    pub fn get_time(&self) -> f32 {
        self.time
    }
    /// Saves the current state of the sequencer to `SequencerState`
    pub fn save_to_state(&self) -> SequencerState {
        SequencerState {
            repeats: self.repeats,
            speed: self.speed,
            keyframes: self.keyframes.lock().unwrap().clone(),
            // images: self.images.lock().unwrap().clone(),
        }
    }
    /// Loads the sequencer with the `SequencerState`
    pub fn load_from_state(&mut self, state: SequencerState) {
        let mut shared_kfs = self.keyframes.lock().unwrap();
        let mut shared_pkfs = self.keyframe_state.lock().unwrap();
        shared_kfs.clear();
        shared_kfs.extend(state.keyframes.into_iter());
        shared_pkfs.clear();
        shared_pkfs.extend(vec![0; shared_kfs.len()].into_iter());
        self.speed = state.speed;
        self.repeats = state.repeats;
    }
    /// Toggles whether the sequencer is playing or not
    pub fn toggle_play(&mut self) {
        self.play
            .swap(!self.play.load(Ordering::Relaxed), Ordering::Relaxed);
    }
    /// Reset the time and playhead to 0 seconds
    pub fn reset_time(&mut self) {
        self.time = 0.;
    }
    /// Increase the current time by 0.1 seconds
    pub fn step_time(&mut self) {
        self.time += 0.1;
    }
    /// Increases the scale of the keyframes to zoom in
    pub fn zoom(&mut self, delta: f32) {
        let multiplier = 1.0 / 100.0;
        self.scale = (self.scale + delta * multiplier).clamp(0.01, 10.0);
    }
    /// Scrolls through the keyframes
    pub fn scroll(&mut self, delta: f32) {
        let multiplier = 1.0 / 80.0;
        self.scroll = (self.scroll + delta * multiplier / self.scale).clamp(0., f32::INFINITY);
    }
    /// Copy the selected keyframes to clipboard
    pub fn copy(&mut self) {
        let keyframes = self.keyframes.lock().unwrap();
        if !self.selected_keyframes.is_empty() {
            self.clip_board.clear();
            let now = Instant::now();
            for i in 0..keyframes.len() {
                let x = self.selected_keyframes.binary_search(&keyframes[i].uid);
                if x.is_ok() {
                    self.clip_board.push(keyframes[i].clone());
                }
            }
            println!("{:?}", now.elapsed());
        }
    }
    ///Paste the clipboard
    pub fn paste(&mut self) {
        // Todo(addis): this only copies the keyframes, not their respective images (easy fix but idk brev)
        if !self.clip_board.is_empty() {
            let mut images = self.images.lock().unwrap();

            // Selected keyframes will be reset and then filled with the new keyframes
            self.selected_keyframes.clear();
            // Used to update the state for new keyframes
            let mut keyframe_state = self.keyframe_state.lock().unwrap();

            let mut clip_board: Vec<Keyframe> = self
                .clip_board
                .clone()
                .into_iter()
                .map(|mut kf| {
                    // Shift them all forward slightly so its clear what has been copied
                    kf.timestamp += 1.;
                    // Change the UIDs for the copied keyframes
                    let new_uid = Uuid::new_v4().to_bytes_le();
                    // Check if the keyframe had an image, clone it with the new UID if so
                    if let Some(image) = images.get(&kf.uid).cloned() {
                        images.insert(new_uid, image);
                    }
                    // Update the UID so there are no duplicates
                    kf.uid = new_uid;

                    // Use the new UUIDs as the currently selected keyframes
                    self.selected_keyframes.push(new_uid);
                    keyframe_state.push(0);
                    kf
                })
                .collect();

            self.keyframes.lock().unwrap().append(&mut clip_board);
            // since the keyframes array has changed, it should be resorted
            self.should_sort = true;
        }
    }
    /// Copy the selected keyframes to clipboard and delete them from the keyframes vec
    pub fn cut(&mut self) {
        let mut keyframes = self.keyframes.lock().unwrap();
        self.clip_board.clear();

        for i in (0..keyframes.len()).rev() {
            let x = self.selected_keyframes.binary_search(&keyframes[i].uid);
            if x.is_ok() {
                self.clip_board.push(keyframes[i].clone());
                keyframes.remove(i);
                self.keyframe_state.lock().unwrap().remove(i);
            }
        }
        // Since the clipboard starts empty, if it isnt now that means keyframes were copied and then removed

        if !self.clip_board.is_empty() {
            self.changed.swap(true, Ordering::Relaxed);
        }
        self.selected_keyframes.clear();
    }
    /// Select all keyframes
    pub fn select_all(&mut self) {
        self.keyframes.lock().unwrap().iter().for_each(|kf| {
            self.selected_keyframes.push(kf.uid);
        });
    }
    /// Toggle whether the sequencer is recording keystrokes or not
    ///
    /// * When starting recording: If `clear_before_recording` is `true`, reset the sequencer and record from 0 seconds
    /// otherwise record from where the sequencer left off
    ///
    /// * When stopping recording: Remove the last input if it was used to stop the recording (mouse pressing the stop button)
    pub fn toggle_recording(&mut self) {
        if !self.recording.load(Ordering::Relaxed) {
            let mut keyframes = self.keyframes.lock().unwrap();
            let mut keyframe_state = self.keyframe_state.lock().unwrap();
            let last = keyframes.last();
            if let Some(last) = last {
                if (last.timestamp + last.duration - self.time).abs() <= 0.04 {
                    let is_record_stop_keyframe = match last.keyframe_type {
                        KeyframeType::KeyBtn(rdev::Key::F8) => true,
                        KeyframeType::MouseBtn(rdev::Button::Left) => true,
                        _ => false,
                    };
                    if is_record_stop_keyframe {
                        keyframes.pop();
                        keyframe_state.pop();
                        // END
                        screenshot();
                    }
                }
                std::mem::drop(keyframes);
                std::mem::drop(keyframe_state);
                if self.clear_before_recording {
                    self.time = 0.;
                }
                log::info!("Stop Recording");
            }
        } else {
            let mut rec_instant = self.recording_instant.lock().unwrap();
            if self.clear_before_recording {
                self.time = 0.;
                self.keyframes.lock().unwrap().clear();
                self.keyframe_state.lock().unwrap().clear();
                let _ = std::mem::replace(&mut *rec_instant, Instant::now());
            } else {
                let _ = std::mem::replace(
                    &mut *rec_instant,
                    Instant::now() - Duration::from_secs_f32(self.time),
                );
            }
            log::info!("Start Recording");
        }
    }
    /// Loops through all the sequencer's keyframes and renders them accordingly
    ///
    /// Also handles deleting keyframes due to convenience
    fn render_keyframes(&mut self, ui: &mut Ui, max_rect: &Rect) {
        let mut keyframes = self.keyframes.lock().unwrap();
        let mut keyframe_state = self.keyframe_state.lock().unwrap();
        let mut delete = false;
        let mut cut = false;
        for i in 0..keyframes.len() {
            let offset_y = ui.spacing().item_spacing.y;
            // Determine which row to draw the keframe on depending on its type
            let y = match keyframes[i].kind {
                0 => offset_y,
                1 => ROW_HEIGHT * 2. + 9.,
                2 => ROW_HEIGHT + offset_y * 2.,
                3 => ROW_HEIGHT + offset_y * 2.,
                _ => 0.,
            };
            // Calculate the rect for the keyframe
            let rect = time_to_rect(
                scale(ui, keyframes[i].timestamp, self.scale) - scale(ui, self.scroll, self.scale)
                    + 4.0,
                scale(ui, keyframes[i].duration, self.scale),
                scale(
                    ui,
                    max_rect.width() * (1.0 / scale(ui, 1.0, self.scale)),
                    self.scale,
                ),
                ui.spacing().item_spacing,
                max_rect.translate(vec2(0., y)),
            );
            // Time_to_rect clips all keyframes that are not visible for performance, this skips them
            if let Some(rect) = rect {
                // Used to determine different interactions with keyframes
                let ctrl = ui.input(|i| i.modifiers.ctrl);
                // Handle when the user is drag selecting over keyframes
                if self.selecting {
                    if selection_contains_keyframe(&self.compute_selection_rect(&max_rect), rect) {
                        match self.selected_keyframes.binary_search(&keyframes[i].uid) {
                            Ok(_) => {}
                            Err(index) => {
                                self.selected_keyframes.insert(index, keyframes[i].uid);
                            }
                        }
                    } else {
                        if !ctrl {
                            match self.selected_keyframes.binary_search(&keyframes[i].uid) {
                                Ok(index) => {
                                    self.selected_keyframes.remove(index);
                                }
                                Err(_) => {}
                            }
                        }
                    }
                }

                let color = match keyframes[i].kind {
                    0 => egui::Color32::LIGHT_RED,              //Keyboard
                    1 => egui::Color32::from_rgb(95, 186, 213), //Mouse move
                    2 => egui::Color32::LIGHT_GREEN,            //Button Click
                    3 => egui::Color32::LIGHT_YELLOW,           //Scroll
                    _ => egui::Color32::LIGHT_GRAY,
                };
                let stroke = match keyframe_state[i] {
                    1 => egui::Stroke::new(1.5, egui::Color32::LIGHT_RED), //Playing
                    2 => egui::Stroke::new(1.5, egui::Color32::from_rgb(233, 181, 125)), //Selected
                    _ => egui::Stroke::new(0.4, egui::Color32::from_rgb(15, 37, 42)), //Not selected
                };

                let keyframe = ui
                    .allocate_rect(rect, egui::Sense::click_and_drag())
                    .on_hover_text(format!("{:?}", keyframes[i].keyframe_type));
                ui.painter()
                    .rect(rect, egui::Rounding::same(2.0), color, stroke);

                // Checks if it is worth displaying a label for the keyframe based of its width
                if rect.width() > 10.0 {
                    let label = format!(
                        "{}",
                        match &keyframes[i].keyframe_type {
                            KeyframeType::KeyBtn(key) => key_to_char(key),
                            KeyframeType::MouseBtn(btn) => button_to_char(btn),
                            KeyframeType::MouseMove(_) => "".to_string(),
                            KeyframeType::Scroll(delta) => scroll_to_char(delta),
                            KeyframeType::Wait(secs) => format!("{}s", secs).to_string(),
                        }
                    );
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        format!("{}", label),
                        FontId::default(),
                        egui::Color32::BLACK,
                    );
                }
                // Handles the user clicking a keyframe
                if keyframe.clicked() {
                    // Check whether there was more than one keyframe selected before clearing the vec, (used for edgecase)
                    let was_empty = self.selected_keyframes.is_empty();
                    // Attempt to find the selected keyframe using its uuid
                    let x = self.selected_keyframes.binary_search(&keyframes[i].uid);
                    // If not ctrl clicked, only a single keyframe can ever be selected so we clear the vec early
                    if !ctrl {
                        self.selected_keyframes.clear();
                    }
                    match x {
                        // Already selected
                        Ok(index) => {
                            // If ctrl clicked while already selected, deselect it (note that the vec is not emptied because ctrl was not pressed)
                            if ctrl {
                                self.selected_keyframes.remove(index);
                            }
                            // If ctrl was not pressed, and one of several already selected keyframes was clicked, leave only that one selected (note that vec is empty here)
                            if was_empty {
                                println!("test");
                                self.selected_keyframes.push(keyframes[i].uid)
                            }
                        }
                        // not already selected
                        Err(index) => {
                            if !ctrl {
                                // If not already selected, select it and push (note we use push instead of insert here because the vec is empty and it will be placed at index 0 by default)
                                self.selected_keyframes.push(keyframes[i].uid)
                            } else {
                                // If ctrl is pressed, then insert the keyframe while keeping order (note we need a sorted vec to allow for binary search later on)
                                self.selected_keyframes.insert(index, keyframes[i].uid)
                            }
                        }
                    }
                }
                // Todo(addis): change sense to drag only,  not click_and_drag
                // Todo(addis): then sense clicks as a drag without displacement, to remove the small delay between physical and electronic drag start
                // Todo(addis): maybe not, since it will interfere with the different actions taken when selecting keyframes dependant on if its a click or drag event
                // Handles the user starting to drag a keyframe
                if keyframe.drag_started() {
                    if let Some(start) = keyframe.interact_pointer_pos() {
                        self.drag_start = start;
                        self.dragging = true;
                        let ctrl = ui.input(|i| {
                            return i.modifiers.ctrl;
                        });
                        // Attempt to find the selected keyframe using its uuid
                        match self.selected_keyframes.binary_search(&keyframes[i].uid) {
                            // Already selected
                            Ok(_) => { /* Do nothing */ }
                            // Not already selected
                            Err(index) => {
                                if !ctrl {
                                    // If not already selected, drag only this keyframe
                                    self.selected_keyframes = vec![keyframes[i].uid]
                                } else {
                                    // If ctrl is pressed, then add it to the selected keyframes and drag them all
                                    self.selected_keyframes.insert(index, keyframes[i].uid)
                                }
                            }
                        }
                    }
                }
                // Handles the user hovering a keyframe
                if keyframe.hovered() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                }
                // Handles the user dragging a keyframe
                if self.dragging {
                    if let Some(end) = keyframe.interact_pointer_pos() {
                        let drag_delta =
                            (end.x - self.drag_start.x) * (1.0 / scale(ui, 1.0, self.scale));
                        let t = keyframes[i].timestamp + drag_delta;
                        if t > 0.0 {
                            for j in 0..keyframe_state.len() {
                                if keyframe_state[j] == 2 {
                                    keyframes[j].timestamp += drag_delta;
                                }
                            }
                            self.drag_start.x = end.x;
                            self.changed.swap(true, Ordering::Relaxed);
                        }
                    }
                }
                // Resets drag variables when user stops dragging
                if keyframe.drag_stopped() {
                    self.drag_start = pos2(0., 0.);
                    self.dragging = false;
                    self.resizing = false;
                    // Since there is a chance that the chronological order of the keyframes has changed,
                    // we need to update the keyframes vec to match the new order
                    self.should_sort = true;
                }

                ui.input_mut(|input| {
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::Delete) {
                        delete = true;
                    }
                });
                // let uid = keyframes[i].uid;
                keyframe.context_menu(|ui| {
                    // Right clicking a keyframe does not guarrantee that it is selected, so we make sure here
                    let index = self.selected_keyframes.binary_search(&keyframes[i].uid);
                    if let Err(index) = index {
                        self.selected_keyframes.insert(index, keyframes[i].uid);
                    }
                    // add the current one somehow
                    if ui
                        .add(egui::Button::new("Cut").shortcut_text("Ctrl+X"))
                        .clicked()
                    {
                        cut = true;
                        ui.close_menu();
                    }
                    if ui
                        .add(egui::Button::new("Copy").shortcut_text("Ctrl+C"))
                        .clicked()
                    {
                        if !self.selected_keyframes.is_empty() {
                            self.clip_board.clear();
                            // Loop through all keyframes
                            for i in 0..keyframes.len() {
                                // Check if the current keyframe is selected
                                let x = self.selected_keyframes.binary_search(&keyframes[i].uid);
                                if x.is_ok() {
                                    // Add to clip_board if so..
                                    self.clip_board.push(keyframes[i].clone());
                                }
                            }
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add(egui::Button::new("Paste").shortcut_text("Ctrl+V"))
                        .clicked()
                    {
                        if !self.clip_board.is_empty() {
                            // Clone the clip_board to update the keyframes with new UUIDs
                            let mut clip_board: Vec<Keyframe> = self
                                .clip_board
                                .clone()
                                .into_iter()
                                .map(|mut kf| {
                                    // Shift them all forward slightly so its clear what has been copied
                                    kf.timestamp += 0.1;
                                    // Change the UUIDs for the copied keyframes
                                    kf.uid = uuid::Uuid::new_v4().to_bytes_le();
                                    kf
                                })
                                .collect();
                            // Use the new UUIDs as the currently selected keyframes
                            self.selected_keyframes.clear();
                            for i in 0..clip_board.len() {
                                self.selected_keyframes.push(clip_board[i].uid);
                            }
                            // Update keyframe_state to be all unselected (this will be updated with the new selected_keyframes later)
                            self.keyframe_state
                                .lock()
                                .unwrap()
                                .append(&mut vec![0; clip_board.len()]);
                            // Finally paste the copied keyframes
                            keyframes.append(&mut clip_board);
                            // Since there is a chance that the chronological order of the keyframes has changed,
                            // we need to update the keyframes vec to match the new order
                            self.should_sort = true;
                        }
                        ui.close_menu();
                    }
                    if ui.button("Delete").clicked() {
                        delete = true;
                        ui.close_menu();
                    }
                });
            }
        }
        // Same as copy also deletes
        if cut {
            self.clip_board.clear();
            for uid in &self.selected_keyframes {
                let mut index = 0;
                for i in 0..keyframes.len() {
                    if keyframes[i].uid == *uid {
                        index = i;
                        self.clip_board.push(keyframes[i].clone());
                        break;
                    }
                }
                keyframes.remove(index);
                keyframe_state.remove(index);
            }
            self.selected_keyframes.clear();
        }
        // If there are keyframes selected to delete
        if delete && !self.selected_keyframes.is_empty() {
            let number_of_selected_keyframes = self.selected_keyframes.len();
            let number_of_keyframes = keyframe_state.len();
            // Sort the selected list from least the greatest index
            self.selected_keyframes.sort();
            self.selected_keyframes.reverse();
            // Check if we can do a quick delete if every keyframe is selected
            if number_of_keyframes == number_of_selected_keyframes {
                keyframes.clear();
                keyframe_state.clear();
                self.selected_keyframes.clear();
            } else {
                // Otherwise loop through keyframes and remove from last to first (avoids index out of bounds)
                let mut last_index = 0;
                for uid in &self.selected_keyframes {
                    let mut index = 0;
                    for i in 0..keyframes.len() {
                        if keyframes[i].uid == *uid {
                            index = i;
                            break;
                        }
                    }
                    keyframes.remove(index);
                    keyframe_state.remove(index);
                    last_index = index;
                }
                // If there are still keyframes left, we want to select the last one before the selection
                if !keyframes.is_empty() {
                    // If the last keyframe before selection was the very last keyframe then we get the second last
                    if last_index == keyframes.len() {
                        last_index -= 1;
                    }
                    self.selected_keyframes = vec![keyframes[last_index].uid];
                }
            }

            self.changed.swap(true, Ordering::Relaxed);
        }
    }
    /// Handles rendering the control bar
    fn render_control_bar(&mut self, ui: &mut Ui) {
        if ui.button("⏪").on_hover_text("Restart").clicked() {
            self.reset_time();
        }
        if ui.button("⏴").on_hover_text("Reverse").clicked() {}
        if self.play.load(Ordering::Relaxed) {
            if ui.button("⏸").on_hover_text("Pause").clicked() {
                self.toggle_play();
            }
        } else {
            if ui.button("⏵").on_hover_text("Play").clicked() {
                self.toggle_play();
            }
        }
        if ui.button("⏩").on_hover_text("Step").clicked() {
            self.step_time();
        }
        ui.add(
            egui::DragValue::new(&mut self.time)
                .range(0.0..=(60.0 * 60.0 * 10.0))
                .speed(0.100)
                .custom_formatter(|n, _| {
                    let mins = ((n / 60.0) % 60.0).floor() as i32;
                    let secs = (n % 60.0) as i32;
                    let milis = ((n * 1000.0) % 1000.0).floor() as i32;
                    format!("{mins:02}:{secs:02}:{milis:03}")
                })
                .custom_parser(|s| {
                    if s.contains(':') {
                        let parts: Vec<&str> = s.split(':').collect();
                        if parts.len() == 2 {
                            parts[0]
                                .parse::<f32>()
                                .and_then(|m| {
                                    parts[1].parse::<f32>().map(|s| ((m * 60.0) + s) as f64)
                                })
                                .ok()
                        } else {
                            None
                        }
                    } else {
                        s.parse::<f64>().ok()
                    }
                }),
        )
        .on_hover_text("Time");

        ui.add(
            egui::DragValue::new(&mut self.repeats)
                .speed(1)
                .range(1..=10000),
        )
        .on_hover_text("Number of repeats");
        ui.add(
            egui::DragValue::new(&mut self.speed)
                .speed(1)
                .suffix("x")
                .range(1.0..=20.0),
        )
        .on_hover_text("Playback Speed");

        if self.recording.load(Ordering::Relaxed) {
            if ui.button("⏹").on_hover_text("Stop Recording: F8").clicked() {
                self.recording.swap(false, Ordering::Relaxed);
            }
        } else {
            if ui
                .button(egui::RichText::new("⏺").color(egui::Color32::LIGHT_RED))
                .on_hover_text("Start Recording: F8")
                .clicked()
            {
                self.recording.swap(true, Ordering::Relaxed);
            }
        }
    }
    /// Render the timeline with numbers and notches
    fn render_timeline(&self, ui: &mut Ui, max_rect: Rect) {
        let pos = time_to_rect(4.0, 0.0, 0.0, ui.spacing().item_spacing, max_rect)
            .unwrap()
            .min;
        //offset so that the left most digit is fully visible += 4.0;
        let (_, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click());
        for i in self.scroll as i32
            ..(max_rect.width() * (1.0 / scale(ui, 1.0, self.scale)) + self.scroll).ceil() as i32
        {
            let sizing = i.checked_ilog10().unwrap_or(0);
            let (font_size, y_offset) = match sizing {
                3 => (8., 4.),
                2 => (10., 2.),
                _ => (12., 0.),
            };
            let point =
                pos + vec2(scale(ui, i as f32 - self.scroll, self.scale), 0.0) + vec2(0., y_offset);
            painter.text(
                point,
                egui::Align2::CENTER_TOP,
                format!("{}", i),
                egui::FontId::monospace(font_size),
                // egui::FontId::monospace(12.0),
                egui::Color32::GRAY,
            );
            painter.line_segment(
                [
                    pos2(point.x, max_rect.max.y),
                    pos2(point.x, max_rect.max.y) + egui::vec2(0.0, -6.0),
                ],
                egui::Stroke::new(1.0, egui::Color32::GRAY),
            );
        }
    }
    /// Render the playhead at whatever time the sequencer is at
    fn render_playhead(&mut self, ui: &mut Ui, rows: i32, rect: Rect) {
        let point = time_to_rect(
            scale(ui, self.time - self.scroll, self.scale) + 3., //add 3. for offset to allow left most digit to always be visible
            0.0,
            0.0,
            ui.spacing().item_spacing,
            rect,
        )
        .unwrap()
        .min;
        let p1 = pos2(point.x + 1., point.y - 2.);
        let p2 = pos2(p1.x, p1.y + ROW_HEIGHT * rows as f32 + (3 * rows) as f32);
        let padding = 3.0;
        let playhead = ui.allocate_rect(
            Rect {
                min: pos2(p1.x - padding, p1.y - padding),
                max: pos2(p1.x + padding, p2.y + padding),
            },
            egui::Sense::click_and_drag(),
        );
        if playhead.hovered() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
        }
        if playhead.drag_started() {
            if let Some(start) = playhead.interact_pointer_pos() {
                self.drag_start = start;
                self.dragging = true;
            }
        }
        if self.dragging {
            if let Some(end) = playhead.interact_pointer_pos() {
                if end.x > rect.min.x {
                    let drag_delta =
                        (end.x - self.drag_start.x) * (1.0 / scale(ui, 1.0, self.scale));
                    let t = self.time + drag_delta;
                    if t > 0.0 {
                        self.time = t;
                        self.drag_start.x = end.x;
                    }
                } else {
                    self.time = 0.;
                }
            }
        }
        if playhead.drag_stopped() {
            self.drag_start = pos2(0., 0.);
            self.dragging = false;
        }
        // clip the playhead so it is not visible when off the timeline
        let painter = ui.painter().with_clip_rect(rect.expand2(vec2(0., 4.0)));
        painter.text(
            p1 - egui::vec2(0.0, 3.0),
            egui::Align2::CENTER_TOP,
            "⏷",
            egui::FontId::monospace(10.0),
            egui::Color32::LIGHT_RED,
        );
        painter.line_segment([p1, p2], egui::Stroke::new(1.0, egui::Color32::LIGHT_RED));
    }
    /// Render the whole sequencer ui
    ///
    /// Handles the controlbar, timeline, playhead and keyframes
    pub fn show(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("Sequencer").show(ctx, |ui| {
            use egui_extras::{Column, TableBuilder};

            let mut max_rect = ui
                .max_rect()
                .translate(vec2(6.5, 0.))
                .translate(vec2(0., (ROW_HEIGHT + ui.spacing().item_spacing.y) * 2.));

            max_rect.max.y = max_rect.min.y + (ROW_HEIGHT) * 4. + ui.spacing().item_spacing.y;
            max_rect.min.x += 60.;
            max_rect.max.y -= ROW_HEIGHT;

            let mut table = TableBuilder::new(ui)
                .striped(false)
                .resizable(false)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::initial(60.0).range(60.0..=60.0))
                .column(Column::remainder())
                .drag_to_scroll(false)
                .sense(egui::Sense::hover());
            //allow rows to be clicked
            table = table.sense(egui::Sense::click()).resizable(true);

            table
                .header(ROW_HEIGHT, |mut header| {
                    header.col(|ui| {
                        ui.strong("Inputs");
                    });
                    header.col(|ui| {
                        self.render_control_bar(ui);
                    });
                })
                .body(|mut body| {
                    body.row(ROW_HEIGHT, |mut row| {
                        row.set_hovered(true);
                        row.col(|_| {});
                        row.col(|ui| {
                            let rect = ui.max_rect();
                            self.render_timeline(
                                ui,
                                Rect {
                                    min: pos2(max_rect.min.x, rect.min.y),
                                    max: pos2(max_rect.max.x, rect.max.y),
                                },
                            );
                        });
                    });
                    body.row(ROW_HEIGHT, |mut row| {
                        row.col(|ui| {
                            ui.label("Keyboard").on_hover_text("id: 0");
                        });
                        row.col(|ui| {
                            self.sense(ui);
                        });
                    });

                    body.row(ROW_HEIGHT, |mut row| {
                        row.col(|ui| {
                            ui.label("Mouse").on_hover_text("id: 2,3");
                        });
                        row.col(|ui| {
                            self.sense(ui);
                        });
                    });
                    body.row(ROW_HEIGHT, |mut row| {
                        row.col(|ui| {
                            ui.label("Movement").on_hover_text("id: 1");
                        });
                        row.col(|ui| {
                            self.sense(ui);
                        });
                    });
                    body.row(ROW_HEIGHT, |mut row| {
                        row.col(|_| {});
                        row.col(|ui| {
                            let mut max_t = max_rect.width() * (1.0 / scale(ui, 1.0, self.scale));
                            {
                                let keyframes = self.keyframes.lock().unwrap();
                                let last = keyframes.last();
                                if let Some(last) = last {
                                    let t = last.timestamp + last.duration;
                                    if t >= max_t {
                                        max_t = t;
                                    }
                                }
                            }
                            let rect = ui.max_rect();
                            self.render_scroll_bar(
                                ui,
                                max_t,
                                Rect {
                                    min: pos2(max_rect.min.x, rect.min.y),
                                    max: pos2(max_rect.max.x, rect.max.y),
                                },
                            );
                        });
                    });
                });

            self.render_keyframes(ui, &max_rect);
            if self.selecting {
                ui.painter().rect(
                    self.compute_selection_rect(&max_rect),
                    egui::Rounding::same(2.0),
                    egui::Color32::from_rgba_unmultiplied(0xAD, 0xD8, 0xE6, 20),
                    egui::Stroke::new(0.4, egui::Color32::LIGHT_BLUE),
                );
            }

            self.render_playhead(ui, 3, max_rect);
        });
    }
    /// Render the scroll bar
    /// Gives a view of how scrolled in the sequencer is
    fn render_scroll_bar(&mut self, ui: &mut Ui, max_t: f32, max_rect: Rect) {
        let t_width = max_rect.width() * (1.0 / scale(ui, 1.0, self.scale));
        let t_ratio = (t_width / max_t).clamp(0.0, 1.0);
        let d = t_width * t_ratio;
        let t = self.scroll.clamp(0.0, t_width - d);
        let rect = time_to_rect(
            scale(ui, t, self.scale),
            scale(ui, d, self.scale),
            0.0,
            ui.spacing().item_spacing,
            max_rect,
        )
        .unwrap()
        .translate(vec2(0., 2.));
        ui.painter().rect(
            rect,
            egui::Rounding::same(2.0),
            egui::Color32::DARK_GRAY,
            egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
        );
    }
    /// Renders a debug panel with relevant information
    pub fn debug_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("Debug")
            .max_width(200.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Debug");
                ui.separator();
                //todo: add mouse position
                ui.checkbox(&mut self.clear_before_recording, "Overwrite Recording");
                if ui.button("Debug selection").clicked() {
                    // println!("{:?}",self.selected_keyframes);
                    println!("{:?}", self.selected_keyframes.len());
                }
            });
    }
    /// Renders the editable data of the selected keyframe
    pub fn selected_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("Selected Keyframe")
            .min_width(155.0)
            .max_width(155.0)
            .resizable(false)
            .show(ctx, |ui| {
                if let Some(uid) = self.selected_keyframes.last() {
                    let mut keyframes = self.keyframes.lock().unwrap();
                    let mut index = 0;
                    for i in 0..keyframes.len() {
                        if keyframes[i].uid == *uid {
                            index = i;
                        }
                    }
                    let keyframe = &mut keyframes[index];

                    match &keyframe.keyframe_type {
                        KeyframeType::KeyBtn(key) => {
                            ui.strong("Keyboard Button press");
                            ui.label("key stroke");
                            ui.label(format!("{:?}", key));
                        }
                        KeyframeType::MouseBtn(key) => {
                            ui.strong("Mouse Button press");
                            ui.label(format!("button: {:?}", key));
                        }
                        KeyframeType::MouseMove(pos) => {
                            ui.strong("Mouse move");
                            ui.label(format!("position: {:?}", pos));
                        }
                        KeyframeType::Scroll(delta) => {
                            ui.strong("Scroll");
                            ui.label(format!("delta: {:?}", delta));
                        }
                        KeyframeType::Wait(secs) => {
                            ui.strong("Wait");
                            ui.label(format!("{:?}s", secs));
                        }
                    }
                    // Used later to check if the keyframe was edited
                    let (tmpx, tmpy) = (keyframe.timestamp, keyframe.duration);

                    ui.horizontal(|ui| {
                        ui.label("Timestamp");
                        ui.add(
                            egui::DragValue::new(&mut keyframe.timestamp)
                                .speed(0.2)
                                .range(0.0..=3600.0),
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.label("Duration");
                        ui.add(
                            egui::DragValue::new(&mut keyframe.duration)
                                .speed(0.1)
                                .range(0.00001..=100.0),
                        );
                    });
                    ui.small(format!(
                        "UUID: {}",
                        Uuid::from_bytes_le(keyframe.uid).to_string()
                    ));
                    // Check if the selected keyframe was changed
                    if (tmpx, tmpy) != (keyframe.timestamp, keyframe.duration) {
                        self.changed.swap(true, Ordering::Relaxed);
                        self.should_sort = true;
                    }
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.small("Select a keyframe to view and edit its details");
                    });
                }
            });
    }
    /// Renders the central panel used to display images and video
    pub fn central_panel(&self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Todo(addis): Keep specific 16/9 ratio so images are displayed properly
            egui_extras::install_image_loaders(ctx);
            ui.vertical_centered_justified(|ui| {
                if let Some(texture) = &self.current_image {
                    let size = Vec2::new(ui.available_height() * (16. / 9.), ui.available_height());
                    ui.image((texture.id(), size));
                }
            });
        });
    }
    /// Calculates the `Rect` created by mouse selection
    ///
    /// Manipulates the rect to draw properly with min being top left and max being bottom right
    fn compute_selection_rect(&self, max_rect: &Rect) -> Rect {
        let mut rect = self.selection;
        if self.selection.min.y > self.selection.max.y {
            rect = Rect {
                min: pos2(rect.min.x, rect.max.y),
                max: pos2(rect.max.x, rect.min.y),
            };
        }
        if self.selection.min.x > self.selection.max.x {
            rect = Rect {
                min: pos2(rect.max.x, rect.min.y),
                max: pos2(rect.min.x, rect.max.y),
            };
        }
        Rect {
            min: rect.min.clamp(max_rect.min, max_rect.max),
            max: rect.max.clamp(max_rect.min, max_rect.max),
        }
    }
    /// Handles sensing input relevant to the sequencer
    fn sense(&mut self, ui: &mut Ui) {
        let sequencer = ui.allocate_response(
            ui.available_size_before_wrap(),
            egui::Sense::click_and_drag(),
        );
        ui.input_mut(|i| {
            // Keybind(ctrl+a): Select all keyframes when focused in the sequencer timeline
            if i.consume_key(egui::Modifiers::CTRL, egui::Key::A) {
                self.select_all();
            }
            // Egui handles ctrl+[c,v,x] weirdly and results in multiple events for each press, once_bool avoids this
            if !self.once_bool {
                self.once_bool = i.events.iter().any(|e| match e {
                    egui::Event::Copy => {
                        self.copy();
                        true
                    }
                    egui::Event::Paste(_) => {
                        self.paste();
                        true
                    }
                    egui::Event::Cut => {
                        self.cut();
                        true
                    }
                    _ => false,
                });
            }
            if sequencer.hovered() {
                if i.pointer.any_pressed() {
                    self.selection.min = sequencer.interact_pointer_pos().unwrap();
                    self.selection.max = self.selection.min;
                }
            }
            if sequencer.drag_started() {
                if !i.modifiers.ctrl {
                    self.selected_keyframes.clear();
                }
                self.selecting = true;
            }
        });

        if sequencer.clicked() {
            ui.input(|i| {
                if !i.modifiers.ctrl {
                    self.selected_keyframes.clear();
                }
            });
        }
        if self.selecting {
            self.selection.max += sequencer.drag_delta();
        }
        if sequencer.drag_stopped() {
            self.selecting = false;
            self.selection = Rect::ZERO;
        }
        sequencer.context_menu(|ui| {
            if ui
                .add_enabled(
                    !self.selected_keyframes.is_empty(),
                    egui::Button::new("Cut").shortcut_text("Ctrl+X"),
                )
                .clicked()
            {
                self.cut();
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    !self.selected_keyframes.is_empty(),
                    egui::Button::new("Copy").shortcut_text("Ctrl+C"),
                )
                .clicked()
            {
                self.copy();
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    !self.clip_board.is_empty(),
                    egui::Button::new("Paste").shortcut_text("Ctrl+V"),
                )
                .clicked()
            {
                self.paste();
                ui.close_menu();
            }
        });
    }
    /// Handles keeping state, and replaying keystrokes when playing
    pub fn update(&mut self, last_instant: &mut Instant, ctx: &egui::Context, offset: Vec2) {
        // Handle focus of the window when recording and when not
        if self.was_recording != self.recording.load(Ordering::Relaxed) {
            self.was_recording = self.recording.load(Ordering::Relaxed);
            self.toggle_recording();
            if !self.was_recording {
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }

        let mut keyframes = self.keyframes.lock().unwrap();
        let mut keyframe_state = self.keyframe_state.lock().unwrap();

        // make sure that the keyframes and their respective state are synced correctly (probably are)
        if self.recording.load(Ordering::Relaxed) {
            if keyframes.len() != keyframe_state.len() {
                panic!("playing vec is out of sync")
            }
        }

        if self.should_sort {
            keyframes.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap());
            self.should_sort = false;
        }

        keyframe_state.iter_mut().for_each(|state| {
            // Reset the selected keyframes to be recomputed below
            if state == &2 {
                *state = 0;
            }
        });
        // Compute keyframe state from the selected keyframes
        // Todo(addis): Make sure that selected keyframes dont get deselected when highlighted by playhead
        for uid in &self.selected_keyframes {
            let mut index = 0;
            for i in 0..keyframes.len() {
                if keyframes[i].uid == *uid {
                    index = i;
                    break;
                }
            }
            keyframe_state[index] = 2; // 2 == selected/highlighted (orange)
        }

        // Code to get the mose recently selected keyframe and display it's image if possible, otherwise show start/end image
        let mut tmp = keyframe_state.clone();
        tmp.reverse();
        // Finds the first selected keyframe state in the list (effectivly the last keyframe)
        let x = tmp.iter().position(|&state| state == 2);
        if let Some(index) = x {
            // Since the tmp vec is reversed we need to invert it below
            let uid = keyframes[keyframes.len() - index - 1].uid;
            if self.current_image_uid != uid {
                if let Some(screenshot) = &self.images.lock().unwrap().get(&uid) {
                    let x =
                        ColorImage::from_rgba_unmultiplied([1920, 1080], &screenshot.as_slice());
                    // Todo(addis): stop this from being called several times per image
                    // Maybe load all of them with URIs then draw image using that instead
                    self.current_image =
                        Some(ctx.load_texture("screenshot", x, Default::default()));
                    self.current_image_uid = uid;
                }
            }
        } else {
        }

        let now = Instant::now();
        let dt = now - *last_instant;
        let play = self.play.load(Ordering::Relaxed);
        if play || self.recording.load(Ordering::Relaxed) {
            self.time += dt.as_secs_f32() * self.speed;
        }
        let last = keyframes.last();
        if play {
            if let Some(last) = last {
                if self.time >= last.timestamp + last.duration {
                    if self.repeats > 1 {
                        // Repeat the automation
                        self.time = 0.0;
                        self.repeats -= 1;
                    } else {
                        // self.current_image = "end".to_string();
                        self.play.swap(false, Ordering::Relaxed);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                }
            }
        }
        // check if the time has changed
        if self.prev_time != self.time {
            //The playhead has moved if the current time is not equal to the previous time
            // Todo(addis): create a slice of keyframes to come (without already played keyframes), to skip checking needlessly when playing
            // Todo(addis): or create a current and next keyframe tuple and only check those, then update it if one is handled
            for i in 0..keyframes.len() {
                let keyframe = &keyframes[i];
                let current_keyframe_state = keyframe_state[i]; //1 if playing, 0 if not
                                                                // checks if the playhead is entering or exiting the current keyframe, (far left or far right of keyframe in terms of time)
                if self.time >= keyframe.timestamp
                    && self.time <= keyframe.timestamp + keyframe.duration
                {
                    keyframe_state[i] = 1; //change keyframe state to playing, highlight
                    if self.current_image_uid != keyframe.uid {
                        if let Some(screenshot) = &self.images.lock().unwrap().get(&keyframe.uid) {
                            let x = ColorImage::from_rgba_unmultiplied(
                                [1920, 1080],
                                &screenshot.as_slice(),
                            );
                            self.current_image =
                                Some(ctx.load_texture("screenshot", x, Default::default()));
                            self.current_image_uid = keyframe.uid;
                        }
                    }
                    // Checks if the keyframe has changed since the playhead moved
                    if current_keyframe_state != keyframe_state[i] {
                        // If so and the sequencer is playing
                        if play {
                            handle_playing_keyframe(keyframe, true, offset);
                        }
                    }
                } else {
                    // Unhighlight an already highlighted keyframe making sure to avoid selected keyframes
                    if keyframe_state[i] == 1 {
                        keyframe_state[i] = 0; //change keyframe state to not playing, no highlight
                    }
                    // Checks if the keyframe has changed since the playhead moved
                    if current_keyframe_state != keyframe_state[i] {
                        // If so and the sequencer is playing
                        if play {
                            handle_playing_keyframe(keyframe, false, offset);
                        }
                    }
                }
            }
        }
        self.once_bool = false;
        //update previous time to keep track of when time changes
        self.prev_time = self.time;
        *last_instant = now;
    }
}
/// Simulates the given keyframe
///
/// `start` decides whether to treat this as the start or end of a keyframe
fn handle_playing_keyframe(keyframe: &Keyframe, start: bool, offset: Vec2) {
    match &keyframe.keyframe_type {
        KeyframeType::KeyBtn(key) => {
            if start {
                rdev::simulate(&rdev::EventType::KeyPress(*key))
                    .expect("Failed to simulate keypress");
            } else {
                rdev::simulate(&rdev::EventType::KeyRelease(*key))
                    .expect("Failed to simulate keyrelease");
            }
        }
        KeyframeType::MouseBtn(btn) => {
            if start {
                rdev::simulate(&rdev::EventType::ButtonPress(*btn))
                    .expect("Failed to simulate Button Release");
            } else {
                rdev::simulate(&rdev::EventType::ButtonRelease(*btn))
                    .expect("Failed to simulate Button Release");
            }
        }
        KeyframeType::MouseMove(pos) => {
            if start {
                rdev::simulate(&rdev::EventType::MouseMove {
                    x: (pos.x + offset.x) as f64,
                    y: (pos.y + offset.y) as f64,
                })
                .expect(
                    "Failed to simulate Mouse Movement (Probably due to an anticheat installed)",
                );
            }
        }
        KeyframeType::Scroll(delta) => {
            if start {
                rdev::simulate(&rdev::EventType::Wheel {
                    delta_x: (delta.x) as i64,
                    delta_y: (delta.y) as i64,
                })
                .expect("Failed to simulate Mouse Scroll (Possibly due to anticheat)");
            }
        }
        KeyframeType::Wait(secs) => {
            if start {
                // Todo(addis): multiply dt so that it takes *secs* seconds to traverse 1 second of sequecer time
                // This will remove the need to block the thread and freeze the application, and keep the playhead moving in a slow but satisfying way
                thread::sleep(Duration::from_secs_f32(secs.clone()));
            }
        }
    }
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}
