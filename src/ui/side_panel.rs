use super::{annotated_video::AnnotatedVideo, app::BioTrackerUIContext};
use crate::biotracker::protocol::*;

pub struct SidePanel {}

impl SidePanel {
    pub fn new() -> Self {
        Self {}
    }

    pub fn show(
        &mut self,
        egui_ctx: &egui::Context,
        ctx: &mut BioTrackerUIContext,
        video_view: &mut AnnotatedVideo,
    ) {
        egui::SidePanel::left("side_panel").show(egui_ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.collapsing("Experiment", |ui| {
                    if ui.button("Add Entity").clicked() {
                        ctx.bt.command(Command::AddEntity("".to_string())).unwrap();
                    }
                    if ui.button("Remove Entity").clicked() {
                        ctx.bt
                            .command(Command::RemoveEntity("".to_string()))
                            .unwrap();
                    }
                    match RecordingState::from_i32(ctx.experiment.recording_state).unwrap() {
                        RecordingState::Initial | RecordingState::Finished => {
                            egui::ComboBox::from_label("Select Recording Image")
                                .selected_text(ctx.record_image.clone())
                                .show_ui(ui, |ui| {
                                    for image in
                                        ctx.image_streams.iter().chain([&"Annotated".to_owned()])
                                    {
                                        if ui
                                            .selectable_label(*image == *ctx.record_image, image)
                                            .clicked()
                                        {
                                            ctx.record_image = image.clone();
                                        }
                                    }
                                });
                            if ui.button("Start Recording").clicked() {
                                ctx.bt
                                    .command(Command::VideoEncoderConfig(VideoEncoderConfig {
                                        video_path: "test.mp4".to_string(),
                                        fps: 25.0,
                                        width: 1024,
                                        height: 1024,
                                        image_stream_id: ctx.record_image.clone(),
                                    }))
                                    .unwrap();
                                ctx.bt
                                    .command(Command::RecordingState(
                                        RecordingState::Recording as i32,
                                    ))
                                    .unwrap();
                            }
                        }
                        RecordingState::Recording => {
                            if ui.button("Stop Recording").clicked() {
                                ctx.bt
                                    .command(Command::RecordingState(
                                        RecordingState::Finished as i32,
                                    ))
                                    .unwrap();
                            }
                        }
                    }
                });
            });
            ui.collapsing("Interface", |ui| {
                ui.checkbox(&mut ctx.persistent_state.dark_mode, "Dark Mode");
                match ctx.persistent_state.dark_mode {
                    true => egui_ctx.set_visuals(egui::Visuals::dark()),
                    false => egui_ctx.set_visuals(egui::Visuals::light()),
                }
                let response = ui.add(egui::Slider::new(
                    &mut ctx.persistent_state.scaling,
                    0.5..=3.0,
                ));
                if response.drag_released() || response.lost_focus() {
                    egui_ctx.set_pixels_per_point(ctx.persistent_state.scaling);
                }
            });
            ui.collapsing("Video", |ui| {
                video_view.show_settings(ui);
                egui::ComboBox::from_label("Show Image")
                    .selected_text(ctx.view_image.clone())
                    .show_ui(ui, |ui| {
                        for image in &ctx.image_streams {
                            if image == "Annotated" {
                                continue;
                            }
                            if ui
                                .selectable_label(*image == *ctx.view_image, image)
                                .clicked()
                            {
                                ctx.view_image = image.clone();
                            }
                        }
                    });
            });
        });
    }
}
