use super::{side_panel::SidePanel, video_plane::VideoPlane};
use libtracker::{message_bus::Client, CommandLineArguments, Message, Seekable, State, Timestamp};

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
        Some(Self {
            persistent_state,
            msg_bus,
            video_scale: 1.0,
            play_state: State::Stop,
            video_plane: VideoPlane::new(),
            side_panel: SidePanel::new(),
            seekable: None,
            current_pts: Timestamp(0),
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
        while let Ok(Some(msg)) = self.msg_bus.poll(0) {
            match msg {
                Message::Image(img) => {
                    let render_state = frame.wgpu_render_state().unwrap();
                    self.current_pts = img.pts;
                    self.video_plane.update_texture(render_state, &img);
                    break;
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
                    self.video_plane.update_entities(entities);
                }
                _ => panic!("Unexpected message"),
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
                            egui::Slider::new(&mut self.current_pts.0, 0..=seekable.end.0)
                                .show_value(false),
                        );
                        if response.drag_released() || response.lost_focus() || response.changed() {
                            self.msg_bus
                                .send(Message::Command(State::Seek(self.current_pts)))
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
                        self.video_plane.show(ui, self.video_scale);
                    });
            });
        }
        ctx.request_repaint();
    }

    fn on_exit(&mut self) {
        self.msg_bus.send(Message::Shutdown).unwrap();
    }
}
