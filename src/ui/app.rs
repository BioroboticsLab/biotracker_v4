use crate::*;

struct UiState {
    settings_open: bool,
    dark_mode: bool,
    scaling: f32,
    video_scale: f32,
}

pub struct BioTracker {
    video_sampler: Option<core::Sampler>,
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
        if let Some(render_state) = &cc.wgpu_render_state {
            render_state.device.limits().max_texture_dimension_2d = 8192;
        }

        Some(Self {
            video_sampler: None,
            play_state: core::VideoEvent::Play,
            video_plane: None,
            seekable: None,
            pts: core::Timestamp(0),
            ui_state: UiState {
                dark_mode: false,
                scaling: 1.5,
                settings_open: false,
                video_scale: 1.0,
            },
        })
    }

    pub fn filemenu(&mut self) {
        if let Some(pathbuf) = rfd::FileDialog::new().pick_file() {
            if let Some(path_str) = pathbuf.to_str() {
                let sampler = core::Sampler::new(path_str).expect("Failed to create video sampler");
                sampler.play().unwrap();
                if let Some(old_sampler) = self.video_sampler.take() {
                    old_sampler.stop().unwrap();
                }
                self.seekable = None;
                self.video_sampler = Some(sampler);
                self.video_plane = None;
            } else {
                eprintln!("Failed to get unicode string from pathbuf");
            }
        }
    }
}

impl eframe::App for BioTracker {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let zoom_delta = ctx.input().zoom_delta();
        if zoom_delta != 1.0 {
            self.ui_state.video_scale *= zoom_delta;
        }
        let render_state = frame.wgpu_render_state().unwrap();
        if let Some(sampler) = &mut self.video_sampler {
            if let Some(sampler_event) = sampler.poll_event() {
                match sampler_event {
                    core::SamplerEvent::Seekable(seekable) => {
                        self.seekable = Some(seekable);
                    }
                    core::SamplerEvent::Event(event) => {
                        self.play_state = event;
                    }
                }
            }
            if let Ok(sample) = sampler.sample_rx.try_recv() {
                if let Some(caps) = sample.sample.caps() {
                    let gst_info = gst_video::VideoInfo::from_caps(&caps).unwrap();
                    if self.video_plane.is_none() {
                        self.video_plane = Some(ui::TextureImage::new(
                            &render_state,
                            wgpu::Extent3d {
                                width: gst_info.width(),
                                height: gst_info.height(),
                                depth_or_array_layers: 1,
                            },
                        ));
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
        }

        {
            egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Open Media").clicked() {
                            self.filemenu();
                            ui.close_menu();
                        }
                        if ui.button("Quit").clicked() {
                            frame.close();
                        }
                    });
                    ui.menu_button("View", |ui| {
                        if ui.button("Settings").clicked() {
                            self.ui_state.settings_open = true;
                            ui.close_menu();
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::warn_if_debug_build(ui);
                    });
                });
            });

            egui::TopBottomPanel::bottom("video_control").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(sampler) = &self.video_sampler {
                        if let Some(seekable) = &self.seekable {
                            if self.play_state == core::VideoEvent::Play {
                                if ui.add(egui::Button::new("⏸")).clicked() {
                                    sampler.pause().unwrap();
                                }
                            } else {
                                if ui.add(egui::Button::new("⏵")).clicked() {
                                    sampler.play().unwrap();
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
                            if response.drag_released()
                                || response.lost_focus()
                                || response.changed()
                            {
                                sampler.seek(&self.pts);
                            }
                            ui.label(&seekable.end.to_string());
                        }
                    } else {
                        if ui.add(egui::Button::new("⏵")).clicked() {
                            self.filemenu();
                        }
                    }
                });
            });

            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    ui.with_layout(
                        egui::Layout::centered_and_justified(egui::Direction::TopDown),
                        |ui| {
                            if let Some(video_plane) = &self.video_plane {
                                video_plane.show(ui, self.ui_state.video_scale);
                            }
                        },
                    );
                });
                egui::Window::new("Settings")
                    .open(&mut self.ui_state.settings_open)
                    .resizable(false)
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
        if let Some(sampler) = &self.video_sampler {
            sampler.stop().unwrap();
        }
    }
}
