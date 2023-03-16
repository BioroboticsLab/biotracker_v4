use super::{
    app::BioTrackerUIContext,
    settings::{filemenu, foldermenu},
};
use crate::biotracker::protocol::*;
use chrono::{Datelike, Timelike};

pub struct RecordButton {}

impl Default for RecordButton {
    fn default() -> Self {
        Self {}
    }
}

impl RecordButton {
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &mut BioTrackerUIContext) {
        match RecordingState::from_i32(ctx.experiment.recording_state).unwrap() {
            RecordingState::Replay => {
                let recording_icon = egui::RichText::new("⏺").color(egui::Color32::GREEN);
                if ui.button(recording_icon).clicked() {
                    if let Some(save_path) = filemenu() {
                        ctx.bt.command(Command::SaveTrack(save_path));
                    }
                    ctx.bt
                        .command(Command::RecordingState(RecordingState::Initial as i32));
                }
            }
            RecordingState::Recording => {
                let recording_icon = egui::RichText::new("⏺").color(egui::Color32::RED);
                if ui.button(recording_icon).clicked() {
                    ctx.bt
                        .command(Command::RecordingState(RecordingState::Finished as i32));
                }
            }
            RecordingState::Initial | RecordingState::Finished => {
                self.record_button(ui, ctx);
            }
        }
    }

    fn record_button(&mut self, ui: &mut egui::Ui, ctx: &mut BioTrackerUIContext) {
        let video_info = match &ctx.experiment.video_info {
            Some(video_info) => video_info,
            None => return,
        };
        let recording_icon = egui::RichText::new("⏺").color(egui::Color32::GRAY);
        if ui.button(recording_icon).clicked() {
            if let Some(recording_folder) = foldermenu() {
                if let Some(base_path) = std::path::Path::new(&recording_folder)
                    .join(timestamp())
                    .to_str()
                    .map(|s| s.to_owned())
                {
                    let image_stream_id = ctx.recording_image_id.clone();
                    ctx.bt
                        .command(Command::InitializeRecording(RecordingConfig {
                            base_path,
                            fps: video_info.fps,
                            width: video_info.width,
                            height: video_info.height,
                            image_stream_id,
                        }));
                    ctx.bt
                        .command(Command::RecordingState(RecordingState::Recording as i32));
                } else {
                    log::error!("Could not create recording path");
                }
            }
        }
    }
}

fn timestamp() -> String {
    let now = chrono::Local::now();
    format!(
        "{}-{:02}-{:02}-{:02}-{:02}-{:02}",
        now.year(),
        now.month(),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}
