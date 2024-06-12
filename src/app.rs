use std::{fs::File, io::{Read, Write}, time::Instant};
use crate::sequencer::{Keyframe, KeyframeType, Sequencer};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    // Example stuff:
    label: String,
    #[serde(skip)] // This how you opt-out of serialization of a field
    // value: f32,
    sequencer: Sequencer,
    #[serde(skip)]
    last_instant: Instant,
}

impl Default for App {
    fn default() -> Self {
        Self {
            label: "Automate".to_owned(),
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
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        self.sequencer.keyframes.lock().unwrap().clear();
                        self.sequencer.playing_keyframes.lock().unwrap().clear();
                        ui.close_menu();
                    }
                    
                    if ui.button("Save").clicked() {
                        let json = serde_json::to_string(self.sequencer.keyframes.as_ref());
                        if json.is_ok(){
                            let mut file = File::create("file.auto").unwrap();
                            let json = json.unwrap(); 
                            file.write_all(json.as_bytes()).unwrap();
                            log::info!("Save file: 'file.auto'");
                        }else{
                            log::error!("Failed to save 'file.auto'");
                        }
                        ui.close_menu();
                    }
                    if ui.button("Load").clicked(){
                        log::info!("Load file: 'file.auto'");
                        let mut file = File::open("file.auto").unwrap();
                        let mut contents = String::new();
                        file.read_to_string(&mut contents).unwrap();
                        let data: Vec<Keyframe> = serde_json::from_str(&contents.as_str()).unwrap();
                        let mut shared_kfs = self.sequencer.keyframes.lock().unwrap();
                        let mut shared_pkfs = self.sequencer.playing_keyframes.lock().unwrap();
                        shared_kfs.clear();
                        shared_kfs.extend(data.into_iter());
                        shared_pkfs.clear();
                        shared_pkfs.extend(vec![0;shared_kfs.len()].into_iter());
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
