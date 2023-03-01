use super::{
    annotated_video::AnnotatedVideo,
    annotator::Annotator,
    camera_button::CameraButton,
    color::{Palette, ALPHABET},
    controller::BioTrackerController,
    entity_switcher::EntitySwitcher,
    metrics::MetricsPlot,
    record_button::RecordButton,
    settings::{file_open_buttons, open_video, settings_window},
};
use crate::{
    biotracker::{protocol::*, CommandLineArguments},
    util::framenumber_to_hhmmss,
};
use anyhow::Result;
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
    pub image_updated: bool,
    pub current_image: Option<Image>,
    pub color_palette: Palette,
    pub entity_switcher_open: bool,
    pub annotator_open: bool,
    pub experiment_setup_open: bool,
}

pub struct BioTrackerUIComponents {
    pub video_view: AnnotatedVideo,
    pub entity_switcher: EntitySwitcher,
    pub annotator: Annotator,
    pub record_button: RecordButton,
    pub camera_button: CameraButton,
    pub metrics_plot: MetricsPlot,
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
        Some(Self {
            context: BioTrackerUIContext {
                bt,
                experiment: Experiment::default(),
                persistent_state,
                current_frame_number: 0,
                image_updated: false,
                current_image: None,
                color_palette: Palette { colors: &ALPHABET },
                entity_switcher_open: false,
                annotator_open: false,
                experiment_setup_open: false,
            },
            components: BioTrackerUIComponents {
                video_view: AnnotatedVideo::new(render_state),
                entity_switcher: EntitySwitcher::default(),
                annotator: Annotator::default(),
                record_button: RecordButton::default(),
                camera_button: CameraButton::new(),
                metrics_plot: MetricsPlot::new(),
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
                    self.context.image_updated = true;
                }
            }

            self.context.current_image = Some(image.clone());
            let render_state = frame.wgpu_render_state().unwrap();
            self.components.video_view.update_image(image, render_state);
            self.context.current_frame_number = image.frame_number;
        }
    }

    fn update_context(&mut self, frame: &mut eframe::Frame) {
        self.context.experiment = self.context.bt.get_state().unwrap();
        self.update_image(frame);
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) -> Result<()> {
        if ctx.input(|input| input.key_pressed(egui::Key::ArrowRight)) {
            self.context
                .bt
                .command(Command::Seek(self.context.current_frame_number + 1))?;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::ArrowLeft)) {
            if self.context.current_frame_number > 0 {
                self.context
                    .bt
                    .command(Command::Seek(self.context.current_frame_number - 1))?;
            }
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Space)) {
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
        }
        Ok(())
    }
}

impl eframe::App for BioTrackerUI {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.update_context(frame);
        self.handle_shortcuts(ctx).unwrap();

        // Top Toolbar
        egui::TopBottomPanel::top("Toolbar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                file_open_buttons(ui, &mut self.context);
                self.components.camera_button.show(ui, &mut self.context);

                ui.separator();
                let switch_icon = "🔀";
                ui.toggle_value(&mut self.context.entity_switcher_open, switch_icon)
                    .on_hover_text("Switch entity IDs");
                let annotator_icon = "📝";
                ui.toggle_value(&mut self.context.annotator_open, annotator_icon)
                    .on_hover_text("Annotation tool");
                self.components.metrics_plot.show_button(ui);
                let settings_icon = "⛭";
                ui.toggle_value(&mut self.context.experiment_setup_open, settings_icon)
                    .on_hover_text("Open Settings");
            });
        });

        // Video controls
        egui::TopBottomPanel::bottom("video_control").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(video_info) = self.context.experiment.video_info.clone() {
                    let frame_count = video_info.frame_count;
                    self.components.record_button.show(ui, &mut self.context);
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
                    let slider_size = 0.95 * available_size - (label_size * 2.0);

                    let current_frame = &mut self.context.current_frame_number;
                    ui.label(framenumber_to_hhmmss(*current_frame, video_info.fps));
                    ui.spacing_mut().slider_width = slider_size.x;
                    if frame_count > 0 {
                        let response = ui.add(
                            egui::Slider::new(current_frame, 0..=frame_count).show_value(false),
                        );
                        if response.drag_released() || response.lost_focus() || response.changed() {
                            self.context
                                .bt
                                .command(Command::Seek(*current_frame))
                                .unwrap();
                        }
                        ui.label(framenumber_to_hhmmss(frame_count, video_info.fps));
                    }
                } else {
                    if ui.add(egui::Button::new("▶")).clicked() {
                        open_video(&mut self.context);
                    }
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            settings_window(ui, &mut self.context, &mut self.components);
            // Video view
            self.components.video_view.show(ui, &mut self.context);
            self.components.entity_switcher.show(ctx, &mut self.context);
            // Metrics view
            if self.components.metrics_plot.open {
                self.components.metrics_plot.show(ui, &mut self.context);
            }
        });

        ctx.request_repaint();
    }

    fn post_rendering(&mut self, _window_size_px: [u32; 2], _frame: &eframe::Frame) {
        self.components.video_view.post_rendering(&mut self.context);
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
