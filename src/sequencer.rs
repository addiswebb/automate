use eframe::egui::{self, pos2, vec2, Button, Ui, Vec2};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Transition {
    #[default]
    None,
    CloseOnNextFrame,
    CloseImmediately,
}

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

/// A simple keypad widget.
pub struct Sequencer {
    id: egui::Id,
}

impl Sequencer {
    pub fn new() -> Self {
        Self {
            id: egui::Id::new("keypad"),
        }
    }

    pub fn bump_events(&self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        let events = ctx.memory_mut(|m| {
            m.data
                .get_temp_mut_or_default::<State>(self.id)
                .events
                .take()
        });
        if let Some(mut events) = events {
            events.append(&mut raw_input.events);
            raw_input.events = events;
        }
    }

    

    pub fn show(&self, ctx: &egui::Context) {
        let (focus, mut state) = ctx.memory(|m| {
            (
                m.focused(),
                m.data.get_temp::<State>(self.id).unwrap_or_default(),
            )
        });

        state.open = true;
        state.start_pos = pos2(100.0, 100.0);
        state.focus = focus;

        if state.close_on_next_frame {
            state.open = false;
            state.close_on_next_frame = false;
            state.focus = None;
        }

        let mut open = state.open;

        let w = egui::Window::new("Sequencer");
        let resp = w
            .movable(true)
            .resizable(true)
            .collapsible(false)
            .open(&mut open)
            . show(ctx, |ui| {
                use egui_extras::{Column, TableBuilder};
                let mut table = TableBuilder::new(ui) 
                    .striped(true)
                    .resizable(false)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto())
                    .column(Column::remainder())
                    .sense(egui::Sense::hover());
                //allow rows to be clicked
                table = table.sense(egui::Sense::click());

                table.header(20.0,|mut header|{
                    header.col(|ui|{
                        ui.strong("Inputs");
                    });
                    header.col(|ui|{
                        ui.strong("Keyframes");
                    });
                })
                .body(|mut body|{
                    body.row(20.0,|mut row|{
                        row.col(|ui|{
                            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());

                            // Create a "canvas" container to paint Shapes inside of and check for hover
                            let (response, painter) = ui.allocate_painter(
                                vec2(ui.available_width(), 300.0),
                                egui::Sense::hover(),
                            );
                            ui.put(
                                egui::Rect {
                                            // Coordinates of "top left"
                                    min: egui::Pos2 { x: 0.0, y: 0.0 },
                                            // Coordinates of "bottom right"
                                    max: egui::Pos2 { x: 30.0, y: 40.0 },
                                },
                                egui::Label::new("#1"),
                            );
                        });
                        row.col(|ui|{
                            ui.label("keyframes go here:");
                        });
                    });
                    
                    body.row(20.0,|mut row|{
                        row.col(|ui|{
                            ui.label("Mouse");
                        });
                        row.col(|ui|{
                            ui.label("keyframes go here:");
                        });
                    });
                })

                
            });

        state.open = open;
        
        ctx.memory_mut(|m| m.data.insert_temp(self.id, state));
    }
}   

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}