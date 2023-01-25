use super::{
    annotated_video::AnnotatedVideo, offscreen_renderer::OffscreenRenderer, side_panel::SidePanel,
};
use crate::components::biotracker::BioTrackerCommand;
use anyhow::{anyhow, Result};
use libtracker::{message_bus::Client, protocol::*, CommandLineArguments};
use std::collections::HashSet;

pub struct PersistentState {
    pub dark_mode: bool,
    pub scaling: f32,
}

pub struct BioTrackerUI {
    experiment: ExperimentState,
    persistent_state: PersistentState,
    msg_bus: Client,
    side_panel: SidePanel,
    current_timestamp: u64,
    seek_target: u32,
    render_offscreen: bool,
    offscreen_renderer: OffscreenRenderer,
    video_view: AnnotatedVideo,
    image_streams: HashSet<String>,
    view_image: String,
    record_image: String,
}

impl BioTrackerUI {
    pub fn new(cc: &eframe::CreationContext, _args: CommandLineArguments) -> Option<Self> {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        cc.egui_ctx.set_pixels_per_point(1.5);

        let persistent_state = PersistentState {
            dark_mode: false,
            scaling: 1.5,
        };

        let msg_bus = Client::new().unwrap();
        msg_bus
            .subscribe(&[
                Topic::Features,
                Topic::Entities,
                Topic::ExperimentState,
                Topic::Image,
            ])
            .unwrap();

        let render_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("WGPU render state not available");
        let offscreen_renderer = OffscreenRenderer::new(
            render_state.device.clone(),
            render_state.queue.clone(),
            1024,
            1024,
        );

        Some(Self {
            experiment: Default::default(),
            persistent_state,
            msg_bus,
            side_panel: SidePanel::new(),
            current_timestamp: 0,
            seek_target: 0,
            offscreen_renderer,
            render_offscreen: false,
            video_view: AnnotatedVideo::new(),
            image_streams: HashSet::new(),
            view_image: "Tracking".to_string(),
            record_image: "Tracking".to_string(),
        })
    }

    fn send_command(&self, command: BioTrackerCommand) {
        self.msg_bus
            .send(Message::ComponentMessage(ComponentMessage {
                recipient_id: "BioTracker".to_owned(),
                content: Some(component_message::Content::CommandJson(
                    serde_json::to_string(&command).unwrap(),
                )),
            }))
            .unwrap();
    }

    fn filemenu(&mut self) -> Result<()> {
        if let Some(pathbuf) = rfd::FileDialog::new().pick_file() {
            let path_str = pathbuf
                .to_str()
                .ok_or(anyhow!("Failed to get string from pathbuf"))?;
            self.send_command(BioTrackerCommand::OpenVideo(path_str.to_owned()));
        }
        Ok(())
    }
}

impl eframe::App for BioTrackerUI {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.video_view.update_scale(ctx.input().zoom_delta());
        let mut last_image = None;
        while let Ok(Some(msg)) = self.msg_bus.poll(0) {
            match msg {
                Message::Image(img) => {
                    if !self.image_streams.contains(&img.stream_id) {
                        self.image_streams.insert(img.stream_id.clone());
                    }
                    if img.stream_id == self.view_image {
                        self.render_offscreen = true;
                        last_image = Some(img);
                    }
                }
                Message::Features(features) => {
                    self.video_view.update_features(features);
                }
                Message::ExperimentState(state) => {
                    self.experiment = state;
                }
                Message::Entities(entities) => {
                    self.video_view.update_entities(entities);
                }
                _ => eprintln!("Unexpected message {:?}", msg),
            }
        }

        // We render the offscreen image after receiving a new image, but before actually loading
        // it. This way, entities that are received between two frames, will be rendered on the
        // first. If entities arrive after this cutoff, they will lag behind in the rendered video.
        if self.render_offscreen {
            self.offscreen_renderer.render(|offscreen_ctx| {
                egui::CentralPanel::default().show(offscreen_ctx, |ui| {
                    self.video_view.show_offscreen(ui);
                });
            });
        }

        // we defer actually reading the image until after the message queue is drained. This way,
        // we always skip to the newest frame. This happens, when the application does not render,
        // because it is minimised.
        if let Some(image) = last_image {
            self.current_timestamp = image.timestamp;
            let render_state = frame.wgpu_render_state().unwrap();
            if self.offscreen_renderer.texture.size.width != image.width
                || self.offscreen_renderer.texture.size.height != image.height
            {
                self.offscreen_renderer = OffscreenRenderer::new(
                    render_state.device.clone(),
                    render_state.queue.clone(),
                    image.width,
                    image.height,
                );
            }
            self.video_view.update_image(
                image,
                render_state,
                &self.offscreen_renderer.render_state,
            );
        }

        // Window menu
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Media").clicked() {
                        self.filemenu().unwrap();
                        ui.close_menu();
                    }
                    if ui.button("Quit").clicked() {
                        frame.close();
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::warn_if_debug_build(ui);
                });
            });
        });

        // Video controls
        egui::TopBottomPanel::bottom("video_control").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(video_info) = &self.experiment.video_info {
                    let frame_count = video_info.frame_count;
                    match PlaybackState::from_i32(self.experiment.playback_state) {
                        Some(PlaybackState::Playing) => {
                            if ui.add(egui::Button::new("⏸")).clicked() {
                                self.send_command(BioTrackerCommand::PlaybackState(
                                    PlaybackState::Paused,
                                ));
                            }
                        }
                        _ => {
                            if ui.add(egui::Button::new("▶")).clicked() {
                                self.send_command(BioTrackerCommand::PlaybackState(
                                    PlaybackState::Playing,
                                ));
                            }
                        }
                    };

                    let available_size = ui.available_size_before_wrap();
                    let label_size = ui.available_size() / 8.0;
                    let slider_size = available_size - (label_size);

                    ui.label(&self.current_timestamp.to_string());
                    ui.spacing_mut().slider_width = slider_size.x;
                    if frame_count > 0 {
                        let response = ui.add(
                            egui::Slider::new(&mut self.seek_target, 0..=frame_count)
                                .show_value(false),
                        );
                        if response.drag_released() || response.lost_focus() || response.changed() {
                            self.send_command(BioTrackerCommand::Seek(self.seek_target));
                        }

                        ui.label(&frame_count.to_string());
                    }
                } else {
                    if ui.add(egui::Button::new("▶")).clicked() {
                        self.filemenu().unwrap();
                    }
                }
            });
        });

        // Side panel
        if let Some(cmd) = self.side_panel.show(
            ctx,
            &mut self.experiment,
            &mut self.persistent_state,
            &mut self.video_view,
            &self.image_streams,
            &mut self.view_image,
            &mut self.record_image,
        ) {
            self.send_command(cmd);
        }

        // Video view
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both()
                .max_width(f32::INFINITY)
                .max_height(f32::INFINITY)
                .show(ui, |ui| {
                    self.video_view.show_onscreen(ui);
                });
        });

        ctx.request_repaint();
    }

    fn post_rendering(&mut self, _window_size_px: [u32; 2], _frame: &eframe::Frame) {
        if self.render_offscreen {
            self.offscreen_renderer
                .post_rendering(&self.msg_bus, self.current_timestamp)
                .unwrap();
            self.render_offscreen = false;
        }
    }

    fn on_exit(&mut self) {
        self.msg_bus.send(Message::Shutdown(Shutdown {})).unwrap();
    }
}
