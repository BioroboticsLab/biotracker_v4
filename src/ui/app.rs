use crate::{
    core::{message_bus::Client, BufferManager, Message, Timestamp, VideoSeekable, VideoState},
    *,
};

use super::TextureImage;

struct PersistentState {
    settings_open: bool,
    dark_mode: bool,
    scaling: f32,
}

pub struct BioTrackerUI {
    persistent_state: PersistentState,
    msg_bus: Client,
    buffer_manager: BufferManager,
    video_scale: f32,
    play_state: VideoState,
    video_plane: Option<ui::TextureImage>,
    seekable: Option<VideoSeekable>,
    current_pts: Timestamp,
}

impl BioTrackerUI {
    pub fn new(cc: &eframe::CreationContext) -> Option<Self> {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        cc.egui_ctx.set_pixels_per_point(1.5);

        let persistent_state = PersistentState {
            settings_open: false,
            dark_mode: false,
            scaling: 1.5,
        };

        let msg_bus = Client::new().unwrap();
        msg_bus.subscribe("Seekable").unwrap();
        msg_bus.subscribe("Event").unwrap();
        msg_bus.subscribe("Sample").unwrap();
        Some(Self {
            persistent_state,
            msg_bus,
            buffer_manager: BufferManager::new(),
            video_scale: 1.0,
            play_state: VideoState::Stop,
            video_plane: None,
            seekable: None,
            current_pts: Timestamp(0),
        })
    }

    pub fn filemenu(&mut self) {
        if let Some(pathbuf) = rfd::FileDialog::new().pick_file() {
            if let Some(path_str) = pathbuf.to_str() {
                self.msg_bus
                    .send(Message::Command(VideoState::Open(path_str.to_owned())))
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
            self.video_scale *= zoom_delta;
        }
        if let Ok(Some(msg)) = self.msg_bus.poll(0) {
            //eprintln!("Ui: {:?}", msg);
            match msg {
                Message::Sample(sample) => {
                    let image_buffer = self.buffer_manager.get(&sample.id).unwrap();
                    let render_state = frame.wgpu_render_state().unwrap();
                    if let Some(pts) = sample.pts {
                        self.current_pts = pts;
                    }

                    if self.video_plane.is_none() {
                        self.video_plane = Some(TextureImage::new(
                            &render_state,
                            sample.width,
                            sample.height,
                        ));
                    }

                    if let Some(video_plane) = &mut self.video_plane {
                        unsafe {
                            video_plane.update(
                                &render_state,
                                sample.width,
                                sample.height,
                                image_buffer.as_slice(),
                            )
                        }
                    }
                }
                Message::Seekable(seekable) => {
                    self.seekable = Some(seekable);
                }
                Message::Event(video_state) => {
                    self.play_state = video_state;
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
                    ui.menu_button("View", |ui| {
                        if ui.button("Settings").clicked() {
                            self.persistent_state.settings_open = true;
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
                    if let Some(seekable) = &self.seekable {
                        if self.play_state == VideoState::Play {
                            if ui.add(egui::Button::new("⏸")).clicked() {
                                self.msg_bus
                                    .send(Message::Command(VideoState::Pause))
                                    .unwrap();
                            }
                        } else {
                            if ui.add(egui::Button::new("⏵")).clicked() {
                                self.msg_bus
                                    .send(Message::Command(VideoState::Play))
                                    .unwrap();
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
                                .send(Message::Command(VideoState::Seek(self.current_pts)))
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

            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    ui.with_layout(
                        egui::Layout::centered_and_justified(egui::Direction::TopDown),
                        |ui| {
                            if let Some(video_plane) = &self.video_plane {
                                video_plane.show(ui, self.video_scale);
                            }
                        },
                    );
                });
                egui::Window::new("Settings")
                    .open(&mut self.persistent_state.settings_open)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.checkbox(&mut self.persistent_state.dark_mode, "Dark Mode");
                        match self.persistent_state.dark_mode {
                            true => ctx.set_visuals(egui::Visuals::dark()),
                            false => ctx.set_visuals(egui::Visuals::light()),
                        }
                        let response = ui.add(egui::Slider::new(
                            &mut self.persistent_state.scaling,
                            0.5..=3.0,
                        ));
                        if response.drag_released() || response.lost_focus() {
                            ctx.set_pixels_per_point(self.persistent_state.scaling);
                        }
                    });
            });
        }
        ctx.request_repaint();
    }

    fn on_exit(&mut self) {
        self.msg_bus.send(Message::Shutdown).unwrap();
    }
}
