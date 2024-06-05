use std::time::Instant;

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
                duration: 4.0,
                keyframe_type: KeyframeType::MouseBtn(0),
                id: 1,
            })
            .add_keyframe(Keyframe{
                timestamp: 2.5,
                duration: 2.0,
                keyframe_type: KeyframeType::KeyBtn("test".to_owned()),
                id: 0,
            })
            .add_keyframe(Keyframe{
                timestamp: 13.0,
                duration: 2.0,
                keyframe_type: KeyframeType::MouseMove(egui::Vec2{x: 0.0,y:0.0}),
                id: 2,
            })
            .add_keyframe(Keyframe{
                timestamp: 8.0,
                duration: 3.0,
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
                    if ui.button("Open Sequencer").clicked(){
                        self.sequencer.open(true);
                    }
                });
            });
        });

        egui::Window::new("Selected Keyframe")
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui|{
            if let Some(i) = self.sequencer.selected_keyframe {
                match &self.sequencer.keyframes[i].keyframe_type {
                    KeyframeType::KeyBtn(keys) => {
                        ui.strong("Keyboard Button press");
                        ui.label("key strokes");
                        ui.text_edit_singleline(&mut keys.to_string());
                    }
                    KeyframeType::MouseBtn(key) =>{
                        ui.strong("Mouse Button press");
                        ui.label(format!("button: {:?}",key));
                    }
                    KeyframeType::MouseMove(pos)=>{
                        ui.strong("Mouse move");
                        //ui.text_edit_singleline(&mut self.sequencer.keyframes[i].keyframe_type)
                        ui.label(format!("position: {:?}",pos));
                    }
                }

                ui.label("Timestamp");
                ui.add(egui::DragValue::new(&mut self.sequencer.keyframes[i].timestamp).speed(0.25).clamp_range(0.0..=100.0));
                ui.label("Duration");
                ui.add(egui::DragValue::new(&mut self.sequencer.keyframes[i].duration).speed(0.1).clamp_range(0.1..=10.0));
            }
        });
        
        self.sequencer.show(ctx);
        let mut last_instant = Instant::now();
        self.sequencer.update(&mut last_instant);
        ctx.request_repaint();
    }
}