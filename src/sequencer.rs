use std::num;

use eframe::egui::{self, pos2, Ui, Vec2};
use egui::{emath::RectTransform, Pos2, Rect};

const ROW_HEIGHT: f32 = 24.0;

#[derive(Clone, Debug)]
struct State {
    open: bool,
    closable: bool,
    close_on_next_frame: bool,
    start_pos: egui::Pos2,
    focus: Option<egui::Id>,
    events: Option<Vec<egui::Event>>,
}

impl State {
    fn new() -> Self {
        Self {
            open: false,
            closable: false,
            close_on_next_frame: false,
            start_pos: pos2(100.0, 100.0),
            focus: None,
            events: None,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}
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

pub struct Sequencer {
    id: egui::Id,
    open: bool,
    dragging: bool,
    can_resize: bool,
    could_resize: bool,
    resize_left: bool, //left: true, right: false
    resizing: bool,
    drag_start: Pos2,
    keyframes: Vec<Keyframe>,
    scale: f32,// egui points to seconds scale
}

impl Sequencer {
    pub fn new() -> Self {
        Self {
            id: egui::Id::new("sequencer"),
            open: true,
            keyframes: vec![],
            drag_start: pos2(0., 0.),
            dragging: false,
            can_resize: false,
            could_resize: false,
            resizing: false,
            resize_left: false,
            scale: 0.5,
        }
    }
    pub fn open(&mut self, open: bool){
        self.open = open;
    }
    pub fn add_keyframe(mut self, keyframe: Keyframe) -> Sequencer {
        println!("add keyframe: {:?}", keyframe);
        let mut tmp = vec![keyframe];
        self.keyframes.append(&mut tmp);
        self
    }

    fn render_keyframes(&mut self, ui: &mut Ui, id: u8) {
        let max_rect = ui.max_rect();
        for kf in self.keyframes.iter_mut() {
            if kf.id == id {
                let keyframe = ui.put(
                    time_to_rect(
                        kf.timestamp,
                        kf.duration,
                        self.scale,
                        ui.spacing().item_spacing,
                        max_rect,
                    ),
                    egui::Button::new("")
                        .sense(egui::Sense::click_and_drag())
                        .fill(egui::Color32::from_rgb(95, 186, 213))
                        .stroke(egui::Stroke::new(0.4, egui::Color32::from_rgb(15, 37, 42))),
                );
                //change icon to resize when at the edges of a keyframe
                if keyframe.hovered() {
                    let delta = 3.0;
                    let t = keyframe.hover_pos().unwrap().x;
                    let inner_left = keyframe.interact_rect.min.x + delta;
                    let inner_right = keyframe.interact_rect.max.x - delta;
                    if t < inner_left && t > keyframe.interact_rect.min.x {
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeEast);
                        self.resize_left = true; //resize left
                    } else if t < keyframe.interact_rect.max.x && t > inner_right {
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeEast);
                        self.resize_left = false; //resize right
                    } else {
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                    }
                    //self.drag_start = pos2(0., 0.);
                }
                if keyframe.drag_started() {
                    if let Some(start) = keyframe.interact_pointer_pos() {
                        self.drag_start = start;
                        self.dragging = true;
                    }
                }

                // if self.resizing{
                //     if let Some(end) = keyframe.interact_pointer_pos(){
                //         println!("resizing");
                //         let drag_delta = end.x - self.drag_start.x;
                //         let t = kf.timestamp + drag_delta;
                //         if t > 0.0{
                //             kf.duration += drag_delta;
                //             println!("increase duration {}", drag_delta);
                //             if self.resize_left{
                //                 //kf.timestamp +=drag_delta;
                //                 println!("move timestamp {}", drag_delta);
                //             }
                //         }
                //     }
                // }
                if self.dragging {
                    if let Some(end) = keyframe.interact_pointer_pos() {
                        //println!("dragging");
                        let drag_delta = end.x - self.drag_start.x;
                        let t = kf.timestamp + drag_delta;
                        //&& t < pos_to_time(max_rect.max, max_rect)-kf.duration
                        //stop from going to far left vv | ^^ to far right

                        if t > 0.0 {
                            kf.timestamp = kf.timestamp + drag_delta;
                            self.drag_start.x = end.x;
                        }
                    }
                }
                if keyframe.drag_stopped() {
                    println!("drag stopped");
                    self.drag_start = pos2(0., 0.);
                    self.dragging = false;
                    self.can_resize = false;
                    self.resizing = false;
                }
            }
        }
    }

    fn render_control_bar(&mut self, ui: &mut Ui) {
        if ui.button("⏪").clicked() { println!("go back"); }
        if ui.button("⏴").clicked() { println!("reverse");}
        if ui.button("⏵").clicked() { println!("play");}
        if ui.button("⏩").clicked() { println!("go forward");}
        ui.add(egui::DragValue::new(&mut self.scale).speed(0.1).clamp_range(0.01..=1.0));
    }
    fn render_timeline(&self, ui: &mut Ui){
        let width = ui.max_rect().size().x;
        let scale = 30.0 + self.scale*40.0;
        let num_of_digits = width/scale;
        let spacing = width/(num_of_digits);
        let pos = time_to_rect(0.0, 0.0,self.scale, ui.spacing().item_spacing, ui.max_rect()).min;
        for i in 0..(num_of_digits).ceil() as i32{
            let point = pos + egui::vec2(i as f32 * spacing, 0.0);
            ui.painter().text(point, egui::Align2::CENTER_TOP, format!("{}",i), egui::FontId::monospace(12.0), egui::Color32::GRAY);
            ui.painter().line_segment(
                [
                    pos2( point.x,ui.max_rect().max.y ),
                    pos2( point.x,ui.max_rect().max.y ) +egui::vec2(0.0,-6.0),
                ]
                , egui::Stroke::new(1.0, egui::Color32::GRAY));
        }
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        let mut open = self.open;

        let w = egui::Window::new("Sequencer");
        let resp = w
            .movable(true)
            .resizable(true)
            .collapsible(false)
            .open(&mut open)
            .show(ctx, |ui| {
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
                            row.col(|_| {
                            });
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
                            });
                        });
                    })
            });
        self.open = open;
    }
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}

fn pos_to_time(pos: Pos2, res_rect: Rect) -> f32 {
    let to_screen =
        RectTransform::from_to(res_rect, Rect::from_min_size(Pos2::ZERO, res_rect.size()));
    to_screen.transform_pos(pos).x
}
fn time_to_rect(t: f32, d: f32, scale: f32, spacing: Vec2, res_rect: Rect) -> Rect {
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
