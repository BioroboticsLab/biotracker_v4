use super::{
    annotated_video::AnnotatedVideo,
    annotator::Annotator,
    color::{Palette, ALPHABET},
    controller::BioTrackerController,
    entity_switcher::EntitySwitcher,
    offscreen_renderer::OffscreenRenderer,
    settings::{filemenu, settings_window, video_open_buttons},
};
use crate::biotracker::{protocol::*, CommandLineArguments};
use anyhow::Result;
use std::collections::HashSet;
use std::sync::Arc;
use std::thread::JoinHandle;

pub struct PersistentState {
    pub dark_mode: bool,
    pub scaling: f32,
}

pub struct BioTrackerUIContext {
    pub bt: BioTrackerController,
    pub experiment: Experiment,
    pub persistent_state: PersistentState,
    pub current_frame_number: u32,
    pub seek_target: u32,
    pub render_offscreen: bool,
    pub image_streams: HashSet<String>,
    pub view_image: String,
    pub current_image: Option<Image>,
    pub current_entities: Option<Entities>,
    pub current_features: Option<Features>,
    pub color_palette: Palette,
    pub entity_switcher_open: bool,
    pub annotator_open: bool,
    pub experiment_setup_open: bool,
    pub default_video_encoder_config: VideoEncoderConfig,
}

pub struct BioTrackerUIComponents {
    pub offscreen_renderer: OffscreenRenderer,
    pub video_view: AnnotatedVideo,
    pub entity_switcher: EntitySwitcher,
    pub annotator: Annotator,
}

pub struct BioTrackerUI {
    components: BioTrackerUIComponents,
    context: BioTrackerUIContext,
    core_thread: Option<JoinHandle<()>>,
}

impl BioTrackerUI {
    pub fn new(
        cc: &eframe::CreationContext,
        rt: Arc<tokio::runtime::Runtime>,
        core_thread: JoinHandle<()>,
        args: CommandLineArguments,
    ) -> Option<Self> {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        cc.egui_ctx.set_pixels_per_point(1.5);

        let address = format!("http://127.0.0.1:{}", args.port);
        let bt = BioTrackerController::new(address, rt.clone());

        let persistent_state = PersistentState {
            dark_mode: false,
            scaling: 1.5,
        };

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
            context: BioTrackerUIContext {
                bt,
                experiment: Experiment::default(),
                persistent_state,
                current_frame_number: 0,
                seek_target: 0,
                render_offscreen: false,
                image_streams: HashSet::new(),
                view_image: "Tracking".to_string(),
                current_image: None,
                current_entities: None,
                current_features: None,
                color_palette: Palette { colors: &ALPHABET },
                entity_switcher_open: false,
                annotator_open: false,
                experiment_setup_open: true,
                default_video_encoder_config: VideoEncoderConfig::default(),
            },
            components: BioTrackerUIComponents {
                offscreen_renderer,
                video_view: AnnotatedVideo::new(),
                entity_switcher: EntitySwitcher::default(),
                annotator: Annotator::default(),
            },
            core_thread: Some(core_thread),
        })
    }

    fn update_image(&mut self, frame: &mut eframe::Frame) {
        if let Some(image) = &self.context.experiment.last_image {
            if let Some(current_image) = &self.context.current_image {
                if current_image.frame_number == image.frame_number {
                    return;
                }
            }

            if let Some(encoder_config) = &self.context.experiment.video_encoder_config {
                if encoder_config.image_stream_id == "Annotated" {
                    self.context.render_offscreen = true;
                }
            }

            self.context.current_image = Some(image.clone());
            let render_state = frame.wgpu_render_state().unwrap();
            if self.components.offscreen_renderer.texture.size.width != image.width
                || self.components.offscreen_renderer.texture.size.height != image.height
            {
                self.components.offscreen_renderer = OffscreenRenderer::new(
                    render_state.device.clone(),
                    render_state.queue.clone(),
                    image.width,
                    image.height,
                );
            }
            self.components.video_view.update_image(
                image,
                render_state,
                &self.components.offscreen_renderer.render_state,
            );
            self.context.current_frame_number = image.frame_number;
        }
    }

    fn update_context(&mut self, frame: &mut eframe::Frame) {
        self.context.experiment = self.context.bt.get_state().unwrap();
        self.update_image(frame);
        self.context.current_features = self.context.experiment.last_features.clone();
        self.context.current_entities = self.context.experiment.last_entities.clone();
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) -> Result<()> {
        if ctx.input().key_pressed(egui::Key::ArrowRight) {
            self.context
                .bt
                .command(Command::Seek(self.context.current_frame_number + 1))?;
        }
        if ctx.input().key_pressed(egui::Key::ArrowLeft) {
            if self.context.current_frame_number > 0 {
                self.context
                    .bt
                    .command(Command::Seek(self.context.current_frame_number - 1))?;
            }
        }
        if ctx.input().key_pressed(egui::Key::Space) {
            match PlaybackState::from_i32(self.context.experiment.playback_state).unwrap() {
                PlaybackState::Playing => {
                    self.context
                        .bt
                        .command(Command::PlaybackState(PlaybackState::Paused as i32))?;
                }
                PlaybackState::Paused | PlaybackState::Stopped => {
                    self.context
                        .bt
                        .command(Command::PlaybackState(PlaybackState::Playing as i32))?;
                }
                _ => {}
            }
            self.context
                .bt
                .command(Command::Seek(self.context.current_frame_number - 1))?;
        }
        Ok(())
    }
}

