use crate::core::{message_bus, CommandLineArguments};
use clap::Parser;
use ui::app::BioTrackerUI;

mod components;
mod core;
mod ui;
mod util;

fn main() {
    let args = CommandLineArguments::parse();

    if let Some(topic) = &args.inspect_bus {
        let msg_bus = message_bus::Client::new().unwrap();
        msg_bus.subscribe(topic).unwrap();
        while let Ok(Some(msg)) = msg_bus.poll(-1) {
            eprintln!("{:?}", msg);
        }
        return;
    }

    std::thread::spawn(move || {
        let server = message_bus::Server::new().unwrap();
        server.run().unwrap();
    });

    std::thread::spawn(move || {
        let mut video = components::sampler::Sampler::new(&args).unwrap();
        video.run().unwrap();
    });

    std::thread::spawn(move || {
        let mut tracker = components::Tracker::new().unwrap();
        tracker.run().unwrap();
    });

    std::thread::spawn(move || {
        let mut matcher = components::Matcher::new().unwrap();
        matcher.run().unwrap();
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
