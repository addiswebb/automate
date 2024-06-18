use egui::Vec2;
use rfd::FileDialog;
use std::{
    fs::File, io::{Read, Write}, path::{Path, PathBuf}, sync::atomic::Ordering, time::Instant
};

use crate::sequencer::{Sequencer, SequencerState};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    #[serde(skip)] //Serializing creates two threads somehow
    sequencer: Sequencer,
    #[serde(skip)]
    last_instant: Instant,
    offset: Vec2,
    #[serde(skip)] 
    file: String,
    #[serde(skip)] 
    allowed_to_close: bool,
    #[serde(skip)] 
    show_close_dialog: bool
}

impl Default for App {
    fn default() -> Self {
        Self {
            sequencer: Sequencer::new(),
            last_instant: Instant::now(),
            offset: Vec2::ZERO,
            file: "untitled.auto".to_string(),
            allowed_to_close: false,
            show_close_dialog: false,
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
    fn new_file(&mut self) {
        self.sequencer.keyframes.lock().unwrap().clear();
        self.sequencer.playing_keyframes.lock().unwrap().clear();
        self.sequencer.reset_time();
        self.file = "untitled.auto".to_string();
        self.sequencer.loaded_file = self.file.clone();
    }
    fn save_file(&mut self) {
        let state = self.sequencer.save_to_state();
        let json = serde_json::to_string(&state);
        if json.is_ok() {
            let path = FileDialog::new()
                .add_filter("automate", &["auto"])
                .set_directory("/")
                .save_file()
                .unwrap();
            let mut file = File::create(path.clone()).unwrap();
            let json = json.unwrap();
            file.write_all(json.as_bytes()).unwrap();
            self.file = path.file_name().unwrap().to_str().unwrap().to_string();
            self.sequencer.loaded_file = self.file.clone();
            log::info!("Save file: {:?}", path);
        } else {
            log::error!("Failed to save 'file.auto'");
        }
    }
    fn open_file(&mut self) {
        let path = FileDialog::new()
            .add_filter("automate", &["auto"])
            .set_directory("/")
            .pick_file()
            .unwrap();
        self.load_file(&path);
    }
    fn load_file(&mut self, path: &PathBuf) {
        let mut file = File::open(path.clone()).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        let data: SequencerState = serde_json::from_str(&contents.as_str()).unwrap();
        self.sequencer.load_from_state(data);
        self.file = path.file_name().unwrap().to_str().unwrap().to_string();
        self.sequencer.loaded_file = self.file.clone();
        log::info!("Load file: {:?}", path);
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
            println!("Loading file");
            let file = self.file.clone();
            self.load_file(&PathBuf::from(file));
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                "{} - Automate",
                self.file.replace(".auto", "")
            )));
        }
        ctx.input(|i| {
            //Keybinds within app
            if i.key_released(egui::Key::F8) {
                self.sequencer.recording.swap(!self.sequencer.recording.load(Ordering::Relaxed), Ordering::Relaxed);
            }
            if i.key_pressed(egui::Key::Space) {
                self.sequencer.toggle_play();
            }
            if i.key_pressed(egui::Key::ArrowLeft) {
                self.sequencer.reset_time();
            }
            if i.key_pressed(egui::Key::ArrowRight) {
                self.sequencer.step_time();
            }
            if i.viewport().close_requested()
                && !self.sequencer.keyframes.lock().unwrap().is_empty()
                && self.file == "untitled.auto".to_string()
            {
                if !self.allowed_to_close{
                    log::info!("Trying to close without saving");
                    self.show_close_dialog = true;
                    //Todo: Make this work without freezing
                    //ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose); (this freezes)
                }
            }
        });

        if self.show_close_dialog {
            egui::Window::new("Automate")
                .resizable(false)
                .movable(true)
                .collapsible(false)
                .fixed_size(Vec2::new(200., 80.))
                .show(ctx, |ui| {
                    ui.heading("Do you want to save changes to Untitled?");
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            self.save_file();
                            self.show_close_dialog = false;
                            self.allowed_to_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button("Don't Save").clicked() {
                            self.show_close_dialog = false;
                            self.allowed_to_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
                    if ui.button("New").clicked() {
                        self.new_file();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                            "{} - Automate",
                            self.file.replace(".auto", "")
                        )));
                        ui.close_menu();
                    }

                    if ui.button("Save").clicked() {
                        self.save_file();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                            "{} - Automate",
                            self.file.replace(".auto", "")
                        )));
                        ui.close_menu();
                    }
                    if ui.button("Open").clicked() {
                        self.open_file();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                            "{} - Automate",
                            self.file.replace(".auto", "")
                        )));
                        ui.close_menu();
                    }
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        self.sequencer.update(&mut self.last_instant, ctx, self.offset);
        self.sequencer.show(ctx);
        self.sequencer.debug_panel(ctx, &mut self.offset);
        self.sequencer.selected_panel(ctx);

        ctx.request_repaint();
    }
}
