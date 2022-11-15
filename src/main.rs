use crate::core::message_bus;
use crate::core::BioTracker;
use ui::app::BioTrackerUI;

mod core;
mod ui;
mod util;

fn main() {
    std::thread::spawn(move || {
        let server = message_bus::Server::new().unwrap();
        server.run().unwrap();
    });

    std::thread::spawn(move || {
        let mut biotracker = BioTracker::new().unwrap();
        biotracker.run();
    });

    let options = eframe::NativeOptions {
        drag_and_drop_support: true,
        initial_window_size: Some([1280.0, 1024.0].into()),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: egui_wgpu::WgpuConfiguration {
            power_preference: egui_wgpu::wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        },
        ..Default::default()
    };

    eframe::run_native(
        "BioTracker",
        options,
        Box::new(|cc| Box::new(BioTrackerUI::new(cc).unwrap())),
    );
}
