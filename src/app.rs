use egui::{KeyboardShortcut, Vec2};
use egui_extras::{Column, TableBuilder};
use rfd::FileDialog;
use uuid::Uuid;
use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
    sync::atomic::Ordering,
    time::Instant,
};

use crate::{
    keyframe::{Keyframe, KeyframeType},
    sequencer::{Sequencer, SequencerState},
};

#[derive(serde::Deserialize, serde::Serialize)]
enum KeybindType {
    SaveFile,
    NewFile,
    OpenFile,
    Undo,
    Redo,
    ToggleSettings,
    NextKeyframe,
    PreviousKeyframe,
    TogglePlay,
    ResetTime,
    ToggleRecording,
    ToggleExecution,
    AddKeyframe,
    SelectAll,
}

enum SettingsPage {
    Preferences,
    Shortcuts,
}
impl Default for SettingsPage {
    fn default() -> Self {
        Self::Preferences
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Keybind {
    text: String,
    kind: KeybindType,
    keybind: KeyboardShortcut,
}
impl Keybind {
    pub fn new(text: String, kind: KeybindType, keybind: KeyboardShortcut) -> Self {
        Self {
            text,
            kind,
            keybind,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Settings {
    #[serde(skip)]
    keybind_search: String,
    keybinds: Vec<Keybind>,
    offset: Vec2,
    #[serde(skip)]
    page: SettingsPage,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            keybind_search: "".to_string(),
            keybinds: vec![
                Keybind::new(
                    "Save File".to_string(),
                    KeybindType::SaveFile,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::S),
                ),
                Keybind::new(
                    "New File".to_string(),
                    KeybindType::NewFile,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::N),
                ),
                Keybind::new(
                    "Open File".to_string(),
                    KeybindType::OpenFile,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::O),
                ),
                Keybind::new(
                    "Undo".to_string(),
                    KeybindType::Undo,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Z),
                ),
                Keybind::new(
                    "Redo".to_string(),
                    KeybindType::Redo,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Y),
                ),
                Keybind::new(
                    "Toggle Settings".to_string(),
                    KeybindType::ToggleSettings,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Comma),
                ),
                Keybind::new(
                    "Next Keyframe".to_string(),
                    KeybindType::NextKeyframe,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::ArrowRight),
                ),
                Keybind::new(
                    "Previous Keyframe".to_string(),
                    KeybindType::PreviousKeyframe,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::ArrowLeft),
                ),
                Keybind::new(
                    "Toggle Play".to_string(),
                    KeybindType::TogglePlay,
                    KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Space),
                ),
                Keybind::new(
                    "Reset Time".to_string(),
                    KeybindType::ResetTime,
                    KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::ArrowLeft),
                ),
                Keybind::new(
                    "Toggle Recording".to_string(),
                    KeybindType::ToggleRecording,
                    KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F8),
                ),
                Keybind::new(
                    "Toggle Execution".to_string(),
                    KeybindType::ToggleExecution,
                    KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Escape),
                ),
                Keybind::new(
                    "Add Keyframe".to_string(),
                    KeybindType::AddKeyframe,
                    KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F9),
                ),
                Keybind::new(
                    "Select All".to_string(),
                    KeybindType::SelectAll,
                    KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::A),
                ),
            ],
            offset: Vec2::NAN,
            page: SettingsPage::Preferences,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct App {
    #[serde(skip)] //Serializing creates two threads somehow
    sequencer: Sequencer,
    #[serde(skip)]
    last_instant: Instant,
    file: String,
    #[serde(skip)]
    file_uptodate: bool,
    #[serde(skip)]
    allowed_to_close: bool,
    #[serde(skip)]
    show_close_dialog: bool,
    #[serde(skip)]
    // weird name, basically determines whether the save before exiting dialog closes the window or creates a new file
    is_dialog_to_close: bool,
    #[serde(skip)]
    show_settings: bool,
    settings: Settings,
}

