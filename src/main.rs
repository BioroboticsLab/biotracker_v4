use crate::core::BioTracker;
use ui::app::BioTrackerUI;

mod core;
mod ui;
mod util;

fn main() {
    let (ui_tx, core_rx) = std::sync::mpsc::channel();
    let (core_tx, ui_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut biotracker = BioTracker::new(core_tx, core_rx);
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
        Box::new(|cc| Box::new(BioTrackerUI::new(cc, ui_tx, ui_rx).unwrap())),
    );
}
