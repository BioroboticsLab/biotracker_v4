use std::collections::HashSet;

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
        image_streams: &HashSet<String>,
        view_image: &mut String,
        record_image: &mut String,
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
                    match RecordingState::from_i32(experiment.recording_state).unwrap() {
                        RecordingState::Initial | RecordingState::Finished => {
                            egui::ComboBox::from_label("Select Recording Image")
                                .selected_text(record_image.clone())
                                .show_ui(ui, |ui| {
                                    for image in image_streams {
                                        if ui
                                            .selectable_label(*image == *record_image, image)
                                            .clicked()
                                        {
                                            *record_image = image.clone();
                                        }
                                    }
                                });
                            if ui.button("Start Recording").clicked() {
                                command = Some(BioTrackerCommand::RecordingState(
                                    RecordingState::Recording,
                                ));
                            }
                        }
                        RecordingState::Recording => {
                            if ui.button("Stop Recording").clicked() {
                                command = Some(BioTrackerCommand::RecordingState(
                                    RecordingState::Finished,
                                ));
                            }
                        }
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
                egui::ComboBox::from_label("Show Image")
                    .selected_text(view_image.clone())
                    .show_ui(ui, |ui| {
                        for image in image_streams {
                            if image == "Annotated" {
                                continue;
                            }
                            if ui.selectable_label(*image == *view_image, image).clicked() {
                                *view_image = image.clone();
                            }
                        }
                    });
            });
        });
        command
    }
}
