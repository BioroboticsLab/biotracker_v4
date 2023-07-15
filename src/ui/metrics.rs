use crate::biotracker::metrics_recorder::MetricsRecorder;
use egui::plot::{BoxElem, BoxPlot, BoxSpread, Legend, Plot};
use egui_extras::{Column, TableBuilder};

use super::app::BioTrackerUIContext;

pub struct MetricsPlot {
    pub open: bool,
    metrics: &'static MetricsRecorder,
}

impl MetricsPlot {
    pub fn new(metrics: &'static MetricsRecorder) -> Self {
        Self {
            open: false,
            metrics,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &mut BioTrackerUIContext) {
        let target_fps = ctx.experiment.target_fps;
        self.metrics.update_summaries();
        if self.open {
            let mut box_plots = vec![];
            let mut median_latency = vec![];
            let mut box_position = 0.5;
            egui::Window::new("Tracking Latency").show(ui.ctx(), |ui| {
                self.metrics.visit_summaries(|key, summary, description| {
                    let name = match description {
                        Some(description) => description.text.to_string(),
                        None => key.name().to_string(),
                    };

                    median_latency.push((name.clone(), summary.quantile(0.5).unwrap_or(0.0)));
                    let box_plot = BoxPlot::new(vec![BoxElem::new(
                        box_position,
                        BoxSpread::new(
                            summary.min(),
                            summary.quantile(0.25).unwrap_or(0.0),
                            summary.quantile(0.5).unwrap_or(0.0),
                            summary.quantile(0.75).unwrap_or(0.0),
                            summary.max(),
                        ),
                    )
                    .name(name.as_str())])
                    .name(name.as_str());
                    box_plots.push(box_plot);
                    box_position += 0.5;
                });

                // show median frequency values in a table with three columns
                // | key | median frequency (hz) | health indicator
                TableBuilder::new(ui)
                    .column(Column::auto().resizable(true))
                    .column(Column::auto().resizable(true))
                    .column(Column::auto().resizable(true))
                    .column(Column::remainder())
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.heading("Metric");
                        });
                        header.col(|ui| {
                            ui.heading("Latency (ms)");
                        });
                        header.col(|ui| {
                            ui.heading("Frequency (hz)");
                        });
                        header.col(|ui| {
                            ui.heading("Health");
                        });
                    })
                    .body(|mut body| {
                        for (name, latency) in median_latency {
                            let fps = 1.0 / latency;
                            body.row(30.0, |mut row| {
                                row.col(|ui| {
                                    ui.label(name);
                                });
                                row.col(|ui| {
                                    ui.label(format!("{:.2}", latency * 1000.0));
                                });
                                row.col(|ui| {
                                    ui.label(format!("{:.2}", fps));
                                });
                                row.col(|ui| {
                                    let target_fps_ratio = target_fps as f64 / fps;
                                    let mut color = egui::Color32::GREEN;
                                    if target_fps_ratio > 1.03 {
                                        color = egui::Color32::YELLOW;
                                    }
                                    if target_fps_ratio > 1.1 {
                                        color = egui::Color32::RED
                                    }

                                    ui.label(egui::RichText::new("⏺").color(color));
                                });
                            });
                        }
                    });

                ui.separator();
                Plot::new("Latency")
                    .legend(Legend::default())
                    .show(ui, |plot_ui| {
                        for box_plot in box_plots {
                            plot_ui.box_plot(box_plot);
                        }
                    });
                ui.end_row();
            });
        }
    }

    pub fn show_button(&mut self, ui: &mut egui::Ui) {
        let chart_icon = "🗠";
        ui.toggle_value(&mut self.open, chart_icon)
            .on_hover_text("Show Tracking Metrics");
    }
}