impl eframe::App for BioTrackerUI {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.components
            .video_view
            .update_scale(ctx.input().zoom_delta());
        self.update_context(frame);
        self.handle_shortcuts(ctx).unwrap();

        // Top Toolbar
        egui::TopBottomPanel::top("Toolbar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                let settings_icon = "⛭";
                ui.toggle_value(&mut self.context.experiment_setup_open, settings_icon)
                    .on_hover_text("Open Settings");
                video_open_buttons(ui, &mut self.context);
                ui.separator();
                let switch_icon = "🔀";
                ui.toggle_value(&mut self.context.entity_switcher_open, switch_icon)
                    .on_hover_text("Switch entity IDs");
                let annotator_icon = "📝";
                ui.toggle_value(&mut self.context.annotator_open, annotator_icon)
                    .on_hover_text("Annotation tool");
            });
        });

        // Video controls
        egui::TopBottomPanel::bottom("video_control").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(video_info) = &self.context.experiment.video_info {
                    let frame_count = video_info.frame_count;
                    match PlaybackState::from_i32(self.context.experiment.playback_state).unwrap() {
                        PlaybackState::Playing => {
                            if ui.add(egui::Button::new("⏸")).clicked() {
                                self.context
                                    .bt
                                    .command(Command::PlaybackState(PlaybackState::Paused as i32))
                                    .unwrap();
                            }
                        }
                        _ => {
                            if ui.add(egui::Button::new("▶")).clicked() {
                                self.context
                                    .bt
                                    .command(Command::PlaybackState(PlaybackState::Playing as i32))
                                    .unwrap();
                            }
                        }
                    };

                    let available_size = ui.available_size_before_wrap();
                    let label_size = ui.available_size() / 8.0;
                    let slider_size = available_size - (label_size);

                    ui.label(&self.context.current_frame_number.to_string());
                    ui.spacing_mut().slider_width = slider_size.x;
                    if frame_count > 0 {
                        let response = ui.add(
                            egui::Slider::new(&mut self.context.seek_target, 0..=frame_count)
                                .show_value(false),
                        );
                        if response.drag_released() || response.lost_focus() || response.changed() {
                            self.context
                                .bt
                                .command(Command::Seek(self.context.seek_target))
                                .unwrap();
                        }

                        ui.label(&frame_count.to_string());
                    }
                } else {
                    if ui.add(egui::Button::new("▶")).clicked() {
                        filemenu(&mut self.context);
                    }
                }
            });
        });

        // Video view
        egui::CentralPanel::default().show(ctx, |ui| {
            settings_window(ui, &mut self.context, &mut self.components);
            egui::ScrollArea::both()
                .max_width(f32::INFINITY)
                .max_height(f32::INFINITY)
                .show(ui, |ui| {
                    self.components
                        .video_view
                        .show_onscreen(ui, &mut self.context);
                    self.components.entity_switcher.show(ctx, &mut self.context);
                });
        });

        if self.context.render_offscreen {
            self.components.offscreen_renderer.render(|ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.components
                        .video_view
                        .show_offscreen(ui, &mut self.context);
                });
            });
        }
        ctx.request_repaint();
    }

    fn post_rendering(&mut self, _window_size_px: [u32; 2], _frame: &eframe::Frame) {
        if self.context.render_offscreen {
            let image = self
                .components
                .offscreen_renderer
                .texture_to_image(self.context.current_frame_number)
                .unwrap();
            self.context.render_offscreen = false;
            self.context.bt.add_image(image).unwrap();
        }
    }

    fn on_exit(&mut self) {
        self.context
            .bt
            .command(Command::Shutdown(Empty {}))
            .unwrap();
        match self.core_thread.take().unwrap().join() {
            Ok(_) => {}
            Err(e) => {
                eprintln!("BioTracker core exited with error: {:?}", e);
            }
        }
    }
}
