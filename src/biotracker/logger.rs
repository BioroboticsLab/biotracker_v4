use log::{Level, Log, Metadata, Record};
use std::sync::{Arc, RwLock};

pub struct LogLine {
    pub level: Level,
    pub target: String,
    pub msg: String,
}

pub struct Logger {
    pub lines: Arc<RwLock<Vec<LogLine>>>,
}

impl Logger {
    pub fn new() -> Self {
        Self {
            lines: Arc::new(RwLock::new(vec![])),
        }
    }

    pub fn drain_lines(&self) -> Vec<LogLine> {
        self.lines.write().unwrap().drain(..).collect()
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
        self.lines.write().unwrap().push(LogLine {
            level: record.level(),
            target: record.target().to_string(),
            msg: record.args().to_string(),
        });
    }

    fn flush(&self) {}
}
