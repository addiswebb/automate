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
}

impl Default for App {
    fn default() -> Self {
        let sequencer = Sequencer::new()
            // .add_keyframe(Keyframe{
            //     timestamp: 0.0,
            //     duration: 1.0,
            //     keyframe_type: KeyframeType::KeyBtn("Hello World".to_owned()),
            // })
            .add_keyframe(Keyframe{
                timestamp: 2.0,
                duration: 50.0,
                keyframe_type: KeyframeType::MouseBtn(0),
                id: 1,
            })
            .add_keyframe(Keyframe{
                timestamp: 150.0,
                duration: 10.0,
                keyframe_type: KeyframeType::KeyBtn("test".to_owned()),
                id: 0,
            })
            .add_keyframe(Keyframe{
                timestamp: 70.0,
                duration: 30.0,
                keyframe_type: KeyframeType::MouseBtn(0),
                id: 1,
            });
        Self {
            label: "Automate".to_owned(),
            sequencer,
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
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        
        self.sequencer.show(ctx);
        
    }
}