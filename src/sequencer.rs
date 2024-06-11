use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{thread, time::Instant};

use eframe::egui::{self, pos2, Ui, Vec2};
use egui::{emath::RectTransform, Pos2, Rect};
use rdev::Button;

const ROW_HEIGHT: f32 = 24.0;

#[derive(Clone, Debug)]
pub enum KeyframeType {
    KeyBtn(String),  //0
    MouseBtn(u8),    //1
    MouseMove(Vec2), //2
}

#[derive(Clone, Debug)]
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
            keyframe_type: KeyframeType::KeyBtn("".to_string()),
            id: 0,
        }
    }
}

#[derive(Debug)]
pub struct Sequencer {
    dragging: bool,
    //can_resize: bool,
    //resize_left: bool, //left: true, right: false
    resizing: bool,
    drag_start: Pos2,
    pub keyframes: Arc<Mutex<Vec<Keyframe>>>,
    pub selected_keyframe: Option<usize>,
    pub playing_keyframes: Arc<Mutex<Vec<usize>>>,
    scale: f32, // egui points to seconds scale
    repeats: i32,
    speed: f32,
    time: f32,
    prev_time: f32,
    play: bool,
    mouse_movement_record_resolution: Arc<AtomicI32>,
    recording: Arc<AtomicBool>,
    recording_instant: Arc<Mutex<Instant>>,
}

