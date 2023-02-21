use super::{annotated_video::AnnotatedVideo, app::BioTrackerUIContext};
use crate::biotracker::protocol::*;

pub struct WindowMenu {}

impl WindowMenu {
    pub fn new() -> Self {
        Self {}
    }

    pub fn show(
        &mut self,
        egui_ctx: &egui::Context,
        ctx: &mut BioTrackerUIContext,
        frame: &mut eframe::Frame,
        video_view: &mut AnnotatedVideo,
    ) {
        egui::TopBottomPanel::top("menu_bar").show(egui_ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Media").clicked() {
                        self.filemenu(ctx);
                        ui.close_menu();
                    }
                    if ui.button("Quit").clicked() {
                        frame.close();
                    }
                });
                ui.menu_button("Experiment", |ui| {
                    if ui.button("Add Entity").clicked() {
                        ctx.bt.command(Command::AddEntity("".to_string())).unwrap();
                    }
                    if ui.button("Remove Entity").clicked() {
                        ctx.bt
                            .command(Command::RemoveEntity("".to_string()))
                            .unwrap();
                    }
                    if ui
                        .checkbox(&mut ctx.experiment.realtime_mode, "Realtime Tracking")
                        .changed()
                    {
                        ctx.bt
                            .command(Command::RealtimeMode(ctx.experiment.realtime_mode))
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
                ui.menu_button("Video", |ui| {
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
                ui.menu_button("Settings", |ui| {
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
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::warn_if_debug_build(ui);
                });
            });
        });
    }

    pub fn filemenu(&self, ctx: &mut BioTrackerUIContext) {
        if let Some(pathbuf) = rfd::FileDialog::new().pick_file() {
            let path_str = pathbuf
                .to_str()
                .ok_or(anyhow::anyhow!("Failed to get string from pathbuf"))
                .unwrap();
            match ctx.bt.command(Command::OpenVideo(path_str.to_owned())) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Failed to open video: {}", e);
                }
            }
        }
    }
}