impl Default for App {
    fn default() -> Self {
        Self {
            sequencer: Sequencer::new(),
            last_instant: Instant::now(),
            file: "untitled.auto".to_string(),
            file_uptodate: true,
            allowed_to_close: false,
            show_close_dialog: false,
            is_dialog_to_close: false,
            show_settings: false,
            settings: Settings::default(),
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load previous app state if any
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
    /// Safely creates a new file
    ///
    /// If the current file has not been saved, gives the option to do so.
    fn new_file(&mut self) {
        if self.file_uptodate {
            //reset the sequencer
            self.sequencer.keyframes.lock().unwrap().clear();
            self.sequencer.keyframe_state.lock().unwrap().clear();
            self.sequencer.reset_time();
            self.file = "untitled.auto".to_string();
            self.sequencer.loaded_file = self.file.clone();
            self.file_uptodate = true;
            self.sequencer.changed.swap(false, Ordering::Relaxed);
            log::info!("New file: {:?}", "untitled.auto");
        } else {
            // offer to save the current file before making a new one
            self.show_close_dialog = true;
            self.is_dialog_to_close = false;
        }
    }
    /// Safely saves the current file
    ///
    /// Overwrites the current file if it already exists otherwise allows the creation of a new file.
    fn save_file(&mut self) {
        let state = self.sequencer.save_to_state();
        let data = bincode::serialize(&state).unwrap();
        // let json = serde_json::to_string_pretty(&state);
        // if let Ok(json) = json {
            // if its a new file, save as a new file
            if self.file == "untitled.auto" {
                self.file = FileDialog::new()
                    .add_filter("automate", &["auto"])
                    .set_directory("/")
                    .save_file()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();
            }
            // save the current file (if it was "untitled.auto", it has now been replaced)
            let file = File::create(self.file.clone());
            if let Ok(mut file) = file {
                file.write_all(&data).unwrap();
                self.sequencer.loaded_file = self.file.clone();
                self.file_uptodate = true;
                self.sequencer.changed.swap(false, Ordering::Relaxed);
                log::info!("Save file: {:?}", self.file);
            } else {
                log::error!("Failed to save {:?}", file);
            }
        // } else {
        //     log::error!("Failed to save sequencer to json");
        // }
    }
    /// Open a file using the native file dialog
    fn open_file(&mut self) {
        FileDialog::new()
            .add_filter("automate", &["auto"])
            .set_directory("/")
            .pick_file()
            .and_then(|path| {
                self.load_file(&path);
                Some(())
            });
    }
    ///Load an ".auto" file from the given path
    fn load_file(&mut self, path: &PathBuf) {
        let stream = File::open(path.clone());
        if let Ok(mut file) = stream {
            let mut contents = Vec::new();
            file.read(&mut contents).unwrap();
            let data: SequencerState =bincode::deserialize_from(file).unwrap();
            self.sequencer.load_from_state(data);
            self.file = path.file_name().unwrap().to_str().unwrap().to_string();
            self.sequencer.loaded_file = self.file.clone();
            self.file_uptodate = true;
            log::info!("Load file: {:?}", path);
        } else {
            self.new_file();
            log::info!(
                "Failed to load file: {:?} - {:?}",
                path,
                stream.err().unwrap()
            );
        }
    }
    /// Set the title of the window dependant on the current file status
    ///
    /// e.g "file.auto" if saved and "file.auto*" if there are changes to be saved.
    fn set_title(&self, ctx: &egui::Context) {
        let saved = match self.file_uptodate {
            true => "",
            false => "*",
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{}{} - Automate",
            self.file, //.replace(".auto", ""),
            saved,
        )));
    }
}