impl Sequencer {
    pub fn new() -> Self {
        let keyframes: Arc<Mutex<Vec<Keyframe>>> = Arc::new(Mutex::new(vec![]));
        let playing_keyframes = Arc::new(Mutex::new(vec![]));
        let recording = Arc::new(AtomicBool::new(false));
        let mouse_movement_record_resolution = Arc::new(AtomicI32::new(20));
        let recording_instant = Arc::new(Mutex::new(Instant::now()));

        let shared_kfs = Arc::clone(&keyframes);
        let shared_pkfs = Arc::clone(&playing_keyframes);
        let shared_rec = Arc::clone(&recording);
        let shared_count = Arc::clone(&mouse_movement_record_resolution);
        let shared_instant = Arc::clone(&recording_instant);

        let mut mouse_presses = vec![];
        let mut mouse_pressed_at = vec![];

        let mut key_presses = vec![];
        let mut key_pressed_at = vec![];
        
        let mut previous_mouse_position = Vec2::new(0.0,0.0);
        let mut mouse_move_count = 20;
        // this needs to get reset every time recording starts
        thread::spawn(move || {
            log::info!("Created Recording Thread");
            if let Err(error) = rdev::listen(move |event: rdev::Event| {
                // if dt == t, then ignore the event
                if shared_rec.load(Ordering::Relaxed) {
                    let dt: Duration = Instant::now() - *shared_instant.lock().unwrap();
                    let keyframe = match &event.event_type {
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
                                    keyframe_type: KeyframeType::MouseBtn(match btn {
                                        Button::Left => 0,
                                        Button::Right => 1,
                                        _ => 2,
                                    }),
                                    id: 1,
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
                                    keyframe_type: KeyframeType::KeyBtn(
                                        key_to_char(key).to_string(),
                                    ),
                                    id: 0,
                                }),
                                false => None,
                            }
                        }
                        rdev::EventType::MouseMove { x, y } => {
                            let pos = Vec2::new(*x as f32, *y as f32);
                            mouse_move_count -=1;
                            println!("{mouse_move_count}");
                            match previous_mouse_position == pos {
                                false => {
                                    match mouse_move_count <= 0{
                                        true =>{
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
                                    }
                                },
                                true => None,
                            }
                        },
                        _ => None,
                    };

                    if keyframe.is_some() {
                        let mut keyframes = shared_kfs.lock().unwrap();
                        let mut playing_keyframes = shared_pkfs.lock().unwrap();
                        keyframes.push(keyframe.unwrap());
                        playing_keyframes.push(0);
                    }
                }
            }) {
                log::error!("Error: {:?}", error)
            }
        });
        Self {
            keyframes,
            drag_start: pos2(0., 0.),
            dragging: false,
            //can_resize: false,
            resizing: false,
            //resize_left: false,
            scale: 0.01,
            repeats: 1,
            speed: 1.0,
            time: 0.0,
            prev_time: 0.0,
            play: false,
            mouse_movement_record_resolution,
            selected_keyframe: None,
            playing_keyframes,
            recording,
            recording_instant,
        }
    }
    // pub fn add_keyframe(&mut self, keyframe: Keyframe) -> &Sequencer {
    //     log::info!("add keyframe: {:?}", keyframe);
    //     let mut keyframes = self.keyframes.lock().unwrap();
    //     keyframes.push(keyframe);
    //     self.playing_keyframes.push(0);
    //     keyframes.sort_unstable_by(|a, b| {
    //         if a.id == b.id || (a.id + b.id) == 3 {
    //             a.duration.partial_cmp(&b.duration).unwrap()
    //         } else {
    //             a.id.partial_cmp(&b.id).unwrap()
    //         }
    //     });
    //     keyframes.reverse();
    //     self
    // }

    fn render_keyframes(&mut self, ui: &mut Ui, id: u8) {
        let mut keyframe_to_delete: Option<usize> = None;
        let max_rect = ui.max_rect();
        let mut keyframes = self.keyframes.lock().unwrap();
        for i in 0..keyframes.len() {
            if keyframes[i].id == id {
                let rect = time_to_rect(
                    scale(ui, keyframes[i].timestamp, self.scale),
                    scale(ui, keyframes[i].duration, self.scale),
                    ui.spacing().item_spacing,
                    max_rect,
                );
                let width = rect.width();
                let mut label = format!("{:?}", keyframes[i].keyframe_type);

                if width / 10.0 < label.len() as f32 {
                    label.truncate((width / 10.0) as usize);
                }
                if width < 20.0 {
                    label = match &keyframes[i].keyframe_type {
                        KeyframeType::KeyBtn(key) => key.to_owned(),
                        _ => "".to_string(),
                    };
                }

                let mut stroke = if self.selected_keyframe == Some(i) {
                    egui::Stroke::new(1.5, egui::Color32::from_rgb(233, 181, 125))
                //Selected
                } else {
                    egui::Stroke::new(0.4, egui::Color32::from_rgb(15, 37, 42)) //Not selected
                };

                if self.playing_keyframes.lock().unwrap()[i] == 1 {
                    stroke = egui::Stroke::new(1.5, egui::Color32::LIGHT_RED); //Playing
                }
                let keyframe = ui
                    .put(
                        rect,
                        egui::Button::new(
                            egui::RichText::new(format!("{}", label)).color(egui::Color32::BLACK),
                        )
                        .sense(egui::Sense::click_and_drag())
                        .wrap(false)
                        .fill(egui::Color32::from_rgb(95, 186, 213))
                        .stroke(stroke),
                    )
                    .on_hover_text(format!("{:?}", keyframes[i].keyframe_type));

                if keyframe.clicked() {
                    self.selected_keyframe = if self.selected_keyframe == Some(i) {
                        None
                    } else {
                        Some(i)
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
                        let x = 1.0 / scale(ui, 1.0, self.scale);
                        let drag_delta = end.x - self.drag_start.x;
                        let t = keyframes[i].timestamp + drag_delta * x;
                        //&& t < pos_to_time(max_rect.max, max_rect)-self.keyframes[i].duration
                        //stop from going to far left vv | ^^ to far right
                        if t > 0.0 {
                            keyframes[i].timestamp = t;
                            self.drag_start.x = end.x;
                        }
                    }
                }
                if keyframe.drag_stopped() {
                    self.drag_start = pos2(0., 0.);
                    self.dragging = false;
                    //self.can_resize = false;
                    self.resizing = false;
                }

                ui.input(|input| {
                    if input.key_released(egui::Key::Delete) && self.selected_keyframe == Some(i) {
                        keyframe_to_delete = Some(i);
                    }
                });
                keyframe.context_menu(|ui| {
                    if ui.button("Delete").clicked() {
                        keyframe_to_delete = Some(i);
                        ui.close_menu();
                    }
                });
            }
        }
        if let Some(i) = keyframe_to_delete {
            keyframes.remove(i);
        }
    }

    fn render_control_bar(&mut self, ui: &mut Ui) {
        if ui.button("⏪").on_hover_text("Restart").clicked() {
            self.time = 0.0;
        }
        if ui.button("⏴").on_hover_text("Reverse").clicked() {
            println!("reverse");
        }
        if ui.button("⏵").on_hover_text("Play").clicked() {
            self.play = !self.play;
        }
        if ui.button("⏩").on_hover_text("Step").clicked() {
            self.time += 0.01;
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
                .clamp_range(0.01..=2.0),
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
        let mut keyframe_to_add: Option<Keyframe> = None;
        ui.menu_button("➕", |ui| {
            if ui.button("Key Strokes").clicked() {
                keyframe_to_add = Some(Keyframe {
                    timestamp: 5.0,
                    duration: 3.0,
                    keyframe_type: KeyframeType::KeyBtn("helo world".to_owned()),
                    id: 0,
                });
                ui.close_menu();
            }
            if ui.button("Mouse Button").clicked() {
                ui.close_menu();
            }
            if ui.button("Mouse Moves").clicked() {
                ui.close_menu();
            }
        })
        .response
        .on_hover_text("Add keyframe");
        let mut keyframes = self.keyframes.lock().unwrap();
        if let Some(keyframe) = keyframe_to_add {
            keyframes.push(keyframe);
            self.playing_keyframes.lock().unwrap().push(0);
            keyframes.sort_unstable_by(|a, b| {
                if a.id == b.id || (a.id + b.id) == 3 {
                    a.duration.partial_cmp(&b.duration).unwrap()
                } else {
                    a.id.partial_cmp(&b.id).unwrap()
                }
            });
            keyframes.reverse();
        }

        let mut resolution = self.mouse_movement_record_resolution.load(Ordering::Relaxed);
        ui.add(
            egui::DragValue::new(&mut resolution)
                .speed(1)
                .clamp_range(0.0..=100.0),
        )
        .on_hover_text("Recording Resolution");
        self.mouse_movement_record_resolution.store(resolution, Ordering::Relaxed);

        if self.recording.load(Ordering::Relaxed) {
            if ui.button("⏹").on_hover_text("Stop Recording: F8").clicked() {
                self.recording.swap(false, Ordering::Relaxed);
                self.time = 0.;
                keyframes.pop();
                self.playing_keyframes.lock().unwrap().pop();
                log::info!("Stop Recording");
            }
        } else {
            if ui
                .button(egui::RichText::new("⏺").color(egui::Color32::LIGHT_RED))
                .on_hover_text("Start Recording: F8")
                .clicked()
            {
                keyframes.clear();
                self.playing_keyframes.lock().unwrap().clear();
                let mut rec_instant = self.recording_instant.lock().unwrap();
                let _ = std::mem::replace(&mut *rec_instant, Instant::now());
                std::mem::drop(keyframes);
                std::mem::drop(rec_instant);
                self.recording.swap(true, Ordering::Relaxed);
                log::info!("Start Recording");
            }
        }
    }
    fn render_timeline(&self, ui: &mut Ui) {
        let pos = time_to_rect(0.0, 0.0, ui.spacing().item_spacing, ui.max_rect()).min;
        for i in 0..(ui.max_rect().width() * (1.0 / scale(ui, 1.0, self.scale))).ceil() as i32 {
            let point = pos + egui::vec2(scale(ui, i as f32, self.scale), 0.0);
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
    fn render_playhead(&mut self, ui: &mut Ui) {
        let point = time_to_rect(
            scale(ui, self.time, self.scale),
            0.0,
            ui.spacing().item_spacing,
            ui.max_rect(),
        )
        .min;

        let p1 = pos2(point.x, ui.max_rect().min.y - ROW_HEIGHT - 6.0);
        let p2 = pos2(p1.x, p1.y + ROW_HEIGHT * 2.0 + 6.0);
        ui.painter().text(
            p1 - egui::vec2(0.0, 2.0),
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
            let mut table = TableBuilder::new(ui)
                .striped(false)
                .resizable(false)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::initial(60.0).range(60.0..=60.0))
                .column(Column::remainder())
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
                            ui.label("Keyboard");
                        });
                        row.col(|ui| {
                            self.render_keyframes(ui, 0);
                        });
                    });

                    body.row(ROW_HEIGHT, |mut row| {
                        row.col(|ui| {
                            ui.label("Mouse");
                        });
                        row.col(|ui| {
                            self.render_keyframes(ui, 1);
                            self.render_keyframes(ui, 2);
                            self.render_playhead(ui);
                        });
                    });
                });
        });
    }

    pub fn debug_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("Debug")
            .min_width(200.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Debug");
                ui.label(format!("Keyframe state: {:?}", self.playing_keyframes));
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
                if let Some(i) = self.selected_keyframe {
                    let mut keyframes = self.keyframes.lock().unwrap();
                    if i < keyframes.len() {
                        let keyframe = &mut keyframes[i];

                        match &keyframe.keyframe_type {
                            KeyframeType::KeyBtn(keys) => {
                                ui.strong("Keyboard Button press");
                                ui.label("key strokes");
                                ui.text_edit_singleline(&mut keys.to_string());
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
                                .clamp_range(0.1..=10.0),
                        );
                    }
                }
            });
    }
    pub fn update(&mut self, last_instant: &mut Instant) {
        let keyframes = self.keyframes.lock().unwrap();
        let mut playing_keyframes = self.playing_keyframes.lock().unwrap();
        if self.recording.load(Ordering::Relaxed) {
            if keyframes.len() != playing_keyframes.len() {
                panic!("playing vec is out of sync")
            }
        }
        let now = Instant::now();
        let dt = now - *last_instant;
        if self.play || self.recording.load(Ordering::Relaxed) {
            self.time += dt.as_secs_f32();
        }
        if self.prev_time != self.time {
            //The playhead has moved if the current time is not equal to the previous time
            for i in 0..keyframes.len() {
                let keyframe = &keyframes[i];
                let current_keyframe_state = playing_keyframes[i]; //1 if playing, 0 if not
                if self.time >= keyframe.timestamp
                    && self.time <= keyframe.timestamp + keyframe.duration
                {
                    playing_keyframes[i] = 1; //change keyframe state to playing, highlight
                    if current_keyframe_state != playing_keyframes[i] {
                        if self.play {
                            handle_playing_keyframe(keyframe, true);
                        }
                    }
                } else {
                    playing_keyframes[i] = 0; //change keyframe state to not playing, no highlight
                    if current_keyframe_state != playing_keyframes[i] {
                        if self.play {
                            handle_playing_keyframe(keyframe, false);
                        }
                    }
                }
            }
        }

        self.prev_time = self.time;
        *last_instant = now;
    }
}
fn handle_playing_keyframe(keyframe: &Keyframe, start: bool) {
    match &keyframe.keyframe_type {
        KeyframeType::KeyBtn(keys) => {
            if start {
                let keys = string_to_keys(keys);
                for key in keys {
                    if start {
                        rdev::simulate(&rdev::EventType::KeyPress(key)).ok();
                    } else {
                        rdev::simulate(&rdev::EventType::KeyRelease(key)).ok();
                    }
                }
            }
        }
        KeyframeType::MouseBtn(key) => {
            let button = match key {
                0 => rdev::Button::Left,
                1 => rdev::Button::Left,
                _ => rdev::Button::Left,
            };
            if start {
                rdev::simulate(&rdev::EventType::ButtonPress(button)).ok();
            } else {
                rdev::simulate(&rdev::EventType::ButtonRelease(button)).ok();
            }
        }
        KeyframeType::MouseMove(pos) => {
            if start {
                rdev::simulate(&rdev::EventType::MouseMove {
                    x: pos.x as f64,
                    y: pos.y as f64,
                })
                .ok();
            }
        }
    }
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}
fn time_to_rect(t: f32, d: f32, spacing: Vec2, res_rect: Rect) -> Rect {
    let to_screen =
        RectTransform::from_to(Rect::from_min_size(Pos2::ZERO, res_rect.size()), res_rect);
    let p1 = Pos2 {
        x: t + spacing.y,
        y: spacing.y,
    };
    let height = ROW_HEIGHT - (spacing.y * 2.0);
    let width = if d < height { height } else { d };
    let p2 = p1
        + Vec2 {
            x: width,
            y: height,
        };
    Rect {
        min: to_screen.transform_pos(p1),
        max: to_screen.transform_pos(p2),
    }
}

