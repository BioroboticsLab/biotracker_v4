use super::{
    offscreen_renderer::OffscreenRenderer, side_panel::SidePanel, texture::Texture,
    video_plane::VideoPlane,
};
use egui_wgpu::wgpu;
use libtracker::{
    message_bus::Client, CommandLineArguments, Message, Seekable, SharedBuffer, State, Timestamp,
};

pub struct PersistentState {
    pub dark_mode: bool,
    pub scaling: f32,
}

pub struct BioTrackerUI {
    persistent_state: PersistentState,
    msg_bus: Client,
    video_scale: f32,
    play_state: State,
    video_plane: VideoPlane,
    side_panel: SidePanel,
    seekable: Option<Seekable>,
    current_pts: Timestamp,
    seek_pts: Timestamp,
    offscreen_renderer: OffscreenRenderer,
    texture: Option<Texture>,
    onscreen_id: egui::TextureId,
    offscreen_id: egui::TextureId,
    entities_received: bool,
}

impl BioTrackerUI {
    pub fn new(cc: &eframe::CreationContext, args: CommandLineArguments) -> Option<Self> {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        cc.egui_ctx.set_pixels_per_point(1.5);

        let persistent_state = PersistentState {
            dark_mode: false,
            scaling: 1.5,
        };

        let msg_bus = Client::new().unwrap();
        msg_bus.subscribe("Seekable").unwrap();
        msg_bus.subscribe("Event").unwrap();
        msg_bus.subscribe("Image").unwrap();
        msg_bus.subscribe("Feature").unwrap();
        msg_bus.subscribe("Entities").unwrap();
        if let Some(path) = &args.video {
            msg_bus
                .send(Message::Command(State::Open(path.to_owned())))
                .unwrap();
        }

        let render_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("WGPU render state not available");
        let offscreen_renderer =
            OffscreenRenderer::new(render_state.device.clone(), render_state.queue.clone());

        Some(Self {
            persistent_state,
            msg_bus,
            video_scale: 1.0,
            play_state: State::Stop,
            video_plane: VideoPlane::new(),
            side_panel: SidePanel::new(),
            seekable: None,
            current_pts: Timestamp(0),
            seek_pts: Timestamp(0),
            offscreen_renderer,
            texture: None,
            onscreen_id: egui::epaint::TextureId::default(),
            offscreen_id: egui::epaint::TextureId::default(),
            entities_received: false,
        })
    }

    pub fn filemenu(&mut self) {
        if let Some(pathbuf) = rfd::FileDialog::new().pick_file() {
            if let Some(path_str) = pathbuf.to_str() {
                self.msg_bus
                    .send(Message::Command(State::Open(path_str.to_owned())))
                    .unwrap();
            } else {
                eprintln!("Failed to get unicode string from pathbuf");
            }
        }
    }
}

impl eframe::App for BioTrackerUI {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let zoom_delta = ctx.input().zoom_delta();
        if zoom_delta != 1.0 {
            self.video_scale = 0.1f32.max(self.video_scale * zoom_delta);
        }
        let mut last_image = None;
        while let Ok(Some(msg)) = self.msg_bus.poll(0) {
            match msg {
                Message::Image(img) => {
                    last_image = Some(img);
                }
                Message::Seekable(seekable) => {
                    self.seekable = Some(seekable);
                }
                Message::Event(video_state) => {
                    match video_state {
                        State::Open(_) => {
                            self.video_plane = VideoPlane::new();
                            self.seekable = None;
                        }
                        _ => {}
                    }
                    self.play_state = video_state;
                }
                Message::Features(features) => {
                    self.video_plane.update_features(features);
                }
                Message::Entities(entities) => {
                    self.entities_received = true;
                    self.video_plane.update_entities(entities);
                }
                _ => eprintln!("Unexpected message {:?}", msg),
            }
        }

        // we defer actually reading the image until after the message queue is drained. This way,
        // we always skip to the newest frame. This happens, when the application does not render,
        // because it is minimised.
        if let Some(img) = last_image {
            self.current_pts = img.pts;
            let render_state = frame.wgpu_render_state().unwrap();
            let image_buffer = SharedBuffer::open(&img.shm_id).unwrap();

            let mut reinitialize_texture = self.texture.is_none();
            if let Some(texture) = &mut self.texture {
                if texture.size.width != img.width || texture.size.height != img.height {
                    reinitialize_texture = true;
                }
            }

            if reinitialize_texture {
                let texture = Texture::new(
                    &render_state.device,
                    img.width,
                    img.height,
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    Some("egui_video_texture"),
                );
                self.onscreen_id =
                    texture.egui_register(&render_state.device, &render_state.renderer);
                self.offscreen_id = texture.egui_register(
                    &render_state.device,
                    &self.offscreen_renderer.render_state.renderer,
                );
                self.texture = Some(texture);
            }

            unsafe {
                self.texture
                    .as_mut()
                    .expect("Texture not initialized")
                    .update(
                        &render_state.queue,
                        img.width,
                        img.height,
                        image_buffer.as_slice(),
                    )
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
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::warn_if_debug_build(ui);
                    });
                });
            });

            egui::TopBottomPanel::bottom("video_control").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(seekable) = &self.seekable {
                        if self.play_state == State::Play {
                            if ui.add(egui::Button::new("⏸")).clicked() {
                                self.msg_bus.send(Message::Command(State::Pause)).unwrap();
                            }
                        } else {
                            if ui.add(egui::Button::new("⏵")).clicked() {
                                self.msg_bus.send(Message::Command(State::Play)).unwrap();
                            }
                        }

                        let available_size = ui.available_size_before_wrap();
                        let label_size = ui.available_size() / 8.0;
                        let slider_size = available_size - (label_size);

                        ui.label(&self.current_pts.to_string());
                        ui.spacing_mut().slider_width = slider_size.x;
                        let response = ui.add(
                            egui::Slider::new(&mut self.seek_pts.0, 0..=seekable.end.0)
                                .show_value(false),
                        );
                        if response.drag_released() || response.lost_focus() || response.changed() {
                            self.msg_bus
                                .send(Message::Command(State::Seek(self.seek_pts)))
                                .unwrap();
                        }
                        ui.label(&seekable.end.to_string());
                    } else {
                        if ui.add(egui::Button::new("⏵")).clicked() {
                            self.filemenu();
                        }
                    }
                });
            });

            self.side_panel.show(
                ctx,
                &mut self.msg_bus,
                &mut self.persistent_state,
                &mut self.video_plane,
            );

            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::both()
                    .max_width(f32::INFINITY)
                    .max_height(f32::INFINITY)
                    .show(ui, |ui| {
                        self.video_plane.show(
                            ui,
                            Some(self.video_scale),
                            &self.texture,
                            self.onscreen_id,
                        );
                    });
            });

            if self.entities_received {
                self.offscreen_renderer.render(|offscreen_ctx| {
                    egui::CentralPanel::default().show(offscreen_ctx, |ui| {
                        self.video_plane
                            .show(ui, None, &self.texture, self.offscreen_id);
                    });
                });
            }
            ctx.request_repaint();
        }
    }

    fn post_rendering(&mut self, _window_size_px: [u32; 2], _frame: &eframe::Frame) {
        if self.entities_received {
            self.offscreen_renderer
                .post_rendering(&self.msg_bus, &self.current_pts)
                .unwrap();
            self.entities_received = false;
        }
    }

    fn on_exit(&mut self) {
        self.msg_bus.send(Message::Shutdown).unwrap();
    }
}