impl eframe::App for App {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.file != self.sequencer.loaded_file && self.file != "untitled.auto" {
            let file = self.file.clone();
            self.load_file(&PathBuf::from(file));
            self.set_title(ctx);
        }
        if self.sequencer.changed.load(Ordering::Relaxed) || !self.file_uptodate {
            self.file_uptodate = false;
        }
        self.set_title(ctx);
        let mut cancel_close = false;
        ctx.input(|i| {
            if !self.show_close_dialog && !self.show_settings {
                self.sequencer.zoom(i.smooth_scroll_delta.x);
                self.sequencer.scroll(i.smooth_scroll_delta.y);
            }
            // Todo(addis): check which of the following keybinds should only work when focused on the sequencer, and move them to sequencer.sense() if so
            // Todo(addis): change necessary keybinds to use consume_key instead of key_pressed, for those that should not repeat
            // Handle keybinds within app with focus
            if i.modifiers.ctrl {
                // Keybind(ctrl+s): Save file
                if i.key_pressed(egui::Key::S) {
                    self.save_file();
                }
                // Keybind(ctrl+n): Create a new file
                else if i.key_pressed(egui::Key::N) {
                    self.new_file();
                }
                // Keybind(ctrl+o): Open a file
                else if i.key_pressed(egui::Key::O) {
                    self.open_file();
                }
                // Keybind(ctrl+z): Undo last change
                else if i.key_pressed(egui::Key::O) {
                    println!("Undo: to be implemented");
                }
                // Keybind(ctrl+y): Redo last change
                else if i.key_pressed(egui::Key::O) {
                    println!("Redo: to be implemented");
                }
                // Keybind(ctrl+,): Toggle settings window
                else if i.key_pressed(egui::Key::Comma) {
                    self.show_settings = !self.show_settings;
                }

                let keyframes = self.sequencer.keyframes.lock().unwrap();
                // Keybind(ctrl+right): Select the next keyframe to the right
                if i.key_pressed(egui::Key::ArrowRight) {
                    let keyframe_state = self.sequencer.keyframe_state.lock().unwrap();
                    let mut last_index = 0;

                    if !keyframe_state.is_empty() {
                        if !self.sequencer.selected_keyframes.is_empty() {
                            let last_uuid =
                                self.sequencer.selected_keyframes.last().unwrap().clone();
                            let mut next = 0;
                            for i in 0..keyframes.len() {
                                if keyframes[i].uid == last_uuid {
                                    next = i;
                                    break;
                                }
                            }
                            if keyframe_state.len() > next + 1 {
                                last_index = next + 1;
                            } else {
                                last_index = next;
                            }
                        }
                        let uid = keyframes[last_index].uid;
                        if i.modifiers.shift {
                            match self.sequencer.selected_keyframes.binary_search(&uid) {
                                Ok(_) => {}
                                Err(index) => self.sequencer.selected_keyframes.insert(index, uid),
                            }
                        } else {
                            self.sequencer.selected_keyframes = vec![uid];
                        }
                    }
                }
                // Keybind(ctrl+left): Select the next keyframe to the left
                if i.key_pressed(egui::Key::ArrowLeft) {
                    let keyframe_state = self.sequencer.keyframe_state.lock().unwrap();
                    let mut last_index = 0;
                    if !keyframe_state.is_empty() {
                        let last_uuid = self.sequencer.selected_keyframes.last().unwrap().clone();
                        let mut next = 0;
                        for i in 0..keyframes.len() {
                            if keyframes[i].uid == last_uuid {
                                next = i;
                                break;
                            }
                        }
                        if next > last_index {
                            last_index = next - 1;
                        } else {
                            last_index = 0;
                        }
                        let uid = keyframes[last_index].uid;
                        if i.modifiers.shift {
                            match self.sequencer.selected_keyframes.binary_search(&uid) {
                                Ok(_) => {}
                                Err(index) => self.sequencer.selected_keyframes.insert(index, uid),
                            }
                        } else {
                            self.sequencer.selected_keyframes = vec![uid];
                        }
                    }
                }
            } else {
                // Keybind(right): Step forward 0.1 seconds in time
                if i.key_pressed(egui::Key::ArrowRight) {
                    self.sequencer.step_time();
                }
                // Keybind(space): Toggle play
                else if i.key_pressed(egui::Key::Space) {
                    self.sequencer.toggle_play();
                }
                // Keybind(left): Reset the playhead/time to 0 seconds
                else if i.key_pressed(egui::Key::ArrowLeft) {
                    self.sequencer.reset_time();
                }
                // Keybind(F8): Toggle recording
                else if i.key_released(egui::Key::F8) {
                    self.sequencer.recording.swap(
                        !self.sequencer.recording.load(Ordering::Relaxed),
                        Ordering::Relaxed,
                    );
                }
            }

            if i.viewport().close_requested() && !self.file_uptodate {
                if !self.allowed_to_close {
                    log::info!("Close without saving?");
                    self.show_close_dialog = true;
                    self.is_dialog_to_close = true;
                    cancel_close = true;
                }
            }
        });

        if self.show_close_dialog {
            egui::Window::new("Automate")
                .resizable(false)
                .movable(true)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(format!("Do you want to save changes to {:?}", self.file));
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            self.save_file();
                            self.show_close_dialog = false;
                            if self.is_dialog_to_close {
                                self.allowed_to_close = true;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            } else {
                                self.new_file();
                            }
                        }
                        if ui.button("Don't Save").clicked() {
                            self.show_close_dialog = false;
                            if self.is_dialog_to_close {
                                self.allowed_to_close = true;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            } else {
                                // set file_uptodate to true to force create a new file, avoids infinite loop
                                self.file_uptodate = true;
                                self.new_file();
                            }
                        }

