use util::ScopedTimer;

mod ui;
mod util;
mod video;

struct UiState {
    settings_open: bool,
    dark_mode: bool,
    scaling: f32,
}

struct BioTracker {
    video_sampler: video::Sampler,
    _timer: util::ScopedTimer,
    video_plane: ui::TextureImage,
    ui_state: UiState,
}

impl BioTracker {
    fn new(cc: &eframe::CreationContext) -> Option<Self> {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        cc.egui_ctx.set_pixels_per_point(1.5);
        let video_sampler = video::Sampler::new().expect("Failed to create video sampler");
        video_sampler.play().unwrap();
        let wgpu_render_state = cc.wgpu_render_state.as_ref()?;
        let video_plane = ui::TextureImage::new(&wgpu_render_state);

        Some(Self {
            video_sampler,
            _timer: ScopedTimer::new("update_first"),
            video_plane,
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
        self.video_sampler.poll_event();
        if let Ok(sample) = self.video_sampler.sample_rx.try_recv() {
            if let Some(data) = sample.data() {
                let render_state = frame.wgpu_render_state().unwrap();
                self.video_plane.update(&render_state, data.as_slice());
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
                    egui::warn_if_debug_build(ui);
                });
            });

            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    self.video_plane.show(ui);
                });
                egui::Window::new("Settings")
                    .open(&mut self.ui_state.settings_open)
                    .show(ctx, |ui| {
                        ui.checkbox(&mut self.ui_state.dark_mode, "Dark Mode");
                        match self.ui_state.dark_mode {
                            true => ctx.set_visuals(egui::Visuals::dark()),
                            false => ctx.set_visuals(egui::Visuals::light()),
                        }
                        ui.add(egui::Slider::new(&mut self.ui_state.scaling, 0.5..=3.0));
                        ctx.set_pixels_per_point(self.ui_state.scaling);
                    });
            });
        }
        ctx.request_repaint();
    }

    fn on_exit(&mut self) {
        self.video_sampler.stop().unwrap();
    }
}

fn main() {
    let options = eframe::NativeOptions {
        drag_and_drop_support: true,
        initial_window_size: Some([1280.0, 1024.0].into()),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "BioTracker",
        options,
        Box::new(|cc| Box::new(BioTracker::new(cc).unwrap())),
    );
}