fn string_to_keys(string: &String) -> Vec<rdev::Key> {
    let mut keys = vec![];
    for c in string.chars().into_iter() {
        keys.push(char_to_key(c));
    }
    return keys;
}

fn char_to_key(c: char) -> rdev::Key {
    match c {
        'a' => rdev::Key::KeyA,
        'b' => rdev::Key::KeyB,
        'c' => rdev::Key::KeyC,
        'd' => rdev::Key::KeyD,
        'e' => rdev::Key::KeyE,
        'f' => rdev::Key::KeyF,
        'g' => rdev::Key::KeyG,
        'h' => rdev::Key::KeyH,
        'i' => rdev::Key::KeyI,
        'j' => rdev::Key::KeyJ,
        'k' => rdev::Key::KeyK,
        'l' => rdev::Key::KeyL,
        'm' => rdev::Key::KeyM,
        'n' => rdev::Key::KeyN,
        'o' => rdev::Key::KeyO,
        'p' => rdev::Key::KeyP,
        'q' => rdev::Key::KeyQ,
        'r' => rdev::Key::KeyR,
        's' => rdev::Key::KeyS,
        't' => rdev::Key::KeyT,
        'u' => rdev::Key::KeyU,
        'v' => rdev::Key::KeyV,
        'w' => rdev::Key::KeyW,
        'x' => rdev::Key::KeyX,
        'y' => rdev::Key::KeyY,
        'z' => rdev::Key::KeyZ,
        _ => rdev::Key::Delete,
    }
}
fn key_to_char(k: &rdev::Key) -> char {
    match k {
        rdev::Key::KeyA => 'a',
        rdev::Key::KeyB => 'b',
        rdev::Key::KeyC => 'c',
        rdev::Key::KeyD => 'd',
        rdev::Key::KeyE => 'e',
        rdev::Key::KeyF => 'f',
        rdev::Key::KeyG => 'g',
        rdev::Key::KeyH => 'h',
        rdev::Key::KeyI => 'i',
        rdev::Key::KeyJ => 'j',
        rdev::Key::KeyK => 'k',
        rdev::Key::KeyL => 'l',
        rdev::Key::KeyM => 'm',
        rdev::Key::KeyN => 'n',
        rdev::Key::KeyO => 'o',
        rdev::Key::KeyP => 'p',
        rdev::Key::KeyQ => 'q',
        rdev::Key::KeyR => 'r',
        rdev::Key::KeyS => 's',
        rdev::Key::KeyT => 't',
        rdev::Key::KeyU => 'u',
        rdev::Key::KeyV => 'v',
        rdev::Key::KeyW => 'w',
        rdev::Key::KeyX => 'x',
        rdev::Key::KeyY => 'y',
        rdev::Key::KeyZ => 'z',
        _ => ' ',
    }
}

fn scale(ui: &Ui, i: f32, scale: f32) -> f32 {
    let width = ui.max_rect().size().x;
    let s = 20.0 + scale * 40.0;
    let num_of_digits = width / s;
    let spacing = width / (num_of_digits);
    i * spacing
}
