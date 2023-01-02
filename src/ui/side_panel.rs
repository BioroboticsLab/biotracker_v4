use super::app::PersistentState;
use super::video_plane::VideoPlane;
use libtracker::{message_bus::Client, protocol::*};

pub struct SidePanel {}

impl SidePanel {
    pub fn new() -> Self {
        Self {}
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        msg_bus: &mut Client,
        experiment_state: &mut ExperimentState,
        persistent_state: &mut PersistentState,
        video_plane: &mut VideoPlane,
    ) {
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.collapsing("Experiment", |ui| {
                    let mut entity_count_changed = false;
                    if ui.button("Add Entity").clicked() {
                        experiment_state.entity_count += 1;
                        entity_count_changed = true;
                    }
                    if ui.button("Remove Entity").clicked() && experiment_state.entity_count > 0 {
                        experiment_state.entity_count -= 1;
                        entity_count_changed = true;
                    }
                    if entity_count_changed {
                        msg_bus
                            .send(Message::ExperimentState(experiment_state.clone()))
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
