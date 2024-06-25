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
    file: String,
    #[serde(skip)] 
    saved_file_uptodate: bool,
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
            saved_file_uptodate: true,
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
        if self.file != "untitled.auto"{
            self.sequencer.keyframes.lock().unwrap().clear();
            self.sequencer.keyframe_state.lock().unwrap().clear();
            self.sequencer.reset_time();
            self.file = "untitled.auto".to_string();
            self.sequencer.loaded_file = self.file.clone();
            self.saved_file_uptodate = true;
            self.sequencer.changed.swap(false, Ordering::Relaxed);
            log::info!("New file: {:?}","untitled.auto");
        }else{
            self.show_close_dialog = true;
        }
    }
    fn save_file(&mut self){
        let state = self.sequencer.save_to_state();
        let json = serde_json::to_string_pretty(&state);
        if json.is_ok() {
            if self.file == "untitled.auto"{
                self.file = FileDialog::new()
                .add_filter("automate", &["auto"])
                .set_directory("/")
                .save_file()
                .unwrap().file_name().unwrap().to_str().unwrap().to_string();
            }

            let mut file = File::create(self.file.clone()).unwrap();
            file.write_all(json.unwrap().as_bytes()).unwrap();
            self.sequencer.loaded_file = self.file.clone();
            self.saved_file_uptodate = true;
            self.sequencer.changed.swap(false, Ordering::Relaxed);
            log::info!("Save file: {:?}", self.file);
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
        let stream = File::open(path.clone());
        if stream.is_ok(){
            let mut file = stream.unwrap();
            let mut contents = String::new();
            file.read_to_string(&mut contents).unwrap();
            let data: SequencerState = serde_json::from_str(&contents.as_str()).unwrap();
            self.sequencer.load_from_state(data);
            self.file = path.file_name().unwrap().to_str().unwrap().to_string();
            self.sequencer.loaded_file = self.file.clone();
            self.saved_file_uptodate = true;
            log::info!("Load file: {:?}", path);
        }else{
            self.new_file();
            log::info!("Failed to load file: {:?} - {:?}", path,stream.err().unwrap());
        }
    }
    fn set_title(&self, ctx: &egui::Context){
        let saved = match self.saved_file_uptodate{
            true=>"",
            false=>"*",
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{}{} - Automate",
            self.file,//.replace(".auto", ""),
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
        if self.sequencer.changed.load(Ordering::Relaxed) || !self.saved_file_uptodate{
            self.saved_file_uptodate = false;
        }
        self.set_title(ctx);
        let mut cancel_close = false;
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
            if i.modifiers.ctrl{
                if i.key_pressed(egui::Key::S){
                    self.save_file();
                }
                if i.key_pressed(egui::Key::N){
                    self.new_file();
                }
                if i.key_pressed(egui::Key::ArrowRight) {
                    self.sequencer.selected_keyframes.sort();
                    let keyframe_state = self.sequencer.keyframe_state.lock().unwrap();
                    let mut last= 0;
                    if !keyframe_state.is_empty(){
                        if !self.sequencer.selected_keyframes.is_empty() { 
                            let next = self.sequencer.selected_keyframes.last().unwrap().clone();
                            println!("next {next}");
                            if keyframe_state.len() > last{
                                println!(" +1");
                                last = next+1;
                            }else{
                                println!(" -2");
                                last = next -2;
                            }
                        }
                        if i.modifiers.shift {
                            println!("  shift");
                            if !self.sequencer.selected_keyframes.contains(&last){
                                println!("  not contains: push");
                                self.sequencer.selected_keyframes.push(last)
                            }else{
                                println!("  contains: no push");
                            }
                        }else{
                            self.sequencer.selected_keyframes = [last].into();
                        }
                    }
                }
                if i.key_pressed(egui::Key::ArrowLeft){
                    self.sequencer.selected_keyframes.sort();
                    let keyframe_state = self.sequencer.keyframe_state.lock().unwrap();
                    let mut last = 0;
                    if !keyframe_state.is_empty(){
                        if !self.sequencer.selected_keyframes.is_empty() { 
                            let next = self.sequencer.selected_keyframes.first().unwrap().clone();
                            println!("next {next}");
                            if next > 0{
                                last = next - 1;
                                println!(" -1");
                            }else{
                                println!("  0");
                                last = 0;
                            }
                        }else{
                            last = 0;
                        }
                        if i.modifiers.shift {
                            println!("  shift");
                            if !self.sequencer.selected_keyframes.contains(&last){
                                println!("  not contain: push");
                                self.sequencer.selected_keyframes.push(last)
                            }else {
                                println!("  contain: no push");
                            }
                        }else{
                            self.sequencer.selected_keyframes = [last].into();
                        }
                    }
                }
            }else{//For keybinds that have conflicting keystrokes with a modifier
                if i.key_pressed(egui::Key::ArrowRight) {
                    self.sequencer.step_time();
                }
            }
            if i.key_pressed(egui::Key::Tab){
                println!("{:?}",i.viewport().inner_rect.unwrap().size())
            }

            if i.viewport().close_requested()
                && !self.saved_file_uptodate
            {
                if !self.allowed_to_close{
                    log::info!("Close without saving?");
                    self.show_close_dialog = true;
                    cancel_close = true;
                }
            }
        });

        if self.show_close_dialog {
            egui::Window::new("Automate")
                .resizable(false)
                .movable(true)
                .collapsible(false)
                // .fixed_size(Vec2::new(400., 150.))
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
                        self.set_title(ctx);
                        ui.close_menu();
                    }

                    if ui.button("Save").clicked() {
                        self.save_file();
                        self.set_title(ctx);
                        ui.close_menu();
                    }
                    if ui.button("Open").clicked() {
                        self.open_file();
                        self.set_title(ctx);
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

        if cancel_close{
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        ctx.request_repaint();
    }
}
