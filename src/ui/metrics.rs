use crate::biotracker::metric::DurationMetric;

use super::app::BioTrackerUIContext;

pub struct MetricsPlot {
    pub open: bool,
    pub ui_frame_time: DurationMetric,
}

impl MetricsPlot {
    pub fn new() -> Self {
        Self {
            open: false,
            ui_frame_time: DurationMetric::default(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &mut BioTrackerUIContext) {
        egui::Window::new("Tracking Metrics").show(ui.ctx(), |ui| {
            let metrics = ctx.experiment.tracking_metrics.as_ref().unwrap();
            ui.label(format!(
                "Tracking FPS: {:.1}",
                1.0 / metrics.tracking_frame_time
            ));
            ui.label(format!(
                "Playback FPS: {:.1}",
                1.0 / metrics.playback_frame_time
            ));
            ui.label(format!(
                "UI FPS: {}",
                (1.0 / self.ui_frame_time.update()) as u32
            ));
            ui.label(format!("Features: {}", metrics.detected_features));
            ui.label(format!(
                "Encoder dropped frames: {}",
                metrics.encoder_dropped_frames
            ));
            ui.label(format!(
                "Playback dropped frames: {}",
                metrics.playback_dropped_frames
            ));
        });
    }

    pub fn show_button(&mut self, ui: &mut egui::Ui) {
        let chart_icon = "🗠";
        ui.toggle_value(&mut self.open, chart_icon)
            .on_hover_text("Show Tracking Metrics");
    }
}
