use util::ScopedTimer;

mod ui;
mod util;
mod video;

struct BioTracker {
    video_sampler: video::Sampler,
    _timer: util::ScopedTimer,
    video_plane: ui::TextureImage,
}

impl BioTracker {
    fn new(cc: &eframe::CreationContext) -> Option<Self> {
        let video_sampler = video::Sampler::new().expect("Failed to create video sampler");
        video_sampler.play().unwrap();
        let wgpu_render_state = cc.wgpu_render_state.as_ref()?;
        let video_plane = ui::TextureImage::new(&wgpu_render_state);

        Some(Self {
            video_sampler,
            _timer: ScopedTimer::new("update_first"),
            video_plane,
        })
    }
}

impl eframe::App for BioTracker {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.video_sampler.poll_event();
        //self._timer = util::ScopedTimer::new("update");
        if let Ok(sample) = self.video_sampler.sample_rx.try_recv() {
            if let Some(data) = sample.data() {
                let render_state = _frame.wgpu_render_state().unwrap();
                self.video_plane.update(&render_state, data.as_slice());
            }
        }

        {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    self.video_plane.show(ui);
                })
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
