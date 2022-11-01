use crate::*;

struct UiState {
    settings_open: bool,
    dark_mode: bool,
    scaling: f32,
}

pub struct BioTracker {
    video_sampler: core::Sampler,
    play_state: core::VideoEvent,
    video_plane: Option<ui::TextureImage>,
    seekable: Option<core::VideoSeekable>,
    pts: core::Timestamp,
    ui_state: UiState,
}

impl BioTracker {
    pub fn new(cc: &eframe::CreationContext) -> Option<Self> {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        cc.egui_ctx.set_pixels_per_point(1.5);
        let video_sampler =
            core::Sampler::new("/home/max/Downloads/CameraCapture2022-09-29T15_27_47_1177687.avi")
                .expect("Failed to create video sampler");
        video_sampler.play().unwrap();

        Some(Self {
            video_sampler,
            play_state: core::VideoEvent::Pause,
            video_plane: None,
            seekable: None,
            pts: core::Timestamp(0),
            ui_state: UiState {
                dark_mode: false,
                scaling: 1.5,
                settings_open: false,
            },
        })
    }
}

impl eframe::App for BioTracker {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let render_state = frame.wgpu_render_state().unwrap();
        if let Some(sampler_event) = self.video_sampler.poll_event() {
            match sampler_event {
                core::SamplerEvent::Seekable(seekable) => {
                    self.seekable = Some(seekable);
                }
                core::SamplerEvent::Event(event) => {
                    self.play_state = event;
                }
            }
        }

        if let Ok(sample) = self.video_sampler.sample_rx.try_recv() {
            if let Some(caps) = sample.sample.caps() {
                let gst_info = gst_video::VideoInfo::from_caps(&caps).unwrap();
                let video_texture_size = wgpu::Extent3d {
                    width: gst_info.width(),
                    height: gst_info.height(),
                    depth_or_array_layers: 1,
                };
                match &self.video_plane {
                    Some(video_plane) => {
                        if video_plane.size != video_texture_size {
                            self.video_plane =
                                Some(ui::TextureImage::new(&render_state, video_texture_size))
                        }
                    }
                    None => {
                        self.video_plane =
                            Some(ui::TextureImage::new(&render_state, video_texture_size))
                    }
                }
            }

            if let Some(pts) = sample.pts() {
                self.pts = core::Timestamp(pts.nseconds());
            }

            if let Some(data) = sample.data() {
                if let Some(video_plane) = &self.video_plane {
                    video_plane.update(&render_state, data.as_slice());
                }
            }
        }

        {
            egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            frame.close();
                        }
                    });
                    ui.menu_button("View", |ui| {
                        if ui.button("Settings").clicked() {
                            self.ui_state.settings_open = !self.ui_state.settings_open;
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::warn_if_debug_build(ui);
                    });
                });
            });

            egui::TopBottomPanel::bottom("video_control").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(seekable) = &self.seekable {
                        if self.play_state == core::VideoEvent::Play {
                            if ui.add(egui::Button::new("⏸")).clicked() {
                                self.video_sampler.pause().unwrap();
                            }
                        } else {
                            if ui.add(egui::Button::new("⏵")).clicked() {
                                self.video_sampler.play().unwrap();
                            }
                        }

                        let available_size = ui.available_size_before_wrap();
                        let label_size = ui.available_size() / 8.0;
                        let slider_size = available_size - (label_size);

                        ui.label(&self.pts.to_string());
                        ui.spacing_mut().slider_width = slider_size.x;
                        let response = ui.add(
                            egui::Slider::new(&mut self.pts.0, 0..=seekable.end.0)
                                .show_value(false),
                        );
                        if response.drag_released() || response.lost_focus() || response.changed() {
                            self.video_sampler.seek(&self.pts);
                        }
                        ui.label(&seekable.end.to_string());
                    }
                });
            });

            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    if let Some(video_plane) = &self.video_plane {
                        video_plane.show(ui);
                    }
                });
                egui::Window::new("Settings")
                    .open(&mut self.ui_state.settings_open)
                    .show(ctx, |ui| {
                        ui.checkbox(&mut self.ui_state.dark_mode, "Dark Mode");
                        match self.ui_state.dark_mode {
                            true => ctx.set_visuals(egui::Visuals::dark()),
                            false => ctx.set_visuals(egui::Visuals::light()),
                        }
                        let response =
                            ui.add(egui::Slider::new(&mut self.ui_state.scaling, 0.5..=3.0));
                        if response.drag_released() || response.lost_focus() {
                            ctx.set_pixels_per_point(self.ui_state.scaling);
                        }
                    });
            });
        }
        ctx.request_repaint();
    }

    fn on_exit(&mut self) {
        self.video_sampler.stop().unwrap();
    }
}
