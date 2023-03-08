use crate::biotracker::logger::Logger;

pub struct LogView {
    layout_job: egui::text::LayoutJob,
    logger: &'static Logger,
    // Fetching log lines every frame would cause contention on the log rwlock. Instead, we only
    // fetch them every 100 ms.
    next_update: std::time::Instant,
}

impl LogView {
    pub fn new(logger: &'static Logger) -> Self {
        Self {
            layout_job: egui::text::LayoutJob::default(),
            logger,
            next_update: std::time::Instant::now(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let now = std::time::Instant::now();
        if now > self.next_update {
            for line in self.logger.drain_lines() {
                self.layout_job.append(
                    &line.level.to_string(),
                    0.0,
                    egui::text::TextFormat::simple(
                        egui::FontId::default(),
                        Self::level_color(line.level),
                    ),
                );
                self.layout_job.append(
                    &format!(" {} - {}\n", line.target, line.msg),
                    0.0,
                    egui::text::TextFormat::simple(egui::FontId::default(), egui::Color32::BLACK),
                );
            }
            self.next_update = now + std::time::Duration::from_millis(100);
        }
        let mut layouter = |ui: &egui::Ui, _: &str, wrap_width: f32| {
            let mut layout_job: egui::text::LayoutJob = self.layout_job.clone();
            layout_job.wrap.max_width = wrap_width;
            ui.fonts(|f| f.layout_job(layout_job))
        };
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .id_source("log_area")
            .show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut "").layouter(&mut layouter))
            })
            .inner
    }

    fn level_color(level: log::Level) -> egui::Color32 {
        match level {
            log::Level::Error => egui::Color32::from_rgb(248, 79, 49),
            log::Level::Warn => egui::Color32::from_rgb(238, 210, 2),
            log::Level::Info => egui::Color32::from_rgb(35, 197, 82),
            log::Level::Debug => egui::Color32::BLUE,
            log::Level::Trace => egui::Color32::BLUE,
        }
    }
}
