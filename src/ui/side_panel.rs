use crate::components::biotracker::BioTrackerCommand;

use super::annotated_video::AnnotatedVideo;
use super::app::PersistentState;
use libtracker::protocol::*;

pub struct SidePanel {}

impl SidePanel {
    pub fn new() -> Self {
        Self {}
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        experiment: &mut ExperimentState,
        persistent_state: &mut PersistentState,
        video_view: &mut AnnotatedVideo,
    ) -> Option<BioTrackerCommand> {
        let mut command = None;
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.collapsing("Experiment", |ui| {
                    if ui.button("Add Entity").clicked() {
                        command = Some(BioTrackerCommand::AddEntity);
                    }
                    if ui.button("Remove Entity").clicked() {
                        command = Some(BioTrackerCommand::RemoveEntity);
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
                video_view.show_settings(ui);
            });
            ui.collapsing("Recording", |ui| {
                match RecordingState::from_i32(experiment.recording_state).unwrap() {
                    RecordingState::Initial | RecordingState::Finished => {
                        if ui.button("Start Recording").clicked() {
                            command =
                                Some(BioTrackerCommand::RecordingState(RecordingState::Recording));
                        }
                    }
                    RecordingState::Recording => {
                        if ui.button("Stop Recording").clicked() {
                            command =
                                Some(BioTrackerCommand::RecordingState(RecordingState::Finished));
                        }
                    }
                }
            });
        });
        command
    }
}
