use super::app::BioTrackerUIContext;
use crate::{biotracker::protocol::*, util::generate_project_basename};

pub struct RecordButton {
    pub base_name: String,
    pub record_image_id: String,
    pub record_video: bool,
    pub dialog_open: bool,
}

impl Default for RecordButton {
    fn default() -> Self {
        Self {
            base_name: generate_project_basename(),
            record_image_id: "Tracking".to_owned(),
            record_video: true,
            dialog_open: false,
        }
    }
}

impl RecordButton {
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &mut BioTrackerUIContext) {
        match RecordingState::from_i32(ctx.experiment.recording_state).unwrap() {
            RecordingState::Recording => {
                let recording_icon = egui::RichText::new("⏺").color(egui::Color32::RED);
                if ui
                    .button(recording_icon.color(egui::Color32::RED))
                    .clicked()
                {
                    ctx.bt
                        .command(Command::RecordingState(RecordingState::Finished as i32))
                        .unwrap();
                }
            }
            RecordingState::Initial | RecordingState::Finished => {
                self.dialog_button(ui, ctx);
            }
        }
    }

    fn dialog_button(&mut self, ui: &mut egui::Ui, ctx: &mut BioTrackerUIContext) {
        let video_info = match &ctx.experiment.video_info {
            Some(video_info) => video_info,
            None => return,
        };
        let recording_icon = egui::RichText::new("⏺").color(egui::Color32::GRAY);
        let response = ui.toggle_value(&mut self.dialog_open, recording_icon);
        if self.dialog_open {
            egui::Window::new("Configure Recording")
                .fixed_pos(response.rect.center_top())
                .pivot(egui::Align2::LEFT_BOTTOM)
                .collapsible(false)
                .resizable(true)
                .show(ui.ctx(), |ui| {
                    egui::Grid::new("experiment_setup").show(ui, |ui| {
                        ui.end_row();
                        ui.label("Record video");
                        ui.toggle_value(&mut self.record_video, "");
                        ui.end_row();
                        ui.label("Recorded image");
                        egui::ComboBox::from_id_source("image_streams")
                            .selected_text(self.record_image_id.clone())
                            .show_ui(ui, |ui| {
                                for image in ["Tracking", "Annotated"] {
                                    if ui
                                        .selectable_label(*image == *self.record_image_id, image)
                                        .clicked()
                                    {
                                        self.record_image_id = image.to_owned();
                                    }
                                }
                            });
                        ui.end_row();
                        ui.label("Project name");
                        egui::TextEdit::singleline(&mut self.base_name)
                            .hint_text("Set base name for recorded files")
                            .show(ui);
                        ui.end_row();
                        if ui.button("Start").clicked() {
                            if self.record_video {
                                ctx.bt
                                    .command(Command::VideoEncoderConfig(VideoEncoderConfig {
                                        video_path: format!("{}.mp4", self.base_name),
                                        fps: video_info.fps,
                                        width: video_info.width,
                                        height: video_info.height,
                                        image_stream_id: self.record_image_id.clone(),
                                    }))
                                    .unwrap();
                            }
                            ctx.bt
                                .command(Command::RecordingState(RecordingState::Recording as i32))
                                .unwrap();
                            self.dialog_open = false;
                        }
                        if ui.button("Cancel").clicked() {
                            self.dialog_open = false;
                        }
                    });
                });
        }
    }
}
