use crate::core::{message_bus, CommandLineArguments, Sampler};
use clap::Parser;
use ui::app::BioTrackerUI;

mod core;
mod ui;
mod util;

fn main() {
    let args = CommandLineArguments::parse();
    std::thread::spawn(move || {
        let server = message_bus::Server::new().unwrap();
        server.run().unwrap();
    });

    std::thread::spawn(move || {
        let mut video = Sampler::new(&args).unwrap();
        video.run().unwrap();
    });

    std::thread::spawn(move || {
        let mut tracker = crate::core::tracker::Tracker::new().unwrap();
        tracker.run().unwrap();
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
