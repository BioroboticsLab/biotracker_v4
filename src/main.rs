use biotracker::{logger::LOGGER, CommandLineArguments, Core};
use clap::Parser;
use std::sync::Arc;
use ui::BioTrackerUI;

mod biotracker;
mod ui;
mod util;

fn main() {
    let args = CommandLineArguments::parse();
    cv::core::set_num_threads(args.cv_worker_threads as i32).unwrap();

    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(log::LevelFilter::Warn))
        .unwrap();

    let args_copy = args.clone();

    let rt = Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap(),
    );
    let rt_clone = rt.clone();
    let core_thread = std::thread::Builder::new()
        .name("BioTrackerCore".to_string())
        .spawn(move || {
            rt.block_on(async move {
                match Core::new(&args).await {
                    Ok(core) => {
                        core.run().await.unwrap();
                    }
                    Err(e) => {
                        println!("Failed to start BioTracker Core: {}", e);
                    }
                }
            })
        })
        .unwrap();

    eframe::run_native(
        "BioTracker",
        eframe::NativeOptions {
            drag_and_drop_support: true,
            initial_window_size: Some([1280.0, 1024.0].into()),
            renderer: eframe::Renderer::Wgpu,
            wgpu_options: egui_wgpu::WgpuConfiguration {
                power_preference: egui_wgpu::wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            },
            ..Default::default()
        },
        Box::new(|cc| Box::new(BioTrackerUI::new(cc, rt_clone, core_thread, args_copy).unwrap())),
    )
    .unwrap();
}
