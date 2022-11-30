use anyhow::Result;
use clap::Parser;
use components::{Matcher, Sampler, Tracker};
use libtracker::{component::ComponentRunner, message_bus, CommandLineArguments};
use ui::BioTrackerUI;

mod components;
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
    let _ = || -> Result<ComponentRunner> {
        let args_copy = args.clone();
        let mut component_runner = libtracker::component::ComponentRunner::new().unwrap();
        component_runner.add_component(|msg_bus| Tracker::new(msg_bus))?;
        component_runner.add_component(|msg_bus| Matcher::new(msg_bus, args_copy))?;
        component_runner.add_component(|msg_bus| Sampler::new(msg_bus))?;
        Ok(component_runner)
    }()
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
    );
}
