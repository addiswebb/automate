use std::{fs::File, io::{Read, Write}, time::Instant};
use rfd::FileDialog;

use crate::sequencer::{Sequencer, SequencerState};


/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    #[serde(skip)]//Serializing creates two threads somehow
    sequencer: Sequencer,
    #[serde(skip)]
    last_instant: Instant,
}

impl Default for App {
    fn default() -> Self {
        Self {
            sequencer: Sequencer::new(),
            last_instant: Instant::now(),
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
}

impl eframe::App for App {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {

        //Keybinds within app
        ctx.input(|i| {
            if i.key_pressed(egui::Key::F8){
                self.sequencer.toggle_recording();
            }
            if i.key_pressed(egui::Key::Space){
                self.sequencer.toggle_play();
            }

            if i.key_pressed(egui::Key::ArrowLeft){
                self.sequencer.reset_time();
            }

            if i.key_pressed(egui::Key::ArrowRight){
                self.sequencer.step_time();
            }
            
        });

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        self.sequencer.keyframes.lock().unwrap().clear();
                        self.sequencer.playing_keyframes.lock().unwrap().clear();
                        self.sequencer.reset_time();
                        ui.close_menu();
                    }
                    
                    if ui.button("Save").clicked() {
                        let state = self.sequencer.save_to_state();
                        let json = serde_json::to_string(&state);
                        if json.is_ok(){
                            let path = FileDialog::new()
                                .add_filter("automate", &["auto"])
                                .set_directory("/")
                                .save_file().unwrap();
                            let mut file = File::create(path.clone()).unwrap();
                            let json = json.unwrap(); 
                            file.write_all(json.as_bytes()).unwrap();
                            log::info!("Save file: {:?}",path);
                        }else{
                            log::error!("Failed to save 'file.auto'");
                        }
                        ui.close_menu();
                    }
                    if ui.button("Open").clicked(){
                        let path = FileDialog::new()
                            .add_filter("automate", &["auto"])
                            .set_directory("/")
                            .pick_file().unwrap();
                        let mut file = File::open(path.clone()).unwrap();
                        let mut contents = String::new();
                        file.read_to_string(&mut contents).unwrap();
                        let data: SequencerState = serde_json::from_str(&contents.as_str()).unwrap();
                        self.sequencer.load_from_state(data);
                        log::info!("Load file: {:?}",path);
                        ui.close_menu();     
                    }
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });


        self.sequencer.update(&mut self.last_instant);
        self.sequencer.show(ctx);
        self.sequencer.debug_panel(ctx);
        self.sequencer.selected_panel(ctx);
        
        ctx.request_repaint();
    }
}
