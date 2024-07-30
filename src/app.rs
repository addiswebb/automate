use egui_extras::{Column, TableBuilder};
use rfd::FileDialog;
use std::{
    fs::File,
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use uuid::Uuid;
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

use crate::{
    keyframe::{Keyframe, KeyframeType},
    sequencer::{Change, ChangeData, Sequencer, SequencerState},
    settings::{MonitorEdge, Settings, SettingsPage},
};

/// Determines the outcome of closing the "Save" dialog
pub enum DialogPurpose{
    Close,
    Open,
    New, 
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
    show_save_dialog: bool,
    #[serde(skip)]
    // weird name, basically determines whether the save before exiting dialog closes the window or creates a new file
    dialog_purpose: DialogPurpose,
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
            show_save_dialog: false,
            dialog_purpose: DialogPurpose::Close,
            settings: Settings::default(),
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {

        // Add Phosphor icons to fonts
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

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
            self.sequencer.reset_time();
            self.file = "untitled.auto".to_string();
            self.sequencer.loaded_file = self.file.clone();
            self.file_uptodate = true;
            self.sequencer.changed.swap(false, Ordering::Relaxed);
            self.sequencer.keyframes.clear();
            self.sequencer.keyframe_state.clear();
            log::info!("New file: {:?}", "untitled.auto");
        } else {
            // offer to save the current file before making a new one
            self.show_save_dialog = true;
            self.dialog_purpose = DialogPurpose::New;
        }
    }
    /// Safely saves the current file
    ///
    /// Overwrites the current file if it already exists otherwise allows the creation of a new file.
    fn save_file(&mut self) {
        // No need to save if the file is up to date (Just ensure this is accurate)
        if self.file_uptodate {
            return;
        }
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

        if let Ok(state) = bincode::serialize(&self.sequencer.save_to_state()) {
            // save the current file (if it was "untitled.auto", it has now been replaced)
            let now = Instant::now();
            let file = File::create(self.file.clone());
            if let Ok(file) = file {
                // write
                let mut zip = ZipWriter::new(file);
                let options =
                    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
                zip.start_file("data", options).unwrap();
                zip.write_all(&state).unwrap();

                let images = self.sequencer.images.lock().unwrap();
                for uid in images.keys() {
                    zip.start_file(Uuid::from_bytes_le(*uid).to_string(), options)
                        .unwrap();
                    zip.write_all(images.get(uid).unwrap().as_slice()).unwrap();
                }
                zip.finish().unwrap();

                self.sequencer.loaded_file = self.file.clone();
                self.file_uptodate = true;
                self.sequencer.changed.swap(false, Ordering::Relaxed);
                log::info!("Save file: {:?} - {:?}", self.file, now.elapsed());
            } else {
                log::error!("Failed to save {:?}", file);
            }
        }
    }
    /// Saves the current file always asking where and under what name to save it as
    fn save_as(&mut self) {
        self.file = FileDialog::new()
            .add_filter("automate", &["auto"])
            .set_directory("/")
            .save_file()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        self.file_uptodate = false;
        self.save_file();
    }
    /// Open a file using the native file dialog
    fn open_file(&mut self) {
        if self.file_uptodate {
            FileDialog::new()
                .add_filter("automate", &["auto"])
                .set_directory("/")
                .pick_file()
                .and_then(|path| {
                    self.load_file(&path);
                    Some(())
                });
        } else {
            // offer to save the current file before making a new one
            self.show_save_dialog = true;
            self.dialog_purpose = DialogPurpose::Open;
        }
    }
    ///Load an ".auto" file from the given path
    fn load_file(&mut self, path: &PathBuf) {
        let now = Instant::now();
        let stream = File::open(path.clone());
        if let Ok(file) = stream {
            let reader = BufReader::new(file);
            let mut zip = ZipArchive::new(reader).unwrap();

            // File of index 0 stores keyframes and general sequencer state
            let mut state = zip.by_index(0).unwrap();
            let mut bytes = Vec::new();
            state.read_to_end(&mut bytes).unwrap();
            if let Ok(data) = bincode::deserialize::<SequencerState>(bytes.as_slice()) {
                self.sequencer.load_from_state(data);
                self.file = path.to_str().unwrap().to_string();
                self.sequencer.loaded_file = self.file.clone();
                self.file_uptodate = true;
                std::mem::drop(state);
                // Load images, all other entries (excluding index: 0) are files named the UUID of the keyframe their image refers to
                for i in 1..zip.len() {
                    let mut image = zip.by_index(i).unwrap();
                    let mut bytes = Vec::new();
                    image.read_to_end(&mut bytes).unwrap();
                    self.sequencer
                        .images
                        .lock()
                        .unwrap()
                        .insert(Uuid::parse_str(image.name()).unwrap().to_bytes_le(), bytes);
                }
                log::info!("Loaded file: {:?} - {:?}", path, now.elapsed());
            } else {
                self.new_file();
                log::info!(
                    "Failed to load file: {:?}, most likely the file was created with an older version of Automate",
                    path,
                );
            }
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
    fn update_title(&self, ctx: &egui::Context) {
        let saved = match self.file_uptodate {
            true => "",
            false => "*",
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{}{} - Automate",
            Path::new(&self.file).file_name().unwrap().to_str().unwrap().to_string(), //.replace(".auto", ""),
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
            self.update_title(ctx);
        }
        let mut cancel_close = false;
        ctx.input(|i| {
            // Make sure that mouse scrolling only zooms/scrolls when sequencer is in focus
            if !self.show_save_dialog && !self.settings.show {
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
                    println!("opend, setting title");
                }
                // Keybind(ctrl+z): Undo last change
                else if i.key_pressed(egui::Key::Z) {
                    self.sequencer.undo();
                }
                // Keybind(ctrl+y): Redo last change
                else if i.key_pressed(egui::Key::Y) {
                    self.sequencer.redo();
                }
                // Keybind(ctrl+,): Toggle settings window
                else if i.key_pressed(egui::Key::Comma) {
                    self.settings.show = !self.settings.show;
                }

                // Keybind(ctrl+right): Select the next keyframe to the right
                if i.key_pressed(egui::Key::ArrowRight) {
                    let mut last_index = 0;

                    if !self.sequencer.keyframe_state.is_empty() {
                        if let Some(last_uuid) = self.sequencer.selected_keyframes.last() {
                            let mut next = 0;
                            for i in 0..self.sequencer.keyframes.len() {
                                if self.sequencer.keyframes[i].uid == *last_uuid {
                                    next = i;
                                    break;
                                }
                            }
                            if self.sequencer.keyframe_state.len() > next + 1 {
                                last_index = next + 1;
                            } else {
                                last_index = next;
                            }
                        }
                        let uid = self.sequencer.keyframes[last_index].uid;
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
                    let mut last_index = 0;
                    if !self.sequencer.keyframe_state.is_empty() {
                        let last_uuid = self.sequencer.selected_keyframes.last().unwrap().clone();
                        let mut next = 0;
                        for i in 0..self.sequencer.keyframes.len() {
                            if self.sequencer.keyframes[i].uid == last_uuid {
                                next = i;
                                break;
                            }
                        }
                        if next > last_index {
                            last_index = next - 1;
                        } else {
                            last_index = 0;
                        }
                        let uid = self.sequencer.keyframes[last_index].uid;
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
                    self.sequencer.toggle_recording();
                }
            }

            if i.viewport().close_requested() && !self.file_uptodate {
                if !self.allowed_to_close {
                    log::info!("Close without saving?");
                    self.show_save_dialog = true;
                    self.dialog_purpose = DialogPurpose::Close;
                    cancel_close = true;
                }
            }
        });

        if self.show_save_dialog {
            egui::Window::new("Automate")
                .resizable(false)
                .movable(true)
                .collapsible(false)
                .show(ctx, |ui| {
                    let before_saving = match self.dialog_purpose {
                        DialogPurpose::Close => " before exiting",
                        DialogPurpose::Open => "",
                        DialogPurpose::New => " before creating a new file",
                    };
                    ui.label(format!("Do you want to save changes to {:?}{}?", Path::new(&self.file).file_name().unwrap().to_str().unwrap().to_string(),before_saving));
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            self.save_file();
                            self.show_save_dialog = false;
                            match self.dialog_purpose {
                                DialogPurpose::Close => {
                                    self.allowed_to_close = true;
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                                DialogPurpose::New => {
                                    self.new_file();
                                }
                                DialogPurpose::Open => {
                                    self.open_file();
                                }
                            } 
                            self.update_title(ctx);
                        }
                        if ui.button("Don't Save").clicked() {
                            self.show_save_dialog = false;
                            match self.dialog_purpose{
                                DialogPurpose::Close => {
                                    self.allowed_to_close = true;
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                                DialogPurpose::New => {
                                    // set file_uptodate to true to force create a new file, avoids infinite loop
                                    self.file_uptodate = true;
                                    self.new_file();
                                }
                                DialogPurpose::Open => {
                                    self.file_uptodate = true;
                                    self.open_file();
                                    self.update_title(ctx);
                                }
                            }
                        }

                        if ui.button("Cancel").clicked() {
                            self.show_save_dialog = false;
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
                        self.update_title(ctx);
                        ui.close_menu();
                    }
                    if ui
                        .add(egui::Button::new("Open File...").shortcut_text("Ctrl+O"))
                        .clicked()
                    {
                        self.open_file();
                        self.update_title(ctx);
                        ui.close_menu();
                    }
                    if ui
                        .add(egui::Button::new("Save").shortcut_text("Ctrl+S"))
                        .clicked()
                    {
                        self.save_file();
                        self.update_title(ctx);
                        ui.close_menu();
                    }
                    if ui
                        .add(egui::Button::new("Save As...").shortcut_text("Ctrl+Shift+S"))
                        .clicked()
                    {
                        self.save_as();
                        self.update_title(ctx);
                        ui.close_menu();
                    }
                    ui.separator(); 
                    if ui
                        .add(egui::Button::new("Settings").shortcut_text("Ctrl+,"))
                        .clicked()
                    {
                        self.settings.show = true;
                        ui.close_menu();
                    }
                    ui.separator(); 
                    if ui
                        .add(egui::Button::new("Exit").shortcut_text("Alt+F4"))
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.add_enabled(!self.sequencer.changes.0.is_empty(),egui::Button::new("Undo")).clicked(){
                        self.sequencer.undo();
                        ui.close_menu();
                    }
                    if ui.add_enabled(!self.sequencer.changes.1.is_empty(),egui::Button::new("Redo")).clicked(){
                        self.sequencer.redo();
                        ui.close_menu();
                    }

                    ui.separator();
                    if ui.add_enabled(!self.sequencer.keyframes.is_empty(),egui::Button::new("Cull Minor Moves")).on_hover_text("Remove all unnecessary mouse move keyframes").clicked(){
                        self.sequencer.cull_minor_movement_keyframes(); 
                    }
                    self.sequencer.context_menu(ui, None);
                    ui.separator();
                    ui.menu_button("Add", |ui| {
                        if ui.button("Wait").clicked() {
                            let keyframe = Keyframe {
                                timestamp: self.sequencer.get_time(),
                                duration: 1.,
                                keyframe_type: KeyframeType::Wait(1.),
                                kind: 4,
                                enabled: true,
                                uid: Uuid::new_v4().to_bytes_le(),
                            };
                            self.sequencer.keyframes.push(keyframe.clone());
                            self.sequencer.changes.0.push(Change { uids: vec![], data: vec![ChangeData::AddKeyframes(vec![keyframe])]});
                            self.sequencer.changed();
                            ui.close_menu();
                        }
                        if ui.button("Magic Move").clicked() {
                            let keyframe = Keyframe {
                                timestamp: self.sequencer.get_time(),
                                duration: 0.2,
                                keyframe_type: KeyframeType::MagicMove("target.png".to_string()),
                                kind: 6,
                                enabled: true,
                                uid: Uuid::new_v4().to_bytes_le(),
                            };
                            self.sequencer.keyframes.push(keyframe.clone());
                            self.sequencer.changes.0.push(Change { uids: vec![], data: vec![ChangeData::AddKeyframes(vec![keyframe])]});
                            self.sequencer.changed();
                            ui.close_menu();
                        }
                        if ui.button("Loop").clicked() {
                            let keyframe = Keyframe {
                                timestamp: self.sequencer.get_time(),
                                duration: 5.,
                                keyframe_type: KeyframeType::Loop(10,1),
                                kind: 7,
                                enabled: true,
                                uid: Uuid::new_v4().to_bytes_le(),
                            };
                            self.sequencer.keyframes.push(keyframe.clone());
                            self.sequencer.changes.0.push(Change { uids: vec![], data: vec![ChangeData::AddKeyframes(vec![keyframe])]});
                            self.sequencer.changed();
                            ui.close_menu();
                        }
                    });
                });
            });
        });

        // if self.show_settings{
        egui::Window::new("Settings")
            .resizable(false)
            .movable(true)
            .collapsible(false)
            .open(&mut self.settings.show)
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
                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    // Monitor offset
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
                                        ui.label("Monitor Offset is used to correctly simulate mouse movements when using multiple monitors.");
                                        ui.add_space(4.);
                                        ui.horizontal(|ui|{
                                            if ui.add(egui::Button::new("Calibrate")).on_hover_text("Calibrates the offset necessary to correctly move the mouse when using multiple monitors").clicked() {
                                                self.sequencer.calibrate.swap(true, Ordering::Relaxed);
                                                rdev::simulate(&rdev::EventType::MouseMove { x: 0., y: 0. }).unwrap();
                                                let mut recording_keyframes = self.sequencer.recording_keyframes.lock().unwrap();
                                                if let Some(last) = recording_keyframes.last(){
                                                    // Keyframe kind of 255 is used only for calibrating monitor offset
                                                    if last.kind == u8::MAX{
                                                        if let KeyframeType::MouseMove(pos) = last.keyframe_type{
                                                            // Invert the pos so it brings us back to (0,0)
                                                            self.settings.offset = pos * egui::Vec2::new(-1.,-1.);
                                                        }
                                                    }
                                                }
                                                recording_keyframes.pop();
                                                self.sequencer.calibrate.swap(false, Ordering::Relaxed);
                                                log::info!("Calibrated Monitor Offset: {:?}", self.settings.offset);
                                            }
                                        });
                                    });
                                    ui.add_space(6.);
                                    ui.separator();
                                    ui.add_space(6.);

                                    // Recording resolution
                                    ui.vertical(|ui|{
                                        ui.horizontal(|ui|{
                                            ui.strong("Recording Resolution");

                                            let mut resolution = self
                                                .sequencer
                                                .mouse_movement_record_resolution
                                                .load(Ordering::Relaxed);
                                            ui.add(
                                                egui::DragValue::new(&mut resolution)
                                                    .custom_formatter(|n, _| {
                                                        format!("{}%",n as u32)
                                                    })
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
                                    ui.add_space(6.);
                                    ui.separator();
                                    ui.add_space(6.);
                                    // Fail safe
                                    ui.vertical(|ui|{
                                        ui.horizontal(|ui|{
                                            ui.strong("Fail safe");
                                            let mut monitor_edge = *self.sequencer.failsafe_edge.lock().unwrap();
                                            
                                            egui::ComboBox::from_label("")
                                                .selected_text(format!("{:?}", monitor_edge))
                                                .show_ui(ui, |ui| {
                                                    ui.selectable_value(&mut monitor_edge, MonitorEdge::Left, "Left");
                                                    ui.selectable_value(&mut monitor_edge, MonitorEdge::Right, "Right");
                                                    ui.selectable_value(&mut monitor_edge, MonitorEdge::Bottom, "Bottom");
                                                    ui.selectable_value(&mut monitor_edge, MonitorEdge::Top, "Top");
                                                });
                                            *self.sequencer.failsafe_edge.lock().unwrap() = monitor_edge;
                                        });
                                        ui.label("Incase of failure during playback, quickly slam the mouse into the selected edge to stop.");
                                        ui.small("Only works for main monitor");
                                    });
                                    ui.add_space(6.);
                                    ui.separator();
                                    ui.add_space(6.);
                                    // Fail detection
                                    ui.vertical(|ui|{
                                        ui.horizontal(|ui|{
                                            ui.strong("Fail detection");
                                            ui.checkbox(&mut self.settings.fail_detection, "");
                                            ui.add(egui::DragValue::new(&mut self.settings.max_fail_error)
                                            .custom_formatter(|n, _| {
                                                format!("{}%",n)
                                            })
                                            .speed(1)
                                            .range(0..=100));
                                        });
                                        ui.label("Computes the percentage different between the keyframe's expect screenshot vs what is on the screen and stops execution if it is beyond the threshold above, using computer vision.");
                                        ui.small("Only works for main monitor");
                                    });
                                    ui.add_space(6.);
                                });
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
                                TableBuilder::new(ui)
                                    .striped(false)
                                    .resizable(true)
                                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                                    // Shortcut column
                                    .column(Column::auto())
                                    // Keybindings column
                                    .column(Column::auto())
                                    .drag_to_scroll(false)
                                    .sense(egui::Sense::click())
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
            .update(&mut self.last_instant, ctx, &self.settings);
        
        self.sequencer.show(ctx);
        self.sequencer.debug_panel(ctx, &mut self.settings);
        self.sequencer.selected_panel(ctx, &self.settings);
        self.sequencer.central_panel(ctx);
        self.sequencer.modal(ctx);

        // If sequencer has changed or the file is not uptodate
        self.file_uptodate = !self.sequencer.changed.load(Ordering::Relaxed);
        // Should be called after saving and opening, but cannot be called within ctx.input so we call here
        self.update_title(ctx);

        if cancel_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        ctx.request_repaint();
    }
}
