use egui::Vec2;
use rfd::FileDialog;
use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
    sync::atomic::Ordering,
    time::Instant,
};

use crate::sequencer::{Sequencer, SequencerState};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct App {
    #[serde(skip)] //Serializing creates two threads somehow
    sequencer: Sequencer,
    #[serde(skip)]
    last_instant: Instant,
    offset: Vec2,
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
}

impl Default for App {
    fn default() -> Self {
        Self {
            sequencer: Sequencer::new(),
            last_instant: Instant::now(),
            offset: Vec2::ZERO,
            file: "untitled.auto".to_string(),
            file_uptodate: true,
            allowed_to_close: false,
            show_close_dialog: false,
            is_dialog_to_close: false,
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
        let json = serde_json::to_string_pretty(&state);
        if let Ok(json) = json {
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
                file.write_all(json.as_bytes()).unwrap();
                self.sequencer.loaded_file = self.file.clone();
                self.file_uptodate = true;
                self.sequencer.changed.swap(false, Ordering::Relaxed);
                log::info!("Save file: {:?}", self.file);
            } else {
                log::error!("Failed to save {:?}", file);
            }
        } else {
            log::error!("Failed to save sequencer to json");
        }
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
            let mut contents = String::new();
            file.read_to_string(&mut contents).unwrap();
            let data: SequencerState = serde_json::from_str(&contents.as_str()).unwrap();
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
            

            self.sequencer.zoom(i.smooth_scroll_delta.x);
            self.sequencer.scroll(i.smooth_scroll_delta.y);

            // Handle keybinds within app with focus
            if i.modifiers.ctrl {
                // Keybind(ctrl+s): Save file 
                if i.key_pressed(egui::Key::S) {
                    self.save_file();
                }
                // Keybind(ctrl+n): Create a new file
                if i.key_pressed(egui::Key::N) {
                    self.new_file();
                }
                // Todo(addis): make this work with uuids instead of indices
                // Keybind(ctrl+right): Select the next keyframe to the right
                // if i.key_pressed(egui::Key::ArrowRight) {
                //     self.sequencer.selected_keyframes.sort();
                //     let keyframe_state = self.sequencer.keyframe_state.lock().unwrap();
                //     let mut last = 0;
                //     if !keyframe_state.is_empty() {
                //         if !self.sequencer.selected_keyframes.is_empty() {
                //             let next = self.sequencer.selected_keyframes.last().unwrap().clone();
                //             if keyframe_state.len() > next + 1{
                //                 last = next + 1;
                //             } else {
                //                 last = next;
                //             }
                //         }
                //         if i.modifiers.shift {
                //             if !self.sequencer.selected_keyframes.contains(&last) {
                //                 self.sequencer.selected_keyframes.push(last)
                //             } else {
                //             }
                //         } else {
                //             self.sequencer.selected_keyframes = [last].into();
                //         }
                //     }
                // }
                // // Keybind(ctrl+left): Select the next keyframe to the left
                // if i.key_pressed(egui::Key::ArrowLeft) {
                //     self.sequencer.selected_keyframes.sort();
                //     let keyframe_state = self.sequencer.keyframe_state.lock().unwrap();
                //     let mut last = 0;
                //     if !keyframe_state.is_empty() {
                //         let next = self.sequencer.selected_keyframes.first();
                //         if let Some(&next) = next {
                //             if next > last {
                //                 last = next - 1;
                //             } else {
                //                 last = 0;
                //             }
                //         } else {
                //             last = 0;
                //         }
                //         if i.modifiers.shift {
                //             if !self.sequencer.selected_keyframes.contains(&last) {
                //                 self.sequencer.selected_keyframes.push(last)
                //             }
                //         } else {
                //             self.sequencer.selected_keyframes = [last].into();
                //         }
                //     }
                //}
            } else {
                // Keybind(right): Step forward 0.1 seconds in time
                if i.key_pressed(egui::Key::ArrowRight) {
                    self.sequencer.step_time();
                }
                // Keybind(space): Toggle play
                if i.key_pressed(egui::Key::Space) {
                    self.sequencer.toggle_play();
                }
                // Keybind(left): Reset the playhead/time to 0 seconds
                if i.key_pressed(egui::Key::ArrowLeft) {
                    self.sequencer.reset_time();
                }
                // Keybind(F8): Toggle recording
                if i.key_released(egui::Key::F8){
                    self.sequencer.recording.swap(!self.sequencer.recording.load(Ordering::Relaxed), Ordering::Relaxed,);
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
                    if ui.button("New File...").clicked() {
                        self.new_file();
                        self.set_title(ctx);
                        ui.close_menu();
                    }

                    if ui.button("Save File...").clicked() {
                        self.save_file();
                        self.set_title(ctx);
                        ui.close_menu();
                    }
                    if ui.button("Open File...").clicked() {
                        self.open_file();
                        self.set_title(ctx);
                        ui.close_menu();
                    }
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        self.sequencer
            .update(&mut self.last_instant, ctx, self.offset);
        self.sequencer.show(ctx);
        self.sequencer.debug_panel(ctx, &mut self.offset);
        self.sequencer.selected_panel(ctx);

        if cancel_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        ctx.request_repaint();
    }
}
