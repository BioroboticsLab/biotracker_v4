use super::app::PersistentState;
use super::video_plane::VideoPlane;
use libtracker::{message_bus::Client, Action, Message};

pub struct SidePanel {}

impl SidePanel {
    pub fn new() -> Self {
        Self {}
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        msg_bus: &mut Client,
        persistent_state: &mut PersistentState,
        video_plane: &mut VideoPlane,
    ) {
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.collapsing("Experiment", |ui| {
                    if ui.button("Add Entity").clicked() {
                        msg_bus
                            .send(Message::UserAction(Action::AddEntity))
                            .unwrap();
                    }
                    if ui.button("Remove Entity").clicked() {
                        msg_bus
                            .send(Message::UserAction(Action::RemoveEntity))
                            .unwrap();
                    }
                });
            });
            ui.collapsing("Interface", |ui| {
                ui.checkbox(&mut persistent_state.dark_mode, "Dark Mode");
                match persistent_state.dark_mode {
                    true => ctx.set_visuals(egui::Visuals::dark()),
                    false => ctx.set_visuals(egui::Visuals::light()),
                }
                let response = ui.add(egui::Slider::new(&mut persistent_state.scaling, 0.5..=3.0));
                if response.drag_released() || response.lost_focus() {
                    ctx.set_pixels_per_point(persistent_state.scaling);
                }
            });
            ui.collapsing("Video", |ui| {
                video_plane.show_settings(ui);
            });
        });
    }
}
