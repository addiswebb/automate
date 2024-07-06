use std::iter;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{thread, time::Instant};

use eframe::egui::{self, pos2, Ui, Vec2};
use egui::{emath::RectTransform, Pos2, Rect};
use egui::{vec2, Align2, FontId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::keyframe::{Keyframe, KeyframeType};

const ROW_HEIGHT: f32 = 24.0;

#[derive(Debug, Serialize, Deserialize)]
pub struct SequencerState {
    pub keyframes: Vec<Keyframe>,
    pub repeats: i32,
    pub speed: f32,
}
/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
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
    pub selected_keyframes: Vec<Uuid>,
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
    #[serde(skip)]
    mouse_movement_record_resolution: Arc<AtomicI32>,
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

        let shared_kfs = Arc::clone(&keyframes);
        let shared_pkfs = Arc::clone(&keyframe_state);
        let shared_rec = Arc::clone(&recording);
        let shared_play = Arc::clone(&play);
        let shared_count = Arc::clone(&mouse_movement_record_resolution);
        let shared_instant = Arc::clone(&recording_instant);
        let shared_changed = Arc::clone(&changed);

        let mut mouse_presses = vec![];
        let mut mouse_pressed_at = vec![];

        let mut key_presses = vec![];
        let mut key_pressed_at = vec![];

        let mut previous_mouse_position = Vec2::ZERO;
        // this needs to get reset every time recording starts
        let mut mouse_move_count = 20;

        // Spawn the recording thread
        let _ = thread::Builder::new()
            .name("Record Thread".to_owned())
            .spawn(move || {
                log::info!("Created Recording Thread");
                if let Err(error) = rdev::listen(move |event: rdev::Event| {
                    let is_recording = shared_rec.load(Ordering::Relaxed);
                    let mut keyframe = None;
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
                                        key_presses = vec![];
                                        key_pressed_at = vec![];
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
                                    keyframe = Some(Keyframe {
                                        timestamp: dt.as_secs_f32(),
                                        duration: 0.1,
                                        keyframe_type: KeyframeType::MouseMove(
                                            previous_mouse_position,
                                        ),
                                        kind: 1,
                                        uid: Uuid::new_v4(),
                                    });
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                    if is_recording && keyframe.is_none() {
                        keyframe = match &event.event_type {
                            // Button & Key Press events just push info
                            rdev::EventType::ButtonPress(btn) => {
                                mouse_presses.push(btn.clone());
                                mouse_pressed_at.push(dt);
                                None
                            }
                            rdev::EventType::KeyPress(key) => {
                                key_presses.push(key.clone());
                                key_pressed_at.push(dt);
                                None
                            }
                            // Button & Key Release events search for the matching keypress event to create a full keyframe
                            rdev::EventType::ButtonRelease(btn) => {
                                let mut found_press = false;
                                let mut indices_to_remove = vec![];
                                let mut pressed_at = Duration::from_secs(0);
                                if mouse_presses.contains(btn) {
                                    for i in 0..mouse_presses.len() {
                                        if mouse_presses[i] == *btn {
                                            if !found_press {
                                                pressed_at = mouse_pressed_at[i];
                                                found_press = true;
                                            }
                                            indices_to_remove.push(i);
                                        }
                                    }
                                }
                                for i in 0..indices_to_remove.len() {
                                    let index = indices_to_remove[i] - (i * 1);
                                    mouse_presses.remove(index);
                                    mouse_pressed_at.remove(index);
                                }

                                match found_press {
                                    true => Some(Keyframe {
                                        timestamp: pressed_at.as_secs_f32(),
                                        duration: (dt - pressed_at).as_secs_f32(),
                                        keyframe_type: KeyframeType::MouseBtn(*btn),
                                        kind: 2,
                                        uid: Uuid::new_v4(),
                                    }),
                                    false => None,
                                }
                            }
                            rdev::EventType::KeyRelease(key) => {
                                let mut found_press = false;
                                let mut indices_to_remove = vec![];
                                let mut pressed_at = Duration::from_secs(0);
                                if key_presses.contains(key) {
                                    for i in 0..key_presses.len() {
                                        if key_presses[i] == *key {
                                            if !found_press {
                                                pressed_at = key_pressed_at[i];
                                                found_press = true;
                                            }
                                            indices_to_remove.push(i);
                                        }
                                    }
                                }
                                for i in 0..indices_to_remove.len() {
                                    let index = indices_to_remove[i] - (i * 1);
                                    key_presses.remove(index);
                                    key_pressed_at.remove(index);
                                }

                                match found_press {
                                    true => Some(Keyframe {
                                        timestamp: pressed_at.as_secs_f32(),
                                        duration: (dt - pressed_at).as_secs_f32(),
                                        keyframe_type: KeyframeType::KeyBtn(*key),
                                        kind: 0,
                                        uid: Uuid::new_v4(),
                                    }),
                                    false => None,
                                }
                            }
                            rdev::EventType::MouseMove { x, y } => {
                                let pos = Vec2::new(*x as f32, *y as f32);
                                mouse_move_count -= 1;
                                match previous_mouse_position == pos {
                                    false => match mouse_move_count <= 0 {
                                        true => {
                                            previous_mouse_position = pos;
                                            mouse_move_count = shared_count.load(Ordering::Relaxed);
                                            Some(Keyframe {
                                                timestamp: dt.as_secs_f32(),
                                                duration: 0.1,
                                                keyframe_type: KeyframeType::MouseMove(pos),
                                                kind: 1,
                                                uid: Uuid::new_v4(),
                                            })
                                        }
                                        false => None,
                                    },
                                    true => None,
                                }
                            }
                            rdev::EventType::Wheel { delta_x, delta_y } => {
                                match *delta_x == 0 && *delta_y == 0 {
                                    true => None,
                                    false => Some(Keyframe {
                                        timestamp: dt.as_secs_f32(),
                                        duration: 0.1,
                                        keyframe_type: KeyframeType::Scroll(Vec2::new(
                                            *delta_x as f32,
                                            *delta_y as f32,
                                        )),
                                        kind: 3,
                                        uid: Uuid::new_v4(),
                                    }),
                                }
                            }
                        };
                        // If a keyframe was created push the necessary data to sequencer
                        if let Some(keyframe) = keyframe {
                            shared_kfs.lock().unwrap().push(keyframe);
                            shared_pkfs.lock().unwrap().push(0);
                            shared_changed.swap(true, Ordering::Relaxed);
                        }
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
        }
    }

    /// Saves the current state of the sequencer to `SequencerState`
    pub fn save_to_state(&self) -> SequencerState {
        SequencerState {
            keyframes: self.keyframes.lock().unwrap().clone(),
            repeats: self.repeats,
            speed: self.speed,
        }
    }
    /// Loads the sequencer with the `SequencerState`
    pub fn load_from_state(&mut self, mut state: SequencerState) {
        let mut shared_kfs = self.keyframes.lock().unwrap();
        let mut shared_pkfs = self.keyframe_state.lock().unwrap();
        // Uuid is skipped when serializing so it is necessary to assign uuids now
        for x in state.keyframes.iter_mut() {
            if x.uid.is_nil() {
                x.uid = Uuid::new_v4();
            }
        }
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
        self.scale += delta * multiplier;
    }
    /// Scrolls through the keyframes
    pub fn scroll(&mut self, delta: f32) {
        let multiplier = 1.0 / 80.0;
        self.scroll += delta * multiplier / self.scale.clamp(0.1, f32::INFINITY);
    }
    /// Copy the selected keyframes to clipboard
    pub fn copy(&mut self) {
        let keyframes = self.keyframes.lock().unwrap();
        if !self.selected_keyframes.is_empty() {
            self.clip_board.clear();
            for i in 0..keyframes.len() {
                let keyframe = keyframes[i];
                let x = self.selected_keyframes.binary_search(&keyframe.uid);
                if x.is_ok() {
                    self.clip_board.push(keyframe);
                }
            }
        }
    }
    ///Paste the clipboard
    pub fn paste(&mut self) {
        if !self.clip_board.is_empty() {
            let mut clip_board: Vec<Keyframe> = self
                .clip_board
                .clone()
                .into_iter()
                .map(|mut kf| {
                    // Shift them all forward slightly so its clear what has been copied
                    kf.timestamp += 0.1;
                    // Change the UUIDs for the copied keyframes
                    kf.uid = uuid::Uuid::new_v4();
                    kf
                })
                .collect();
            let uids: Vec<Uuid> = clip_board.clone().into_iter().map(|kf| kf.uid).collect();
            self.selected_keyframes = uids;
            self.keyframe_state
                .lock()
                .unwrap()
                .append(&mut vec![0; clip_board.len()]);
            self.keyframes.lock().unwrap().append(&mut clip_board);
            // since the keyframes array has changed, it should be resorted
            self.should_sort = true;
        }
    }
    /// Copy the selected keyframes to clipboard and delete them from the keyframes vec
    pub fn cut(&mut self) {
        let mut keyframes = self.keyframes.lock().unwrap();
        self.clip_board.clear();
        for uid in &self.selected_keyframes {
            let mut index = 0;
            for i in 0..keyframes.len() {
                if keyframes[i].uid == *uid {
                    index = i;
                    self.clip_board.push(keyframes[i]);
                    break;
                }
            }
            keyframes.remove(index);
            self.keyframe_state.lock().unwrap().remove(index);
        }
        self.selected_keyframes.clear();
    }
    /// Select all keyframes
    pub fn select_all(&mut self) {
        let keyframes = self.keyframes.lock().unwrap();
        for i in 0..keyframes.len() {
            self.selected_keyframes.push(keyframes[i].uid);
        }
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
        let offset = scale(ui, self.scroll, self.scale);
        let mut delete = false;
        let mut cut = false;
        for i in 0..keyframes.len() {
            let offset_y = ui.spacing().item_spacing.y;
            let y = match keyframes[i].kind {
                0 => offset_y,
                1 => ROW_HEIGHT * 2. + 9.,
                2 => ROW_HEIGHT + offset_y * 2.,
                3 => ROW_HEIGHT + offset_y * 2.,
                _ => 0.,
            };
            let rect = time_to_rect(
                scale(ui, keyframes[i].timestamp, self.scale) - offset + 4.0,
                // +4 offset to allow for left most digit to always be visible
                scale(ui, keyframes[i].duration, self.scale),
                scale(
                    ui,
                    max_rect.width() * (1.0 / scale(ui, 1.0, self.scale)),
                    self.scale,
                ),
                ui.spacing().item_spacing,
                max_rect.translate(vec2(0., y)),
            );
            if rect.is_none() {
                continue;
            }
            let rect = rect.unwrap();

            {
                let mut ctrl = false;
                ui.input(|i| {
                    ctrl = i.modifiers.ctrl;
                });
                if self.selecting {
                    if selection_contains_keyframe(&self.compute_selection_rect(&max_rect), rect) {
                        match self.selected_keyframes.binary_search(&keyframes[i].uid) {
                            Ok(_) => {}
                            Err(index) => self.selected_keyframes.insert(index, keyframes[i].uid),
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
            }

            let width = rect.width();
            let label = format!(
                "{}",
                match &keyframes[i].keyframe_type {
                    KeyframeType::KeyBtn(key) => key_to_char(key),
                    KeyframeType::MouseBtn(btn) => button_to_char(btn),
                    KeyframeType::MouseMove(_) => "".to_string(),
                    KeyframeType::Scroll(delta) => scroll_to_char(delta),
                }
            );
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

            if width > 10.0 {
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    format!("{}", label),
                    FontId::default(),
                    egui::Color32::BLACK,
                );
            }
            if keyframe.clicked() {
                let ctrl = ui.input(|i| {
                    return i.modifiers.ctrl;
                });
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
                        if !was_empty {
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
                } else {
                    panic!("failed to get interact pos when dragging, just trying to see if this ever happens");
                }
            }
            if keyframe.hovered() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
            }

            if self.dragging {
                if let Some(end) = keyframe.interact_pointer_pos() {
                    let drag_delta =
                        (end.x - self.drag_start.x) * (1.0 / scale(ui, 1.0, self.scale));
                    let t = keyframes[i].timestamp + drag_delta;
                    if t > 0.0 {
                        keyframes[i].timestamp = t;
                        for j in 0..keyframe_state.len() {
                            if keyframe_state[j] == 2 && j != i {
                                keyframes[j].timestamp += drag_delta;
                            }
                        }
                        self.drag_start.x = end.x;
                        self.changed.swap(true, Ordering::Relaxed);
                    }
                }
            }
            if keyframe.drag_stopped() {
                self.drag_start = pos2(0., 0.);
                self.dragging = false;
                self.resizing = false;
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
                        for i in 0..keyframes.len() {
                            let keyframe = keyframes[i];
                            let x = self.selected_keyframes.binary_search(&keyframe.uid);
                            if x.is_ok() {
                                self.clip_board.push(keyframe);
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
                        let mut clip_board: Vec<Keyframe> = self
                            .clip_board
                            .clone()
                            .into_iter()
                            .map(|mut kf| {
                                // Shift them all forward slightly so its clear what has been copied
                                kf.timestamp += 0.1;
                                // Change the UUIDs for the copied keyframes
                                kf.uid = uuid::Uuid::new_v4();
                                kf
                            })
                            .collect();
                        let uids: Vec<Uuid> =
                            clip_board.clone().into_iter().map(|kf| kf.uid).collect();
                        self.selected_keyframes = uids;
                        self.keyframe_state
                            .lock()
                            .unwrap()
                            .append(&mut vec![0; clip_board.len()]);
                        keyframes.append(&mut clip_board);
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
        if cut {
            self.clip_board.clear();
            for uid in &self.selected_keyframes {
                let mut index = 0;
                for i in 0..keyframes.len() {
                    if keyframes[i].uid == *uid {
                        index = i;
                        self.clip_board.push(keyframes[i]);
                        break;
                    }
                }
                keyframes.remove(index);
                keyframe_state.remove(index);
            }
            self.selected_keyframes.clear();
        }
        // if there are keyframes selected to delete
        if delete {
            let number_of_selected_keyframes = self.selected_keyframes.len();
            let number_of_keyframes = keyframe_state.len();
            // sort the selected list from least the greatest index
            self.selected_keyframes.sort();
            self.selected_keyframes.reverse();
            // check if we can do a quick delete if every keyframe is selected
            if number_of_keyframes == number_of_selected_keyframes {
                keyframes.clear();
                keyframe_state.clear();
                self.selected_keyframes.clear();
            } else {
                // otherwise loop through keyframes and remove from last to first
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
                // if there are still keyframes left, we want to select the last one before the selection
                if !keyframes.is_empty() {
                    // if the last keyframe before selection was the very last keyframe then we get the second last
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
                .clamp_range(0.0..=(60.0 * 60.0 * 10.0))
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
            egui::DragValue::new(&mut self.scale)
                .speed(0.1)
                .clamp_range(0.00..=10.0),
        )
        .on_hover_text("Zoom");

        ui.add(
            egui::DragValue::new(&mut self.repeats)
                .speed(1)
                .clamp_range(1..=10000),
        )
        .on_hover_text("Number of repeats");
        ui.add(
            egui::DragValue::new(&mut self.speed)
                .speed(1)
                .suffix("x")
                .clamp_range(1.0..=20.0),
        )
        .on_hover_text("Speed");

        let mut resolution = self
            .mouse_movement_record_resolution
            .load(Ordering::Relaxed);
        ui.add(
            egui::DragValue::new(&mut resolution)
                .speed(1)
                .clamp_range(0.0..=100.0),
        )
        .on_hover_text("Recording Resolution");
        self.mouse_movement_record_resolution
            .store(resolution, Ordering::Relaxed);
        ui.add(
            egui::DragValue::new(&mut self.scroll)
                .speed(1.)
                .clamp_range(0.0..=1000.0),
        )
        .on_hover_text("Scroll");

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
            scale(ui, self.time-self.scroll, self.scale) + 3., //add 3. for offset to allow left most digit to always be visible
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
        let response = ui.allocate_rect(
            Rect {
                min: pos2(p1.x - padding, p1.y - padding),
                max: pos2(p1.x + padding, p2.y + padding),
            },
            egui::Sense::click_and_drag(),
        );
        if response.hovered() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
        }
        if response.drag_started() {
            if let Some(start) = response.interact_pointer_pos() {
                self.drag_start = start;
                self.dragging = true;
            }
        }
        if self.dragging {
            if let Some(end) = response.interact_pointer_pos() {
                if end.x > rect.min.x {
                    let drag_delta =
                        (end.x - self.drag_start.x) * (1.0 / scale(ui, 1.0, self.scale));
                    let t = self.time + drag_delta;
                    if t > 0.0 {
                        self.time = t;
                        self.drag_start.x = end.x;
                        self.changed.swap(true, Ordering::Relaxed);
                    }
                }else{
                    self.time = 0.;
                }
            }
        }
        if response.drag_stopped() {
            self.drag_start = pos2(0., 0.);
            self.dragging = false;
        }
        // clip the playhead so it is not visible when off the timeline
        let painter = ui.painter().with_clip_rect(rect.expand2(vec2(0.,4.0)));
        painter.text(
            p1 - egui::vec2(0.0, 3.0),
            egui::Align2::CENTER_TOP,
            "⏷",
            egui::FontId::monospace(10.0),
            egui::Color32::LIGHT_RED,
        );
        painter
            .line_segment([p1, p2], egui::Stroke::new(1.0, egui::Color32::LIGHT_RED));
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
    pub fn debug_panel(&mut self, ctx: &egui::Context, offset: &mut Vec2) {
        egui::SidePanel::right("Debug")
            .min_width(200.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Debug");
                let (w, h) = rdev::display_size().unwrap();
                ui.label(format!("Display: ({},{})", w, h));

                ui.label(format!("selected: {:?}", self.selected_keyframes));
                //todo: add mouse position
                ui.horizontal(|ui| {
                    ui.label("Offset: ");
                    ui.add(egui::DragValue::new(&mut offset.x).speed(1))
                        .on_hover_text("X");
                    ui.add(egui::DragValue::new(&mut offset.y).speed(1))
                        .on_hover_text("Y");
                });
                ui.label(format!(
                    "Rec: {}",
                    self.was_recording == self.recording.load(Ordering::Relaxed)
                ));
                ui.checkbox(&mut self.clear_before_recording, "Clear Before Recording");
                ui.label(format!("Time: {}s", self.time));
                ui.label(format!("Previous Time: {}s", self.prev_time));
                ui.label(format!("Scale: {}", self.scale));
            });
    }

    /// Renders the editable data of the selected keyframe
    pub fn selected_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("Selected Keyframe")
            .min_width(115.0)
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
                    }
                    let (tmpx, tmpy) = (keyframe.timestamp, keyframe.duration);
                    ui.label("Timestamp");
                    ui.add(
                        egui::DragValue::new(&mut keyframe.timestamp)
                            .speed(0.25)
                            .clamp_range(0.0..=1000.0),
                    );
                    ui.label("Duration");
                    ui.add(
                        egui::DragValue::new(&mut keyframe.duration)
                            .speed(0.1)
                            .clamp_range(0.00001..=100.0),
                    );

                    ui.label(format!("UUID: {:?}", keyframe.uid));
                    // Check if the selected keyframe was changed
                    if (tmpx, tmpy) != (keyframe.timestamp, keyframe.duration) {
                        self.changed.swap(true, Ordering::Relaxed);
                        self.should_sort = true;
                    }
                }
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
        let response = ui.allocate_response(
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
            if response.hovered() {
                if i.pointer.any_pressed() {
                    self.selection.min = response.interact_pointer_pos().unwrap();
                    self.selection.max = self.selection.min;
                }
            }
            if response.drag_started() {
                if !i.modifiers.ctrl {
                    self.selected_keyframes.clear();
                }
                self.selecting = true;
            }
        });

        if response.clicked() {
            ui.input(|i| {
                if !i.modifiers.ctrl {
                    self.selected_keyframes.clear();
                }
            });
        }
        if self.selecting {
            self.selection.max += response.drag_delta();
        }
        if response.drag_stopped() {
            self.selecting = false;
            self.selection = Rect::ZERO;
        }
        response.context_menu(|ui| {
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

        // Compute keyframe state from the selected keyframes
        keyframe_state.fill(0);
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
                    if current_keyframe_state != keyframe_state[i] {
                        if play {
                            handle_playing_keyframe(keyframe, true, offset);
                        }
                    }
                } else {
                    keyframe_state[i] = 0; //change keyframe state to not playing, no highlight
                    if current_keyframe_state != keyframe_state[i] {
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
/// Simulates a given keyframe
///
/// `start` decides whether it is a keypress or release
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
    }
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}
fn time_to_rect(t: f32, d: f32, max_t: f32, spacing: Vec2, res_rect: Rect) -> Option<Rect> {
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

fn selection_contains_keyframe(selection: &Rect, keyframe: Rect) -> bool {
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

#[allow(dead_code)]
fn strings_to_keys(string: &String) -> Vec<rdev::Key> {
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
fn string_to_keys(c: &str) -> Option<rdev::Key> {
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
fn key_to_char(k: &rdev::Key) -> String {
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
fn button_to_char(b: &rdev::Button) -> String {
    match b {
        rdev::Button::Left => "⏴".to_string(),
        rdev::Button::Right => "⏵".to_string(),
        rdev::Button::Middle => "◼".to_string(),
        _ => "".to_string(),
    }
}
fn scroll_to_char(delta: &Vec2) -> String {
    return if delta.x != 0. {
        "⬌".to_string()
    } else if delta.y != 0. {
        "⬍".to_string()
    } else {
        "".to_string()
    };
}

/// Correctly scales a given time `i` to screen position
fn scale(ui: &Ui, i: f32, scale: f32) -> f32 {
    let width = ui.max_rect().size().x;
    let s = 20.0 + scale * 40.0;
    let num_of_digits = width / s;
    let spacing = width / (num_of_digits);
    i * spacing
}
