use anyhow::Result;
use clap::Parser;
use components::{Matcher, PythonRunner, Sampler};
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
        loop {
            let msg_result = msg_bus.poll(-1);
            match msg_result {
                Ok(Some(msg)) => eprintln!("{:?}", msg),
                Ok(None) => {}
                Err(e) => eprintln!("Error: {}", e),
            }
        }
    }

    let _ = || -> Result<ComponentRunner> {
        let args = args.clone();
        let mut component_runner = libtracker::component::ComponentRunner::new().unwrap();
        if let Some((venv, cmd)) = args
            .tracker_venv_path
            .as_ref()
            .zip(args.tracker_cmd_path.as_ref())
        {
            let (venv, cmd) = (venv.clone(), cmd.clone());
            component_runner.add_component(|_| PythonRunner::new(venv, cmd))?;
        }
        if args.tracker_venv_path.is_some() && args.tracker_cmd_path.is_some() {
            let venv_path = args.tracker_venv_path.clone();
            let cmd_path = args.tracker_cmd_path.clone();
            component_runner
                .add_component(|_| PythonRunner::new(venv_path.unwrap(), cmd_path.unwrap()))?;
        }
        component_runner.add_component(|msg_bus| Matcher::new(msg_bus, args))?;
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
