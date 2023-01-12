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
        experiment: &mut ExperimentState,
        persistent_state: &mut PersistentState,
        video_plane: &mut VideoPlane,
    ) {
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.collapsing("Experiment", |ui| {
                    let mut new_entity_count = experiment.entity_count;
                    if ui.button("Add Entity").clicked() {
                        new_entity_count += 1;
                    }
                    if ui.button("Remove Entity").clicked() && experiment.entity_count > 0 {
                        new_entity_count -= 1;
                    }
                    if new_entity_count != experiment.entity_count {
                        msg_bus
                            .send(Message::ExperimentUpdate(ExperimentUpdate {
                                entity_count: Some(new_entity_count),
                                ..Default::default()
                            }))
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
            ui.collapsing("Video Recorder", |ui| {
                match experiment.video_encoder_state.as_ref() {
                    Some(state) => {
                        ui.label(format!("Recording to {}", state.path));
                    }
                    None => {
                        ui.label("Video Encoder not running");
                    }
                }

                //match (&component_state.decoder, &mut component_state.encoder) {
                //(Some(_decoder), Some(_encoder)) => {
                //if ui.button("Stop Recording").clicked() {
                //let cmd = VideoEncoderCommand {
                //state: Some(VideoState::Stopped.into()),
                //..Default::default()
                //};
                //msg_bus.send(Message::VideoEncoderCommand(cmd)).unwrap();
                //}
                //}
                //(Some(decoder), None) => {
                //if ui.button("Start Recording").clicked() {
                //let cmd = VideoEncoderCommand {
                //settings: Some(VideoEncoderState {
                //path: "test.mp4".to_string(),
                //width: decoder.width,
                //height: decoder.height,
                //fps: decoder.fps,
                //state: VideoState::Playing.into(),
                //}),
                //state: None,
                //};
                //msg_bus.send(Message::VideoEncoderCommand(cmd)).unwrap();
                //}
                //}
                //(None, _) => {
                //ui.label("No video source found");
                //}
                //}
            });
        });
    }
}
