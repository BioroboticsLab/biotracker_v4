use clap::Parser;
use components::biotracker::BioTracker;
use libtracker::{message_bus, CommandLineArguments};
use ui::BioTrackerUI;

mod components;
mod ui;
mod util;

fn main() {
    let args = CommandLineArguments::parse();

    if let Some(topic) = &args.inspect_bus {
        let msg_bus = message_bus::Client::new().unwrap();
        msg_bus.subscribe_str(topic).unwrap();
        loop {
            let msg_result = msg_bus.poll(-1);
            match msg_result {
                Ok(Some(msg)) => eprintln!("{:?}", msg),
                Ok(None) => {}
                Err(e) => eprintln!("Error: {}", e),
            }
        }
    }

    let args_copy = args.clone();
    std::thread::Builder::new()
        .name("BioTracker".to_string())
        .spawn(move || {
            let biotracker = BioTracker::new(&args_copy).unwrap();
            biotracker.run();
        })
        .unwrap();

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
    )
    .unwrap();
}
