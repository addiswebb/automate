use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{thread, time::Instant};

use eframe::egui::{self, pos2, Ui, Vec2};
use egui::{emath::RectTransform, Pos2, Rect};
use egui::{vec2, NumExt, Widget};
use serde::{Deserialize, Serialize};

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
    dragging: bool,
    #[serde(skip)]
    drag_start: Pos2,
    #[serde(skip)]
    selecting: bool,
    #[serde(skip)]
    selection: Rect,
    //can_resize: bool,
    //resize_left: bool, //left: true, right: false
    #[serde(skip)]
    resizing: bool,
    #[serde(skip)]
    pub keyframes: Arc<Mutex<Vec<Keyframe>>>,
    #[serde(skip)]
    pub selected_keyframes: Vec<usize>,
    #[serde(skip)]
    pub keyframe_state: Arc<Mutex<Vec<usize>>>,
    scale: f32, // egui points to seconds scale
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
}

impl Sequencer {
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
        let mut mouse_move_count = 20;
        // this needs to get reset every time recording starts
        let _ = thread::Builder::new()
            .name("Record Thread".to_owned())
            .spawn(move || {
                log::info!("Created Recording Thread");
                if let Err(error) = rdev::listen(move |event: rdev::Event| {
                    let is_recording = shared_rec.load(Ordering::Relaxed);
                    let mut keyframe = None;
                    let dt = Instant::now().duration_since(*shared_instant.lock().unwrap());
                    match &event.event_type {
                        rdev::EventType::KeyRelease(key) => {
                            match key {
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
                                rdev::Key::Escape => {
                                    shared_play.swap(false, Ordering::Relaxed);
                                    //panic!("Stopped execution due to ESCAPE keybind");
                                }
                                rdev::Key::F9 => {
                                    keyframe = Some(Keyframe {
                                        timestamp: dt.as_secs_f32(),
                                        duration: 0.1,
                                        keyframe_type: KeyframeType::MouseMove(
                                            previous_mouse_position,
                                        ),
                                        id: 1,
                                    });
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                    // if dt == t, then ignore the event
                    if is_recording && keyframe.is_none() {
                        keyframe = match &event.event_type {
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
                                        id: 2,
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
                                    shared_changed.swap(true, Ordering::Relaxed);
                                }

                                match found_press {
                                    true => Some(Keyframe {
                                        timestamp: pressed_at.as_secs_f32(),
                                        duration: (dt - pressed_at).as_secs_f32(),
                                        keyframe_type: KeyframeType::KeyBtn(*key),
                                        id: 0,
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
                                                id: 1,
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
                                        id: 3,
                                    }),
                                }
                            } //_ => None,
                        };
                        if keyframe.is_some() {
                            let mut keyframes = shared_kfs.lock().unwrap();
                            let mut keyframe_state = shared_pkfs.lock().unwrap();
                            keyframes.push(keyframe.unwrap());
                            keyframe_state.push(0);
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
            drag_start: pos2(0., 0.),
            dragging: false,
            selection: Rect::ZERO,
            selecting: false,
            //can_resize: false,
            resizing: false,
            //resize_left: false,
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
        }
    }

    pub fn save_to_state(&self) -> SequencerState {
        SequencerState {
            keyframes: self.keyframes.lock().unwrap().clone(),
            repeats: self.repeats,
            speed: self.speed,
        }
    }
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

    pub fn toggle_play(&mut self) {
        self.play
            .swap(!self.play.load(Ordering::Relaxed), Ordering::Relaxed);
    }
    pub fn reset_time(&mut self) {
        self.time = 0.;
    }
    pub fn step_time(&mut self) {
        self.time += 0.1;
    }
    pub fn zoom(&mut self, delta: f32){
        let multiplier = 1.0;
        println!("{}",self.scale);
        self.scale += delta * multiplier;
        println!("{}",self.scale);
    }
    pub fn scroll(&mut self,delta: f32){
        let multiplier = 1.0/80.0;
        self.scroll += delta * multiplier;
    }
    pub fn toggle_recording(&mut self) {
        if !self.recording.load(Ordering::Relaxed) {
            let mut keyframes = self.keyframes.lock().unwrap();
            let mut keyframe_state = self.keyframe_state.lock().unwrap();
            let last = keyframes.last().unwrap();
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
        } else {
            let mut keyframes = self.keyframes.lock().unwrap();
            let mut keyframe_state = self.keyframe_state.lock().unwrap();
            let mut rec_instant = self.recording_instant.lock().unwrap();
            if self.clear_before_recording {
                self.time = 0.;
                keyframes.clear();
                keyframe_state.clear();
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
    fn render_keyframes(&mut self, ui: &mut Ui, ids: Vec<u8>) {
        let max_rect = ui.max_rect();
        let mut keyframes = self.keyframes.lock().unwrap();
        let offset = scale(ui, self.scroll, self.scale);
        let mut delete = false;
        for i in 0..keyframes.len() {
            if ids.contains(&keyframes[i].id) {
                let rect = time_to_rect(
                    scale(ui, keyframes[i].timestamp, self.scale) - offset,
                    scale(ui, keyframes[i].duration, self.scale) - offset,
                    scale(
                        ui,
                        ui.max_rect().width() * (1.0 / scale(ui, 1.0, self.scale)),
                        self.scale,
                    ),
                    ui.spacing().item_spacing,
                    max_rect,
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
                        if selection_contains_keyframe(self.compute_selection_rect(max_rect), rect)
                        {
                            if !self.selected_keyframes.contains(&i) {
                                self.selected_keyframes.push(i);
                            }
                        } else {
                            if !ctrl {
                                if !self.selected_keyframes.is_empty() {
                                    let index = self.selected_keyframes.binary_search(&i);
                                    if index.is_ok() {
                                        self.selected_keyframes.remove(index.unwrap());
                                    }
                                }
                            }
                        }
                    }
                }

                let width = rect.width();
                let mut label = format!("{:?}", keyframes[i].keyframe_type);

                if width / 10.0 < label.len() as f32 {
                    label.truncate((width / 10.0) as usize);
                }
                if width < 20.0 {
                    label = match &keyframes[i].keyframe_type {
                        KeyframeType::KeyBtn(key) => format!("{:?}", key),
                        _ => "".to_string(),
                    };
                }

                let color = match keyframes[i].id {
                    0 => egui::Color32::LIGHT_RED,              //Keyboard
                    1 => egui::Color32::from_rgb(95, 186, 213), //Mouse move
                    2 => egui::Color32::LIGHT_GREEN,            //Button Click
                    3 => egui::Color32::LIGHT_YELLOW,           //Scroll
                    _ => egui::Color32::LIGHT_GRAY,
                };
                let stroke = match self.keyframe_state.lock().unwrap()[i] {
                    1 => egui::Stroke::new(1.5, egui::Color32::LIGHT_RED), //Playing
                    2 => egui::Stroke::new(1.5, egui::Color32::from_rgb(233, 181, 125)), //Selected
                    _ => egui::Stroke::new(0.4, egui::Color32::from_rgb(15, 37, 42)), //Not selected
                };

                // let keyframe = ui
                //     .put(
                //         rect,
                //         egui::Button::new(
                //             egui::RichText::new(format!("{}", label)).color(egui::Color32::BLACK),
                //         )
                //         .sense(egui::Sense::click_and_drag())
                //         .wrap(false)
                //         .fill(egui::Color32::from_rgb(95, 186, 213))
                //         .stroke(stroke),
                //     )
                //     .on_hover_text(format!("{:?}", keyframes[i].keyframe_type));
                let keyframe = ui
                    .allocate_rect(rect, egui::Sense::click_and_drag())
                    .on_hover_text(format!("{:?}", keyframes[i].keyframe_type));
                ui.painter()
                    .rect(rect, egui::Rounding::same(2.0), color, stroke);

                if keyframe.clicked() {
                    let mut ctrl = false;
                    ui.input(|i| {
                        ctrl = i.modifiers.ctrl;
                    });
                    if self.selected_keyframes.contains(&i)
                        || self.keyframe_state.lock().unwrap()[i] == 2
                    {
                        if !ctrl {
                            let index = self.selected_keyframes.binary_search(&i).unwrap();
                            self.selected_keyframes.remove(index);
                        }
                    } else {
                        if !ctrl {
                            self.selected_keyframes.clear();
                        }
                        self.selected_keyframes.push(i);
                    }
                }

                if keyframe.drag_started() {
                    if let Some(start) = keyframe.interact_pointer_pos() {
                        self.drag_start = start;
                        self.dragging = true;
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
                            let state = self.keyframe_state.lock().unwrap();
                            keyframes[i].timestamp = t;
                            for j in 0..state.len() {
                                if state[j] == 2 && j != i {
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
                }
                ui.input_mut(|input| {
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::Delete) {
                        println!("delete");
                        delete = true;
                    }
                });
                keyframe.context_menu(|ui| {
                    if !self.selected_keyframes.contains(&i){
                        self.selected_keyframes.push(i);
                    }
                    if ui.button("Delete").clicked() {
                        delete = true;
                        ui.close_menu();
                    }
                });
            }
        }

        if delete && !self.selected_keyframes.is_empty() {
            let mut keyframe_state = self.keyframe_state.lock().unwrap();
            let selected_len = self.selected_keyframes.len();
            let kf_len = keyframe_state.len();
            self.selected_keyframes.sort();
            self.selected_keyframes.reverse();

            let mut next = self.selected_keyframes.first().unwrap().clone() + 1;
            for i in &self.selected_keyframes {
                if selected_len == kf_len {
                    keyframes.clear();
                    keyframe_state.clear();
                    break;
                }
                keyframes.remove(*i);
                keyframe_state.remove(*i);
                if next != 0{
                    next -= 1;
                }
            }
            self.selected_keyframes.clear();
            if !keyframes.is_empty(){
                self.selected_keyframes.push(next);
            }
            self.changed.swap(true, Ordering::Relaxed);
        }
    }
    fn render_control_bar(&mut self, ui: &mut Ui) {
        if ui.button("⏪").on_hover_text("Restart").clicked() {
            self.reset_time();
        }
        if ui.button("⏴").on_hover_text("Reverse").clicked() {}
        if self.play.load(Ordering::Relaxed) {
            if ui.button("⏸").on_hover_text("Pause").clicked() {
                self.toggle_play();
            }
        }else{
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
                .clamp_range(0.01..=2.5),
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
    fn render_timeline(&self, ui: &mut Ui) {
        let pos = time_to_rect(0.0, 0.0, 0.0, ui.spacing().item_spacing, ui.max_rect())
            .unwrap()
            .min;
        for i in 0..(ui.max_rect().width() * (1.0 / scale(ui, 1.0, self.scale)) + self.scroll)
            .ceil() as i32
        {
            let point = pos + egui::vec2(scale(ui, i as f32 - self.scroll, self.scale), 0.0);
            ui.painter().text(
                point,
                egui::Align2::CENTER_TOP,
                format!("{}", i),
                egui::FontId::monospace(12.0),
                egui::Color32::GRAY,
            );
            ui.painter().line_segment(
                [
                    pos2(point.x, ui.max_rect().max.y),
                    pos2(point.x, ui.max_rect().max.y) + egui::vec2(0.0, -6.0),
                ],
                egui::Stroke::new(1.0, egui::Color32::GRAY),
            );
        }
    }
    fn render_playhead(&mut self, ui: &mut Ui, rows: i32, rect: Rect) {
        let point = time_to_rect(
            scale(ui, self.time, self.scale),
            0.0,
            0.0,
            ui.spacing().item_spacing,
            rect,
        )
        .unwrap()
        .min;

        // let point = point;
        let p1 = pos2(point.x + 1., point.y - 5.);
        let p2 = pos2(p1.x, p1.y + ROW_HEIGHT * rows as f32 + (3 * rows) as f32);
        ui.painter().text(
            p1 - egui::vec2(0.0, 4.0),
            egui::Align2::CENTER_TOP,
            "⏷",
            egui::FontId::monospace(10.0),
            egui::Color32::LIGHT_RED,
        );
        ui.painter()
            .line_segment([p1, p2], egui::Stroke::new(1.0, egui::Color32::LIGHT_RED));
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("Sequencer").show(ctx, |ui| {
            use egui_extras::{Column, TableBuilder};
            let mut max_rect = Rect::ZERO;
            let mut table = TableBuilder::new(ui)
                .striped(false)
                .resizable(false)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::initial(60.0).range(60.0..=60.0))
                .column(Column::remainder())
                .drag_to_scroll(false)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
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
                            self.render_timeline(ui);
                        });
                    });
                    body.row(ROW_HEIGHT, |mut row| {
                        row.col(|ui| {
                            ui.label("Keyboard").on_hover_text("id: 0");
                        });
                        row.col(|ui| {
                            max_rect = ui.max_rect();
                            self.sense(ui);
                            self.render_keyframes(ui, vec![0]);
                        });
                    });

                    body.row(ROW_HEIGHT, |mut row| {
                        row.col(|ui| {
                            ui.label("Mouse").on_hover_text("id: 2,3");
                        });
                        row.col(|ui| {
                            self.sense(ui);
                            self.render_keyframes(ui, vec![2, 3]);
                        });
                    });
                    body.row(ROW_HEIGHT, |mut row| {
                        row.col(|ui| {
                            ui.label("Movement").on_hover_text("id: 1");
                        });
                        row.col(|ui| {
                            self.sense(ui);
                            self.render_keyframes(ui, vec![1]);
                        });
                    });
                    body.row(ROW_HEIGHT, |mut row| {
                        row.col(|_| {});
                        row.col(|ui| {
                            let mut max_t =
                                ui.max_rect().width() * (1.0 / scale(ui, 1.0, self.scale));
                            {
                                let keyframes = self.keyframes.lock().unwrap();
                                let last = keyframes.last();
                                if last.is_some() {
                                    let last = last.unwrap();
                                    let t = last.timestamp + last.duration;
                                    if t >= max_t {
                                        max_t = t;
                                    }
                                }
                            }
                            self.scroll_bar(ui, max_t);
                        });
                    });
                });

            max_rect.max.y += ROW_HEIGHT * 2.0;
            if self.selecting {
                ui.painter().rect(
                    self.compute_selection_rect(max_rect),
                    egui::Rounding::same(2.0),
                    egui::Color32::from_rgba_unmultiplied(0xAD, 0xD8, 0xE6, 20),
                    egui::Stroke::new(0.4, egui::Color32::DARK_BLUE),
                );
            }

            self.render_playhead(ui, 3, max_rect);
        });
    }
    fn scroll_bar(&mut self, ui: &mut Ui, max_t: f32) {
        let t_width = ui.max_rect().width() * (1.0 / scale(ui, 1.0, self.scale));
        let t_ratio = (t_width / max_t).clamp(0.0, 1.0);

        let x = self.scroll.clamp(0.0, t_width - (t_width * t_ratio));
        let rect = time_to_rect(
            scale(ui, x, self.scale),
            scale(ui, t_width * t_ratio - 0.01, self.scale),
            0.0,
            ui.spacing().item_spacing,
            ui.max_rect(),
        )
        .unwrap();
        ui.painter().rect(
            rect,
            egui::Rounding::same(2.0),
            egui::Color32::DARK_GRAY,
            egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
        );
    }
    pub fn debug_panel(&mut self, ctx: &egui::Context, offset: &mut Vec2) {
        egui::SidePanel::right("Debug")
            .min_width(200.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Debug");
                let (w, h) = rdev::display_size().unwrap();
                ui.label(format!("Display: ({},{})", w, h));

                ui.label(format!("selected: {:?}", self.selected_keyframes));
                // ui.label(format!("selection: {:?}", self.selection));
                //todo: add mouse position
                ui.horizontal(|ui| {
                    ui.label("Offset: ");
                    ui.add(egui::DragValue::new(&mut offset.x).speed(1))
                        .on_hover_text("X");
                    ui.add(egui::DragValue::new(&mut offset.y).speed(1))
                        .on_hover_text("Y");
                });
                //ui.label(format!("Keyframe state: {:?}", self.keyframe_state));
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
    pub fn selected_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("Selected Keyframe")
            .min_width(115.0)
            .resizable(false)
            .show(ctx, |ui| {
                if let Some(i) = self.selected_keyframes.last() {
                    let mut keyframes = self.keyframes.lock().unwrap();
                    if *i < keyframes.len() {
                        let keyframe = &mut keyframes[*i];

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
                                //ui.text_edit_singleline(&mut self.sequencer.keyframes[i].keyframe_type)
                                ui.label(format!("position: {:?}", pos));
                            }
                            KeyframeType::Scroll(delta) => {
                                ui.strong("Scroll");
                                //ui.text_edit_singleline(&mut self.sequencer.keyframes[i].keyframe_type)
                                ui.label(format!("delta: {:?}", delta));
                            }
                        }

                        ui.label("Timestamp");
                        ui.add(
                            egui::DragValue::new(&mut keyframe.timestamp)
                                .speed(0.25)
                                .clamp_range(0.0..=100.0),
                        );
                        ui.label("Duration");
                        ui.add(
                            egui::DragValue::new(&mut keyframe.duration)
                                .speed(0.1)
                                .clamp_range(0.00001..=10.0),
                        );
                    }
                }
            });
    }
    fn compute_selection_rect(&self, max_rect: Rect) -> Rect {
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
    fn sense(&mut self, ui: &mut Ui) {
        let response = ui.allocate_response(
            ui.available_size_before_wrap(),
            egui::Sense::click_and_drag(),
        );
        ui.input(|i| {
            if i.modifiers.ctrl && i.key_pressed(egui::Key::A) {
                for i in 0..self.keyframe_state.lock().unwrap().len() {
                    self.selected_keyframes.push(i);
                }
            }
        });
        if response.hovered() {
            ui.input(|i| {
                if i.pointer.any_pressed() {
                    self.selection.min = response.interact_pointer_pos().unwrap();
                    self.selection.max = self.selection.min;
                }
            });
        }
        if response.clicked() {
            ui.input(|i| {
                if !i.modifiers.ctrl {
                    self.selected_keyframes.clear();
                }
            });
        }
        if response.drag_started() {
            ui.input(|i| {
                if !i.modifiers.ctrl {
                    self.selected_keyframes.clear();
                }
            });
            self.selecting = true;
        }
        if self.selecting {
            self.selection.max += response.drag_delta();
        }
        if response.drag_stopped() {
            self.selecting = false;
            self.selection = Rect::ZERO;
        }
    }
    pub fn update(&mut self, last_instant: &mut Instant, ctx: &egui::Context, offset: Vec2) {
        if self.was_recording != self.recording.load(Ordering::Relaxed) {
            self.was_recording = self.recording.load(Ordering::Relaxed);
            self.toggle_recording();
            if !self.was_recording {
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }
        let keyframes = self.keyframes.lock().unwrap();
        let mut keyframe_state = self.keyframe_state.lock().unwrap();
        if self.recording.load(Ordering::Relaxed) {
            if keyframes.len() != keyframe_state.len() {
                panic!("playing vec is out of sync")
            }
        }
        keyframe_state.fill(0);
        for i in &self.selected_keyframes {
            keyframe_state[*i] = 2;
        }
        let now = Instant::now();
        let dt = now - *last_instant;
        let play = self.play.load(Ordering::Relaxed);
        if play || self.recording.load(Ordering::Relaxed) {
            self.time += dt.as_secs_f32() * self.speed;
        }
        if play && keyframes.last().is_some() {
            let last = keyframes.last().unwrap();
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
        if self.prev_time != self.time {
            //The playhead has moved if the current time is not equal to the previous time
            for i in 0..keyframes.len() {
                let keyframe = &keyframes[i];
                let current_keyframe_state = keyframe_state[i]; //1 if playing, 0 if not
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

        self.prev_time = self.time;
        *last_instant = now;
    }
}
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

fn selection_contains_keyframe(selection: Rect, keyframe: Rect) -> bool {
    if selection_contains_point(selection, keyframe.left_top())
        || selection_contains_point(selection, keyframe.left_bottom())
        || selection_contains_point(selection, keyframe.right_top())
        || selection_contains_point(selection, keyframe.right_bottom())
    {
        true
    } else {
        false
    }
}
fn selection_contains_point(selection: Rect, p: Pos2) -> bool {
    (selection.min.x <= p.x)
        & (p.x < selection.max.x)
        & (selection.min.y <= p.y)
        & (p.y < selection.max.y)
}

fn strings_to_keys(string: &String) -> Vec<rdev::Key> {
    let mut keys = vec![];
    for x in string.split(' ') {
        let key = string_to_keys(x);
        if key.is_some() {
            keys.push(key.unwrap());
        } else {
            for y in x.chars().into_iter() {
                let key = string_to_keys(y.to_string().as_str());
                if key.is_some() {
                    keys.push(key.unwrap());
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

fn scale(ui: &Ui, i: f32, scale: f32) -> f32 {
    let width = ui.max_rect().size().x;
    let s = 20.0 + scale * 40.0;
    let num_of_digits = width / s;
    let spacing = width / (num_of_digits);
    i * spacing
}
