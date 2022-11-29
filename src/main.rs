use crate::core::{message_bus, CommandLineArguments};
use clap::Parser;
use ui::app::BioTrackerUI;

mod components;
mod core;
mod ui;
mod util;

fn main() {
    let args = std::sync::Arc::new(CommandLineArguments::parse());

    if let Some(topic) = &args.inspect_bus {
        let msg_bus = message_bus::Client::new().unwrap();
        msg_bus.subscribe(topic).unwrap();
        while let Ok(Some(msg)) = msg_bus.poll(-1) {
            eprintln!("{:?}", msg);
        }
        return;
    }

    crate::core::component::run_components(args.clone()).unwrap();

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
        Box::new(|cc| Box::new(BioTrackerUI::new(cc, args).unwrap())),
    );
}