                        if ui.button("Cancel").clicked() {
                            self.show_close_dialog = false;
                            self.allowed_to_close = false;
                        }
                    });
                });
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui
                        .add(egui::Button::new("New File...").shortcut_text("Ctrl+N"))
                        .clicked()
                    {
                        self.new_file();
                        self.set_title(ctx);
                        ui.close_menu();
                    }
                    if ui
                        .add(egui::Button::new("Save File...").shortcut_text("Ctrl+S"))
                        .clicked()
                    {
                        self.save_file();
                        self.set_title(ctx);
                        ui.close_menu();
                    }
                    if ui
                        .add(egui::Button::new("Open File...").shortcut_text("Ctrl+O"))
                        .clicked()
                    {
                        self.open_file();
                        self.set_title(ctx);
                        ui.close_menu();
                    }
                    if ui
                        .add(egui::Button::new("Settings").shortcut_text("Ctrl+,"))
                        .clicked()
                    {
                        self.show_settings = true;
                        ui.close_menu();
                    }
                    if ui
                        .add(egui::Button::new("Exit").shortcut_text("Alt+F4"))
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo").clicked() {
                        println!("Undo: to be implemented");
                        ui.close_menu();
                    }
                    if ui.button("Redo").clicked() {
                        println!("Redo: to be implemented");
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.menu_button("Add", |ui|{
                        if ui.button("Wait").clicked() {
                            let mut keyframes = self.sequencer.keyframes.lock().unwrap();
                            keyframes.push(Keyframe{
                                timestamp: self.sequencer.get_time(),
                                duration: 1.,
                                keyframe_type: KeyframeType::Wait(1.),
                                kind: 4,
                                uid: Uuid::new_v4().to_bytes_le(),
                                screenshot: None,
                            });
                            self.sequencer.keyframe_state.lock().unwrap().push(0);
                            ui.close_menu();
                        }
                        if ui.button("Key").clicked() {
                            ui.close_menu();
                        }
                    });
                    ui.separator();
                    if ui
                        .add_enabled(
                            !self.sequencer.selected_keyframes.is_empty(),
                            egui::Button::new("Cut").shortcut_text("Ctrl+X"),
                        )
                        .clicked()
                    {
                        self.sequencer.cut();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            !self.sequencer.selected_keyframes.is_empty(),
                            egui::Button::new("Copy").shortcut_text("Ctrl+C"),
                        )
                        .clicked()
                    {
                        self.sequencer.copy();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            !self.sequencer.clip_board.is_empty(),
                            egui::Button::new("Paste").shortcut_text("Ctrl+V"),
                        )
                        .clicked()
                    {
                        self.sequencer.paste();
                        ui.close_menu();
                    }
                });
            });
        });


        // if self.show_settings{
        egui::Window::new("Settings")
            .resizable(false)
            .movable(true)
            .collapsible(false)
            .open(&mut self.show_settings)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Side panel for settings list
                    ui.vertical(|ui| {
                        ui.set_width(100.);
                        ui.set_height(250.);
                        ui.vertical_centered_justified(|ui| {
                            if ui
                                .selectable_label(
                                    match self.settings.page {
                                        SettingsPage::Preferences => true,
                                        _ => false,
                                    },
                                    "Preferences",
                                )
                                .clicked()
                            {
                                self.settings.page = SettingsPage::Preferences;
                            }
                            if ui
                                .selectable_label(
                                    match self.settings.page {
                                        SettingsPage::Shortcuts => true,
                                        _ => false,
                                    },
                                    "Shortcuts",
                                )
                                .clicked()
                            {
                                self.settings.page = SettingsPage::Shortcuts;
                            }
                        });
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_min_width(340.);
                        ui.set_max_width(340.);
                        ui.set_width(340.);
                        ui.set_height(250.);
                        match self.settings.page {
                            SettingsPage::Preferences => {
                                ui.heading(egui::RichText::new("Preferences").strong());
                                ui.separator();
                                ui.add_space(4.);
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui|{
                                        ui.strong("Monitor Offset ");
                                        ui.add(
                                            egui::DragValue::new(&mut self.settings.offset.x).speed(1),
                                        )
                                        .on_hover_text("X");
                                        ui.add(
                                            egui::DragValue::new(&mut self.settings.offset.y).speed(1),
                                        )
                                        .on_hover_text("Y");
                                    });
                                    ui.label("Monitor Offset is used to correctly simulate mouse movements when using multiple monitors");
                                    ui.add_space(4.);
                                    ui.horizontal(|ui|{
                                        if ui.add(egui::Button::new("Calibrate")).on_hover_text("Calibrates the offset necessary to correctly move the mouse when using multiple monitors").clicked() {
                                            self.sequencer.calibrate.swap(true, Ordering::Relaxed);
                                            rdev::simulate(&rdev::EventType::MouseMove { x: 0., y: 0. }).unwrap();
                                            let mut keyframes = self.sequencer.keyframes.lock().unwrap();
                                            let last = keyframes.last();
                                            if let Some(last) = last{
                                                // Keyframe kind of 255 is used only for calibrating monitor offset
                                                if last.kind == u8::MAX{
                                                    if let KeyframeType::MouseMove(pos) = last.keyframe_type{
                                                        self.settings.offset = pos;
                                                    }
                                                }
                                            }
                                            keyframes.pop();
                                            self.sequencer.calibrate.swap(false, Ordering::Relaxed);
                                            log::info!("Calibrated Monitor Offset: {:?}", self.settings.offset);
                                        }
                                    });
                                });
                                ui.add_space(6.);
                                ui.separator();
                                ui.add_space(6.);
                                ui.vertical(|ui|{
                                    ui.horizontal(|ui|{
                                        ui.strong("Recording Resolution");

                                        let mut resolution = self
                                            .sequencer
                                            .mouse_movement_record_resolution
                                            .load(Ordering::Relaxed);
                                        ui.add(
                                            egui::DragValue::new(&mut resolution)
                                                .speed(1)
                                                .range(0..=100),
                                        )
                                        .on_hover_text("Recording Resolution");
                                        self.sequencer.mouse_movement_record_resolution
                                            .store(resolution, Ordering::Relaxed);
                                    });
                                    ui.label("The resolution at which mouse movement events are captured as keyframes, higher is better for accuracy.");
                                    ui.small("0 disables mouse recording, use F9 to record manually");
                                });
                                // ui.spacing();
                                // ui.separator();
                            }
                            SettingsPage::Shortcuts => {
                                ui.heading(egui::RichText::new("Shortcuts").strong());
                                ui.horizontal(|ui| {
                                    ui.centered_and_justified(|ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(
                                                &mut self.settings.keybind_search,
                                            )
                                            .hint_text("Type to search keybindings"),
                                        );
                                    });
                                    if ui.button("X").clicked() {}
                                });
                                ui.spacing();
                                let mut table = TableBuilder::new(ui)
                                    .striped(false)
                                    .resizable(false)
                                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                                    // Shortcut column
                                    .column(Column::auto())
                                    // Keybindings column
                                    .column(Column::remainder())
                                    .drag_to_scroll(false)
                                    .sense(egui::Sense::hover());
                                //allow rows to be clicked
                                table = table.sense(egui::Sense::click()).resizable(true);

                                table
                                    .header(22., |mut header| {
                                        header.col(|ui| {
                                            ui.strong("Shortcut");
                                        });
                                        header.col(|ui| {
                                            ui.strong("Keybind");
                                        });
                                    })
                                    .body(|body| {
                                        body.rows(22., self.settings.keybinds.len(), |mut row| {
                                            let keybind = &self.settings.keybinds[row.index()];
                                            row.col(|ui| {
                                                ui.label(format!("{}", keybind.text));
                                            });
                                            row.col(|ui| {
                                                let keybind = keybind.keybind;
                                                let ctrl = if keybind.modifiers.ctrl {
                                                    "Ctrl+".to_string()
                                                } else {
                                                    "".to_string()
                                                };
                                                ui.label(format!(
                                                    "{}{:#?}",
                                                    ctrl, keybind.logical_key,
                                                ));
                                            });
                                        });
                                    });
                            }
                        }
                    });
                });
            });

        self.sequencer
            .update(&mut self.last_instant, ctx, self.settings.offset);
        self.sequencer.show(ctx);
        self.sequencer.debug_panel(ctx);
        self.sequencer.selected_panel(ctx);
        self.sequencer.central_panel(ctx);

        
        if cancel_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        ctx.request_repaint();
    }
}
